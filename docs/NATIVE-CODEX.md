# Native ChatGPT sign-in and GPT models

## Use

1. Restart the rebuilt app; open **Settings → Codex → Connect ChatGPT**.
2. Complete OpenAI's browser sign-in, MFA and approval. The provider handles PKCE, callback/state verification, exchange and storage; these steps cannot be bypassed.
3. Close Settings, explicitly **Load models**, select an available Codex/GPT model, then **Send**.

- Connect reads local status, opens the official page once and enables a missing `openai-codex` route after matching authorization. Existing configuration/session model stays unchanged.
- Navigation/connection never automatically loads models or prompts. Entitlement, limits and availability remain OpenAI-owned; not every API model is unlocked.
- **Advanced:** explicit status, browser reopening, masked Review/Submit callback fallback and separately reviewed route controls.
- Unsupported: device login, sign-out, account switching, token/URL viewer and Codex CLI credential import. Stored credentials are not fresh account-validity proof.

## Ownership and privacy

- Isolated `0.2.1-alpha.1` Host owns authorization/storage through pi-ai `0.87.1`; installed RC/profile stays untouched.
- URLs, callbacks, tokens and provider diagnostics stay outside public events, logs, reports and model requests.
- Attempt-scoped browser authority validates the installed OpenAI HTTPS URL; launches only `/usr/bin/xdg-open`, without a shell or ambient keys/NODE_OPTIONS/BROWSER overrides. Cancel/terminal/fault/stop/Drop revoke it, including delayed URLs.
- Callback: 1–16384 printable non-whitespace ASCII bytes, no trimming/reveal/readback; consuming owner, fenced editor/review/operation and explicit confirmation.
- Owned Rust buffers are wiped; browser/widget/clipboard/allocator/Node/kernel/swap copies are not. Masking is not memory protection; trusted provider paths are not confined.

## Cancellation and uncertain outcomes

- **Cancel is not sign-out, deletion or rollback:** a late provider credential write can follow cancellation.
- Failed/cancelled/timed-out login blocks retry until Backend restart. Unknown auth outcomes also block unrelated credential/settings edits. Status cannot prove a particular attempt committed.
- Keep Codex open while connecting. Leaving clears drafts and remaining automatic steps, not admitted work. Re-entry requires explicit reconnect/status/cancel.
- Shutdown stops the owned Host; the external browser remains user-owned.

## Model-route activation

- Exact provider entry/path, grant, writable settings and expected revision are required.
- Connect consents to missing-route activation; Advanced retains Review/Confirm. Revision-CAS sets only missing values to `{}` and waits for reconciliation.
- Unknown activation requires fresh status before review. Configuration/revision is not entitlement or generation.
- Active/uncertain login excludes key/plugin/export writes; keyless smoke forbids auth. Generic plugin security opt-outs remain intact.

## Qualification

**Local evidence, ignored on GitHub:** [one-click record](<../app/evidence/codex-one-click-r8si3gst/qualification.json>), release `a3ed4509b847ccda203e98cd84433cc352ad0a40dd7b14257c2c32577adbaa8c`.

- All-target check; 259 app/262 component tests; 30 state-machine and five App/queue tests; eight PUBLIC component runs at app scales 1/1.25; real keyless startup/fold/clean stop.
- Inspected Connect/startup frames; mocked receipts dispatch no effects. Early-terminal race covered in both orders.
- **Not qualified:** real OAuth/token persistence/refresh, route persistence, entitlement/GPT generation, physical input/IME/accessibility. Tests open no browser and copy no existing credentials.
