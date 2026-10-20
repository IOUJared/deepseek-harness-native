# Python to Rust tooling port

The repository's development tooling was Python. It is moving to the `tools/xtask`
crate so a checkout with Rust can run every predicate without an interpreter. This
file is the stage map; it is deleted with the last Python file.

## Rules

- **Parity before deletion.** A family is deleted only after its Rust replacement
  passes the same gates, and the plans/outputs are compared with the Python
  original while both still exist.
- **One commit per family.** Never mix a port with a feature or parity round.
- **Historical records stay.** `CHANGELOG.md`, `RELEASES/` and the round logs in
  `docs/NATIVE-UI-PARITY.md` record what was run at the time; they are not rewritten.
- **Do not run Cargo while another writer builds** (`AGENTS.md`). The parity loop
  also commits in this worktree, so stage additively and commit explicit paths.

## Landed

| Item | Was | Now |
|---|---|---|
| Verification runner | `scripts/verify-native.py` | `xtask verify` (`--suite`, `--plan`, `--online`, `--cargo-home`, `--timeout`) |
| Formatting ratchet | `scripts/format-debt.py` | `xtask format-debt` (`--update`, `--baseline`, `--root`) |
| Benchmark runner | `benchmarks/run.py` (CLI) | `xtask bench run` (same `DSH_BENCH:` protocol, bounded child, owned tree sampling) |
| Benchmark analyzer | `benchmarks/analyze.py` | `xtask bench analyze RESULTS [--markdown]` |
| Idle sampler | `scripts/measure-native-idle.py` | `xtask idle` (same warm-window identity rules) |
| Archive verifier | `scripts/verify-development-archive.py` | `xtask archive verify` (same member bounds, extraction modes and inventory checks) |
| History tools | `app/measure_native_history.py`, `app/compare_native_history.py` | `xtask history measure` / `xtask history compare` |
| Dialog/Codex fixtures | `scripts/qualify-dialog-components.py`, `scripts/qualify-codex-components.py` | `xtask qualify dialog` / `xtask qualify codex`, on a shared `qualify` module (`prepare`, isolated environment, owned-component run) |
| Detail captures | `app/qualify_formatted_details.py` | `xtask qualify details` |
| Scroll-app probe | `app/qualify_scroll_app.py` | `xtask qualify scroll` |
| Menu panes | `app/qualify_model_submenus.py` | `xtask qualify submenus` |
| Control layout | `app/qualify_control_layout.py` | `xtask qualify control` (32 public captures + one real-keyless run) |
| Slint scale | `benchmarks/slint/qualify-scale.py` | `xtask bench scale` (both renderers, sequential) |
| Composed settings | `scripts/qualify-composed-settings.py` | `xtask qualify composed-settings`: pidfd root pinning, parent-chain fencing, bounded TERM/KILL cleanup. The Python file **stays** until its qualification dependents are ported |
| Lazy account | `scripts/qualify-lazy-account.py` + guard | `xtask qualify lazy-account`, on a shared `run_owned` (same pin/fence/cleanup rules, private report, source digests, exact lifecycle validator); guard mutation cases ported before deletion |
| Agent discovery | `scripts/qualify-agent-discovery.py` (+ its guard) | `xtask qualify agent-discovery [--sdk]` |
| Client projections | `scripts/qualify-client-projections.py` | `xtask qualify client-projections` |
| Turn outline | `scripts/qualify-turn-outline.py` | `xtask qualify turn-outline` |
| Upload uncertainty | `scripts/qualify-upload-uncertainty.py` + guard | `xtask qualify upload-uncertainty [--fail-at]`: exact fact/count/bridge/cleanup sets, expected-failure containment, fixture order/digest, and the symlink-above-store rejection; guard cases ported before deletion |
| Composed decisions | `scripts/qualify-composed-decisions.py` + guard | `xtask qualify composed-decisions [--viewport]`, on a shared `guard` seam extracted from composed-settings: pin/fence/cleanup plus the four Host phase receipts, branch codes and private captures |
| Skill loading | `scripts/qualify-impeccable-skill.py` | `xtask qualify impeccable-skill`: three scopes, full catalog/body/tool load, instruction digests taken from the pinned skill files |
| Composed files | `scripts/qualify-composed-files.py` + guard | `xtask qualify composed-files`: ticket/request/phase chain, raw Host receipts, private PNG captures (headers/dimensions only) and the storage oracle; guard cases ported before deletion |
| Native file prompt | `scripts/qualify-native-file-prompt.py` + guard | `xtask qualify native-file-prompt`: three prompt ACKs, durable inbox refs and claimed pre-step rejections, every generation counter zero |
| Native file stage | `scripts/qualify-native-file-stage.py` + guard | `xtask qualify native-file-stage`: the worker runs through `run_owned`; stored fixtures re-verified and the refusal fixture must not be published |
| File upload | `scripts/qualify-file-upload.py` + guard | `xtask qualify file-upload`: shared ownership runner plus independent private-store check (0400 canonical object and hardlinked alias, same inode, exact bytes) |
| App qualifier (root) | `scripts/qualify-native-app.py` | `xtask qualify app`: owned-tree sampling, per-thread CPU, pidfd identity guard, SIGINT/SIGTERM cleanup, compositor mapping proxy, keyless-idle medians. The Python file **stays** until its qualification dependents are ported |
| Their guards | 4 Python test modules | 65 tests in `tools/xtask` (`src/*` units + `tests/cli.rs`) |
| Entry point | `check.sh` → `python3` | `check.sh` → `cargo run -- verify` |

