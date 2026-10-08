# Native app

Rust/Iced frontend for the isolated **0.2.1-alpha.1** backend. Native Wayland, Rosé Pine, no primary webview. Startup sends no prompt and loads no model catalog.

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
| Chat | Compact tool rows; click for stored details |
| Composer | Send, stop, model/effort popup |
| Settings | General, API login, Codex and Plugins |
| More | Manage, export and information |
| Enter / Ctrl+Enter | Send non-whitespace text; file-only drafts require Send button |
| Shift+Enter | Newline |
| Ctrl+B / Escape | Toggle navigation / close auxiliary views |

- [Title events](<../docs/NATIVE-TITLE-CONTROLS.md>) update labels, never chat rows; raw history stays retained.
- Unknown required events disable Send; reload to recover. No automatic reconnect.
- Drafts survive failed Send. An ACK is not generation or tool-execution success.
- Model/effort choices use real catalog metadata. Load/Reload may contact providers; opening the popup does not.
- Speed and output-detail preferences remain unavailable, not simulated.

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
- Settings deactivate the background. Closing/category changes discard unsaved drafts, not submitted writes.
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
