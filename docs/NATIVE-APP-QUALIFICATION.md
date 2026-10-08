# Real native application qualification — development, not parity release

**Historical build-specific evidence.** Later features have their own guides; this page is not a current parity/performance certificate. All evidence links below are **local ignored artifacts, unavailable on GitHub**.

## Native Codex support

[Codex](<NATIVE-CODEX.md>): explicit same-version Host browser sign-in, masked fallback and revision-CAS model route. Keyless tests/status/rendering do not establish real authorization, entitlement or GPT generation.

## Historical modernized frontend

[Design](<NATIVE-FRONTEND-DESIGN.md>) · [qualification](<../app/evidence/frontend-modernization-qppgwtsc/qualification.json>) · [125% frame](<../app/evidence/frontend-modernization-qppgwtsc/final-native-scale125/own-window.png>).

**83 tests**, formatting/all-target/release checks, two isolated-alpha keyless Wayland runs at app scales 1/1.25, ten PUBLIC component fixtures. Earlier frames below do not represent later frontend changes.

## Verified scope

- Real release app + isolated alpha Host, not synthetic benchmark; fresh private homes/workspaces, no model/catalog/provider requests or installed-data/desktop changes.
- [First](<../app/evidence/qualification-native-01/result.json>)/[optimized](<../app/evidence/qualification-native-04-owned-traversal/result.json>): matched PID/app ID, native Wayland/no XWayland, raw-display handle, workspace/session/follow and real cursor-2 fold.
- Three bootstrap records: `permission/preset`, `sandbox/mode`, `approval/policy`—metadata, not generated chat.
- Authenticated Core/HTTP/WS and graceful exit zero/no observed descendants; no external forced cleanup.
- [Own-renderer frame](<../app/evidence/qualification-native-04-owned-traversal/own-window.png>) inspected: readable DejaVu Sans 14/Rosé Pine, policy/sidebar/composer/disabled smoke Send. No other windows captured.
- Machine `paintVerified:false` remains unchanged; human inspection is separate. IME/CJK/accessibility/full typography unqualified.

## Application-controlled scale qualification

[1.25 report](<../app/evidence/qualification-native-05-scale-125/result.json>)/[frame](<../app/evidence/qualification-native-05-scale-125/own-window.png>): real fold/cleanup/native mapping, enlarged readable view without observed clipping.

Physical tile **1272×1402**, logical **1024×1128**; unexplained 8px geometry/content difference also at scale 1. Compositor stayed 1.0—not compositor fractional scaling or matched 1200×800 benchmark.

## Found and corrected idle CPU regression

- Initial keyless idle ≈**11.4% of one core**. [Correct task probe](<../app/evidence/qualification-native-03-task-stat/result.json>) traced Core's all-desktop `/proc` scan every 25ms, not recurring frontend redraw/model work.
- Now traverses owned roots/all threads/verified direct descendants, with PPID/start-time/rechecked-parent/pidfd fencing and unchanged deadlines. Reparent/group changes retain authority; no guessed-PID signaling. Unreaped-root-only killpg invariant remains.
- [Fork/group fixture](<../core/src/process_tests.rs>) and lifecycle/privacy tests pass. Sampled ownership—not adversarial cgroup confinement.

| Optimized UI+Node, 15 late samples | Median |
| --- | ---: |
| CPU, one-core percentage | 0.0% at sampling resolution |
| RSS / PSS | 334,528 / 254,881 KiB (326.7 / 248.9 MiB) |
| Available DRM VRAM | 57,788 KiB (56.4 MiB) |

- ≈558ms mapping is a proxy, not presented-frame/Host-ready latency. Short keyless samples are not long-history/toolkit/workload qualification; synthetic frontend 74MiB is separate.
- [Earlier thread probe](<../app/evidence/qualification-native-02-threads/result.json>) used aggregate `/proc/tid/stat`: per-thread values invalid; full-tree metrics independent.

## Rebuilt approval/question release smoke

[Report](<../app/evidence/qualification-native-06-dialog-build/result.json>)/[parent](<../app/evidence/parent-dialog-build-qualification.json>)/[frame](<../app/evidence/qualification-native-06-dialog-build/own-window.png>); SHA `d9da9b01b8057dd90baf1b16e3ba8a45134c5e4365a38ab8872ef6e2ce5d1121`.

- Native fold/cleanup and inspected bootstrap paint; 12 late medians: CPU 0% sampled, RSS **331,752**, PSS **253,368**, VRAM **57,788 KiB**. Not statistical improvement.
- Fixed self-ACK retirement (Gateway cancels other clients only) and illegal positional arrays with object/authority/stale-attempt fencing.
- No decision request/reply in this smoke. [PUBLIC key-storage proof](<../smoke-home/api-key-public-fixture-22xiqsvu/result.json>) is separate, not composed UI persistence.

## Masked settings and actual-component qualification

[Parent](<../app/evidence/parent-settings-qualification.json>), SHA `b5ab8a3d0cfcc8be567b4e6a65b32d466bf08807e5a08726c70b5bc410b87c0d`; real runs [1](<../app/evidence/qualification-native-10-settings-header-final/result.json>)/[1.25](<../app/evidence/qualification-native-11-settings-header-scale-125/result.json>).

- Inspected readable bounded heading; scale-1 medians CPU 0% sampled, RSS **336732**, PSS **258289**, VRAM **57788 KiB**.
- Components [1](<../app/evidence/dialog-components-02-contrast/result.json>)/[1.25](<../app/evidence/dialog-components-03-scale-125/result.json>): ten PUBLIC approval/question/unsupported/masked/confirmation renderer runs; no Host/RPC/credentials/physical input. Contrast/heading defects fixed and re-inspected; machine paint/input/live flags stay false.
- [Audit](<../app/evidence/settings-audit.md>): batching/epoch/wiping/draft/stop-before-join fixes. **Review+Confirm**, metadata-only output, honest indeterminate errors; `has_api_key` cannot prove an attempt committed.

## Scripted composed native API-key persistence

[Guide](<NATIVE-COMPOSED-SETTINGS.md>)/[parent](<../app/evidence/parent-composed-settings-qualification.json>), fixture SHA `85bed3c8f39ac0fd7b4921f83bd9109cbac704cd8a052bd918ee59220a9aa17a`.

- Both app scales: real matching ACK, independent 0600 PUBLIC-dummy file, metadata refresh, inspected masked views, native Wayland and clean exit.
- **86 tests** (83 shared + three guards); 11 composed runner/15 total runner tests, including TERM-ignoring pidfd descendant after root exit.
- Source audit found no default automatic external request—not packet tracing/custom-profile guarantee. No physical input/real-key proof or replacement production-performance manifest.

## Evidence and remaining gates

| Historical test scope | Passed |
| --- | --- |
| Shared app/component / composed example | 83 / 86 |
| Core with / without transport | 46 / 44 plus four privacy doctests |
| Transport | 19 fixture groups + Agent/Session doctest |
| Native qualification runners / benchmark runners | 15 / 15 |

Later registry/export/files/decisions have separate guides; old deferred lists are not current status. See [feature status](<FEATURE-PARITY.md>) and [unsupported areas](<../app/README.md#deliberate-unsupported-areas>).

**Still no parity claim:** real account/model generation, physical input/IME/accessibility, representative integrated long-chat/scroll/resources, self-contained Linux install/update and full Windows/macOS coverage. Optional guests remain lazy/isolated; primary app has no Electron or mandatory webview.
