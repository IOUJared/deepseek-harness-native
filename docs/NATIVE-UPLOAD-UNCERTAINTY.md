# Upload commit uncertainty and receipt scope

Actual isolated alpha `0.2.1-alpha.1` full public profile/default attachment provider; extends [bounded uploads](<NATIVE-FILE-UPLOAD.md>), not native intake UI.

## Qualified behavior

Ignored [Host report](<../app/evidence/upload-uncertainty-02/result.json>) / [storage report](<../app/evidence/upload-uncertainty-02/uncertainty-qualification.json>) are **local-only**.

| Real barrier | Observed result |
|---|---|
| Hold return after upload/staging; abort client | Bridge observes disconnect before release; original Agent resolves staged receipt; canonical/alias bytes remain |
| Hold completed save before staging; dispose Agent | `session/not-found`, no receipt; stored bytes remain |
| Foreign root/same-ID resumed Agent | Cannot resolve/bind owner's receipt; foreign retirement leaves it intact |
| Binding lifecycle | Uncommitted dispose restores unbound; temporary dispose restores committed binding; temporary retirement preserves receipt, committed retirement removes it; Session disposal retires receipt |

Direct `bindPrompt`/commit/dispose/`retirePrompt` tests **do not submit prompts or qualify inbox/history retirement**. Behavior supports, but does not uniquely prove, exact-Session WeakMap implementation.

## Cleanup and evidence refusal

- [Qualifier](<../scripts/qualify-upload-uncertainty.mjs>) wraps only same-instance methods and restores exact descriptors; no provider/prototype/global replacement.
- Await actual delegated bridge continuation, not local fetch rejection. Abort clients/release gates, join admitted handlers, restore descriptors, retire observer, dispose Agents, verify registry, stop Host. **Shipping webserver shutdown does not itself join detached HTTP handlers**.
- Injected ACK/store-held failures remain failed, **exit 1**. Separate acceptance proves exact injection + complete cleanup; never successful upload.
- [Storage oracle](<../scripts/qualify-file-upload.py>) checks full bytes/digests, 0400 canonical/alias hardlinks and 0700 directories; refuses symlinks. Not race-proof descriptor-relative confinement.
- Python denial tests, fresh Rust SDK/five-case storage regressions passed. [Frozen parent manifest](<../app/evidence/parent-upload-uncertainty-qualification.json>) is local-only.

## Remaining limits

**Admitted failure/cancellation/lost ACK may be committed.** No automatic retry, rollback/deletion or cross-Host/Session reuse.

Zero subject events/pre-step are not process-wide network/credential tracing; required services still mount. Cleanup is observed PID/start-time/pidfd ownership, not adversarial descendant containment. Node HTTP barriers cover default-provider post-operation timing, not every partial-write/provider race or Rust/GUI cancellation. Chooser/draft/epoch binding, image preview, prompt delivery and physical input remain separate gates; historical packages are unchanged.
