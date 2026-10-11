# Native conversation and diagnostic records

Routine inbox/policy JSON no longer appears as chat. A **Chat / Trajectory** tab strip above the conversation switches projection: **Chat** presents the conversation, **Trajectory** retains every raw row with read-only Details. Presentation creates no optimistic messages, generation, ACKs or file authority.

## One displayed-row projection

- [Shared iterator](<../app/src/reducer.rs>) drives App view, virtualization, durable anchors and active-transcript detection. Exact permission/preset, sandbox/mode, approval/policy and recognized ordinary inbox mutations are secondary.
- [Classifier](<../app/src/transcript_presentation.rs>) requires known target, safe integer coordinates, real mutation and exact recognized user/source/text/file shapes. Empty/**4 MiB** file metadata stays supported. This is presentation classification, not new alpha/provider validation.
- A `user/message` is human history only for the `user` producer; `runtime-context`, `time-context`, `skill-catalog` and every other declared source kind project to **Injected context** and stay out of Chat. A message carrying no kind is kept, because every real producer declares one and hiding an unlabelled human message is the worse error.
- Extensions/malformed/unknown producers/outcomes/cancellation remain primary; classification never uses truncated/redacted JSON. Exact canceled outcome displays red **Canceled queued input**. Unknown required events preserve warning/error and disable Send.
- Chat shows the conversation: human messages, assistant replies, tool activity, turn notices and the cancellation/splice kind. **Trajectory** owns the system prompt, developer injects, model-only context, `Injected context` and every kind in `known::LOG_EVENTS` (`session-log-*`, `command/*`, `todo/write`, `subagent/*`, …). The exclusion is a known list rather than an allowlist, so an unrecognized kind stays visible and a future required diagnostic cannot be dropped. Unusual records are not completed work or durable user-message history.
- All consumers share durable key/order/heights. Live text is cache-only trailing extent; disclosure count is raw count. Toggle captures old anchor, changes projection and rotates revision/widget ID; late prior-mode feedback drops. Neighbor fallback/native clamp may limit alignment; reader/follow intent survives.

## Evidence

Ignored evidence is **local-only**: [parent manifest](<../app/evidence/parent-native-records-projection-qualification.json>).

| Historical checks | Result / scope |
|---|---|
| Real composed scales 1/1.25 | Three uploads/Sends/ACKs/inbox claims per run; 5003-byte/empty/4 MiB storage, refusal guards, disclosure roundtrip, graceful Rust/Node exit |
| Inspected native captures | Routine JSON hidden; five raw rows disclosed; admission-not-generation status, one Settings, visible Stop |
| Tests | 435 driver tests incl. 11 new groups; 13 validator groups; feature check; later 443 feature tests with concurrent work |
| [Canceled paint preset](<../app/src/layout_fixture.rs>) | Actual App heading, zero worker/Host/inbox/model; **not live cancellation delivery** |

Older page expectations changed to displayed-row geometry. Source checkpoints/artifacts differ; test growth is not solely projection work. Scripts do not qualify physical input, models or user-message/history receipt retirement.

## Limits

Debug source qualification, not release/install or measured performance. Application scale is not compositor fractional scale. Physical input/IME/accessibility, OS chooser/drop, rich previews, OAuth/models, continued claims/guests, history retirement and distribution parity remain pending. Hiding routine metadata is not pending-message semantics or every-producer optimization.
