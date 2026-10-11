# Native session pin/archive/rename/fork controls

Pin/unpin and archive/restore are reversible metadata operations, **without activity-stop authority**. They are not deletion, fork, export or parity. Rename is a separate title write with the same consent discipline. Fork is a session-creating write: it is not a metadata edit, and its child identity is authoritative only from the ACK.

## Operator flow

- Select an ordinary session → **Manage** → review → explicit Confirm. Current roster, Root and registry baseline are required; new registry frames invalidate review. No automatic retry.
- **Rename…** in the same row options opens a compact editor pre-filled with the title the operator currently sees. Local validation refuses a blank/whitespace-only title, one over **1024 UTF-8 bytes** (the bound `observe_title` already applies to `session/title`), or any control/invisible formatting character the Host strips; the editor stays open and keeps the typed text. Review → Confirm sends exactly one `session/rename`; Close/Cancel changes nothing and a rename in flight blocks another.
- **Fork…** in the same row options opens a review → explicit Confirm for the latest completed-turn prefix (`atSeq` omitted). It sends exactly one `session/fork`. Close/Cancel changes nothing, and a fork in flight shares the same single management/rename/export flight, so no second mutation can start. The child id is taken only from the ACK; no roster row, title or selection is predicted.
- Reveal is frame-backed, never ACK-backed. After the ACK, the child is selected only when this App's own roster or a workspace follow frame already contains it. A child that is not yet visible is reported with its authoritative id and **left unselected**, with no invented sidebar row; it appears only after a real roster/follow update.
- Indeterminate outcomes are explicit and never retried. `session/fork-unavailable` means the Host refused before creating a child (no completed turn, or no such cut) and is reported as no child. `session/workspace-attach-failed` means a child was created but could not attach; the redacted error does not return its id, so it stays indeterminate with no retry and no rollback claim. Refusal, failure, timeout and disconnect share that indeterminate report.
- **View archived** opens read-only history; Restore returns to recent sessions without repinning. Archived/unbaselined sessions cannot Send.
- Archive checks active turns/jobs/subagents/schedules, retains logs/workspace accounting and durably unpins. No `stopActivity` or forced cancellation.
- Pins are registry-global, newest pin first; repinning does not reorder. Full arrays replace snapshots. Cached ranks avoid repeated scans. Bounds: **200 displayed / 4096 roster / 8192 registry IDs**.

## Implementation and lifetimes

- [Transport](<../transport/src/http.rs>): `pin_session`, `unpin_session`, `archive_session`, `unarchive_session`; strict `sessionId` DTO rejects unknown keys, arrays, `stopActivity`, `agent`, `signal`. `session_rename` maps to `session/rename` with `SessionRenameRequest {sessionId,title}` → `SessionRenameValue {title,seq}` and rejects a blank or negative-seq ACK as `InvalidDto`. `session_fork` maps to `session/fork` with `SessionForkRequest {sessionId,atSeq?}` → `SessionForkValue {sessionId}`; `atSeq` is omitted when not requested, an out-of-range cut is refused locally, and an empty or source-equal child ACK fails closed as `InvalidDto`. `session/fork-unavailable` and `session/workspace-attach-failed` map to distinct redacted errors. Cookie/Origin, bounds and revocation remain.
- [Worker](<../app/src/worker.rs>): owned epoch, no smoke/stopping writes, eight operation slots, one management flight across sessions; a rename and a fork share that single flight with registry mutations and export and never call Agent resume/model/catalog.
- [Reducer](<../app/src/management.rs>): epoch/selection/target/operation/review/panel fences. Closing hides controls, **not an admitted write**; disconnect invalidates old receipts. Modal background actions are inert.
- Rename ([rename.rs](<../app/src/rename.rs>)): epoch/generation/target/ticket/editor fences, explicit Review → Confirm, and local validation repeated on the consumed draft. The sidebar title changes **only from the ACK**, which is applied once for the exact pending receipt; a malformed ACK or foreign/stale receipt changes nothing. Close hides the editor without rollback; unknown outcomes stay explicit and are never retried. The changed title never becomes a chat message.
- Fork ([fork.rs](<../app/src/fork.rs>)): epoch/generation/target/ticket fences, explicit Review → Confirm, and one `Submission` per admission. Only the exact pending receipt accepts a child, a child equal to its source is refused as indeterminate, and Close/disconnect fence late ACKs without rollback. The App returns the acknowledged child once, selects it only from its own frames, and reports unavailability, unattachment and uncertainty distinctly.
- **ACK is not state:** mutation full sets are discarded; current complete stream frames determine membership/order. Queue rejection is NotSent; admitted errors may be indeterminate. Lost ACK may accompany a committed streamed change. No rollback/retry is inferred.
- Invalid/duplicate/oversized lists or increments without baseline fail closed. Archive temporarily suppresses coupled pins. Identity is SessionId, not AgentId.

## Current evidence and remaining gates

Ignored evidence is **local-only, not GitHub downloads**: [qualification](<../app/evidence/parent-registry-qualification.json>).

| Historical checks | Result |
|---|---|
| Rust | 107 initially; 108 + five fixture guards = 113 |
| Runner | 34, including 19 registry guards |
| Transport | 26 fixture groups + identity compile-fail doctest |
| Real composed scales 1/1.25 | Six receipts/run, ordering/archive/restore, graceful zero exit; four inspected PNGs |

Rename is covered by five transport tests plus eighteen app tests (nine state-machine, six App/sidebar, three worker). Fork is covered by seven transport tests (success with omitted and explicit `atSeq`, malformed/empty/source-equal ACK, unavailability and attach-failure mapping, redaction/correlation, timeout and retired clone) plus seventeen app tests (eight state-machine, six App/sidebar, three worker). **No live Host rename or fork was executed**, so real title normalization, real fork seeding/workspace attachment and the durable child id remain unqualified.

These fingerprints do not qualify later shared-source changes. Human input/IME, compositor fractional scale, every-provider active-work refusal, cross-client races, arbitrary-profile containment and parity remain unqualified. Model/network absence is source/counter scope, not packet tracing; nonempty-history retention was not independently compared.

[Exact alpha contract](<NATIVE-SESSION-MANAGEMENT.md>).
