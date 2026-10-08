# Native ChatGPT sign-in and GPT models

## Use

Restart the rebuilt native application, then open **Settings → Codex**. Navigation and startup readiness do not contact OpenAI or load models.

1. Click **Connect ChatGPT**. That one app action reads local status, starts the owned authorization flow when necessary, opens the official OpenAI page once, and enables the missing `openai-codex` model route after the matching successful authorization. It preserves existing configuration and the current session's model.
2. Complete any sign-in, MFA or approval required by OpenAI in your browser. The provider handles PKCE, callback/state verification, token exchange and credential storage automatically. These provider-owned steps cannot be bypassed by the app.
3. After connection, close Settings, use **Load models**, select an available OpenAI Codex/GPT model for the session, then explicitly Send. Connecting does not load models or send a prompt. Subscription entitlement, usage limits and model availability remain provider-owned; not every OpenAI API model is unlocked.

**Advanced** contains explicit status checks, browser reopening and the masked Review/Submit callback fallback if the provider's local callback listener is unavailable, plus the separately reviewed route controls. These are not required in the normal browser flow. An existing stored credential and route are treated as locally configured, not as a fresh account-validity check.

This first pass supports the browser flow only. Device login, sign-out, account switching and a token/URL viewer are not provided. It does not invoke the Codex CLI or copy existing CLI credentials.

## Ownership and privacy

The same-version isolated `0.2.1-alpha.1` Host owns `llm-pi-ai/openai-codex` authorization and credential storage. The native Core adds a fixed private adapter, not a new Remote authorization namespace. OpenAI OAuth itself remains the installed pi-ai `0.87.1` implementation. The installed RC runtime and current Harness profile are untouched.

Authorization URLs, callbacks, tokens and authorization-provider notices/errors never enter public UI events, model requests, logs or qualification reports. Core keeps one attempt-scoped browser capability, validates the exact installed OpenAI HTTPS authorization origin/path/query and launches only absolute `/usr/bin/xdg-open`, without a shell. The launcher receives only an allowlisted caller desktop environment and fixed PATH; ambient API keys, NODE_OPTIONS and BROWSER overrides are excluded. Cancel, terminal state, fault, stop and Drop revoke browser authority. A cancellation tombstone also rejects an older delayed URL arriving after the initial metadata acknowledgment.

Callback input accepts 1–16384 printable non-whitespace ASCII bytes without trimming. It has one consuming opaque owner, independent editor/operation/review tickets, explicit confirmation, and no reveal/readback. Owned Rust editors, input holders and control frames are wiped. Browser argv, URL-parser allocations, Iced/clipboard/allocator/Node/kernel copies and swap are not guaranteed erased; masking is not memory protection. Trusted custom credential-provider paths are not confined by the GUI.

## Cancellation and uncertain outcomes

The existing pi-ai store writes outside the authorization seam's commit callback. Cancellation can therefore settle before a late credential write. **Cancel is not sign-out, deletion or rollback.** Cancelled, failed or timed-out attempts block another sign-in for the current Backend lifetime; restart before retrying. Unknown native auth-command outcomes also fence unrelated credential/settings edits until restart. Explicit status reads remain available, but cannot retroactively prove a particular attempted login committed.

Closing or navigating away clears callback drafts/review and ends the remaining automatic connection steps, without cancelling or undoing an admitted attempt. Keep the Codex section open while connecting. Return to Codex and explicitly reconnect/check status or cancel the tracked attempt if desired. Application shutdown revokes private authority and stops the owned Host before joining blocking operations; the external browser is user-owned and is not killed.

## Model-route activation

The adapter resolves the unique actual provider directory entry, namespace and fixed `providers/openai-codex` path. It requires a configured grant record, writable settings and the mandatory expected revision. **Connect ChatGPT** explicitly authorizes this missing-route activation as part of connecting; the Advanced manual path retains its separate review/confirmation. It uses the Host's settings mutation/CAS seam and waits for reconciliation. Only a missing value becomes `{}`; an existing profile is never replaced. Unknown activation outcomes require a fresh explicit status read before another review. A confirmed route plus an advancing revision confirms configuration, not GPT entitlement or generation.

This dedicated adapter does not weaken generic Plugins' custom-presentation, secret or permission opt-outs. Auth operations have separate short-operation and retained-login reservations; native key/plugin/export writes cannot overlap an active or uncertain sign-in. Both UI and worker prohibit auth requests during real keyless smoke.

## Qualification

The [one-click qualification](<../app/evidence/codex-one-click-r8si3gst/qualification.json>) records release `a3ed4509b847ccda203e98cd84433cc352ad0a40dd7b14257c2c32577adbaa8c`: all-target checking, 259 app tests, 262 component-example tests, eight public component runs at application scales 1/1.25, and a real isolated-keyless alpha startup/fold/graceful-stop run. The two fresh Connect screens and real startup frame were inspected. Thirty new state-machine tests and five actual App/worker-queue tests cover the chained flow, failures, stale receipts and closure; the local render fixture simulates successful receipts without dispatching any effects. An independently reviewed early-terminal deferral race was fixed with both event-order regressions. These checks do **not** authenticate a real account or qualify actual OAuth/token exchange/model-route persistence.

The retained evidence distinguishes keyless reducer/worker/Core and staged mock authorization tests, public own-renderer fixtures, and real-alpha read-only metadata discovery. No agent run authenticates an OpenAI account, opens a browser, requests a model catalog, generates text, or copies existing credentials. Real human sign-in, token refresh, account entitlement, GPT generation, physical keyboard/pointer/IME and accessibility remain unqualified. See the Codex qualification record alongside the application build record for exact tested artifacts and counts.
