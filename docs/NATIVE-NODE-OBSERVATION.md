# Native development Node prerequisites

Development launcher/package checks, **not Core direct-launch policy or bundled runtime**.

## Accepted observations

[Launcher](<../scripts/development-launcher.py>) / [qualifier](<../scripts/qualify-development-package.py>) share strict parser and **five-second** probe. Default `/usr/bin/node --version`; optional `--node` selects an absolute canonical executable, never PATH/fallback. **Even `--check` executes it: trust the selected program.** Credential/Node/loader overrides are scrubbed.

| Accepted metadata | Bound |
|---|---|
| Version | ASCII `v26.<minor>.<patch>`, each numeric component 1–4 digits, optional single trailing LF |
| Exit status | Exact integer zero |
| stdout/stderr | ≤32 / ≤1024 bytes |

Wrong major/malformed/control/extra output/failure/timeout refuse without raw output. Python buffers before length checks: **not adversarial memory containment**. Probe does not bind inode/hash to later launch or change subprocess ownership/Core validation.

`--check` reports `nodeVersionObserved`, `nodeSelectionMode`, `nodeMajorAccepted:26`; legacy `systemNodeVersionObserved` only for default selection. It acquires no data lock, creates no data and starts no GUI/Host; prerequisite-only, not runtime dependency/startup validation. Private five-value `plan()` stays compatible.

Numeric Node-26 acceptance does **not qualify every patch**. Actual tested system: **v26.10.0**; certificate patch correction was earlier builder work.

## Retained qualification

Ignored [parent qualification](<../app/evidence/node-version-observation-apneifwz/qualification.json>) is **local-only**, not a GitHub download.

- **66 guards:** malformed/status/timeout privacy, PUBLIC alternate patches, tuple compatibility, refusal before data/startup.
- Fresh staging with frozen `33f79d…` native executable: real launcher→native→full alpha Host at application scales **1/1.25**, v26.10.0, blank fold, zero catalog/generation, native Wayland, graceful stop/no observed survivors/released lock. Both inspected paints match prior archive.
- That binary **lacks later Details lifecycle fix**; staging did not replace its archive. [Current development package](<NATIVE-DEVELOPMENT-PACKAGE.md>) separately qualifies observed/explicit Node plus lifecycle fixes; older evidence stays historical.
- No Rust/default binary changed in this step. No self-contained Node/Host relocation, physical input, compositor fractional scale or full parity claim; installed data/settings preserved.
