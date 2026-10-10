# Managed Codex accounts: native Core reference

## API

The native Host uses `ctx.get('codexAccounts')` when present. Managed status, Add, cancellation, selection and local logout never read or begin the legacy grant flow. Service loss fails closed; a Host without the service retains the legacy one-click flow. Native profile activation is owned by [the isolated-profile initializer](<../core/runtime/codex-profile.mjs>), not a final configuration overlay that would shadow editable model routes.

| Core command | Private command | Reply |
| --- | --- | --- |
| `CodexAccountsStatus` | `codex-accounts-status` | `CodexAccounts` |
| `CodexAdd { label, expected_epoch }` | `codex-add { label, expectedEpoch }` | `Codex`, immediate attempt receipt |
| `CodexSelect { account_id, expected_epoch }` | `codex-select { accountId, expectedEpoch }` | `CodexAccounts` |
| `CodexSignOut { account_id, expected_epoch }` | `codex-signout { accountId, expectedEpoch }` | `CodexAccounts` |
| `CodexSignOutAll { expected_epoch }` | `codex-signout-all { expectedEpoch }` | `CodexAccounts` |

All private controls also carry `requestId`. Account replies have exactly `mode`, `accounts`, `selected`, `epoch`, `legacy`, `added`. Managed accounts contain only `{opaqueId,label,email:null,plan:null}`; selection is nullable and epoch is a committed safe integer. Legacy replies have empty accounts, null selection/epoch/added and `legacy:'absent'`; this is not an account-identity claim. Managed legacy grants are reported as `unadopted` or `absent`, never imported automatically.

`CodexAccountId` privately stores a strictly lowercase `account-UUIDv4`. `CodexAccountLabel` validates 1–128 Unicode scalars, at most 512 UTF-8 bytes, nonblank and without control/format/surrogate/line/paragraph separators. Labels preserve their original text, including surrounding whitespace; Add UI may trim explicitly. Both expose `new(String)` and `as_str()`; Debug is redacted. Account DTOs are non-Copy and separate from existing `CodexMetadata`. Descriptions allow at most 128 accounts and 61,440 UTF-8 JSON bytes for the complete account reply; oversized metadata is rejected, not truncated, leaving space for private framing. Add may commit at epoch zero without changing selection. Rust's format-character table matches the current Node runtime; authoritative Node Unicode-property validation runs before the pipe.

## Add and Connect

Add creates an unselected account. Successful completion preserves the service's exact durable Add receipt as `added:{accountId,epoch,attempt}`; later status epochs cannot rewrite its commit epoch. `CodexStart` in managed mode uses the local display label `ChatGPT account`, not an inferred issuer identity. Core never selects automatically. A Connect consumer may explicitly select only that exact id and commit epoch while its original attempt, panel ownership and visible connection consent remain current. Uncertain, cancelled or panel-unowned completion cannot authorize automatic selection.

`credentialStored` in existing Codex metadata means a selected managed account exists, not legacy grant presence, fresh validity, entitlement or quota. Route activation retains its settings revision CAS and does not choose a model or send a request.

## Recovery and privacy

- All mutations pass explicit committed epochs; stale decisions fail with `CodexEpochConflict` / `CODEX_EPOCH_CONFLICT`.
- Service ownership compares Cordis's stable `Symbol.for('cordis.original')` instance, not its fresh context-bound proxies. Calls use the current context proxy; service replacement cannot redirect cancellation away from the captured login owner.
- Mutations are serialized. Cancellation and consuming callback input remain outside the mutation queue and reach in-flight login. Stop awaits login, queued work and cancellation before provider disposal.
- Unknown selection/logout results remain sticky. Only an explicit non-mutating `CodexAccountsStatus`, after in-flight work settles, reconciles them; no automatic write retry occurs. Plain `CodexStatus` does not reconcile.
- Cancel is not logout, rollback or grant deletion. A late Add remains unselected and has no usable automatic-connect receipt. Local sign-out does not revoke issuer credentials or select a replacement account.
- Browser URL validation, attempt/prompt freshness and consuming callback buffers remain unchanged. Tokens, issuer ids, diagnostics and authentication files never enter account DTOs or public results. Email, plan and quota remain unknown, not fabricated.
- The external-token Codex app-server and quota projector remain experimental and unwired; this API does not grant Beta consent.

## Native roster freshness

The native owner invalidates cached account-list freshness when Add/Connect sign-in is admitted and when an explicit Refresh accounts begins. An interrupted connection chain, terminal login observation, cancellation metadata or failed/refused/unsent read cannot make the pre-login roster writable again. Cached metadata remains internal; another owned account read restores the list and explicit controls. Even a known unsent login conservatively requires that read, without inventing write uncertainty or retrying automatically.

This is distinct from the SDK selection epoch: Add can commit a new **unselected** account at the same epoch. Native [production reducer and actual-App queue regressions](<../app/src/codex_managed_tests.rs>) cover failed reads, interrupted Add, defensive terminal cancellation metadata and same-epoch readback; [the queue regression](<../app/src/codex_ui_tests.rs>) exercises Settings → Controller → App admission with PUBLIC metadata and no backend/browser. These fixtures are not real OAuth cancellation, physical input or evidence of a backend wrong-epoch mutation. Failed reads neither introduce account-write uncertainty nor release existing uncertainty.

Keyless [actual Cordis proxy regressions](<../core/runtime/codex-cordis.test.mjs>) require the adjacent built alpha Cordis package. [Bridge regressions](<../core/runtime/codex-managed.test.mjs>), [actual private-command parser tests](<../core/runtime/codex-private-commands.test.mjs>) and [typed parser tests](<../core/src/codex_accounts_tests.rs>) use synthetic public fixtures. They do not qualify real sign-in, issuer revocation, provider generation or physical UI input.

The [native local-account/Maintenance scenario](<NATIVE-MANAGED-CODEX-COMPOSITION.md>) exercises the real App/worker/Core/full-alpha path with new owned PUBLIC stored records, explicit selection/logout receipts and restart. Its [native admission compatibility note](<NATIVE-MANAGED-CODEX-UPGRADE.md>) documents empty-all and already-selected Switch rejection before dispatch; a disabled button alone is not enforcement. Core/SDK selection semantics are unchanged; native Switch is not an account/token refresh.
