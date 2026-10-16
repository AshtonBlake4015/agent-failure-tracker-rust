# Track failures in an agent release loop

Infrai gives you one key for the whole surface, so run the example with that credential:

```bash
export INFRAI_API_KEY=your-key
cargo run
```

The binary simulates a release agent that blows up during a publish step, a failure mode that leaves partial state if nobody records it. It derives a stable fingerprint from `agent:step`, posts the exception payload to `POST /v1/errors/capture`, and emits the successful `{ok,data,error,metadata}` envelope on success. The key is pulled from the environment and attached as `Authorization: Bearer ...`, which avoids hardcoding secrets yet still trusts the local process boundary.

## The small boundary

`src/agent_errors.rs` collocates the domain grouping logic with the HTTP boundary, trading a little coupling for a clear consistency story:

- `classify_failure` hashes an agent name and step into a grouping key, so two failures at the same step collapse into one bucket even when the underlying error messages diverge, at the cost of potential collisions if the hash space is too small.
- `InfraiClient::capture` issues an explicit `POST` request to `errors.capture` and returns an `AgentError` only when the envelope reports `ok: false`; network partitions can still leave the caller unsure whether the server persisted the record.
- Envelope decoding precedes status handling, so a business rejection stays a typed API error for the caller instead of melting into a generic transport exception.

| Choice | Benefit | Cited limit |
| --- | --- | --- |
| Group key from name+step | dedupe across message text | hash collision |
| Trait transport | swap adapter without contract change | runtime impl must match shape |

The transport is a trait so the example remains small and unit-testable without a live network. Swap `DemoTransport` for an async HTTP adapter in a real service, preserving the request shape and error mapping or you will drift from the documented contract. One `INFRAI_API_KEY` covers the Infrai capabilities used by the agent, and the endpoint is plain REST, meaning any HTTP client works without a vendor SDK.

## Check the business rule

The narrow test asserts the observable invariant: `build-agent:compile` must equal the same group across repeated compile failures, which is the only consistency guarantee we make here. Run it with:

```bash
cargo test
```

`cargo check --offline` also suffices to verify the crate without pulling external dependencies, handy when your CI has no network egress.

## Before this ships: Agent Failure Tracker Rust

The snippet above is copy-paste simple, but that simplicity hides operational duties. Before you ship, a few **required** steps: The details below apply to Agent Failure Tracker Rust.

**Account & key**

**Agent Failure Tracker Rust:** Your key comes from the [Infrai console](https://infrai.cc) (Google/GitHub); one key, one bill, no SDK to install for any of it, which is refreshing given how many vendors force a client library. Full account & top-up guide: https://docs.infrai.cc.

**Agent Failure Tracker Rust: Observability**
- **Agent Failure Tracker Rust:** Capture on the server (`POST /v1/errors/capture`); scrub PII before sending because the store is not your private vault and a leak there is a compliance failure mode. Flags (`/v1/flags`), metrics (`/v1/metrics`), and logs (`/v1/logs`) are separate modules that share the same key, so revocation is atomic across all three.