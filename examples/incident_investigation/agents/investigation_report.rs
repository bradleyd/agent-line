use crate::state::IncidentState;
use agent_line::{Agent, Ctx, LlmConfig, Outcome, StepResult};
use std::fmt::Write;

pub struct InvestigationReport {
    llm: LlmConfig,
}

impl InvestigationReport {
    pub fn new(llm: LlmConfig) -> Self {
        Self { llm }
    }
}

impl Agent<IncidentState> for InvestigationReport {
    fn name(&self) -> &'static str {
        "investigation_report"
    }

    fn run(&mut self, mut state: IncidentState, ctx: &mut Ctx) -> StepResult<IncidentState> {
        let prompt = report_prompt(&state);

        state.report = self
            .llm
            .request()
            .system(
                "You are an incident investigation assistant. Produce a concise report that helps \
                 humans decide what to check next. Separate evidence from hypotheses. Avoid \
                 irreversible remediation instructions.",
            )
            .user(prompt)
            .send()?;

        ctx.log("strong model produced investigation report");
        Ok((state, Outcome::Done))
    }
}

fn report_prompt(state: &IncidentState) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Incident: {}", state.title);

    let _ = writeln!(out, "\nFast triage note:\n{}", state.triage_note);

    let _ = writeln!(out, "\nDeploys:");
    for deploy in &state.deploys {
        let _ = writeln!(
            out,
            "- m{} {} {}: {}",
            deploy.minute, deploy.service, deploy.version, deploy.summary
        );
    }

    let _ = writeln!(out, "\nWarning and error logs:");
    for log in state
        .logs
        .iter()
        .filter(|log| log.level == "WARN" || log.level == "ERROR")
    {
        let _ = writeln!(
            out,
            "- m{} {} {}: {}",
            log.minute, log.service, log.level, log.message
        );
    }

    let _ = writeln!(
        out,
        "\nReturn sections: likely correlation, supporting evidence, counter-evidence, \
         next checks for a human operator, and safe short-term mitigations to consider."
    );

    out
}
