use crate::ctx::Ctx;
use std::fmt;

/// The result of running a step: a new state plus what to do next.
pub type StepResult<S> = Result<(S, Outcome), StepError>;

/// A sync “agent” that transforms immutable state.
pub trait Agent<S>: Send + 'static {
    fn name(&self) -> &'static str;
    fn run(&mut self, state: S, ctx: &Ctx) -> StepResult<S>;
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
    Invalid(String),
    Other(String),
}

impl StepError {
    pub fn invalid(msg: impl Into<String>) -> Self {
        StepError::Invalid(msg.into())
    }
    pub fn other(msg: impl Into<String>) -> Self {
        StepError::Other(msg.into())
    }
}

impl fmt::Display for StepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(msg) => write!(f, "invalid: {msg}"),
            Self::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for StepError {}
