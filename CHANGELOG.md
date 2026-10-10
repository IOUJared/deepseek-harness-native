# Changelog

## 2026.10.10-r1 — Native Linux development release

October 10, 2026, America/Chicago. Calendar versions use `year.month.day-rN`; `N` is the release number for that day. This release collects the current frontend, Core, transport and companion-backend changes—not only the last Settings fix. [Release checks and limits](<RELEASES/2026.10.10-r1.md>).

### Settings and Codex

- Centered Settings with Account, Models & runtime, Images and Maintenance tabs, an inactive background and visible consent/error/cancellation status.
- Managed local account roster, Add, epoch-checked Switch, local Sign out and Sign out all. Explicit Review → Confirm; stale callbacks, unknown outcomes, selected-account Switch and empty-All guards. Interrupted sign-in invalidates cached roster controls until fresh owned readback. Add does not automatically select an account; Connect follows only its own exact added-account receipt. No legacy grant import.
- Eight runtime/image-input preferences: transport, cache retention, request/connect/idle timeouts and image pixel/per-file/total byte budgets. Revision-checked Refresh/Save, local Reset drafts, inherited/effective/override distinctions and explicit reconciliation after uncertain saves.
- Revisioned private usage preferences with bounded admission, cooperative flock and atomic writes. Supported local opt-outs persist; unavailable opt-ins stay disabled and owner-rejected. Quota/app-server/reset-policy foundations remain experimental and unwired, not live quota support.

### Models and composer

- Anchored provider/model, reasoning, service-tier and output-detail menus with responsive Back/toggle/outside-click behavior and owned acknowledgement before selection.
- Advertised provider-backed priority processing and verbosity controls; default omits overrides. Priority is a premium request, not a speed or entitlement guarantee.
- Startup active-model read and automatic startup/picker catalog refresh. Global read tickets reject stale, duplicate and unsolicited results; loading/offline/applying labels reflect actual state. Welcome selection can create a conversation without prompting.
- Enter/Ctrl+Enter send non-whitespace text; Shift+Enter inserts a newline. File-only Send remains explicit.

### Attachments and drafts

- Native in-window file picker, bounded asynchronous browsing, local PNG/JPEG previews and one-slot attachment ownership. Selection/drop stays local; Upload and Send are separate explicit actions. Remove/Cancel does not roll back remote storage.
- Pinned winit 0.30.13 Wayland source patch: bounded single-local-file URI drops on the existing owned queue, COPY-only admission, dispatch/deadline/seat/window limits and visible rejection/Attach fallback. Preserve Apache-2.0 notices, upstream pins and bundled SVG CC-BY-SA attribution.
- Private plaintext per-session drafts, bounded/coalesced persistence, explicit recovery review and Reload after uncertain failures. Separate original Send ownership from newer editor revisions; late acknowledgements retire only original ownership. Exit joins admitted picker/preview/draft filesystem work.

### Conversation and navigation

- Native Markdown/code rendering, modest lexical highlighting, collapsible supplied thoughts, Copy/code Copy and explicit text-only Edit-to-draft. Unsupported markup stays literal; links stay inert. No hidden-reasoning request or Regenerate replay.
- Ctrl+F search of the loaded conversation with native prose/code/thought highlights, navigation and scroll geometry; no implied remote search or older-page fetch.
- Keep Host-confirmed queued input visible through preparation, correlate durable user messages by identity, preserve append-origin conversation across context rewrites and avoid duplicated streamed/durable answers.
- Surface preparation, interruption and output-limit failures. Suppress credential-bearing auth diagnostics and title/model-control events from chat; retain raw records and older/reload actions for empty paginated views.
- Validated context-pressure/last-request usage and Host-owned Working indicators with focus/animation gates. Bound navigation to 200 matching sessions without duplicating the complete session list.

### Delivery, transport and teardown

- Freeze original Send identity/payload/file ownership after acknowledgement uncertainty. Read-only Check submission uses typed prompt-admission evidence; unknown/malformed/missing observations never authorize resend. Admission is not provider completion.
- Explicit observation rejoin requires correlated original-pipe proof from the same owned Host and cached transport. No reauthentication/restart, automatic session follow/resume or mutation replay.
- Reserved Stop admission independent of ordinary worker/transport capacity. Cancellation preserves queued input; its acknowledgement does not mean idle.
- Immediate transport revocation on private-pipe EOF/protocol fault. Retry only wholly unwritten shutdown frames within the original grace deadline; never replay partial/poisoned frames or call an already-exited root forced.
- Incremental decoded UTF-8 framing accounting and validated WebSocket payload ownership transfer without cloning; retain authentication, correlation, envelope and queue guards.

