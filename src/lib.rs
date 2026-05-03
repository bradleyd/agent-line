//! A batteries-included Rust library for building agent workflows.
//!
//! Define agents, wire them into workflows, and let the runner execute them.
//! Agents communicate through shared context ([`Ctx`]) and control flow with
//! outcomes: [`Outcome::Continue`], [`Outcome::Next`], [`Outcome::Retry`],
//! [`Outcome::Wait`], [`Outcome::Done`], and [`Outcome::Fail`].
//!
//! # Quick start
//!
//! ```rust
//! use agent_line::{Agent, Ctx, Outcome, Runner, StepResult, Workflow};
//!
//! #[derive(Clone)]
//! struct State { n: i32 }
//!
//! struct AddOne;
//! impl Agent<State> for AddOne {
//!     fn name(&self) -> &'static str { "add_one" }
//!     fn run(&mut self, state: State, _ctx: &mut Ctx) -> StepResult<State> {
//!         Ok((State { n: state.n + 1 }, Outcome::Done))
//!     }
//! }
//!
//! let mut ctx = Ctx::new();
//! let wf = Workflow::builder("demo")
//!     .register(AddOne)
//!     .build()
//!     .unwrap();
//!
//! let result = Runner::new(wf).run(State { n: 0 }, &mut ctx).unwrap();
//! assert_eq!(result.n, 1);
//! ```
//!
//! # LLM access
//!
//! Each agent that needs an LLM holds its own [`LlmConfig`] and calls
//! [`LlmConfig::request`] to start a chat request.
//!
//! For the simplest case, build an [`LlmConfig`] from environment variables
//! and inject it into the agent that needs it:
//!
//! ```rust,no_run
//! # use agent_line::{Agent, Ctx, LlmConfig, Outcome, StepResult};
//! # #[derive(Clone)]
//! # struct Draft { body: String, summary: String }
//! struct Summarize {
//!     llm: LlmConfig,
//! }
//!
//! impl Summarize {
//!     fn new(llm: LlmConfig) -> Self { Self { llm } }
//! }
//!
//! impl Agent<Draft> for Summarize {
//!     fn name(&self) -> &'static str { "summarize" }
//!     fn run(&mut self, mut draft: Draft, _ctx: &mut Ctx) -> StepResult<Draft> {
//!         draft.summary = self.llm.request()
//!             .system("Summarize the draft in one sentence.")
//!             .user(&draft.body)
//!             .send()?;
//!         Ok((draft, Outcome::Done))
//!     }
//! }
//!
//! // In main():
//! //   let llm = LlmConfig::from_env();   // reads AGENT_LINE_PROVIDER, etc.
//! //   register(Summarize::new(llm))
//! ```
//!
//! [`LlmConfig::from_env`] reads `AGENT_LINE_PROVIDER`, `AGENT_LINE_LLM_URL`,
//! `AGENT_LINE_MODEL`, `AGENT_LINE_API_KEY`, `AGENT_LINE_NUM_CTX`, and
//! `AGENT_LINE_MAX_TOKENS`. Defaults to a local Ollama configuration when
//! nothing is set.
//!
//! For multi-model pipelines, give each agent its own [`LlmConfig`]. A cheap
//! local model handles routine extraction; a stronger remote model handles
//! the harder reasoning step:
//!
//! ```rust,no_run
//! use agent_line::{
//!     Agent, Ctx, LlmConfig, Outcome, Provider, Runner, StepResult, Workflow,
//! };
//!
//! #[derive(Clone)]
//! struct Draft { body: String, notes: String, review: String }
//!
//! struct Researcher { llm: LlmConfig }
//!
//! impl Researcher {
//!     fn new(llm: LlmConfig) -> Self { Self { llm } }
//! }
//!
//! impl Agent<Draft> for Researcher {
//!     fn name(&self) -> &'static str { "researcher" }
//!     fn run(&mut self, mut draft: Draft, _ctx: &mut Ctx) -> StepResult<Draft> {
//!         draft.notes = self.llm.request()
//!             .system("Extract the three key claims from the draft, one per line.")
//!             .user(&draft.body)
//!             .send()?;
//!         Ok((draft, Outcome::Continue))
//!     }
//! }
//!
//! struct Reviewer { llm: LlmConfig }
//!
//! impl Reviewer {
//!     fn new(llm: LlmConfig) -> Self { Self { llm } }
//! }
//!
//! impl Agent<Draft> for Reviewer {
//!     fn name(&self) -> &'static str { "reviewer" }
//!     fn run(&mut self, mut draft: Draft, _ctx: &mut Ctx) -> StepResult<Draft> {
//!         draft.review = self.llm.request()
//!             .system("Critique the draft against its own claims. Be specific.")
//!             .user(format!("Claims:\n{}\n\nDraft:\n{}", draft.notes, draft.body))
//!             .send()?;
//!         Ok((draft, Outcome::Done))
//!     }
//! }
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let cheap = LlmConfig::builder()
//!     .provider(Provider::Ollama)
//!     .base_url("http://localhost:11434")
//!     .model("qwen3:8b")
//!     .build()?;
//!
//! let strong = LlmConfig::builder()
//!     .provider(Provider::Anthropic)
//!     .base_url("https://api.anthropic.com")
//!     .model("claude-sonnet-4-20250514")
//!     .api_key(std::env::var("ANTHROPIC_API_KEY")?)
//!     .max_tokens(1200)
//!     .build()?;
//!
//! let mut ctx = Ctx::new();
//! let wf = Workflow::builder("review")
//!     .register(Researcher::new(cheap))
//!     .register(Reviewer::new(strong))
//!     .start_at("researcher")
//!     .then("reviewer")
//!     .build()?;
//!
//! Runner::new(wf).run(
//!     Draft {
//!         body: "Rust ownership...".into(),
//!         notes: String::new(),
//!         review: String::new(),
//!     },
//!     &mut ctx,
//! )?;
//! # Ok(()) }
//! ```

mod agent;
mod ctx;
mod llm;
mod runner;
pub mod tools;
mod workflow;

pub use agent::{Agent, Outcome, RetryHint, StepError, StepResult};
pub use ctx::Ctx;
pub use llm::{LlmConfig, LlmConfigBuilder, LlmConfigError, LlmRequestBuilder, Provider};
pub use runner::{ErrorEvent, Runner, StepEvent};
pub use workflow::{Workflow, WorkflowBuilder, WorkflowError};
