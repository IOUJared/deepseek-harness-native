# Native crash record

The macOS shell writes `<logs>/crash-<iso>-<source>.log` when the Host fails to
start or a renderer crashes, caps the inspected error it embeds at **256 KiB** and
keeps the newest **ten** reports (`main.js:7647-7742, 10989-11007`). Native had no
durable artifact: a failed start set an in-window status and printed nothing, so a
launch from a desktop entry left no trace to inspect.

`app/src/crash_log.rs` writes one report per fatal backend fault.

## What is written

| Part | Value |
|---|---|
| Path | `<--home>/logs/crash-<YYYYMMDDTHHMMSS-mmm>-<source>.log` |
| Directory / file mode | `0700` / `0600`, created with `create_new` so an existing path (or a planted symlink) is refused |
| Header | app version, the same local stamp, `OS/ARCH`, `source`, `summary` |
| Detail | the fault text, cut on a character boundary at 256 KiB with a `[truncated]` marker |
| Retention | newest ten `crash-*.log` survive; older ones are removed |

The record carries only what the failure already shows in-window plus the
environment around it. It never reads credentials, drafts, the Host's stream or
the user's clipboard, and a failure to write a report never replaces the failure
being reported.

## Where it is written from

- `Event::Fault` in the App (the production path): `worker::run` publishes
  `Event::Fault("Core start: <cause>")` and then
  `Event::Stopped(Err("Core did not start; inspect startup error"))` when Core
  cannot start (`app/src/worker.rs:1748-1758`). The report takes the fault text,
  which is the cause; the in-window status keeps the reference's own line.
  `Event::NoBackendStarted` is the already-closing branch and carries no cause.
- `worker::start` in `main.rs`: the pre-window branch, when the backend owner
  thread itself cannot be created.

Smoke runs and keyless runs hold no `native_home` and therefore write nothing.

## Verification

- `app/src/crash_log.rs` unit tests: the private bounded record, the newest-ten
  retention with a path-shaped source label sanitized away, and that an existing
  report is never overwritten.
- `ui::tests::failed_backend_start_leaves_a_bounded_crash_record`: drives the
  production event pair (`Fault` then `Stopped(Err(..))`) against a temporary
  home and asserts one record carrying the cause, with the status unchanged.
- Real failure: a run with `--node /nonexistent/node` wrote
  `logs/crash-20261016T132003-887-host-start.log` (199 bytes, mode `0600` in a
  `0700` directory) summarizing `Core start: InvalidOptions`, within one second of
  launch.

Blind spots: no renderer-crash path exists on Linux (Iced has no renderer process
to crash), the report does not include a Host stderr tail because the spawn keeps
`stderr(Stdio::null())`, and no crash campaign has exercised retention under
concurrent starts.
