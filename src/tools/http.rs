use std::time::Duration;

use crate::agent::StepError;
use ureq::{self, Agent};

/// Send a GET request and return the response body as a string.
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

/// Send a POST request with a string body and return the response body.
pub fn http_post(url: &str, body: &str) -> Result<String, StepError> {
    let config = Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .build();

    let agent: Agent = config.into();

    let response = agent.post(url).send(body)?.body_mut().read_to_string()?;

    Ok(response)
}

/// Send a POST request with a JSON body and return the response body.
pub fn http_post_json(url: &str, body: &serde_json::Value) -> Result<String, StepError> {
    let config = Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .build();

    let agent: Agent = config.into();

    let response = agent
        .post(url)
        .send_json(body)?
        .body_mut()
        .read_to_string()?;

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_post_bad_url_returns_error() {
        let result = http_post("http://localhost:1/nope", "body content");
        assert!(result.is_err());
    }

    #[test]
    fn test_http_post_json_bad_url_returns_error() {
        let body = serde_json::json!({"key": "value"});
        let result = http_post_json("http://localhost:1/nope", &body);
        assert!(result.is_err());
    }
}
