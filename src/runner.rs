use crate::{Ctx, Outcome, StepError, Workflow};

pub struct Runner<S: Clone + 'static> {
    wf: Workflow<S>,
    max_steps: usize,
    max_retries: usize,
}

impl<S: Clone + 'static> Runner<S> {
    pub fn new(wf: Workflow<S>) -> Self {
        Self {
            wf,
            max_steps: 10_000,
            max_retries: 3,
        }
    }

    /// Prevent accidental infinite loops.
    pub fn with_max_steps(mut self, max_steps: usize) -> Self {
        self.max_steps = max_steps;
        self
    }

    pub fn with_max_retries(mut self, max_retries: usize) -> Self {
        self.max_retries = max_retries;
        self
    }

    pub fn run(&mut self, mut state: S, ctx: &mut Ctx) -> Result<S, StepError> {
        let mut current = self.wf.start();
        let mut retries: usize = 0;

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
                    retries = 0;
                    continue;
                }
                // default routing: use .then() edges
                Outcome::Continue => {
                    if let Some(next) = self.wf.default_next(current) {
                        current = next;
                        retries = 0;
                        continue;
                    }
                    return Err(StepError::other(format!(
                        "step '{current}' returned Continue but no default next step is configured"
                    )));
                }

                Outcome::Retry(hint) => {
                    retries += 1;
                    if retries > self.max_retries {
                        return Err(StepError::other(format!(
                            "step '{}' exceeded max retries ({}): {}",
                            current, self.max_retries, hint.reason
                        )));
                    }
                    continue;
                }
                Outcome::Wait(dur) => {
                    retries += 1;
                    if retries > self.max_retries {
                        return Err(StepError::other(format!(
                            "step '{}' exceeded max retries ({}) while waiting",
                            current, self.max_retries
                        )));
                    }
                    std::thread::sleep(dur);
                    continue;
                }
            }
        }

        Err(StepError::other(format!(
            "max_steps exceeded (possible infinite loop) in workflow {}",
            self.wf.name()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Agent, Outcome, RetryHint, StepResult, Workflow};
    use std::time::Duration;

    #[derive(Clone)]
    struct S(u32);

    struct RetryAgent {
        attempts: u32,
        succeed_on: u32,
    }

    impl Agent<S> for RetryAgent {
        fn name(&self) -> &'static str {
            "retry_agent"
        }
        fn run(&mut self, state: S, _ctx: &mut Ctx) -> StepResult<S> {
            self.attempts += 1;
            if self.attempts >= self.succeed_on {
                Ok((state, Outcome::Done))
            } else {
                Ok((state, Outcome::Retry(RetryHint::new("not ready"))))
            }
        }
    }

    struct AlwaysRetry;
    impl Agent<S> for AlwaysRetry {
        fn name(&self) -> &'static str {
            "always_retry"
        }
        fn run(&mut self, state: S, _ctx: &mut Ctx) -> StepResult<S> {
            Ok((state, Outcome::Retry(RetryHint::new("never ready"))))
        }
    }

    struct WaitOnce {
        waited: bool,
    }
    impl Agent<S> for WaitOnce {
        fn name(&self) -> &'static str {
            "wait_once"
        }
        fn run(&mut self, state: S, _ctx: &mut Ctx) -> StepResult<S> {
            if !self.waited {
                self.waited = true;
                Ok((state, Outcome::Wait(Duration::from_millis(1))))
            } else {
                Ok((state, Outcome::Done))
            }
        }
    }

    #[test]
    fn retry_succeeds_within_limit() {
        let wf = Workflow::builder("test")
            .register(RetryAgent {
                attempts: 0,
                succeed_on: 3,
            })
            .build()
            .unwrap();

        let mut runner = Runner::new(wf);
        let mut ctx = Ctx::new();
        let result = runner.run(S(0), &mut ctx);
        assert!(result.is_ok());
    }

    #[test]
    fn retry_exceeds_limit() {
        let wf = Workflow::builder("test")
            .register(AlwaysRetry)
            .build()
            .unwrap();

        let mut runner = Runner::new(wf).with_max_retries(2);
        let mut ctx = Ctx::new();
        let err = runner.run(S(0), &mut ctx).err().unwrap();
        assert!(err.to_string().contains("exceeded max retries"));
    }

    #[test]
    fn wait_sleeps_and_reruns() {
        let wf = Workflow::builder("test")
            .register(WaitOnce { waited: false })
            .build()
            .unwrap();

        let mut runner = Runner::new(wf);
        let mut ctx = Ctx::new();
        let result = runner.run(S(0), &mut ctx);
        assert!(result.is_ok());
    }
}