`benchmarks/run.py` survives as a **library**: `scripts/qualify-native-app.py`
imports its process-metrics helpers, so the module cannot be deleted until the
qualification family is ported. Its CLI duplicate is `xtask bench run`; do not
change `run.py`'s measurement rules in the meantime, or the two drift. Its guard
(`benchmarks/test_runner.py`) was deleted once its cases were covered by the Rust
`proc` and `bench` tests; the `benchmarks` guard check is retired.

Deliberate differences, all measured against the Python originals:

- `format-debt` is an in-process check (`internal format-debt`) instead of a
  `python3` child, exactly as `binary-freshness` already was.
- `tools/xtask` is included in the clippy and format checks, so the new crate is
  held to the repository's own rules.
- The guards run as `cargo test` (`xtask-guards`) rather than `python3 -m unittest`.
  `benchmark-guards` stays only for `run.py`'s library guard (see above).
- Report keys, per-file source hashes, the aggregate snapshot digest, the analyzed
  JSON and the markdown table are unchanged, so evidence before and after the port
  is comparable. `bench analyze` was diffed against `benchmarks/analyze.py` on the
  same report: no field differed and the markdown was byte-identical.
- `numbers` are read from a *proc root* instead of a hardcoded `/proc`, so the
  measurement rules are tested against a fixture tree rather than the live table.
  Production passes `/proc`.
- The tar crate keeps the trailing `/` that Python's `tarfile` strips from a
  directory entry, so `archive verify` normalizes directory names before
  validating; both tools then report identical fields for the same archive, and
  the extracted trees compared equal apart from the path each wrote into its own
  report.
- `history compare` was diffed against `app/compare_native_history.py` on the same
  two fixture runs: the resulting JSON was identical, including the relative
  qualification paths and the 1.5 observed speed ratio. Unlike the Python tool it
  rejects a native report that lacks `fixtureMetrics` instead of crashing on it.
- `app/` no longer has a Python guard, so the `app-guards` discovery is gone from
  the test suite; the history guards are `xtask history` tests.
- The compositor window probe moved out of the benchmark runner into
  `compositor.rs` so the component qualifiers share one implementation instead of
  growing a second.
- The draft-restart qualifier is fully ported (`drafts`): the validators,
  the hardened private reader (`private`: single-link, owner-only, O_NOFOLLOW,
  duplicate JSON keys refused at any depth), and the two-phase orchestration with a
  frozen artifact, source digests and a 170 s deadline. `draft-interruption` imports
  it, so `draft-interruption` (and its guard) is deleted while `draft-restart`
  stays: `qualify-private-native-seat.py` still imports the latter as a library.
