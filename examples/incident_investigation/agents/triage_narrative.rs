use crate::state::IncidentState;
use agent_line::{Agent, Ctx, LlmConfig, Outcome, StepResult};
use std::fmt::Write;

pub struct TriageNarrative {
    llm: LlmConfig,
}

impl TriageNarrative {
    pub fn new(llm: LlmConfig) -> Self {
        Self { llm }
    }
}

impl Agent<IncidentState> for TriageNarrative {
    fn name(&self) -> &'static str {
        "triage_narrative"
    }

    fn run(&mut self, mut state: IncidentState, ctx: &mut Ctx) -> StepResult<IncidentState> {
        let prompt = evidence_prompt(&state);

        state.triage_note = self
            .llm
            .request()
            .system(
                "You are helping an incident commander correlate telemetry quickly. \
                 Summarize evidence only. Do not claim certainty or prescribe automation.",
            )
            .user(prompt)
            .send()?;

        ctx.log("fast model produced triage narrative");
        Ok((state, Outcome::Continue))
    }
}

fn evidence_prompt(state: &IncidentState) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Incident: {}", state.title);
    let _ = writeln!(out, "\nTop anomalies:");
    for anomaly in state.anomalies.iter().take(5) {
        let _ = writeln!(
            out,
            "- {}.{} baseline {:.1}, incident {:.1}, ratio {:.1}x",
            anomaly.service, anomaly.metric, anomaly.before_avg, anomaly.during_avg, anomaly.ratio
        );
    }

    let _ = writeln!(out, "\nCorrelations:");
    for correlation in &state.correlations {
        let _ = writeln!(
            out,
            "- m{}: {} ({})",
            correlation.minute, correlation.signal, correlation.evidence
        );
    }

    out
}
