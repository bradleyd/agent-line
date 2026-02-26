use std::collections::HashMap;

/// Execution context for agents (services, config, etc.)
pub struct Ctx {
    store: HashMap<String, String>,
    log: Vec<String>,
}

impl Ctx {
    pub fn new() -> Self {
        Self {
            store: HashMap::new(),
            log: vec![],
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
}

impl Default for Ctx {
    fn default() -> Self {
        Self::new()
    }
}
