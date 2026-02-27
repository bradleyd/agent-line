use crate::ctx::Ctx;
use std::fmt;

/// The result of running a step: a new state plus what to do next.
pub type StepResult<S> = Result<(S, Outcome), StepError>;

/// A sync “agent” that transforms immutable state.
pub trait Agent<S>: Send + 'static {
    fn name(&self) -> &'static str;
    fn run(&mut self, state: S, ctx: &mut Ctx) -> StepResult<S>;
}

/// Control flow for the runner.
#[derive(Debug, Clone)]
pub enum Outcome {
    /// Follow the workflow’s default next step (set via `.then()`).
    Continue,

    Done,
    Next(&'static str),
    Retry(RetryHint),
    Wait(std::time::Duration),
    Fail(String),
}

#[derive(Debug, Clone)]
pub struct RetryHint {
    pub reason: String,
}

impl RetryHint {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

#[derive(Debug)]
pub enum StepError {
    /// Bad input or agent logic error. Don't retry, fix the code.
    Invalid(String),
    /// Transient failure (network, rate limit). Retrying might help.
    Transient(String),
    /// Agent decided to fail explicitly via Outcome::Fail.
    Failed(String),
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
    pub fn invalid(msg: impl Into<String>) -> Self {
        StepError::Invalid(msg.into())
    }
    pub fn other(msg: impl Into<String>) -> Self {
        StepError::Other(msg.into())
    }

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
