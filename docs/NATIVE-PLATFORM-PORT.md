# Windows and macOS port boundary

**Status: Linux-only by construction.** `core/src/lib.rs` gates the whole Core crate with
`#![cfg(target_os = "linux")]`, and nothing in this repository has been compiled, run or verified on
Windows or macOS. That cannot change on the current Linux host, so this page is the groundwork: the
exact boundary, the mechanism each platform needs, and how the port must be verified.

## Boundary inventory (first-party Rust, r2 tree)

| Area | Files | Linux mechanism today | Portable replacement required |
| --- | --- | --- | --- |
| Host process ownership | `core/src/process.rs`, `core/src/lib.rs`, `core/src/secret.rs`, `core/src/tests.rs`, `core/src/process_tests.rs`, `core/src/secret_tests.rs` | pidfd, `libc` signals, `/proc` reads, private modes | Windows: job objects plus handle waits/termination. macOS: `posix_spawn`, process groups, `kqueue`/`waitpid` |
| Private stores and file modes | `app/src/draft_store.rs`, `app/src/export_file.rs`, `app/src/file_intake.rs`, `app/src/attachment_picker.rs`, `app/src/attachment_preview.rs`, `app/src/smoke.rs`, `app/src/config.rs` | `rustix` + `std::os::unix` 0600/0700 and `OpenOptionsExt` | Windows: ACLs on the private data root. macOS: same POSIX modes, different default paths |
| Clipboard paste | `app/src/ui.rs` | `wl-clipboard-rs` on its own Wayland connection, because Iced's bundled backend needs a focused-seat event | Windows/macOS: `arboard` or the native pasteboard API |
| Wayland drops and windowing | `app/src/ui.rs`, `app/src/smoke.rs`, `app/vendor/winit` (pinned path patch) | Bounded single-local-file URI drops on the owned winit queue | Native winit drag-and-drop events; the patch and its tests are Linux-only owners |
| Window/GPU backend | `app/src/main.rs` and Iced 0.14 | wgpu, forced `ICED_BACKEND=wgpu`, native Wayland | Shares wgpu code; needs per-platform window, theme and font handling, then separate qualification |
| Launch and packaging | `scripts/development-launcher.py` (being replaced by `xtask launch`), `tools/xtask/src/release.rs`, `RELEASES/*` | External-runtime Python launcher, glibc 2.44 and Wayland prerequisites | Per-platform launcher or a native one, per-OS installers/updaters and code signing |

Test-only Unix use sits in the same private-store files plus `core/src/tests.rs` and
`core/src/secret_tests.rs`; those need cfg gates or portable fixtures. The Node backend and its
dependency closure are separately platform-bound.

## Verification plan

- **Not possible in this session:** no Windows/macOS toolchain, runner or GPU. `rustup target add`
  and MSVC/mingw installation require network access and are not authorized.
- **Required before any portability claim:** `windows-latest` and `macos-latest` CI jobs (or physical
  machines) running `cargo check --all-targets`, the portable subset of the test suite, and a
  rendered-window qualification comparable to [the native app qualification](<NATIVE-APP-QUALIFICATION.md>).
- Until then no release may imply Windows/macOS support; `RELEASE-MANIFEST.json` records only
  `Linux x86_64`, `minimumGlibc` and `nativeWaylandRequired`.

## Ordered next steps

1. Replace the crate-level `#![cfg(target_os = "linux")]` gate with a platform module owning process
   spawn/ownership, private modes and path defaults.
2. Add the non-Linux implementation behind the same interface (job objects; process groups +
   `kqueue`), keeping the Linux owner and its tests unchanged.
3. Provide the portable clipboard and native drag-and-drop path without the winit patch.
4. Split Linux-only tests from portable ones; keep Linux-only owners authoritative for Linux.
5. Add the CI matrix and per-platform packaging — after [item 3 packaging](<NATIVE-DEVELOPMENT-PACKAGE.md>)
   lands on Linux.
