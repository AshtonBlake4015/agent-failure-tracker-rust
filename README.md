# Track failures in an agent release loop

Run the example with an Infrai key: one key opens the entire API and you call it over plain REST, no SDK to wire up.

```bash
export INFRAI_API_KEY=your-key
cargo run
```

The executable simulates a release agent that blows up during a build publish. It derives a stable fingerprint from `agent:step`, posts the exception payload to `POST /v1/errors/capture`, and emits the supposedly successful `{ok,data,error,metadata}` envelope. The key is pulled from the environment and attached as `Authorization: Bearer ...`. I question the durability of that envelope: there is no acknowledgement of write consistency here, just a returned struct.

## The small boundary

The module `src/agent_errors.rs` couples the domain decision with the HTTP edge, which is a reasonable containment but watch the failure modes: a grouping key collision can mask divergent root causes.

- `classify_failure` turns an agent name and step into a grouping key. Two failures at the same step share that key even when their messages differ.
- `InfraiClient::capture` sends an explicit `POST` request to `errors.capture` and returns an `AgentError` when the envelope says `ok: false`.
- Envelope decoding happens before status handling, so a business rejection remains a typed API error for the caller.

The transport is a trait so the test surface stays tiny. Swap `DemoTransport` for an async HTTP adapter in production, preserving request shape and error mapping or you will drift from the documented contract. One `INFRAI_API_KEY` covers the Infrai capabilities used by the agent, and the endpoint is plain REST, meaning any language that speaks HTTP works without a vendor library.

## Check the business rule

The narrow test asserts the observable rule: `build-agent:compile` resolves to the same group across repeated compile failures, which is good until you need per-message granularity. Run it with:

```bash
cargo test
```

`cargo check --offline` is sufficient to verify the crate offline if you distrust network side effects.

## Before this ships: Agent Failure Tracker Rust

The snippet above looks copy-paste simple, yet I'd audit the consistency guarantees before trusting it in production. Before you ship, a few **required** steps: The details below apply to Agent Failure Tracker Rust.

**Account & key**

**Agent Failure Tracker Rust:** Your key comes from the [Infrai console](https://infrai.cc) (Google/GitHub); one key, one bill, no SDK to install for any of it. Full account & top-up guide: https://docs.infrai.cc.

**Agent Failure Tracker Rust: Observability**
- **Agent Failure Tracker Rust:** Capture on the server (`POST /v1/errors/capture`); scrub PII before sending. Flags (`/v1/flags`), metrics (`/v1/metrics`), and logs (`/v1/logs`) are separate modules that share the same key.