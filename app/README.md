# Real native Iced frontend — Linux integration

This application talks to an **owned, explicitly supplied alpha fork Host** through Rust Core and typed authenticated transport. Normal startup contains no benchmark fixture, opens no browser/webview, requests no model catalog, and sends no prompt. This is not full Windows/macOS/Web GUI parity.

## Development archive

The [external-runtime native development package](<../docs/NATIVE-DEVELOPMENT-PACKAGE.md>) contains the qualified one-Settings/Harness-style native UI, bounded Formatted/Source Details, current Details opener/Back/Escape identity fencing, observed-Node launcher, source/examples and available Rust notices. The owning page provides the current archive identity and preserved older records. Complete payload/extraction hashes, extracted-source all-target compilation and actual relocated launcher→native→Host startup/graceful shutdown at application scales 1/1.25 pass, with parent-inspected own-renderer paint; see [current package evidence](<evidence/package-node-copy-refresh-qpwmgtsn/qualification.json>). **It still requires a separately built compatible alpha CLI runtime, Node major 26 (default `/usr/bin/node` or explicit absolute `--node`) and glibc 2.44**; it is not an installer, self-contained runtime or generally portable Linux distribution. Physical input, real account/models and full parity remain unqualified.

Current source and the refreshed archive additionally include the explicit [Copy source action](<../docs/NATIVE-DETAIL-COPY.md>) and [optional Node interpreter path](<../docs/NATIVE-EXPLICIT-NODE.md>) through app and launcher. Default/private-copy extracted startup passes at both app scales; OS clipboard output and physical input remain unqualified.

## Build

Rust 1.99; Iced 0.14.0, locked standalone Cargo workspace. The own target directory and two build jobs are configured in [.cargo/config.toml](<.cargo/config.toml>). Shared dependency cache is intentional; no fork, Core, transport, benchmark or installed-runtime sources are changed by this app.

```bash
CARGO_HOME=/home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-tauri/.cargo-home \
CARGO_BUILD_JOBS=2 cargo test --offline --locked
CARGO_HOME=/home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-tauri/.cargo-home \
CARGO_BUILD_JOBS=2 cargo build --release --offline --locked
```

Run from this directory with explicit paths:

```bash
./target/release/dsh-native-app \
  --runtime /absolute/explicit-fork/apps/cli \
  --expected-version 0.2.1-alpha.1 \
  --home /absolute/isolated-native-home \
  --user-home /absolute/user-home \
  --cwd /absolute/existing-workspace \
  --scale 1.0
```

The supplied runtime is validated by Core; no installed-runtime fallback. Current source additionally accepts optional `--node /absolute/node-executable`; omission retains `/usr/bin/node`, with no PATH discovery. See [explicit interpreter selection](<../docs/NATIVE-EXPLICIT-NODE.md>) for validation and distribution limits. Native home must be distinct from user home. `--user-home` defaults to HOME only when omitted; other paths/version are mandatory. `--scale` accepts 0.75–2.0. `--help` starts neither Core nor GUI.

The renderer explicitly requests **wgpu only**, before any app threads start. Tiny-skia is compiled but not an automatic cross-renderer fallback. The own native display handle is queried once; there is no recurring redraw observer or idle polling timer. Wayland app ID is `ai.deepseek.harness.native.app`. Requested base window is 1200×800, minimum 760×560, resizable. Compositor placement/geometry can differ. Dark standard Rosé Pine tokens and DejaVu Sans 14 are used; actual paint, CJK fallback, IME and fractional-scale behavior require GUI qualification.

## Native session ZIP export

**More → Manage & export… → Export session ZIP…** opens an inert/dimmed in-window destination/review dialog. Enter an absolute **new** `.zip` filename, review the unredacted-content and live-log-flush warnings, then explicitly confirm. Descendants are excluded but referenced root files/images are still included. The current application caps downloads at 4 MiB; it never extracts, adopts a remote filename, overwrites, deletes a failed save or automatically retries. Saved output must be mode 0600 with exact bytes and file/directory sync; later failure may leave an empty/partial/complete private file. Closing hides the panel, not an admitted operation, and shutdown joins owned filesystem work without a hard unhealthy-filesystem deadline.

