# Codex subscription interfaces — October 10, 2026

**Read-only static audit, not live entitlement or integration qualification.** No CLI execution/install, credential/profile access, OAuth/account/quota endpoint or provider requests. Context7 quota was exhausted; official static docs/tagged upstream source and exact local alpha source were reviewed.

| Surface | Established capability | Native integration limit |
|---|---|---|
| Exact alpha `0.2.1-alpha.1` [CodexAccounts](<../../deepseek-harness-linux/packages/llm/llm-pi-ai/src/codex-accounts.ts>) | Describe/Add-login/cancel/select/local sign-out/all | No quota/reset or public token-export method. Selected opaque UUID/epoch, email/label/plan and catalog are not issuer identity or entitlement. |
| Official app-server [account and quota API](https://learn.chatgpt.com/docs/app-server.md) | `account/read`, `account/rateLimits/read`, `account/rateLimits/updated`; multiple buckets and nullable duration/reset | Pinned [0.160.1 tests](https://github.com/openai/codex/blob/rust-v0.160.1/codex-rs/app-server/tests/suite/v2/rate_limits.rs) require ChatGPT auth, reject absent/API-key-only auth. Its separately managed OAuth/store/refresh owner is not automatically the selected Harness SDK account. Feed updates are not native OS alerts. |
| Existing SDK → app-server tokens | Current docs describe experimental host-owned auth; [pinned account protocol](https://github.com/openai/codex/blob/rust-v0.160.1/codex-rs/app-server-protocol/src/protocol/v2/account.rs) labels `chatgptAuthTokens` **internal/unstable—do not use** | No verified stable supported bridge. Do not enable/export/reuse tokens. Beta preference consent alone cannot establish support, identity or scopes. |
| Earned reset credits | Current docs describe `account/rateLimitResetCredit/consume`, caller idempotency key, reset/alreadyRedeemed/nothingToReset/noCredit, then fresh limits read | An account/credit **mutation**, not automatic quota-reset retry or proof of restored permission. No native method. Minimum/introduction version was not established; do not assume pin compatibility. |
| [Sign in with ChatGPT alternative](https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server.md) | App-owned OAuth renewal + Responses provider | Different audience/scopes/provider lifecycle, not a drop-in SDK migration or quota mapping. Official app-server docs restrict its auth to local/open-source use, not commercial/hosted; SIWC guidance does not authorize changing this app's login silently. |

[Account-only client](<../core/runtime/codex-app-server.mjs>) and [quota projector](<../core/runtime/codex-quota.mjs>) remain inert/unwired. The read requests exclude reset-credit details and disable reserve support; projection strips credit/reset-credit data. Unknown permission remains unknown; missing issuer account ID binds only the owned transport, not independent issuer verification. Rust never reads grants/authfiles/keyrings.

## Before any production observer

- Verify a supported auth route, exact executable/protocol/scopes and provider ownership, with explicit approval—not ambient CLI authentication or private SDK grant export.
- Establish selected SDK account/issuer + workspace/selection epoch and immutable request/model-to-quota-bucket mapping. Retire switches/logout/disposal/late refresh; preserve secret handling, bounded transport, freshness and null semantics.
- Qualify real provider-owner/Loader/native bridge with owned PUBLIC fixtures before separately authorized live-account tests.
- Keep existing SDK login/model behavior. No silent alternate-auth adoption, account/credit/email mutation or automatic uncertain replay.

[Reset retry policy](<NATIVE-CODEX-USAGE-POLICY.md>) additionally requires an authoritative original **pre-execution quota denial**, no partial output/uncertainty, immutable request/account/model, fresh post-reset allow observation and atomic bounded execution budget. Time/percent/catalog cannot grant permission. No observer, native notification or reset-retry executor is connected now.

The older [Settings table](<NATIVE-CODEX-SETTINGS.md>) was corrected: managed account vault/mount/Add/select/local sign-out are implemented; local choices have durable storage, but quota effects remain inactive. Local logout is not issuer revocation. These limits do not imply that official quota interfaces exist nowhere.
