# Native live-decision ownership

## Scope

Worker-admitted approval/untimed-question replies require an exact delivery, worker key, supported request and explicit valid answer. **Timed/continued UI is unsupported**. Later [real composed qualification](<NATIVE-COMPOSED-DECISIONS.md>) is separate from earlier model/fake-wire evidence and frozen packages.

## Exact transport delivery

- [Mux intake](<../transport/src/event.rs>) captures opaque non-Clone/non-serializable `EventDelivery`; `next_event()` returns `OwnedRemoteEvent`. Only Waterfall carries authority; DTO-only `next()` discards it without replying.
- Cancel, root teardown, invalidation and matching ACK revoke it. `is_live()`/`cancelled()` indicate local retirement, **not exclusive authority or business outcome**.
- `reply_approval_for`, `reply_question_for`, `cancel_question_for` check generation/kind/exact Arc before queueing, after RPC capacity and before send. Old ACK cannot revoke a reused ID's new delivery.
- Errors are fixed/redacted. A transmitted request may settle despite read cancellation; no rollback or guaranteed NotSent.
- Legacy ID-based futures capture exact delivery at invocation, before polling, but do not validate Agent/call/native answers. New code uses owned APIs; exhaustive consumers need updates. Wire/session formats are unchanged.

## Worker admission and UI keys

- [Coordinator](<../app/src/decision_worker.rs>): **64 entries / 512 KiB**; binds Agent/event/request/owner/checked serial. GUI gets `Key {epoch,event_id,serial}`, never capability.
- UI rejects missing/replayed/zero keys. Shared decoder refuses timed/malformed/oversized batches. Answers require exact IDs, unique offered labels, valid multi/custom combinations, bounded nonblank custom text and **≤32 KiB** complete size. Empty selection/no custom is explicit skip.
- Worker fences epoch/smoke/shutdown/liveness/key/capacity and one attempt. Attempts strictly increase; active duplicates are ignored before capacity rejection. Question Cancel never stops a Turn; Unavailable cannot substitute for choice.
- Cancel removes entry; ACK retires exact delivery. Failure retains only current authority for an explicit newer attempt. Do not sweep in-flight entries merely on token retirement: ACK precedes task receipt. Shutdown aborts/joins owners, not remote writes.

## Verification and limits

Ignored [verification](<../app/evidence/decision-ownership-01/verification.json>) is **local-only**: seven transport groups, five privacy doctests, 46 worker/19 interaction tests; 12 coordinator groups/helpers retested; all-target compilation passed. Fake-wire barriers/model owners do not establish real Host or physical input; late-ACK cleanup also relies on exact-Arc source review.

[Timed claims](<NATIVE-TIMED-QUESTIONS.md>) require separate lease/race coordination beyond ACK. No automatic approval/answer/delegation, provider request, policy mutation or countdown.
