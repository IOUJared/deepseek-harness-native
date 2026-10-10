# Codex usage preferences and policy

Native Settings → Codex → Account reads saved choices and persists explicit **opt-outs** in the isolated profile. Quota display/observer, alerts and reset-retry effects are unavailable; enabling/threshold editing is disabled, not an inert working preference. See the [native compatibility change](<NATIVE-CODEX-USAGE-UPGRADE.md>). Preference commands do not read credentials, fetch quota, start a CLI or send model requests.

## Qualified scope

- Storage: 96 focused Node checks on tmpfs and the workspace ext-family filesystem, including 17 real-flock storage cases. Checks cover restart, contention, process exit and injected write/fsync/rename/close failures; not power loss or other platforms.
- Integration: 353 Node tests; Core 74/88 unit tests plus seven doctests per feature configuration; App 676/683 tests with default/PUBLIC-fixture features, with one unrelated ignored test per run. Serialized offline/locked Cargo used two jobs; final source hashes were unchanged.
- Real Loader: the exact `0.2.1-alpha.1` full base+web profile saved preferences, rejected a stale revision and restored them after restart. Owned PUBLIC grants only; zero external fetches, generation requests or OAuth logins.
- Rust pipe: the existing Core smoke now has explicit `initial`, `restored` and `corrupt` preference-only stages. Against the real alpha profile, typed `Backend.request()` saved fractional preferences, rejected a stale save, restored the exact committed snapshot after restart and rejected repeated corrupt loads without altering the corrupt document. All three runs stopped gracefully with no observed descendants remaining; two example unit tests passed. No account, OAuth or model commands were issued by these stages.
- Earlier account-flow failures, a concurrent source change, a search-style compile error and a stale UI-copy assertion remain in private evidence. OAuth status receipts no longer schedule preference work; only a standalone status action can request that bounded follow-up.

The earlier human-approved frozen snapshot additionally passes 119 Codex tests per App feature configuration, builds the release binary and renders both Account and Usage preferences through native Wayland/wgpu with zero Host/worker commands. The parent inspected both screenshots; copied source hashes remained unchanged. The retained binary is 16,650,768 bytes; it is a development snapshot, not a broadly qualified release. Its broader App suite retains one failing concurrent chat-search/draft-recovery test; six initial snapshot failures came from missing fixture directories/dependencies and were resolved without Rust source edits. Later live-tree render/build attempts recorded source changes, so they are not stable-source application qualification.

These checks do not establish a clean whole-application release, physical input, live-account quota, notifications, retry execution or cross-platform parity.

## Native opt-out composition

[Actual native composition](<NATIVE-CODEX-USAGE-COMPOSITION.md>) now verifies Settings → real worker → Core private pipe → production usage store in the unchanged full-alpha profile: real revision-5 opt-out ACK, explicit readback and an independent process restart, two inspected Wayland/wgpu frames, clean stops and released kernel locks. **756 default-feature example tests + one ignored**, three new fixture regressions and release build passed. This is local preference persistence—not SDK Settings mutation, physical input or live quota. The private fixture suppresses startup catalog I/O through a reserved **unsent** flight; normal startup is unchanged. Prior 219 native sources are unchanged; one private example and four frozen supervision/metrics sources only. No new capability, API/schema/dependency/profile/login/model behavior change.

## Current native surface

[Opt-out qualification](<../app/evidence/usage-surface-n_7wv6j9/qualification.json>): **753 default/760 PUBLIC App tests; 762 PUBLIC layout-example tests + one ignored each**, all-target/release checks, five inspected own native frames and two extracted unchanged full-alpha PUBLIC launches. One policy in view and Settings owner rejects enabling/threshold edits and unchanged/increasing saves. Readback and explicit Off reductions retain actual App queue rejection/hiding/ACK/uncertainty coverage. A matching owned fresh READ rebases the read-only threshold after a changed baseline without losing Off drafts; wrong/stale/retired/failed/version-invalid/Save/duplicate/unrelated callbacks cannot. Six new regressions; two initial obsolete enabling-save consumer failures retained, consumers migrated—not production weakened. Prior generic persistence/Loader/pipe qualification stays separate; this surface-only snapshot does not compose native preference writes or qualify SDK Settings mutation, physical input or real-account quota. Frozen prior snapshot + seven owners only.

## Previous status visibility

