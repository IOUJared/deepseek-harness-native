# Initial native UI: Iced with GPU rendering

- **Choice:** Iced 0.14.0/wgpu for the initial Linux UI; no mandatory Electron, Tauri, Qt or web engine.
- This is a measured-workload decision, **not universal toolkit superiority or desktop parity**.

## Evidence

[Qualified measurements](<../benchmarks/results-comparison-03/MEASUREMENTS.md>): release builds, application scales 1/1.25, owned compositor PID/app_id, mapped native Wayland, matching geometry. Both toolkits virtualized 1,000 synthetic 72px rows with DejaVu Sans 14 and equivalent layout/phases; timing excluded snapshots and concurrent builds.

| Scale 1 renderer | RSS MiB | PSS MiB | Scroll CPU, one core |
|---|---:|---:|---:|
| Iced wgpu | 74.48 | 36.34 | 3.92% |
| Slint FemtoVG | 96.02 | 40.38 | 10.42% |
| Iced tiny-skia | 20.43 | 13.44 | 95.05% |
| Slint software | 30.93 | 17.02 | 12.98% |

- Settled idle means: 0.00% **at sampling resolution**. Each six-second phase issued 374 scroll/119 stream updates, not presented frames.
- Iced GPU used less scroll CPU/RSS/PSS than Slint GPU, but more idle DRM VRAM: **44.95 versus 35.47 MiB**. Do not blindly add VRAM and RSS/PSS.
- Iced is MIT; Slint's license expression remains unchosen. No proprietary license was accepted.

## Qualification limits

- One corrected sequential pass per renderer/scale; repeat before release. Earlier batches had scaling/layout/idle-label defects or were cancelled.
- Startup proxies differ: Iced redraw/handle, Slint GPU submission-before-swap, Slint software after-show. They are **not equivalent first-pixel, presentation or input latency**.
- Application-controlled 1.25 scale is verified; compositor fractional scaling is not.
- No Node/Host in toolkit measurements. Separate backend smoke passed 12 checks; its ignored evidence is local-only, not a GitHub download. Integrated process-tree performance is unmeasured.
- Models, account mutation, rich panels, accessibility, plugins, installation/update and parity remain separate gates. Desktop settings were unchanged.
