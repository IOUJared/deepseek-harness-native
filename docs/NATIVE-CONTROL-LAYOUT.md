# Native control layout

The user resumed implementation after the earlier shared-writer pause and requested fewer default paragraphs, small detail disclosures and Harness-style tool activity. The standalone native source now has one Settings entry, a More panel, collapsed secondary information and compact tool rows. Fresh execution and paint evidence is recorded below; historical manifests do not qualify this source.

## Latest refinement

[Native content-aware chat geometry](<NATIVE-CHAT-GEOMETRY.md>) supersedes the fixed 160px chat-preview spacing below. The new release uses measured 87–145px slots and narrow short-message user bubbles, while tools remain 40px and other raw roles 160px. Twenty PUBLIC captures plus actual keyless startup/teardown are separately qualified; the previous development archive remains frozen. The subsequent [reader-anchor refinement](<NATIVE-READER-ANCHORS.md>) adds generation/revision/feedback-fenced durable row restoration, with 24 source/model tests and a separate actual native operation probe. Its final capture qualification retains exact executables instead of mutable build targets; all twenty public frames match the fully inspected preceding snapshot. Production scroll/input coupling and real attachment effects remain separate gates.

## Observed baseline

The [current native own-window screenshot](<../app/evidence/purposeful-layout-epxodggq/before/own-window.png>) shows two Settings entries (header and sidebar), a Manage/Settings/Info header cluster, persistent technical status, and account/shutdown controls next to everyday navigation. This is a real isolated-keyless native startup capture, not a desktop screenshot. Its binary/source provenance is recorded in [baseline evidence](<../app/evidence/purposeful-layout-epxodggq/baseline.json>) and its startup/teardown checks in [the run report](<../app/evidence/purposeful-layout-epxodggq/before/result.json>). The binary is the preceding release; concurrently changing source is not claimed to match that screenshot.

## Reference layouts and what to borrow

