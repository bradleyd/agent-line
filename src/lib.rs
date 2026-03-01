//! A batteries-included Rust library for building agent workflows.
//!
//! Define agents, wire them into workflows, and let the runner execute them.
//! Agents communicate through shared context ([`Ctx`]) and control flow with
//! outcomes like [`Outcome::Continue`], [`Outcome::Next`], [`Outcome::Retry`],
//! and [`Outcome::Done`].
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

mod agent;
mod ctx;
mod runner;
pub mod tools;
mod workflow;

pub use agent::{Agent, Outcome, RetryHint, StepError, StepResult};
pub use ctx::Ctx;
pub use runner::{ErrorEvent, Runner, StepEvent};
pub use workflow::{Workflow, WorkflowBuilder, WorkflowError};
