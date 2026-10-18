# Development verification

Reference for native frontend and backend changes. [Impeccable Rust](../.agents/skills/impeccable-rust/SKILL.md) supplies the reusable method; [upstream provenance](../.agents/skills/impeccable-rust/UPSTREAM.json) pins the retained MIT sources.

## Commands

Run from the native repository. The runner prints plans without running checks, executes serially with two Cargo jobs, defaults to locked/offline Cargo, never installs tools and records private evidence. An explicit `--online` permits Cargo dependency access; it does not install verification tools.

```sh
scripts/check.sh
python3 -B scripts/verify-native.py --suite test --plan
python3 -B scripts/verify-native.py --suite doctor
python3 -B scripts/verify-native.py --suite lint
python3 -B scripts/verify-native.py --suite clippy
python3 -B scripts/verify-native.py --suite examples
python3 -B scripts/verify-native.py --suite test
```

`scripts/check.sh` is the single entry point: with no arguments it runs `--suite all`, and any other argument is forwarded unchanged (`scripts/check.sh --plan`). Use `--cargo-home ABS_PATH` for an existing cache.

## Formatting and lint debt

- `format-debt` owns formatting. It runs the scoped `cargo fmt --manifest-path <crate>/Cargo.toml -- --check` for Core, transport and App, deliberately without `--all`, so the pinned third-party `app/vendor/winit` tree is never reported as ours. Pass a path list to keep a change scoped: `cargo fmt -- <paths>` formats the whole crate regardless, which is how the App crate became rustfmt-clean in one pass.
- `AGENTS.md` forbids rewriting all shared Rust files to resolve local formatting debt, so the remaining first-party debt is recorded per file in `scripts/format-debt-baseline.json` rather than waived. The check prints current file and hunk totals and **fails when a file that was clean becomes unformatted, or a new file is added unformatted**. Removing debt is reported, not failed; record an intentional change with `python3 -B scripts/format-debt.py --update`. Baseline at 2026-10-11: **0 files, 0 hunks** — every first-party file is rustfmt-clean, so any new debt is now a genuine regression rather than a recorded allowance.
- Clippy denies unfinished macros and implicit unsafe operations; other warnings stay visible by policy, and a guard asserts the suite does not deny all baseline warnings.
- Visible totals measured 2026-10-11 for the **production** targets (`cargo clippy -- -D warnings`): App **0**, Core **0**, transport **0** — down from App 32, Core 1, transport 0. This is the plain target set, not `--all-targets --all-features`; the wider set still reports findings in test and fixture code and is not a gate this suite runs. The reduction came from the machine-applicable `collapsible_if` subset (Rust 2024 let-chains, no behaviour change), documented `#[allow(dead_code)]` on retained API, boxing the four enum variants whose payload dwarfed their peers, and a function-scoped allow for `select!` branches. Under `--all-targets --all-features` the same findings multiply per target (App 1,133 summary lines); compare like with like.
- **Every `app/examples` target that includes production sources must declare the same shared module set** (`crash_log`, `goal_ui`, `keybindings`, `queue_dock`, `queue_update`, `single_instance`). `--suite clippy` compiles `--all-targets --all-features`, so a stale include list fails that gate even while `--suite test` is green. Eleven composed smoke examples and the geometry probe had drifted; they are synchronized as of 2026-10-17. The geometry probe includes `ui.rs` inside a wrapper so `chat_geometry`'s inline helpers keep the same module ancestry as the App instead of needing private UI modules made `pub(crate)`.
- **Drift note (2026-10-17).** The historical "App 0" measurement of 2026-10-11 no longer describes the current tree: the App bin reports 20 rustc dead-code warnings and **36** clippy findings under `-D warnings` (clippy 1.99.0), and `--all-targets --all-features` reports more in test and fixture code. The suite's contract is unchanged — it denies only `todo`, `unimplemented` and `unsafe_op_in_unsafe_fn` — so those findings stay visible rather than failing or being waived. Do not read the 0 as current.
- The four fields/variants boxed on 2026-10-11 are `terminal::CreateOutcome::Created`, `plugins::Effect::Save::namespace`, `worker::Command::PluginSave::namespace` and the two value sets of `codex_runtime::Request::Save` / `codex_worker::RuntimeOperation::Save`. Boxing dropped `Copy` from those enums; that is intended, not incidental.
- Two `manual_clamp` sites keep the `max().min()` form under a documented allow: `f32::clamp` propagates NaN where these answer a finite 1.0, and a NaN width would reach the layout as a degenerate measurement.
- Three findings are answered by a scoped allow rather than a rewrite, each stating why at the site: `clippy::result_large_err` on `worker::Handle::send`, whose error type is [`Command`] because returning the unsent value is the `mpsc::Sender::send` idiom; `clippy::possible_missing_else` on `worker::run`, whose dense branches are rustfmt's own output inside `select!`; and the two `manual_clamp` sites noted above.
- One `too_many_arguments` site remains grouped rather than allowed: `cloudflare::CloudflareSaveChoice` carries the three save switches, which a caller could previously transpose positionally. Grouping those booleans changed a core public signature, so the app's call site and the core example were updated together. The other such site is gone: `worker::Admission` is now one shared gate built from the `Flights` table, so management, rename, fork, export, terminal and MCP admission are one 3-argument question instead of a per-command field list.
- Reducing either debt takes one dedicated, reviewed commit per change; do not mix it with feature work. A `-D warnings` gate is **not** this suite's contract: it would require clearing the remaining inventory above, so it needs its own agreed campaign rather than an incidental flip. Note that `cargo clippy --fix` rewrites files across the whole crate and leaves them unformatted, so follow it with `cargo fmt --manifest-path <crate>/Cargo.toml` and re-run the tests rather than trusting the rewrite.

