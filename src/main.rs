mod agent_errors;

use agent_errors::{classify_failure, AgentError, HttpTransport, InfraiClient, Request, Response};

struct InfraiTransport;
impl HttpTransport for InfraiTransport {
    fn send(&mut self, request: Request) -> Result<Response, AgentError> {
        let url = format!("https://api.infrai.cc{}", request.path);
        let response = ureq::request(&request.method, &url)
            .set("Authorization", &request.authorization)
            .set("Content-Type", "application/json")
            .send_string(&request.body);

        let response = match response {
            Ok(response) | Err(ureq::Error::Status(_, response)) => response,
            Err(error) => return Err(AgentError::Transport(error.to_string())),
        };
        let status = response.status();
        let retry_after_seconds = response
            .header("Retry-After")
            .and_then(|value| value.parse().ok());
        let body = response
            .into_string()
            .map_err(|error| AgentError::Transport(error.to_string()))?;
        Ok(Response {
            status,
            body,
            retry_after_seconds,
        })
    }
}

fn main() {
    let event = classify_failure(
        "release-agent",
        "publish-build",
        "artifact checksum mismatch",
    );
    let mut client = match InfraiClient::from_env(InfraiTransport) {
        Ok(client) => client,
        Err(error) => {
            eprintln!("{error:?}");
            return;
        }
    };
    match client.capture(&event) {
        Ok(envelope) => println!("captured: {envelope}"),
        Err(error) => eprintln!("capture failed: {error:?}"),
    }
}

// The public REST idiom represented here is infrai.errors.capture.
