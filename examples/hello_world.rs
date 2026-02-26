use agent_line::{Agent, Ctx, Outcome, StepResult};

#[derive(Clone)]
struct State {
    n: i32,
}

struct AddOne;

impl Agent<State> for AddOne {
    fn name(&self) -> &'static str {
        "add_one"
    }

    fn run(&mut self, state: State, _ctx: &Ctx) -> StepResult<State> {
        let next = State { n: state.n + 1 };
        Ok((next, Outcome::Done))
    }
}

fn main() {
    let ctx = Ctx;
    let mut agent = AddOne;
    let (state, out) = agent.run(State { n: 1 }, &ctx).unwrap();
    println!("state.n={} outcome={out:?}", state.n);
}
