# Composer controls

| Control | Behavior |
|---|---|
| Enter / Ctrl+Enter | Send non-whitespace text in the focused composer |
| Shift+Enter | Insert a newline |
| Send button | Text or an explicitly staged file-only draft |
| Model pill | Open a compact Rosé Pine popup above the actual button |
| Model / Effort | Choose only catalog-advertised routes and effort IDs |
| Speed / Output detail | [Provider-backed choices](<NATIVE-REQUEST-CONTROLS.md>); disabled for unsupported routes |
| Escape / outside click | Close the popup without passing the click through |

- Opening automatically refreshes available models—no Load button. Click a model to switch this chat after the Host accepts it; running generations stay unchanged.
- Menu choices carry epoch, session generation and opening tickets. Unknown/stale choices, covered panels, pending operations and smoke mode cannot apply them.
- Applying state is distinct from a normalized Host ACK. Failure or queue rejection restores authoritative selection metadata; no cost or persistence guarantee is invented.
- Keyboard sends recheck text and existing Send guards. A blank draft with a staged file never submits from Enter. Duplicate/pending sends stay blocked; acknowledgment still owns draft clearing.
- Popup layout does not resize the conversation. It clamps to the viewport and captures input; higher-priority settings/consent/shutdown panels remain in charge.

Implementation: [model controls](<../app/src/model_controls.rs>), [native overlay](<../app/src/anchored_popup.rs>), [composer routing](<../app/src/ui.rs>), [keyboard/file guard tests](<../app/src/attachment_ui_tests.rs>).

Checks: 528 app / 225 fixture-UI tests and nine inspected zero-Host menu modes passed; see [model panel qualification](<NATIVE-MODEL-SUBMENUS.md>). [Provider request controls](<NATIVE-REQUEST-CONTROLS.md>) owns backend scope. Physical input, live execution, Fast latency, full parity and older binaries remain unqualified.
