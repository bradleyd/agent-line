use crate::state::{Correlation, IncidentState};
use agent_line::{Agent, Ctx, Outcome, StepResult};

pub struct CorrelateTimeline;

impl Agent<IncidentState> for CorrelateTimeline {
    fn name(&self) -> &'static str {
        "correlate_timeline"
    }

    fn run(&mut self, mut state: IncidentState, ctx: &mut Ctx) -> StepResult<IncidentState> {
        let mut correlations = Vec::new();

        for deploy in &state.deploys {
            let nearby_errors = state
                .logs
                .iter()
                .filter(|log| {
                    log.minute >= deploy.minute
                        && log.minute <= deploy.minute + 8
                        && (log.level == "WARN" || log.level == "ERROR")
                })
                .count();

            if nearby_errors > 0 {
                correlations.push(Correlation {
                    minute: deploy.minute,
                    signal: format!(
                        "{} {} deploy preceded {} warning/error logs",
                        deploy.service, deploy.version, nearby_errors
                    ),
                    evidence: deploy.summary.to_string(),
                });
            }
        }

        for anomaly in state.anomalies.iter().take(4) {
            let related_logs: Vec<_> = state
                .logs
                .iter()
                .filter(|log| log.service == anomaly.service)
                .filter(|log| log.level == "WARN" || log.level == "ERROR")
                .map(|log| format!("m{} {}: {}", log.minute, log.level, log.message))
                .collect();

            correlations.push(Correlation {
                minute: 4,
                signal: format!(
                    "{}.{} rose {:.1}x during incident",
                    anomaly.service, anomaly.metric, anomaly.ratio
                ),
                evidence: related_logs.join(" | "),
            });
        }

        correlations.sort_by_key(|c| c.minute);
        state.correlations = correlations;

        ctx.log(format!(
            "built {} deploy/log/metric correlations",
            state.correlations.len()
        ));
        Ok((state, Outcome::Continue))
    }
}
