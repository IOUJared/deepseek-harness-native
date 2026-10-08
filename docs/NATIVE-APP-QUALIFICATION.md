# Real native application qualification — development, not parity release

## Native Codex support

Native **Settings → Codex** now connects explicit same-version Host ChatGPT browser sign-in, a masked reviewed callback and separately reviewed revision-CAS model-route activation. The [Codex guide](<NATIVE-CODEX.md>) distinguishes credential metadata, auth acceptance, configuration and actual model entitlement. Qualification uses keyless Core/controller/worker regressions, public own-renderer fixtures and isolated real-alpha status reads; it does not authenticate an account or qualify real GPT generation.

## Historical modernized frontend

The ChatGPT-inspired native frontend is now separately qualified: **83 tests**, clean all-target/format checks, final release build, two real isolated-alpha keyless Wayland runs at 100%/125%, and ten public local component fixtures. [Current summary](<../app/evidence/frontend-modernization-qppgwtsc/qualification.json>) pins the final binary; [design and limits](<NATIVE-FRONTEND-DESIGN.md>) describe the responsive shell, finite motion and preserved authority. [Actual current frame](<../app/evidence/frontend-modernization-qppgwtsc/final-native-scale125/own-window.png>) was inspected from the app's own renderer only. Native input, minimum-window paint, model generation and full parity remain unqualified.

The earlier runs below remain historical evidence for their recorded builds; their older screenshots do not represent the modern frontend.

## Verified scope

The parent executed the **release Iced application with its real isolated alpha Host**, not the synthetic frontend benchmark. Every run used new private test homes/workspaces, made zero model prompts and no model-catalog/provider request, and changed no desktop settings or installed Harness data. The compositor owns normal placement; the actual tiled window was 1272×1402 rather than the benchmark's matched 1200×800.

[First report](../app/evidence/qualification-native-01/result.json) and [optimized report](../app/evidence/qualification-native-04-owned-traversal/result.json) pass:

- Independently matched owned app PID, `ai.deepseek.harness.native.app`, mapped native Wayland and no XWayland.
- Own Iced raw-display handle also reports Wayland.
- Real workspace registration, one keyless session creation/listing, live follow and successful actual snapshot fold at cursor 2. The three bootstrap records are `permission/preset`, `sandbox/mode`, `approval/policy`; these are real read-only metadata, not generated chat content.
- Authenticated Core/HTTP/WS composition, graceful owned exit zero, no observed descendants left, and no forced external cleanup.
- [Own renderer frame](../app/evidence/qualification-native-04-owned-traversal/own-window.png) captured exactly once after the real snapshot via Iced's window screenshot API. The parent inspected this actual PNG: readable text, standard dark Rosé Pine surfaces/lavender controls, visible real policy records, sidebar, composer and disabled smoke Send/model loading. No other desktop windows or user sessions were captured. This qualifies this keyless view, not full typography/IME or chat rendering.

The application's machine report intentionally retains `paintVerified: false`; **this parent inspection is separate evidence**, not retroactive alteration of an application claim. Requested DejaVu Sans 14 and consistent palette are visible; runtime CJK fallback/accessibility remain unqualified.

## Application-controlled scale qualification

[Scale 1.25 report](../app/evidence/qualification-native-05-scale-125/result.json) also passes real session/fold/shutdown and native mapping. The parent inspected its [own frame](../app/evidence/qualification-native-05-scale-125/own-window.png): enlarged readable text, controls and metadata cards, with no observed clipping in this view. The compositor retained a 1272×1402 physical tile while the app's queried logical geometry became 1024×1128; the same 8 px difference between reported geometry and rendered content also occurs at scale 1. The exact cause of that difference is not qualified here. This uses compositor-controlled placement, not an equivalent-size renderer benchmark. Native compositor scale stayed 1.0. Thus **application scale** is qualified here, not genuine compositor fractional scaling or every resize/input combination.

## Found and corrected idle CPU regression

The first native app measured approximately **11.4% of one CPU core** while keyless-idle. [Correct task-specific thread evidence](../app/evidence/qualification-native-03-task-stat/result.json) attributes it to Core's persistent ownership monitor, not recurring frontend redraw or Node model activity. Its original descendant sampler walked all desktop `/proc` PIDs every 25 ms.

