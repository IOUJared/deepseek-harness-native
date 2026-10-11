#!/usr/bin/env bash
# Install or upgrade the native Harness client for the current user, or stage it with DESTDIR.
#
#   packaging/install.sh --binary /ABS/dsh-native-app --version 2026.10.10-r2
#   packaging/install.sh --upgrade --binary /ABS/dsh-native-app --version 2026.10.10-r2
#   packaging/install.sh --dry-run --upgrade --binary /ABS/dsh-native-app
#   packaging/install.sh --binary ./bin/dsh-native-app --prefix /usr --destdir /tmp/stage
#   packaging/install.sh --uninstall
#
# A plain install never replaces an existing file unless --force is given, so a
# hand-written desktop entry is reported instead of silently overwritten.
# --upgrade replaces only this installer's own four files, in place and atomically,
# keeps the configuration, and refuses a downgrade against the recorded manifest
# version unless --force is given. --dry-run prints the plan and writes nothing.
# The manifest at PREFIX/share/dsh-native/install-manifest records the version, the
# installed binary hash and the install time; --uninstall removes it as well.
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
upgrade=""
dry_run=""
release_version="${DSH_NATIVE_VERSION:-}"

usage() {
  awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "$0"
  exit "${1:-0}"
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --binary) binary="${2:?--binary needs a path}"; shift 2 ;;
    --prefix) prefix="${2:?--prefix needs a path}"; shift 2 ;;
    --destdir) destdir="${2:?--destdir needs a path}"; shift 2 ;;
    --version) release_version="${2:?--version needs a value}"; shift 2 ;;
    --force) force=1; shift ;;
    --upgrade) upgrade=1; shift ;;
    --dry-run) dry_run=1; shift ;;
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
manifest_dir="$prefix/share/dsh-native"
app_target="$bin_dir/dsh-native-app"
wrapper_target="$bin_dir/dsh-native"
desktop_target="$app_dir/ai.deepseek.harness.native.app.desktop"
icon_target="$icon_dir/ai.deepseek.harness.native.app.svg"
manifest_target="$manifest_dir/install-manifest"

temps=()
cleanup() {
  local temp
  for temp in ${temps[@]+"${temps[@]}"}; do rm -f -- "$temp"; done
}
trap cleanup EXIT

remove_file() {
  if [ -L "$1" ]; then
    echo "refusing to remove symlink: $1" >&2
    return 0
  fi
  if [ -f "$1" ]; then rm -f -- "$1"; echo "removed $1"; else echo "absent $1"; fi
}

# Stage beside the target and rename over it, so an interrupted upgrade can never
# leave a half-written executable or desktop entry in place.
atomic_install() { # MODE SOURCE TARGET
  local mode="$1" source="$2" target="$3" dir temp
  if [ -n "$dry_run" ]; then
    echo "would write $target (mode $mode)"
    return 0
  fi
  dir=$(dirname -- "$target")
  mkdir -p -- "$dir"
  temp="$dir/.$(basename -- "$target").new.$$"
  temps+=("$temp")
  install -m "$mode" -- "$source" "$temp"
  mv -f -- "$temp" "$target"
  echo "wrote $target"
}

manifest_value() { # KEY
  [ -f "$manifest_target" ] || return 0
  sed -n "s/^$1=//p" "$manifest_target" | head -1
}

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -- "$1" | cut -d' ' -f1
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -- "$1" | cut -d' ' -f1
  fi
}

# Compare two calendar versions (YYYY.MM.DD-rN): prints -1, 0 or 1. Prints nothing
# when either side does not follow the scheme, so the installer never claims a
# downgrade for a hand-supplied value it cannot order.
compare_versions() {
  local a="$1" b="$2"
  [[ "$a" =~ ^([0-9]{4})\.([0-9]{2})\.([0-9]{2})-r([0-9]+)$ ]] || return 0
  local a_date="${BASH_REMATCH[1]}${BASH_REMATCH[2]}${BASH_REMATCH[3]}" a_release="${BASH_REMATCH[4]}"
  [[ "$b" =~ ^([0-9]{4})\.([0-9]{2})\.([0-9]{2})-r([0-9]+)$ ]] || return 0
  local b_date="${BASH_REMATCH[1]}${BASH_REMATCH[2]}${BASH_REMATCH[3]}" b_release="${BASH_REMATCH[4]}"
  if [ "$a_date" -lt "$b_date" ] || { [ "$a_date" = "$b_date" ] && [ "$a_release" -lt "$b_release" ]; }; then
    echo -1
  elif [ "$a_date" = "$b_date" ] && [ "$a_release" = "$b_release" ]; then
    echo 0
  else
    echo 1
  fi
}

