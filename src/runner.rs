use crate::{Ctx, Outcome, StepError, Workflow};
use std::time::{Duration, Instant};

/// Passed to the `on_step` hook after each successful agent step.
pub struct StepEvent<'a> {
    pub agent: &'a str,
    pub outcome: &'a Outcome,
    pub duration: Duration,
    pub step_number: usize,
    pub retries: usize,
}

/// Passed to the `on_error` hook when an agent errors or a limit is exceeded.
pub struct ErrorEvent<'a> {
    pub agent: &'a str,
    pub error: &'a StepError,
    pub step_number: usize,
}

pub struct Runner<S: Clone + 'static> {
    wf: Workflow<S>,
    max_steps: usize,
    max_retries: usize,
    on_step: Option<Box<dyn FnMut(&StepEvent)>>,
    on_error: Option<Box<dyn FnMut(&ErrorEvent)>>,
}

impl<S: Clone + 'static> Runner<S> {
    pub fn new(wf: Workflow<S>) -> Self {
        Self {
            wf,
            max_steps: 10_000,
            max_retries: 3,
            on_step: None,
            on_error: None,
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

    /// Register a callback that fires after each successful agent step.
    pub fn on_step(mut self, cb: impl FnMut(&StepEvent) + 'static) -> Self {
        self.on_step = Some(Box::new(cb));
        self
    }

    /// Register a callback that fires when an agent errors or a limit is exceeded.
    pub fn on_error(mut self, cb: impl FnMut(&ErrorEvent) + 'static) -> Self {
        self.on_error = Some(Box::new(cb));
        self
    }

    /// Set both hooks to print step transitions and errors to stderr.
    pub fn with_tracing(self) -> Self {
        self.on_step(|e| {
            eprintln!(
                "[step {}] {} -> {:?} ({:.3}s)",
                e.step_number,
                e.agent,
                e.outcome,
                e.duration.as_secs_f64()
            );
        })
        .on_error(|e| {
            eprintln!("[error] {} at step {}: {}", e.agent, e.step_number, e.error);
        })
    }

    pub fn run(&mut self, mut state: S, ctx: &mut Ctx) -> Result<S, StepError> {
        let mut current = self.wf.start();
        let mut retries: usize = 0;
        let mut step_number: usize = 0;

        for _ in 0..self.max_steps {
            step_number += 1;

            let agent = self
                .wf
                .agent_mut(current)
                .ok_or_else(|| StepError::other(format!("unknown step: {current}")))?;

            let start = Instant::now();
            let result = agent.run(state.clone(), ctx);
            let duration = start.elapsed();

            match result {
                Err(err) => {
                    if let Some(cb) = &mut self.on_error {
                        cb(&ErrorEvent {
                            agent: current,
                            error: &err,
                            step_number,
                        });
                    }
                    return Err(err);
                }
                Ok((next_state, outcome)) => {
                    if let Some(cb) = &mut self.on_step {
                        cb(&StepEvent {
                            agent: current,
                            outcome: &outcome,
                            duration,
                            step_number,
                            retries,
                        });
                    }

                    state = next_state;

                    match outcome {
                        Outcome::Done => return Ok(state),
                        Outcome::Fail(msg) => return Err(StepError::other(msg)),
                        Outcome::Next(step) => {
                            current = step;
                            retries = 0;
                            continue;
                        }
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
                                let err = StepError::other(format!(
                                    "step '{}' exceeded max retries ({}): {}",
                                    current, self.max_retries, hint.reason
                                ));
                                if let Some(cb) = &mut self.on_error {
                                    cb(&ErrorEvent {
                                        agent: current,
                                        error: &err,
                                        step_number,
                                    });
                                }
                                return Err(err);
                            }
                            continue;
                        }
                        Outcome::Wait(dur) => {
                            retries += 1;
                            if retries > self.max_retries {
                                let err = StepError::other(format!(
                                    "step '{}' exceeded max retries ({}) while waiting",
                                    current, self.max_retries
                                ));
                                if let Some(cb) = &mut self.on_error {
                                    cb(&ErrorEvent {
                                        agent: current,
                                        error: &err,
                                        step_number,
                                    });
                                }
                                return Err(err);
                            }
                            std::thread::sleep(dur);
                            continue;
                        }
                    }
                }
            }
        }

        let err = StepError::other(format!(
            "max_steps exceeded (possible infinite loop) in workflow {}",
            self.wf.name()
        ));
        if let Some(cb) = &mut self.on_error {
            cb(&ErrorEvent {
                agent: current,
                error: &err,
                step_number,
            });
        }
        Err(err)
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

    // --- hook tests ---

    struct DoneAgent;
    impl Agent<S> for DoneAgent {
        fn name(&self) -> &'static str {
            "done_agent"
        }
        fn run(&mut self, state: S, _ctx: &mut Ctx) -> StepResult<S> {
            Ok((state, Outcome::Done))
        }
    }

    struct FailingAgent;
    impl Agent<S> for FailingAgent {
        fn name(&self) -> &'static str {
            "failing_agent"
        }
        fn run(&mut self, _state: S, _ctx: &mut Ctx) -> StepResult<S> {
            Err(StepError::transient("boom"))
        }
    }

    struct AlwaysContinue;
    impl Agent<S> for AlwaysContinue {
        fn name(&self) -> &'static str {
            "always_continue"
        }
        fn run(&mut self, state: S, _ctx: &mut Ctx) -> StepResult<S> {
            Ok((state, Outcome::Continue))
        }
    }

    #[test]
    fn on_step_fires_on_success() {
        use std::sync::{Arc, Mutex};

        let count = Arc::new(Mutex::new(0usize));
        let count_clone = Arc::clone(&count);

        let wf = Workflow::builder("test")
            .register(DoneAgent)
            .build()
            .unwrap();

        let mut runner = Runner::new(wf).on_step(move |_e| {
            *count_clone.lock().unwrap() += 1;
        });

        let mut ctx = Ctx::new();
        runner.run(S(0), &mut ctx).unwrap();
        assert_eq!(*count.lock().unwrap(), 1);
    }

    #[test]
    fn on_error_fires_on_agent_error() {
        use std::sync::{Arc, Mutex};

        let count = Arc::new(Mutex::new(0usize));
        let count_clone = Arc::clone(&count);

        let wf = Workflow::builder("test")
            .register(FailingAgent)
            .build()
            .unwrap();

        let mut runner = Runner::new(wf).on_error(move |_e| {
            *count_clone.lock().unwrap() += 1;
        });

        let mut ctx = Ctx::new();
        let _ = runner.run(S(0), &mut ctx);
        assert_eq!(*count.lock().unwrap(), 1);
    }

    #[test]
    fn on_error_fires_on_max_retries() {
        use std::sync::{Arc, Mutex};

        let count = Arc::new(Mutex::new(0usize));
        let count_clone = Arc::clone(&count);

        let wf = Workflow::builder("test")
            .register(AlwaysRetry)
            .build()
            .unwrap();

        let mut runner = Runner::new(wf)
            .with_max_retries(1)
            .on_error(move |_e| {
                *count_clone.lock().unwrap() += 1;
            });

        let mut ctx = Ctx::new();
        let _ = runner.run(S(0), &mut ctx);
        assert_eq!(*count.lock().unwrap(), 1);
    }

    #[test]
    fn on_error_fires_on_max_steps() {
        use std::sync::{Arc, Mutex};

        let count = Arc::new(Mutex::new(0usize));
        let count_clone = Arc::clone(&count);

        let wf = Workflow::builder("test")
            .register(AlwaysContinue)
            .register(DoneAgent)
            .start_at("always_continue")
            .then("done_agent")
            .build()
            .unwrap();

        // Two agents ping-pong via Continue, but max_steps=1 cuts it short
        let mut runner = Runner::new(wf)
            .with_max_steps(1)
            .on_error(move |e| {
                assert!(e.error.to_string().contains("max_steps exceeded"));
                *count_clone.lock().unwrap() += 1;
            });

        let mut ctx = Ctx::new();
        let _ = runner.run(S(0), &mut ctx);
        assert_eq!(*count.lock().unwrap(), 1);
    }

    #[test]
    fn on_step_receives_correct_step_number() {
        use std::sync::{Arc, Mutex};

        let steps = Arc::new(Mutex::new(Vec::new()));
        let steps_clone = Arc::clone(&steps);

        let wf = Workflow::builder("test")
            .register(RetryAgent {
                attempts: 0,
                succeed_on: 3,
            })
            .build()
            .unwrap();

        let mut runner = Runner::new(wf).on_step(move |e| {
            steps_clone
                .lock()
                .unwrap()
                .push((e.step_number, e.retries));
        });

        let mut ctx = Ctx::new();
        runner.run(S(0), &mut ctx).unwrap();

        let steps = steps.lock().unwrap();
        // 3 steps total: retry at step 1, retry at step 2, done at step 3
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0], (1, 0)); // first retry, 0 retries accumulated yet
        assert_eq!(steps[1], (2, 1)); // second retry, 1 retry accumulated
        assert_eq!(steps[2], (3, 2)); // success, 2 retries accumulated
    }

    // --- Outcome::Next ---

    struct NextAgent;
    impl Agent<S> for NextAgent {
        fn name(&self) -> &'static str {
            "next_agent"
        }
        fn run(&mut self, state: S, _ctx: &mut Ctx) -> StepResult<S> {
            Ok((S(state.0 + 1), Outcome::Next("done_agent")))
        }
    }

    #[test]
    fn next_jumps_to_named_agent() {
        let wf = Workflow::builder("test")
            .register(NextAgent)
            .register(DoneAgent)
            .build()
            .unwrap();

        let mut runner = Runner::new(wf);
        let mut ctx = Ctx::new();
        let result = runner.run(S(0), &mut ctx).unwrap();
        assert_eq!(result.0, 1);
    }

    // --- Outcome::Fail ---

    struct FailOutcomeAgent;
    impl Agent<S> for FailOutcomeAgent {
        fn name(&self) -> &'static str {
            "fail_outcome"
        }
        fn run(&mut self, state: S, _ctx: &mut Ctx) -> StepResult<S> {
            Ok((state, Outcome::Fail("reason".into())))
        }
    }

    #[test]
    fn fail_outcome_returns_step_error() {
        let wf = Workflow::builder("test")
            .register(FailOutcomeAgent)
            .build()
            .unwrap();

        let mut runner = Runner::new(wf);
        let mut ctx = Ctx::new();
        let err = runner.run(S(0), &mut ctx).err().unwrap();
        assert_eq!(err.to_string(), "reason");
    }

    // --- Continue without default_next ---

    #[test]
    fn continue_without_default_next_errors() {
        let wf = Workflow::builder("test")
            .register(AlwaysContinue)
            .build()
            .unwrap();

        let mut runner = Runner::new(wf);
        let mut ctx = Ctx::new();
        let err = runner.run(S(0), &mut ctx).err().unwrap();
        assert!(err.to_string().contains("no default next step"));
    }

    // --- Wait exceeds max_retries ---

    struct AlwaysWait;
    impl Agent<S> for AlwaysWait {
        fn name(&self) -> &'static str {
            "always_wait"
        }
        fn run(&mut self, state: S, _ctx: &mut Ctx) -> StepResult<S> {
            Ok((state, Outcome::Wait(Duration::from_millis(1))))
        }
    }

    #[test]
    fn wait_exceeds_max_retries() {
        let wf = Workflow::builder("test")
            .register(AlwaysWait)
            .build()
            .unwrap();

        let mut runner = Runner::new(wf).with_max_retries(1);
        let mut ctx = Ctx::new();
        let err = runner.run(S(0), &mut ctx).err().unwrap();
        assert!(err.to_string().contains("exceeded max retries"));
    }

    // --- Retry counter resets on step transition ---

    struct RetryOnceThenContinue {
        attempts: u32,
    }
    impl Agent<S> for RetryOnceThenContinue {
        fn name(&self) -> &'static str {
            "retry_once_then_continue"
        }
        fn run(&mut self, state: S, _ctx: &mut Ctx) -> StepResult<S> {
            self.attempts += 1;
            if self.attempts < 2 {
                Ok((state, Outcome::Retry(RetryHint::new("not yet"))))
            } else {
                Ok((state, Outcome::Continue))
            }
        }
    }

    #[test]
    fn retry_counter_resets_on_step_transition() {
        use std::sync::{Arc, Mutex};

        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = Arc::clone(&events);

        let wf = Workflow::builder("test")
            .register(RetryOnceThenContinue { attempts: 0 })
            .register(DoneAgent)
            .start_at("retry_once_then_continue")
            .then("done_agent")
            .build()
            .unwrap();

        let mut runner = Runner::new(wf).on_step(move |e| {
            events_clone
                .lock()
                .unwrap()
                .push((e.agent.to_string(), e.retries));
        });

        let mut ctx = Ctx::new();
        runner.run(S(0), &mut ctx).unwrap();

        let events = events.lock().unwrap();
        // retry_once_then_continue fires twice (retry then continue), done_agent fires once
        assert_eq!(events.len(), 3);
        // done_agent should have retries=0 (reset after transition)
        let done_event = events.iter().find(|(name, _)| name == "done_agent").unwrap();
        assert_eq!(done_event.1, 0);
    }
}
