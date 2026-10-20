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
| Their guards | 4 Python test modules | 65 tests in `tools/xtask` (`src/*` units + `tests/cli.rs`) |
| Entry point | `check.sh` → `python3` | `check.sh` → `cargo run -- verify` |

`benchmarks/run.py` survives as a **library**: `scripts/qualify-native-app.py`
imports its process-metrics helpers, so the module cannot be deleted until the
qualification family is ported. Its CLI duplicate is `xtask bench run`; do not
change `run.py`'s measurement rules in the meantime, or the two drift. Its guard
(`benchmarks/test_runner.py`) therefore stays in `benchmark-guards`.

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
- The benchmark runner is tested end to end against a stand-in script (protocol
  parsing, sampling, a killed overrunning child), except `benchmarks/run.py` was
  not compared against `xtask bench run` on a real app: that needs a GPU/Wayland
  session the port could not claim. The Python library stays until its last
  consumer is ported, so the two runners are a named, temporary duplication.

## Remaining

| Family | Files | Lines | Target |
|---|---|---|---|
| `scripts/qualify-*` | 24 | 3,947 | `xtask qualify <name>`; the assertions are already Rust examples writing `result.json` |
| `scripts/test_*`, `app/*.py` | 29 | 4,141 | Rust tests next to the qualifiers; rewrite each guard's subject, never copy it verbatim |
| `.github/scripts/*`, `verify-development-archive.py` | 5 | 543 | `xtask package` / `xtask publish`; needs `tar` + `flate2`, `gh` stays a child process |
| `development-launcher.py` | 1 | 356 | **Shipped** as `launch.py` in the development package: a Rust `dsh-native-launch` binary replaces it, so the archive stops requiring Python 3.11+; update the archive verifier and install docs together |
| `benchmarks/run.py` + `test_runner.py` | 2 | 417 | Deleted with the qualification family, whose scripts import the library; `xtask bench run` is already the replacement |
| `benchmarks/slint/qualify-scale.py` | 1 | 116 | `xtask bench qualify-scale`; needs the standalone Slint prototype binary, so it was left until the main benchmark runner settled |

Also outstanding, outside `*.py`: `core/src/process_tests.rs` spawns `/usr/bin/python3`
as a forked fake-process tree for supervisor containment. It needs a small Rust
fixture binary (or a `sh`/`setsid` chain) before `python3` can leave CI.

## Out of scope

The sibling `deepseek-harness-linux` repository keeps its Python: `python/sdk`,
`python/sdk-runtime` and the office/skill scripts are shipped product, not tooling.