[Export qualification and limitations](<../docs/NATIVE-SESSION-EXPORT.md>) cover 221 Rust tests, 46 runner tests, two actual isolated UI/worker/Host Wayland application-scale-1/1.25 runs, four parent-inspected frames and independent bounded ZIP CRC/DEFLATE/v4-header checks. These are scripted public-data integration tests, not real keyboard/IME, rich-attachment or full desktop parity proof.

## Modern native frontend

The ChatGPT-desktop-inspired layout keeps Rosé Pine, native rendering and explicit backend actions: collapsible sidebar/compact rail with **one Settings entry**, a short title/More header, centered conversation, role-aware previews, rounded composer and matching settings/decision sheets. Less-frequent Manage/export, information and owned shutdown actions live under More. Secondary explanatory text is collapsed behind labelled chevrons; critical consent, errors and uncertainty remain visible. Metadata-only sessions have a welcome view with original records available explicitly; unknown/tool/system rows remain visible. Sidebar motion is finite (180 ms) and gated off at idle, when unfocused or behind modal panels.

- **Ctrl+B:** toggle navigation; narrow windows use a scrollable navigation/workspace sheet.
- **Escape:** leave auxiliary layout views without answering requests or changing permissions; when settings are open, close the modal and wipe the unsaved API-key draft.
- **Ctrl+Enter in the focused composer:** explicit Send, subject to the same lifecycle/smoke/modal guards. Enter remains a newline.
- **Stop** remains accessible while examining nonmodal full-text/details/info/navigation panes.

See [control-layout rationale and qualification](<../docs/NATIVE-CONTROL-LAYOUT.md>) and [earlier frontend design](<../docs/NATIVE-FRONTEND-DESIGN.md>) for evidence and scope. User messages are right-aligned bubbles; assistant text is left-aligned. Read/edit/write/shell calls and results are compact muted-grey summaries naming the tool and affected file or bounded command; click the chevron/row for full stored arguments/output in a read-only detail pane. Exact recorded error flags remain marked, without inferring execution or success. Raw sequence identity, unknown rows and older-page anchors remain intact with variable-height virtualization. This is bounded plaintext presentation, not a full Markdown/attachment renderer.

## Settings modal

Settings opens a centered in-window card over the dimmed native conversation, not a second OS window. The background is drawn but cannot receive mouse, scroll, keyboard, touch, IME, focus operations or popup overlays. Close/Escape explicitly wipe unsaved key drafts; outside clicks do not dismiss or save anything. Live backend updates and shutdown still work.

The sidebar contains **General**, **API login**, **Codex** and **Plugins**. API login retains its masked, explicitly reviewed/confirmed persistence flow. General displays native appearance/shortcut information. Plugins connects to the same-version alpha inventory and live-settings APIs with explicit Refresh and reviewed/confirmed field edits. Codex offers **Connect ChatGPT**: one click checks local status, opens the official browser sign-in and activates only the missing GPT route after matching authorization. OpenAI may still require sign-in/MFA/approval. Advanced recovery controls retain masked callback review and manual route review; navigation alone does not authenticate, load plugin settings or change profile files. See [native Codex use and safeguards](<../docs/NATIVE-CODEX.md>).

Switching categories wipes unsaved API-key, Codex callback and plugin drafts and invalidates old confirmation/input tickets. An already submitted write is not undone or retried by page navigation or closing; its receipt remains independently fenced. Narrow windows use category tabs and an internally scrollable body.

## Live plugin settings

**Plugins → Refresh plugins & live settings** explicitly reads configured Loader entries and live settings descriptors from the owned alpha Host. Opening, category navigation, namespace selection and startup readiness issue no plugin requests. Inventory enablement/status is informational, not an installed-package catalog or permission to enable, disable, install or remove anything. Namespace IDs come from the settings provider; the app does not guess associations from Loader prefixes.

Only supported boolean, finite bounded number and bounded nonsecret string fields under schema-declared live configuration with automatic presentation enabled are editable. Provider-owned custom presentations and permission/policy controls remain read-only. Secrets, credential references, sensitive-named paths, expressions, complex schemas and unsupported constraints remain excluded or read-only. Private wire JSON and Schemastery reference graphs are projected in transport before any GUI event; no raw JSON/config/schema/default/base/user editor or dump is provided. Custom plugin-provided UI and non-live boot configuration are not rendered.

