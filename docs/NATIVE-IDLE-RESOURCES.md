# Native idle resource observations

Local ignored evidence, not a release/performance certificate. Primary UI remains native Iced/wgpu/Wayland; no Electron or mandatory webview.

## Matched frozen snapshots

[Contract](<../app/evidence/native-package-idle-ab-wmmauno5/contract.json>) · [comparison](<../app/evidence/native-package-idle-ab-wmmauno5/comparison.json>) · [process attribution](<../app/evidence/native-package-idle-ab-wmmauno5/cpu-attribution.json>)

- Old `ac91` / Codex-controls `2e17` binaries: unchanged hashes, identical release settings (`opt-level=3`, thin LTO, one codegen unit, stripped), Iced 0.14 and dependency manifests.
- **Three fresh eight-second runs each**, ABBAAB order, app scale 1, own Wayland 1272×1402, `/usr/bin/node` v26.11.1, original full isolated alpha composition, private PUBLIC blank homes/workspaces/sessions.
- Six existing lifecycle oracles pass: real blank fold, no catalog/prompts/credentials, graceful stop, no observed survivors, private markers/released locks. Observer source unchanged. Full external Host dependency closure is not hash-attested.

| Warm observation, median (range) | Previous snapshot | Codex snapshot |
| --- | ---: | ---: |
| Full-tree CPU, % one core over window | 0.30 (0.30–1.22) | 2.13 (0.61–2.74) |
| Full-tree RSS, KiB | 344,148 | 343,896 |
| Full-tree PSS, KiB | 284,644 | 284,287 |
| Own window mapping proxy, ms | 328 | 330 |

Memory differences are within noise. CPU differs in this small sample: current snapshot's extra ticks mostly belong to its owned backend, **not established as sustained idle regression or its cause**. The 3–6.5s window may include startup/GC; no Host-ready timestamp or stack profile was measured. Do not optimize frontend redraw based on these numbers alone.

## Why the older 0% median was insufficient

[Package qualifier](<../scripts/qualify-development-package.py>) now retains at most **256 owned-process samples** and reports endpoint CPU over the entire warm span plus one-tick resolution (about **0.30 percentage points** here). Identity sets must remain equal throughout; nonpositive time, missing identities or decreasing counters produce **unknown**, not zero. Per-sample medians still exist for compatibility; zero median can conceal brief work.

[13 verifier tests](<../app/evidence/native-package-idle-ab-wmmauno5/metric-guard-tests.log>) pass, including counter/time/identity-churn/bounds. This is sampled ownership and accounting, not adversarial confinement. No new profiler/dependency/commit was installed or created.

## Limits / next owner

- Observation baseline only: no production performance optimization, speedup, toolkit-superiority or causal Codex-overhead claim. Many features differ between snapshots; `n=3` is not a statistical regression verdict.
- Eight-second blank profiles are **not** long-history/scrolling/model/physical-input benchmarks. Mapping is not presented-frame or backend-ready latency. Load averages/raw ticks remain in each report; no desktop settings were changed.
- Next: longer owned backend CPU/GC attribution with the same workload before an optimization hypothesis; retain the oracle and compare interleaved releases. Original [Slint/Iced synthetic comparison](<../benchmarks/results-comparison-03/MEASUREMENTS.md>) is a separate workload, not integrated-backend proof.
- Real account/quota/alerts/reset retry/generation, standalone packaging and cross-platform parity remain separately gated; [feature status](<FEATURE-PARITY.md>).
