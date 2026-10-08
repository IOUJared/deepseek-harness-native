# Native Details identity and dismissal

Details opener, dismissal and formatting routing is local and leaves the wire protocol, lockfile and main-chat preview unchanged. The separate explicit [Copy source action](<NATIVE-DETAIL-COPY.md>) uses the same panel identity and visibility checks.

## Local routing

- Transcript buttons, including tool/raw rows and the synthesized live-assistant preview, emit an opener lease containing generation, prior consumed global opening identity and record key. A successful opening consumes that identity once. Delayed requests from another generation or a previously opened/closed panel cannot reopen or adopt the same numeric key in the current conversation.
- Back and Formatted/Source actions carry the exact generation/opening stamp of the visible frozen panel. Effective visibility excludes Settings, export, management, decisions, close confirmation and shell covers.
- Escape was an independent missed path: generic `DismissPanel` previously cleared Details without a stamp. The keyboard subscription now uses Iced's context-bearing subscription identity to capture the visible panel stamp. Old queued Escape carries the old ticket; generic stateless/composer dismissal only closes covers and never clears Details. Closing a sidebar/information/menu cover preserves the underlying frozen panel.
- The checked opening counter is global to this App instance, not reset by conversation selection. Exhaustion disables future opener controls and cannot mint a reusable ticket. A last readable panel and its valid Back/format ticket remain usable. A missing key or absent live stream does not consume an identity.
- Disconnected local panels remain readable and dismissible. Source remains the bounded reducer display snapshot, not full raw wire/final-reply authority. Same-generation source resolves at valid-opener delivery; this is **not a per-record revision lease**.

## Verified evidence

[Lifecycle qualification](<../app/evidence/detail-lifecycle-1f8h6xg_/qualification.json>) records **17 Details admission tests** (nine new), alongside **474 default / 481 feature / 483 layout-example / 485 scroll-example** regressions:

- [Default tests](<../app/evidence/detail-lifecycle-1f8h6xg_/default-tests.log>) / [feature tests](<../app/evidence/detail-lifecycle-1f8h6xg_/feature-tests.log>)
- [Layout tests](<../app/evidence/detail-lifecycle-1f8h6xg_/layout-tests.log>) / [scroll tests](<../app/evidence/detail-lifecycle-1f8h6xg_/scroll-tests.log>)
- [All-target feature check](<../app/evidence/detail-lifecycle-1f8h6xg_/all-targets-check.log>) / [owned-source formatting](<../app/evidence/detail-lifecycle-1f8h6xg_/owned-format.log>)

Model tests cover stale generation/consumed opener leases, old Back and Escape after reopening, stateless dismissal, shell/modal opener/Back/mode rejection, actual App Select/Reload routes, missing keys, maximum-counter failure and preserving the final valid Back. Select/Reload tests capture expected PUBLIC `Command::Select` intents; no business executor runs. Geometry invariance is qualified for mode switching after opening, **not** for the whole open→Back lifecycle: mounting/unmounting intentionally invalidates scroll restoration identity.

[Native paint qualification](<../app/evidence/formatted-details-paint-dqslw1qt/qualification.json>) passed four fresh owned-Wayland captures, all directly inspected:

- [Source](<../app/evidence/formatted-details-paint-dqslw1qt/assistant-details-source/own-window.png>) / [wide Formatted](<../app/evidence/formatted-details-paint-dqslw1qt/assistant-details-formatted/own-window.png>)
- [narrow Formatted](<../app/evidence/formatted-details-paint-dqslw1qt/assistant-details-narrow/own-window.png>) / [unsupported Source fallback](<../app/evidence/formatted-details-paint-dqslw1qt/assistant-details-fallback/own-window.png>)

Actual native operations find exactly one bounded Back control in each case, plus exact expected Formatted/Source controls; snapshots retain zero captured/dispatched commands. PNG hashes are unchanged from earlier inspected Details paint. Frozen scoped probe: 13,572,128 bytes, SHA-256 `1c5d287733b8d667a7e933a721b93a2c66950050f5f991993473df1008ca145b`. Source pins were stable through captures. No Host/worker startup, desktop capture, business or clipboard executor was used. This is controlled actual-App/native widget geometry and scripted model routing, **not physical keyboard/pointer delivery**.

Pinned local `iced_futures 0.14.0` source documents that the `.with` value participates in subscription identity and is cloned into each event's context. Context7 remains quota-exhausted; this is local pinned-source verification, not freshly retrieved documentation.

## Boundaries

The preexisting unchecked `choose()` generation increment remains outside this scoped opening-counter fix. This does not claim every application lifecycle counter is checked, a reservation of `u64::MAX` against future durable wire keys, immutable source-at-click identity, accessibility/IME/physical input, integrated latency/resources or full parity.

The [current development archive](<NATIVE-DEVELOPMENT-PACKAGE.md>) ships this fix in a separately rebuilt and qualified executable; earlier 33f/e933 archives remain frozen and do not acquire it in place. The [new distribution qualification](<../app/evidence/package-lifecycle-refresh-_20f978m/qualification.json>) is separate from the original source/probe evidence above. Installed Harness GUI/runtime/profile and desktop configuration remain unchanged. Parser scope and display-source limits remain documented in [bounded formatted Details](<NATIVE-FORMATTED-DETAILS.md>).
