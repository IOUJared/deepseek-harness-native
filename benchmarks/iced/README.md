# Iced benchmark

Synthetic frontend—not the Harness app. No Host, models, accounts, network or profile writes. Uses the [matched workload](<../WORKLOAD.md>).

## Builds

| Variant | Renderer | Artifact |
|---|---|---|
| Base | tiny-skia | 6,441,160 bytes |
| `gpu` | wgpu | 11,422,088 bytes |

- Iced `=0.14.0`; MSRV 1.88; tested with Rust 1.99.
- Defaults disabled; Wayland, basic shaping and Tokio enabled. No X11, Qt or Google Skia.
- Explicit `ICED_BACKEND` prevents renderer fallback; a GPU label alone does not establish hardware acceleration.
- Both builds: opt-level 3, thin LTO, one codegen unit, stripped.

```sh
cargo test --locked -j2
cargo build --locked --release -j2
cargo test --locked -j2 --features gpu
cargo build --locked --release -j2 --features gpu
```

Seven model tests passed per variant. Artifacts and raw logs stay local.

## Screen

- 1200×800 logical units; Rosé Pine, DejaVu Sans 14, 72-pixel rows.
- Retain all 1,000 records; construct visible rows plus three-row overscan per side.
- Scroll uses native `Scrollable`; Send/Stop affect synthetic data only.
- App scale 1.25 requests 1500×1000 physical pixels at native scale 1. Verify actual geometry; older double-scaled runs are invalid.

## Timing and safety

| Event | Meaning |
|---|---|
| `ready` | First redraw-event proxy plus native-window queries—not presentation |
| `settled_geometry` | One query at ready+750 ms; physical size is derived |
| Phase events | [Shared 6/6/6/4-second schedule](<../WORKLOAD.md#execution-and-protocol>) |

- JSON prefix: `DSH_BENCH:`. Counts are operations/callbacks, not displayed frames.
- No repeating application timer during idle; normal toolkit events may still occur.
- Wayland IDs: `org.deepseek.harness.bench.iced.tinyskia` / `.wgpu`.
- Graphical execution requires an approved owned-PID runner; preserve desktop settings and isolate XDG homes.
- `--screenshot` captures only internal synthetic pixels to a new in-package PNG. Separate capture runs from performance baselines.
- MIT; dependency redistribution review remains separate. No full-app or universal performance claim.
