# Native frontend modernization

Historical qualification; [control layout](<NATIVE-CONTROL-LAYOUT.md>) supersedes default tool cards and fixed-height tool rows.

## Design intent

- ChatGPT-inspired navigation → conversation → composer, not an OpenAI-branded/pixel-exact copy.
- Native Iced/wgpu, standard dark Rosé Pine and existing font; no Electron, primary webview, new UI dependency or desktop changes.

## Changes

| Area | Earlier modernization |
| --- | --- |
| Navigation | Rounded sidebar, real titles/search, collapse, narrow rail and scrollable workspace sheet |
| Conversation | Centered width, right user bubbles, restrained assistant text, distinct tool/config cards |
| Empty session | Honest welcome; original metadata records disclosed, no invented chat |
| Composer | Rounded editor; explicit model/catalog/reasoning and Send/Stop |
| Sheets | Centered settings/decisions; explicit credential/approval wording |
| Motion | Finite sidebar transition; event-driven hover/focus, no perpetual timer |
| Keyboard | Focused Ctrl+Enter uses normal Send guards; Enter adds newline |

## Preserved invariants

- Backend, transport, persistence, credential and decision authority stay unchanged.
- Pane opening/motion never prompts or loads catalogs. Keyless smoke independently forbids prompt/catalog/settings writes and decision replies.
- Historical history uses bounded 160px virtual rows, overscan and stable detail/sequence keys; not complete Markdown/images/attachments/long-message rendering.
- Masked, one-use credential edits require Review/Confirm; confirmation is not validation/sign-in and failures may be indeterminate.
- Decision replies retain Agent/Event/epoch/attempt fences; ACK is not tool-execution success.

## Validation record

**Local evidence, ignored on GitHub:** [summary](<../app/evidence/frontend-modernization-qppgwtsc/qualification.json>), [125% own-renderer frame](<../app/evidence/frontend-modernization-qppgwtsc/final-native-scale125/own-window.png>), [retained records/backups](<../app/evidence/frontend-modernization-qppgwtsc/>).

- Offline/locked two-job release build, formatting/all-target checks and **83 app tests** passed.
- Two isolated-alpha keyless Wayland runs at application scales 1/1.25; ten PUBLIC component fixtures.
- Real three-record bootstrap fold, zero prompts/catalog requests and graceful exit with no observed descendants.
- Inspected app and selected decision/masked-confirmation frames: readable Rosé Pine controls and spacing.
- Short UI+Node medians: RSS ≈323/329 MiB, PSS ≈247/252 MiB, CPU 0% at sampling resolution—not matched long-chat benchmarks or proof of zero work.
- Unqualified: generation/live approvals, physical input/IME, arbitrary/minimum resizing, CJK/accessibility, compositor fractional scaling and parity.
- One prior component-style backup was reconstructed after reading, not a byte-identical pre-edit snapshot.
