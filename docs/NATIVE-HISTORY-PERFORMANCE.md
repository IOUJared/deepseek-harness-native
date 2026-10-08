# Native large-history reflow performance

## Measured result

Two-width geometry caching improved synthetic full-retention reflow **71.55→3.86ms median (~18.5×)**, issue→matching widget receipt—not presentation, wheel latency, toolkit ranking or backend performance.

| Rows, initial→final | Baseline median | Cached median | Samples/build |
|---|---:|---:|---:|
| 80→92 | 1.755ms | 0.493ms | 12 |
| 4084→4096 | 71.545ms | 3.864ms | 12 |

First unseen narrow width remained expensive: **71.34/67.11ms**. Traversal remains O(N). Large-run paragraph constructions **32688→8183**; nested construction time **521–525→122–125ms**, not additive to update/view timings.

## Equivalent workload and retained proof

- [Actual-App rig](<../app/examples/scroll_app_smoke.rs>): PUBLIC synthetic rows, no worker/Host/Node/credentials/business executor; physical input shielded.
- Two fresh processes/build/size: reader key112 or mid-history key2142/intra12; seven **1040×800↔650×800** reflows; twelve-row prepend; decision hide/Cancel remount; exact receipt/native top −12px, following=false, zero commands. Idle **2000ms after receipts**.
- Eight measured processes passed. Ignored [paired comparison](<../app/evidence/native-history-bounded-lanes-4th1bizc/comparison.json>) / [paint comparison](<../app/evidence/native-history-paint-067f1di5/qualification.json>) are **local-only**: exact native/model geometry parity and byte-identical inspected renderer PNGs. Source pins stable, not full dependency/compiler closure.

## Memory and idle

| Large runs | RSS MiB | PSS MiB |
|---|---:|---:|
| Optimized | 94.59–95.35 | 55.84–56.19 |
| Baseline | 94.41–95.29 | 55.67–56.22 |

Owned PID/thread accounting observed no direct children/missing samples, not adversarial containment. Idle: **0% at resolution**, ~1.95s/39 samples, 100 ticks/s; no VRAM/backend measurement. Fresh XDG, not cleared global font/GPU/OS caches. Two repeats are limited evidence.

## Implementation and instrumentation

- [Cache](<../app/src/chat_geometry.rs>): one role/preview, two scalar geometries, no retained Paragraphs; LRU third-width eviction, role/text invalidation, **4097 keys** maximum. Chat caps **640px user/800px assistant**; tool/opaque keys retain lane width.
- Feature-only counters/rig. View timing is Element construction, not layout/GPU. Receipt timestamp precedes diagnostics; fsync/observer work remains in wall latency.
- Added tests: six cache/five rig/four CLI/four sampler. Historical default **436**, scroll **447** tests include concurrent work.

## Remaining gates

Mixed/Unicode/live/continuous-width workloads, physical input, slot census, accessibility, presentation/FPS, window/compositor scaling and full Host process-tree performance remain unqualified. No automatic archive refresh or universal performance claim.
