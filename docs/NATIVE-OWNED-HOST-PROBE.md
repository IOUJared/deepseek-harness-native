# Same-owned-Host recovery proof

## Core API

With the `transport` feature, `Backend::probe_owned_host(self: &Arc<Self>, timeout: Duration) -> Result<NativeClient, Error>` is an async, read-only check. It sends the existing correlated `Inspect` command through this Backend's original inherited private pipe, then returns a clone from its already initialized Core-owned transport slot. It never creates a Host, performs another authentication exchange, accepts an endpoint, promotes an Agent, follows a Session or reconciles an uncertain Codex mutation.

The retained `Arc<Backend>` keeps this exact owner alive while the bounded blocking worker runs. Before and after the matching private ACK, Core checks readiness, registered transport, root lifetime, stop/fault state, writer framing, private-channel EOF and the owned transport cancellation token. Invalid timeouts, missing initial transport, malformed/mismatched replies, EOF, faults and stop races fail closed. Timeout must be positive and at most 30 seconds; dropping the caller future does not cancel an already admitted bounded worker.

## Consumer obligations

- Use only the returned cached handle for an explicitly authorized read-only rejoin or refresh. No other Host endpoint or reconstructed client is accepted by this API.
- Proof means the original owned Host answered at that point. It does not prove idle state, HTTP/WS health, entitlement, prompt acceptance or cancellation. An inspection with unknown task counts is still an authenticated private-pipe ACK, not known idle.
- The handle shares the existing connection lifetime. If any clone was closed, proof may return that same closed handle; its read fails `Closed`, without reauthentication or replacement. A stop immediately after proof likewise revokes every clone.
- Do not replay a prompt or mutating request, automatically restart the Host, clear uncertain receipt ownership or resume an Agent on proof alone. Those decisions belong to the explicit recovery consumer.

Core revokes the transport clone and in-flight exchange immediately when its private pipe reaches EOF or reports a protocol fault, before publishing that state or waiting for monitor escalation. Stop and Host exit retain their existing revocation and drainage.

[Private-pipe and loopback-HTTP fixtures](<../core/src/owned_host_probe_tests.rs>) exercise cached-clone identity, no second authentication, timeout, invalid admission, mismatched ACK, stop-after-ACK fencing, immediate fault/EOF revocation and poisoned framing. They use synthetic public credentials and do not qualify physical UI recovery or real provider work.
