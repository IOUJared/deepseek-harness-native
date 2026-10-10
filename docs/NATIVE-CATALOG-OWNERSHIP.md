# Catalog read ownership

Model-directory metadata is **not entitlement or quota**. Existing normal startup and picker-opening reads remain automatic and may contact providers; smoke mode never reads the catalog.

| Boundary | Production rule |
|---|---|
| Read identity | [Global read state](<../app/src/catalog_read.rs>) owns one `(transport epoch, non-reused serial)` flight. No zero epoch, wrap or new read after serial exhaustion. Queue refusal consumes the serial. |
| Popup/session | Close/reopen may join the current global read. Session changes do not invalidate directory metadata; neither popup nor session generation is the read identity. |
| Receipt | [UI reducer](<../app/src/ui.rs>) settles only the exact current ticket once. Unsolicited, wrong-epoch, retired, stale or duplicate success **and error** cannot replace current flight/cache/default/error/status. |
| Loss/close | Core exit, fault, no backend, stopped, observation loss, shutdown and disabling overflow retire the read. Closing/unverified UI does not apply a receipt. Old cached model names are not renewed authority. |
| Worker | [Command/result routing](<../app/src/worker.rs>) retains the exact ticket through success, error and refusal. Wrong epoch/smoke rejects before RPC construction. When first polled, a read rechecks live closing, closed shutdown owner and lost UI delivery. Existing operation cap and priority cancellation remain. |

These are trusted same-process enum/channel ownership checks, **not hostile-wire authentication tokens, cross-App nonces, cancellation of an already-started provider read, or an atomic shutdown/un-send guarantee**. Current backend epoch is fixed for its owned lifetime; serials remain distinct after retirement even if that epoch is reused. No new backend wire/API/config/dependency format.

## Verification owners

- [State tests](<../app/src/catalog_read.rs>): zero/wrong epoch, one flight, completion once, retirement/reuse and maximum serial.
- [UI tests](<../app/src/catalog_ui_tests.rs>): stale success/error/duplicates, unsolicited/wrong epoch, all terminal/lost-observation events, global flight, queue refusal, cached owned error, exhaustion, inactive close and actual reserved shutdown watch.
- Worker tests: correlated refusal/failure, wrong epoch/smoke/stopping, and shutdown/lost delivery/closed shutdown owner between future construction and first poll, without a client or network.
- Existing picker regressions retain cached selection, close/reopen joining, startup reads, and normal broad Stop despite uncertain/missing activity. Existing example tests/checks own all changed consumers.

[Local frozen evidence](<../app/evidence/catalog-ownership-qamfwnwa/>) retains initial compile/test failures and read-only review fixes. Frozen input uses the prior qualified source plus targeted catalog deltas; unrelated live projection, manifest, draft-store, license, visibility and newly introduced examples are excluded, not overwritten.

Live provider catalog execution, physical input/IME/accessibility, actual accounts, subscription quota/reset/alerts/retry, generation/sketch and cross-platform/full parity remain separate qualification or unavailable capabilities. No runtime/account/profile/desktop installation changes. See [Codex limits](<NATIVE-CODEX-USAGE-POLICY.md>) and [development package limits](<NATIVE-DEVELOPMENT-PACKAGE.md>).
