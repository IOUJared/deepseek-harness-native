# Unknown native API-key save receipts

## Native app policy

- An uncertain write retains its original `Option<Ticket>` independently of panel phase. Only an exact pending receipt—or loss of its lifecycle context—latches uncertainty.
- While latched, Edit/Review/Confirm are rejected; blocked one-use secrets are consumed/dropped. Editor/Save are inactive; uncertainty stays visible across hide/reopen/navigation/epochs.
- Presence/writability metadata, read failures and stale/late receipts **cannot unlock replacement**. Explicit nonsecret Refresh and unrelated controls retain their own guards.
- `pending()` means awaiting a receipt, not uncertainty. Matching Confirmed/Refused/definite NotSent permits a later reviewed save. Timeout/error means **unknown**, neither persisted nor unsent.

## Ownership and recovery

- This is conservative UI policy, not a concurrent-write repair. Worker preflight and ten-second Core wait are unchanged; worker completion may precede Host mutation settlement.
- Supervisor reserves its mutation lane synchronously until the awaited handler/setter settles. UI neither cancels/rolls back the write nor extends deadlines.
- Recovery requires **fresh app construction after the owned backend fully stops**. Ready/reconnect/epoch changes do not prove replacement. No same-app reset, generic Retry or metadata bypass exists.
- Restart does not validate/undo the key. There is no key readback; trusted-provider side effects may outlive process/request failure. Teardown is not transactional rollback.
- Guard can remain latched after an actually-unsent request or settled handler. It does not globally prohibit direct Core/SDK/worker commands; their own fences remain.

## Upgrade note

Native 0.1.0 formerly cleared visible uncertainty on close; now the original receipt blocks API editing/saving for the app lifetime. Public schemas, upstream versions, session/SQLite formats, Node lane and cancellation semantics are unchanged.

## Evidence

Ignored [parent qualification](<../app/evidence/parent-save-uncertainty-qualification.json>) is **local-only, not a GitHub download**. Five [coordinator tests](<../app/src/settings_uncertainty_tests.rs>) actively attempt replacements and check original receipts, consumed secrets and known-outcome recovery.

[Real PUBLIC save fixture](<NATIVE-COMPOSED-SETTINGS.md>) verifies successful App→worker→Core→full alpha profile persistence, **not a held provider or real ten-second timeout**. Unknown-state paint, physical input, held-write shutdown/timing and arbitrary-provider confinement remain unqualified; no archive or parity claim.
