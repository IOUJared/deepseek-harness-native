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
| Completion row | Complete the command the pointer names, then keep the caret |
| Card, completion menu, metadata strip | Keep the caret; the press owns no other action |
| Search close | Return the caret to the composer |

- Opening automatically refreshes available models—no Load button. Click a model to switch this chat after the Host accepts it; running generations stay unchanged.
- Menu choices carry epoch, session generation and opening tickets. Unknown/stale choices, covered panels, pending operations and smoke mode cannot apply them.
- Applying state is distinct from a normalized Host ACK. Failure or queue rejection restores authoritative selection metadata; no cost or persistence guarantee is invented.
- Keyboard sends recheck text and existing Send guards. A blank draft with a staged file never submits from Enter. Duplicate/pending sends stay blocked; acknowledgment still owns draft clearing.
- Popup layout does not resize the conversation. It clamps to the viewport and captures input; higher-priority settings/consent/shutdown panels remain in charge.

## Caret retention

Iced's text editor clears its own focus on any left press outside its bounds, and it reads the press before the widget wrapped around it does, so `mouse_area` cannot intercept it. `FocusKeeper` therefore runs the content first and asks for focus back only when no child consumed the event. Consequences:

- Presses inside the composer that own no action (card padding, completion menu, metadata gaps) publish `ComposerFocus`, which is a focus operation and nothing else.
- A control's press is consumed by that control, so Send, Attach, the model pill and the plan chip keep their own meaning and no second request is published.
- Completion rows are pointer-selectable and complete the row the pointer named, never the highlighted one; the draft keeps every typed character.
- Text fields outside the composer are untouched: the picker, settings, consent dialogs and the search field own their own focus, and search now hands the caret back to the composer when it closes.

Not qualified: rendered-window pointer behavior. The unit tests pin the message routing and the keeper's event precedence; a real GPU renderer is needed to build the rendered composer headlessly, so no test here presses a physical or synthetic pointer on the real view.

Implementation: [model controls](<../app/src/model_controls.rs>), [native overlay](<../app/src/anchored_popup.rs>), [composer routing](<../app/src/ui.rs>), [keyboard/file guard tests](<../app/src/attachment_ui_tests.rs>).

Checks: 528 app / 225 fixture-UI tests and nine inspected zero-Host menu modes passed; see [model panel qualification](<NATIVE-MODEL-SUBMENUS.md>). [Provider request controls](<NATIVE-REQUEST-CONTROLS.md>) owns backend scope. Physical input, live execution, Fast latency, full parity and older binaries remain unqualified.
