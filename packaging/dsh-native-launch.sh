#!/usr/bin/env bash
# Launch the installed native app with the configured isolated alpha runtime.
#
# Configuration lives outside this script so the desktop entry stays argument-free:
#
#   mkdir -p ~/.config/dsh-native
#   cat > ~/.config/dsh-native/config.env <<'EOF'
#   DSH_NATIVE_RUNTIME=/absolute/deepseek-harness-linux/apps/cli
#   DSH_NATIVE_HOME=/absolute/private-native-home
#   DSH_NATIVE_WORKSPACE=/absolute/project
#   EOF
#
# DSH_NATIVE_USER_HOME defaults to $HOME, DSH_NATIVE_EXPECTED_VERSION to 0.2.1-alpha.1
# and DSH_NATIVE_SCALE to 1.0. No installed-runtime fallback is used.
set -euo pipefail

self_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
binary="$self_dir/dsh-native-app"
config="${DSH_NATIVE_CONFIG:-${XDG_CONFIG_HOME:-$HOME/.config}/dsh-native/config.env}"

if [ ! -x "$binary" ]; then
  echo "dsh-native: missing executable $binary" >&2
  exit 1
fi
if [ -r "$config" ]; then
  # shellcheck disable=SC1090
  . "$config"
fi

: "${DSH_NATIVE_RUNTIME:?set DSH_NATIVE_RUNTIME in $config (built 0.2.1-alpha.1 apps/cli)}"
: "${DSH_NATIVE_HOME:?set DSH_NATIVE_HOME in $config (private native home, separate from any Harness profile)}"
: "${DSH_NATIVE_WORKSPACE:?set DSH_NATIVE_WORKSPACE in $config (existing absolute workspace)}"

exec "$binary" \
  --runtime "$DSH_NATIVE_RUNTIME" \
  --expected-version "${DSH_NATIVE_EXPECTED_VERSION:-0.2.1-alpha.1}" \
  --home "$DSH_NATIVE_HOME" \
  --user-home "${DSH_NATIVE_USER_HOME:-$HOME}" \
  --cwd "$DSH_NATIVE_WORKSPACE" \
  --scale "${DSH_NATIVE_SCALE:-1.0}" \
  "$@"
