# Slint native frontend comparison prototype

An isolated, synthetic toolkit prototype, **not a finished native Harness application**. The authoritative shared workload is [WORKLOAD.md](../WORKLOAD.md). This package does not use Electron, Tauri, HTTP, Node, a webview, browser engines, real sessions, models, accounts, or normal Harness state. It writes no application state or settings. No desktop, display, Hyprland, theme, or launcher configuration is changed.

## Build and headless tests

Rust MSRV 1.92; installed compiler was 1.99.0. Slint is pinned to **1.18.1**. The committed Cargo lock file pins the resolved dependency graph. Separate target directory avoids contention with the Iced prototype:

```sh
export CARGO_HOME=/home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-tauri/.cargo-home
export CARGO_TARGET_DIR=/home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-native/benchmarks/slint/target
export CARGO_BUILD_JOBS=3
cargo test --locked --manifest-path /home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-native/benchmarks/slint/Cargo.toml
cargo build --release --locked --manifest-path /home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-native/benchmarks/slint/Cargo.toml
```

Executable: `/home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-native/benchmarks/slint/target/release/dsh-slint-benchmark`.

Exact public Slint features, with `default-features = false`:

- `std`
- `backend-winit-wayland`
- `renderer-software`
- `renderer-femtovg`
- `compat-1-18`

No Qt, Skia, WGPU/Vulkan renderer, X11 window backend, system tray, accessibility bridge, image-default-formats, debug instrumentation, or web backend is selected. Upstream Slint's **native** FemtoVG dependency unconditionally requests core `image-decoders` and `svg`; its limited PNG/JPEG/GIF/WebP codecs and SVG support still compile transitively. These cannot be disabled through the supported public features while retaining this renderer. Large optional AVIF/TIFF/EXR codec groups are not selected. A downloaded package in Cargo's cache or an optional entry in the lock file does **not** mean that package is compiled. The release profile is exactly `opt-level=3`, thin LTO, one codegen unit, stripped symbols, matching the shared specification.

## CLI and approved parent-runner execution

Flags: `--bench`, `--scale NUMBER`, `--renderer software|femtovg`, `--snapshot ABSOLUTE_NEW.png`, `--help` (`-h`). Without `--bench`, the resizable interactive prototype remains open. Renderer defaults to FemtoVG and honors `SLINT_BACKEND=winit-software` or `winit-femtovg`; an explicit `--renderer` takes precedence. Unknown arguments and unsupported backend values fail, rather than silently selecting another renderer.

