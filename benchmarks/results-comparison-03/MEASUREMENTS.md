# Qualified native prototype measurements

Frontend prototypes only: no Harness/Node Host process. CPU is percent of one logical core; memory values are phase medians in MiB. Startup columns are different proxies, not presented frames or a fair first-frame ranking.

| Variant | UI scale | Idle RSS | Idle PSS | Idle CPU mean | Scroll CPU mean | Stream CPU mean | Final idle CPU mean | Idle DRM VRAM | Startup proxy ms | Qualified |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| iced-gpu | 1.00 | 74.48 | 36.34 | 0.00 | 3.92 | 0.98 | 0.00 | 44.95 | 113.64 | yes |
| iced-gpu | 1.25 | 74.26 | 36.16 | 0.00 | 3.93 | 0.98 | 0.00 | 47.95 | 115.42 | yes |
| slint-gpu | 1.00 | 96.02 | 40.38 | 0.00 | 10.42 | 1.77 | 0.00 | 35.47 | 110.31 | yes |
| slint-gpu | 1.25 | 96.18 | 40.58 | 0.00 | 10.61 | 1.57 | 0.00 | 38.34 | 110.61 | yes |
| iced-software | 1.00 | 20.43 | 13.44 | 0.00 | 95.05 | 3.15 | 0.00 | unknown | 45.99 | yes |
| iced-software | 1.25 | 25.11 | 16.09 | 0.00 | 96.56 | 4.33 | 0.00 | unknown | 47.87 | yes |
| slint-software | 1.00 | 30.93 | 17.02 | 0.00 | 12.98 | 0.79 | 0.00 | unknown | 30.84 | yes |
| slint-software | 1.25 | 35.14 | 19.21 | 0.00 | 14.16 | 0.98 | 0.00 | unknown | 32.71 | yes |

All qualifying runs independently match their owned PID/app_id, mapped=true, xwayland=false and expected compositor geometry. Application scaling 1.25 is not compositor fractional-scaling qualification. No hardware-completion, presentation-latency or input-latency measurements were taken.

Startup definitions: Iced first-redraw event plus own-window handle query; Slint GPU render-command submission callback before swap; Slint software event-loop-after-show proxy. Raw events and samples remain in sibling results.json; geometry and phase summaries are in summary.json.
