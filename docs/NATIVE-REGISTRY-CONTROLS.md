# Native session pin/archive controls

Pin/unpin and archive/restore are reversible metadata operations, **without activity-stop authority**. They are not deletion, rename, fork, export or parity.

## Operator flow

- Select an ordinary session → **Manage** → review → explicit Confirm. Current roster, Root and registry baseline are required; new registry frames invalidate review. No automatic retry.
- **View archived** opens read-only history; Restore returns to recent sessions without repinning. Archived/unbaselined sessions cannot Send.
- Archive checks active turns/jobs/subagents/schedules, retains logs/workspace accounting and durably unpins. No `stopActivity` or forced cancellation.
- Pins are registry-global, newest pin first; repinning does not reorder. Full arrays replace snapshots. Cached ranks avoid repeated scans. Bounds: **200 displayed / 4096 roster / 8192 registry IDs**.

## Implementation and lifetimes

- [Transport](<../transport/src/http.rs>): `pin_session`, `unpin_session`, `archive_session`, `unarchive_session`; strict `sessionId` DTO rejects unknown keys, arrays, `stopActivity`, `agent`, `signal`. Cookie/Origin, bounds and revocation remain.
- [Worker](<../app/src/worker.rs>): owned epoch, no smoke/stopping writes, eight operation slots, one management flight across sessions; no Agent resume/model/catalog calls.
- [Reducer](<../app/src/management.rs>): epoch/selection/target/operation/review/panel fences. Closing hides controls, **not an admitted write**; disconnect invalidates old receipts. Modal background actions are inert.
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

These fingerprints do not qualify later shared-source changes. Human input/IME, compositor fractional scale, every-provider active-work refusal, cross-client races, arbitrary-profile containment and parity remain unqualified. Model/network absence is source/counter scope, not packet tracing; nonempty-history retention was not independently compared.

[Exact alpha contract](<NATIVE-SESSION-MANAGEMENT.md>).
