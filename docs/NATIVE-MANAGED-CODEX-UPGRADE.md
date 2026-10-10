# Native local-account admission guards

## Roster freshness after interrupted sign-in

Native Add/Connect sign-in admission and explicit Refresh accounts now invalidate cached roster freshness. Failed reads or an interrupted login chain cannot restore stale Switch, Add or local-sign-out controls. An owned account read restores them; a newly added unselected account can retain the same selection epoch. Even a known unsent login conservatively needs another read. No automatic retry or new write-uncertainty flag.

[Source qualification](<../app/evidence/account-roster-freshness-25tj1srr/qualification.json>): **four behavioral RED → GREEN regressions**, including actual Settings → App queue denial/recovery; **797 default/804 PUBLIC App/809 shared-example tests pass, one ignored each**, both releases and PUBLIC all-target check. Existing local accounts, usage/runtime restarts and default-main keyless startup pass: **nine full-alpha starts/graceful stops, zero observed survivors, 12 inspected of 20 captured frames**. Five consent frames remain visible. Failed source-closure/output-path attempts are retained, not counted as behavioral RED or Host starts.

This qualifies frontend freshness, not real Add/OAuth cancellation or a backend wrong-epoch write. SDK/API/profile/persistence semantics are unchanged. The frozen baseline separately pins its already-owned Cargo winit patch because the package source selector excludes vendor sources. **This source qualification did not rebuild an archive.** The [subsequent dae2 development package](<NATIVE-DEVELOPMENT-PACKAGE.md>) includes the freshness guard and its previously omitted vendor sources; extracted source compilation and blank-session main startup are separately qualified. No installation, publication or live-account qualification. See [freshness semantics](<NATIVE-MANAGED-CODEX.md#native-roster-freshness>).

## Already-selected account

Native **Switch** now rejects review and confirmation for the already-selected account; confirmation also rechecks roster membership. The same predicate controls Switch and its review's Confirm button. Different-account switching and selected-account local sign-out remain unchanged. No SDK/service API, schema, dependency or profile change.

The [exact alpha SDK](<../../deepseek-harness-linux/packages/llm/llm-pi-ai/src/codex-account-owner.ts#L278-L330>) does **not** make same-ID selection a no-op: it retires/drains the binding, increments and commits an epoch, then rebinds. This native change enforces an existing disabled-button decision in the [owner](<../app/src/codex_managed.rs>), not a token-refresh interpretation or backend semantic change.

[New source qualification](<../app/evidence/selected-account-guard-qlnmtfe3/qualification.json>): two owner tests fail on the frozen previous implementation, then pass with the guard; **757 default/764 PUBLIC App/767 shared-example + one ignored each**, both releases. After genuine select-A ACK/read at epoch 1, actual App rejects repeated review/confirmation without operation-ticket changes; another genuine read still returns 1/A before valid switch-B. Accounts, usage/runtime restart regressions and default-main startup pass: **nine native/full-alpha starts, 15 inspected frames, zero observed survivors**.

Composed tickets check effect admission, not absence of transient review state; the unit owner explicitly checks no review. The defensive same-epoch metadata-edit confirmation test is not a demonstrated SDK race. No physical-input/live-auth/network-instrumentation claim. **The preserved aeaa archive predates this guard.** The [earlier 5ddf package](<NATIVE-DEVELOPMENT-PACKAGE.md>) includes both native admission guards; fresh main/UI/extracted full-alpha startup checks pass. Those blank-session starts do not repeat account mutations. No installation or live-account qualification.

## Empty-list local sign-out

The native Account owner now rejects **Sign out all** review/confirmation when its current roster is empty, matching the already-disabled button. Confirmation rechecks the roster as well as the captured ticket/epoch. Nonempty reviewed local sign-out behavior is unchanged.

The exact alpha SDK's empty sign-out-all is not a harmless read: it cancels login, retires selection and commits an epoch change. A disabled native button must not leave that mutation admissible through direct/replayed native actions. This change is enforced by the [production owner](<../app/src/codex_managed.rs>) and covered by [owner regressions](<../app/src/codex_managed_tests.rs>).

No SDK/service API, persistence version, account/grant format, dependency, profile or existing account data is changed. The previous 6ffa development archive predates this guard. The [new aeaa archive](<../app/evidence/accounts-package-f6s3vl4c/archive/dsh-native-linux-x86_64-development-0.1.0-aeaa6939e5d6.tar.gz>) includes it; fresh default-main/PUBLIC tests, native rendering and extracted full-alpha keyless starts pass. Packaging is not installation or live-account/physical-input qualification. See the [package reference](<NATIVE-DEVELOPMENT-PACKAGE.md>) and [managed-account reference](<NATIVE-MANAGED-CODEX.md>).
