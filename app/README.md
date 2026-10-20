# Native app

Rust/Iced frontend for the isolated **0.2.1-alpha.1** backend. Native Wayland, Rosé Pine, no primary webview. Startup loads the catalog to show the actual active model; no prompt is sent.

## Build and run

```sh
cargo test --locked -j2
cargo build --locked --release -j2
./target/release/dsh-native-app \
  --runtime /absolute/deepseek-harness-linux/apps/cli \
  --expected-version 0.2.1-alpha.1 \
  --home /absolute/isolated-native-home \
  --user-home /absolute/user-home \
  --cwd /absolute/workspace \
  --scale 1.0
```

- Requires Rust 1.99, Node 26 and a separately built compatible backend.
- Paths must be absolute; native home must differ from user home. No installed-runtime fallback.
- Optional `--node /absolute/node`; default `/usr/bin/node`, never PATH discovery.
- Scale: 0.75–2.0. `--help` starts neither backend nor GUI.

## Controls

| Area | Use |
|---|---|
| Sidebar | Create, select, filter and manage sessions |
| Chat | Collapsible supplied thoughts, native Markdown/code, Copy/Edit; compact tool details |
| Composer | Send, stop, model/effort popup; Attach opens the native file picker |
| Settings | General, Models, Built-in plugins, Codex, Cloudflare and Agent presets |
| More | Manage, export and information |
| Enter / Ctrl+Enter | Send non-whitespace text; file-only drafts require Send button |
| Shift+Enter | Newline |
| Drag / Ctrl+C | Select message text inside one block and copy it; paste into the composer |
| Ctrl+F | Find in loaded messages, with native highlights |
| Ctrl+B / Escape | Toggle navigation / close auxiliary views |

- [Sidebar options](<../docs/NATIVE-SIDEBAR.md>): row-targeted pin/archive, local ordering/workspace grouping and recoverable Trash. Grouping never changes the agent working directory; Trash never stops work or erases history.
- Navigation shows at most 200 matching sessions in pin/manual/update order. Large-workspace lookup indexes borrow IDs and last only for the current iteration.
- [Title events](<../docs/NATIVE-TITLE-CONTROLS.md>) update labels, never chat rows; raw history stays retained.
- Unknown required events disable Send. Reconnect observations explicitly only after Core verifies the same owned Host; no automatic restart or session follow. Saved sessions require explicit resume and may run queued input.
- Lost Send ACK freezes original text, file ownership, IDs and payload; new draft stays separate. **Check submission** is read-only; unknown/missing/malformed results never permit resend. Frozen recovery is in-memory; exiting is not rollback.
- Verified queued/claimed/admitted/recorded admission releases original ownership, not provider completion. Unsafe writes remain blocked while unresolved. Stop cancels the active turn through its reserved lane, preserves queued inbox, and never implies idle.
- [Attachments](<../docs/NATIVE-ATTACHMENT-UX.md>): one local file, bounded PNG/JPEG preview, explicit Upload then Send; Cancel/Remove never undo Host storage. Wayland drops use the pinned native protocol patch; physical file-manager input remains unqualified.
- [Chat features](<../docs/NATIVE-CHAT-FEATURES.md>): Edit fills a local draft; Send stays explicit. Regenerate is unsupported. Usage is read-only, unavailable without valid Host data.
- [Chat visibility](<../docs/NATIVE-CHAT-VISIBILITY.md>): submitted messages remain visible through preparation; streamed replies, supplied reasoning and failures appear in chat. Context rewrites do not erase conversation.
- Opening the model selector automatically fetches available models and may contact providers. Click a model to switch this chat; no separate Load button. [Catalog receipts](<../docs/NATIVE-CATALOG-OWNERSHIP.md>) now own a global epoch/read ticket; stale/duplicate success or error cannot replace a newer read.
- Without a selection/read, the pill says **Choose model**, not loading. Offline/error beats pending-read text; known names/applying state remain. Smoke Stop is muted/disabled, matching its worker guard; normal cancellation stays reachable despite unknown/false roster activity.
- [Speed/output detail](<../docs/NATIVE-REQUEST-CONTROLS.md>) use provider-backed choices; unsupported routes remain unavailable. Fast may increase billing or quota consumption.

