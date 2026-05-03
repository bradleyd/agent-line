use crate::agent::StepError;
use std::{env, fmt, sync::Arc};

/// Reusable LLM configuration. Each agent that needs an LLM holds its own
/// `LlmConfig` and calls [`LlmConfig::request`] to start a chat request.
///
/// Build one with [`LlmConfig::builder`] for explicit settings, or with
/// [`LlmConfig::from_env`] to read from `AGENT_LINE_*` environment variables.
/// Multiple agents can share one config or each hold their own (cheap fast
/// model for one step, strong reasoning model for another).
#[derive(Clone, PartialEq, Eq)]
pub struct LlmConfig {
    base_url: String,
    model: String,
    num_ctx: u32,
    max_tokens: u32,
    api_key: Option<String>,
    provider: Provider,
}

impl fmt::Debug for LlmConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LlmConfig")
            .field("provider", &self.provider)
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("num_ctx", &self.num_ctx)
            .field("max_tokens", &self.max_tokens)
            .field(
                "api_key",
                &if self.api_key.is_some() {
                    "set"
                } else {
                    "not set"
                },
            )
            .finish()
    }
}

/// Error returned when building an [`LlmConfig`] without required fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmConfigError {
    /// No provider was configured.
    MissingProvider,
    /// No base URL was configured.
    MissingBaseUrl,
    /// No model name was configured.
    MissingModel,
}

impl fmt::Display for LlmConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingProvider => write!(f, "LlmConfig missing provider"),
            Self::MissingBaseUrl => write!(f, "LlmConfig missing base_url"),
            Self::MissingModel => write!(f, "LlmConfig missing model"),
        }
    }
}

impl std::error::Error for LlmConfigError {}

/// Builder for [`LlmConfig`].
#[derive(Default)]
pub struct LlmConfigBuilder {
    provider: Option<Provider>,
    base_url: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    num_ctx: Option<u32>,
    max_tokens: Option<u32>,
}

/// Builder for LLM chat requests. Obtained via [`LlmConfig::request`].
pub struct LlmRequestBuilder {
    config: Arc<LlmConfig>,
    system: Option<String>,
    messages: Vec<String>,
}

/// LLM provider. Selected via [`LlmConfigBuilder::provider`] or the
/// `AGENT_LINE_PROVIDER` env var when using [`LlmConfig::from_env`].
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    /// Ollama (default). Local inference, no API key needed.
    Ollama,
    /// OpenAI-compatible APIs (OpenRouter, etc.).
    OpenAi,
    /// Anthropic API.
    Anthropic,
}

impl Provider {
    /// Parse a provider name. Unrecognized values default to Ollama.
    pub(crate) fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "openai" => Provider::OpenAi,
            "anthropic" => Provider::Anthropic,
            _ => Provider::Ollama,
        }
    }

    pub(crate) fn endpoint(&self, base_url: &str) -> String {
        let base = base_url.trim_end_matches('/');
        match self {
            Provider::Ollama => format!("{base}/api/chat"),
            Provider::OpenAi => format!("{base}/v1/chat/completions"),
            Provider::Anthropic => format!("{base}/v1/messages"),
        }
    }

    pub(crate) fn parse_response(&self, json: &serde_json::Value) -> Result<String, StepError> {
        let content = match self {
            Provider::Ollama => json["message"]["content"].as_str(),
            Provider::OpenAi => json["choices"][0]["message"]["content"].as_str(),
            Provider::Anthropic => json["content"][0]["text"].as_str(),
        };
        content
            .map(|s| s.to_string())
            .ok_or_else(|| StepError::other("llm response missing message content"))
    }
}

impl LlmConfig {
    /// Start building an explicit LLM configuration.
    pub fn builder() -> LlmConfigBuilder {
        LlmConfigBuilder::default()
    }