- [Current OpenAI desktop Settings documentation](https://learn.chatgpt.com/docs/reference/settings.md) consolidates preferences into an application settings panel. [Project documentation](https://learn.chatgpt.com/docs/projects.md) groups chat search, projects and recent conversations in the sidebar. Borrow separation of navigation from preferences, not unsupported product features.
- [Claude Desktop integration documentation](https://support.claude.com/en/articles/10949351-getting-started-with-local-mcp-servers-on-claude-desktop) separates configuration in Settings, tool use at the composer, and technical connection diagnostics in Developer settings. Borrow this use/configure/debug distinction.
- [LM Studio UI modes](https://lmstudio.ai/docs/app/user-interface/modes) explicitly describes a simpler User interface and advanced Developer controls behind Settings. Its [chat documentation](https://lmstudio.ai/docs/app/basics/chat) puts duplication behind conversation-local more actions. The [official reference image](https://lmstudio.ai/assets/marketing/docs/chat.png), [retained locally](<../app/evidence/purposeful-layout-epxodggq/references/lmstudio-chat.png>), was pixel-inspected: chat/project navigation is left, conversation-specific actions are contextual, model choice is separate from message content, and composition stays at the bottom. The image itself labels LM Studio **0.3.2**; it is a historical official illustration, not proof of the latest app's exact appearance.

The placement reasons below are design inference, not statements from the original designers or measured usability superiority. No reference app/account was launched. Only our native app's own renderer was captured.

## Placement inventory

| Element | Purpose | Placement/reason |
| --- | --- | --- |
| Brand and collapse/Menu | Identify the app and recover navigation at narrow sizes | Sidebar top; preserve finite, visibility-gated native motion |
| New conversation | Start independent work | Beside navigation, not mixed with backend setup |
| Search, recent conversations, archive view | Find/resume stored work | One sidebar history region; archive is a contextual history filter |
| Workspaces disclosure | Select/open the filesystem work context | Grouped with navigation; hide directory controls until requested |
| **Settings** | Application preferences, API login, Codex and plugins | **One stable footer entry**; the expanded sidebar or compact rail uses the same action and label, never both or an additional header shortcut |
| Conversation title and running indication | Identify the selected work and its observed state | A short header; avoid repeating the model selector here |
| More options | Less-frequent conversation/application actions | One explicit secondary disclosure; Manage opens the existing reviewed pin/archive/export flow; Information contains capabilities/diagnostics; Close retains owned-backend lifecycle semantics |
| Transcript and detail disclosure | Distinguish user/assistant work from tool activity | User bubble right, assistant left; slim muted-grey 40px single-line tool summaries with a small detail chevron. Other records retain 160px cards; the same raw-row height function drives visible ranges, spacers and older-page anchors |
| Editor | Compose without accidentally submitting | Bottom composer; Ctrl+Enter remains explicit Send, Enter remains a new line |
| Model/catalog and reasoning controls | Choose request-affecting options | Near composition; all provider catalog/model operations remain explicit, with no automatic loading |
| Send | Submit the user's draft | Primary composer action; preserve all current guards |
| Stop | Request cancellation of selected work | Composer action, or fixed header/toolbar when composition is replaced by a local panel; do not infer safety from a stale idle summary |
| Status/diagnostics | Explain progress, failures and uncertainty | Suppress only exact, controlled routine-success copy from the normal header; full status remains in Information. Preserve every warning, unknown/error, refusal and indeterminate result |
| Decisions | Explicitly allow/reject/answer pending owned requests | Preserve current cards and isolated decision sheets, tickets and acknowledgement semantics |

Optional/legacy Account metadata is not ChatGPT/Codex login state. Move it to Information with an explicit label rather than displaying a confusing global SignedOut badge. ChatGPT connection status belongs in Settings → Codex.

Compact navigation's Done/Stop toolbar should be fixed outside its scrollable body. Shorter keyboard hints and separately laid-out model/reasoning controls should reserve action space at narrow heights. Long catalog labels must not be silently changed into ambiguous model identities.

## Progressive disclosure and conversation

General appearance and shortcut explanations, API help/privacy, Codex recovery and About Harness features/diagnostics are collapsed behind small labelled chevrons. Consent, replacement/no-validation warnings, confirmations, retry uncertainty, errors and active cancellation stay visible. More and Info messages are rejected when their actual controls are covered by navigation or modal panels; disclosures never issue business operations or erase the conversation draft.

The same-version Harness [ToolRow source](<../../deepseek-harness-linux/packages/client/ui-tool/src/client/tool/components/ToolRow.tsx>) separates a compact secondary summary from lazily requested input/output. Native [activity summaries](<../app/src/activity.rs>) borrow that hierarchy, not React/webview implementation. Read/edit/write rows name a file; shell rows show a bounded safe command. Full arguments/output and timestamps require the local read-only detail pane. Exact recorded error flags remain marked in collapsed rows; a recorded call/result never implies execution or success. Pairing uses exact alpha call ID, source, turn and step metadata, with generic fallback for missing/ambiguous data. Known secret-field/command suppression is conservative, not a general secret detector. Durable records, surface folding, sequence identity and source truncation are unchanged.

## Safety and verification

Cancellation admission stays unchanged: selected work may be active before roster.running catches up, and prompt/cancel acknowledgements do not prove idle. Cosmetic simplification must not hide the only cancellation path. Modal backgrounds remain inert; no new exemption allows stale menu messages to bypass settings/management/export/decision fences.

Qualification uses serialized locked/offline Cargo with two jobs and the feature-gated [actual-App public fixture](<../app/src/layout_fixture.rs>) plus [own-renderer runner](<../app/examples/layout_smoke.rs>). Its public presets cover wide, 760×560 and embedded 608×448 logical frames (including application scale 1.25), menu/navigation, long model/reasoning, warning, General/Codex, collapsed/expanded information and tool activity. Widget operations measure only known container bounds; inactive settings backgrounds are painted but excluded from operations. A public fixture has no App subscription, worker start or effect executor, counts and discards captured Commands, and drops widget messages. Programmatic disclosure tests are not physical input.

### Latest execution evidence

The [final qualification](<../app/evidence/control-layout-fyk2oz31/qualification.json>) pins production SHA-256 `e93345bd6921e6a58b6dfe52fb8b02caa02ba082cb3b104f55db87cd0a57b19c` (15,415,656 bytes), separate fixture hash and app-source hashes. Formatting, feature all-target checks, 314 default app tests, 316 feature app tests, 318 fixture-example tests and both optimized builds passed. All 19 public rendering/known-control-bound cases and one actual isolated keyless Wayland startup/real snapshot/graceful teardown passed, with zero catalog/prompt requests. Parent paint inspection covers changed final frames directly and unchanged frames by byte identity with the directly inspected prior capture. [Harness-style conversation](<../app/evidence/control-layout-fyk2oz31/tool-activity/own-window.png>), [requested tool details](<../app/evidence/control-layout-fyk2oz31/tool-activity-details/own-window.png>), [short General settings](<../app/evidence/control-layout-fyk2oz31/settings-general/own-window.png>) and [real keyless native app](<../app/evidence/control-layout-fyk2oz31/real-keyless/own-window.png>) show the actual renderer, not image generation.

The supplied user screenshot prompted the final change from rounded tool cards to slim single-line grey disclosures. Input and output records remain separately inspectable, with an Output/error suffix that cannot disappear behind a truncated tool name; records are not merged, discarded or treated as proof of execution. Details currently open a local read-only pane rather than the Web UI's inline rich diff/read renderer. Tiny scrollable content may be below the fold, while Settings and the applicable Stop/Send controls stay inside measured frames.

An embedded requested frame is not compositor fractional-scaling qualification. No physical pointer/keyboard/IME, accessibility, integrated performance, real-account/model generation or full-parity claim follows from these checks. The installed Web GUI, real credentials/profiles, desktop settings and historical development package remain unchanged.
