# Native Details source copying

**Copy source** copies the frozen, bounded reducer display source in either Source or Formatted mode—not rendered text, timestamps, raw wire data or necessarily a complete final reply. Reasoning/tool-call text may be included. [Formatting](<NATIVE-FORMATTED-DETAILS.md>) stays inert.

## Admission and effects

- [App admission](<../app/src/ui.rs>): visible Details, current generation and exact opening stamp. Settings/export/management/decisions/closing/shell covers reject queued Copy; Back/reopen/conversation changes retire old tickets.
- Disconnected panels can copy their original snapshot; transcript updates do not refresh it. The final valid ticket survives opening-counter exhaustion.
- [Panel](<../app/src/detail_panel.rs>): nonempty source, at most **32 KiB UTF-8**, sharing the [parser bound](<../app/src/formatted_details.rs>). Empty/oversized source disables Copy; no truncation or clipboard clearing.
- One Iced Standard-clipboard write task; no clipboard read/Primary selection, worker command, network/send or draft/reader/Send-authority change.
- Clipboard is accessible to other apps; storage/wiping is not guaranteed. No write ACK, so no “Copied” success claim. Later Back cannot revoke an admitted platform task.

## Verification and distribution

**Local evidence, ignored on GitHub:** [Copy record](<../app/evidence/detail-copy-source-h2s9xcjb/qualification.json>), [four paint cases](<../app/evidence/formatted-details-paint-nn0d28p4/qualification.json>), [narrow frame](<../app/evidence/formatted-details-paint-nn0d28p4/assistant-details-narrow/own-window.png>).

- 22 Details tests, including five Copy cases; **479/486/488/490** default/feature/layout/scroll regressions; all-target and four-file formatting passed. Full-App formatting remains unqualified.
- Tests inspect exact source/task admission, then **drop every clipboard task**. Four actual-App frames show one contained Copy/Back, expected mode controls and zero business effects; all inspected.
- No Host/clipboard executor/desktop capture. Physical clicks, OS clipboard output/ownership, accessibility and integrated performance remain unqualified.
- [Explicit-Node qualification](<NATIVE-EXPLICIT-NODE.md>) separately rebuilt the default executable: four assistant frames match the inspected probe; tool Details inspected separately.
- [Development archive](<NATIVE-DEVELOPMENT-PACKAGE.md>) includes Copy; older ba14 remains frozen without it. Package startup does not qualify clipboard output. Installed runtime/profile/GUI and desktop settings stay untouched.
