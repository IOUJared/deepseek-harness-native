# Native session management: isolated-alpha audit

## Status and scope

The audited runtime is the isolated `deepseek-harness-linux` fork, version `0.2.1-alpha.1`, not the installed RC runtime. Native transport now exposes typed pin/unpin and archive/restore methods **without `stopActivity`** alongside workspace-follow state. Rename, fork and workspace registration deletion remain unimplemented in this transport. Native [ZIP export controls and actual-Host qualification](<NATIVE-SESSION-EXPORT.md>) are now implemented as a separate binary route, not an RPC. Transport-only tests use a local fake BFF; they do not qualify application controls, live Host mutations, model/provider calls or a running GUI. Native guarded UI/worker controls are now implemented with stream-authoritative order and reachable archived restore; see [implementation and qualification scope](<NATIVE-REGISTRY-CONTROLS.md>). Initial application/unit validation passes; separate real Host composition is being qualified. This document preserves the independently developed settings/modal layout and does not attribute those visual changes to the registry feature.

## Authority and wire format

The Host applies its Host/Origin trust check and authentication; admitted requests speak for the operator, not a model or approval responder. See [operator admission](../../deepseek-harness-linux/packages/client/connection/src/rpc-host.ts#L103-L112). Future native calls must reuse the owned worker's private authenticated client, retain isolated-runtime/version and lifetime fencing, and remain disabled during real keyless smoke or closing/stopped states. Use actual `SessionId`/`WorkspaceId` values; never substitute `AgentId` or invent ownership associations.

Mutations use `POST /api/<namespace>/<method>`, `Content-Type: application/json`, the private cookie and same-origin header, and named arguments:

```json
{"type":"client-request","rpcId":"native-…","method":"workspace/pinSession","payload":{"args":{"request":{"sessionId":"session-…"}}}}
```

The reply is `type:"server-response"` with the same `rpcId` and `result:{ok:true,value:…}` or `result:{ok:false,error:{code,message,details}}`. The argument wrapper is **`args.request`**, not a positional array or a bare request body. Native encoding, bounded decoding, correlation and cancellation are in [HTTP transport](<../transport/src/http.rs#L314-L389>) and [request wrapping](<../transport/src/http.rs#L592-L596>). The [endpoint allowlist](<../transport/src/http.rs#L160-L197>) includes exactly the four registry operations described below; other audited mutations/export remain outside it. Remote errors collapse to `RemoteFailure`, so richer refusal handling requires an explicitly bounded, allowlisted typed error representation rather than raw Host diagnostics.

## Exact supported operations

| Operation | Method and `args.request` | Successful value |
| --- | --- | --- |
| Session rename | `session/rename`, `{sessionId,title}` | `{title,seq}` |
| Archive | `workspace/archiveSession`, `{sessionId,stopActivity?:boolean}` | `{archivedSessionIds:[…]}` |
| Restore | `workspace/unarchiveSession`, `{sessionId}` | `{archivedSessionIds:[…]}` |
| Pin | `workspace/pinSession`, `{sessionId}` | `{pinnedSessionIds:[…]}` |
| Unpin | `workspace/unpinSession`, `{sessionId}` | `{pinnedSessionIds:[…]}` |
| Fork | `session/fork`, `{sessionId,atSeq?:number}` | `{sessionId:newChild}` |
| Workspace registration removal | `workspace/delete`, `{workspaceId}` | `{deleted:true}` |

Declarations: [session methods](../../deepseek-harness-linux/packages/api/session-controller/src/index.ts#L404-L423), [session request/reply fields](../../deepseek-harness-linux/packages/api/session-controller/src/types.ts#L308-L330), [workspace methods](../../deepseek-harness-linux/packages/api/workspace-controller/src/index.ts#L119-L186), and [workspace request/reply fields](../../deepseek-harness-linux/packages/api/workspace-controller/src/types.ts#L86-L150).

### Implemented Rust registry surface

| Rust method | Request | Successful receipt |
| --- | --- | --- |
| `NativeClient::pin_session` / `unpin_session` | `&SessionRegistryRequest { session_id: SessionId }` | `PinnedSessionsValue { pinned_session_ids: Vec<SessionId> }` |
| `NativeClient::archive_session` / `unarchive_session` | same request | `ArchivedSessionsValue { archived_session_ids: Vec<SessionId> }` |

The [DTOs](<../transport/src/dto.rs#L111-L193>) serialize camelCase and require one-field JSON objects, rejecting positional arrays and unknown/duplicate request keys. No request field can supply `stopActivity`, `agent` or `signal`; even `stopActivity:false` is omitted rather than offered. The [four methods](<../transport/src/http.rs#L451-L487>) reuse existing authenticated, bounded and cancellable invocation. Response vectors preserve the complete global Host order; they are receipts, never instructions to overwrite authoritative follow state. [Transport fixture tests](<../transport/tests/transport.rs>) pass exact envelopes for all four methods, object-only parsing, order, redacted refusal/correlation errors, body bounds, timeouts, retired-client denial and in-flight cancellation, without booting Harness or mutating a real registry.

### Pin and archive semantics

Pin/archive sets are **registry-global**, not per workspace. Pin validates that the session exists and is not archived; unknown sessions fail as `session/not-found`, and archived-session pinning fails as `gateway/bad-request`. Successful pin prepends the ID; an already pinned ID is a no-op and does not move it. Unpin and restore are idempotent removals, including when the underlying session has disappeared. Preserve the complete returned pin order, **most recently pinned first**; membership-only sets cannot represent it. See [pin execution](../../deepseek-harness-linux/packages/workspace/workspace/src/index.ts#L410-L463) and [wire error mapping](../../deepseek-harness-linux/packages/api/workspace-controller/src/commands.ts#L197-L226).

Default archive checks running turns, subagents, jobs and schedules through the Host activity providers; reported activity yields `workspace/session-active` without the archive write. Archive retains session logs and workspace accounting, removes the pin in the same durable write, and blocks later model steps for the archived session and its subagent descendants. Restore returns the session to its accounted position but does not restore a former pin. See [archive/restore execution](../../deepseek-harness-linux/packages/workspace/workspace/src/index.ts#L346-L407) and [archived-session gate](../../deepseek-harness-linux/packages/api/session-controller/src/archived-session-gate.ts#L23-L55).

**Do not implement `stopActivity` in the initial native feature.** The actual executor durably archives/unpins **before** issuing stop requests, contrary to the request documentation's stated ordering. Stop requests are not proof that all work has stopped; a later failure does not establish rollback of the archive. Actual order: [registry write then stop](../../deepseek-harness-linux/packages/workspace/workspace/src/index.ts#L371-L383). Omit the field; never escalate a refusal automatically.

### Rename and fork semantics

Rename explicitly resolves/resumes an ordinary Agent, so it is not a cold registry-only metadata mutation; subagent lifecycle ownership and writer conflicts can refuse it. The Host strips deceptive/control text, normalizes whitespace, truncates to its configured UTF-8 byte limit, and rejects an empty normalized title. It appends `session/title` with `source:{kind:"user"}` and `messageSeqs:[]`, superseding automatic title generation and pinning the user title against later automatic updates. The event is log-only, not model history; the reply supplies the accepted title and event sequence, not a model result. See [rename command](../../deepseek-harness-linux/packages/api/session-controller/src/commands.ts#L188-L211), [Agent resolution](../../deepseek-harness-linux/packages/api/session-controller/src/agent.ts#L183-L227), [title acceptance](../../deepseek-harness-linux/packages/session/session-title/src/index.ts#L390-L420), and [title projection](../../deepseek-harness-linux/packages/session/session-title/src/index.ts#L262-L275).

Fork reads an exact source observation. `atSeq` must be a nonnegative safe integer naming an existing event and is inclusive. Omission selects the latest completed-turn prefix; a source with no completed turn can refuse the omitted boundary. The new ordinary Agent uses current default model selection, inherits source history and preset/cwd metadata, and may attach to the source workspace or a subagent ancestor's workspace. The seed adds `session/end-seed` and synthetic forked open-tail closers without changing the source log. See [fork execution](../../deepseek-harness-linux/packages/api/session-controller/src/commands.ts#L221-L303), [workspace selection](../../deepseek-harness-linux/packages/api/session-controller/src/commands.ts#L568-L577), and [seed construction](../../deepseek-harness-linux/packages/core/session/src/fork.ts#L10-L29).

**Fork errors/timeouts are potentially indeterminate, with no automatic retry.** A child can exist even when the call fails: workspace attachment occurs after creation and may return `session/workspace-attach-failed` with the child identity. Each retry can create another child; losing an ACK is not proof that creation failed. Expose a bounded recovery path rather than silently creating again or guessing which child was created.

### No session deletion or storage manipulation

No session-delete BFF method was found in the audited session controller. Do not invent `session/delete`, remove persistence files, invoke internal storage services, or substitute archive for permanent deletion. `workspace/delete` removes only a workspace registration, retaining its directory and every session log; an unknown workspace is a remote refusal. See [workspace removal](../../deepseek-harness-linux/packages/api/workspace-controller/src/commands.ts#L93-L104) and [retained storage](../../deepseek-harness-linux/packages/workspace/workspace/src/index.ts#L300-L309).

## Stream-authoritative state and races

Workspace follow starts with a complete baseline, then ordered `upsert`, `remove`, `order`, `archived`, and `pinned` increments. Archive/pin increments contain **complete replacement sets**, not additions/removals; archiving a pinned session can emit both archive and pin changes. Existing [native DTOs](<../transport/src/dto.rs#L477-L509>) represent these frames. Host publication: [workspace feed](../../deepseek-harness-linux/packages/api/workspace-controller/src/feed.ts#L80-L139).

Keep the current stream generation authoritative. Neither mutation replies nor workspace frames expose a comparable revision field, so HTTP reply arrival must not overwrite a newer stream state. Fence operation receipts by owned-client epoch, target ID, operation serial and relevant view lifetime; use ACKs to settle matching operations, not blindly install their potentially older returned sets. Preserve complete ordered pin values, replace state on a valid reconnect baseline, and never merge stale-generation sets. If immediate RPC-state adoption is introduced, it needs an explicit stream-progress fence and race tests; do not infer ordering across HTTP and the stream. Queue rejection is known-not-sent; an admitted mutation timeout does not prove no write. Never retry mutations automatically.

## Export: separate bounded binary operation

Export is **not `session/export` RPC**. When mounted, the Host serves authenticated `GET /api/session.export?sessionId=<encoded-id>&includeDescendants=true|false`; descendants default to false, and `HEAD` is supported. Success is `application/zip` with an attachment filename. Invalid query is 400, missing session is 404, and preparation/service failures are 500. The Web `/export` command accepts no destination path and its “requested” result is not proof of a completed download. See [route](../../deepseek-harness-linux/packages/session-query/session-log-export/src/routes.ts#L7-L11) and [export handler](../../deepseek-harness-linux/packages/session-query/session-log-export/src/index.ts#L73-L168).

The Host flushes live logs, reads committed persisted history, and streams canonical logical logs plus referenced attachments; optional descendants are subagents. Streaming does not impose a total download bound, and root-log preparation still materializes its text on the Host. See [archive behavior](../../deepseek-harness-linux/packages/session-query/session-log-export/src/archive.ts#L1-L20) and [flush/read](../../deepseek-harness-linux/packages/session-query/session-log-export/src/archive.ts#L72-L168).

The implemented native export uses a worker-owned authenticated bounded binary request, **not the JSON response decoder**. **Manage → Export session ZIP…** requires an absolute new destination, a warning review and explicit confirmation. Its 4 MiB consumer limit/deadline does not bound the Host's root-log preparation. Structural ZIP acceptance is not complete CRC/inflate/session-identity proof. A fully received accepted archive moves into separately owned blocking filesystem work; directory-fd NOFOLLOW ancestors and an exclusive mode-0600 final file prevent ordinary symlink/overwrite races. Exact accounting and file/directory sync precede a Saved receipt. There is **no staging rename, rollback, deletion or automatic retry**: failure after final creation may retain an empty/partial/complete private file, explicitly reported as indeterminate. Close cancels/awaits downloads and joins admitted saves before Stopped, without a hard unhealthy-filesystem deadline. Live export can already have durably flushed the source log; cancellation cannot undo that. There is no extraction or automatic opener. [Export qualification](<NATIVE-SESSION-EXPORT.md>) documents actual composed UI/worker/Host fixtures, independent bounded archive checks, private saves, no-overwrite refusal, receipt fencing and scope limits.

## Recommended worker/UI integration order

1. **Pin/unpin:** smallest reversible registry-only operation; no Agent resume, history rewrite or model validation. Preserve global scope and most-recent-first ordering.
2. **Archive/restore without `stopActivity`:** reversible, Host-enforced refusal for active work; add an archived-session view so restoration is reachable, and explain that archive is not deletion.

The typed transport methods/DTOs and native pin/unpin/archive-without-stopActivity/restore worker/UI controls are implemented. [Registry acceptance](<NATIVE-APP-QUALIFICATION.md>) and [parent composed registry evidence](<../app/evidence/parent-registry-qualification.json>) cover explicit reviewed actions, metadata-only receipts, current-feed authority, stale reply/full-set/order/coupling fences, real Host ACKs and isolated keyless cleanup. [Native binary export](<NATIVE-SESSION-EXPORT.md>) has a separately scoped actual-Host qualification. These scripted fixtures do not prove physical input, all later shared-source builds or full desktop parity. Rename and fork remain later work; permanent session deletion remains unsupported by the audited BFF.
