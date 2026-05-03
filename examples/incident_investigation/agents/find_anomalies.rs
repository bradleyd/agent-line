use crate::state::{Anomaly, IncidentState};
use agent_line::{Agent, Ctx, Outcome, StepResult};
use std::collections::BTreeMap;

pub struct FindAnomalies;

type MetricKey<'a> = (&'a str, &'a str);
type MetricBuckets = (Vec<f64>, Vec<f64>);

impl Agent<IncidentState> for FindAnomalies {
    fn name(&self) -> &'static str {
        "find_anomalies"
    }

    fn run(&mut self, mut state: IncidentState, ctx: &mut Ctx) -> StepResult<IncidentState> {
        let mut grouped: BTreeMap<MetricKey<'_>, MetricBuckets> = BTreeMap::new();

        for point in &state.metrics {
            let (baseline, incident) = grouped
                .entry((point.service, point.metric))
                .or_insert_with(|| (Vec::new(), Vec::new()));

            if point.minute <= 3 {
                baseline.push(point.value);
            } else if (4..=10).contains(&point.minute) {
                incident.push(point.value);
            }
        }

        state.anomalies = grouped
            .into_iter()
            .filter_map(|((service, metric), (baseline, incident))| {
                let before_avg = average(&baseline)?;
                let during_avg = average(&incident)?;
                let ratio = if before_avg == 0.0 {
                    during_avg
                } else {
                    during_avg / before_avg
                };

                (ratio >= 1.5).then(|| Anomaly {
                    service: service.to_string(),
                    metric: metric.to_string(),
                    before_avg,
                    during_avg,
                    ratio,
                })
            })
            .collect();

        state.anomalies.sort_by(|a, b| b.ratio.total_cmp(&a.ratio));

        ctx.log(format!(
            "identified {} anomalous metrics",
            state.anomalies.len()
        ));
        Ok((state, Outcome::Continue))
    }
}

fn average(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}