[Usage-status snapshot](<../app/evidence/usage-footer-ci1_cl_2/qualification.json>): 747 default/754 PUBLIC app tests and 756 PUBLIC layout-example tests plus one ignored each; release/all-target checks and five inspected own native PUBLIC frames. Unknown/conflict/unavailable warnings survive invalid edits and remain outside scrolling across Settings categories/tabs; valid input clears only local validation. A lost read is unavailable; a lost write is unknown. Refresh must reconcile before another uncertain Save. Local preferences are explicitly inactive, not pending sign-in or working quota/alerts/retry. No provider/account commands or physical input were tested; frozen source only, broader mutable tree excluded.

[Official app-server interfaces](<NATIVE-CODEX-INTERFACE-AUDIT.md>) exist separately, but no supported exact selected-SDK-account observer/auth bridge is established. Experimental Beta consent is not permission to export SDK grants or use the upstream internal/unstable token variant.

## Preferences

| Field | Values | Default |
| --- | --- | --- |
| `composerDisplay` | `off`, `percent`, `progressBar`, `runway` | `off` |
| `usageAlert` | `off`, `percent20`, `earlyWarning`, `custom` | `off` |
| `customRemainingPercent` | Finite number, inclusive 0–100 | 20 |
| `retryAfterReset` | Boolean | false |
| `experimentalQuota` | Saved Boolean; new native opt-in unavailable | false |

`defaultCodexUsagePreferences()` and `validateCodexUsagePreferences()` return detached immutable values. Unknown fields, invalid modes and implicit coercion are rejected.

`createCodexUsagePreferencesStore({ readDocument, commitDocument, maxPendingUpdates, maxDocumentBytes })` requires explicit limits and a durable storage provider. The document is exactly `{ version: 1, revision, preferences }`. Reads accept that document, complete JSON/UTF-8 bytes, or `null` for an unwritten store. Corruption, read failure and unknown versions never become defaults.

`load()` and `update(expectedRevision, preferences)` return `{ version, revision, preferences, persisted }`. Unwritten defaults have revision 0 and `persisted: false`. An update rereads current state and invokes `commitDocument({ expectedRevision, document })`; the provider must perform an atomic revision-checked durable commit. Exact receipts are `{ status: 'committed' }`, `conflict`, or `indeterminate`. Only committed returns success. A failed/unknown write is indeterminate; recovery uses an explicit fresh read, not automatic replay.

The consumer owns effective consent, not merely the last saved checkbox. New opt-in cannot authorize observation before its commit ACK. Local opt-out immediately stops observation, cached presentation, alerts and retry admission—even if saving fails. Late replies recheck consent and ownership before publication. Persistence uncertainty must remain visible and must not restore permission from stale saved state.

Admission counts queued and in-flight updates. Limits have security ceilings of 128 updates, 128 total read/write operations, and 65,536 complete document bytes. `dispose()` closes admission, rejects queued work, prevents a pending read from starting a new write, and waits for already-invoked storage work to actually settle. A hung provider leaves disposal pending rather than claiming cleanup.

## Native integration

`Command::CodexUsageLoad` / `CodexUsageSave` use two fixed private-pipe commands and [typed preferences](<../core/src/codex_usage.rs>). The profile lazily opens `codex-usage/usage.v1.json` with the exact pinned flock dependency, four pending updates and a 4,096-byte document limit. Initialization failure remains sticky; Refresh reconciles readable storage, but a failed opening or poisoned owner may require restarting the app.

The native editor captures revision, preferences and edit generation. Only the exact persisted revision+1 ACK confirms Save. Conflict or uncertainty requires explicit Refresh; drafts and local opt-out survive hiding, reopening and late replies. Preference work cannot continue login/model activation, and reserved OAuth cancellation does not discard an admitted preference write.

## Remaining supported controls

