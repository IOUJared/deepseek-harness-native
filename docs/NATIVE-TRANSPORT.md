# Native Linux transport reference and minimal vertical slice

## Scope and evidence

Historical **source-audited plan**, not implementation/startup evidence. Target: isolated alpha `0.2.1-alpha.1`; installed Inspect was rc2, Client Inspect timed out. Neither proves alpha equality. Later implementation qualifications have their own fingerprints.

## Architecture decision

- Native Rust owns private lifecycle/transport; Node owns AgentLoop, persistence, models, tools, approvals and plugins.
- Use authenticated HTTP ClientRPC/Gateway mux, **not arbitrary JS/service dispatch or a second backend**.
- Keep full same-version `dsh-base` + `dsh-web-app` composition; no required HTML, React or Client JS execution.

### Private startup and HTTP-cookie exchange

- Explicit validated fork/version + isolated home; public `runProfile` boot with `--no-open --host 127.0.0.1 --port 0`. Never installed-runtime fallback or `dsh desktop`.
- Privately validate ready URL: exact `http://127.0.0.1:<port>/`, expected version, one nonempty token, no userinfo/fragment.
- GET **exact ready URL**, redirects disabled → **303**, `Location: ./`, opaque `Set-Cookie`; do not fetch HTML. No `/api/login` or bearer endpoint.
- Cookie: authority-derived `dsh-auth-<base64url SHA256>`, signed `v1` value, HttpOnly/SameSite=Strict/Path=/; loopback HTTP has no Secure. Do not manufacture it.
- Every request/upgrade uses private Cookie and exact Origin; Host/Origin/Fetch-Metadata fences apply. Cookie grants **operator authority**. Destroy launch token/URL after exchange.
- Fixed allowlisted paths/current generation only; no proxies, external redirects, arbitrary DNS or forwarding cookies to tools/model/document URLs. Credentials stay outside GUI/logs/serialization; optional approved Account Guest is separately scoped.

## Physical routes and envelopes

[Carrier source](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/client/connection/src/rpc-host.ts>): **not JSON-RPC 2.0**.

| Purpose | Route | Body |
|---|---|---|
| Cookie | GET `/?token=<private>` | 303 exchange |
| Unary | POST `/api/<namespace>/<method>` | ClientRPC JSON |
| Streams | WS `/api/remote.mux` | UTF-8 JSON |
| Event outcome | POST `/api/$events/result` | ClientRPC |
| File | POST `/api/session/uploadFileBinary?sessionId=<id>&name=<encoded>` | Raw octet-stream; result-only JSON |

```json
{"type":"client-request","rpcId":"rpc-1","method":"session/list","payload":{"args":{"_request":{}}}}
{"type":"server-response","rpcId":"rpc-1","result":{"ok":true,"value":{"items":[]}}}
```

- Verify echoed `rpcId` and exact URL/method match; args use authored names, identities replace lookup parameters, signals omitted.
- Void success may omit `value`. Business failures normally use HTTP 200 with `ok:false,error:{code,message,details}`; inspect status/content type first. Explicitly reject unsupported multipart or implement bounded parsing.

### Service keys are not wire namespaces

| Service key | Namespace |
|---|---|
| `workspaceController` | `workspace` |
| `sessionController` | `session` |
| `userQuestions` | `userQuestions` |
| `fileUploads` | `fileUploads` |
| `typertGateway` | `$events`, `$events/result` |
| `accountController` | `account` |

Private supervisor service access does not authorize arbitrary Remote calls; account BFF does not expose raw Platform credentials.

### WebSocket carrier

```json
{"type":"open","streamId":"g1-workspaces","endpoint":"workspace/follow","payload":{"args":{}}}
{"type":"item","streamId":"g1-workspaces","value":{"type":"baseline","value":{"items":[],"archivedSessionIds":[],"pinnedSessionIds":[]}}}
{"type":"cancel","streamId":"g1-workspaces"}
```

- Client: open/item/end/cancel; server: item/end/error. `end` is direction-dependent; read streams have no uplink items. Logical cancellation need not ACK; retire locally.
- IDs are generation-specific. Duplicate active IDs close socket; trailing retired frames drop. Binary closes 1003, malformed protocol 1008.
- Ping→Pong; Host heartbeat **2000ms**, termination after two missed opportunities. Upgrade shares Cookie/Host/Origin admission; no WS auth message or HTTP subscribe.

## Twelve primary operations: serialized examples

Compact synthetic args below; wrap unary args in the ClientRPC envelope or stream args in `open`. [Session declarations](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/api/session-controller/src/types.ts>) remain authoritative.

### 1. Workspace list/watch — `workspace/follow` (WS)

`args:{}`. Atomic baseline, then upsert/remove/order/archived/pinned frames. Preserve supplied order; no unary `workspace/list`.

### 2. Adopt/open workspace directory — POST `/api/workspace/create`

