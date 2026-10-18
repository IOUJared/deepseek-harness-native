# Native Codex runtime configuration

Open **Settings → Codex → Models & runtime** or **Images**. (The Images tab configures image
*input* limits only; this backend offers no image generation or sketch tools.) The two tabs share one eight-field draft. **Refresh runtime** reads the exact isolated backend; **Save runtime** submits all eight fields against its current revision. Tab navigation, typing and **Reset to inherited defaults** do not save or send prompts.

| Fields | Accepted values |
| --- | --- |
| Transport | Default/inherit, `sse`, `websocket`, `websocket-cached`, `auto` |
| Cache retention | Default/inherit, `none`, `short`, `long` |
| Request / WebSocket-connect timeout | Blank to inherit; decimal natural number including explicit zero |
| Stream-idle timeout | Blank to inherit; positive decimal milliseconds, fractions retained, maximum 2147483647 |
| Image pixel / per-image byte / total image byte budgets | Blank to inherit; positive decimal integer |

Integer fields are bounded by JavaScript's safe-integer maximum. Signs, whitespace, exponents, non-finite values and implicit coercion are rejected by the native draft. Image fields govern **model input**, not image generation/sketch. A valid setting is not provider execution evidence or an optimal default.

## Save and inheritance

- The configured fixed `openai-codex` route and writable Settings service must exist. Empty keyless profiles do not acquire a route or credential by opening these tabs.
- Native ownership and actual backend revision-CAS/ACK are required; edits are not saved until the owned receipt. Failure/unknown writes stay visible; no automatic replay. Explicit Refresh reconciles uncertainty.
- SDK Settings revisions are **process-local**, not durable restart identifiers. ACK may keep the revision for a no-op or advance it once. Restart verification compares effective/base/user values and the private document—not an old process's revision number.
- Reset changes the local draft, then requires explicit Save. The exact SDK may normalize unset fields to inherited raw values or remove a whole configuration equal to its inherited configuration. Consequently, generic ACK acceptance is base/effective-value-aware; blanket null user-override equality would reject valid SDK behavior.
- Saved metadata distinguishes effective values, inherited base and user overrides. General explicit-pin preservation when an entire configuration equals raw inherited configuration remains governed by SDK normalization; do not claim stronger durable pin identity from effective values alone.

## Native composition qualification

[Round-37 evidence](<../app/evidence/runtime-composed-t2qwrtmw/qualification.json>): **760 default-feature example tests + one ignored**, four new focused checks and release build. The previous 219 production/fixture sources are unchanged; one private example is extended and one private Runtime scenario added, with four frozen Python helpers. No SDK/DTO/schema/dependency/profile/API change or new main archive.

Three actual native App/worker/Core/full-alpha processes use one new keyless profile with a **PUBLIC route-only label**, not a credential/grant. Native Save persists `websocket-cached`, `long`, two zero timeouts, fractional `12.5` idle milliseconds and three image-input budgets; matching actual ACK and explicit Refresh agree. The first restart reads all eight pins, then native Reset + explicit Save restores inheritance. The second restart reads inherited values without reseeding/saving. The SDK's private mode-0600 YAML contains exactly the requested eight pins, then removes them in this closed raw-base-empty route; label/native flags/unrelated fields remain unchanged. Revisions observed were **2→3**, **0→1**, then **0**—not durable IDs.

The parent inspected Models and Images frames after each phase: saved values, blank inherited drafts, effective defaults, limits and actions are readable. Both existing usage opt-out processes also pass unchanged semantics. All **five** native/full-alpha trees stopped gracefully, with no observed survivors; **eight** own Wayland/wgpu frames were inspected. This is scripted native configuration composition, not physical input, provider/network/transport/cache/timer/image-processing execution, parallel SDK writers, crash/power-loss or performance qualification.

The [existing composition adapter](<../scripts/qualify-composed-usage.py>) defaults to `--scope usage`; `--scope runtime` selects the three-process case. Build the [shared native example](<../app/examples/usage_composed_smoke.rs>) as described in the [usage composition guide](<NATIVE-CODEX-USAGE-COMPOSITION.md>), use a new output beneath that compiled manifest's evidence directory, and pass `--scope runtime`. Runtime document verification requires existing **PyYAML** (qualified here with **6.0.3**); no app/backend dependency was added. The initial compile failure and corrections remain in evidence. Private unsent catalog suppression/pre-Ready Codex selection are fixture-only; the current 6ffa development archive and installed state are unchanged.

The [usage opt-out policy](<NATIVE-CODEX-USAGE-UPGRADE.md>) and [official quota/auth interface limits](<NATIVE-CODEX-INTERFACE-AUDIT.md>) are separate. Runtime configuration does not grant credentials, observe quota, execute alerts/reset retry or generate images. Installed runtime/profiles/accounts remain untouched by disposable PUBLIC qualification.
