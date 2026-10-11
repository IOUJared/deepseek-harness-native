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
| `Arc<Backend>::probe_owned_host(timeout).await` | Read-only original-pipe proof; return the existing transport clone (`transport` feature) |

- Start, requests and blocking stop belong on workers, never the UI thread.
- Drop requests teardown; keep the owner alive until completion.
- Commands are fixed, not a generic JSON dispatcher. Widgets receive metadata, never credentials.
- [Account observation](<../docs/NATIVE-LAZY-ACCOUNT.md>) is opt-in; [Agent discovery](<../docs/NATIVE-AGENT-DISCOVERY.md>) returns registered identity without activation.
- [Same-owned-Host proof](<../docs/NATIVE-OWNED-HOST-PROBE.md>) returns only the cached transport after a correlated original-pipe ACK; it proves neither idle nor HTTP/WS health and never authenticates, restarts or replays a mutation.

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

Malformed packets, partial writes and early EOF fail closed. Dropped events are counted; stderr is discarded. Node line framing counts decoded UTF-8 bytes, including line endings, incrementally; oversized lines stay discarded through their newline. Invalid UTF-8 replacement bytes count toward the limit.

## Secrets and authorization

- Launch URLs, tokens and cookies stay private; stop/fault/exit revokes every transport clone.
- `SecretApiKey` and `SecretCodexCallback` consume 1–16,384 printable non-whitespace ASCII bytes; no plaintext getter, serialization or cloning.
- API-key save: true confirms persistence, false means refusal, errors/timeouts are **indeterminate**. Presence cannot identify the saved key; never retry automatically.
- Default credential storage: 0700 directories/0600 files. Custom providers may write elsewhere.
- Wiping covers owned Rust input/frames—not GUI, Node, kernel, clipboard or swap copies.
- [Codex](<../docs/NATIVE-CODEX.md>): explicit status/start/callback/browser controls; fixed provider and revision-checked route activation. No generic browser or credential-read access.
- [Managed Codex accounts](<../docs/NATIVE-MANAGED-CODEX.md>): non-Copy `CodexAccountsMetadata`, branded redacted `CodexAccountId`/`CodexAccountLabel`, explicit Add/select/local logout with epoch checks. Add never selects; unknown writes require explicit `CodexAccountsStatus` reconciliation. No email, plan or quota is inferred.

- [Cloudflare](<../docs/NATIVE-CLOUDFLARE.md>): revision-checked configuration, explicit Workers Free attestation and [browser OAuth](<../docs/NATIVE-CLOUDFLARE-OAUTH.md>). Private setup-bound PKCE browser capability; separate reserved Cancel and owned opener permit. Grants stay in the Host; uncertain writes never retry. Explicit credential-mode changes disable features until reviewed.
- [Codex usage preferences](<../docs/NATIVE-CODEX-USAGE-POLICY.md>): typed local load/save, durable revision checks and explicit conflict/unknown outcomes. No quota, credential, notification or retry access.
- [Codex runtime settings](<../docs/NATIVE-CODEX-USAGE-POLICY.md#remaining-supported-controls>): typed `CodexRuntimeRead` / `CodexRuntimeSave` expose eight fixed transport/cache/timeout/image-input fields through public Settings CAS. Closed snapshots distinguish effective values, inherited defaults and actual user overrides; no credential, endpoint or arbitrary provider access. Native Models/Images editors retain drafts and require explicit reconciliation after uncertainty.

The lifecycle smoke also accepts exclusive `--cloudflare-oauth-stage initial|restored`: PUBLIC client/scope configuration, explicit saved-token mode selection and restart persistence, with both features off. It never starts OAuth or launches a browser; synthetic callback qualification and live authorization are separate checks.

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

[Lifecycle smoke](<examples/smoke.rs>) · [Transport smoke](<examples/transport_smoke.rs>) · [PUBLIC key-save fixture](<examples/api_key_smoke.rs>) · [Cold prompt-admission smoke](<examples/prompt_admission_smoke.rs>) · [File upload](<../docs/NATIVE-FILE-UPLOAD.md>)

The lifecycle smoke accepts `--usage-stage initial|restored|corrupt` for [preference-only pipe qualification](<../docs/NATIVE-CODEX-USAGE-POLICY.md>). Use one owned isolated home for initial/restored; corrupt only its PUBLIC preference fixture before the corrupt stage. These stages never issue account, OAuth or model commands; omission preserves the existing lifecycle checks. `--runtime-stage initial|restored` instead qualifies typed runtime save/stale-CAS/restart/reset on an owned PUBLIC configured route; it cannot be combined with a usage stage.

Fixtures and scripted backend checks do not establish real-key validity, physical input or full app parity. Generated evidence stays local.
