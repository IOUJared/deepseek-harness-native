# Native large-history reflow performance

## Measured result

The native production geometry cache now retains two recently used measurement widths per row. In the bounded PUBLIC native workload, repeated full-retention two-width reflow improved from **71.55ms to3.86ms median** issue→matching widget-operation receipt, approximately18.5×. This is **not** presented-frame time, physical wheel latency, a Slint comparison, or whole backend performance.

| Initial→final retained rows | Baseline subsequent reflow median | Two-width cache median | Samples |
| --- | ---: | ---: | ---: |
| 80→92 | 1.755ms | 0.493ms |12 per build |
| 4084→4096 | 71.545ms | 3.864ms |12 per build |

The first unseen narrow width remains expensive: large-history first-narrow medians71.34ms baseline /67.11ms optimized. Full projection traversal remainsO(N); this does not solve arbitrary continuous resize widths or live generation. Native paragraph construction counts dropped32688→8183 per large run, with nested construction time roughly521–525ms→122–125ms. These nested durations must not be added to update/view timings.

## Equivalent workload and retained proof

The [extended actual-App driver](<../app/examples/scroll_app_smoke.rs>) runs the real App view/update/receive and direct widget operations, using the [PUBLIC rig](<../app/src/scroll_fixture.rs>) without worker subscriptions, Host, Node, credentials or any business executor. The installed runtime/profile/desktop configuration is untouched. Its physical-input shield remains active; controller events, not physical pointer input, establish the reader position.

Each release executable ran two fresh processes at each initial size80/4084. Each process:

- Establishes key112/intra12 for80 rows, or mid-history key2142/index2042/intra12 for4084 rows100–4183.
- Observes seven real controlled-frame reflows1040×800↔650×800, ending narrow.
- Prepends twelve PUBLIC older rows88–99, reaching92/4096 without retention error.
- Hides the actual transcript for a PUBLIC decision, then restores through worker-shaped Cancel/remount.
- Requires one exact current receipt for each active operation, a fresh ID, real native slot top−12px, matching native/model offsets, followingfalse and zero commands/effects.
- Holds idle for2000ms **after all operations are observed**, not as a substitute for restoration completion.

The [baseline qualification](<../app/evidence/native-history-baseline-b16y6y2w/qualification.json>) and [optimized qualification](<../app/evidence/native-history-bounded-lanes-4th1bizc/qualification.json>) retain exact executables, source observations, native per-step reports, phase markers and raw resource samples. All eight measured processes passed. [Paired comparison](<../app/evidence/native-history-bounded-lanes-4th1bizc/comparison.json>) verifies exact native viewport/content/anchor-slot/frame geometry and model key/intra/height/offset/extent parity at every step, not just fewer shape calls. Both source snapshots stayed unchanged during their runs; complete compiler/Core/transport/dependency/runtime closure remains unqualified.

A separate [full-retention paint comparison](<../app/evidence/native-history-paint-067f1di5/qualification.json>) ran those same retained executables and found byte-identical own-renderer PNGs, SHA256 `faa22afce839dcd02e68357693e476d23be2da362fc0d75cc64a1208f26e25a8`. The [optimized frame](<../app/evidence/native-history-paint-067f1di5/bounded-lanes/own-window.png>) was directly inspected. Its unused right side is intentionally outside the650px controlled App frame in the1040px owned window. This is no desktop capture or window-manager resize qualification.

## Memory and idle

The [external sampler](<../app/measure_native_history.py>) reads only its owned fresh Rust PID, fences its start-time identity, includes its threads in process CPU ticks and checks direct children across its task list. No direct children were observed and no samples were missing; that census is observational, not adversarial process-tree containment. Cargo and the sampler are outside the measured process. RSS counts shared resident pages; PSS apportions them. Neither measures GPU VRAM or the optional Harness backend.

Large optimized runs held roughly94.59–95.35MiB RSS and55.84–56.19MiB PSS; baseline ranges94.41–95.29MiB RSS and55.67–56.22MiB PSS overlap. Idle measured0% of one core over about1.95s/39 samples per run with100CPU ticks/second. This is0 at the accounting resolution, not proof of zero CPU. Profiles/XDG caches are fresh per process; OS/font/GPU-global caches are not cleared. Two repeats are limited observational evidence, not a universal confidence guarantee.

## Implementation and instrumentation

The [bounded cache](<../app/src/chat_geometry.rs>) shares one role/preview string pair and stores at most two small scalar geometries per key, never two shaped Paragraphs. Secondary hits promote; a third width discards only the least-recent geometry. Role/preview changes invalidate both widths. Entry capacity remains4097 (4096 retained keys plus the cache-only live key). Canonical chat width is exactly the existing native paragraph cap640px for You /800px for Assistant; extra empty lane width above the cap cannot alter geometry. Fixed-height tools/opaque records retain actual lane width keys.

Timing/shape counters and the rig are feature-only; default product builds do not carry them. Scalar/frame/receipt inspection does not run projection or shape previews. Full diagnostic model calls are counted/timed separately. App view timing measures Element construction, not Iced layout/GPU/presentation; operation timestamps are captured at matching receipt delivery before diagnostic processing. Observer/control messages themselves can cause view rebuilds, and phase-log fsync/diagnostic work is included in observed wall latencies—not subtracted to invent an independently measured product time.

Six added cache regressions cover exact fresh-native geometry parity over cap boundaries648/808, Unicode/malformed widths, LRU promotion/third-width eviction, role/text invalidation and4097-key dual-width capacity. Five rig tests cover passive scalar inspection and full-retention mid-history reflow/prepend. Four CLI tests enforce numeric bounds/duplicates/missing flags/cycle order. Four [pure sampler accounting tests](<../app/measure_native_history_test.py>) check CPU denominators, nearest-rank percentiles, missing PSS and idle boundaries. Final retained [default app tests](<../app/evidence/native-history-bounded-lanes-4th1bizc/final-default-tests.log>) pass436; [scroll example tests](<../app/evidence/native-history-bounded-lanes-4th1bizc/final-scroll-example-tests.log>) pass447. Counts include concurrent work, not only this cache refinement.

## Remaining gates

Mixed-role/Unicode/long-preview native workloads, continuous live text, physical wheel/keyboard/IME, actual rendered-slot-count census, accessibility, compositor presentation/FPS, native window negotiation, fractional compositor scale, full owned Host/Node process-tree performance, rich diff/Markdown parity and refreshed product packaging remain separate gates. The older default renderer capture and development archive do not automatically acquire this new source refinement.
