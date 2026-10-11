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

- `format-debt` owns formatting. It runs the scoped `cargo fmt --manifest-path <crate>/Cargo.toml -- --check` for Core, transport and App, deliberately without `--all`, so the pinned third-party `app/vendor/winit` tree is never reported as ours; that upstream tree accounted for 1,139 of the former failures.
- `AGENTS.md` forbids rewriting all shared Rust files to resolve local formatting debt, so the remaining first-party debt is recorded per file in `scripts/format-debt-baseline.json` rather than waived. The check prints current file and hunk totals and **fails when a file that was clean becomes unformatted, or a new file is added unformatted**. Removing debt is reported, not failed; record an intentional change with `python3 -B scripts/format-debt.py --update`. Baseline at 2026-10-10: 46 files, 3,229 hunks — hand-written style such as `fn f(&self)->bool`, which no rustfmt option accepts.
- Clippy denies unfinished macros and implicit unsafe operations; other warnings stay visible by policy, and a guard asserts the suite does not deny all baseline warnings. Visible totals measured 2026-10-10: App 211, Core 17, transport 6 — mostly collapsible `if` statements, missing `else`, and large enum variants.
- Reducing either debt takes one dedicated, reviewed commit per change; do not mix it with feature work.

Reports contain exact commands, outcomes, toolchain details and before/after source hashes. A source change during a run invalidates the combined evidence. This detects drift; it is not an attestation of the complete compiler dependency closure. Output is retained beneath ignored `app/evidence/`; no source or user profile is rewritten.

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
