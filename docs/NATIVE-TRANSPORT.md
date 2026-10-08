# Native Linux transport reference and minimal vertical slice

## Scope and evidence

This is a source-audited transport plan for a Slint or Iced primary UI, not a WebView or Electron carrier. Rust owns private process/network state; the existing Node Host owns AgentLoop, persistence, model adapters, tools, approvals, account grants, and plugins. No Rust implementation, runtime configuration, plugin installation, build, test, model call, Host launch, or GUI launch was performed for this document.

The authoritative target is the isolated [Linux fork manifest](<../../deepseek-harness-linux/package.json#L1-L9>), version `0.2.1-alpha.1`. Read [root guidance](<../../deepseek-harness-linux/AGENTS.md>), [package guidance](<../../deepseek-harness-linux/packages/AGENTS.md>), [documentation guidance](<../../deepseek-harness-linux/docs/AGENTS.md>), [architecture](<../../deepseek-harness-linux/docs/architecture.md>), and [lifecycle rules](<../../deepseek-harness-linux/docs/defensive-patterns.md>). Live Host Inspect was queried for the Service directory and `sessionController`; that is installed `0.2.0-rc.2`, not this fork. Client Inspect timed out; no reconnection was attempted. Neither installed Inspect nor browser fixtures establish alpha-schema equality.

The previously missing dualface built artifact was reported repaired by the parent during this audit; that is not a verification performed here. The parent owns source-runtime smoke and graphical benchmarking. This document makes no new startup-success claim.

## Architecture decision

Use the existing authenticated public BFF: HTTP unary ClientRPC plus the Gateway WebSocket mux. Do not introduce a generic privileged JS invocation bridge, arbitrary service dispatch, inline JavaScript, or a second agent backend. The private [fork supervisor](<../../deepseek-harness-tauri/runtime/fork-supervisor.mjs#L195-L349>) remains responsible only for startup/readiness, isolated profile/account ownership, native account commands, task inspection, and shutdown. Normal sessions/chat/models/cancellation use the public transport below.

Node still loads the existing `dsh-base` and `dsh-web-app` profile bundles; the native UI need not fetch the HTML shell, instantiate React, materialize Client plugins, or execute Client JS. Host plugins and tools remain operational subject to their composed dependencies. Native rendering is independently implemented from typed wire data, not from WidgetProps or arbitrary plugin source.

### Private startup and HTTP-cookie exchange

1. Rust spawns the existing supervisor with an explicit built fork anchor/version and an isolated home, never the installed CLI or shared profile. The [loader](<../../deepseek-harness-tauri/runtime/fork-supervisor.mjs#L98-L145>) verifies package versions and profile isolation. It launches through `runProfile`, with `--no-open --host 127.0.0.1 --port 0`; the OS chooses the port.
2. Consume the inherited-pipe `ready` packet privately: `{type:"ready",url,version,profile:"desktop",optionalPayloads}`. The [ready URL validator](<../../deepseek-harness-tauri/runtime/supervisor.mjs#L188-L198>) requires `http://127.0.0.1:<port>/` origin, no userinfo, and the authenticated URL. Rust additionally validates one nonempty `token` query and the expected version before admitting any network request. Never publish this URL to UI state, logs, error messages, screenshots, or diagnostic snapshots.
3. Issue **GET of the exact ready URL**, disabling automatic redirects. Expect `303`, `Location: ./`, and `Set-Cookie`; do not load the subsequent HTML. Retain only the private cookie pair and clean origin. [BrowserAuth](<../../deepseek-harness-linux/packages/client/connection/src/browser-auth.ts#L218-L279>) requires GET `/`, exactly one process launch token, and derives an authority-bound signed cookie. There is no separate `/api/login` or bearer-token endpoint.
4. The cookie name is `dsh-auth-` plus base64url SHA-256 of the request authority; its value is signed `v1.<payload>.<signature>`. Treat it as opaque. Attributes are `HttpOnly; SameSite=Strict; Path=/`, with configured expiry/Max-Age, and no `Secure` on this loopback HTTP deployment. Do not manufacture a cookie or infer a port-independent name. The signing secret is persisted by the isolated credential provider; the launch token is process-owned. [Cookie implementation](<../../deepseek-harness-linux/packages/client/connection/src/browser-auth.ts#L100-L215>).
5. Add the same private `Cookie` to every HTTP request and WS upgrade, and send `Origin: http://127.0.0.1:<port>` consistently. This is a native same-origin client, not CORS. The [trust fence](<../../deepseek-harness-linux/packages/client/connection/src/api-request-trust.ts#L85-L118>) checks Host on every request, rejects cross-site Fetch Metadata and mismatched/opaque Origin; [admission](<../../deepseek-harness-linux/packages/client/connection/src/rpc-host.ts#L104-L123>) returns 403 for trust failure, 401 for unauthenticated requests, otherwise the operator Peer. Cookie admission grants operator authority, not a harmless read-only role.
6. Destroy the launch URL/token after exchange. Keep cookie and any raw `platform-session` token/headers only in Rust private state, outside serialization and native widget data. An intentionally approved, isolated native Account Guest may consume platform credentials through a separately scoped private path; that is not permission to pass them to the primary GUI or a general Web engine.

The ready URL is a capability; validating loopback once is insufficient. Every request/upgrade must resolve a fixed, allowlisted path against the current clean origin, enforce the exact generation's authority, disable cross-origin redirects, and reject external/userinfo/fragment URLs. Do not forward cookies to model URLs, tools, plugin content, document links, or arbitrary user-supplied endpoints.

## Physical routes and envelopes

The [Connection HTTP implementation](<../../deepseek-harness-linux/packages/client/connection/src/rpc-host.ts#L222-L311>) and [envelope validators](<../../deepseek-harness-linux/packages/client/connection/src/rpc-schema.ts>) define these fields. This is **not** JSON-RPC 2.0.

| Purpose | Exact route/method | Encoding |
|---|---|---|
| Startup cookie | GET `/?token=<private launch token>` | 303 exchange above; not a ClientRPC envelope |
| Unary Remote | POST `/api/<namespace>/<method>` | `Content-Type: application/json`, ClientRPC envelope |
| All logical Remote streams | WS `ws://127.0.0.1:<port>/api/remote.mux` | Cookie-authenticated upgrade, UTF-8 JSON text frames |
| Forwarded waterfall outcome | POST `/api/$events/result` | Ordinary ClientRPC envelope; special payload below |
| Generic file bytes | POST `/api/session/uploadFileBinary?sessionId=<id>&name=<encoded name>` | `application/octet-stream`; raw bytes, result-only JSON |

Unary request example:

```json
{"type":"client-request","rpcId":"rpc-create-1","method":"session/create","payload":{"args":{"request":{"workspaceId":"workspace-example","sessionId":"session-example","agentPreset":"standard"}}}}
```

Unary response examples:

```json
{"type":"server-response","rpcId":"rpc-create-1","result":{"ok":true,"value":{"sessionId":"session-example","agentPreset":"standard"}}}
{"type":"server-response","rpcId":"rpc-create-1","result":{"ok":false,"error":{"code":"workspace/not-found","message":"workspace not found","details":{"workspaceId":"workspace-example"}}}}
```

`rpcId` is client-minted transport correlation and must be echoed/checked. `method` must exactly equal the URL endpoint without `/api/`; arbitrary extra endpoint segments or mismatches are not accepted. `payload` is exactly `{args:{...}}`; args keys are **authored parameter names**, except lookup parameters become identities and cancellation signals are omitted. [Parameter generator](<../../deepseek-harness-linux/packages/typert/generator/src/analyzer.ts#L1105-L1147>) and [Gateway parser](<../../deepseek-harness-linux/packages/api/gateway/src/index.ts#L1109-L1146>). Important: `session/list` uses `_request`, not `request`; the [alpha declaration](<../../deepseek-harness-linux/packages/api/session-controller/src/index.ts#L252-L261>) and generated [descriptor](<../../deepseek-harness-linux/packages/api/session-controller/lib/typert.host.js#L1174-L1193>) agree.

Successful void methods serialize `result:{ok:true}` with absent `value`, not necessarily `null`. Business/gateway failures normally return a JSON envelope at HTTP 200. HTTP 400/404/415/500 and trust/auth/size failures are transport failures; parse status/content type before parsing a response envelope. The general carrier also supports multipart binary results (`metadata` plus `bytes-N` parts and attachment paths); none of the twelve primary methods below requires that mode. Either reject unsupported multipart explicitly or add bounded parsing before exposing filesystem/terminal APIs.

### Service keys are not wire namespaces

| Host service key | Public Remote namespace | Native use |
|---|---|---|
| `workspaceController` | `workspace` | Workspace commands and follow stream |
| `sessionController` | `session` | Session commands, roster, history, models, attachments, projections/control |
| `userQuestions` | `userQuestions` | Agent-scoped timed claim and continued answer |
| `fileUploads` | `fileUploads` | Agent-scoped base64 upload fallback; raw route remains under session |
| `typertGateway` | Gateway-owned `$events` and `$events/result` | Forwarded event stream and correlated outcomes, not generic Service calls |
| `accountController` | `account` | Exists in alpha; safe account UI routes are outside the twelve-operation chat minimum |

The private supervisor calls `deepseekAccount`, `credentials`, `settings`, and task/workspace owners directly through its fixed native command enum. Those private Host service keys are not instructions to add arbitrary native Remote calls. The [account BFF](<../../deepseek-harness-linux/packages/api/account-controller/src/index.ts>) exposes safe view/commands, not the supervisor's raw Platform session credentials.

### WebSocket carrier

[Exact stream types/parser](<../../deepseek-harness-linux/packages/api/gateway/src/stream-protocol.ts#L234-L337>) and [stream server](<../../deepseek-harness-linux/packages/api/gateway/src/stream-server.ts#L146-L291>) are authoritative. There is no `subscribe` HTTP method, WS rpcId, or custom WS authentication message.

```json
{"type":"open","streamId":"g1-workspaces","endpoint":"workspace/follow","payload":{"args":{}}}
{"type":"item","streamId":"g1-workspaces","value":{"type":"baseline","value":{"items":[],"archivedSessionIds":[],"pinnedSessionIds":[]}}}
{"type":"end","streamId":"g1-workspaces"}
{"type":"error","streamId":"g1-workspaces","error":{"code":"gateway/service-unavailable","message":"service unavailable","details":{}}}
{"type":"cancel","streamId":"g1-workspaces"}
```

Client messages are `open`, optional uplink `item`, uplink half-close `end`, or logical `cancel`. Server messages are `item`, terminal `end`, or terminal `error`. The same `end` name has direction-dependent meaning. The primary read streams send no uplink items. Normal cancellation need not produce a terminal acknowledgment; retire local state when cancelling. Duplicate active stream IDs close the socket; trailing items/end/cancel for an already removed stream are dropped. Binary messages close 1003; malformed protocol closes 1008. Distinct IDs belong to distinct physical generations.

Answer WebSocket Ping with Pong; the default Host heartbeat is 2,000 ms and it terminates after two missed heartbeat opportunities. [Gateway defaults/admission](<../../deepseek-harness-linux/packages/api/gateway/src/index.ts#L130-L264>) and [heartbeat lifecycle](<../../deepseek-harness-linux/packages/api/gateway/src/stream-server.ts#L40-L124>). WS admission uses the same Cookie/Host/Origin fence as HTTP and waits for launcher readiness. No native GUI readiness polling needs to boot a second server.

## Twelve primary operations: serialized examples

The following examples are synthetic non-secret identities/content consistent with alpha types, not captures from the installed rc2 runtime. Each request line is either the **complete HTTP body** for the indicated POST route or a **complete WS frame**. Each answer line is the matching complete envelope/frame; example values illustrate legal fields, not installed models, paths, or durable IDs. DTO names refer to [Session types](<../../deepseek-harness-linux/packages/api/session-controller/src/types.ts>) and [Workspace types](<../../deepseek-harness-linux/packages/api/workspace-controller/src/types.ts>).

### 1. Workspace list/watch — `workspace/follow` (WS)

Service `workspaceController`, Remote namespace `workspace`; no unary `workspace/list`. Every generation starts with a complete baseline.

```json
{"type":"open","streamId":"g1-workspaces","endpoint":"workspace/follow","payload":{"args":{}}}
{"type":"item","streamId":"g1-workspaces","value":{"type":"baseline","value":{"items":[{"workspaceId":"workspace-example","path":"/tmp/native-example","title":"native-example","sessionIds":["session-example"],"createdAt":"2026-10-06T00:00:00.000Z","updatedAt":"2026-10-06T00:00:00.000Z"}],"archivedSessionIds":[],"pinnedSessionIds":[]}}}
```

Later frame values are `{type:"upsert",workspace}`, `{type:"remove",workspaceId}`, `{type:"order",workspaceIds}`, `{type:"archived",archivedSessionIds}`, or `{type:"pinned",pinnedSessionIds}`. Preserve order; replace baseline atomically on reconnect. DTO: `WorkspaceFollowFrame`. [Owner](<../../deepseek-harness-linux/packages/api/workspace-controller/src/index.ts#L189-L197>).

### 2. Adopt/open workspace directory — POST `/api/workspace/create`

This adopts an **existing** directory; it is not filesystem mkdir or a native file-manager opener. Select an existing workspace locally; adopt a directory from an explicit native picker gesture.

```json
{"type":"client-request","rpcId":"rpc-ws-1","method":"workspace/create","payload":{"args":{"request":{"path":"/tmp/native-example"}}}}
{"type":"server-response","rpcId":"rpc-ws-1","result":{"ok":true,"value":{"workspace":{"workspaceId":"workspace-example","path":"/tmp/native-example","title":"native-example","sessionIds":[],"createdAt":"2026-10-06T00:00:00.000Z","updatedAt":"2026-10-06T00:00:00.000Z"},"created":true}}}
```

DTOs: `WorkspaceCreateRequest/Value`; idempotent existing registration returns `created:false`. [Owner](<../../deepseek-harness-linux/packages/api/workspace-controller/src/index.ts#L80-L88>).

### 3. Cold-safe session roster — POST `/api/session/list`

```json
{"type":"client-request","rpcId":"rpc-list-1","method":"session/list","payload":{"args":{"_request":{}}}}
{"type":"server-response","rpcId":"rpc-list-1","result":{"ok":true,"value":{"items":[{"agentAvailable":false,"sessionId":"session-example","updatedAt":1791244800000,"running":false,"blank":true,"cwd":"/tmp/native-example"}]}}}
```

DTOs: `SessionListRequest/Value`, `SessionSummary`. Required `agentAvailable` means live Agent ownership, distinct from `running`. Optional fields: `parentSessionId`, `origin:"subagent"`, `cwd`, `projections:{kind:"cached"|"sequenced",asOfSeq,values}`. Read does not resume an Agent. The optional `cursor` type field is currently ignored: this implementation returns **all** visible rows, not a bounded paginated list. Do not invent a `nextCursor`. Group rows by workspace `sessionIds`; local selection is not a Host mutation.

### 4. Create/adopt session — POST `/api/session/create`

```json
{"type":"client-request","rpcId":"rpc-create-1","method":"session/create","payload":{"args":{"request":{"workspaceId":"workspace-example","sessionId":"session-example","agentPreset":"standard"}}}}
{"type":"server-response","rpcId":"rpc-create-1","result":{"ok":true,"value":{"sessionId":"session-example","agentPreset":"standard"}}}
```

DTOs: `SessionCreateRequest/Value`. `workspaceId` and `cwd` are mutually exclusive; omitted ID is Host-minted. Stable client ID supports explicit adoption/retry, but cwd/preset conflicts and existing writer ownership are errors. Workspace attachment can fail **after creation** (`session/workspace-attach-failed`); reconcile roster before retry. [Command owner](<../../deepseek-harness-linux/packages/api/session-controller/src/commands.ts#L100-L143>).

### 5. Open session and watch journal — `session/follow` (WS)

```json
{"type":"open","streamId":"g1-session-example","endpoint":"session/follow","payload":{"args":{"request":{"address":{"kind":"session","sessionId":"session-example"},"maxMessages":50,"assistantStream":true}}}}
{"type":"item","streamId":"g1-session-example","value":{"type":"snapshot","header":{"version":4,"id":"session-example","createdAt":1791244800000,"cwd":"/tmp/native-example","isSeeded":false,"delegationDepth":0,"agentPreset":"standard"},"cursor":-1,"records":[],"hasMore":false,"projections":{"asOfSeq":-1,"values":{}},"assistantStream":{"revision":0}}}
{"type":"item","streamId":"g1-session-example","value":{"type":"event","event":{"seq":0,"time":1791244801000,"type":"turn/start","data":{"turn":1}}}}
```

DTOs: `SessionFollowRequest/Frame`, `SessionAddress`, `SessionWireHeader/Event`. Empty journal cursor may be `-1`. Opening snapshot replaces the current retained window; all subsequent durable events are contiguous from `cursor+1`. There is **no request cursor/resumeSeq parameter** despite stale method JSDoc mentioning one. [Actual follow](<../../deepseek-harness-linux/packages/api/session-controller/src/history.ts#L120-L240>) promotes cold ordinary sessions after snapshot delivery; this is an activation operation, unlike page/list/projections. Direct subagent address is `{kind:"subagent",parentSessionId,childSessionId,mode:"one-shot"|"continuable"|"unknown"}`; do not submit ordinary-session prompts to it.

### 6. Earlier history — POST `/api/session/page`

```json
{"type":"client-request","rpcId":"rpc-page-1","method":"session/page","payload":{"args":{"request":{"address":{"kind":"session","sessionId":"session-example"},"throughSeq":0,"beforeSeq":1,"maxMessages":50}}}}
{"type":"server-response","rpcId":"rpc-page-1","result":{"ok":true,"value":{"records":[{"type":"event","event":{"seq":0,"time":1791244801000,"type":"turn/start","data":{"turn":1}}}],"hasMore":false}}}
```

DTOs: `SessionPageRequest/SessionPage`. Use the matching follow opening `cursor` as inclusive `throughSeq`; `beforeSeq` selects earlier records. Optional `turnWindow:{minMessages,minTurns}` preserves turn-aligned navigation. Default maxMessages is 50; values must be positive safe integers, not arbitrary byte caps. Record count/bytes can exceed message count. Prepend chronologically, deduplicate by durable seq, and never apply an old-generation page to a new window. [Pagination](<../../deepseek-harness-linux/packages/api/session-controller/src/history.ts#L70-L113>).

### 7. Submit user turn — POST `/api/session/prompt`

```json
{"type":"client-request","rpcId":"rpc-prompt-1","method":"session/prompt","payload":{"args":{"request":{"requestId":"prompt-example-1","sessionId":"session-example","mode":"queue","content":[{"type":"text","text":"Explain the current workspace."}],"clientTimeZone":"UTC"}}}}
{"type":"server-response","rpcId":"rpc-prompt-1","result":{"ok":true,"value":{"accepted":true}}}
```

DTOs: `SessionPromptRequest/Value`, `PromptContentPart`. `requestId` is a separate, stable prompt identity persisted as message `source.rpcId`; transport retries mint a new rpcId but reuse the same requestId/content. `queue` calls followup; `steer` targets the next step. ACK means inbox admission, not turn completion or attributable response. Reconcile optimistic draft against durable inbox/user-message events. Empty/whitespace-only content is rejected unless an attachment exists; time zone must be `UTC` or valid IANA Area/Location. [Admission](<../../deepseek-harness-linux/packages/api/session-controller/src/commands.ts#L306-L376>).

### 8. Model catalog — POST `/api/session/modelCatalog`

```json
{"type":"client-request","rpcId":"rpc-models-1","method":"session/modelCatalog","payload":{"args":{}}}
{"type":"server-response","rpcId":"rpc-models-1","result":{"ok":true,"value":{"default":{"provider":"example-provider","model":"example-model"},"routableProviders":["example-provider"],"groups":[{"id":"example-provider","name":"Example","models":[{"id":"example-model","name":"Example model","reasoning":{"efforts":[{"id":"off","name":"Off"}],"defaultEffort":"off"}}]}],"failures":[]}}}
```

DTO: `ModelCatalog`; optional description fields belong to models/efforts, reasoning metadata is model-owned. Isolated provider failures are `{id,name,message}` in `failures`, not proof all models are unavailable. Refresh on forwarded `llm/adapters-updated` and safe account changes. No keys/grants are returned. [Owner](<../../deepseek-harness-linux/packages/api/session-controller/src/index.ts#L312-L319>).

### 9. Select session model — POST `/api/session/selectModel`

```json
{"type":"client-request","rpcId":"rpc-select-1","method":"session/selectModel","payload":{"args":{"request":{"sessionId":"session-example","provider":"example-provider","model":"example-model","reasoningEffort":"off"}}}}
{"type":"server-response","rpcId":"rpc-select-1","result":{"ok":true,"value":{"selected":{"provider":"example-provider","model":"example-model","reasoningEffort":"off"}}}}
```

DTOs: `SessionSelectModelRequest/Value`, `ModelSelection`. Host validates/normalizes the route after resuming the session. Applies to the next model request, not a request already streaming. Saving the deployment default is background work and may fail after a successful reply. `modelSelection` projection distinguishes `lastUsed` and `next`. [Command](<../../deepseek-harness-linux/packages/api/session-controller/src/commands.ts#L146-L185>).

### 10. Cancel active generation — POST `/api/session/cancel`

```json
{"type":"client-request","rpcId":"rpc-cancel-1","method":"session/cancel","payload":{"args":{"request":{"sessionId":"session-example"}}}}
{"type":"server-response","rpcId":"rpc-cancel-1","result":{"ok":true,"value":{"accepted":true}}}
```

DTOs: `SessionCancelRequest/Value`. [Actual command](<../../deepseek-harness-linux/packages/api/session-controller/src/commands.ts#L505-L524>) requests user cancellation with `keepInbox:true`, requires attached ordinary Agent, and returns immediately. Observe durable `turn/end` plus status; queued messages can run afterwards. Logical WS cancel only unsubscribes—it does **not** stop model work. Existing private supervisor `cancel-generation` instead calls hook cancellation and awaits whole-agent idle; do not substitute its semantics for this BFF operation.

### 11. Subscribe to application listeners — `$events` (WS)

```json
{"type":"open","streamId":"g1-events","endpoint":"$events","payload":{"args":{}}}
{"type":"item","streamId":"g1-events","value":{"type":"ready","clientId":"client-generation-example","host":{"home":"/home/example"}}}
{"type":"item","streamId":"g1-events","value":{"type":"emit","event":"api-session/status","args":["session-example",true]}}
{"type":"item","streamId":"g1-events","value":{"type":"waterfall","event":"approval/request","eventId":"event-example-1","agentId":"session-example","request":{"toolName":"bash","callId":"call-example-1","reason":"An explicitly requested operation needs approval."}}}
```

`clientId` belongs to the current logical event generation. `host.home` is a display abbreviation hint, not a grant. Emit carries positional args; waterfall carries JSON request fields with the direct Agent/signal removed and identity moved to `agentId`. Host later sends `{type:"cancel",eventId}` **inside an item value**, distinct from carrier cancellation. Pending waterfalls can be replayed to a reconnecting event generation; emits are not a durable replay log. [Source allowlist](<../../deepseek-harness-linux/packages/api/remotes/src/remote-events.ts#L20-L48>), [projection](<../../deepseek-harness-linux/packages/api/gateway/src/stream-protocol.ts#L140-L172>), [event lifecycle](<../../deepseek-harness-linux/packages/api/gateway/src/index.ts#L475-L668>).

### 12. Approval/question waterfall reply — POST `/api/$events/result`

```json
{"type":"client-request","rpcId":"rpc-approval-1","method":"$events/result","payload":{"args":{"clientId":"client-generation-example","eventId":"event-example-1","outcome":{"kind":"result","value":"allowed-once"}}}}
{"type":"server-response","rpcId":"rpc-approval-1","result":{"ok":true}}
```

Args here directly contain `clientId,eventId,outcome`, **not** `{request:{...}}`. Approval values are exactly `allowed-once`, `rejected`, `cancelled`, or `unavailable`; no persistent/general grant. An unsupported listener replies `{kind:"next"}` to delegate, never silently approves. A question's successful value is `{answers:[{id,selected:[<option label>],custom?:<text>}]}`. Rejection is `{kind:"rejected",error:{name,message,code?,details?}}`. Send results only for a current, received event belonging to this generation, after explicit user action or the defined timeout/cancellation rule. [Validator](<../../deepseek-harness-linux/packages/api/gateway/src/stream-protocol.ts#L98-L137>); [reply](<../../deepseek-harness-linux/packages/api/gateway/src/index.ts#L424-L441>) is void, stale completed deliveries are idempotent no-ops, absent active clientId is a failure.

## Required supplementary capabilities

### Timed and continued questions

Namespace `userQuestions`, service `userQuestions`. [Types](<../../deepseek-harness-linux/packages/interaction/user-questions/src/types.ts#L9-L161>) use `multiSelect`, not the model tool's `multi_select`. Questions have `id,question,detail?,header?,options?:[{label,description?}],multiSelect?,intent?`; plan-review intent has `kind:"plan-review",approve,callId?`. Waterfall request has `questions` and optional `wait:{callId,timed?}`; no direct Agent/signal on wire.

A timed foreground waterfall requires a claim **before starting the UI countdown**:

```json
{"type":"open","streamId":"g1-question-claim","endpoint":"userQuestions/attachWait","payload":{"args":{"agentId":"agent-example","callId":"call-example-2"}}}
{"type":"item","streamId":"g1-question-claim","value":{"remainingMs":30000}}
```

The first frame records acquisition with a sampled remaining duration; end before it means no acquired claim, not a known timeout outcome. Native controls must remain disabled, with no fabricated timer or automatic delegation. [Owned native claim](<NATIVE-TIMED-QUESTIONS.md>) consumes this frame, validates exactly one object/integer item and retains the stream; the application timed UI remains unsupported. After user answer, send `$events/result` with the structured answer, retaining the claim until Host closes it. Timeout replies use `{kind:"rejected",error:{name:"UserQuestionError",message:"ask_user_question timed out before the user answered",code:"ASK_TIMED_OUT"}}`. Cancelling/closing the claim releases it, not a completed answer. [Host attachWait](<../../deepseek-harness-linux/packages/interaction/user-questions/src/index.ts#L208-L220>) and [actual Client lifetime](<../../deepseek-harness-linux/packages/client/ui-user-questions/src/client/index.ts#L208-L279>).

For a projected `continued` question, use a different unary endpoint, not a stale waterfall result:

```json
{"type":"client-request","rpcId":"rpc-question-late-1","method":"userQuestions/answer","payload":{"args":{"agentId":"agent-example","callId":"call-example-2","answer":{"answers":[{"id":"color","selected":["Green"],"custom":"Answered after the timeout"}]}}}}
{"type":"server-response","rpcId":"rpc-question-late-1","result":{"ok":true,"value":true}}
```

Lookup uses `agentId`, not `sessionId`, from the [Agent lookup](<../../deepseek-harness-linux/packages/core/agent/src/index.ts#L255-L269>). Only the exact live root Agent may answer; delegated children cannot open human interaction. A batch must name each question exactly once; an already queued reply errors `REPLY_QUEUED`, and false means no continued question. Accepted late answers steer a new user message and remain queued until admission. [Answer owner](<../../deepseek-harness-linux/packages/interaction/user-questions/src/index.ts#L131-L205>).

### Attachments

Text prompt parts are `{type:"text",text}`. Image intake is `{type:"image",mediaType:"image/png"|"image/jpeg"|"image/webp"|"image/gif",data:<base64>,name?}`; send bytes rather than an invented URL. File intake is `{type:"file",receiptId:<Host receipt>}`, never a guessed attachmentId/path.

Prefer raw streaming upload; [route](<../../deepseek-harness-linux/packages/client/file-upload/src/http-route.ts>) requires POST and octet-stream, mandatory `sessionId` query, optional URL-encoded `name`. Body is exact file bytes. Response is **not ClientRPC**:

```json
{"ok":true,"value":{"receiptId":"receipt-example-1","file":{"attachmentId":"sha256:29a0837077f6f3cbddd98c0605897cc6f2b4e5833fb8e1ca4a8045d89b2c8c60","name":"poem.txt","bytes":16}}}
```

Send a later `session/prompt` content part `{"type":"file","receiptId":"receipt-example-1"}`. Receipts belong to the receiving live Session scope; they bind to the prompt requestId and are retired with queue/history/disposal lifecycle. Upload ACK alone does not add a user message. [Receipt owner](<../../deepseek-harness-linux/packages/client/file-upload/src/index.ts#L55-L106>) and [receipt types](<../../deepseek-harness-linux/packages/client/file-upload/src/types.ts>).

Base64 fallback is POST `/api/fileUploads/upload` with `payload:{args:{agentId,request:{data,name?}}}`; it returns FileUploadValue inside ClientRPC. For session-reachable image display, POST `/api/session/attachment` with `args:{request:{sessionId,attachmentId}}` returns `{attachment:<ImageAttachmentRef>,data:<base64>}`. The Host verifies reachability from the session log; references are not an ambient file reader. Respect `imageLimits` projection and provider modality admission. Generic file/document previews and arbitrary filesystem APIs are outside the minimum.

### Tools, status, projections, and event tree

Status/list listeners are exactly `api-session/added(summary:SessionSummary)`, `api-session/removed(sessionId)`, `api-session/status(sessionId,running)`, `api-session/activity(sessionId,updatedAt)`, and `api-session/error(sessionId,message)`. [Event declarations](<../../deepseek-harness-linux/packages/api/session-controller/src/types.ts#L583-L618>). Subscribe `$events` before roster fetch, buffer emits through roster installation, and reconcile on reconnect; there is no single atomic list-plus-events baseline.

Optional Host-wide `session/control` opens with `{args:{}}`; frame values are `{type:"baseline",value:{projections:{<sessionId>:{asOfSeq,values}}}}`, then `{type:"projection",sessionId,key,value,seq}`. **Current alpha control baseline contains only projections**, not queues, approvals, or questions. [Actual control](<../../deepseek-harness-linux/packages/api/session-controller/src/control.ts#L37-L74>). Queues and continued-question state are owning projections/events, not guessed top-level control fields. Cold projection fetch is POST `/api/session/projections` with `args:{request:{sessionId}}`, yielding `{asOfSeq,values}` or null without activation.

Build the native tree from durable seq and business identities: `turn/start/end` → `step/start/end` → `user/message`, `assistant/message`, `assistant/attempt`, `tool/call`, `tool/result`, plus merge-extensible events. [Core event definitions](<../../deepseek-harness-linux/packages/core/session/src/types.ts#L281-L402>) specify that tool-call arguments are a **raw JSON string**, paired by `callId`; tool result has `message,error?,meta?`, with tool-private JSON metadata. Never evaluate arguments, HTML, Markdown scripts, or meta. Render a safe generic tool card; cap displayed raw fields and provide explicit truncation/loading markers.

`surfaceOp:"append"` and `{op:"replace",startSeq,endSeq}` determine model-visible message placement; event-tree raw order is not the chat's final message order. `sourceEventSeqs` cites earlier events for known surface semantics. Preserve unknown event JSON for a bounded generic row, but refuse unsupported required durable event semantics rather than pretending to understand them; `ignorable:true` explicitly permits ignoring an unknown event. Persisted Session migrations belong to the existing backend, not a homemade JSONL parser in the GUI.

With `assistantStream:true`, follow also emits `{type:"assistant-stream",frame}` with `start|chunk|end`, attemptId/revision, dense chunk index, and committed `{eventType,seq}` or abandoned outcome. These have no independent durable event seq; never advance the durable cursor from a chunk. Replace transient text on final durable settlement, discard abandoned attempts, and rebaseline on discontinuity instead of duplicating text. Port the [settlement-aware native-equivalent fold](<../../deepseek-harness-linux/packages/api/session-controller/src/client/sessions/assistant-stream.ts>) and [wire fields](<../../deepseek-harness-linux/packages/api/session-controller/src/types.ts#L493-L563>); do not treat opaque compact stream tuples as ordinary string deltas. The minimum first slice may omit assistantStream and show only durable assistant settlements; that is honest non-streaming presentation, not streaming parity.

## DTO discipline

Rust serde DTOs must preserve these exact spellings and discriminants, distinguish missing fields from null, check safe integral seq/time/index values, and model identities with newtypes. This wire-level notation is a compact implementation aid; source declarations above remain authoritative. `JsonValue` means recursively validated JSON, not `any`, a JS object, or executable content.

```text
JsonValue = null | boolean | finite number | string | JsonValue[] | map<string,JsonValue>
RemoteFailure = { code:string, message:string, details:map<string,JsonValue> }
RemoteResult<T> = {ok:true,value?:T} | {ok:false,error:RemoteFailure}
ClientRequest<A> = {type:"client-request",rpcId:RpcId,method:Endpoint,payload:{args:A}}
ServerResponse<T> = {type:"server-response",rpcId:RpcId,result:RemoteResult<T>}
SessionListArgs = {_request:{cursor?:string}}
RequestArgs<T> = {request:T}
EmptyArgs = {}
ModelSelection = {provider:string,model:string,reasoningEffort?:string}
PromptContentPart = {type:"text",text:string}
  | {type:"image",mediaType:ImageMediaType,data:string,name?:string}
  | {type:"file",receiptId:FileUploadReceiptId}
SessionPromptRequest = {requestId:SessionRequestId,sessionId:SessionId,
  mode:"queue"|"steer",content:PromptContentPart[],clientTimeZone?:string}
SessionCreateRequest = {workspaceId?:WorkspaceId,cwd?:string,sessionId?:SessionId,agentPreset?:string}
SessionAddress = {kind:"session",sessionId:SessionId}
  | {kind:"subagent",parentSessionId:SessionId,childSessionId:SessionId,
     mode:"one-shot"|"continuable"|"unknown"}
HistoryWindow = {maxMessages?:number,turnWindow?:{minMessages:number,minTurns:number}}
SessionFollowRequest = HistoryWindow & {address:SessionAddress,assistantStream?:true}
SessionPageRequest = HistoryWindow & {address:SessionAddress,throughSeq:number,beforeSeq?:number}
SessionProjectionBaseline = {asOfSeq:number,values:map<string,JsonValue>}
SessionWireEvent = {type:string,seq:number,time:number,data:JsonValue,
  ignorable?:true,sourceEventSeqs?:JsonValue,surfaceOp?:JsonValue}
SessionHistoryRecord = {type:"event",event:SessionWireEvent}
SessionPage = {records:SessionHistoryRecord[],hasMore:boolean}
WorkspaceView = {workspaceId:WorkspaceId,path:string,title:string,sessionIds:SessionId[],
  createdAt:string,updatedAt:string}
WorkspaceBaseline = {items:WorkspaceView[],archivedSessionIds:SessionId[],pinnedSessionIds:SessionId[]}
ApprovalRequestWire = {toolName:string,callId?:ToolCallId,reason?:string,
  displayReason?:map<string,string>}
QuestionAnswer = {answers:{id:string,selected:string[],custom?:string}[]}
EventOutcome = {kind:"next"} | {kind:"result",value?:JsonValue}
  | {kind:"rejected",error:{name:string,message:string,code?:string,details?:JsonValue}}
EventResultArgs = {clientId:RemoteEventClientId,eventId:RemoteEventId,outcome:EventOutcome}
QuestionAnswerArgs = {agentId:SessionId,callId:ToolCallId,answer:QuestionAnswer}
QuestionClaimArgs = {agentId:SessionId,callId:ToolCallId}
FileUploadValue = {receiptId:FileUploadReceiptId,
  file:{attachmentId:AttachmentId,name:string,bytes:number}}
```

Define full SessionWireHeader, AssistantStream, ModelCatalog, WorkspaceFollowFrame, and forwarded-event enum DTOs directly from the linked alpha declarations; do not replace them with flattened strings. Unknown plugin payload fields remain bounded JsonValue until a declared owner-specific presenter is added. Optional fields are omission, not automatic null.

## Native lifecycle, limits, and unsafeActions guard

The following are **native implementation requirements**, not claims that the alpha BFF already applies them. `unsafeActions` is a proposed native policy/guard purpose, **not an existing alpha request field, service, or security mode**. Its job is to prevent implicit privilege escalation from native UI callbacks while preserving explicit user-approved operations. The server still enforces its composed approval/sandbox policies.

- Use a closed native action enum mapped to the specific twelve endpoints and required supplementary endpoints. Ordinary buttons cannot provide an endpoint string, eval text, Host service name, arbitrary JS expression, subprocess args, or raw credential object. Require current session identity, generation, explicit gesture, and operation-specific DTO validation for mutation actions. Approval replies must match a pending event; `allowed-once` is never synthesized from UI dismissal or an unknown option.
- Keep `unsafeActions` disabled for runtime/plugin-manager mutations, profile edits, credential reads/writes, arbitrary filesystem calls, desktop openers, terminal creation/input, inspect-query execution, client package installation/evaluation, and unsupported Cordis actions. Add any such capability separately with its real server authorization and an intentional user-facing guard. Do not change a running user's policy just to let native UI answer approvals. Unsupported `cordis/request-run`, `cordis/dynamic-package`, or inspect-related emits are inert diagnostics, never instructions to execute JS.
- All primary transport network requests are exact-generation loopback; disable proxy inheritance and arbitrary DNS resolution for these connections. Bind the ready port, origin, cookie, WS, pending RPCs, event clientId, file receipts, and session followers to one private Rust generation. On child exit/restart invalidate them together; reacquire readiness and cookie instead of reusing an old port. Drop every late old-generation callback and reply.
- Selection does not dispose the actual backend Session or cancel its Agent. Release native history followers/claims and drafts according to their own ownership; Node retains the real session lifecycle. Closing one UI tab is not archive/delete. Whole-app shutdown suppresses new publications first, closes subscriptions/cancels read HTTP, settles private pipe shutdown, awaits actual Host disposal/child exit, then releases account tokens/cookie. The supervisor already aborts account watchers and waits pending work before `shutdown-complete`; do not replace quiescence with a mere kill request.
- Model cancellation is a distinct gesture using `session/cancel`; WS logical cancel and HTTP-read abort release observations only. An accepted prompt and an idle status are not a per-message success interval; retries/steering/multiple queued inputs can share one running interval. Preserve the existing AgentLoop behavior, including interrupted durable prefixes and still-pending inbox work.
- Native result shaping must be bounded at the complete result/frame **before** entering widget state: cap incoming complete-message bytes before JSON decoding, then validate decoded nesting/item counts and retained-window totals. Suggested initial native policy: 8 MiB unary/WS JSON result limit, 16 MiB retained selected-session data, 10,000 roster items, and bounded RPC/stream counts/timeouts; these are proposed budgets to review against real fixtures, not server defaults. An oversized authoritative baseline/required event is a visible unsupported-size error, not silently truncated state. Large history can use smaller maxMessages but a single large event can still exceed budget. Only display strings/tool raw-preview text may be visibly truncated without falsifying cursor/history continuity. Keep raw streaming file bytes on a separate checked path with validated complete sizes and no base64 expansion unless required.
- Existing carrier limits are different: HTTP buffered request cap is **300 MiB** ([bridge](<../../deepseek-harness-linux/packages/client/connection/src/http-bridge.ts#L10-L76>)); Gateway logical uplink buffering defaults to **262,144 bytes**, not outbound result/frame size. The mux does not establish an application-level capped outbound snapshot, roster count, or result byte budget. Native bounds do not prevent Node from constructing an oversized unpaginated roster first. Honest resource-bounded parity may eventually need an owning BFF change, outside this documentation-only slice.
- Dispatch decoded native view updates through the toolkit thread; coalesce visual chunk publication without dropping or reordering durable events. Never put raw launch URL, cookies, Platform tokens/request headers, or unsanitized diagnostic packets into WidgetProps, serialized GUI models, crash output, tracing spans, or logs. Preserve secret-redaction before diagnostic truncation. There is no primary Web engine memory to inject credentials into.

## Minimal native vertical slice

1. Reuse the supervisor and private generation-owned process state; perform the root 303 exchange in Rust. Maintain a clean-origin HTTP client plus one authenticated WS mux; publish only startup/account-safe status to native widgets.
2. Open `$events` and await its ready clientId; open `workspace/follow` and optional `session/control`; then fetch `session/list` with `_request:{}` and install buffered list/status emits. Render native workspace/session lists with bounded loading/error/reconnect states.
3. On native directory selection, call workspace/create. On new-session gesture, call session/create with workspaceId and stable client sessionId. Selecting/opening a row opens session/follow; cancelled followers are UI ownership changes, not backend cancellation. Load older windows with session/page.
4. Render a virtualized native chat/event tree with generic tool cards; first get durable settlements working, then add the exact assistant-stream fold. Fetch modelCatalog and wire selectModel. Keep draft requestId separate from rpcId and confirm prompt acceptance/history echoes.
5. Add binary file upload → same-session receipt → prompt; add image admission/display with explicit byte bounds. Never expose an arbitrary Host file read action through the attachment button.
6. Render approval and question waterfalls from the current event generation; add timed wait claim and continued answer path. Cancellation clears pending interaction affordances; native unknown listeners delegate safely. Wire Stop to session/cancel, not WS unsubscribe or supervisor's semantically different cancel hook.
7. Reconnect replaces all authoritative baselines, reconciles prompt IDs, and rebuilds only current-generation pending interaction state. Restart reacquires the private cookie and account state. Close waits actual shutdown/disposal.

This slice needs no new Cordis plugin, no current-runtime configuration changes, no Client JS execution, and no mandatory web rendering. Implement Rust transport/data ownership once and adapt view DTOs to either Slint or Iced; toolkit benchmarking does not require duplicating backend semantics.

## Source fixtures and acceptance evidence (not executed here)

Use the existing public entry/profile and keyless replay assets for the parent's later tests; no local pipeline was run for this audit. A source fixture is evidence about its tested behavior, not proof this native implementation exists.

| Evidence | Exact useful facts and limitations |
|---|---|
| [Public URL acceptance](<../../deepseek-harness-linux/apps/cli/tests/profiles/web/tests/public-url.expected.e2e.ts#L134-L221>) | Built shipped Web profile with random port, 303 cookie exchange, direct-loopback authenticated create, and foreign Host rejection. Proxy prefix behavior is irrelevant to the direct native origin. |
| [Built Remote acceptance](<../../deepseek-harness-linux/packages/api/remotes/tests/built-lib.e2e.ts#L116-L147>) | Real HTTP root-cookie exchange and generated Remote calls across built bundles. It uses a hand-built test Host, not full native/product startup. |
| [File-upload round](<../../deepseek-harness-linux/apps/web/tests/file-upload-round.e2e.ts#L1-L38>) and [current recorded session](<../../deepseek-harness-linux/snapshots/web/file-upload-round/session.v4.jsonl#L1-L24>) | Keyless replay-driven real upload → prompt → attachment block → real read-tool round trip. Recorded source header has V4; durable user content stores file/image attachment refs, while the intake API uses staged receipt/base64. Several test constants still name V3; select fixtures according to the owning snapshot policy, not a guessed writer version. |
| [Live interactions acceptance](<../../deepseek-harness-linux/apps/web/tests/live-interactions.e2e.ts#L1-L59>) | Real composition/wire cancellation, draft queueing, retry/error behavior with replay overrides; not a live provider requirement for deterministic testing. |
| [Approval composer](<../../deepseek-harness-linux/apps/web/tests/approval-composer.e2e.ts#L33-L59>) | Keyless replay approval gesture and audit outcome, plus actual workspace result; native must retain one-shot answer semantics, not browser geometry. |
| [Question composer](<../../deepseek-harness-linux/apps/web/tests/question-composer.e2e.ts#L1-L44>) | Blocking, timed/continued, queued and cancelled question paths through shipped answerer composition and persisted output. |
| [Assembled Remote fixture](<../../deepseek-harness-linux/apps/web/tests/fixtures/assembled-remote.fixture.json>) and [its adapter](<../../deepseek-harness-linux/apps/web/tests/assembled-remote.ts>) | Useful synthetic model/workspace/follow examples, but **not authoritative current alpha DTOs**: roster fixture lacks required agentAvailable and control fixture includes legacy queues/approvals/questions. Use current source control/list types instead. |
| [Snapshot ownership](<../../deepseek-harness-linux/snapshots/AGENTS.md>) | Source replay is keyless/read-only, recordings are versioned and owner-controlled, process starts through public dsh profile; do not invent hidden test Host entrypoints or rewrite fixtures to fit a client bug. |

Later native acceptance should check serialized golden requests/responses for all twelve operations, valid/invalid correlation IDs and exact args, cookie/Origin rejection, WS Ping/Pong and logical cancellation, baseline replacement, page-generation races, prompt retry identity, queued cancellation, transient settlement deduplication, staged-file lifetime, one-shot approvals, timed-question claims, late replies, child restart, secret-free view serialization, oversized result handling, and quiescent shutdown. Those are future acceptance requirements, not reported passing tests.

## True limits: tools and plugins

The same Node composition preserves model/tool execution, persistence, jobs, subagents, goals, schedules, skills, MCP and external tools when their providers/policies are composed and available. It does not magically turn browser-side renderers, Cordis Slots, React components, plugin-defined controls, terminal emulators, Office previews, browser automation surfaces, or UI callbacks into native widgets.

Native first-slice tools render as generic bounded call/result/status rows; specialized diff/document/artifact presenters need native implementations using their owning DTOs. Model/browser tools that require a browser carrier stay unavailable when the supervisor's optional browser payload is disabled; Office payload is reported unavailable/not-configured. That is a real composition limitation, not transport parity. Arbitrary external tool results may be large or plugin-defined; fallback is inert JSON/text with explicit unsupported-state handling.

New Host plugins can affect model behavior without changing this native transport, but new public Remote endpoints/projections/events need reviewed native enum/DTO support. New Client-only plugins cannot execute in the native UI. HMR/Client package notifications do not authorize a native JavaScript evaluator. Credentials/config/plugin-management APIs are intentionally outside the minimal allowlist despite the authenticated operator transport being technically capable of reaching composed endpoints.
