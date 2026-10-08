# Iced native frontend benchmark prototype

Standalone Rust UI, **not the integrated Harness application**. All sessions/transcript content and token updates are fake. No Electron, browser/webview, models, account access, application networking, user-profile/state persistence, desktop configuration changes, or subprocesses. Backend integration is forthcoming. The authoritative [matched workload](</home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-native/benchmarks/WORKLOAD.md>) applies.

## Versions, features, and renderer variants

[Cargo manifest](</home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-native/benchmarks/iced/Cargo.toml>) pins Iced **=0.14.0**, edition 2024, MSRV **1.88**. This environment has Rust 1.99.0. The [crates.io 0.14.0 record](https://crates.io/api/v1/crates/iced/0.14.0) was queried directly, including its current complete feature list and `rust_version`.

Framework defaults are disabled. Explicit features: `tiny-skia`, `wayland`, `basic-shaping`, `tokio`. No `x11`, framework `debug`, `unconditional-rendering`, desktop-theme detection, Qt, or Skia renderer feature. `tiny-skia` is the independent small Rust rasterizer, **not Google Skia**. `tokio` supplies sleeping timers; idle does not use a repeating subscription. Cargo feature **`gpu = ["iced/wgpu"]`** adds wgpu only for the second binary; the software build does not reference wgpu-typed values/futures.

- Base binary: tiny-skia only.
- GPU binary: wgpu code compiled; startup sets `ICED_BACKEND=wgpu` before starting any threads. Iced's dual-renderer implementation attempts only this explicit backend candidate, and tiny-skia rejects `wgpu`. GPU initialization failure therefore fails rather than silently benchmarking software under a GPU label.
- Base startup likewise sets `ICED_BACKEND=tiny-skia`. Selection does not depend on inherited renderer variables.

This behavior was checked against actual [fallback compositor](https://github.com/iced-rs/iced/blob/0.14.0/renderer/src/fallback.rs), [tiny-skia compositor](https://github.com/iced-rs/iced/blob/0.14.0/tiny_skia/src/window/compositor.rs), and [wgpu compositor](https://github.com/iced-rs/iced/blob/0.14.0/wgpu/src/window/compositor.rs) implementations. GPU still can select a CPU/software Vulkan adapter; the approved parent runner must record adapter/driver environment instead of claiming hardware acceleration from the binary name alone.

Both release variants use identical `opt-level=3`, thin LTO, `codegen-units=1`, and symbol stripping. The [lockfile](</home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-native/benchmarks/iced/Cargo.lock>) freezes dependencies including Iced component patch releases.

## Build and tests (no GUI launch)

Run from this package directory. Only its own target/output directories are used; the shared absolute Cargo cache is intentional.

```sh
export CARGO_HOME=/home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-tauri/.cargo-home
export CARGO_BUILD_JOBS=3
cargo check --locked
cargo test --locked
cargo build --release --locked
# Copy target/release/dsh-iced-bench to build-output/iced-tiny-skia;
# verify resolved destination is inside this package before copying.
cargo check --locked --features gpu
cargo test --locked --features gpu
cargo build --release --locked --features gpu
# Copy target/release/dsh-iced-bench to build-output/iced-wgpu.
```

[Model and tests](</home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-native/benchmarks/iced/src/model.rs>) cover deterministic 1,000-row data, partial-visible-row boundaries and three-row overscan, constant total spacer+row height across scroll positions/resizes, pinned geometry, boundary bouncing, SendSynthetic, StopSynthetic, and stale streaming generation rejection after cancellation/session selection. Tests open no windows.

### Completed build evidence

Both renderer variants passed `cargo check`, **7 model tests each**, and release build. Both non-GUI `--help` exits passed. Final artifacts are stripped x86-64 ELF PIE executables:

- [tiny-skia executable](</home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-native/benchmarks/iced/build-output/iced-tiny-skia>): **6,441,160 bytes**.
- [wgpu executable](</home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-native/benchmarks/iced/build-output/iced-wgpu>): **11,422,088 bytes**.

Exact hashes, build environment, and limitations are in [machine-readable build evidence](</home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-native/benchmarks/iced/build-output/build-evidence.json>). The [geometry-corrected final validation log](</home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-native/benchmarks/iced/build-output/geometry-validation.log>) records formatting checks, checks, tests, builds, validated copies, help, and hashes. The [software dependency tree](</home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-native/benchmarks/iced/build-output/software-dependency-tree.txt>) contains no wgpu, X11, Qt, or Google Skia; the [GPU dependency tree](</home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-native/benchmarks/iced/build-output/gpu-dependency-tree.txt>) shows the added wgpu stack.

## UI and virtualization

[UI implementation](</home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-native/benchmarks/iced/src/main.rs>) uses the actual Iced 0.14 `application(boot, update, view)` builder, `.title`, `.theme`, `.scale_factor`, `.subscription`, `.window`, and `.run` APIs. API reference: [application builder source](https://github.com/iced-rs/iced/blob/0.14.0/src/application.rs).

Initial **UI logical size 1200×800**, sidebar 260, header 48, composer 104, transcript viewport 648. Interactive mode is resizable. The application supplies **base `size`, `min_size`, and `max_size` of 1200×800**, without multiplying by `--scale` itself. Benchmark mode disables resize to discourage compositor tiling from changing the matched size; `ready.logical_size` and the later `settled_geometry.logical_size` report actual dimensions, never assume the request was honored.

Actual upstream conversion was inspected: [iced_winit 0.14.1 window conversion](https://docs.rs/iced_winit/0.14.1/src/iced_winit/conversion.rs.html) multiplies initial `settings.size` by the application scale, but forwards explicit minimum/maximum dimensions unscaled. [Winit 0.30.13 Wayland window creation](https://docs.rs/winit/0.30.13/src/winit/platform_impl/linux/wayland/window/mod.rs.html) installs those bounds before `set_resizable(false)`, whose [implementation](https://docs.rs/winit/0.30.13/src/winit/platform_impl/linux/wayland/window/state.rs.html) replaces both bounds with the initial, already-scaled inner size. Thus `--scale 1.25` with native scale 1 requests/pins a **1500×1000** native window while preserving **1200×800 UI logical units**, subject to actual compositor allocation. The earlier double-multiplied request was corrected after the parent's measured geometry exposed it; prior 1.25 results are not matched-geometry comparisons.

Dark Rosé Pine explicitly supplies all workload colors: base `#191724`, surface `#1f1d2e`, overlay `#26233a`, text `#e0def4`, muted `#6e6a86`, subtle `#908caa`, accent `#c4a7e7`, border `#403d52`. Explicit **DejaVu Sans**, 14 UI logical pixels, matching Slint; 18-pixel absolute line height keeps three row text lines inside the pinned 72-pixel row. Font fallback is not introspected; `initialized` honestly reports that limitation, and parent environment/font evidence should establish availability. This prototype does not read desktop theme settings.

All 1,000 message records stay resident. Each `view` constructs **only the visible data slice plus three rows of overscan on each side**, and two `Space` widgets for the skipped top/bottom extent. At a 648-pixel viewport there are initially 12 row widgets, normally at most 16 (including a partially visible row); never a 1,000-row widget tree. Text is no-wrap and clipped inside a fixed 72-pixel row, including padding/border. Streaming grows the data string without changing row height.

The native Iced `Scrollable` retains its own offset and scrollbar. User input produces `on_scroll(Viewport)` callbacks. Programmatic bench scroll invokes **`iced::widget::operation::scroll_to("transcript", scrollable::AbsoluteOffset { x: 0.0, y })`**; Iced 0.14 moved this task API out of the older `widget::scrollable::scroll_to` location. The slice is updated with the requested offset and then reconciled to the actual native viewport callback. Exact source: [runtime widget operations](https://github.com/iced-rs/iced/blob/0.14.0/runtime/src/widget/operation.rs), [scrollable viewport](https://github.com/iced-rs/iced/blob/0.14.0/widget/src/scrollable.rs). It is not a custom fake scroll animation.

Selecting one of twelve sessions/workspaces changes FakeSession selection and cancels synthetic streaming. All fake selections share the same generated transcript storage, rather than allocating twelve 1,000-row transcripts. Send appends a fake user row and fake assistant row, starts 50ms fake tokens, and scrolls to the end. Stop only cancels synthetic tokens. Settings toggles an in-prototype label; no settings backend is claimed.

## CLI and exact timing/event protocol

```sh
build-output/iced-tiny-skia --help
build-output/iced-tiny-skia --bench
build-output/iced-wgpu --bench --scale 1.25
# Explicit optional internal output, never a desktop/window-manager capture:
build-output/iced-tiny-skia --bench --screenshot ./build-output/own-fake-content.png
```

**Do not launch these commands outside the approved parent's graphical runner.** Without `--bench`, the interactive prototype stays open. `--scale NUMBER` (finite 0.5–3, default 1) is an application-controlled UI scale multiplier, **not a compositor configuration change or proof of compositor fractional scaling**.

Every event is one newline JSON object, prefixed `DSH_BENCH:`, with `framework: "iced"`, `event`, and `elapsed_ms` measured from entry to main. Events:

1. `initialized`: model allocation complete; renderer variant, 1,000 rows, requested font/fallback caveat.
2. `ready`: **`measurement: "first_redraw_event_proxy_plus_window_handle_query_not_presented_frame"`**. A startup-only `event::listen_raw` filters the first own-window `RedrawRequested`, then is immediately removed. The app subsequently queries its own native raw display handle, native scale, and Iced UI logical size; `ready` waits for both the redraw-event proxy and those query responses. This is neither a compositor-mapped-window claim nor a presented-frame claim. It can precede hardware/compositor presentation. No frame interval measurement is claimed.
3. `idle_start`: initial idle, 6,000ms.
4. `scroll_start` at ready+6s: 72 UI logical pixels every 16ms, bouncing at transcript bounds.
5. `scroll_end` at ready+12s: elapsed scroll phase, requested scroll operation count, actual native viewport callback count, offset, peak row widget count. These are **not frame counts**.
6. `stream_start` immediately after scroll end: update last actually visible row by the exact ` token` fragment every 50ms.
7. `stream_end` at ready+18s: phase elapsed time, actual update count and target row; stop streaming and invalidate queued ticks.
8. `idle_start`: final idle, 4,000ms.
9. `finished` at ready+22s: phase elapsed time, counts, retained row count, clean `iced::exit()`.

A single additional `settled_geometry` event is emitted approximately **ready+750ms** after a bounded one-shot delay and fresh own-window queries. Its `measurement` is `settled_window_geometry_not_presentation`; fields include `renderer`, `app_id`, `window_backend` verified by raw display handle, `logical_size`, `native_window_scale`, `application_scale`, `effective_scale`, and `physical_size`. Iced's public size query returns the actual current UI logical viewport. `physical_size` is therefore explicitly labeled **`derived_from_queried_iced_viewport_and_effective_scale_not_os_geometry_query`**, rounded from that viewport × effective scale; the parent's own-PID compositor geometry remains the independent OS measurement. This event never resets a phase, repeats, writes an image, or claims a presented frame. It causes one bounded update during initial idle, so exclude the startup settling window from steady-idle sampling.

Four **one-shot absolute-deadline phase tasks** are armed relative to ready, independently of the geometry task. Repeating 16ms/50ms subscriptions exist only during their active phases. Neither idle phase subscribes to frames, schedules repeating application timers, nor feeds every redraw back into an update. The native Scrollable's change-guarded viewport notification can cause an initial layout update, not a perpetual feedback redraw loop. Manual close cancels streaming and exits, dropping tasks/subscriptions. Normal window activity and the native text-input cursor may still generate toolkit-initiated events; idle means no application-driven animation loop, not a promise of zero toolkit CPU.

Timer delivery depends on event-loop scheduling; delayed deadlines are reported, not hidden by claiming exact physical frame counts. As the schedule is absolute, expensive work cannot extend the total intentionally beyond 22s. The parent runner remains responsible for OS spawn-to-ready/map evidence and process CPU/RSS/PSS.

## Native Wayland and safe evidence

The public Linux `window::Settings.platform_specific.application_id` is set to **`org.deepseek.harness.bench.iced.tinyskia`** or **`org.deepseek.harness.bench.iced.wgpu`**. Iced maps that setting to Winit's Wayland `with_name`, and Winit sets the own-window xdg-toplevel app id. It is reported as `ready.application_id` and `settled_geometry.app_id`; the parent should independently verify the mapped child's class/id. No desktop file or launcher registration is installed.

The approved runner should remove `DISPLAY`, retain `WAYLAND_DISPLAY`, and use isolated XDG homes in the child environment only. It should not configure displays or the compositor. This app verifies its **actual own-window display handle** using `iced::window::run`: `RawDisplayHandle::Wayland` produces `ready.window_backend: "wayland"`, Xlib/Xcb produces `x11`; other/error is labeled honestly. Actual source: [window run/scale/size/screenshot API](https://github.com/iced-rs/iced/blob/0.14.0/runtime/src/window.rs). Native scale from `window::scale_factor` is the underlying winit scale; `effective_scale` is native scale × application scale. `logical_size` is from `window::size`, the Iced layout viewport. Any parent compositor check must be limited to the test child PID/window. No GUI has been launched by this implementation agent. The non-GUI `--help` exits were checked for each variant; invalid scale (`nan`, `0`), unknown options, and screenshot paths outside this package were verified to reject before GUI boot.

`--screenshot` is optional and captures only the internally rendered own synthetic window via `iced::window::screenshot`; never other windows or the desktop. Output must be an explicitly named `.png` inside this package directory with an existing canonical parent. It uses `create_new`, refusing existing files and symlinks. No screenshot/state/output writes occur without this flag. Screenshot adds an explicitly labeled `screenshot` protocol event and may disturb startup measurements; use a separate evidence run, not the primary memory/performance run. The source screenshot API returns own-renderer RGBA bytes, not a screenshot-service capture.

Licensing: Iced is MIT. This benchmark's own package is MIT; dependency licenses and distribution obligations remain a separate release review. No full-app feature parity, integrated-app memory, hardware presentation, or performance superiority is claimed.