### Companion backend

- Opt-in managed Codex vault/owner with private permissions, lifetime POSIX flock, atomic recovery, optimistic selection epochs, immutable account/epoch SDK bindings and drainage before switching/deletion. Ordinary profiles retain legacy behavior; account service stays Host-only and value-free.
- Explicit request controls through LLM, defaults, Session Controller, TS/Python SDKs, ACP, subagents and replay. Exact supported endpoint/model allowlist; reject unsupported controls before credentials/provider I/O. Omission preserves defaults; route changes clear overrides.
- Payload-correct bounded prompt-admission ledger: reserve before asynchronous intake, identical concurrent requests join, changed payload conflicts, predecessor missing digest stays unverifiable. Nonactivating status and historical Inbox evidence cover receipt-loss gaps; no automatic resend.
- Immutable forwarded Remote event snapshots with separate source/client/pending count and UTF-8 byte budgets and explicit overflow errors; lifecycle cleanup and one-argument source-factory compatibility retained.
- Lazy optional-browser HMR polling: no recurring bundle scans without SSE consumers, catch-up before opening, one shared poller and silent stale callbacks/disposal. Web composition remains available.
- Focused negative/lifecycle/Loader tests, current-v4 request-control SDK/session/Python recordings, bilingual catalogs, type-equivalence and persistence acknowledgements, and upgrade guides. [Companion changelog](https://github.com/IOUJared/deepseek-harness-linux/blob/v2026.10.10-r1/CHANGELOG-NATIVE.md).

### Verification and distribution

- Risk-based verification skills/instructions, serial two-job locked/offline runner, doctor/lint/example suites, selected Rust safety checks, source-drift detection and retained failures.
- Expanded production reducer, worker, parser, fake-wire and storage tests; PUBLIC composed account/runtime/usage/file/draft and isolated native-seat/drop/interruption tooling. These owners do not imply a combined physical-input/live-provider certificate.
- Frozen source/binary selection, one bounded source copy/inventory/overlap policy including App vendor sources, exact extraction checks, explicit missing UI-certificate status and qualified external-runtime launcher. Source hashes are consistency—not compiler ancestry, signatures or reproducibility.
- Release verification fixes branded Settings event fixtures, regenerated Cordis API metadata, dispatch JSDoc and the pinned winit locale filter's Rust 1.99 lint compatibility. Preserve historical Session recordings; current admission-aware Web replay has its own fixture owner.
- Calendar native crate/archive/tag identity; explicit calendar publication prevents CI from racing verified assets. Legacy plain-version `dev.run` automation remains separate. Native-only CI explicitly reports companion-artifact tests as skipped; strict companion-artifact runs must fail if prerequisites are absent.

### Compatibility and unfinished work

- Exact rebuilt alpha backend `0.2.1-alpha.1`, Node 26, Python 3.11+ tooling and Linux/Wayland/system fonts/GPU libraries required. Backend SDK version remains unchanged; calendar tags are GitHub release identities, not npm/PyPI releases. Installed RC is incompatible.
- Pre-stable public enums, model selection/catalog and request fields require matching rebuilt consumers. Session format remains v4 and SQLite version unchanged; predecessor generations are preserved.
- No live OAuth/provider, verified subscription quota/reset/alerts/retry, image generation/sketch, physical mouse/keyboard/IME, accessibility, adversarial-containment or power-loss certificate. Local sign-out is not issuer revocation; metadata/catalog is not entitlement.
- One attachment/4 MiB intake, bounded PNG/JPEG previews—not a resource sandbox. MOVE-negotiated file-manager drops may be rejected; Attach is the fallback. Drafts/vaults are private local plaintext, not encryption or legacy migration.
- Not standalone/installer/AppImage/update service, general Linux portability, Windows/macOS parity, clean-machine packaging or complete redistribution-license certification. No new performance ranking or long-chat benchmark claim. Private evidence, profiles, tokens and screenshots are excluded from GitHub source.
