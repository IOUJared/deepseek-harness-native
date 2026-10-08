# Native account observation

Private supervisor/Iced reference; full isolated alpha profile.

## Deferred work

- No startup `AccountSubscribe`; readiness/sessions/workspaces are independent. Sidebar starts **not loaded**, not signed out. Disconnect clears labels; late metadata drops; warnings do not disconnect/resubscribe.
- Lightweight `deepseekAccount` binding remains. `watch()`/`getPlatformSession()` require explicit Core subscription; one owner, repeated subscriptions share it.
- Priority unsubscribe revokes intake/mirrored session and joins watch/session **and all admitted raw `getState()` reads**. Non-abortable reads remain owned and block replacements; late credentials cannot publish. Old disposers cannot clear newer owners.
- Provider replacement/loss/watch failure requires explicit resubscription. No retry/sign-in/sign-out/profile/balance/model/key mutation. Subscription and priority cancellation are separate from the mutation lane.
- Retirement requires watch completion plus empty read registry; do not self-join wrappers. Observation teardown never rolls back/confirms credential writes. App currently exposes no Platform observer action.

## Intentionally retained work

Settings → API login, explicit Refresh and save preflight still perform one-shot `credentials.describe()`/`account.getState()` metadata reads. No persistent observer/token export/server credential validation. Codex has its separate explicit flow.

Full profile still mounts account/credential/settings/LLM services, may read stored grants, maintains credential watchers and creates/loads browser signing material. **Not an account-free Host or complete browser/plugin laziness.**

## SDK compatibility

Native 0.1.0 changed eager mirroring/notification-only unsubscribe to explicit observation plus read-draining cancellation. Handle unavailable/timeouts; abort cannot settle an unresponsive provider. No public schema/session/upstream version changes.

## Evidence and limits

Ignored evidence is **local-only**: [priority cancellation](<../app/evidence/parent-account-cancellation-qualification.json>) / [earlier observer](<../app/evidence/parent-lazy-account-qualification.json>).

| Historical evidence | Result / limit |
|---|---|
| Real profile | Zero observed native watch/session calls at Ready/two metadata reads; two explicit watches ended, max one live; clean exit |
| Helper/oracle tests | 18 fake-provider cases/four Python guards; not actual blocked-provider/HMR qualification |
| Core/App | 50 + six privacy doctests; 280 App tests, later 285 save-policy regressions |
| 20s blank production sample | 31 late samples: median RSS 333,536/PSS 262,983 KiB; CPU median/mean/max 0/0.43/1.91%; ~555ms map proxy |

Counters start after boot, not whole-runtime tracing. Sample is not first-frame timing or matched improvement. Hot replacement, physical input, unknown-save paint/timing and arbitrary-provider containment remain unqualified; teardown cannot certify throwing cleanup. Packages stay frozen.
