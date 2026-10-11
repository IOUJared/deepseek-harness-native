# Sidebar organization

Use **⋯** beside a conversation to open its options without selecting or resuming it.

- **Pin / Unpin** and **Archive / Restore** use the Host registry and explicit confirmation. Archive keeps history, removes its pin, and may be refused while work is active. It does not force-stop work. Restore does not repin.
- **Export** opens the existing unredacted ZIP export review for that row, not the currently selected chat. Choose a new absolute destination and explicitly confirm.
- **Move up / down** sets native-local manual order across groups in All conversations, or within the filtered workspace. Pinned conversations keep Host pin order; unpin before reordering.
- **Sidebar workspace** changes grouping only. The original execution directory, backend workspace accounting, files and conversation history remain unchanged. Original workspace removes the grouping override; Ungrouped shows the conversation only in All conversations.
- **Delete → Trash** needs confirmation and is recoverable. Trash hides the conversation from recent, pinned and archived lists. It does not cancel agents, jobs or schedules, erase files/history, or archive the session. Trash lists all groups; restore before opening a trashed conversation. A currently open trashed conversation stays readable with Stop available and sending disabled.

Restore preserves the previous local grouping/order and existing archive state. Missing workspace groups show as unavailable in options, remain retained, and can be reassigned. Sidebar organization is local to this native home, not synchronized to Web or another device. No permanent deletion or Empty Trash is offered.

## Local persistence

A separate private `native-sidebar.v1.json` document and lock use the existing Linux atomic-save owner; composer drafts have a different document/lock. At most 128 conversations can have custom metadata. Metadata stores only session/workspace IDs, manual ranks and Trash flags, never conversation text or credentials. Session logs and released format generations are untouched.

One read/write is admitted at a time. Changes appear only after a successful save acknowledgement. The writer compares the expected disk snapshot under its lock, so a stale cooperating process cannot overwrite newer organization. Failures or uncertain saves require explicit Reload; no automatic mutation replay. Closing options does not cancel an admitted save, and application exit waits for its join. An explicit Exit anyway acknowledges unresolved local persistence; it does not roll back a potentially committed write.

## Verification owners

Strict organization codec/reducer tests own bounds, duplicate IDs, stale menus, confirmation, commit-only projection, failure/reload and missing-session retention. Existing private-file tests plus sidebar namespace/CAS tests own permissions, symlinks, locks, restart, stale writers and injected before/after-rename failures. App tests own row targeting, modal fences, sidebar projection and shutdown joins; PUBLIC native fixtures own rendered options/Trash layouts. These checks do not establish physical pointer input, all kernel schedules, power-loss recovery, live provider execution or cross-device synchronization.
