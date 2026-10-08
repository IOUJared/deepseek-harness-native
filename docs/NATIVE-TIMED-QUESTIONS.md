# Native timed-question transport ownership

## Scope

Transport has an acquired `QuestionClaim`; **native timed/continued UI remains unsupported**. No app coordinator, timer or timed answer channel is enabled. [Delivery ownership](<NATIVE-DECISION-OWNERSHIP.md>) retires on ACK and is **not a timed lease**.

## Host behavior and authority

- Request: `wait:{callId,timed?:boolean}`; timed requests use `timed:true`. No public deadline/lease field.
- `userQuestions/attachWait` requires the exact live registered root Agent/call. AgentId is not manufactured from SessionId, roster or history.
- First generator iteration acquires a **nonexclusive** claim and yields `{remainingMs}` from the original Host deadline. Opening alone is not acquisition; zero can be valid.
- Any claim suppresses expiry past that deadline; last release resumes original-deadline expiry, immediately if overdue. Disconnect/return/Host close releases ownership.
- Reply ACK can be an idempotent losing-client no-op. Retain the claim through **Host stream termination**, not ACK. End gives no answered/expired/aborted reason.

## Native API

- `Mux::hold_question(&AgentId,&ToolCallId)`: one queue/transmission/first-item timeout. End-before-item → `Error::QuestionUnavailable`; malformed/second items → `InvalidDto`. Exactly one object/unsigned-integer `remainingMs`, no extra fields.
- `QuestionClaim` is non-Clone/non-serializable. Duration is an opening sample, not renewed TTL/exclusive authority; no automatic answer, timer, retry or delegation.
- `wait_closed(&mut self)` has sticky terminals; abandoning the read preserves ownership. Check buffered closure and delivery before enabling answers.
- `release(self).await`/Drop unsubscribe locally, **not release ACK, rejection or Turn cancellation**. Retain through uncertain replies; independently fence generation/Agent/call/delivery/UI lifetime.
- Exported additions require exhaustive-match updates. Low-level `question_wait` remains, but open is not a grant. [Implementation](<../transport/src/question.rs>).

## Verification and remaining work

Ignored [verification](<../app/evidence/question-claim-foundation-01/verification.json>) is **local-only**: 73 transport tests, four privacy doctests, eight real-alpha wait-component tests, 12 interaction regressions and all-target compilation. Fake-wire/component tests are not full Host/App composition; pre-send queue-stall timing remains untested.

Remaining: app race coordination, claim retention after ACK, real composition/input/paint. Countdown must use received duration and checked monotonic arithmetic. Continued `answer_continued` true means queued, false means unavailable; reconcile projection/inbox admission/discard. Projection supplies neither Agent authority nor lease; no continued Cancel endpoint.
