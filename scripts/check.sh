#!/usr/bin/env bash
# Single entry point for DeepSeek Harness native repository checks.
#
#   scripts/check.sh                     # full lint + test suite (default)
#   scripts/check.sh --suite clippy      # one suite
#   scripts/check.sh --plan              # print commands without executing
#
# Suites, offline defaults, two-job Cargo serialization and evidence handling are
# owned by tools/xtask; this wrapper only supplies the default suite.
# The opt-in `examples`, `doctor` and `--online` paths stay explicit arguments.
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

if [ "$#" -eq 0 ]; then
  set -- --suite all
fi

exec cargo run --quiet --manifest-path "$root/tools/xtask/Cargo.toml" --locked -- verify "$@"
