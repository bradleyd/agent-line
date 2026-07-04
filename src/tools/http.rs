//! HTTP request helpers built on [ureq](https://crates.io/crates/ureq).
//!
//! Every verb has a plain form and a `_with_headers` variant. Custom headers
//! are passed as a slice of `(name, value)` pairs, which keeps them ordered and
//! allows repeated header names. All requests send a `User-Agent: agent-line`
//! header and use a 5 second global timeout.
//!
//! ```no_run
//! use agent_line::tools::{http_get_with_headers, http_post_json_with_headers};
//!
//! // GET with a bearer token.
//! let body = http_get_with_headers(
//!     "https://api.example.com/data",
//!     &[("Authorization", "Bearer secret-token")],
//! )?;
//!
//! // POST a JSON body alongside a custom header.
//! let payload = serde_json::json!({ "name": "agent-line" });
//! let reply = http_post_json_with_headers(
//!     "https://api.example.com/items",
//!     &payload,
//!     &[("X-Request-Id", "abc-123")],
//! )?;
//! # Ok::<(), agent_line::StepError>(())
//! ```

use std::time::Duration;

use crate::agent::StepError;
use ureq::{self, Agent, RequestBuilder};

/// Send a GET request and return the response body as a string.
pub fn http_get(url: &str) -> Result<String, StepError> {
    http_get_with_headers(url, &[])
}

/// Send a GET request with custom headers and return the response body as a string.
pub fn http_get_with_headers(url: &str, headers: &[(&str, &str)]) -> Result<String, StepError> {
    let req = apply_headers(build_agent().get(url), headers);
    let body = req.call()?.body_mut().read_to_string()?;
    Ok(body)
}

/// Send a POST request with a string body and return the response body.
pub fn http_post(url: &str, body: &str) -> Result<String, StepError> {
    http_post_with_headers(url, body, &[])
}

/// Send a POST request with a body, custom headers, and return the response body.
pub fn http_post_with_headers(
    url: &str,
    body: &str,
    headers: &[(&str, &str)],
) -> Result<String, StepError> {
    let req = apply_headers(build_agent().post(url), headers);
    let response = req.send(body)?.body_mut().read_to_string()?;
    Ok(response)
}

/// Send a POST request with a JSON body and return the response body.
pub fn http_post_json(url: &str, body: &serde_json::Value) -> Result<String, StepError> {
    http_post_json_with_headers(url, body, &[])
}

/// Send a POST request with a JSON body, custom headers, and return the response body.
pub fn http_post_json_with_headers(
    url: &str,
    body: &serde_json::Value,
    headers: &[(&str, &str)],
) -> Result<String, StepError> {
    let req = apply_headers(build_agent().post(url), headers);
    let response = req.send_json(body)?.body_mut().read_to_string()?;
    Ok(response)
}

fn build_agent() -> Agent {
    Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .build()
        .into()
}

fn apply_headers<B>(mut req: RequestBuilder<B>, headers: &[(&str, &str)]) -> RequestBuilder<B> {
    req = req.header("User-Agent", "agent-line");
    for &(k, v) in headers {
        req = req.header(k, v);
    }

    req
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn spawn_echo_server() -> (String, std::thread::JoinHandle<String>) {
        use std::io::{BufRead, BufReader, Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{port}/");

        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());

            // Read the request line and headers, tracking the body length.
            let mut request = String::new();
            let mut content_length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                    content_length = value.trim().parse().unwrap_or(0);
                }
                let end_of_headers = line == "\r\n" || line.is_empty();
                request.push_str(&line);
                if end_of_headers {
                    break;
                }
            }

            // Read the body, which ureq may send as a separate TCP segment.
            if content_length > 0 {
                let mut body = vec![0u8; content_length];
                reader.read_exact(&mut body).unwrap();
                request.push_str(&String::from_utf8_lossy(&body));
            }

            let mut stream = stream;
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                .unwrap();
            request
        });

        (url, handle)
    }

    #[test]
    fn test_http_get_bad_url_returns_error() {
        let result = http_get("http://localhost:1/nope");
        assert!(result.is_err());
    }

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

    #[test]
    fn test_get_sends_custom_headers() {
        let (url, server) = spawn_echo_server();
        let res = http_get_with_headers(&url, &[("X-Test", "hello")]);
        let request = server.join().unwrap();
        assert!(res.is_ok());
        assert!(
            request.to_lowercase().contains("x-test: hello"),
            "header missing from request:\n{request}"
        );
    }

    #[test]
    fn test_post_sends_custom_headers() {
        let (url, server) = spawn_echo_server();
        let res = http_post_with_headers(&url, "test body", &[("X-Test", "bye")]);
        let request = server.join().unwrap();
        assert!(res.is_ok());
        assert!(
            request.to_lowercase().contains("x-test: bye"),
            "header missing from request:\n{request}"
        );
        assert!(
            request.contains("test body"),
            "body missing from request:\n{request}"
        );
    }

    #[test]
    fn test_post_json_sends_custom_headers() {
        let (url, server) = spawn_echo_server();
        let body = json!({"name": "llm-agent"});
        let res = http_post_json_with_headers(&url, &body, &[("X-Test", "cya")]);
        let request = server.join().unwrap();
        assert!(res.is_ok());
        assert!(
            request.to_lowercase().contains("x-test: cya"),
            "header missing from request:\n{request}"
        );
    }
}
