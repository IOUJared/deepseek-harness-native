# Native clipboard paste

Paste reads clipboard text through **`wl-clipboard-rs`**, not Iced's bundled `window_clipboard` → `smithay-clipboard` backend.

## Why

Iced's Wayland clipboard selects a seat only from key/pointer serials, so an ordinary Paste click or Ctrl+V fails with `no events received on any seat` and `read()` returns `None` — paste silently does nothing. `wl-clipboard-rs` owns its own connection, needs no focused seat, and reads the same bytes `wl-paste` sees.

Measured on this Wayland session (Hyprland, `wayland-1`):

| Reader | Result |
|---|---|
| `wl-paste` | 35 bytes |
| `smithay-clipboard` (Iced's backend, foreign and own connection) | `ERR no events received on any seat` |
| app `native_clipboard::read_text()` | 35 bytes |

## Behaviour

- Settings → Models **Paste** button reads through the worker-thread reader.
- Ctrl+V on the Models page is caught by the app's keyboard subscription (Iced's secure `text_input` pastes only through the unusable backend) and replaces the draft.
- Composer Ctrl+V is intercepted in `composer_binding` and inserted as `text_editor::Edit::Paste`.
- Reads are bounded: 256 KiB text cap and a 1.5 s wait, on a detached thread — the UI thread never blocks.
- A failed or empty read leaves the draft untouched; Models shows `Nothing to paste, or the clipboard is unavailable.`
- With `WAYLAND_DISPLAY` unset (X11) the app keeps Iced's own clipboard path.

## Dependency

`wl-clipboard-rs = "=0.9.4"` adds `os_pipe`, `petgraph`, `tree_magic_mini`, `nom` and `fixedbitset`. It is required only because the transitive Iced backend cannot read on this compositor.

## Limits

Not qualified: X11 paste, physical keyboard/IME, compositors without `wlr-data-control` (the crate then falls back to the data device, which needs focus), and non-text clipboard payloads.