Reports contain exact commands, outcomes, toolchain details and before/after source hashes. A source change during a run invalidates the combined evidence. This detects drift; it is not an attestation of the complete compiler dependency closure.

`--suite test` and `--suite all` run one in-process check first, `binary-freshness`: every **present** release artifact (`app/target/release/dsh-native-app`, `app/target/release/examples/layout_smoke`) must be at least as new as the first-party sources that feed it. A stale artifact fails the suite, because a green report describing code you cannot launch is the failure this check exists to prevent; an artifact that was never built is not a failure, so a test-only checkout stays green. The log names the stale artifacts and the rebuild commands. It compares modification times against the worktree, not `HEAD`, so a newer uncommitted edit — including another writer's — re-triggers it, which is intended. Output is retained beneath ignored `app/evidence/`; no source or user profile is rewritten.

## Failure owners

| Production owner | Affected failures | Headless verification |
|---|---|---|
| Iced App/reducers | Stale session/epoch actions, duplicate clicks, consent, ACK ownership, transcript projection | App tests with default and PUBLIC fixture features |
| Pinned winit Wayland drop owner | Local URI validation, byte/dispatch/time limits, COPY/drop ordering, stale transfer/window/seat publication | `wayland-drops`: production data-device module tests; not physical file-manager input |
| Layout/scroll examples | Observer/rectangle bounds, invalid CLI options, cycle sequencing | Existing example test harnesses; compilation alone is insufficient |
| Opt-in real Wayland drop | Compositor event ordering and native drop → local path review/PNG preview | Existing `layout_smoke` App fixture with explicit `--drop-file`; private nested compositor/source/pointer runner below; not hardware mouse/IME or provider execution |
| Core | Pipe/framing limits, owned descendants, shutdown, secret handling, persistence uncertainty | Core tests with and without transport |
| HTTP/WebSocket transport | Authentication/privacy, malformed JSON, correlation, byte/item/depth limits, cancellation, no blind mutation retry | Transport tests against the fixed fake-wire fixture |
| JavaScript Host bridge | Command/framing validation, diagnostic privacy, account/Codex disposal and uncertain writes | Production-module Node tests |
| Packaging/qualification | Evidence guardrails and archive/launcher behavior | Existing Python guard tests |

