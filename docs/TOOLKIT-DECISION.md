# Initial native UI: Iced with GPU rendering

Select Iced 0.14.0 with wgpu for the initial Linux primary UI. No Electron, Tauri, Qt or mandatory web engine is required. This is an implementation decision for the measured workload and machine, not a universal toolkit ranking or a parity-qualified release.

## Evidence

[Qualified measurements](../benchmarks/results-comparison-03/MEASUREMENTS.md) cover all four release renderer variants at application scale 1 and 1.25. Every run exits zero and independently matches its owned compositor PID/app_id, mapped=true, xwayland=false and expected geometry. Both toolkits retain the same 1,000 synthetic 72px transcript rows, virtualize visible rows, request DejaVu Sans 14, and use the same sidebar/header/composer dimensions and bounded idle/scroll/stream phases. Internal synthetic-content snapshots were inspected separately; timing runs contain no snapshots. Toolkit builds were stopped and the next core build was held during measurement.

At scale 1, initial-idle RSS/PSS and scrolling CPU mean are:

| Renderer | RSS MiB | PSS MiB | Scrolling CPU, one core |
|---|---:|---:|---:|
| Iced wgpu | 74.48 | 36.34 | 3.92% |
| Slint FemtoVG | 96.02 | 40.38 | 10.42% |
| Iced tiny-skia | 20.43 | 13.44 | 95.05% |
| Slint software | 30.93 | 17.02 | 12.98% |

Measured settled idle CPU means are 0.00% at the sampling resolution, not proof of literally zero future CPU use. Both toolkits issued 374 synthetic scroll updates and 119 stream updates in their six-second phases. Native scroll callbacks and GPU submission callbacks are not presented frames or input latency.

Iced wgpu provides the strongest measured scrolling-CPU result and lower RSS/PSS than Slint's GPU variant. Software Iced has the smallest memory footprint but nearly saturates one core during this scrolling workload; it is a functional fallback candidate, not the performance default. Slint software is a credible lower-memory alternative, but uses more scrolling CPU than Iced GPU. Iced uses more measured idle DRM VRAM than Slint GPU (44.95 versus 35.47 MiB); this tradeoff remains explicit. Do not add RSS/PSS and VRAM blindly or confuse binary size with runtime memory.

Iced is MIT. Slint's exact available license expression remains unchosen; no proprietary license has been accepted on the user's behalf. License simplicity supports this choice but does not replace measured evidence.

## Qualification limits

This is one corrected sequential pass per renderer/scale, not a statistically repeated benchmark. Earlier comparison batches are invalid for toolkit selection: the first had scaling/layout mismatches and a final-idle labeling bug; the second was cancelled when visual qualification found a Slint layout defect. Raw samples remain unchanged, and analysis derives phase ranges from event timestamps. Repeat representative measurements and interactive input/scroll tests before release.

The frontend has no Harness/Node process in these measurements. The [real isolated backend smoke](../smoke-home/fork-backend-hS3o50/result.json) separately passes 12 checks, but integrated process-tree performance remains unmeasured. Native account mutation, real model generation, terminal/file/document panels, accessibility, plugin compatibility, install/update and end-to-end feature parity are separate unfinished gates.

Startup labels differ: Iced first-redraw event plus own-window handle query; Slint GPU command-submission callback before swap; Slint software event-loop-after-show proxy. Do not present the reported milliseconds as equivalent first-pixel latency. There is no hardware-completion/presentation/input-latency measurement. Application-controlled 1.25 scaling is verified; genuine compositor fractional scaling is not. Monitor settings, refresh, VRR, theme, global fonts, icons and bindings were not changed.