## Safety

| Operation | Rule |
|---|---|
| Approval/question | Explicit answer; closing leaves requests pending |
| API key | Masked input → Review → Confirm; never read back |
| Plugin field | Refresh → Edit → Review → Confirm; revision checked |
| Codex | Connect ChatGPT; official browser login/MFA still required |
| Export | New absolute ZIP path; unredacted content, 4 MiB cap |
| Shutdown | Busy/unknown state requires confirmation; only owned processes stopped |

- Timeouts/disconnects may follow a committed write. **No automatic retry; closing is not rollback.**
- Buffer wiping covers owned Rust allocations, not all widget, Node, kernel, clipboard or swap copies.
- Custom providers may store credentials elsewhere; default-provider checks do not guarantee confinement.
- Codex accounts: Refresh explicitly; local labels/selection are not verified identity or entitlement. Add stays unselected; Connect selects only its exact just-added receipt. Quota is unavailable.
- Account switch/local sign-out require Review → Confirm at the committed epoch; consent replaces the connection/account list. Local sign-out does not revoke OpenAI authorization; legacy grants are never imported. Unknown writes require explicit reconciliation, never automatic retry.
- [Codex usage preferences](<../docs/NATIVE-CODEX-USAGE-POLICY.md>) save locally with revision checks. Local quota opt-out fences immediately; quota display, notifications and reset retry remain unavailable.
- [Codex Models/Images](<../docs/NATIVE-CODEX-USAGE-POLICY.md#remaining-supported-controls>) edit transport, cache retention, timeouts and image-input limits through revision-checked Save. Refresh reconciles uncertainty; Reset edits the shared draft only. Critical runtime status stays in the fixed modal footer.
- [Cloudflare](<../docs/NATIVE-CLOUDFLARE.md>): Workers AI models and Browser Run tools, [Connect Cloudflare](<../docs/NATIVE-CLOUDFLARE-OAUTH.md>) with one-time own-client setup, reserved Cancel, Disconnect and explicit saved API-token selection. Workers Free confirmation remains mandatory; no paid fallback, automatic credential downgrade or model selection. Plan/global quotas are not verified.
- Settings deactivate the background. Closing/category changes never roll back submitted writes; runtime/usage drafts and local opt-out remain retained.
- Transcript: 4,096 records/16 MiB; Details: 32 KiB. Limits fail explicitly, without silently dropping history.

## Reference

[Settings](<../docs/NATIVE-COMPOSED-SETTINGS.md>) · [Codex](<../docs/NATIVE-CODEX.md>) · [Decisions](<../docs/NATIVE-DECISION-OWNERSHIP.md>) · [Export](<../docs/NATIVE-SESSION-EXPORT.md>) · [Session controls](<../docs/NATIVE-REGISTRY-CONTROLS.md>) · [Feature status](<../docs/FEATURE-PARITY.md>)

## Testing and distribution

- Scripted checks cover native rendering, backend startup, snapshots and clean shutdown—not physical input, live models or full parity.
- PUBLIC fixtures use synthetic data; only explicitly composed fixtures run the Host. Evidence and screenshots stay local.
- [Development archive](<../docs/NATIVE-DEVELOPMENT-PACKAGE.md>): external runtime required; not an installer or portable release.
- Frozen `ac91` includes [Copy source](<../docs/NATIVE-DETAIL-COPY.md>) and [Node selection](<../docs/NATIVE-EXPLICIT-NODE.md>), but not newer title changes.

## Deliberate unsupported areas

- Timed/continued questions, rich attachments, terminal/document/plugin panels, accessibility and cross-platform parity.
