# Slint validation

Historical checks: 2026-10-06, Rust 1.99. Synthetic geometry qualification—not performance or integrated-app evidence. MSRV 1.92 was not separately tested.

## Results

| Check | Result |
|---|---|
| Fetch/check, formatting, release build | Passed |
| Headless tests | 13 passed |
| Internal scale-1.25 snapshots | Correct layout; directly inspected |
| App exit | 0 for both renderers |
| Exact host-PID compositor match | Failed; helper exited 1 |
| Hardware acceleration/presentation | Unverified |

## Corrections

- Explicit logical size before `show()` prevents preferred-size overwrite.
- Explicit pane origins prevent sidebar/header overlap.
- One geometry query at ready+750 ms; no redraw, repeating timer or schedule change.

| Geometry at app scale 1.25 | Observed |
|---|---|
| Logical / physical size | 1200×800 / 1500×1000 |
| Sidebar XYWH | 0, 0, 260, 800 |
| Header XYWH | 260, 0, 940, 48 |
| Transcript XYWH | 260, 48, 940, 648 |
| Composer XYWH | 260, 696, 940, 104 |
| Updates | 374 scroll; 119 stream |
| Ready-to-finish | ~22 seconds; scheduling only |

## Artifact and limits

- Historical executable: 15,152,656 bytes; x86-64 stripped ELF PIE.
- SHA-256: `946e88fb9bad6e85a644a346a54d5a4eee38a974fbbb3e8ebd91b2efb14e9c86`.
- Raw reports/PNGs remain local, not published GitHub downloads.
- PID-namespace mismatch prevented native host mapping proof; Mesa warnings prevented a hardware-GPU claim.
- Reject older 960×640 logical runs and pre-layout-fix snapshots.
- Still unverified: physical input, presentation, clean performance comparisons and full-app memory.
- [Renderer semantics and licensing](<README.md>); no Slint license option accepted for the user.
