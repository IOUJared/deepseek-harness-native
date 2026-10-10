# Native attachment selection

**Attach** opens an in-window Iced file picker over an inactive, dimmed conversation. Browse folders, go Up/Home, toggle hidden entries or enter an absolute folder. **Enter a file path instead** keeps the existing path editor; Cancel dismisses an empty selection. Only one attachment is owned at a time.

Selection stays local: review the path, click **Upload**, then **Send**. Choosing a file does not upload or prompt. Removing/canceling after upload revokes local authority, not stored Host bytes. Unknown upload/Send results retain the existing explicit-reconciliation restrictions.

## Local work

- Picker scans run on one joined blocking operation; at most 4,096 visited/512 displayed entries. Directories sort first. Symlinks, special files, non-UTF-8/control names and invalid file leaves are excluded. Large folders require a smaller folder or the direct-path fallback.
- PNG/JPEG previews use the upload intake's final-component `NOFOLLOW`, regular-file check, 4 MiB ceiling and cooperative cancellation, without accessing an upload body/receipt. Other formats remain ordinary files.
- Preview dimensions are at most 4,096 per edge/4 million pixels; advertised decoded output is at most 32 MiB. Codec scratch limits are best-effort, **not a total-memory or CPU-time sandbox**. Native RGBA thumbnails are at most 160×120, with no EXIF orientation, color management or animation.
- Preview is a separate local snapshot. Upload reopens the reviewed path; it may contain different bytes. Pathnames are not inode leases or confinement; parent symlinks remain permitted.
- Folder and preview completions have distinct monotonic owners. Edit, Cancel, Remove, session changes and shutdown revoke publication; only the exact joined callback releases its operation slot. Closing waits for admitted local operations. Blocking filesystem calls and codec CPU intervals cannot be forcibly canceled.

## Drag-and-drop

The App's drop admission validates the native window and current session/lifecycle/modal authority before creating a local review. It rejects replacement of an existing draft/receipt and never uploads automatically. A [pinned winit patch](<../app/vendor/README.md>) produces the standard Iced drop event on the existing owned Wayland queue; unpatched 0.30.13 does not support it.

Only one local `file:///` URI is accepted, never a remote URL or a partially validated list. Transfer limits: 64 KiB/64 scanned URI lines, 4,096 decoded path bytes, eight seat/offer sessions, 8 KiB per dispatch and an absolute three-second drop deadline. COPY is the only negotiated action; no MOVE/source deletion. Seat/window removal, replacement, pre-drop leave, malformed input, failed reads and timeout retire owned pipe/timer/offer state. A dropped leave may finish its already-admitted transfer; stale callbacks cannot publish into a removed window. Failed admitted drops publish standard cancellation feedback, shown as a generic visible message with an Attach fallback; successful delivery does not also report failure. This adds no borrowed-display sidecar or I/O thread. Arbitrary compositor metadata retained by SCTK is outside these payload bounds.

A real private Hyprland 0.56.2 check found and fixed SCTK's lost **pre-enter COPY action**. The bounded patch retains only confirmed compositor actions, not source capability guesses; provisional MOVE can renegotiate to COPY before drop. Thirteen production-module tests pass.

- **Passed:** software-pointer GTK COPY drag → actual Iced drop → local path review/PNG preview (spaces/Unicode filename); two-file rejection with visible feedback. Original PUBLIC files stayed unchanged; no Upload/Send/Host/model ran.
- **Failed compatibility check:** ordinary Thunar 4.20.10 drag offers COPY|MOVE, but this Hyprland selects MOVE despite the receiver requesting COPY. The App rejects it with visible feedback; original files stayed unchanged. Use **Attach** instead. Desktop/compositor configuration was not changed.

These are real compositor/client transfers in owned nested windows, **not physical hardware-mouse or real-model qualification**.

## Verification scope

Production reducer/channel tests cover modal fences, local-only selection, stale callbacks, replacement, distinct repeated preview owners, upload/remove association and exit waiting. Disk tests exercise the actual bounded picker/preview operations and intake rejection parity. PUBLIC layout presets render the real picker and thumbnail without Host/model work. The opt-in existing layout fixture forwards only actual native drop/cancellation events and production local-preview Tasks; all business widget messages remain excluded. Hardware pointer behavior and real-model image interpretation require separate checks. Earlier frozen binaries/archives do not inherit these source changes. No new archive was produced: archive source/license collectors must retain the patched local winit source and Apache-2.0 notices before redistributing this build.
