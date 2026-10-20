# GitHub checks and releases

[Workflow](<../.github/workflows/native.yml>) · [Actions](https://github.com/IOUJared/deepseek-harness-native/actions) · [Releases](https://github.com/IOUJared/deepseek-harness-native/releases)

| Event | Result |
|---|---|
| Any push or pull request | Linux compilation, headless tests and release-build `--help` smoke |
| Successful plain-version push to `main` | Legacy automatic `dev.run` prerelease |
| Calendar-version push to `main` | Checks only; explicit combined-source release publication |
| Failed checks | No release |
| Manual workflow run | Checks only |
| Rerun of a published main build | Verify tag target and original downloaded assets; do not overwrite them |

## Versions

Calendar [Cargo versions](<../app/Cargo.toml>) use `year.month.day-rN` in America/Chicago; `N` counts that day's explicit releases. `2026.10.10-r1` maps to `v2026.10.10-r1`, `2026.10.10-r2` to `v2026.10.10-r2` and `2026.10.20-r1` to `v2026.10.20-r1`, independent of CI run numbers. The release collects reviewed combined agent work, a changelog, matching rebuilt consumers and verified assets; CI does not race or replace these calendar assets. [Changelog](<../CHANGELOG.md>) · [r1 checks](<../RELEASES/2026.10.10-r1.md>) · [r2 checks](<../RELEASES/2026.10.10-r2.md>) · [2026.10.20-r1 checks](<../RELEASES/2026.10.20-r1.md>).

Legacy plain versions retain `v0.1.0-dev.run` automation. Gaps reflect workflow runs; reruns keep their number and verify existing assets. Every release targets its exact tested commit, not a moving branch tip. Automation does not edit manifests or create commit loops.

## Checks

- Ubuntu 24.04 / Linux x86_64, Rust 1.99.0 and Node 26.10.0.
- Separate Core, transport and app suites; PUBLIC layout models and all-target compilation.
- Supervisor tests, the remaining Python qualification guards, `tools/xtask` tests (including the release commands' own) and the packaging/benchmark guards that moved into them.
- Locked public dependencies and standalone [fake-wire npm dependency](<../transport/tests/package.json>). Native-only CI has no built sibling alpha: **22 real-flock/Cordis tests explicitly skip**, while seven portable bridge tests still run. `DSH_REQUIRE_ALPHA_TEST_ARTIFACTS=1` makes absent artifacts fail; wrong versions, broken imports/ABI/exports and assertions always fail. Strict companion-artifact checks and full-profile native composition remain separate release evidence.
- Empty synthetic test scratch only; no private evidence, API keys, login, GUI startup or real model calls.

Read-only permissions apply to checks. Only the successful main-push publishing job receives `contents: write`. Actions are pinned to commit IDs. PR updates cancel superseded PR checks; distinct pushes keep their own runs.

## Downloads

Assets are uploaded to a draft, downloaded and verified before publication. Interrupted drafts can resume; published downloads are never overwritten. Each prerelease contains a GNU/Linux executable archive and SHA-256 checksum. Inside: exact-commit source snapshot, build metadata, measured glibc symbol requirements/dynamic dependencies, MIT license and Rust dependency license inventory. [Packager](<../tools/xtask/src/release.rs>) uses that runner's Cargo registry, not a local developer cache.

The compatible `0.2.1-alpha.1` backend, Node, profiles and credentials are **not included**. Rendering still requires native Wayland, GPU drivers, xkbcommon and system fonts. These are development downloads—not an installer, self-contained runtime, AppImage, general Linux portability claim or finished Windows/macOS release. Headless CI does not verify rendering, physical input, accessibility, real accounts or model behavior.

## Local checks

```sh
npm ci --prefix transport/tests --ignore-scripts
mkdir -p app/evidence
node --test core/runtime/*.test.mjs
cargo test --locked --manifest-path tools/xtask/Cargo.toml -- --test-threads=2
python3 -B -m unittest discover -s scripts -p 'test_*.py'
cargo test --locked --manifest-path core/Cargo.toml --features transport -- --test-threads=2
cargo test --locked --manifest-path transport/Cargo.toml -- --test-threads=2
cargo test --locked --manifest-path app/Cargo.toml --bin dsh-native-app -- --test-threads=2
```

Rust fixtures use `/usr/bin/node` and `/usr/bin/python3`. The workflow binds Node only on its disposable hosted runner; do not replace system interpreters locally. The workflow lists the remaining checks. GitHub's repository settings must allow Actions and workflow-token release creation; no personal token is required.
