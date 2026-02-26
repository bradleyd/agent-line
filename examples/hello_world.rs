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

    fn run(&mut self, state: State, _ctx: &mut Ctx) -> StepResult<State> {
        let next = State { n: state.n + 1 };
        Ok((next, Outcome::Done))
    }
}

fn main() {
    let mut ctx = Ctx::new();
    let mut agent = AddOne;
    let (state, out) = agent.run(State { n: 1 }, &mut ctx).unwrap();
    println!("state.n={} outcome={out:?}", state.n);
}