Edit one field, **Review**, then **Confirm**. The worker refreshes that descriptor to verify the field still exists, then sends exactly one path-addressed set with the revision the user reviewed. The Host validates and persists through its settings provider; no profile file is edited directly by the GUI. A revision conflict clears the draft and requires explicit Refresh, never an automatic retry or overwrite. A mutation acknowledgment is not proof of plugin reload or runtime behavior. Backend refusals can include persistence/reconciliation failures, so their persistence outcome may be uncertain; timeouts, malformed acknowledgments and disconnects are also indeterminate.

Plugin operations share the bounded worker queue/task budget and their own single-flight reservation; plugin and API-key writes cannot overlap. UI and worker independently prohibit requests in real keyless smoke. Panel, epoch, description, editor, review and operation tickets prevent stale actions or receipts from changing another draft. Closing/category changes discard drafts without undoing submitted writes. Wiping covers owned editable buffers, not guaranteed GUI/Node/kernel/clipboard/swap erasure.

[First-pass qualification](<evidence/plugin-integration-r56lr8qd/qualification.json>) records the release, 138 app tests, transport projection/HTTP/privacy tests, six public own-renderer runs at application scales 1/1.25 and real isolated read-only alpha discovery (188 configured entries, 19 namespaces, 13 supported primitive candidates). Public write effects never reach a worker/Host; the real bridge performs reads only and exits cleanly. Actual UI→worker→alpha settings persistence, physical input/IME and accessibility are not qualified by these runs.

## Actual frontend behavior

- Session/workspace roster, workspace opening, new session creation, session filtering/selection and running markers come from real DTOs and live subscriptions.
- **Manage** exposes explicit review/confirm pin/unpin and archive/restore. Pins preserve registry-global Host order; archive retains history/accounting and unpins without force-stop; **View archived** makes Restore reachable and restoring does not repin. [Actual-App/worker/Host PUBLIC qualification](<../docs/NATIVE-REGISTRY-CONTROLS.md>) passes six ACKs plus independent feed/order/navigation checks at application scales 1/1.25; no real credentials or model calls. This does not qualify physical input, all active-work refusal cases or full desktop parity.
- Selecting follows an existing session without sending a prompt. History pages retain the opening cursor, deduplicate durable sequence numbers, and never silently mix a newer opening cut.
- Core surface append/replace and citation checks, alpha compact assistant records, reconnect baselines, contiguous stream revisions, dense chunk indices, live/durable settlement staging and generation isolation are folded locally for display.
- Policy/model/goal/todo/plugin diagnostics are read-only bounded JSON. Repository alpha event vocabulary is explicit. Unknown ignorable events remain opaque; unknown required events display a warning and disable Send until reload. **No sandbox, approval policy or permission preset is changed.**
- Model catalogs are loaded only by the **Load models** button: provider catalog work may perform external I/O. Model selection and reasoning effort use typed RPCs. Explicit Send (button or focused-composer Ctrl+Enter) posts a typed prompt with UTC and a unique client request ID. An accepted acknowledgment is not a claim of generation completion.
- Drafts survive failed Send and session changes; accepted old-session responses cannot erase the current session's draft. Stop waits for actual typed cancellation acceptance and live status; it does not fake idle.
- Tool activity defaults to slim single-line disclosures, inspired by the supplied Harness chat reference; full stored plaintext arguments/results require a click. Known sensitive summary fields/commands are suppressed, but this is not a general secret detector or redaction guarantee for stored content. Generic diagnostic JSON retains existing common-secret-field redaction. This is not a full Markdown, attachment or plugin-panel renderer.

## Native human decision dialogs

Live requests are presented in a scrollable native list and an in-window decision dialog. Approval offers **Allow once / Reject / Cancel request**; closing the panel leaves the request pending. Untimed questions support explicit single/multiple selections, custom text and explicit per-question skips. Nothing is selected or answered automatically. A plan-review intent remains a generic question with its full plaintext plan/detail and exact offered labels; an invisible plan or missing approval option is unsupported.

The dialogs derive exclusively from typed Remote Event frames. `AgentId` is distinct from `SessionId`; no session association is guessed. Replies use fixed `NativeClient::reply_approval`, `reply_question` and `cancel_question` RPCs. Cancel question uses the audited `ASK_CANCELLED` business error, not delegation. Native UI and worker both disable replies in keyless smoke.

