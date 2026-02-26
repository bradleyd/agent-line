use crate::{Ctx, Outcome, StepError, Workflow};

pub struct Runner<S: Clone + 'static> {
    wf: Workflow<S>,
    max_steps: usize,
}

impl<S: Clone + 'static> Runner<S> {
    pub fn new(wf: Workflow<S>) -> Self {
        Self {
            wf,
            max_steps: 10_000,
        }
    }

    /// Prevent accidental infinite loops.
    pub fn with_max_steps(mut self, max_steps: usize) -> Self {
        self.max_steps = max_steps;
        self
    }

    pub fn run(&mut self, mut state: S, ctx: &mut Ctx) -> Result<S, StepError> {
        let mut current = self.wf.start();

        for _ in 0..self.max_steps {
            let agent = self
                .wf
                .agent_mut(current)
                .ok_or_else(|| StepError::other(format!("unknown step: {current}")))?;

            let (next_state, outcome) = agent.run(state, ctx)?;
            state = next_state;

            match outcome {
                Outcome::Done => return Ok(state),
                Outcome::Fail(msg) => return Err(StepError::other(msg)),
                Outcome::Next(step) => {
                    current = step;
                    continue;
                }
                // default routing: use .then() edges
                Outcome::Continue => {
                    if let Some(next) = self.wf.default_next(current) {
                        current = next;
                        continue;
                    }
                    return Err(StepError::other(format!(
                        "step '{current}' returned Continue but no default next step is configured"
                    )));
                }

                Outcome::Retry(_) => return Err(StepError::other("retry not implemented yet")),
                Outcome::Wait(_) => return Err(StepError::other("wait not implemented yet")),
            }
        }

        Err(StepError::other(format!(
            "max_steps exceeded (possible infinite loop) in workflow {}",
            self.wf.name()
        )))
    }
}
