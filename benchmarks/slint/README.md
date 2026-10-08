# Slint benchmark

Synthetic frontend—not the Harness app. No Node, models, accounts, network or profile writes. Uses the [matched workload](<../WORKLOAD.md>).

## Build

- Slint `=1.18.1`; MSRV 1.92; tested with Rust 1.99.
- Defaults disabled: `std`, `backend-winit-wayland`, `renderer-software`, `renderer-femtovg`, `compat-1-18`.
- No Qt, Skia, wgpu or X11 backend. Limited image/SVG codecs compile transitively.
- Release: opt-level 3, thin LTO, one codegen unit, stripped.

```sh
cargo test --locked -j2
cargo build --locked --release -j2
```

## Screen and CLI

- 1200×800 logical units; Rosé Pine, DejaVu Sans 14, 72-pixel rows.
- Retain 1,000 records; project visible rows plus two-row overscan per side.
- Send/Stop affect synthetic data only; settings are not persisted.
- Flags: `--bench`, `--scale`, `--renderer software|femtovg`, `--snapshot`, `--help`.
- Explicit logical size fixes the old scale bug: at native scale 1, app scale 1.25 requests 1500×1000 pixels. Verify actual geometry.

## Readiness

| Renderer | `ready` means |
|---|---|
| FemtoVG/OpenGL | Render submission complete, before swap—not GPU completion |
| Software | One-shot event-loop proxy after show—not rendering or mapping |

- Do not rank these different proxies as identical startup measurements.
- `settled_geometry` queries dimensions/panes once at ready+750 ms; no redraw or schedule reset.
- `DSH_BENCH:` events follow the shared 6/6/6/4-second schedule. Software submission counts stay null.
- Only scroll/stream phases repeat timers; idle has no application animation loop.

## Graphical safety

- Use an approved owned-PID runner and isolated XDG homes. Do not alter desktop/display settings.
- Verify mapped PID, app ID `org.deepseek.NativeBenchmark.Slint`, native Wayland and actual size.
- `--snapshot` accepts only a new absolute in-package PNG; symlinks, traversal and overwrite are refused.
- Internal snapshots capture synthetic content only. Run separately from performance baselines.
- [Validation](<VALIDATION.md>) distinguishes corrected geometry from unverified host mapping and hardware acceleration.

## Distribution

Slint license expression:

```text
GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
```

No option is accepted for the user. Review [licensing](https://slint.dev/license.html) before distribution; backend MIT remains unchanged.
