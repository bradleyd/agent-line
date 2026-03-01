use crate::agent::StepError;
use std::{collections::HashMap, env, sync::Arc};

/// Execution context for agents (services, config, etc.)
pub struct Ctx {
    store: HashMap<String, String>,
    log: Vec<String>,
    llm_client: Arc<LlmClient>,
}

struct LlmClient {
    base_url: String,
    model: String,
    num_ctx: u32,
    api_key: Option<String>,
    provider: Provider,
}

/// Builder for LLM chat requests. Obtained via [`Ctx::llm`].
pub struct LlmRequestBuilder {
    client: Arc<LlmClient>,
    system: Option<String>,
    messages: Vec<String>,
}

/// LLM provider, set via the `AGENT_LINE_PROVIDER` env var.
#[derive(Debug, PartialEq)]
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
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "openai" => Provider::OpenAi,
            "anthropic" => Provider::Anthropic,
            _ => Provider::Ollama,
        }
    }

    /// Return the full chat endpoint URL for this provider.
    pub fn endpoint(&self, base_url: &str) -> String {
        let base = base_url.trim_end_matches('/');
        match self {
            Provider::Ollama => format!("{base}/api/chat"),
            Provider::OpenAi => format!("{base}/v1/chat/completions"),
            Provider::Anthropic => format!("{base}/v1/messages"),
        }
    }

    /// Extract the assistant message from a provider-specific JSON response.
    pub fn parse_response(&self, json: &serde_json::Value) -> Result<String, StepError> {
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

        let body = match &self.client.provider {
            Provider::Ollama => serde_json::json!({
                "model": self.client.model,
                "messages": messages,
                "stream": false,
                "options": {
                    "num_ctx": self.client.num_ctx
                }
            }),
            Provider::OpenAi => serde_json::json!({
                "model": self.client.model,
                "messages": messages,
                "stream": false,
                "max_tokens": self.client.num_ctx
            }),
            Provider::Anthropic => serde_json::json!({
                "model": self.client.model,
                "messages": messages,
                "stream": false,
                "max_tokens": self.client.num_ctx
            }),
        };

        let url = self.client.provider.endpoint(&self.client.base_url);
        let mut request = ureq::post(&url);

        match &self.client.provider {
            Provider::Anthropic => {
                if let Some(key) = &self.client.api_key {
                    request = request.header("x-api-key", key);
                }
                request = request.header("anthropic-version", "2023-06-01");
                request = request.header("content-type", "application/json");
            }
            _ => {
                if let Some(key) = &self.client.api_key {
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

        self.client.provider.parse_response(&json)
    }
}

impl Ctx {
    /// Create a new context. Configuration is read from environment variables
    /// (see the crate-level docs for the full list).
    pub fn new() -> Self {
        let model = env::var("AGENT_LINE_MODEL").unwrap_or_else(|_| "llama3.1:8b".to_string());
        let base_url =
            env::var("AGENT_LINE_LLM_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());

        let num_ctx = match env::var("AGENT_LINE_NUM_CTX") {
            Ok(v) => v.parse::<u32>().unwrap_or(4096),
            Err(_) => 4096,
        };

        let api_key = env::var("AGENT_LINE_API_KEY").ok();
        let provider = Provider::from_str(
            &env::var("AGENT_LINE_PROVIDER").unwrap_or_else(|_| "ollama".to_string()),
        );

        if env::var("AGENT_LINE_DEBUG").is_ok() {
            eprintln!(
                "[debug] provider: {:?}\n\
                 [debug] model: {}\n\
                 [debug] base_url: {}\n\
                 [debug] num_ctx: {}\n\
                 [debug] api_key: {}",
                provider,
                model,
                base_url,
                num_ctx,
                if api_key.is_some() { "set" } else { "not set" },
            );
        }

        Self {
            store: HashMap::new(),
            log: vec![],
            llm_client: Arc::new(LlmClient {
                base_url,
                model,
                num_ctx,
                api_key,
                provider,
            }),
        }
    }

    /// Insert or overwrite a key in the KV store.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.store.insert(key.into(), value.into());
    }

    /// Look up a key in the KV store.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.store.get(key).map(|s| s.as_str())
    }

    /// Remove a key from the KV store, returning its value if it existed.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.store.remove(key)
    }

    /// Append a message to the event log.
    pub fn log(&mut self, msg: impl Into<String>) {
        self.log.push(msg.into());
    }

    /// Return all log messages in order.
    pub fn logs(&self) -> &[String] {
        &self.log
    }

    /// Clear the event log, leaving the KV store intact.
    pub fn clear_logs(&mut self) {
        self.log.clear();
    }

    /// Clear both the KV store and the event log.
    pub fn clear(&mut self) {
        self.store.clear();
        self.log.clear();
    }

    /// Start building an LLM chat request.
    pub fn llm(&self) -> LlmRequestBuilder {
        LlmRequestBuilder {
            client: Arc::clone(&self.llm_client),
            system: None,
            messages: Vec::new(),
        }
    }
}

impl Default for Ctx {
    fn default() -> Self {
        Self::new()
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

    // --- KV store ---

    #[test]
    fn set_then_get() {
        let mut ctx = Ctx::new();
        ctx.set("key", "value");
        assert_eq!(ctx.get("key"), Some("value"));
    }

    #[test]
    fn get_missing_key() {
        let ctx = Ctx::new();
        assert_eq!(ctx.get("nope"), None);
    }

    #[test]
    fn set_overwrites() {
        let mut ctx = Ctx::new();
        ctx.set("key", "first");
        ctx.set("key", "second");
        assert_eq!(ctx.get("key"), Some("second"));
    }

    #[test]
    fn remove_returns_value() {
        let mut ctx = Ctx::new();
        ctx.set("key", "value");
        assert_eq!(ctx.remove("key"), Some("value".to_string()));
        assert_eq!(ctx.get("key"), None);
    }

    #[test]
    fn remove_missing_key() {
        let mut ctx = Ctx::new();
        assert_eq!(ctx.remove("nope"), None);
    }

    // --- Logging ---

    #[test]
    fn log_appends_and_logs_returns_in_order() {
        let mut ctx = Ctx::new();
        ctx.log("first");
        ctx.log("second");
        ctx.log("third");
        assert_eq!(ctx.logs(), &["first", "second", "third"]);
    }

    #[test]
    fn clear_logs_preserves_store() {
        let mut ctx = Ctx::new();
        ctx.set("key", "value");
        ctx.log("msg");
        ctx.clear_logs();
        assert!(ctx.logs().is_empty());
        assert_eq!(ctx.get("key"), Some("value"));
    }

    #[test]
    fn clear_empties_both() {
        let mut ctx = Ctx::new();
        ctx.set("key", "value");
        ctx.log("msg");
        ctx.clear();
        assert!(ctx.logs().is_empty());
        assert_eq!(ctx.get("key"), None);
    }
}