A request is fenced by the owned transport epoch, Remote Event id, local request serial and submission attempt. Submitting requests cannot submit twice. Errors retain the request for explicit retry; cancellation removes only the matching request, and stale acknowledgments cannot restore or alter a replacement. A matching successful RPC acknowledgment removes that pending delivery and shows only a bounded transport receipt, never approval/tool execution success. The audited [Gateway](<../../deepseek-harness-linux/packages/api/gateway/src/index.ts#L614-L667>) excludes the replying Client from subsequent Cancel frames, so waiting for a self-Cancel would leak pending capacity. Unacknowledged requests remain pending; disconnection retains stale requests read-only and warns that an attempted outcome may be indeterminate. This app has one NativeClient lifetime and no reconnect; its worker epoch is fixed to that lifetime.

Limits: 64 pending requests and a 512 KiB reserved request/draft budget, 32 KiB complete request/answer JSON, 1–8 questions, ≤32 unique options per question, and 2 KiB custom text per question. Unsupported previews are ≤8 KiB. Capacity exhaustion reports an error without sending a reply or delegation. See the test-only [interaction fixtures](<fixtures/interactions.json>); these tests are not real GUI or model execution proof.

## Masked API-key settings

**API-key settings** opens a secure native input, never reads an existing key, and queries only logged-in/key-present/writable booleans. Metadata refresh is explicit. A panel opened before startup adopts the ready backend epoch safely, clears old input/tickets and requires explicit Refresh. No sign-in, catalog or model-validation request is made.

Save requires **Review explicit Save → Confirm Save API key**. Drafts accept 1–16384 printable non-whitespace ASCII bytes without trimming. Rejected input clears the prior draft so an old masked key cannot accidentally be saved. Fresh batched input messages share an editor-lifetime fence; edits invalidate old confirmation tickets. Closing/submitting/disconnecting invalidates that editor lifetime. Input clones share a one-use opaque holder, not plaintext clones. The worker command moves Core's non-Clone secret wrapper; events contain only nonsecret tickets/metadata/status. UI and worker independently forbid writes in real keyless smoke.

Only Core `Ok(true)` confirms persistence; refusal is distinct from **indeterminate** timeout/error. Metadata presence cannot identify which key committed and never upgrades an indeterminate result. No automatic retry/replay or draft restoration. Pending writes cannot overlap; late results do not confirm another panel. Closing is **not rollback**: an admitted write may commit. Core stop is requested before carrier waits, and blocking Core tasks are joined after stop.

The trusted default isolated profile stores credentials under native home with 0700 directory/0600 file. Trusted custom providers may write elsewhere; neither this UI nor Core constrains their configured paths. This app wipes its owned draft/input allocations and Core wipes its owned input/control frame. Iced widget/render buffers, clipboard, allocator/kernel/Node copies and swap are outside that guarantee. Secure input disables its copy/cut clipboard writes, but masking is not memory protection. Key presence is not account sign-in or validated model access.

## Composed PUBLIC-dummy persistence qualification

The separate [composed fixture](<examples/settings_composed_smoke.rs>) uses actual App settings handlers/view and actual worker/Core/default provider, not mocked receipts. It requires new owned private homes and an explicit isolated alpha fork fixed in source; no key/runtime/profile overrides are accepted. Manual widget/business messages are dropped. A fixed PUBLIC draft is reviewed, captured masked, explicitly confirmed through the real command path, acknowledged, separately checked in the owned 0600 file and metadata, then the owned backend stops cleanly.

```bash
CARGO_HOME=/absolute/shared-cache CARGO_BUILD_JOBS=2 cargo build --offline --locked --release --example settings_composed_smoke
python3 ../scripts/qualify-composed-settings.py --output /absolute/native/app/evidence/NEW-composed-run --scale 1.0
```

[Parent evidence](<evidence/parent-composed-settings-qualification.json>) passes scale 1/application scale 1.25 and own renderer inspection. Example tests pass 86 (83 shared app/component + three fixture guards); all runners pass 15 headless tests including 11 owned cleanup regressions. Normal production smoke remains write/reply-fenced. Physical input/IME/clipboard erasure, real-key validation, arbitrary-provider confinement and live Host approval delivery are not qualified. [Detailed boundaries](<../docs/NATIVE-COMPOSED-SETTINGS.md>) distinguish source-only side-effect audit from a network trace.

## Bounds and lifecycle

- Visible-only transcript: fixed 160 px cards, two-row overscan, ≤10 cards for a 600 px viewport regardless of retained history size. Excerpts: 240 Unicode codepoints/four lines; detail: 32 KiB, including explicit truncation marker.
- Transcript retention: 4096 records/16 MiB serialized event payload, checked before insertion. Live text: total 32 KiB and 256 blocks. A failed fold freezes further incremental mutation until a fresh snapshot.
- Transport: 4 MiB result/frame bound, 1 MiB per-stream queue, 8 streams, 4 concurrent RPCs, 64 pending stream events, 100,000 JSON items, 15 s RPC deadline. UI bridge queues: 32 entries; operations: 8; roster/sidebar limits are explicit in code.
- UI holds public DTOs/redacted metadata plus explicit private editable key/callback drafts; never launch URLs, authentication cookies, native account sessions or private runtime objects. Secret input is masked/opaque and move-only at persistence transfer. Core/account work and transport stay on the owner thread/runtime.
- One monotonic watch signal bypasses the business queue for every shutdown/startup phase. Owner Drop signals before joining. Teardown first requests owned Core stop/revocation, invalidates transport authority, aborts operations, cancels streams/closes transport, finishes Core stop, joins non-abortable blocking operations, and joins the metadata bridge after a terminal Core event.
- Known-idle close stops gracefully. Busy/unknown/failed inspection requires explicit confirmation. Uncertain stop is reported; Core's persistent owner retains containment responsibility, and the metadata bridge is not falsely joined forever. Exit anyway is explicit. No unowned process is signaled.

## Bounded, real, keyless GUI smoke (parent-owned execution)

This is opt-in, not normal application behavior. It creates **one real blank session** in an empty/new native home, opens only the supplied workspace, follows its real snapshot, then closes the owned Core at a one-shot deadline. User Send/catalog/model-selection and approval/question reply clicks are fenced during smoke. It never loads fixtures into runtime state.

```bash
./target/release/dsh-native-app \
  --runtime /absolute/explicit-fork/apps/cli \
  --expected-version 0.2.1-alpha.1 \
  --home /absolute/NEW-OR-EMPTY-private-native-home \
  --user-home /absolute/private-user-home \
  --cwd /absolute/existing-private-empty-workspace \
  --scale 1.0 \
  --smoke-new-session --exit-after-seconds 8 \
  --smoke-report /home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-native/app/evidence/NEW-report.json \
  --smoke-screenshot /home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-native/app/evidence/NEW-own-window.png
```

Deadline: 3–60 s; total process duration also includes Core's bounded teardown. Report must be a **new file inside this app's existing evidence directory**; created exclusively with mode 0600, never overwritten. It includes own PID/app ID, raw native display family, queried logical geometry/scales, counts/cursor/known event kinds, request counters and actual Core stop metadata. It includes no titles, session/workspace IDs, paths, excerpts, cookies, authorization URLs or raw errors. Status passes only after real workspace/session/snapshot, own Wayland handle, no model/catalog request and clean Core stop. Main joins the owner before treating a failed/missing report as nonzero exit. Paint is explicitly not claimed verified by this report; own compositor/renderer qualification is separate.

`--smoke-screenshot` is optional and smoke-only: a NEW `.png` exclusively inside app/evidence, distinct from the report, mode 0600, never overwritten. Exactly one own Iced renderer RGBA/sRGB frame is requested 750 ms after a **successful real session snapshot fold** and own window creation. No desktop capture/API/helper, no other windows, no benchmark data. PNG encoding runs off the GUI thread; RGBA validation is capped at 64 MiB. Report adds snapshot-fold validity, screenshot success and physical pixel dimensions, never a screenshot path. A requested but missing/failed screenshot fails smoke. `paintVerified` remains **false**: only parent inspection of the actual PNG may qualify appearance. Allow sufficient deadline time for startup, settling, capture and writing (e.g. 12 seconds).

## Deliberate unsupported areas

Device/sign-out/account-switching controls, timed foreground and continued question replies, plugin panels, terminal/document editors, rich attachments/images, session rename/fork and subagent steering remain unsupported; reversible pin/unpin/archive/restore have separate scripted Host qualification. Masked API-key settings and scripted actual GUI→worker→Core/default-provider PUBLIC-value persistence are implemented/qualified separately from the no-Host component fixtures. Physical keyboard/pointer/IME input and real-key/model validation remain unqualified. Timed questions require an owned `question_wait(AgentId, callId)` stream, Host-duration deadline and claim retention through Host acknowledgment; continued replies require authoritative late-answer state. This frontend does not invent either, open a claim, send a timeout or delegate. Unsupported requests remain bounded read-only cards; malformed/unknown requests fail closed. Model/account setup still requires separately supported backend configuration; no account engine is implemented in this frontend.

There is no automatic reconnect timer. A stream failure disables further unsafe action and requires explicit selection/restart. Runtime typography/IME, real model generation, integrated native-app performance and mapped Wayland/paint proofs are not established by keyless Rust tests.

## Parent-qualified real application

The parent qualified the earlier core/session/screenshot release with efficient Core-owned descendant traversal against the real isolated alpha Host. Decision/settings components have separate parent-owned public local renderer fixtures; real Host decision delivery, native keyboard/pointer input remain unqualified. Scripted actual GUI-to-worker/default-provider persistence now has separate [PUBLIC-dummy composed evidence](<../docs/NATIVE-COMPOSED-SETTINGS.md>) with real acknowledgment/private-file verification and owned cleanup; it is not input or real-key validation. [Parent evidence](evidence/parent-qualification.json) verifies owned mapped native Wayland, real workspace/session/snapshot folding, graceful exit zero and no observed descendants left at application scales 1 and 1.25. The parent inspected [scale 1 own frame](evidence/qualification-native-04-owned-traversal/own-window.png) and [scale 1.25 own frame](evidence/qualification-native-05-scale-125/own-window.png): readable real keyless policy records, sidebar/composer and standard dark Rosé Pine. This is not genuine compositor fractional scaling, full typography/IME/input/accessibility or full feature parity.

The [qualification notes](../docs/NATIVE-APP-QUALIFICATION.md) document a measured Core monitor idle-CPU regression and its correction: desktop-wide PID scanning was replaced by identity/pidfd-fenced owned descendant traversal. Short keyless whole-tree medians after the fix: CPU 0% at sampling resolution, ~327 MiB RSS / ~249 MiB PSS. No model prompts or catalog requests were made. These are UI **plus real Node Host** measurements, not synthetic benchmark numbers or long-chat performance claims.

## Actual-component public rendering fixtures

The separate developer-only [component fixture](<examples/dialog_smoke.rs>) path-imports the real app decision/settings modules, palette, font and own-buffer PNG encoder. It starts **no Node Host/client/worker/provider**, sends no RPC and uses only public fake requests/key text. Local scripted actions exercise actual reducers, simulate ACK/indeterminate receipts and immediately drop any constructed fake secret; these are not human clicks or real approvals/storage.

```bash
CARGO_HOME=/absolute/shared-cache CARGO_BUILD_JOBS=2 cargo build --offline --locked --release --example dialog_smoke
python3 ../scripts/qualify-dialog-components.py --output /absolute/native/app/evidence/NEW-private-run --scale 1.0
```

The runner clears ambient credentials, uses new private homes/output, matches only its owned Wayland PID/app ID, captures only the own Iced renderer, verifies clean exit/no observed children, and preserves false live-Host/keyboard-input/paint flags. Parent PNG inspection is a separate record. Modes: approval, question, unsupported timed request, masked settings and explicit save confirmation. It changes no desktop configuration or installed Harness data.

## Keyless evidence

[tests.log](<evidence/tests.log>), [check.log](<evidence/check.log>) and [release.log](<evidence/release.log>) record build/test commands. [alpha-session.json](<fixtures/alpha-session.json>) and [alpha-session.txt](<fixtures/alpha-session.txt>) are **test-only authored golden fixtures**, based on alpha DTO/compact stream definitions; not live runtime proof. They cover Unicode, surface replacement, tool plaintext, bounded/redacted unknown JSON and reconnect prefix behavior. Worker/UI tests cover priority shutdown under a full command queue, terminal metadata forwarding, unexpected channel closure, typed Send, draft retention, unknown inspection and smoke fencing. [Interaction fixtures](<fixtures/interactions.json>) and pure UI/worker tests additionally cover explicit reply encoding, incomplete/invalid answers, malformed/duplicate/unsupported requests, UTF-8 budgets, submitting/cancel/acknowledgment/error races, repeated ids, distinct Agent/Session identity, and independent UI/worker smoke and epoch fences. Parent-owned actual Core/transport and GUI results are separate evidence, never replaced by these fixtures.