`args:{request:{path:"/tmp/native-example"}}`. Adopts an **existing directory**, not mkdir/opener; explicit picker gesture. `created:false` means existing registration.

### 3. Cold-safe session roster — POST `/api/session/list`

`args:{_request:{}}`, **not request**. Required `agentAvailable` differs from `running`; no resume. Cursor is currently ignored: all visible rows, no invented pagination/nextCursor.

### 4. Create/adopt session — POST `/api/session/create`

`args:{request:{workspaceId,sessionId,agentPreset}}`. `workspaceId` and `cwd` are exclusive; ID may be Host-minted. Stable IDs support explicit reconciliation; ownership/cwd/preset conflicts fail. Workspace attachment may fail **after creation**: reconcile roster before retry.

### 5. Open session and watch journal — `session/follow` (WS)

`args:{request:{address:{kind:"session",sessionId},maxMessages:50,assistantStream:true}}`.

Snapshot replaces retained window; events continue from `cursor+1`, empty cursor can be `-1`. **No request resume cursor**. Follow activates cold ordinary Agents after snapshot. Subagent address uses parent/child IDs and mode, not ordinary prompt authority.

### 6. Earlier history — POST `/api/session/page`

`args:{request:{address,throughSeq,beforeSeq,maxMessages:50}}`. Opening cursor is inclusive cut; optional turnWindow aligns turns. Message count is not byte/record cap. Prepend chronologically/deduplicate seq; reject old-generation pages.

### 7. Submit user turn — POST `/api/session/prompt`

`args:{request:{requestId,sessionId,mode:"queue",content:[{type:"text",text:"PUBLIC example"}],clientTimeZone:"UTC"}}`.

Stable requestId persists as `source.rpcId`, distinct from transport rpcId; retry preserves identity/content. Queue→followup; steer→next step. ACK means inbox admission, **not completion**. Reconcile durable echoes; blank content needs attachment. Time zone: UTC or valid IANA Area/Location.

### 8. Model catalog — POST `/api/session/modelCatalog`

`args:{}`. Provider/model-owned reasoning metadata and isolated `failures`; no credentials. Refresh on `llm/adapters-updated`/safe account changes.

### 9. Select session model — POST `/api/session/selectModel`

`args:{request:{sessionId,provider,model,reasoningEffort}}`. Resumes/validates route for **next** model request; background default-save may fail after success. Projection distinguishes lastUsed/next.

### 10. Cancel active generation — POST `/api/session/cancel`

`args:{request:{sessionId}}`. Attached ordinary Agent required; immediate `accepted:true`, keepInbox=true. Observe turn/end/status; queued work may run later. WS unsubscribe is **not model cancellation**; supervisor hook cancellation has different idle-wait semantics.

### 11. Subscribe to application listeners — `$events` (WS)

`args:{}`. Ready provides generation clientId; emit uses positional args; waterfall carries eventId/actual agentId/request without Agent/signal objects. Business Cancel is an item value, not carrier cancel. Pending waterfalls can replay; emits are not durable.

### 12. Approval/question waterfall reply — POST `/api/$events/result`

`args:{clientId,eventId,outcome}` directly, no request wrapper. Outcomes: result/next/rejected. Approval values: `allowed-once`, `rejected`, `cancelled`, `unavailable`; never persistent grant. Question value: `{answers:[{id,selected:["offered label"],custom?}]}`. Current received delivery + explicit action required; unsupported listeners delegate `next`. Void ACK/stale no-op is not business settlement.

## Required supplementary capabilities

### Timed and continued questions

- Wire uses `multiSelect`, not tool `multi_select`; optional `wait:{callId,timed?}`.
- `userQuestions/attachWait` stream args `{agentId,callId}`; first `{remainingMs}` is claim acquisition, not open. No claim-before-item, fabricated deadline or automatic delegation. Retain owner through Host termination, beyond reply ACK. [Native timed UI remains unsupported](<NATIVE-TIMED-QUESTIONS.md>).
- Timeout rejection code `ASK_TIMED_OUT`, name `UserQuestionError`; release is not answer/cancellation.
- Continued answer: `userQuestions/answer`, args `{agentId,callId,answer}`. Exact live root only; complete ID set required. `true` means queued, false means unavailable, `REPLY_QUEUED` means already queued. Reconcile inbox/projection; no stale waterfall reply.

### Attachments

| Intake | Form |
|---|---|
| Image | `{type:"image",mediaType,data:<base64>,name?}`; PNG/JPEG/WebP/GIF |
| File | Raw upload → Host receipt → `{type:"file",receiptId}` |
| Base64 fallback | `fileUploads/upload`, args `{agentId,request:{data,name?}}` |
| Reachable image read | `session/attachment`, args `{request:{sessionId,attachmentId}}` |

Raw upload returns `{ok:true,value:{receiptId,file:{attachmentId,name,bytes}}}`, **not ClientRPC**. Receipts are live-Session scoped, bind to prompt requestId and retire with owning lifecycle; ACK alone creates no message. Never guess paths/IDs. Respect imageLimits/modality. No ambient file reader or generic preview authority.

