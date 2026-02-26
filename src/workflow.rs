use crate::Agent;
use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// WorkflowError
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum WorkflowError {
    DuplicateAgent(&'static str),
    UnknownStep(&'static str),
    MissingStart,
}

impl fmt::Display for WorkflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateAgent(name) => write!(f, "duplicate agent name: {name}"),
            Self::UnknownStep(name) => write!(f, "unknown step: {name}"),
            Self::MissingStart => write!(f, "workflow missing start step"),
        }
    }
}

impl std::error::Error for WorkflowError {}

// ---------------------------------------------------------------------------
// WorkflowBuilder
// ---------------------------------------------------------------------------

pub struct WorkflowBuilder<S: Clone + 'static> {
    name: &'static str,
    start: Option<&'static str>,
    chain_last: Option<&'static str>,
    agents: HashMap<&'static str, Box<dyn Agent<S>>>,
    default_next: HashMap<&'static str, &'static str>,
    duplicate: Option<&'static str>,
}

impl<S: Clone + 'static> WorkflowBuilder<S> {
    pub fn register<A: Agent<S>>(mut self, agent: A) -> Self {
        let name = agent.name();
        if self.agents.contains_key(name) {
            self.duplicate = Some(name);
        }
        self.agents.insert(name, Box::new(agent));

        // If this is the first agent added and start isn't set, default start to it.
        if self.start.is_none() {
            self.start = Some(name);
        }

        // Also initialize chain_last if it's not set.
        if self.chain_last.is_none() {
            self.chain_last = Some(name);
        }

        self
    }

    pub fn start_at(mut self, step: &'static str) -> Self {
        self.start = Some(step);
        self.chain_last = Some(step);
        self
    }

    /// Chain the next step: current(chain_last) -> next
    pub fn then(mut self, next: &'static str) -> Self {
        let Some(current) = self.chain_last else {
            // No prior step; treat `next` as the start
            self.start = Some(next);
            self.chain_last = Some(next);
            return self;
        };

        self.default_next.insert(current, next);
        self.chain_last = Some(next);
        self
    }

    pub fn build(self) -> Result<Workflow<S>, WorkflowError> {
        // Check for duplicate agents.
        if let Some(name) = self.duplicate {
            return Err(WorkflowError::DuplicateAgent(name));
        }

        // Check for a start step.
        let start = self.start.ok_or(WorkflowError::MissingStart)?;

        // Validate start_at target exists as a registered agent.
        if !self.agents.contains_key(start) {
            return Err(WorkflowError::UnknownStep(start));
        }

        // Validate every `then` target exists as a registered agent.
        for &target in self.default_next.values() {
            if !self.agents.contains_key(target) {
                return Err(WorkflowError::UnknownStep(target));
            }
        }

        Ok(Workflow {
            name: self.name,
            start,
            agents: self.agents,
            default_next: self.default_next,
        })
    }
}

// ---------------------------------------------------------------------------
// Workflow (validated, only constructed via build())
// ---------------------------------------------------------------------------

pub struct Workflow<S: Clone + 'static> {
    name: &'static str,
    start: &'static str,
    agents: HashMap<&'static str, Box<dyn Agent<S>>>,
    default_next: HashMap<&'static str, &'static str>,
}

impl<S: Clone + 'static> Workflow<S> {
    pub fn builder(name: &'static str) -> WorkflowBuilder<S> {
        WorkflowBuilder {
            name,
            start: None,
            chain_last: None,
            agents: HashMap::new(),
            default_next: HashMap::new(),
            duplicate: None,
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    // --- stuff the runner uses (keep pub(crate)) ---
    pub(crate) fn start(&self) -> &'static str {
        self.start
    }

    pub(crate) fn agent_mut(&mut self, name: &'static str) -> Option<&mut Box<dyn Agent<S>>> {
        self.agents.get_mut(name)
    }

    pub(crate) fn default_next(&self, from: &'static str) -> Option<&'static str> {
        self.default_next.get(from).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Ctx, Outcome, StepResult};

    #[derive(Clone)]
    struct S;

    struct FakeAgent(&'static str);

    impl Agent<S> for FakeAgent {
        fn name(&self) -> &'static str {
            self.0
        }
        fn run(&mut self, state: S, _ctx: &mut Ctx) -> StepResult<S> {
            Ok((state, Outcome::Done))
        }
    }

    #[test]
    fn build_valid_workflow() {
        let wf = Workflow::builder("test")
            .register(FakeAgent("a"))
            .register(FakeAgent("b"))
            .start_at("a")
            .then("b")
            .build();

        assert!(wf.is_ok());
        let wf = wf.unwrap();
        assert_eq!(wf.name(), "test");
        assert_eq!(wf.start(), "a");
        assert_eq!(wf.default_next("a"), Some("b"));
    }

    #[test]
    fn missing_start_on_empty_builder() {
        let err = Workflow::<S>::builder("test").build().err().unwrap();
        assert!(matches!(err, WorkflowError::MissingStart));
    }

    #[test]
    fn unknown_start_at_step() {
        let err = Workflow::builder("test")
            .register(FakeAgent("a"))
            .start_at("missing")
            .build()
            .err()
            .unwrap();

        assert!(matches!(err, WorkflowError::UnknownStep("missing")));
    }

    #[test]
    fn unknown_then_target() {
        let err = Workflow::builder("test")
            .register(FakeAgent("a"))
            .start_at("a")
            .then("missing")
            .build()
            .err()
            .unwrap();

        assert!(matches!(err, WorkflowError::UnknownStep("missing")));
    }

    #[test]
    fn first_agent_becomes_default_start() {
        let wf = Workflow::builder("test")
            .register(FakeAgent("first"))
            .build();

        assert!(wf.is_ok());
    }

    #[test]
    fn duplicate_agent_rejected() {
        let err = Workflow::builder("test")
            .register(FakeAgent("a"))
            .register(FakeAgent("a"))
            .build()
            .err()
            .unwrap();

        assert!(matches!(err, WorkflowError::DuplicateAgent("a")));
    }
}
