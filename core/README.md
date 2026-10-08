# Native lifecycle core

MIT Rust library owning a private Node Host. No GUI or model engine. Optional `transport` connects the [typed client](<../transport/README.md>).

## API

| Call | Purpose |
|---|---|
| `Backend::start(options)` | Start owned Host; return handle and public events |
| `wait_ready()` | Wait for authenticated readiness |
| `request(command, timeout)` | Typed, bounded control request |
| `stop_async()` | Queue teardown without blocking UI |
| `stop()` | Stop and join worker threads |
| `is_known_idle()` | Check confirmed idle; unknown is not idle |

- Start, requests and blocking stop belong on workers, never the UI thread.
- Drop requests teardown; keep the owner alive until completion.
- Commands are fixed, not a generic JSON dispatcher. Widgets receive metadata, never credentials.
- [Account observation](<../docs/NATIVE-LAZY-ACCOUNT.md>) is opt-in; [Agent discovery](<../docs/NATIVE-AGENT-DISCOVERY.md>) returns registered identity without activation.

## Launch requirements

- Absolute built runtime, exact `0.2.1-alpha.1`, isolated native home, user home and workspace.
- No installed-runtime fallback or legacy-home aliases.
- Node defaults to `/usr/bin/node`; overrides must resolve to executable regular files. No PATH lookup.
- Child environment is cleared; fixed PATH `/usr/bin:/bin`. Ambient credentials and `NODE_OPTIONS` are not inherited.
- Embedded supervisors are staged privately. `DSH_TAURI:` is framing compatibility, not a Tauri dependency.

## Bounds

| Resource | Limit |
|---|---:|
| Startup / reply | 30 s |
| Each stop phase | 10 s |
| Control write | 500 ms |
| Complete input/output frame | 65,536 bytes |
| Events / in-flight requests | 128 each |

Malformed packets, partial writes and early EOF fail closed. Dropped events are counted; stderr is discarded.

## Secrets and authorization

- Launch URLs, tokens and cookies stay private; stop/fault/exit revokes every transport clone.
- `SecretApiKey` and `SecretCodexCallback` consume 1–16,384 printable non-whitespace ASCII bytes; no plaintext getter, serialization or cloning.
- API-key save: true confirms persistence, false means refusal, errors/timeouts are **indeterminate**. Presence cannot identify the saved key; never retry automatically.
- Default credential storage: 0700 directories/0600 files. Custom providers may write elsewhere.
- Wiping covers owned Rust input/frames—not GUI, Node, kernel, clipboard or swap copies.
- [Codex](<../docs/NATIVE-CODEX.md>): explicit status/start/callback/browser controls; fixed provider and revision-checked route activation. No generic browser or credential-read access.

## Process ownership

- Persistent spawning thread owns Node; Linux parent-death notification follows that thread.
- Owned descendants use PID/start-time checks and pidfds, not desktop-wide scans.
- Busy control writers: retry only an unwritten shutdown frame, within the original grace deadline. Never replay partial/poisoned frames.
- Graceful stop requires ACK plus successful root exit. Forced/incomplete cleanup reports uncertainty.
- Cooperative lifecycle management, **not adversarial containment**; escaped unobserved descendants require stronger cgroup ownership.

## Checks and examples

```sh
cargo test --locked -j2 -- --test-threads=2
cargo test --locked -j2 --features transport -- --test-threads=2
```

[Lifecycle smoke](<examples/smoke.rs>) · [Transport smoke](<examples/transport_smoke.rs>) · [PUBLIC key-save fixture](<examples/api_key_smoke.rs>) · [File upload](<../docs/NATIVE-FILE-UPLOAD.md>)

Fixtures and scripted backend checks do not establish real-key validity, physical input or full app parity. Generated evidence stays local.
