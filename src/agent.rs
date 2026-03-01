use crate::ctx::Ctx;
use std::fmt;

/// The result of running a step: a new state plus what to do next.
pub type StepResult<S> = Result<(S, Outcome), StepError>;

/// A sync agent that transforms state one step at a time.
///
/// Implement this trait on your own structs and register them into a
/// [`crate::Workflow`] to build a pipeline.
pub trait Agent<S>: Send + 'static {
    /// A unique name for this agent, used for routing with [`Outcome::Next`].
    fn name(&self) -> &'static str;

    /// Run one step. Returns the updated state and an [`Outcome`] that tells
    /// the runner what to do next.
    fn run(&mut self, state: S, ctx: &mut Ctx) -> StepResult<S>;
}

/// Control flow for the runner.
#[derive(Debug, Clone)]
pub enum Outcome {
    /// Follow the workflow’s default next step (set via `.then()`).
    Continue,

    /// Workflow complete, return the final state.
    Done,
    /// Jump to a specific agent by name.
    Next(&'static str),
    /// Re-run the current agent (counted against `max_retries`).
    Retry(RetryHint),
    /// Sleep for the given duration, then re-run (counted against `max_retries`).
    Wait(std::time::Duration),
    /// Stop the workflow with an error.
    Fail(String),
}

/// Metadata attached to an [`Outcome::Retry`] to explain why the agent
/// wants to retry.
#[derive(Debug, Clone)]
pub struct RetryHint {
    /// Human-readable reason for the retry.
    pub reason: String,
}

impl RetryHint {
    /// Create a new hint with the given reason.
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

/// Error type for agent steps, with variants designed around what the caller
/// can do about them.
#[derive(Debug)]
pub enum StepError {
    /// Bad input or agent logic error. Don't retry, fix the code.
    Invalid(String),
    /// Transient failure (network, rate limit). Retrying might help.
    Transient(String),
    /// Agent decided to fail explicitly via Outcome::Fail.
    Failed(String),
    /// Everything else. Inspect the message for details.
    Other(String),
}

impl From<ureq::Error> for StepError {
    fn from(e: ureq::Error) -> Self {
        StepError::Transient(e.to_string())
    }
}

impl From<std::io::Error> for StepError {
    fn from(e: std::io::Error) -> Self {
        StepError::Other(e.to_string())
    }
}

impl StepError {
    /// Create an [`Invalid`](StepError::Invalid) error.
    pub fn invalid(msg: impl Into<String>) -> Self {
        StepError::Invalid(msg.into())
    }

    /// Create an [`Other`](StepError::Other) error.
    pub fn other(msg: impl Into<String>) -> Self {
        StepError::Other(msg.into())
    }

    /// Create a [`Transient`](StepError::Transient) error.
    pub fn transient(msg: impl Into<String>) -> Self {
        StepError::Transient(msg.into())
    }
}

impl fmt::Display for StepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(msg) => write!(f, "invalid: {msg}"),
            Self::Other(msg) => write!(f, "{msg}"),
            Self::Transient(msg) => write!(f, "transient: {msg}"),
            Self::Failed(msg) => write!(f, "failed: {msg}"),
        }
    }
}

impl std::error::Error for StepError {}

#[cfg(test)]
mod tests {
    use super::*;

    // --- StepError constructors ---

    #[test]
    fn invalid_constructor() {
        let err = StepError::invalid("bad input");
        assert!(matches!(err, StepError::Invalid(msg) if msg == "bad input"));
    }

    #[test]
    fn other_constructor() {
        let err = StepError::other("something");
        assert!(matches!(err, StepError::Other(msg) if msg == "something"));
    }

    #[test]
    fn transient_constructor() {
        let err = StepError::transient("timeout");
        assert!(matches!(err, StepError::Transient(msg) if msg == "timeout"));
    }

    // --- StepError Display ---

    #[test]
    fn display_invalid() {
        let err = StepError::Invalid("bad input".into());
        assert_eq!(err.to_string(), "invalid: bad input");
    }

    #[test]
    fn display_other() {
        let err = StepError::Other("something".into());
        assert_eq!(err.to_string(), "something");
    }

    #[test]
    fn display_transient() {
        let err = StepError::Transient("timeout".into());
        assert_eq!(err.to_string(), "transient: timeout");
    }

    #[test]
    fn display_failed() {
        let err = StepError::Failed("nope".into());
        assert_eq!(err.to_string(), "failed: nope");
    }

    // --- From conversions ---

    #[test]
    fn from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let step_err: StepError = io_err.into();
        assert!(matches!(step_err, StepError::Other(msg) if msg.contains("file missing")));
    }

    // --- RetryHint ---

    #[test]
    fn retry_hint_new() {
        let hint = RetryHint::new("reason");
        assert_eq!(hint.reason, "reason");
    }
}
