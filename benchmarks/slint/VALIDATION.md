# Build and isolated geometry qualification

Validated on 2026-10-06 with Rust 1.99.0; declared/upstream MSRV 1.92 (not separately tested with a 1.92 toolchain). Initial implementation was build-only. The parent subsequently authorized isolated, owned graphical qualification to fix the observed application-scale and layout issues. These are synthetic frontend qualification runs, **not performance baselines or integrated-app evidence**.

## Build

- `cargo fetch` and `cargo check --locked`: passed.
- Final `cargo test --locked`: **13 passed**, 0 failed, all headless; exact synthetic data, bounded visible rows, projection, bounce, stream target/content, CLI, ready-relative schedule, PNG round-trip/path policy, logical-to-physical scaling, detection of the 960×640 scale regression, and detection of centered sidebar/header pane geometry.
- `cargo fmt --check`: passed.
- Final `cargo build --release --locked`: passed; opt-level 3, thin LTO, one codegen unit, stripped symbols.
- Runtime dependency-tree audit: no Qt, Skia, WGPU, Tauri, Wry, or WebKit engine crates enabled. Limited PNG/JPEG and transitive SVG GIF/WebP remain upstream, without image-default-formats/AVIF/TIFF/EXR groups.
- Explicit Slint features: `std`, `backend-winit-wayland`, `renderer-software`, `renderer-femtovg`, `compat-1-18`; defaults disabled; version exactly 1.18.1.
- Workspace shared absolute Cargo cache, own absolute target directory, `CARGO_BUILD_JOBS=3`.
- Host Fontconfig resolved `DejaVu Sans` at `/usr/share/fonts/TTF/DejaVuSans.ttf`; no font installation or desktop changes.

## Final executable identity

```text
/home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-native/benchmarks/slint/target/release/dsh-slint-benchmark
15,152,656 bytes
SHA256 946e88fb9bad6e85a644a346a54d5a4eee38a974fbbb3e8ebd91b2efb14e9c86
ELF 64-bit LSB PIE, x86-64, dynamically linked, stripped
```

All implementation-agent compiler and graphical jobs ended and were collected before notifying the parent to resume comparative runs.

## Corrections and observed proof

1. **Scale/lifecycle:** public `Window::set_size(LogicalSize::new(1200, 800))` before `show()` marks an explicit request and prevents Slint 1.18.1's later preferred-size calculation from overwriting scale-adjusted creation attributes. No upstream patch or extra framework feature was added. Benchmark constraints remain fixed only in `--bench`; normal mode remains resizable.
2. **Pane origins:** explicit sidebar/content/header/transcript/composer origins prevent Slint's implicit centering defaults from putting the sidebar behind the transcript and the header in the middle of the content. Composer top border is explicitly at its origin. Visual qualification caught this issue; compilation alone had not.
3. **Bounded observer:** exactly one read-only `settled_geometry` event approximately 750 ms after original `ready`, with actual logical/physical dimensions, effective scale, actual pane rectangles, and checked geometry/layout flags. It does not request redraw, repeat, infer presentation, relabel startup readiness, or alter absolute ready-relative 6/12/18/22-second benchmark deadlines.

Final corrected-layout qualification artifacts:

- [Software result JSON](qualification-scale-fix-1791316340077490488/software/result.json) and [internally rendered synthetic PNG](qualification-scale-fix-1791316340077490488/software/synthetic-scale-1.25.png).
- [FemtoVG result JSON](qualification-scale-fix-1791316340077490488/femtovg/result.json) and [internally rendered synthetic PNG](qualification-scale-fix-1791316340077490488/femtovg/synthetic-scale-1.25.png).

Both cases requested application scale 1.25 and observed:

```text
logical_size [1200, 800]
physical_size [1500, 1000]
effective_scale 1.25
sidebar XYWH [0, 0, 260, 800]
header XYWH [260, 0, 940, 48]
transcript XYWH [260, 48, 940, 648]
composer XYWH [260, 696, 940, 104]
geometry_matches_shared_workload true
layout_matches_shared_workload true
```

Both internal PNGs are 1500×1000 and were inspected directly: all three workspaces, twelve sessions, Settings, header, fixed-height transcript rows and composer are visible in their intended regions with no sidebar/header overlap. The renderers' text rasterization differs, as expected. PNGs contain only the internally rendered synthetic app, never other windows or a desktop screenshot.

Both app children exited **0**, without timeout; each completed 374 scroll updates and 119 stream updates. Ready-to-finish was 21,999.58 ms (software) and 21,999.28 ms (FemtoVG), demonstrating bounded scheduling, **not comparative performance**. FemtoVG observed its actual `AfterRendering` command-submission callback with `GraphicsAPI::NativeOpenGL`; its first ready observation already had correct 1.25 geometry. Software's original ready remains honestly labeled `event_loop_after_show_proxy_not_render_or_presentation` and reports initial scale 1.0 before native creation; its one-shot settled observation reports the correct scale 1.25. Software submission counts remain null. No presentation or GPU completion claim is made.

## Evidence limitations and rejected earlier results

The delegated command sandbox uses a PID namespace: app PIDs were 3 and 8, not necessarily the compositor's host PIDs. The qualification helper's exact own-PID `hyprctl` match returned no client, so **overall helper qualification exited 1**, deliberately refusing to claim native Wayland mapping proof. It did not match some other window or bypass the fixed sandbox. GPU stderr included Mesa device-initialization warnings, so these sandbox cases do not establish hardware acceleration, hardware-GPU performance, or comparability with the parent's approved host-context runs.

The parent must still verify its spawned host PID's app id, `mapped: true`, `xwayland: false`, and physical size, then run clean no-snapshot timing/process-tree baselines. Its earlier scale-1.25 960×640 logical results are not comparable. Earlier isolated artifacts under `qualification-scale-fix-1791315976619750148` corrected size but still had the **invalid pre-layout-fix** sidebar/header overlap; do not use them as appearance or performance qualification.

Still unverified here: live interactive Send/Stop/resizing, host PID-native mapping, hardware acceleration, compositor presentation/scanout, clean frontend CPU/RSS/PSS comparisons, integrated-backend performance or full-app memory. Read [the README](README.md) for flags, readiness semantics, snapshot safety and licensing. No Slint distribution license option has been accepted for the user.