**Graphical tests belong to the approved parent runner; do not launch these examples without that approval.** Parent runner should remove `DISPLAY`, preserve `WAYLAND_DISPLAY` and valid `XDG_RUNTIME_DIR`, and provide isolated `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, and `XDG_DATA_HOME` in the test child. Examples of *child* arguments/environment:

```text
SLINT_BACKEND=winit-software  dsh-slint-benchmark --bench --scale 1
SLINT_BACKEND=winit-femtovg   dsh-slint-benchmark --bench --scale 1.25
```

`--scale` sets the **test child's** `SLINT_SCALE_FACTOR` before Slint or any threads initialize; Rust 2024's unsafe environment mutation is localized with this explicit safety invariant. Slint 1.18.1's winit backend reads the override, adjusts logical sizes to physical sizes, and sets the window's UI scale. This is supported application-controlled UI scaling, **not** proof of compositor fractional scaling. Initial logical content is 1200×800; at application scale 1.25, requested physical content is 1500×1000. `--bench` fixes min/max logical dimensions to 1200×800 so Hyprland's ordinary fixed-size-window handling can maintain equivalent geometry; interactive mode remains resizable. Before `show()`, the prototype now calls the supported public `Window::set_size(LogicalSize::new(1200, 800))`. In pinned Slint 1.18.1, relying only on `preferred-width`/`preferred-height` lets `ensure_window()` overwrite the scale-adjusted attributes with its preferred size after the scale-factor fixup; parent qualification exposed a 960×640 logical / 1200×800 physical viewport at app scale 1.25. An explicit pre-creation logical request sets `has_explicit_size`, preventing that later preferred-size overwrite while allowing the supported scale conversion. No upstream library was patched. Actual dimensions and effective scale are emitted; the parent must reject or separately report runs where actual geometry differs. Software `ready` is deliberately an early startup proxy and can precede native mapping and scale initialization; it is not settled geometry evidence.

### Separate opt-in synthetic snapshot qualification

`--snapshot /absolute/path/inside/this/slint/package/new.png` requests one internal `Window::take_snapshot()` capture 250 ms after the initial readiness observation. The parent directory must already exist and canonicalize inside this package. Relative paths, parent-directory traversal, existing files (including symlinks), and symlink ancestors are refused. Output creation uses descriptor-by-descriptor `openat` traversal with `NOFOLLOW` for every directory and `EXCL|NOFOLLOW` for the new file, preventing symlink substitution from redirecting output. The file is a PNG of only the app's internally rendered synthetic content, never a compositor/desktop screenshot. No capture or file write occurs without this flag. A failed or unsupported renderer snapshot emits `snapshot_error`, prints the actual failure, cancels timers, exits with failure, and never substitutes a desktop capture or fake image. Success emits `snapshot_saved`. These two optional events supplement, not replace, the shared timing protocol.

Use snapshots only in a **separate qualification run**, not timing baselines: take_snapshot may re-render the scene and encoding/readback adds work. `ready` explicitly reports `snapshot_requested` and `benchmark_comparable: false` for these runs. Minimal pinned direct PNG encoding (`png = 0.18.1`, defaults disabled) and descriptor-safe path handling (`rustix = 1.1.5`, only `std`/`fs`) reuse crates already in the framework dependency graph; no broad image-format group is added. Headless tests also verify exact PNG pixel round-trip, refused overwrite, refused symlink output/ancestors, and refused paths outside this package. Graphical snapshot validity still needs the approved parent runner.

## Screen and virtualization

Pure compiled Slint DSL widgets and native Rust data. Explicit standard dark Rosé Pine palette, 260-pixel sidebar, 48-pixel header, 104-pixel composer, fixed 72-pixel rows including border/padding. Exactly three synthetic workspaces, twelve synthetic selectable sessions, and 1,000 initial messages. Send appends only a synthetic user message. Stop stops only synthetic streaming; neither operation creates a process or calls a model. Settings only prints an explanation and never touches real settings.

All 1,000 full message records remain in Rust. Only intersecting visible rows plus two rows of overscan per side enter the Slint `VecModel`; no 1,000-row widget tree is created per paint. Scroll wheel callbacks and benchmark scrolling update that bounded slice. Stream updates append the exact ` token` fragment to the last **intersecting** message, never a hidden overscan row. Fixed-height text elides at the right edge, retaining the complete growing string in the model. Headless tests cover exact initial data, half-open row boundaries, overscan limits, every intersecting row, end clamping, bounce behavior, synthetic send/stream content, CLI validation, deadlines, and native row projection.

Font request: **DejaVu Sans, 14 logical pixels**. On this host, `fc-match -f '%{family}\n%{file}\n' 'DejaVu Sans'` resolved `DejaVu Sans` at `/usr/share/fonts/TTF/DejaVuSans.ttf`. Slint and Iced must use the same resolved font. The emitted font family is a request, not a claim that a fallback was observed; the parent should retain font-resolution evidence in the isolated test environment. No fonts are installed or changed by this package.

## StopwatchReady and honest readiness

`StopwatchReady` starts at the first line of `main`. Every newline JSON record is prefixed `DSH_BENCH:` and includes `framework: "slint"`, `event`, and `elapsed_ms` from that instant. Events in a successful bounded run are:

1. `initialized`
2. `ready`
3. `idle_start`
4. `scroll_start`
5. `scroll_end`
6. `stream_start`
7. `stream_end` (also begins final idle)
8. `finished`

A supplemental **`settled_geometry`** event fires from exactly one one-shot timer scheduled 750 ms after `ready`, for both renderer variants. Its precise label is `one_shot_750ms_after_ready_window_geometry_observation_not_frame_or_presentation`. It reads actual root logical dimensions, public-window physical dimensions/scale, derived window logical dimensions, expected physical dimensions, and the logical XYWH rectangles of sidebar/header/transcript/composer. The checked `geometry_matches_shared_workload` flag requires both correct window scaling and correct pane layout; `layout_matches_shared_workload` is also emitted separately. Explicit pane origins avoid Slint's implicit child-centering defaults, which visual qualification exposed as an overlapped sidebar/header. It never requests redraw, rechecks repeatedly, resets readiness, or changes the benchmark schedule. A fixed-delay observation is not proof of presentation or of stability after subsequent compositor changes; validate its dimensions against the same test PID's compositor facts. Parent runner should use this event for geometry qualification, especially because software's early ready proxy can report initial scale 1.0 before native creation. Earlier app-scale 1.25 results with unequal viewport geometry are not comparable and must be rerun.

`ready` includes renderer, measurement label, logical dimensions, physical dimensions at observation, effective scale, requested app scale, font request, process PID/app id, and notifier support. Readiness meanings deliberately differ:

- **FemtoVG/OpenGL:** `first_frame_render_submission_complete_before_swap_not_presented`. The real `RenderingState::AfterRendering` notification occurs after canvas flush and `graphics_backend.submit_commands`, but **before** `present_surface`/swap buffers. It measures a render-command submission-complete observation, not GPU completion, Wayland presentation, compositor scanout, or a shown pixel. The notifier counts later naturally occurring submissions and active-phase intervals; it never requests a redraw.
- **Software:** `event_loop_after_show_proxy_not_render_or_presentation`. If `set_rendering_notifier` returns `Unsupported`, a zero-duration **one-shot** event-loop timer after `show()` emits this explicitly labeled startup proxy. It is not a first rendered frame, a submission, OS mapping, or presentation. Software frame/submission counts are `null`, not fabricated zero or a timer count. Software-proxy latency must not be directly ranked against GPU submission-ready latency as if the measurements were identical.

Sources: [RenderingState](https://docs.slint.dev/latest/docs/rust/slint/enum.RenderingState.html), [Window](https://docs.slint.dev/latest/docs/rust/slint/struct.Window.html). The exact pinned source inspected is `i-slint-core-1.18.1/api.rs` (`AfterRendering` contract) and `i-slint-renderer-femtovg-1.18.1/lib.rs` (submit → notifier → present ordering). Renderer initialization errors are errors, not readiness proxies.

Benchmark deadlines are absolute relative to readiness: initial idle 0–6 seconds; scrolling 6–12 seconds, one 72-pixel step every 16 ms with boundary bounce; synthetic streaming 12–18 seconds, one ` token` update every 50 ms; final idle 18–22 seconds; then `quit_event_loop`. Only scrolling/streaming have repeating timers. Idle has one-shot deadline timers and, only during initial idle, one additional bounded 750-ms geometry observation; no polling loop or animation. The geometry observer does not request redraws. Composer is disabled/read-only only in benchmark mode, preventing toolkit cursor-blink timers even if native focus traversal would otherwise focus the input; normal interactive mode accepts text. No observer-induced redraw loop exists. Actual update counts, phase elapsed times, available submission counts/interval percentiles, final idle duration, and ready-to-finish duration are emitted. A manual close stops all pending timers and quits cleanly. Timer callbacks can run late under OS/event-loop stalls; actual elapsed durations, not nominal frame rates, are reported.

## Native Wayland proof, not an environment assertion

The public `slint::set_xdg_app_id` assigns **`org.deepseek.NativeBenchmark.Slint`** before the window is shown. No URI scheme, desktop shortcut, browser callback, or protocol handler is installed. The package selects winit with only its Wayland window backend; it does not add `unstable-winit-030` because that feature also enables the generic/X11 backend.

The approved parent runner should inspect compositor client facts for **only the spawned test PID** and verify `pid`, matching `class`/app id, `mapped: true`, `xwayland: false`, and the actual `size`. On this host `hyprctl -j clients` provides those facts; retain only this test PID's fields, never unrelated window metadata. `WAYLAND_DEBUG=client` scoped to a separate verification run can additionally show `xdg_toplevel.set_app_id`, `wl_surface` creation and commits; do not use that noisy trace in timed runs. A requested environment variable alone is not runtime backend proof. Native compositor presentation timestamps and physical scanout remain **unmeasured** without a separate approved presentation-feedback method.

## Licensing is unresolved for distribution

Slint 1.18.1's exact published license expression is:

```text
GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
```

This prototype does **not** choose or accept any option on the user's behalf, and its package metadata intentionally does not purport to license a finished app. Review the complete applicable terms, eligibility, obligations, and distribution plan before selecting Slint for release. See [Slint licensing](https://slint.dev/license.html). Iced's MIT licensing and the independent backend's existing MIT license are unaffected; a toolkit choice does not silently relicense that backend.

## Evidence boundary

Initial implementation validation was build-only. The parent subsequently authorized owned, isolated graphical qualification after discovering the scale/geometry bug; [the qualification helper](qualify-scale.py) launches only this package's new test children, captures only internal synthetic PNGs, and retains only matching test-PID compositor facts. Delegated commands run in a PID namespace, so child PIDs do not necessarily match Hyprland's host PIDs: a missing exact PID match is reported as unverified and causes the helper to fail overall qualification, not silently match an unrelated window. Mesa device-init warnings also prevent a hardware-GPU claim from these sandbox runs. The approved parent runner must provide final native-host mapping/hardware and identical-screen comparison evidence. Compilation and synthetic tests alone prove none of those runtime properties. Qualification snapshots are not CPU/RSS/PSS or performance baselines. Report frontend-only process-tree measurements, not integrated-app memory, and record background load before comparing frameworks. Choose neither toolkit merely from this implementation existing.