    /// Build an LLM configuration from `AGENT_LINE_*` environment variables.
    ///
    /// Reads `AGENT_LINE_PROVIDER`, `AGENT_LINE_LLM_URL`, `AGENT_LINE_MODEL`,
    /// `AGENT_LINE_API_KEY`, `AGENT_LINE_NUM_CTX` (Ollama context window),
    /// and `AGENT_LINE_MAX_TOKENS` (OpenAI/Anthropic response cap; falls back
    /// to `AGENT_LINE_NUM_CTX` if unset). Defaults to a local Ollama
    /// configuration when nothing is set.
    ///
    /// If `AGENT_LINE_DEBUG` is set, the resolved config is logged to stderr
    /// once.
    pub fn from_env() -> Self {
        let num_ctx = match env::var("AGENT_LINE_NUM_CTX") {
            Ok(v) => v.parse::<u32>().unwrap_or(4096),
            Err(_) => 4096,
        };
        let max_tokens = match env::var("AGENT_LINE_MAX_TOKENS") {
            Ok(v) => v.parse::<u32>().unwrap_or(num_ctx),
            Err(_) => num_ctx,
        };

        let config = Self {
            provider: Provider::from_str(
                &env::var("AGENT_LINE_PROVIDER").unwrap_or_else(|_| "ollama".to_string()),
            ),
            base_url: env::var("AGENT_LINE_LLM_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            model: env::var("AGENT_LINE_MODEL").unwrap_or_else(|_| "llama3.1:8b".to_string()),
            api_key: env::var("AGENT_LINE_API_KEY").ok(),
            num_ctx,
            max_tokens,
        };
        config.debug_log();
        config
    }

    /// Return a copy of this config with a different model name. All other
    /// fields (provider, base URL, API key, token budgets) are preserved.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Start building an LLM chat request that uses this config.
    ///
    /// Each call creates a fresh [`LlmRequestBuilder`]; chain `.system()`,
    /// `.user()`, and `.send()` on the result. The config itself is not
    /// consumed, so an agent can call `self.llm.request()` repeatedly.
    pub fn request(&self) -> LlmRequestBuilder {
        LlmRequestBuilder {
            config: Arc::new(self.clone()),
            system: None,
            messages: Vec::new(),
        }
    }

    fn debug_log(&self) {
        if env::var("AGENT_LINE_DEBUG").is_ok() {
            eprintln!(
                "[debug] provider: {:?}\n\
                 [debug] model: {}\n\
                 [debug] base_url: {}\n\
                 [debug] num_ctx: {}\n\
                 [debug] max_tokens: {}\n\
                 [debug] api_key: {}",
                self.provider,
                self.model,
                self.base_url,
                self.num_ctx,
                self.max_tokens,
                if self.api_key.is_some() {
                    "set"
                } else {
                    "not set"
                },
            );
        }
    }
}

impl LlmConfigBuilder {
    /// Set the LLM provider. Required.
    pub fn provider(mut self, provider: Provider) -> Self {
        self.provider = Some(provider);
        self
    }

    /// Set the base URL of the LLM endpoint. Required.
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    /// Set the model name. Required.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set the API key. Optional for local providers.
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Set the context window size sent in the `options.num_ctx` field of
    /// Ollama requests. Defaults to 4096. Ignored by OpenAI-compatible and
    /// Anthropic providers; use [`max_tokens`](Self::max_tokens) for those.
    pub fn num_ctx(mut self, num_ctx: u32) -> Self {
        self.num_ctx = Some(num_ctx);
        self
    }

    /// Set the maximum number of generated tokens sent in the `max_tokens`
    /// field of OpenAI-compatible and Anthropic requests. Defaults to 4096.
    /// Ignored by Ollama; use [`num_ctx`](Self::num_ctx) for that.
    pub fn max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Build the [`LlmConfig`].
    pub fn build(self) -> Result<LlmConfig, LlmConfigError> {
        Ok(LlmConfig {
            provider: self.provider.ok_or(LlmConfigError::MissingProvider)?,
            base_url: self.base_url.ok_or(LlmConfigError::MissingBaseUrl)?,
            model: self.model.ok_or(LlmConfigError::MissingModel)?,
            api_key: self.api_key,
            num_ctx: self.num_ctx.unwrap_or(4096),
            max_tokens: self.max_tokens.unwrap_or(4096),
        })
    }
}

impl LlmRequestBuilder {
    /// Set the system prompt.
    pub fn system(mut self, msg: &str) -> Self {
        self.system = Some(msg.to_string());
        self
    }

    /// Append a user message.
    pub fn user(mut self, msg: impl Into<String>) -> Self {
        self.messages.push(msg.into());
        self
    }