Core now traverses only kernel direct-child lists for **every thread of its owned root and verified pidfd-owned descendants**, keeping the same observation interval and shutdown deadlines. It verifies child PPID/start-time, rechecks parent identity and captures a pidfd with identity fencing. Live already-owned parents refresh group identity without losing their pidfd authority, including orphaned/reparented parents which later change groups and fork again. No signal is authorized merely by a guessed numeric PID, and the unreaped-root-only killpg invariant remains.

The new [real Linux fork/group-movement fixture](../core/src/process_tests.rs) and all existing lifecycle/privacy tests pass. This is sampled observed-descendant ownership, **not adversarial cgroup confinement**.

[Optimized keyless run](../app/evidence/qualification-native-04-owned-traversal/result.json), 15 late samples:

| Whole owned process-tree metric | Median |
| --- | ---: |
| CPU, one-core percentage | 0.0% at sampling resolution |
| RSS | 334,528 KiB ≈ 326.7 MiB |
| PSS | 254,881 KiB ≈ 248.9 MiB |
| Available DRM VRAM | 57,788 KiB ≈ 56.4 MiB |

These values include **native UI + real Node Host** and are not the synthetic Iced benchmark's 74 MiB frontend figure. CPU median zero is not literally no CPU work. This is a short keyless-idle sample, not a statistically qualified workload, long-history performance claim, input/presentation latency measurement, or toolkit comparison. The approximately 558 ms compositor mapping observation is a proxy, not a presented-frame or fully-loaded-backend startup measurement.

An earlier [thread probe](../app/evidence/qualification-native-02-threads/result.json) used `/proc/tid/stat`, which exposed aggregate process CPU. Its per-thread values are explicitly marked invalid; only the corrected task-path probe supports thread attribution. Overall process-tree CPU/RSS/PSS values in that run remain independent.

## Rebuilt approval/question release smoke

The [earlier dialog-build report](../app/evidence/qualification-native-06-dialog-build/result.json) and [parent inspection record](../app/evidence/parent-dialog-build-qualification.json) qualify release SHA `d9da9b01b8057dd90baf1b16e3ba8a45134c5e4365a38ab8872ef6e2ce5d1121`: real native Wayland/session/fold and graceful owned shutdown again pass. The parent inspected its [own frame](../app/evidence/qualification-native-06-dialog-build/own-window.png), confirming the readable actual bootstrap view and preserved Rosé Pine appearance. Twelve late samples show median CPU 0% at sampling resolution, RSS 331,752 KiB (~324 MiB), PSS 253,368 KiB (~247 MiB), VRAM 57,788 KiB. Small memory differences are not a statistically qualified improvement.

Native approvals and untimed questions now have explicit typed dialogs and replies. A read-only review caught two real contract defects: waiting for self-Cancel after a successful reply (Gateway cancels only other clients) and serde accepting illegal positional request arrays. Both are fixed with bounded self-ACK retirement, stale/attempt fencing, object-only/reserved-authority validation and source-derived fixtures. This real smoke deliberately sends **no decision reply and receives no decision request**; it is not proof of dialog rendering/input. Timed/continued questions remain deferred. Masked API-key settings were subsequently implemented with separate public component fixtures; neither this earlier smoke nor those fixtures proves composed GUI-to-worker persistence. The separate [public-fixture credential proof](../smoke-home/api-key-public-fixture-22xiqsvu/result.json) tests real default isolated storage without a real key or provider request.

## Masked settings and actual-component qualification

The [final parent record](<../app/evidence/parent-settings-qualification.json>) identifies release SHA `b5ab8a3d0cfcc8be567b4e6a65b32d466bf08807e5a08726c70b5bc410b87c0d`. [Scale 1](<../app/evidence/qualification-native-10-settings-header-final/result.json>) and [application scale 1.25](<../app/evidence/qualification-native-11-settings-header-scale-125/result.json>) pass real owned native Wayland/Core/workspace/session/fold/clean shutdown again, without model/catalog requests. The parent inspected both own frames: readable actual bootstrap view and a bounded heading that no longer overlaps the settings button. Short UI+Node scale-1 medians: CPU 0% sampled, RSS 336732 KiB (~329 MiB), PSS 258289 KiB (~252 MiB), VRAM 57788 KiB. Differences between short runs are not statistical improvements.

