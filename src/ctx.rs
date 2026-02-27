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
}

pub struct LlmRequestBuilder {
    client: Arc<LlmClient>,
    system: Option<String>,
    messages: Vec<String>,
}

impl LlmRequestBuilder {
    pub fn system(mut self, msg: &str) -> Self {
        self.system = Some(msg.to_string());
        self
    }

    pub fn user(mut self, msg: impl Into<String>) -> Self {
        self.messages.push(msg.into());
        self
    }

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

        let body = serde_json::json!({
            "model": self.client.model,
            "messages": messages,
            "stream": false
        });

        let url = format!("{}/api/chat", self.client.base_url);
        let mut request = ureq::post(&url);

        if let Some(key) = &self.client.api_key {
            request = request.header("Authorization", &format!("Bearer {key}"));
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
            eprintln!("[debug] LLM response: {}", &json["message"]["content"]);
        }

        json["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| StepError::other("llm response missing message content"))
    }
}

impl Ctx {
    pub fn new() -> Self {
        let model = env::var("AGENT_LINE_MODEL").unwrap_or_else(|_| "llama3.1:8b".to_string());
        let base_url =
            env::var("AGENT_LINE_LLM_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());

        let num_ctx = match env::var("AGENT_LINE_NUM_CTX") {
            Ok(v) => v.parse::<u32>().unwrap_or(4096),
            Err(_) => 4096,
        };

        let api_key = env::var("AGENT_LINE_API_KEY").ok();

        Self {
            store: HashMap::new(),
            log: vec![],
            llm_client: Arc::new(LlmClient {
                base_url,
                model,
                num_ctx,
                api_key,
            }),
        }
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.store.insert(key.into(), value.into());
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.store.get(key).map(|s| s.as_str())
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.store.remove(key)
    }

    pub fn log(&mut self, msg: impl Into<String>) {
        self.log.push(msg.into());
    }

    pub fn logs(&self) -> &[String] {
        &self.log
    }

    pub fn clear_logs(&mut self) {
        self.log.clear();
    }

    pub fn clear(&mut self) {
        self.store.clear();
        self.log.clear();
    }

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
