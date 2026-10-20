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
files, the manifest and the now-empty manifest directory, and keeps the config; a repeated uninstall
is safe and reports `absent`.

## Upgrade

```sh
packaging/install.sh --upgrade --binary /ABS/new/dsh-native-app --version 2026.10.20-r1
packaging/install.sh --dry-run --upgrade --binary /ABS/new/dsh-native-app
```

- `--upgrade` requires an existing installation and replaces **only this installer's four files**, each
  staged beside its target and renamed over it, so an interrupted upgrade cannot leave a half-written
  executable, wrapper, entry or icon. A plain install still refuses to touch anything it did not write.
- `--version` records the release identity. `PREFIX/share/dsh-native/install-manifest` holds `version`,
  `binary_sha256`, `installed_at` and `prefix`; it never records the path the binary came from.
- An upgrade whose `--version` is **older** than the recorded one is refused unless `--force` is given.
  The comparison understands the `YYYY.MM.DD-rN` scheme; for any other value the installer records it
  and makes no ordering claim rather than guessing.
- `--dry-run` prints every would-be write and changes nothing.
- The config is never rewritten: an existing `config.env` is reported as kept, and editing it before or
  after an upgrade is expected.
- The manifest is a record, **not a signature**: it proves nothing about where a binary came from.

`install.sh` **never replaces an existing file** unless `--force` is passed, so a hand-written entry
at the same desktop path is reported rather than silently overwritten. Compare its `Exec` and
`--home` first: this installer uses a config file, and the config's `DSH_NATIVE_HOME` decides which
session data the app opens.

## Arch package

`packaging/PKGBUILD` packages the released archive (the pinned `2026.10.20-r1` tarball and its
SHA-256) with the same wrapper, desktop entry and icon. `makepkg -si` downloads the release asset at
build time; the backend stays an optional external dependency.

## Gaps (not provided here)

- **No bundled Node or backend**: both must already exist, so this is not yet a standalone install.
- **No remote update check**: `--upgrade` installs a release you already downloaded and only
  orders it against the local manifest. Nothing queries a server, verifies a signature or
  upgrades the backend, Node or the profile.
- **No Windows/macOS package**: see [the port boundary](<NATIVE-PLATFORM-PORT.md>).
- The staged install **and upgrade paths** — replace, version record, downgrade refusal, `--force`
  override, unorderable version, dry run, config preservation and manifest cleanup — are verified by
  `scripts/test_native_install.py`; that does not prove a clean machine, a root-owned system install or
  non-Arch distributions.