### Tools, status, projections, and event tree

- Subscribe `$events` before roster; buffer/reconcile added/removed/status/activity/error emits. No atomic roster-plus-events baseline.
- `session/control` baseline contains **projections only**, not queues/approvals/questions. `session/projections` is cold-safe and nullable.
- Tree: turn/step/message/attempt/tool events by durable seq/business IDs; raw tool args are JSON **strings**, paired by callId. Never evaluate scripts/HTML/meta.
- `surfaceOp` append/replace and sourceEventSeqs govern display order. Unknown required semantics fail closed; ignorable unknowns may be skipped. Session migration stays backend-owned.
- Assistant stream start/chunk/end uses attempt/revision/dense indices, **not durable seq**. Replace transient text at settlement; discard abandoned attempts/rebaseline discontinuity. [Settlement fold](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/api/session-controller/src/client/sessions/assistant-stream.ts>). Durable-only presentation is honest non-streaming, not parity.

## DTO discipline

Exact discriminants/spelling, identity newtypes, safe integers, missing≠null, bounded recursively validated inert JSON. Define full headers/streams/catalog/frames from owning alpha types, not flattened strings. Unknown plugin payloads remain bounded until reviewed presenters exist.

## Native lifecycle, limits, and unsafeActions guard

- Historical `unsafeActions` is a **proposed native guard**, not alpha field/service/mode. Closed action enums/DTOs/gestures; no endpoint/eval/subprocess/credential input from buttons. Runtime/plugin/profile/filesystem/terminal/inspect/Client-JS operations remain outside initial allowlist.
- Bind origin/cookie/socket/RPCs/clientId/receipts/followers to one private generation; restart invalidates all and reacquires authentication. Reject late callbacks.
- Navigation releases observations, not Sessions/Agents. Whole-app close suppresses publication, closes reads, awaits actual Host disposal/child exit, then releases secrets; kill request is not quiescence.
- Bound complete frames **before decoding/widget publication**, then nesting/items/retention. Proposed budgets, **not current defaults**: 8 MiB unary/WS, 16 MiB selected retention, 10,000 roster rows; bounded counts/timeouts. Oversized authoritative state fails visibly, never silent truncation.
- Server limits differ: buffered HTTP request **300 MiB**, mux uplink **262,144 bytes**; neither bounds outbound snapshots. Native caps cannot prevent Node constructing an unpaginated oversized roster. Redact secrets before diagnostic truncation; only display previews may truncate without falsifying continuity.

## Minimal native vertical slice

- Private boot/auth → `$events` ready + workspace/control baselines → buffered roster installation.
- Explicit workspace/session mutations → selected follow/page → virtualized durable chat → exact stream fold.
- Model route/prompt identity → upload receipts → explicit approval/question handling → distinct Stop.
- Reconnect replaces baselines; restart reacquires capabilities; shutdown awaits disposal. No new Cordis plugin or mandatory web rendering required.

## Source fixtures and acceptance evidence (not executed here)

| Source fixture | Use / limit |
|---|---|
| [Public URL](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/apps/cli/tests/profiles/web/tests/public-url.expected.e2e.ts>) | Public-profile 303/auth/foreign-Host acceptance |
| [Built Remote](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/api/remotes/tests/built-lib.e2e.ts>) | Built carrier; hand-built test Host, not native startup |
| [File upload](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/apps/web/tests/file-upload-round.e2e.ts>) | Keyless upload→prompt→read; honor versioned snapshot ownership |
| [Interactions](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/apps/web/tests/live-interactions.e2e.ts>) | Queue/cancel/error replay |
| [Approval](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/apps/web/tests/approval-composer.e2e.ts>) / [questions](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/apps/web/tests/question-composer.e2e.ts>) | One-shot/blocking/timed/continued/cancelled composition |
| [Assembled fixture](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/apps/web/tests/fixtures/assembled-remote.fixture.json>) | **Stale alpha shapes:** missing agentAvailable, legacy control fields |

Future acceptance: exact golden envelopes/correlation, trust/heartbeat/cancel, baseline/page races, prompt identity, settlement deduplication, receipts, decisions/claims, restart/privacy/bounds/quiescence. No tests were executed for this historical audit. Ignored local evidence elsewhere is **not a GitHub download**.

## True limits: tools and plugins

Host composition can retain tools/jobs/subagents/goals/schedules/skills/MCP when available; browser Slots/React/terminal/Office/plugin callbacks do **not** become native widgets. Optional browser-disabled tools stay unavailable; Office was unavailable/not-configured. Generic bounded inert text/JSON is not specialized UI parity. New Remote/projection/event support requires reviewed DTOs/actions; Client notifications never authorize JS evaluation. Credentials/config/plugin management remain outside this initial allowlist.
