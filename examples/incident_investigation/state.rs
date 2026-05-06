#[derive(Clone, Debug)]
pub struct IncidentState {
    pub title: String,
    pub logs: Vec<LogLine>,
    pub metrics: Vec<MetricPoint>,
    pub deploys: Vec<DeployEvent>,
    pub anomalies: Vec<Anomaly>,
    pub correlations: Vec<Correlation>,
    pub triage_note: String,
    pub report: String,
}

impl IncidentState {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            logs: vec![],
            metrics: vec![],
            deploys: vec![],
            anomalies: vec![],
            correlations: vec![],
            triage_note: String::new(),
            report: String::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LogLine {
    pub minute: u32,
    pub service: &'static str,
    pub level: &'static str,
    pub message: &'static str,
}

#[derive(Clone, Debug)]
pub struct MetricPoint {
    pub minute: u32,
    pub service: &'static str,
    pub metric: &'static str,
    pub value: f64,
}

#[derive(Clone, Debug)]
pub struct DeployEvent {
    pub minute: u32,
    pub service: &'static str,
    pub version: &'static str,
    pub summary: &'static str,
}

#[derive(Clone, Debug)]
pub struct Anomaly {
    pub service: String,
    pub metric: String,
    pub before_avg: f64,
    pub during_avg: f64,
    pub ratio: f64,
}

#[derive(Clone, Debug)]
pub struct Correlation {
    pub minute: u32,
    pub signal: String,
    pub evidence: String,
}
