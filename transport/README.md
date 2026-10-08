# Native transport

MIT Rust HTTP/WebSocket client for **0.2.1-alpha.1**. [Core](<../core/README.md>) owns authentication; widgets never receive launch URLs, cookies or tokens.

## Operations

| Area | Supported |
|---|---|
| Sessions | List, create, prompt, history, projections, model/effort selection |
| Workspaces | Create/follow, pin/unpin, archive/restore |
| Decisions | Correlated approval/question replies, cancellation, claim ownership |
| Files | Bounded staging and root-only ZIP export |
| Plugins | Inventory, nonsecret live descriptors, revision-checked field mutation |

[Wire reference](<../docs/NATIVE-TRANSPORT.md>) · [File staging](<../docs/NATIVE-FILE-UPLOAD.md>) · [Export](<../docs/NATIVE-SESSION-EXPORT.md>)

## Privacy and lifecycle

- Only authenticated HTTP `127.0.0.1` launch URLs with explicit port; exact 303 response and scoped HttpOnly/SameSite cookie required.
- No redirects, system proxy, external target or generic privileged dispatcher.
- Client handles are worker capabilities—not serializable GUI state. No credential getters.
- Core stop/fault/exit synchronously revokes clones; `close()` also waits for actor quiescence.
- One physical mux; logical streams cancel independently. Retained unpolled HTTP futures must be polled/dropped before awaiting close.
- Explicit catalog requests may cause Host-side provider I/O; startup must not request them automatically.

## Mutation rules

- ACK settles delivery, **not tool execution, generation completion or idle**.
- [Decision replies](<../docs/NATIVE-DECISION-OWNERSHIP.md>) use exact delivery instances; stale replies cannot acquire reused IDs.
- [Question claims](<../docs/NATIVE-TIMED-QUESTIONS.md>) retain Host ownership through termination. Dropping a claim is unsubscribe, not question cancellation; native timed controls remain disabled.
- [Registry receipts](<../docs/NATIVE-SESSION-MANAGEMENT.md#stream-authoritative-state-and-races>) preserve global order. Follow streams stay authoritative; receipts carry no comparable revision.
- Plugin edits re-read membership/type/writability, then set one reviewed path with `expectedRevision`. No secrets, raw config editor or conflict retry.
- Errors/timeouts may follow commit. **No automatic retry or rollback assumption.**

## Default limits

| Resource | Cap |
|---|---:|
| Complete HTTP result/WS frame | 32 MiB; configurable to 128 MiB |
| JSON depth / values | 64 / 100,000 |
| Queued inbound payload | 256 KiB |
| Streams / RPCs / pending waterfalls | 16 / 8 / 128 |
| Complete JSON uplink | 262,144 bytes |
| File upload | 4 MiB; eight jobs |
| Plugin result / JSON values | 1 MiB / 32,768 |
| Plugin rows / fields | 512 each |

Malformed frames, gaps, overflow and closure return fixed redacted errors. Snapshots fail whole, never silently truncate. Recover with a fresh snapshot, not a guessed cursor.

## Validation and gaps

```sh
cargo test --locked -j2 -- --test-threads=2
```

- [Fake-wire tests](<tests/transport.rs>): envelopes, cookies, correlation, limits, cancellation and privacy; not real Host behavior.
- Scripted real backend checks: authentication, baselines, session/history operations and clean stop. Evidence remains local.
- Semantic chat folding, accessibility and reconnect UI belong to the app.
- Use registered `AgentId` authority, never a substituted `SessionId`. Uploads intentionally use `SessionId`.
- No browser guests, terminal, Office, updater or full attachment/native parity.
