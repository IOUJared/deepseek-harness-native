# Install the native client

User-local install: executable, launcher wrapper, desktop entry and icon. The backend, Node and all
profiles stay external — no root, no system changes, no bundled runtime yet.

## Prerequisites

| Requirement | Value |
| --- | --- |
| Backend | built isolated `0.2.1-alpha.1` CLI (`deepseek-harness-linux/apps/cli`) |
| Node | major 26 (`/usr/bin/node` by default) |
| Display | native Wayland, working GPU/EGL/Vulkan drivers, xkbcommon, DejaVu fonts |
| Python | 3.11+ only for the archive launcher, not for this install |

## Install

```sh
packaging/install.sh --binary /ABS/dsh-native-app     # ~/.local, no root
packaging/install.sh --binary ./bin/dsh-native-app --prefix /usr --destdir /tmp/stage   # staging
packaging/install.sh --uninstall
```

Installed paths:

| Path | Content |
| --- | --- |
| `~/.local/bin/dsh-native-app` | the executable |
| `~/.local/bin/dsh-native` | launcher wrapper that reads the config below |
| `~/.local/share/applications/ai.deepseek.harness.native.app.desktop` | menu entry (Wayland app id) |
| `~/.local/share/icons/hicolor/scalable/apps/ai.deepseek.harness.native.app.svg` | icon |
| `~/.config/dsh-native/config.env` | created once, `0600`, never overwritten |

Edit the config before the first launch:

```sh
DSH_NATIVE_RUNTIME=/absolute/deepseek-harness-linux/apps/cli
DSH_NATIVE_HOME=/absolute/private-native-home      # separate from every Harness profile
DSH_NATIVE_WORKSPACE=/absolute/project
# optional: DSH_NATIVE_USER_HOME, DSH_NATIVE_EXPECTED_VERSION, DSH_NATIVE_SCALE
```

Then launch `dsh-native` (or the **Harness Native** menu entry). Uninstall removes the four installed
files and keeps the config; a repeated uninstall is safe and reports `absent`.

`install.sh` **never replaces an existing file** unless `--force` is passed, so a hand-written entry
at the same desktop path is reported rather than silently overwritten. Compare its `Exec` and
`--home` first: this installer uses a config file, and the config's `DSH_NATIVE_HOME` decides which
session data the app opens.

## Arch package

`packaging/PKGBUILD` packages the released archive (the pinned `2026.10.10-r2` tarball and its
SHA-256) with the same wrapper, desktop entry and icon. `makepkg -si` downloads the release asset at
build time; the backend stays an optional external dependency.

## Gaps (not provided here)

- **No bundled Node or backend**: both must already exist, so this is not yet a standalone install.
- **No updater**: reinstalling a newer archive is manual.
- **No Windows/macOS package**: see [the port boundary](<NATIVE-PLATFORM-PORT.md>).
- The staged install is verified by `scripts/test_native_install.py`; it does not prove a clean
  machine, a system-wide install or non-Arch distributions.
