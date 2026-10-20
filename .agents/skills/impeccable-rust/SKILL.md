---
name: impeccable-rust
description: >-
  Use for writing, reviewing, hardening or optimizing Rust, and for native
  Harness frontend/Core/transport or cross-language backend verification.
  Assign failure modes to production verifiers, test error paths, preserve
  benchmark oracles and report evidence without overstating its bounds.
---

# Impeccable Rust for Harness

Adapted from hexuria/impeccable-rust (MIT). The exact upstream guide, license and tool sources are retained; `UPSTREAM.json` records their pinned revision and hashes. This short entry avoids Harness tool-result pruning. Load details only for the affected risk.

## Before changing code

1. Read project instructions, manifests, existing tests and CI. Identify the affected production owner and an invariant that would expose a regression.
2. Make invalid use hard to express: closed states, distinct identifiers, bounded inputs and explicit ownership. Do not invent a second model or duplicate an existing verifier.
3. Test failure, cancellation, stale callbacks, capacity limits and teardown where applicable—not only successful calls. Preserve the reference implementation during rewrites; compare outputs before deleting it.
4. Treat compilation as a prerequisite, not behavioral evidence. Record intentional gaps and the next useful verification owner.

## Select verification by risk

| Risk | Starting owner | Escalate only for a named gap |
|---|---|---|
| Deterministic frontend/domain logic | Production reducer/unit tests; generated or differential inputs | Property testing |
| Wire/file/model input | Exact parser tests, malformed and boundary cases | Fuzzing |
| Unsafe memory/FFI | Documented safety invariant and error-path tests | Scoped Miri, cargo-careful or sanitizer |
| Small concurrent algorithm | Production cancellation/ownership tests | Loom with a conformance link |
| Multi-actor recovery | Fault injection and real composition tests | TLA+/Stateright plus production trace checks |
| Persistence | Interrupted writes, permissions, old records, ACK uncertainty | Crash campaign |
| Public API/dependencies | Consumer tests, lockfile review and compatibility record | Semver checks, RUSTSEC/deny/vet |
| Performance | Frozen workload and correctness oracle | Controlled repeated release measurements |

Do not call tests, fuzzing or bounded model checking a proof. Name inputs, schedules, bounds, assumptions and blind spots. A second semantic model needs an executed link to the production implementation.

## Native frontend and backend

For the native repository, read `docs/VERIFICATION.md` from its root. Its `tools/xtask` crate provides a bounded plan, doctor, lints and headless regression suites. Run the doctor before selecting additional tools. Serialize Cargo with two jobs, locked dependencies and the available cache; do not run another agent's build concurrently.

For the TypeScript Harness backend, read its `AGENTS.md` and `docs/testing.md`. Apply the same risk/evidence discipline using its existing typecheck, focused Vitest, real Loader and replay checks—not Rust tools or Rust API idioms. Preserve typed same-process trust; validate actual parser, persistence, process and wire input. Never substitute RC API inspection for alpha source/artifact evidence.

Keep the installed RC runtime, isolated alpha profile, credentials and released sessions intact. Skill loading does not authorize a model request, OAuth action, migration or runtime replacement. Unit routing, keyless composition, native paint, physical input and live provider execution are separate claims.

## Detailed resources

Resolve these paths against this skill directory:

- `references/upstream.md`: complete upstream checklist, API design, unsafe/concurrency verification, dependencies and evidence terminology. Read relevant headings in bounded chunks.
- `performance.md`: baseline → one hypothesis → oracle → repeated measurements → keep/revert → convergence. Freeze workloads, compiler/features, profile and shipped flags; record memory, process-tree CPU and latency, not a single wall-clock anecdote.
- `scripts/impeccable`: reviewed upstream toolbox entry. `bash scripts/impeccable doctor` inspects availability; it does not install tools. Toolbox capability is not a completed check.
- `toolbox/tools.txt` and `toolbox/Dockerfile`: pinned tool versions and image recipe, not an attestation of installed tools or reproducible OS packages.

Host installation, Docker/image builds and sanitizer/formal campaigns require explicit user agreement on scope and cost. Do not enable `IMPECCABLE_UNCONFINED`, forward credentials into a toolbox or run unreviewed setup by default. Before using upstream `bench-guard`, inspect its cleanup and verify the resolved lock path is a disposable intended target; never point `IMPECCABLE_BENCH_LOCK` at a project or existing data directory.

## Finish with evidence

Report briefly: affected risk and production verifier; exact checks actually run and outcomes; features/toolchain/bounds; compatibility/dependency impact; deferred checks and blind spots. Retain failures and unstable-source observations. Do not rerun until green and conceal the failed attempt. No speed, security, parity or formal-verification claim without its own evidence.