- The composed-registry **stage contract** is ported (`registry`): six planned
  transitions with their target slot and pin order, the per-stage follow-up
  evidence, a strictly increasing ticket serial, two distinct retained sessions,
  and the owned stop. The PNG writer-profile validator is ported too (chunk order,
  CRCs, one bounded zlib stream, exact row lengths and filter tags, reported-size
  agreement). The owned run is ported too: the pinned probe, the private app
  report, both captures and the artifact digest, with `class`/`xwayland` matched to
  the fixture. `qualify-composed-export.py` and the package qualifier
  still import the Python module, so it is deleted with them.
- The composed-export qualifier is fully ported and deleted (`export`): two exact
  save receipts, both private PNG captures, an independent ZIP inspector (central
  directory and local header bounds, bounded inflate, CRC, UTF-8 object JSONL and
  the ordinary v4 header profile), the owned run and the streamed fixture digest.
- The draft-interruption qualifier is fully ported (`interruption`): nine
  checkpoints with the document expected at each, hold/recover/crash shapes, the
  latch-until-owned-kill crash gate, and the rule that a graceful phase may not
  borrow the crash phase's external cleanup. Its campaign orchestration runs the
  hold case, every crash/recover pair, and re-checks sources and the artifact.
- The `qualify` module now owns `prepare` and the isolated child environment, and
  `xtask qualify app` can run the root workflow, but `scripts/qualify-native-app.py`
  still exists for its 18 dependents and keeps its own copies of those helpers.
  That duplication is transitional and grows the risk of the two drifting: do not
  change one side's measurement rules without the other until the family moves.
- The app run is tested end to end against a stand-in binary (spawn, sample, pidfd
  guard, cleanup, private report); no real keyless app qualification was run, which
  needs the built Host runtime and a Wayland session.
- The composed-settings containment rules are ported as written: pin the root before
  any poll, fence every parent back to it, and signal only pinned identities with
  bounded TERM/KILL budgets. The cleanup path is tested by pinning a live `sleep`
  child and reaping it; the composed verdict is tested against synthetic cleanup and
  window facts. No real composed run was made (needs the fixture binary and a
  Wayland session).
- The benchmark runner is tested end to end against a stand-in script (protocol
  parsing, sampling, a killed overrunning child), except `benchmarks/run.py` was
  not compared against `xtask bench run` on a real app: that needs a GPU/Wayland
  session the port could not claim. The Python library stays until its last
  consumer is ported, so the two runners are a named, temporary duplication.


| Release packaging | `.github/scripts/package_release.py`, `publish_release.py` + guards | Deleted: `xtask release package` assembles the assets and `xtask release publish` runs the draft→verify→publish flow; the CI workflow calls them and the packager link now points at `release.rs` |


| Launcher and package | `development-launcher.py`, `build-development-package.py`, `qualify-development-package.py`, `qualify-explicit-node.py` + guards | Deleted: `xtask launch`, `xtask release package`, `xtask qualify development-package` and `xtask qualify explicit-node` replace them; the archive ships `launch` |

## Remaining

| Family | Files | Lines | Target |
|---|---|---|---|
| `scripts/qualify-*` | 7 | 1,885 | `xtask qualify <name>`; the assertions are already Rust examples writing `result.json` |
| `scripts/test_*` | 6 | 1,294 | Rust tests next to the qualifiers; rewrite each guard's subject, never copy it verbatim. `app/` has no Python left |
| `benchmarks/run.py` | 1 | 330 | Deleted with the qualification family, whose scripts import the library; `xtask bench run` is already the replacement and `test_runner.py` is already deleted |

Also outstanding, outside `*.py`: `core/src/process_tests.rs` spawns `/usr/bin/python3`
as a forked fake-process tree for supervisor containment. It needs a small Rust
fixture binary (or a `sh`/`setsid` chain) before `python3` can leave CI.

## Out of scope

The sibling `deepseek-harness-linux` repository keeps its Python: `python/sdk`,
`python/sdk-runtime` and the office/skill scripts are shipped product, not tooling.
