#!/usr/bin/env bash
# Install the native Harness client for the current user, or stage it with DESTDIR.
#
#   packaging/install.sh --binary /ABS/dsh-native-app          # ~/.local, user-local
#   packaging/install.sh --binary ./bin/dsh-native-app --prefix /usr --destdir /tmp/stage
#   packaging/install.sh --uninstall
#
# Existing files are never replaced unless --force is given.
#
# The backend, Node and any profile stay external: this installs the executable, a
# launcher wrapper, the desktop entry and the icon only.
set -euo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
prefix="${PREFIX:-$HOME/.local}"
destdir="${DESTDIR:-}"
binary=""
action=install
force=""

usage() {
  awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "$0"
  exit "${1:-0}"
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --binary) binary="${2:?--binary needs a path}"; shift 2 ;;
    --prefix) prefix="${2:?--prefix needs a path}"; shift 2 ;;
    --destdir) destdir="${2:?--destdir needs a path}"; shift 2 ;;
    --force) force=1; shift ;;
    --uninstall) action=uninstall; shift ;;
    -h|--help) usage 0 ;;
    *) echo "unknown option: $1" >&2; usage 1 ;;
  esac
done

case "$prefix" in
  ""|"/") echo "refusing unsafe prefix: '$prefix'" >&2; exit 1 ;;
esac
[ -n "$destdir" ] && prefix="${destdir%/}$prefix"

bin_dir="$prefix/bin"
app_dir="$prefix/share/applications"
icon_dir="$prefix/share/icons/hicolor/scalable/apps"
app_target="$bin_dir/dsh-native-app"
wrapper_target="$bin_dir/dsh-native"
desktop_target="$app_dir/ai.deepseek.harness.native.app.desktop"
icon_target="$icon_dir/ai.deepseek.harness.native.app.svg"

remove_file() {
  if [ -L "$1" ]; then
    echo "refusing to remove symlink: $1" >&2
    return 0
  fi
  if [ -f "$1" ]; then rm -f -- "$1"; echo "removed $1"; else echo "absent $1"; fi
}

if [ "$action" = uninstall ]; then
  remove_file "$desktop_target"
  remove_file "$icon_target"
  remove_file "$wrapper_target"
  remove_file "$app_target"
  [ -n "$destdir" ] || echo "configuration kept at ${XDG_CONFIG_HOME:-$HOME/.config}/dsh-native/config.env"
  exit 0
fi

if [ -z "$binary" ]; then
  for candidate in "$here/../app/target/release/dsh-native-app" "$here/../bin/dsh-native-app"; do
    if [ -f "$candidate" ]; then binary="$candidate"; break; fi
  done
fi
if [ -z "$binary" ] || [ ! -f "$binary" ] || [ -L "$binary" ] || [ ! -x "$binary" ]; then
  echo "need --binary pointing at a regular executable dsh-native-app" >&2
  exit 1
fi

require_free() {
  if [ -e "$1" ] || [ -L "$1" ]; then
    if [ -n "$force" ]; then
      [ -L "$1" ] && { echo "refusing to replace symlink: $1" >&2; exit 1; }
      return 0
    fi
    echo "refusing to replace existing $1 (pass --force to overwrite)" >&2
    exit 1
  fi
}
for target in "$app_target" "$wrapper_target" "$desktop_target" "$icon_target"; do require_free "$target"; done

install -Dm755 -- "$binary" "$app_target"
install -Dm755 -- "$here/dsh-native-launch.sh" "$wrapper_target"
install -Dm644 -- "$here/ai.deepseek.harness.native.app.desktop" "$desktop_target"
install -Dm644 -- "$here/ai.deepseek.harness.native.app.svg" "$icon_target"

config_note=""
if [ -z "$destdir" ]; then
  config="${XDG_CONFIG_HOME:-$HOME/.config}/dsh-native/config.env"
  if [ -e "$config" ]; then
    config_note="kept existing $config"
  else
    mkdir -p -- "$(dirname -- "$config")"
    (umask 077; cat > "$config" <<'EOF'
# Required: absolute paths. The backend must be the built 0.2.1-alpha.1 apps/cli.
DSH_NATIVE_RUNTIME=/absolute/deepseek-harness-linux/apps/cli
DSH_NATIVE_HOME=/absolute/private-native-home
DSH_NATIVE_WORKSPACE=/absolute/project
# Optional: DSH_NATIVE_USER_HOME, DSH_NATIVE_EXPECTED_VERSION, DSH_NATIVE_SCALE
EOF
    )
    config_note="wrote $config — set the three paths before launching"
  fi
  if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$app_dir" >/dev/null 2>&1 || true
  fi
fi

echo "installed:"
echo "  $app_target"
echo "  $wrapper_target"
echo "  $desktop_target"
echo "  $icon_target"
[ -n "$config_note" ] && echo "config: $config_note"
case ":$PATH:" in
  *":$bin_dir:"*) ;;
  *) [ -n "$destdir" ] || echo "note: add $bin_dir to PATH (or launch with its full path)";;
esac
