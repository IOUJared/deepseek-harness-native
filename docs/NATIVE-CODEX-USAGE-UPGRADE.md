# Native usage controls — compatibility change

The exact-alpha native UI no longer offers editable switches for **unavailable** quota display, alerts, reset retry or new Beta opt-in. Its [official-interface limits](<NATIVE-CODEX-INTERFACE-AUDIT.md>) have not changed: no supported same-selected-SDK-account observer/executors are connected. A saved boolean is not a working feature.

- Existing profile choices/documents are **not migrated or silently reset**. Saved modes/threshold remain readable; enabled choices can be explicitly turned Off.
- Refresh reads the local preferences. **Save opt-outs** persists only actual reductions using the existing revision CAS/owned ACK. Unchanged writes, enabling or threshold edits are rejected by the native Settings owner as well as disabled in its view.
- Consent Off still fences locally immediately, including while another write is pending or Settings writes are disabled. It is not durably saved until the exact write ACK; errors/unknown outcomes stay visible across categories/hiding. No automatic retry.
- When a matching owned preference read adopts a changed baseline threshold, native read-only threshold text/value rebase to that baseline without losing Off drafts. Wrong/stale/hidden/retired/failed reads, Save ACKs and duplicate/unrelated callbacks cannot use this hook to mutate a draft or restore consent.
- The generic version-1 preference store/controller remain compatible for future supported integration. No private-pipe/public SDK method, document schema, dependency, runtime profile, login/model route or saved account format changed. Foundation tests that allow edits are **not native capability evidence**.

This change prevents inert controls masquerading as full support; it does not implement quota/notification/retry/generation effects. Existing runtime/model/image-input controls and managed local account operations remain separate supported features. Installed runtime/profiles/accounts/desktop are unchanged; any development artifact is a separate file, not an update.
