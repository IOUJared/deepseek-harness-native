# Unknown native API-key save receipts

## Native app policy

The API-login coordinator now retains an `Option<Ticket>` for an uncertain write, independently of the visible save phase. Only an **exact pending receipt** can latch an indeterminate result; losing the lifecycle context of an actual pending receipt also latches it. A closed/hidden/reopened panel cannot hide that receipt and then silently admit a replacement key.

While that receipt exists, the native API panel rejects new Edit, Review and Confirm actions; blocked one-use input holders are still consumed/dropped. The editor has no input handler and Save has no active action. A prominent uncertainty notice is rendered independently of phase. Explicit nonsecret metadata refresh, category navigation and the existing unrelated Codex/plugin/export controls remain governed by their own guards. Presence true or false, writable metadata, read failure, close/reopen, page changes, transport/readiness epochs and late/mismatched save results cannot unlock another API-key write.

`pending()` still means a submitted operation awaiting its matching result—not uncertainty. Existing draft wiping, exact-ticket completion, closed-panel confirmation fencing, consuming secrets and reviewed explicit confirmation are unchanged. Matching Confirmed, Refused or definitely NotSent results do not latch uncertainty and permit a later separately reviewed save. Timeout/error remains **unknown**, not proof of persistence or of an unsent request.

## Ownership and recovery

This is an additional **conservative native UI policy**, not a repair of concurrent Node writes. The unchanged worker performs metadata preflight and a ten-second Core wait; an error maps to Indeterminate. The matching local worker slot can finish while the actual Host mutation is still pending. The production supervisor already reserves its mutation lane synchronously, rejects overlapping mutation commands, and releases it only after the awaited handler/credential setter settles. The new UI policy neither cancels nor rolls back that admitted write, waits indefinitely on the UI thread, nor extends any Core/SDK deadline.

Recovery is a **fresh app construction after the owned backend has fully stopped**. No same-app reset protocol is added. An epoch/Ready/reconnect is not proof a different Host incarnation has replaced the previous owner. Restarting does not undo or validate a key and does not acknowledge the unknown attempt. Nonsecret metadata cannot identify which key is stored; the app has no key-readback API. Custom trusted provider side effects can outlive a failed request or process; owner teardown is not a transactional rollback.

This policy deliberately blocks even a corrective explicit replacement after the old Node handler has actually finished, and can latch after a transport/pre-dispatch error or disconnect even if a later result would have been NotSent. There is no metadata-based, generic Retry or hidden reset bypass. Core/SDK users and direct worker commands are not globally prohibited by this UI guard; their existing authorization/ownership/mutation fences still apply.

## Upgrade note

Native app 0.1.0 formerly cleared the visible indeterminate phase on close and allowed a fresh draft/review. It now retains the original uncertainty receipt for the Settings/app lifetime and keeps API editing/saving disabled after an unknown result. Restart-only recovery is intentionally more restrictive. No upstream package version, public command schema, session/SQLite format, Core request timeout, Node lane or admitted-write cancellation semantics changes.

## Evidence

[Parent qualification](<../app/evidence/parent-save-uncertainty-qualification.json>) records the completed checks and pins source/executable/run identities. Five [new coordinator tests](<../app/src/settings_uncertainty_tests.rs>) cover fresh blocked inputs with both presence values, closed/hidden/reopened results, pending disconnect/epoch adoption, receipt retention through later contexts and stale results, and nonsticky known outcomes. They assert the **original** receipt, consumed blocked holders and no Save effects; unlike the old empty-draft check, they actively attempt a replacement.

The regression uses the existing [actual native PUBLIC-value persistence fixture](<NATIVE-COMPOSED-SETTINGS.md>): successful explicit Review/Confirm still traverses real App → worker → Core → same-version full public profile/default provider and is independently verified. Its source-controlled PUBLIC dummy is not a real credential. This success fixture does **not** simulate an unresponsive provider or qualify a real ten-second save timeout. Sticky-unknown coverage is deterministic reducer testing; physical input, uncertain-state paint, actual held-write shutdown/timing and arbitrary provider confinement remain unqualified. Paused primary UI/layout is not edited or resumed; only the API controller and its status content change. Installed runtime/profile/Web GUI, real credentials and desktop configuration are preserved. No new archive/installer or full parity claim.
