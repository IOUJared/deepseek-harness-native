# Native session management: isolated-alpha audit

## Status and scope

Isolated `0.2.1-alpha.1`, not installed RC. [Registry UI/worker](<NATIVE-REGISTRY-CONTROLS.md>) implements pin/unpin/archive/restore without `stopActivity`; [ZIP export](<NATIVE-SESSION-EXPORT.md>) is separate. Rename/fork/workspace removal remain unimplemented; transport fake-BFF tests alone prove no live GUI/Host mutation.

## Authority and wire format

- [Host/Origin operator admission](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/client/connection/src/rpc-host.ts#L103-L112>), private owned-worker client and runtime/lifetime fences required. No AgentId substitution for SessionId/WorkspaceId; keyless smoke/closing forbids mutations.
- POST `/api/<namespace>/<method>`, JSON, private Cookie/Origin; **named `args.request`**, not arrays/bare bodies:

```json
{"type":"client-request","rpcId":"native-…","method":"workspace/pinSession","payload":{"args":{"request":{"sessionId":"session-…"}}}}
```

- Matching `server-response`/`rpcId`: `result:{ok:true,value:…}` or `{ok:false,error:{code,message,details}}`.
- [Transport](<../transport/src/http.rs#L314-L389>)/[wrapper](<../transport/src/http.rs#L592-L596>)/[allowlist](<../transport/src/http.rs#L160-L197>) bounds/correlates/cancels. Remote diagnostics collapse to `RemoteFailure`; richer errors need bounded allowlisted types.

## Exact supported operations

Host audit—not all native-supported:

| Method | `args.request` | Success |
| --- | --- | --- |
| `session/rename` | `{sessionId,title}` | `{title,seq}` |
| `workspace/archiveSession` | `{sessionId,stopActivity?:boolean}` | `{archivedSessionIds:[…]}` |
| `workspace/unarchiveSession` | `{sessionId}` | `{archivedSessionIds:[…]}` |
| `workspace/pinSession` / `unpinSession` | `{sessionId}` | `{pinnedSessionIds:[…]}` |
| `session/fork` | `{sessionId,atSeq?:number}` | `{sessionId:newChild}` |
| `workspace/delete` | `{workspaceId}` | `{deleted:true}` |

Declarations: [session methods](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/api/session-controller/src/index.ts#L404-L423>)/[fields](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/api/session-controller/src/types.ts#L308-L330>), [workspace methods](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/api/workspace-controller/src/index.ts#L119-L186>)/[fields](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/api/workspace-controller/src/types.ts#L86-L150>).

### Implemented Rust registry surface

- `NativeClient::{pin_session,unpin_session,archive_session,unarchive_session}` accepts `&SessionRegistryRequest { session_id: SessionId }`; returns `PinnedSessionsValue` or `ArchivedSessionsValue` ordered vectors.
- [DTOs](<../transport/src/dto.rs#L111-L193>)/[methods](<../transport/src/http.rs#L451-L487>) reject arrays/unknown/duplicate keys; no `stopActivity`, Agent or signal fields—even false is omitted.
- [Fixtures](<../transport/tests/transport.rs>) cover envelopes/order/bounds/correlation/retirement/cancellation. Returned sets are receipts, not feed replacements.

### Pin and archive semantics

- **Registry-global** sets. [Pin](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/workspace/workspace/src/index.ts#L410-L463>) prepends most-recent-first; existing pin is unchanged. Unknown session fails; archived pin is bad-request. Unpin/restore are idempotent, including disappeared sessions; [error mapping](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/api/workspace-controller/src/commands.ts#L197-L226>).
- [Archive](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/workspace/workspace/src/index.ts#L346-L407>) refuses reported turns/subagents/jobs/schedules, retains logs/accounting and removes pin atomically. [Gate](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/api/session-controller/src/archived-session-gate.ts#L23-L55>) blocks later steps for session/subagents. Restore does not restore pin.
- **Never escalate to `stopActivity`:** actual [write-before-stop order](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/workspace/workspace/src/index.ts#L371-L383>) contradicts request prose. Stop request/failure proves neither idle nor archive rollback.

### Rename and fork semantics

- [Rename](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/api/session-controller/src/commands.ts#L188-L211>) [resumes/resolves Agent](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/api/session-controller/src/agent.ts#L183-L227>), not cold registry metadata. [Title acceptance](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/session/session-title/src/index.ts#L390-L420>) sanitizes/normalizes/UTF-8-truncates; appends user `session/title`, no model history, overriding automatic titles; [projection](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/session/session-title/src/index.ts#L262-L275>).
- [Fork](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/api/session-controller/src/commands.ts#L221-L303>): inclusive existing nonnegative safe-integer `atSeq`; omission requires completed-turn prefix. Uses current default model, inherited preset/cwd/history and [workspace choice](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/api/session-controller/src/commands.ts#L568-L577>); [seed closers](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/core/session/src/fork.ts#L10-L29>) leave source unchanged.
- **Fork errors/timeouts can leave a child**, including attach failure. No automatic retry or guessed recovery; each retry may create another child.

### No session deletion or storage manipulation

No audited `session/delete`. Never remove persistence files or substitute archive. [Workspace removal](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/api/workspace-controller/src/commands.ts#L93-L104>) deletes registration only, [retaining directory/logs](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/workspace/workspace/src/index.ts#L300-L309>).

## Stream-authoritative state and races

- [Follow DTOs](<../transport/src/dto.rs#L477-L509>)/[feed](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/api/workspace-controller/src/feed.ts#L80-L139>): baseline then ordered upsert/remove/order/archive/pin; archive/pin frames are complete replacement sets.
- Current generation/feed wins: no comparable HTTP/stream revision. Fence ACK by epoch/target/serial/view; settle matching operation without installing older sets. Preserve pin order; reconnect replaces, never merges stale sets.
- Queue rejection is known-not-sent; admitted timeout may follow a write. **No automatic mutation retry.** Immediate RPC adoption needs explicit stream-progress fencing/race tests.

## Export: separate bounded binary operation

- [GET route](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/session-query/session-log-export/src/routes.ts#L7-L11>)/[handler](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/session-query/session-log-export/src/index.ts#L73-L168>), not `session/export` RPC; optional descendants may include ordinary forks. Web “requested” is not completed download.
- [Live flush/root materialization](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/session-query/session-log-export/src/archive.ts#L72-L168>) precedes streaming; cancellation is not rollback.
- Native Review/Confirm, 4 MiB/deadline, root attachments, structural-only ZIP checks and exclusive NOFOLLOW 0600 save. Failure after creation may leave partial/complete file; no extraction/overwrite/deletion/retry. Close joins saves without hard filesystem deadline. See [full export limits](<NATIVE-SESSION-EXPORT.md>).

## Recommended worker/UI integration order

1. Pin/unpin: reversible registry-only; preserve global order.
2. Archive/restore without forced stop; reachable archived view, not deletion.

Implemented with reviewed actions/current-feed authority. [Application qualification](<NATIVE-APP-QUALIFICATION.md>); **local evidence, ignored on GitHub:** [composed registry](<../app/evidence/parent-registry-qualification.json>). Scripted ACKs/feeds/clean exits do not qualify physical input, later builds or parity. Rename/fork remain pending; permanent deletion unsupported.
