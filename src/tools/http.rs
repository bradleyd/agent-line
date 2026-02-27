use std::time::Duration;

use crate::agent::StepError;
use ureq::{self, Agent};

pub fn http_get(url: &str) -> Result<String, StepError> {
    let config = Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .build();

    let agent: Agent = config.into();

    let body: String = agent
        .get(url)
        .header("Example-Header", "header value")
        .call()?
        .body_mut()
        .read_to_string()?;

    Ok(body)
}