installed_version=$(manifest_value version)

if [ "$action" = uninstall ]; then
  remove_file "$desktop_target"
  remove_file "$icon_target"
  remove_file "$wrapper_target"
  remove_file "$app_target"
  remove_file "$manifest_target"
  if [ -d "$manifest_dir" ] && [ -z "$(ls -A -- "$manifest_dir" 2>/dev/null)" ]; then rmdir -- "$manifest_dir"; fi
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

if [ -n "$upgrade" ] && [ ! -f "$app_target" ]; then
  echo "--upgrade needs an existing installation at $app_target; install it first" >&2
  exit 1
fi
if [ -n "$upgrade" ] && [ -n "$release_version" ] && [ -n "$installed_version" ]; then
  order=$(compare_versions "$release_version" "$installed_version")
  if [ "$order" = "-1" ] && [ -z "$force" ]; then
    echo "refusing to downgrade $installed_version to $release_version (pass --force to overwrite)" >&2
    exit 1
  fi
fi

require_free() {
  if [ -e "$1" ] || [ -L "$1" ]; then
    if [ -n "$force" ]; then
      [ -L "$1" ] && { echo "refusing to replace symlink: $1" >&2; exit 1; }
      return 0
    fi
    echo "refusing to replace existing $1 (pass --upgrade, or --force to overwrite)" >&2
    exit 1
  fi
}
if [ -z "$upgrade" ]; then
  for target in "$app_target" "$wrapper_target" "$desktop_target" "$icon_target"; do require_free "$target"; done
fi

# Fail before writing anything when the prefix cannot be written, rather than
# reporting a partial install from whichever `install` call fails first.
if [ -z "$dry_run" ]; then
  probe="$bin_dir"
  while [ ! -e "$probe" ] && [ "$probe" != "/" ] && [ "$probe" != "." ]; do probe=$(dirname -- "$probe"); done
  if [ ! -w "$probe" ]; then
    echo "prefix is not writable: $probe (use sudo, or PREFIX=\$HOME/.local)" >&2
    exit 1
  fi
fi

if [ -n "$upgrade" ]; then
  echo "upgrading ${installed_version:-unknown} -> ${release_version:-unknown} at $prefix"
fi
atomic_install 755 "$binary" "$app_target"
atomic_install 755 "$here/dsh-native-launch.sh" "$wrapper_target"
atomic_install 644 "$here/ai.deepseek.harness.native.app.desktop" "$desktop_target"
atomic_install 644 "$here/ai.deepseek.harness.native.app.svg" "$icon_target"

binary_hash=$(sha256_of "$binary")
if [ -n "$dry_run" ]; then
  echo "would write $manifest_target"
  echo "dry run: nothing was written"
  exit 0
fi
manifest_temp="$manifest_dir/.install-manifest.new.$$"
temps+=("$manifest_temp")
mkdir -p -- "$manifest_dir"
{
  echo "version=${release_version:-unknown}"
  echo "binary_sha256=${binary_hash:-unknown}"
  echo "installed_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "prefix=$prefix"
} > "$manifest_temp"
chmod 644 -- "$manifest_temp"
mv -f -- "$manifest_temp" "$manifest_target"
echo "wrote $manifest_target"

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

[ -n "$upgrade" ] && echo "upgraded:" || echo "installed:"
echo "  $app_target"
echo "  $wrapper_target"
echo "  $desktop_target"
echo "  $icon_target"
echo "  $manifest_target (version ${release_version:-unknown})"
[ -n "$config_note" ] && echo "config: $config_note"
case ":$PATH:" in
  *":$bin_dir:"*) ;;
  *) [ -n "$destdir" ] || echo "note: add $bin_dir to PATH (or launch with its full path)";;
esac
