# Native runtime performance

Measured on the released `2026.10.10-r2` binary, 2026-10-10, America/Chicago. Local ignored evidence, not a certificate, toolkit ranking or public download.

**Host:** Ryzen 7 7800X3D (8 logical CPUs, SMT inactive), Radeon RX 7900 XTX 24 GB, Hyprland 0.56.2 on Wayland, LG 2560×1440 ~540 Hz + ASUS 2560×1440 ~240 Hz, glibc 2.44, Node 26.11.1, Python 3.14.8.

**Build:** release, `opt-level = 3`, thin LTO, one codegen unit, stripped; executable SHA-256 `4ecec524d097a40bf6394178dc906ae6aec6aaa707ab561c26aa4229155b4733` (18,355,584 bytes). Integrated runs use the isolated alpha backend `0.2.1-alpha.1`; observation covers the owned tree only (app + its Node Host, 2 processes). The machine's own 1-minute load average was **5.3–5.8** during these runs, so small CPU differences are not attributable to the app alone.

## Startup

Three fresh extracted-package runs (`firstOwnWindowMappedUpperBoundMs`), scale 1.0:

| Run | Window mapped, upper bound |
|---|---:|
| 1 | 324.5 ms |
| 2 | 828.6 ms (retained outlier) |
| 3 | 325.4 ms |

Median **325.4 ms**. This is a window-probe upper bound, **not** presented-frame or backend-ready latency; no Host-ready timestamp is instrumented, and run 2 shows that a single sample can be several times slower on a busy session.

## Integrated idle

Same three 8-second blank-session runs (identity-checked warm window 3.28 s, one-tick resolution 0.305% of one core):

| Metric | Run 1 | Run 2 | Run 3 |
|---|---:|---:|---:|
| Warm RSS | 337.8 MiB | 337.0 MiB | 338.4 MiB |
| Warm PSS | 263.3 MiB | 265.0 MiB | 266.5 MiB |
| Endpoint CPU, % one core | 1.22% | 0.00% | 0.00% |
| Owned processes | 2 | 2 | 2 |

Longer window, same binary, three fresh 60-second blank sessions (`tools/xtask idle`):

| Metric (median of 3) | Value |
|---|---:|
| CPU, % of one core | **0.47%** (range 0.32–0.53%) |
| Warm RSS | **336.1 MiB** |
| Warm PSS | **264.3 MiB** |
| Peak RSS | 336.3 MiB |
| Identity stability | stable in 3/3 runs |

CPU is the endpoint delta over the warm span; individual samples are frequently exactly zero at 10 Hz. Sub-1% differences are below this rig's resolution and are not a regression signal.

## Large-history scroll and reflow

`app/measure_native_history.py` (actual App widget tree, PUBLIC synthetic rows, no Host, physical input shielded), 3 runs per size, 4 cycles, 2 s idle:

| Rows | First reflow (unseen width) | Later reflows, median | Later reflows, max | Whole run wall |
|---|---:|---:|---:|---:|
| 80 | 2.88 ms | 0.66 ms | 1.07 ms | 2.15–2.20 s |
| 4,084 | 68.29 ms | 5.54 ms | 8.95 ms | 2.41 s |

| Rows | Peak RSS | Peak PSS | Idle CPU (1 Hz, ~1.95 s) | Slowest sample read |
|---|---:|---:|---:|---:|
| 80 | 78.2–78.6 MiB | 37.8–38.3 MiB | 0.0% | 1.34 ms |
| 4,084 | 96.8–97.1 MiB | 56.3–56.5 MiB | 0.0% | 1.61 ms |

Reflow latency is issue→matching-widget receipt, not paint, wheel latency or frame presentation. Earlier work on the same rig measured a 71.55→3.864 ms median improvement for the 4,084→4,096-row path and a 67.11–71.34 ms first unseen width ([history performance](<NATIVE-HISTORY-PERFORMANCE.md>)); the 68.29 ms first reflow and 5.54 ms later median above are consistent with that protocol, not a new comparison.

## What is deliberately not measured

- **Input latency**: wheel/keyboard→frame latency is not instrumented, and physical-input qualification remains separate. This is the largest remaining performance gap.
- **Presented-frame latency and GPU cost**: the sampler observes `drm_vram_kib` (72,784–74,172 KiB reported here) but it is not validated as VRAM accounting; no GPU utilization, frame-time or presentation timestamps were collected.
- **Live-host long chat**: streaming model output, provider latency, tool execution, and long sessions driven through the Host are not part of any number above.
- **Sustained and comparative scope**: `n=3` per configuration on one machine and compositor; no X11, other GPUs, other compositors, multi-hour idle, fractional scaling, or toolkit-superiority claim.

## Reproduce

```sh
cargo run --quiet --manifest-path tools/xtask/Cargo.toml --locked -- qualify development-package --package /ABS/extracted-r2 --output /ABS/evidence/q1 --scale 1.0   # x3
python3 -B scripts/measure-native-idle.py --seconds 60 --repeats 3 --output /ABS/evidence/idle
python3 -B app/measure_native_history.py --label r2 --repeats 3 --cycles 4 --idle-ms 2000
python3 -B -m unittest discover -s scripts -p 'test_measure_native_idle.py'
```

Local evidence: `app/evidence/r2-perf-qualify-{1,2,3}/result.json`, `app/evidence/r2-idle-60s/summary.json`, `app/evidence/native-history-r2-2026-10-10-r_t2_3g_/qualification.json`. No desktop setting, profile or installed runtime was changed.
