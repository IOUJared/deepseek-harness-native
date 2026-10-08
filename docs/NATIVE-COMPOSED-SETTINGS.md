# Composed native settings qualification

## Deliberately narrow developer test

The [composed fixture](<../app/examples/settings_composed_smoke.rs>) compiles the actual native UI source/handlers/view, settings, worker, theme and renderer helpers, with an example-only lexical adapter exposing **numeric tickets only**. It starts a real isolated same-version alpha backend through the actual worker, not a mocked Core.

CLI accepts only an owned output directory and application scale (1 or 1.25). There is no key/runtime/profile/path override argument. The runtime is the explicit isolated fork; every home/workspace/config/cache/data/state directory must be new/empty/private and canonical beneath native app evidence. The [runner](<../scripts/qualify-composed-settings.py>) constructs a cleared owned HOME/XDG/Wayland environment. Installed runtime/data, real credentials and desktop settings are not targets.

The only scripted draft is a fixed public dummy declared in source. **All physical widget business/input messages are dropped**; only worker events, own window metadata and close requests enter the wrapper. A fake/public test fixture is not an invitation to paste a real credential. Normal production keyless smoke remains read-only and independently forbids writes/replies in UI and worker.

## Parent-run results

[Parent evidence](<../app/evidence/parent-composed-settings-qualification.json>) records final fixture SHA `85bed3c8f39ac0fd7b4921f83bd9109cbac704cd8a052bd918ee59220a9aa17a`. [Scale 1](<../app/evidence/composed-settings-03-final/result.json>) and [application scale 1.25](<../app/evidence/composed-settings-04-final-scale-125/result.json>) pass actual native Wayland, matching real persistence acknowledgment, independent private PUBLIC-value file check, metadata refresh and graceful zero-exit/no-observed-survivor cleanup. No external cleanup was needed. Parent inspected both own renderer images: masked fixed dummy, explicit warning/confirm/cancel and readable modern Rosé Pine sheet without observed clipping.

The example suite passes **86 tests: 83 shared app/component tests plus three fixture guard tests**; all qualification runners pass 15 tests, including 11 composed-runner regressions. The latter includes a real headless Linux Python-only root-exit/TERM-ignoring-descendant pidfd test. Prelaunch checks reject unrelated output and a key-value CLI argument before starting a Host.

Independent UI restyling changed source/release hashes during this round; final evidence is pinned to the named fixture, not a guessed production-app hash. Original production-app process-tree performance evidence is not replaced by this brief persistence fixture.

## Actual composed path

1. Actual worker starts Core and authenticated transport, then actual App handles Ready.
2. Script calls actual App Settings/Open, issuing its real metadata command. Expected epoch/panel/operation ticket fences the response; empty signed-out writable default provider is required.
3. Script sends a one-use PUBLIC edit and Review through actual App handlers. The real secure settings view is captured from its own Iced renderer before submission.
4. Script issues Confirm through the same handler; the app consumes the editor into move-only Core `SecretApiKey`, sends the real worker command, and invalidates old editor messages.
5. Worker refreshes writable metadata and calls actual Core persistence. **Only matching Confirmed ACK** proceeds, not metadata presence or a simulated receipt.
6. Actual App explicit Refresh issues a new real worker metadata read. The fixture separately checks key presence/sign-out state and the default owned 0600 local file for the fixed PUBLIC bytes without printing them. File inspection is bounded and refuses symlink leaves.
7. Actual App ConfirmClose requests priority shutdown; actual worker/Core stop is observed, owner is retained until Iced exits and joined/dropped afterwards. Public dummy test home/artifacts remain; close is not deletion or rollback.

App report and PNG use private create-new files. Reports contain fixed codes/booleans/nonsecret tickets/stop metadata, not raw inputs/errors/credentials. There is no validation/model/catalog/account sign-in call in the examined path; scope flags are source assertions, not an independent network trace.

## Default-provider side-effect source audit

The [native save adapter](<../core/runtime/fork-supervisor.mjs#L176-L183>) validates/normalizes then writes the [configured API-key reference](<../../deepseek-harness-linux/packages/llm/llm-deepseek-api-key/src/config.ts#L11-L38>). The [local provider](<../../deepseek-harness-linux/packages/credentials/credentials-local/src/index.ts#L733-L765>) atomically commits a 0600 file; [fanout](<../../deepseek-harness-linux/packages/credentials/credentials/src/index.ts#L258-L276>) emits reference-updated, not account-grant record-updated. [Account listeners](<../../deepseek-harness-linux/packages/credentials/deepseek-account-platform/src/index.ts#L147-L189>) observe only the latter; [platform-session lookup](<../../deepseek-harness-linux/packages/credentials/deepseek-account-platform/src/index.ts#L359-L382>) and [watch](<../../deepseek-harness-linux/packages/credentials/deepseek-account-platform/src/index.ts#L484-L498>) read cached local state. API-key storage does not create a sign-in grant.

Browser [model auto-refresh listeners](<../../deepseek-harness-linux/packages/client/ui-model-selection/src/client/service.ts#L46-L57>) are not instantiated: this fixture contains no browser/webview. Configured default [API-key model discovery](<../../deepseek-harness-linux/packages/llm/llm-deepseek-api-key/src/index.ts#L39-L46>) is static, with credential auth resolution deferred until inference. Under this **default/new-private-profile** composition, source review found no automatic external validation/account/credit/catalog request triggered by storage or native subscription. This is not a packet trace or guarantee for custom providers/profiles or separately attached browser consumers.

## Verification and limits

Later [sticky uncertainty policy](<NATIVE-SAVE-UNCERTAINTY.md>) keeps a missing-acknowledgment receipt across UI lifecycles and blocks another native API-key save until a fresh app lifetime; it does not change the Node mutation lane, Core deadline, or cancel an admitted write. [The new success regression](<../app/evidence/save-uncertainty-success-01/result.json>) still passes this same real App/worker/Core/default-provider path at scale 1.25 with a matching ACK, independent PUBLIC file/metadata verification and clean two-process exit. Earlier images/hashes remain historical; unknown-state timeout/paint is not qualified by the successful-save path.

Qualification reports and parent paint findings are separate. A passing result must include native own-PID/app-ID Wayland evidence, matching live persistence ACK, independent private-file dummy verification, refreshed metadata and graceful zero-exit/no-observed-survivor cleanup. Application scale 1.25 is not genuine compositor fractional scaling.

The 25-second in-app deadline **requests** owned shutdown; it is not an immediate kill or write rollback. The runner uses a 40-second polling budget plus up to two seconds TERM/two seconds KILL polling and the existing own-window helper's query timeout; this is not an exact whole-run wall-clock or arbitrary opaque helper/kernel bound. Cleanup uses retained pidfd ownership for descendants, never numeric PID/group guesses. Root pin failure is handled through the directly owned unreaped child resource. All pinned survivors are waited/escalated even if the GUI root already exited; unknown containment is not advertised as success.

The named parent-run evidence proves scripted **UI → worker → Core → default trusted provider** integration. It does not prove physical keyboard/pointer/IME input, clipboard erasure, account validity, a real model call, arbitrary-profile path confinement, live Host tool approvals or desktop parity. Wiping guarantees remain limited to owned app/Core buffers, not widget/kernel/Node/swap copies.

The [session-management contract audit](<NATIVE-SESSION-MANAGEMENT.md>) prioritizes reversible pin/unpin, then archive/restore without forced stop. Those native mutations, timed/continued claims, lazy guests, rich files, accessibility and install/update packaging remain implementation work.
