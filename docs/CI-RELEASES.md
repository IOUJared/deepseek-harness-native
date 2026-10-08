# GitHub checks and releases

[Workflow](<../.github/workflows/native.yml>) · [Actions](https://github.com/IOUJared/deepseek-harness-native/actions) · [Releases](https://github.com/IOUJared/deepseek-harness-native/releases)

| Event | Result |
|---|---|
| Any push or pull request | Linux compilation, headless tests and release-build `--help` smoke |
| Successful push to `main` | Automatic versioned development prerelease |
| Failed checks | No release |
| Manual workflow run | Checks only |
| Rerun of a published main build | Verify tag target and original downloaded assets; do not overwrite them |

## Versions

Tags use the app's plain [Cargo version](<../app/Cargo.toml>) plus the workflow run number: `v0.1.0-dev.1`, `v0.1.0-dev.2`, and so on. Gaps are expected when PR, branch or failed runs consume numbers. Reruns keep their number. Changing the app version starts a new series; automation does not edit manifests or create commit loops. Each tag targets the exact tested push commit, not the latest branch tip.

## Checks

- Ubuntu 24.04 / Linux x86_64, Rust 1.99.0 and Node 26.10.0.
- Separate Core, transport and app suites; PUBLIC layout models and all-target compilation.
- Supervisor tests, Python qualification/packaging guards and release metadata tests.
- Locked public dependencies; standalone [fake-wire npm dependency](<../transport/tests/package.json>) instead of a sibling backend installation.
- Empty synthetic test scratch only; no private evidence, API keys, login, GUI startup or real model calls.

Read-only permissions apply to checks. Only the successful main-push publishing job receives `contents: write`. Actions are pinned to commit IDs. PR updates cancel superseded PR checks; distinct pushes keep their own runs.

## Downloads

Assets are uploaded to a draft, downloaded and verified before publication. Interrupted drafts can resume; published downloads are never overwritten. Each prerelease contains a GNU/Linux executable archive and SHA-256 checksum. Inside: exact-commit source snapshot, build metadata, measured glibc symbol requirements/dynamic dependencies, MIT license and Rust dependency license inventory. [Packager](<../.github/scripts/package_release.py>) uses that runner's Cargo registry, not a local developer cache.

The compatible `0.2.1-alpha.1` backend, Node, profiles and credentials are **not included**. Rendering still requires native Wayland, GPU drivers, xkbcommon and system fonts. These are development downloads—not an installer, self-contained runtime, AppImage, general Linux portability claim or finished Windows/macOS release. Headless CI does not verify rendering, physical input, accessibility, real accounts or model behavior.

## Local checks

```sh
npm ci --prefix transport/tests --ignore-scripts
mkdir -p app/evidence
node --test core/runtime/*.test.mjs
python3 -B -m unittest discover -s .github/scripts -p 'test_*.py'
cargo test --locked --manifest-path core/Cargo.toml --features transport -- --test-threads=2
cargo test --locked --manifest-path transport/Cargo.toml -- --test-threads=2
cargo test --locked --manifest-path app/Cargo.toml --bin dsh-native-app -- --test-threads=2
```

Rust fixtures use `/usr/bin/node` and `/usr/bin/python3`. The workflow binds Node only on its disposable hosted runner; do not replace system interpreters locally. The workflow lists the remaining checks. GitHub's repository settings must allow Actions and workflow-token release creation; no personal token is required.