The alpha [provider profile](<../../deepseek-harness-linux/packages/llm/llm-pi-ai/lib/types/config.d.ts#L54-L145>) supports reasoning, cache retention, transport/timeouts, provider retry policy and image-input budgets. These are not quota-reset retry or image generation. Model/effort/speed/output selection already belongs to the native composer; catalog presence does not verify entitlement.

The private [Codex bridge](<../core/runtime/codex.mjs>) now supports fixed `codex-runtime-read` and `codex-runtime-save` commands for transport, cache retention, request/WebSocket/stream-idle timeouts and three image-input budgets. It derives the route from `llm.listConfigurableProviders()`, reads `settings.describe({ redactSecrets: true })` and applies revisioned [path mutations](<../../deepseek-harness-linux/packages/settings/settings/lib/types/index.d.ts#L109-L114>). All eight input fields are mandatory; null means unset/inherit. Namespace, path, credential, header, endpoint and arbitrary provider fields are forbidden. It never replaces a redacted document. Configuration saves and model activation share the account mutation lane; shutdown drains admitted writes without acknowledging a lost owner or replaying uncertainty.

Public schema defaults are materialized for inheritance even when the lower-layer route is absent. Two retained reset qualification failures exposed that missing-route base values are not the same as a configured route's schema defaults. The [projection helpers](<../core/runtime/codex-runtime-values.mjs>) now read defaults from the exact public Config schema instead of hardcoding image/time limits. Natural request/WebSocket timeouts retain zero, stream idle retains positive fractional milliseconds up to the alpha timer limit, and image budgets require positive safe integers. The seven-field snapshot is `{ version: 1, revision, configured, writable, values, baseValues, overrides }`. Effective `values` and inherited/schema `baseValues` do not identify user overrides; `overrides` is projected separately from public redacted `SettingsDescriptor.user`. Native drafts use actual overrides, preserving explicit pins equal to current defaults. Public unset operations restore effective inherited values; the SDK may normalize their persisted representation. None of these fields establishes model entitlement.

Executed [PUBLIC Loader evidence](<../core/evidence/codex-runtime-checked-rhzny6py/runner-qualification.json>): **385 Node tests passed**, initial save/read/stale-CAS rejection, process-restart restoration, inherited reset and unrelated display-label preservation passed with no source drift. External fetches, generation and OAuth calls were zero. No real credentials or user profile were touched. Earlier reset failures remain in [first qualification](<../core/evidence/codex-runtime-loader-gfc_j_xc/qualification-restored.json>) and [diagnostic qualification](<../core/evidence/codex-runtime-reset-diagnostic-lxlf8n6d/qualification-restored.json>).

The [typed Core consumer](<../core/src/codex_runtime.rs>) validates exact fields, closed enums and bounded private numeric types. [Native Models/Images controls](<../app/src/codex_runtime_view.rs>) share one eight-field draft: Refresh/Save are explicit, blank means inherited, and Reset edits the draft only. Owned acknowledgments require the same inherited values, exact effective requested values and revision unchanged or +1; no-op public Settings writes need not increment revision. Panel/epoch changes fence late receipts, drafts/pending writes survive hiding, and reserved OAuth cancellation cannot discard a save. Conflict/unknown/unavailable warnings survive Reset until an owned Refresh reconciles them; critical status also stays outside scrolling content in the fixed modal footer. Public Settings revisions are process-local CAS observations, not the durable preference-document counter. Generic plugin forms remain blocked (`auto: false`).

[Override qualification](<../core/evidence/codex-runtime-overrides-pjli3kzx/runner-qualification.json>) passes 386 Node tests and real PUBLIC Loader save/stale/restart/reset checks, including an explicit default pin preserved while another field changes. [Typed Core checks](<../core/evidence/codex-runtime-typed-uouxhk6w/qualification.json>) pass 86/100 unit tests plus ten doctests per configuration and four smoke-example tests. [Actual Rust pipe](<../core/evidence/codex-runtime-rust-pipe-2qtvw8uo/qualification.json>) saves all eight fields, rejects stale writes, restores them in a new process and resets to authoritative defaults. Both runs stop gracefully with no observed descendants or dropped events; the PUBLIC account fixture is unchanged. No account, onboarding, workspace or model commands are issued by the runtime-only stages.

[Latest frozen native qualification](<../app/evidence/codex-runtime-frozen-5619buuu/footer-qualification.json>) passes **724/731 App tests** with default/PUBLIC features, one unrelated ignored test each, and 733 layout-example tests plus one ignored case. Models, Images and a scrolled unknown-save warning were rendered through native Wayland/wgpu with zero Host/worker commands; the parent inspected all three screenshots. The fixed warning remains visible while Save is fenced. The release development snapshot is 16,772,760 bytes, SHA-256 `2e17e56acbba41c8cf9a0cf7f476ebbd4e539ea6ee052c10b8bd405c79ca0992`; copied source hashes stayed unchanged. Earlier missing-runtime-module and macro/exhaustive-effect compile failures remain in the same evidence directory; the earlier broader chat-recovery failure remains retained separately. This qualifies the frozen snapshot, not concurrent live sources, physical input, provider execution or platform parity.

The current Harness interface does not expose Codex image generation/sketch or subscription quota/reset observation. An SDK's OpenRouter image API is not ChatGPT subscription support.

## Private disk provider

[`openCodexUsageStorage()`](<../core/runtime/codex-usage-storage.mjs>) supplies the preferences store with `{ directory, lockExclusive, maxPendingUpdates, maxDocumentBytes }`. The directory is explicit, canonical, current-user-owned and private; its parent must already exist. Only the final directory is created. The provider owns separate `usage.v1.json` and stable `owner.lock` files, never an account grant vault.

`lockExclusive(fd)` is a required trusted, nonblocking POSIX flock primitive. The verified dependency is `@deepseek-ai/node-addon-system/flock` 0.1.2, resolved by the owning runtime. Import alone does not establish addon availability. Acquisition failure is loud; the descriptor stays open through actual acquisition settlement and until storage work drains. Linux/Darwin are supported by the API; other platforms fail closed, and untested architectures/filesystems are not qualified.

Writes use a unique exclusive private temporary file, complete writes, file fsync, same-directory rename and directory fsync before success. Loads validate and sync the actual observed file before acknowledging persistence. Only the provider's own unchanged temporary inode may be cleaned up; the lock and committed destination are never removed. Data-write uncertainty requires explicit load reconciliation. Parent/root/lock ownership loss permanently disables the owner.

Descriptor-close failures permanently disable the owner and remain visible through disposal; neither a failed close nor an unknown write permits silent recovery or replay.

The lifetime advisory lock serializes cooperating writers. Content/inode comparisons detect observed external edits; they do not make check-plus-rename atomic against noncooperating or adversarial same-user writers. Within one owner, a previously observed document or possibly invoked rename cannot become unwritten defaults after deletion. Across a fresh restart, missing JSON cannot distinguish first use from manual deletion without durable history; this provider does not invent that history. No power-loss durability, hostile namespace containment or complete platform-matrix claim follows from temporary-file tests.

## Display and alerts

Policies receive sanitized quota observations, the current opaque account/generation/time/model alias, preferences and an explicit spec. The spec requires `maxSnapshotAgeSeconds`, `maxHistorySamples`, `minRateIntervalSeconds`, `maxRateIntervalSeconds`, `maxRetryWaitSeconds`, `maxRetryAttempts`, and `earlyWarningSeconds`.

`projectCodexUsageDisplay()` selects the most constrained relevant actual window. Raw usage above 100% remains raw; only a progress value is clamped. Missing, stale, future or differently bound observations stay unknown. Disabled Beta consent hides cached usage. Duration and reset remain nullable; primary/secondary do not imply fixed five-hour/week windows.

Runway is explicitly a Beta estimate from bounded chronological observations of the same account, generation, bucket, window and reset. Flat, regressed, ambiguous or insufficient history yields unknown. An estimate is neither permission, subscription entitlement nor a reset timestamp.

`decideCodexUsageAlert()` emits once when remaining usage crosses its threshold, or when an initially observed/newly armed window is already below it. Recovery, settings changes, account changes and reset changes rearm appropriately. Early warning requires an actual runway estimate. Revoked consent clears alert state; unknown observations never emit notifications. The caller persists the returned marker and owns notification delivery.

## Reset retry

`decideCodexResetRetry()` evaluates an explicit user-enabled intent; it never executes a retry. The intent captures original request ID, current request ID, immutable account/generation/model alias, whether failure preceded all output, cancellation, attempt count, trusted reset and an absolute request deadline.

Only an authoritatively known original `failureKind: 'quotaDenied'` before execution is eligible. `other` and `indeterminate` are never retried; zero visible output or error prose does not prove a known, uncharged outcome. Cancel, request/model/account changes, partial output, exhausted attempts, expired deadline and out-of-bounds reset deny retry. At/past reset, a fresh observation from after that reset must explicitly allow ordinary usage and show capacity in all relevant windows. Unknown permission requires observation, not retry. Time passage and percentage alone never grant permission.

The executor must consume an attempt atomically and recheck the original immutable request/model/account binding immediately before sending. A pure policy cannot enforce exactly-once execution. Model aliases must come from provider-owned quota mapping, not a UI label or an inferred catalog entitlement.
