# Native Codex settings

## Current milestone

Settings uses a compact category sidebar and Codex subscription tabs: Account, Models & runtime, Images, Maintenance. The centered Rosé Pine modal retains its inactive, dimmed background. Existing API-key review, account connection, callback masking, cancellation and model-route confirmation remain guarded. Tab navigation is local and works offline; it does not cancel admitted backend work. Switching tabs clears callback drafts/reviews and rotates panel ownership. A Runtime receipt arriving after a tab change cannot claim success: the read becomes unavailable or write outcome unknown, requiring explicit Refresh.

[Managed accounts](<NATIVE-MANAGED-CODEX.md>), local sign-out and [runtime controls](<NATIVE-CODEX-RUNTIME.md>) are implemented; **quota, usage alerts and reset retry are not connected**. Stored credentials/configured routes are not verified issuer identity, subscription entitlement or observed quota. No fabricated percentage, email, plan, reset or preference success. Image generation/sketch is unavailable through this backend; image input depends on the selected model.

## Backend implementation status

| Component | Status |
| --- | --- |
| Same-version SDK account vault/service + typed native bridge | Implemented/qualified with PUBLIC fixtures; exclusive durable restore, multiple account Add/select/local sign-out/all, epoch retirement; explicit legacy mode remains supported |
| Native local account/Maintenance facade | [Actual App/worker/Core/full-alpha PUBLIC composition](<NATIVE-MANAGED-CODEX-COMPOSITION.md>): select/switch, local selected logout without replacement, all, two restarts and Maintenance status; [empty-list and already-selected Switch owner/view guards](<NATIVE-MANAGED-CODEX-UPGRADE.md>), with unchanged epoch after rejected reselect/readback. Native target/warning/Confirm/Cancel consent screens inspected; local pre-confirm cancellation/readback qualified. No real sign-in/Add/provider/physical-input claim |
| Native model/runtime/image configuration | Implemented revision/override CAS and uncertainty guards; generation/sketch unavailable |
| Saved usage choices | Revisioned readback + explicit opt-out CAS/ACK. Enabling/threshold edits disabled in native view and Settings owner; no inert working controls. [Compatibility note](<NATIVE-CODEX-USAGE-UPGRADE.md>) |
| Account-only app-server + quota projector | Inert tested foundation, exact 0.160.1; **unwired**, no production account/quota requests. Internal/experimental token bridge is not a supported auth integration |
| Same-selected-account quota observer | Unavailable in exact alpha public CodexAccounts; official app-server reads exist separately but identity/scopes/provider mapping are not established |
| Composer quota, native alerts, reset retry | Unavailable: no verified observer/executors. Clock/percentage/catalog does not authorize retry |

A future observer must use the **same immutable provider-owned account binding** as native model requests. Rust never reads SDK grants/tokens/authfiles. Current native account metadata comes from the alpha SDK; separate app-server `account/read`/`account/rateLimits/read` cannot be substituted without supported auth and exact mapping. Multiple buckets and nullable duration/reset/permission remain distinct. A reset timestamp or percentage never proves restored permission.

## Experimental upstream interface

[Official Codex documentation](https://learn.chatgpt.com/docs/app-server.md) describes host-owned external tokens as experimental and requires `initialize.capabilities.experimentalApi=true`. The installed 0.160.1 generated schema still labels that variant internal/unstable. This disagreement is retained as a compatibility limit, not a stable-support claim.

The private client requires an explicit `enableExperimentalChatgptTokens` boolean. Disabled mode rejects external-token login and refresh callbacks before admission. Enabled mode admits only the documented, pinned token variant; unrelated experimental methods remain unavailable. Production must keep the flag false. A visible Beta toggle alone is not evidence of a supported auth bridge or exact selected-account mapping; see the [official-interface audit](<NATIVE-CODEX-INTERFACE-AUDIT.md>). Observer credentials use explicitly selected ephemeral CLI storage, not global keyrings or another account's files. The upstream refresh bridge has a 10-second deadline; the client requires its owner callback deadline below 10 seconds and discards late completion without claiming refresh success.

## Remaining qualification

[Managed-account Loader/vault tests](<NATIVE-MANAGED-CODEX.md>) qualify existing PUBLIC mounting/switch/logout/cancellation/restore behavior. Future observer/executor integration needs its own owned fake-observer tests for issuer mismatch, stale usage, late refresh and quota-reset retry. Live sign-in/sign-out/usage/providers and physical input need separate consent; none was performed here. Installed profiles/accounts/desktop remain unchanged; development archives are separate files, not installations.