    /// Send the request and return the assistant's response text.
    pub fn send(self) -> Result<String, StepError> {
        let mut messages = Vec::new();

        if let Some(sys) = &self.system {
            messages.push(serde_json::json!({
                "role": "system",
                "content": sys
            }));
        }

        for msg in &self.messages {
            messages.push(serde_json::json!({
                "role": "user",
                "content": msg
            }));
        }

        let body = match &self.config.provider {
            Provider::Ollama => serde_json::json!({
                "model": self.config.model,
                "messages": messages,
                "stream": false,
                // Disable Qwen 3-style "thinking" tokens. Thinking models can
                // otherwise spend minutes generating <think>...</think>
                // reasoning before producing the actual response, which is
                // rarely what an agentic workflow wants. Ignored by models
                // that do not support thinking.
                "think": false,
                "options": {
                    "num_ctx": self.config.num_ctx
                }
            }),
            Provider::OpenAi => serde_json::json!({
                "model": self.config.model,
                "messages": messages,
                "stream": false,
                "max_tokens": self.config.max_tokens
            }),
            Provider::Anthropic => serde_json::json!({
                "model": self.config.model,
                "messages": messages,
                "stream": false,
                "max_tokens": self.config.max_tokens
            }),
        };

        let url = self.config.provider.endpoint(&self.config.base_url);
        let mut request = ureq::post(&url);

        match &self.config.provider {
            Provider::Anthropic => {
                if let Some(key) = &self.config.api_key {
                    request = request.header("x-api-key", key);
                }
                request = request.header("anthropic-version", "2023-06-01");
                request = request.header("content-type", "application/json");
            }
            _ => {
                if let Some(key) = &self.config.api_key {
                    request = request.header("Authorization", &format!("Bearer {key}"));
                }
            }
        }

        if std::env::var("AGENT_LINE_DEBUG").is_ok() {
            eprintln!("[debug] LLM request to {}", url);
            eprintln!(
                "[debug] Messages: {}",
                serde_json::to_string_pretty(&messages).unwrap_or_default()
            );
        }

        let mut response = request
            .send_json(&body)
            .map_err(|e| StepError::transient(format!("llm request failed: {e}")))?;

        let json: serde_json::Value = response
            .body_mut()
            .read_json()
            .map_err(|e| StepError::transient(format!("llm response parse failed: {e}")))?;

        if std::env::var("AGENT_LINE_DEBUG").is_ok() {
            eprintln!("[debug] LLM response: {}", &json);
        }

        self.config.provider.parse_response(&json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Provider::from_str ---

    #[test]
    fn test_provider_from_str_ollama() {
        assert_eq!(Provider::from_str("ollama"), Provider::Ollama);
    }

    #[test]
    fn test_provider_from_str_openai() {
        assert_eq!(Provider::from_str("openai"), Provider::OpenAi);
    }

    #[test]
    fn test_provider_from_str_anthropic() {
        assert_eq!(Provider::from_str("anthropic"), Provider::Anthropic);
    }

    #[test]
    fn test_provider_from_str_case_insensitive() {
        assert_eq!(Provider::from_str("OpenAI"), Provider::OpenAi);
        assert_eq!(Provider::from_str("ANTHROPIC"), Provider::Anthropic);
        assert_eq!(Provider::from_str("Ollama"), Provider::Ollama);
    }

    #[test]
    fn test_provider_from_str_unknown_defaults_to_ollama() {
        assert_eq!(Provider::from_str("something"), Provider::Ollama);
    }

    // --- Provider::endpoint ---

    #[test]
    fn test_ollama_endpoint() {
        assert_eq!(
            Provider::Ollama.endpoint("http://localhost:11434"),
            "http://localhost:11434/api/chat"
        );
    }

    #[test]
    fn test_openai_endpoint() {
        assert_eq!(
            Provider::OpenAi.endpoint("https://openrouter.ai"),
            "https://openrouter.ai/v1/chat/completions"
        );
    }

    #[test]
    fn test_anthropic_endpoint() {
        assert_eq!(
            Provider::Anthropic.endpoint("https://api.anthropic.com"),
            "https://api.anthropic.com/v1/messages"
        );
    }

    #[test]
    fn test_endpoint_strips_trailing_slash() {
        assert_eq!(
            Provider::OpenAi.endpoint("https://openrouter.ai/"),
            "https://openrouter.ai/v1/chat/completions"
        );
    }

    // --- Provider::parse_response ---

    #[test]
    fn test_ollama_parse_response() {
        let json = serde_json::json!({
            "message": { "content": "Hello from Ollama" }
        });
        assert_eq!(
            Provider::Ollama.parse_response(&json).unwrap(),
            "Hello from Ollama"
        );
    }

    #[test]
    fn test_openai_parse_response() {
        let json = serde_json::json!({
            "choices": [{ "message": { "content": "Hello from OpenRouter" } }]
        });
        assert_eq!(
            Provider::OpenAi.parse_response(&json).unwrap(),
            "Hello from OpenRouter"
        );
    }

    #[test]
    fn test_anthropic_parse_response() {
        let json = serde_json::json!({
            "content": [{ "text": "Hello from Claude" }]
        });
        assert_eq!(
            Provider::Anthropic.parse_response(&json).unwrap(),
            "Hello from Claude"
        );
    }

    #[test]
    fn test_parse_response_missing_content_is_error() {
        let json = serde_json::json!({"unexpected": "shape"});
        assert!(Provider::Ollama.parse_response(&json).is_err());
        assert!(Provider::OpenAi.parse_response(&json).is_err());
        assert!(Provider::Anthropic.parse_response(&json).is_err());
    }

    // --- LlmConfig builder ---

    #[test]
    fn llm_config_builder_happy_path() {
        let config = LlmConfig::builder()
            .provider(Provider::OpenAi)
            .base_url("https://example.com")
            .model("gpt-4")
            .api_key("key")
            .num_ctx(8192)
            .max_tokens(2048)
            .build()
            .unwrap();

        assert_eq!(config.provider, Provider::OpenAi);
        assert_eq!(config.base_url, "https://example.com");
        assert_eq!(config.model, "gpt-4");
        assert_eq!(config.api_key.as_deref(), Some("key"));
        assert_eq!(config.num_ctx, 8192);
        assert_eq!(config.max_tokens, 2048);
    }

    #[test]
    fn llm_config_builder_defaults_token_fields_to_4096() {
        let config = LlmConfig::builder()
            .provider(Provider::Ollama)
            .base_url("http://localhost:11434")
            .model("llama3")
            .build()
            .unwrap();

        assert_eq!(config.num_ctx, 4096);
        assert_eq!(config.max_tokens, 4096);
    }

    #[test]
    fn llm_config_builder_api_key_optional() {
        let config = LlmConfig::builder()
            .provider(Provider::Ollama)
            .base_url("http://localhost:11434")
            .model("llama3")
            .build()
            .unwrap();

        assert!(config.api_key.is_none());
    }

    #[test]
    fn llm_config_builder_errors_without_provider() {
        let err = LlmConfig::builder()
            .base_url("http://localhost:11434")
            .model("llama3")
            .build()
            .unwrap_err();

        assert_eq!(err, LlmConfigError::MissingProvider);
    }

    #[test]
    fn llm_config_builder_errors_without_base_url() {
        let err = LlmConfig::builder()
            .provider(Provider::Ollama)
            .model("llama3")
            .build()
            .unwrap_err();

        assert_eq!(err, LlmConfigError::MissingBaseUrl);
    }

    #[test]
    fn llm_config_builder_errors_without_model() {
        let err = LlmConfig::builder()
            .provider(Provider::Ollama)
            .base_url("http://localhost:11434")
            .build()
            .unwrap_err();

        assert_eq!(err, LlmConfigError::MissingModel);
    }

    #[test]
    fn request_uses_owned_config() {
        let cfg = LlmConfig::builder()
            .provider(Provider::Ollama)
            .base_url("http://localhost:11434")
            .model("llama3")
            .build()
            .unwrap();

        let req = cfg.request().system("hi").user("hello");

        assert_eq!(req.config.model, "llama3");
        assert_eq!(req.config.provider, Provider::Ollama);
        assert_eq!(req.config.base_url, "http://localhost:11434");
    }

    #[test]
    fn request_can_be_called_repeatedly_on_same_config() {
        let cfg = LlmConfig::builder()
            .provider(Provider::Ollama)
            .base_url("http://localhost:11434")
            .model("llama3")
            .build()
            .unwrap();

        // Each request() returns its own builder; the config is not consumed.
        let r1 = cfg.request().user("first");
        let r2 = cfg.request().user("second");

        assert_eq!(r1.messages, vec!["first".to_string()]);
        assert_eq!(r2.messages, vec!["second".to_string()]);
        assert_eq!(r1.config.model, "llama3");
        assert_eq!(r2.config.model, "llama3");
    }
}