The fake-wire suite needs the existing `transport/tests` dependencies and `/usr/bin/node`; CI prepares them. The runner does not replace Node or install npm dependencies. Existing tests own their isolated fixtures and cleanup.

## Resolved codex trace-read race

`codex-app-server.test.mjs` counted the fake child's request log (`PUBLIC-requests.jsonl`) right
after the handshake. A frame reaches the *child's stdin* when `writeFrame` resolves, but the child
logs it only when its reader runs, so the `initialized` notification could still be unlogged on a
loaded machine: the count read 1 where the test expected 2. Reproduced **10/10** with eight busy
loops (`yes > /dev/null` ×8) and 0/10 idle; with the probe printing the trace, the missing row was
`initialized` (`AssertionError: ["initialize"]`).

`settledRequests(home, expected)` now waits, at most 2 s in 5 ms steps, for the trace to reach the
count the following assertion requires. The assertion is unchanged and exact — a frame that never
arrives still fails — so this removes a read-before-drain race rather than weakening the check.
Measured after the change: 0/10 failures under the same eight-way load.

**Not fixed, and not a test defect:** under that same load the `initialize wrong-platform` case
once reported `STARTUP_TIMEOUT` instead of `INITIALIZE_MISMATCH` (1 failure in 3 full-file runs).
The product's startup deadline fired before validation under CPU starvation. Relaxing it would
weaken a real containment bound for a synthetic condition, so it stays as measured: the file is
55/55 normally, and one of three runs under eight busy loops took the timeout branch.

## Resolved draft-lock test flake

`ui::draft_store::sidebar_storage_tests` used to fail intermittently with `Failure::Busy` in a different test each run, each passing alone (measured 2026-10-10: 7 failures in 1,070 module runs, ~0.7%; one run in three was reported earlier under heavier load). Every failure landed immediately after a completed `Store::save`/`save_checked` (`absent_and_present_...:153`, `checked_save_retains_...:285`, `semantic_cas_...:170`).

Root cause: an `flock` lock belongs to the open file description, so closing the acquiring descriptor releases it only once every duplicate is closed. `cross_process_sidebar_writer_invalidates_loaded_expected_snapshot` forks this multithreaded test process via `std::process::Command`; the child inherits copies of every lock descriptor open in other threads and holds them until `O_CLOEXEC` closes them at `execve`. During that fork→`execve` window a finished parent operation still looks locked, so the next cooperating `load`/`save` on the same fixture returns `Failure::Busy`. Capture showed `EWOULDBLOCK`, no in-process descriptor for the lock inode except the failing call's own fresh descriptor, and an already-empty `/proc/locks` — a transient out-of-process duplicate, not a retained in-process `Lock`.

Fix: `Lock` implements `Drop` and issues an explicit `flock(LOCK_UN)` in `app/src/draft_store.rs`, ending the critical section with the operation instead of the last inherited descriptor. `Failure::Busy` semantics are unchanged for genuinely live locks. `completed_operation_releases_exclusive_lock_while_a_duplicate_descriptor_remains` reproduces the stranded lock deterministically with `File::try_clone` (the `dup`/`fork` shared open file description): 3/3 failed before, 3/3 pass after. The fixture paths remain unique per test and the thread pin in `app/.cargo/config.toml` was never the mitigation; never weaken these assertions to hide a `Busy` failure.

## Real native drop (opt-in)

Named gap: synthetic App messages and the existing headless Gamescope seat cannot establish actual `wl_data_device` transfer. [qualify-native-drop.py](<../scripts/qualify-native-drop.py>) reuses existing pidfd cleanup owners and `layout_smoke` with an explicit new PUBLIC file. It opens one private nested Hyprland window, disables libseat/systemd integration, isolates HOME/D-Bus/config/runtime, and injects software pointer events **only** into its verified inner socket. It does not reload desktop settings or start a Host.

