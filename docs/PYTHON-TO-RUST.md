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
| Verification runner | `scripts/verify-native.py` | `tools/xtask` `verify` (`--suite`, `--plan`, `--online`, `--cargo-home`, `--timeout`) |
| Formatting ratchet | `scripts/format-debt.py` | `tools/xtask` `format-debt` (`--update`, `--baseline`, `--root`) |
| Their guards | `scripts/test_verify_native.py`, `scripts/test_format_debt.py` | 35 tests in `tools/xtask` (`src/*` units + `tests/cli.rs`) |
| Entry point | `check.sh` → `python3` | `check.sh` → `cargo run -- verify` |

Deliberate differences, all measured against the Python plans:

- `format-debt` is an in-process check (`internal format-debt`) instead of a
  `python3` child, exactly as `binary-freshness` already was.
- `tools/xtask` is included in the clippy and format checks, so the new crate is
  held to the repository's own rules.
- The runner's guards run as `cargo test` (`xtask-guards`) rather than
  `python3 -m unittest`.
- Report keys, per-file source hashes and the aggregate snapshot digest are
  unchanged, so evidence before and after the port is comparable.

## Remaining

| Family | Files | Lines | Target |
|---|---|---|---|
| `scripts/qualify-*` | 24 | 3,947 | `xtask qualify <name>`; the assertions are already Rust examples writing `result.json` |
| `scripts/test_*`, `app/*.py` | 30 | 4,208 | Rust tests next to the qualifiers; rewrite each guard's subject, never copy it verbatim |
| `benchmarks/*.py` | 5 | 645 | `xtask bench run/analyze`; `/proc` and DRM `fdinfo` sampling with `serde_json` |
| `.github/scripts/*`, `verify-development-archive.py` | 5 | 543 | `xtask package` / `xtask publish`; needs `tar` + `flate2`, `gh` stays a child process |
| `development-launcher.py` | 1 | 356 | **Shipped** as `launch.py` in the development package: a Rust `dsh-native-launch` binary replaces it, so the archive stops requiring Python 3.11+; update the archive verifier and install docs together |

Also outstanding, outside `*.py`: `core/src/process_tests.rs` spawns `/usr/bin/python3`
as a forked fake-process tree for supervisor containment. It needs a small Rust
fixture binary (or a `sh`/`setsid` chain) before `python3` can leave CI.

## Out of scope

The sibling `deepseek-harness-linux` repository keeps its Python: `python/sdk`,
`python/sdk-runtime` and the office/skill scripts are shipped product, not tooling.
