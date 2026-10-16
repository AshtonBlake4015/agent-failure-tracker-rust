use std::env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentError {
    Transport(String),
    Api { code: String, message: String, status: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureEvent {
    pub agent: String,
    pub step: String,
    pub message: String,
    pub fingerprint: String,
}

pub trait HttpTransport {
    fn send(&mut self, request: Request) -> Result<Response, AgentError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub body: String,
    pub authorization: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub body: String,
    pub retry_after_seconds: Option<u64>,
}

pub struct InfraiClient<T> {
    transport: T,
    key: String,
}

impl<T: HttpTransport> InfraiClient<T> {
    pub fn from_env(transport: T) -> Result<Self, AgentError> {
        let key = env::var("INFRAI_API_KEY")
            .map_err(|_| AgentError::Transport("INFRAI_API_KEY is required".into()))?;
        Ok(Self { transport, key })
    }

    pub fn capture(&mut self, event: &FailureEvent) -> Result<String, AgentError> {
        let body = format!(
            "{{\"title\":\"agent step failed\",\"message\":\"{}\",\"fingerprint\":[\"{}\"],\"exception\":\"{}\"}}",
            json_escape(&event.message), json_escape(&event.fingerprint), json_escape(&event.message)
        );
        let request = Request {
            method: "POST".into(),
            path: "/v1/errors/capture".into(),
            body,
            authorization: format!("Bearer {}", self.key),
        };
        let mut response = self.transport.send(request.clone())?;
        if response.status == 429 {
            let delay = response.retry_after_seconds.unwrap_or(1);
            std::thread::sleep(std::time::Duration::from_secs(delay));
            response = self.transport.send(request)?;
        }
        decode_envelope(response)
    }

    pub async fn capture_async(&mut self, event: &FailureEvent) -> Result<String, AgentError> {
        self.capture(event)
    }
}

fn decode_envelope(response: Response) -> Result<String, AgentError> {
    // Decode the business envelope before interpreting the HTTP status.
    if response.body.contains("\"ok\":true") {
        return Ok(response.body);
    }
    let code = extract_json_value(&response.body, "code").unwrap_or_else(|| "API_ERROR".into());
    let message = extract_json_value(&response.body, "message").unwrap_or_else(|| "request rejected".into());
    Err(AgentError::Api { code, message, status: response.status })
}

fn extract_json_value(body: &str, key: &str) -> Option<String> {
    let marker = format!("\"{}\":\"", key);
    let start = body.find(&marker)? + marker.len();
    let rest = &body[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn classify_failure(agent: &str, step: &str, message: &str) -> FailureEvent {
    FailureEvent {
        agent: agent.into(),
        step: step.into(),
        message: message.into(),
        fingerprint: format!("{}:{}", agent, step),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct RecordingTransport { response: Response, request: Option<Request> }
    impl HttpTransport for RecordingTransport {
        fn send(&mut self, request: Request) -> Result<Response, AgentError> {
            self.request = Some(request);
            Ok(self.response.clone())
        }
    }

    #[test]
    fn groups_repeated_step_failures_by_agent_and_step() {
        let first = classify_failure("build-agent", "compile", "missing crate");
        let second = classify_failure("build-agent", "compile", "different detail");
        assert_eq!(first.fingerprint, second.fingerprint);
        assert_eq!(first.fingerprint, "build-agent:compile");
    }

    #[test]
    fn reads_success_envelope_before_status() {
        std::env::set_var("INFRAI_API_KEY", "test-key");
        let transport = RecordingTransport {
            response: Response { status: 201, body: "{\"ok\":true,\"data\":{\"event_id\":\"evt-1\"}}".into(), retry_after_seconds: None },
            request: None,
        };
        let mut client = InfraiClient::from_env(transport).unwrap();
        let result = client.capture(&classify_failure("a", "b", "boom")).unwrap();
        assert!(result.contains("evt-1"));
    }
}