Separate [scale-1 component report](<../app/evidence/dialog-components-02-contrast/result.json>) and [scale-1.25 report](<../app/evidence/dialog-components-03-scale-125/result.json>) pass ten native renderer runs of actual approval, question, unsupported timed, masked settings and explicit save-confirmation components. The fixture path-imports actual modules/styles/font/PNG code, but starts **no Host/Node/client**, sends **no RPC**, writes **no credentials** and uses only PUBLIC fake data. Scripted local reducer actions verify exact replies/ACK retirement or explicit secret transfer/indeterminate simulation; they are not keyboard/pointer clicks or composed Host outcomes.

Parent inspected approval/question/unsupported/settings/confirmation frames and selected enlarged frames, seeing masked fake text and visible explicit warning/confirmation. Inspection caught low-contrast question options (fixed by inherited button foreground) and a long real-app heading at enlarged scale (fixed by bounded prefix/fill width); final corresponding frames were re-inspected. Machine paint/input/live-Host flags remain false, with parent findings separate. These inspections qualify only the named views, not full accessibility, CJK/IME, resize/hit-testing or genuine compositor fractional scaling.

The [settings audit](<../app/evidence/settings-audit.md>) records corrected batched-edit loss, stale startup epoch, wiping restoration, rejected-draft handling and stop-before-join. Masked settings require explicit Review+Confirm, metadata-only responses and honest indeterminate errors; `has_api_key` never proves a particular attempt committed. The prior real Core PUBLIC-key storage proof and these component fixtures are **separate**, not composed GUI-to-worker credential persistence proof.

## Scripted composed native API-key persistence

Subsequent [composed qualification](<NATIVE-COMPOSED-SETTINGS.md>) closes the scripted GUI→worker→Core/default-provider save gap without a real key. [Final parent record](<../app/evidence/parent-composed-settings-qualification.json>) pins fixture SHA `85bed3c8f39ac0fd7b4921f83bd9109cbac704cd8a052bd918ee59220a9aa17a`: both application scales pass matching real persistence ACK, independently verified owned 0600 PUBLIC-dummy file, explicit refreshed metadata, native Wayland and graceful zero-exit/no-observed-survivor cleanup. Parent inspected both masked modern settings frames; physical keyboard/pointer/IME input and key validity remain unqualified. Default save/account side-effect source audit found no automatic external request path; this is not a packet trace/custom-profile guarantee.

The separate runner repaired required root pin and finite all-pidfd TERM/KILL cleanup, including a TERM-ignoring pinned descendant after GUI-root exit. Eleven composed runner regressions pass (one real headless Linux Python/pidfd scenario); all runners pass fifteen tests. Eighty-six example tests pass: 83 shared app/component tests plus three new fixture guards. Concurrent independent native restyling was preserved; the named fixture evidence does not replace the earlier production-app performance/release manifest.

## Evidence and remaining gates

- 83 shared native app/component tests pass (86 in composed example including three new guards), including priority shutdown under a full command queue, stream generation isolation, unknown required-event refusal, bounded virtual history, real-smoke fences and exclusive own-renderer screenshot output.
- 46 Core fixture tests and four compile-fail privacy doctests pass with the optional transport composition (44 plus four without it).
- 19 transport fixture groups and one distinct Agent/Session compile-fail doctest pass, including dropped logical-open lifetime, prompt cancelled-stream byte-budget release, self-ACK retirement, malformed forwarded authority refusal and safe-integer request bounds.
- Native/component/composed qualification runners have fifteen headless environment/private-output/owned-cleanup tests; benchmark runner/analyzer retain their separate fifteen headless tests.

[Application limitations](../app/README.md#deliberate-unsupported-areas) remain substantial: real OpenAI sign-in/entitlement qualification, device/sign-out/account-switching controls, physical native API-key input/IME/real-key validity, timed/continued questions and live approval/question qualification, rich attachments, plugin panels, terminal/document/browser guests, session management and subagent steering. Actual model generation, resizing/input/IME/accessibility, long-chat/scrolling responsiveness and integrated matched resource measurements need further qualification. Linux install/update packaging and full Windows/macOS feature parity are **not delivered**. Optional browser/account/plugin guests remain planned, lazy and isolated; the primary app has no Electron or mandatory webview.
