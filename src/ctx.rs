use std::collections::HashMap;

/// Execution context shared across all agents in a workflow.
///
/// `Ctx` carries two things:
/// - a string key-value store for cross-agent data,
/// - an append-only event log for diagnostic messages.
///
/// State is not reset between [`crate::Runner::run`] calls, so the store and
/// log accumulate across runs unless cleared explicitly with
/// [`clear_logs`](Ctx::clear_logs) or [`clear`](Ctx::clear).
///
/// LLM access lives on [`crate::LlmConfig`], not on `Ctx`. Agents that need
/// an LLM hold their own [`crate::LlmConfig`] and call
/// [`crate::LlmConfig::request`] to start a chat request.
pub struct Ctx {
    store: HashMap<String, String>,
    log: Vec<String>,
}

impl Ctx {
    /// Create a new empty context (KV store and event log).
    pub fn new() -> Self {
        Self {
            store: HashMap::new(),
            log: vec![],
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
}

impl Default for Ctx {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn separate_contexts_have_independent_state() {
        let mut a = Ctx::new();
        let b = Ctx::new();

        a.set("topic", "rust");
        a.log("phase 1 done");

        assert_eq!(a.get("topic"), Some("rust"));
        assert_eq!(b.get("topic"), None);
        assert_eq!(a.logs(), &["phase 1 done"]);
        assert!(b.logs().is_empty());
    }
}
