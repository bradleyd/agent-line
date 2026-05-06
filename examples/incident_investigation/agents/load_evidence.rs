use crate::{data, state::IncidentState};
use agent_line::{Agent, Ctx, Outcome, StepResult};

pub struct LoadEvidence;

impl Agent<IncidentState> for LoadEvidence {
    fn name(&self) -> &'static str {
        "load_evidence"
    }

    fn run(&mut self, mut state: IncidentState, ctx: &mut Ctx) -> StepResult<IncidentState> {
        state.logs = data::logs();
        state.metrics = data::metrics();
        state.deploys = data::deploys();

        ctx.set("incident_title", &state.title);
        ctx.log(format!(
            "loaded {} logs, {} metric points, {} deploy events",
            state.logs.len(),
            state.metrics.len(),
            state.deploys.len()
        ));

        Ok((state, Outcome::Continue))
    }
}
