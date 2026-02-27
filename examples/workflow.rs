use agent_line::{Agent, Ctx, Outcome, Runner, StepResult, Workflow};

#[derive(Clone)]
struct State {
    n: i32,
}

struct AddOne;
impl Agent<State> for AddOne {
    fn name(&self) -> &'static str {
        "add_one"
    }
    fn run(&mut self, state: State, _ctx: &mut Ctx) -> StepResult<State> {
        Ok((State { n: state.n + 1 }, Outcome::Continue))
    }
}

struct StopAtThree;
impl Agent<State> for StopAtThree {
    fn name(&self) -> &'static str {
        "stop"
    }
    fn run(&mut self, state: State, _ctx: &mut Ctx) -> StepResult<State> {
        if state.n >= 3 {
            Ok((state, Outcome::Done))
        } else {
            Ok((state, Outcome::Next("add_one")))
        }
    }
}

fn main() {
    let mut ctx = Ctx::new();
    let wf = Workflow::builder("demo")
        .register(AddOne)
        .register(StopAtThree)
        .start_at("add_one")
        .then("stop")
        .build()
        .unwrap();

    let final_state = Runner::new(wf).run(State { n: 0 }, &mut ctx).unwrap();
    println!("final n={}", final_state.n);
}
