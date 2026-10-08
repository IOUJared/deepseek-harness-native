# Matched benchmark workload

Synthetic Slint 1.18.1 / Iced 0.14.0 frontends. No Host, credentials, models or normal user data.

## Common screen

| Item | Setting |
|---|---|
| Window | 1200×800 logical units |
| Sidebar / header / composer | 260 / 48 / 104 pixels |
| Data | Three workspaces, 12 sessions, 1,000 rows |
| Rows | 72 pixels including padding/borders; visible slice plus overscan |
| Font / theme | DejaVu Sans 14 / dark Rosé Pine |

- Retain full strings. Send/Stop affect synthetic content only.
- Row role: `You` when `i % 3 == 0`, otherwise `Assistant`.
- Body: `Message {i}: Review the workspace and keep changes small and tested.`
- Footer: `Tool result: completed` when `i % 5 == 0`; otherwise `Synthetic benchmark transcript`.
- Palette: base `#191724`, surface `#1f1d2e`, overlay `#26233a`, text `#e0def4`, muted `#6e6a86`, subtle `#908caa`, accent `#c4a7e7`, border `#403d52`.

## Execution and protocol

| Seconds after `ready` | Phase |
|---|---|
| 0–6 | Idle |
| 6–12 | Scroll 72 pixels every 16 ms; bounce at bounds |
| 12–18 | Append ` token` to last visible row every 50 ms |
| 18–22 | Idle, then clean exit |

- Same release settings: opt-level 3, thin LTO, one codegen unit, stripped; pinned versions and explicit renderers.
- JSON prefix `DSH_BENCH:`; fields `framework`, `event`, `elapsed_ms`.
- Events: initialized, ready, idle_start, scroll_start/end, stream_start/end, finished; one settled_geometry at ready+750 ms.
- Deadlines are absolute. No repeated idle timers or observer-driven redraw loop; manual close cancels work.
- Startup proxies, render submissions and compositor presentation are distinct. Report actual counts/durations, never fabricated frames.
- Runner measures owned process-tree RSS/PSS, CPU, available DRM VRAM, mapping, load and binary size.
- Exclude first 1.5 s of initial settling and 750 ms at other phase boundaries. CPU uses one logical core; unknown PSS/VRAM stays null.

## Wayland, scaling, safety and evidence

- Match fonts, content, geometry, duration, scale and load across software/GPU variants.
- Verify owned PID/native Wayland, not environment alone. App scale 1.25 at native scale 1 must yield 1500×1000 pixels.
- Application scaling is not compositor fractional-scale qualification.
- Approved runners use isolated XDG homes; preserve monitors, VRR, fonts, focus rules and desktop settings.
- Stop only owned processes. Capture only internal synthetic pixels, separately from timed runs.
- Frontend results are not integrated-app memory or universal rankings. Slint distribution licensing needs review; Iced is MIT.