Compile [native-drop-source.c](<../scripts/native-drop-source.c>) with GTK3; generate the supplied [MIT protocol](<../scripts/protocols/wlr-virtual-pointer-unstable-v1.xml>) with `wayland-scanner` and compile [native-drop-pointer.c](<../scripts/native-drop-pointer.c>) with `wayland-client`. Use a compact release `layout_smoke` snapshot; the frozen-executable guard limits it to 128 MiB.

```sh
python3 -B scripts/qualify-native-drop.py --fixture ABS_RELEASE_LAYOUT_FIXTURE --source ABS_COMPILED_GTK_SOURCE --pointer ABS_COMPILED_POINTER --mode single
# --mode multiple: atomic URI-list rejection; --kind thunar: real private file manager
```

Requires already-installed Hyprland/SDK tools and an authorized outer Wayland window connection; never auto-installs. Reports distinguish actual transfer, native review/PNG paint, source preservation and owned-process cleanup from physical mouse/IME and charged-provider claims. Retain failed attempts; Thunar MOVE negotiation on this compositor is an explicit compatibility failure, not a passing COPY test. [Attachment scope](<NATIVE-ATTACHMENT-UX.md>).

## TypeScript backend

The sibling backend's `AGENTS.md`, `docs/testing.md` and package scripts remain authoritative. Use focused package Vitest tests for changed behavior; typecheck/lint for typed consumers; real Loader tests for composition/disposal; replay snapshots for changed model/user output. Test the built artifact when changing published launch paths. Preserve released records and persistence acknowledgements. Keep source-plane tests distinct from built-artifact qualification and installed RC APIs distinct from alpha APIs.

Do not default to the full backend suite. Do not apply Rust-specific tools to TypeScript or add hostile-input validation at statically trusted same-process interfaces. Cross-language changes update both serializers/consumers and test representative valid, invalid and boundary messages through production implementations.

## Skill availability

The repository skill works for sessions rooted here. Local shared-user and workspace-root aliases also expose it to the current RC Harness and native alpha's default working directory; no plugin/profile changes are required. Other machines can copy this complete skill directory to their own `~/.agents/skills/impeccable-rust`.

The standard and PTC presets discover filesystem skills in their agent scope. A host-only skill list or a raw Agent without its preset is not evidence of agent visibility; minimal presets deliberately omit the loader. The local qualification runs the real alpha profile and standard preset in private state, checking complete catalog/body/tool loading for native, default workspace and backend working directories:

```sh
python3 -B scripts/qualify-impeccable-skill.py --output app/evidence/impeccable-skill-new
```

Use a new output directory and the installed local aliases; the sibling built alpha backend is required. No prompts are sent. The runner pins and reaps owned processes, verifies unchanged source and rejects incomplete loading or unexpected agent work. Shell body/resource edits are observed by filesystem loading; a catalog refresh can wait for the watcher.

## Additional tools and claims

Run the pinned upstream doctor before selecting an expensive tool. Missing tools remain explicitly unrun. No Docker, nightly, host installation, network fuzzing or live provider request is part of the default checks.

Use Miri/sanitizers only on supported memory/FFI scopes; whole Iced/Linux/Node execution is not a Miri claim. Add Loom for a small concurrent implementation, or a system model for recovery, only with a named gap and production conformance checks. Fuzz untrusted inputs when its campaign and oracle are maintained. Dependency advisory/audit and CI security tools require their own results; pinning sources alone does not certify them safe.

Keep benchmarks on a frozen release workload, reference oracle, toolchain/features and shipped flags; compare repeated distributions, process-tree memory and CPU. Do not change the benchmark contract to obtain a win. [Performance method](../.agents/skills/impeccable-rust/performance.md).

Evidence distinguishes planned, passed, failed, timed out, unsupported and deferred checks. Unit routing, keyless profile composition, real native paint, physical pointer/IME, OAuth and charged provider execution are separate. Green checks do not prove all inputs, all schedules, kernel correctness or complete desktop parity.
