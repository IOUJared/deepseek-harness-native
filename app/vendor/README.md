# Pinned native window dependency

`winit/` is an isolated source copy of crates.io **winit 0.30.13**, with its upstream Apache-2.0 license. [WINIT-UPSTREAM.json](WINIT-UPSTREAM.json) records the unmodified cached crate's file hashes. The shared Cargo registry and installed Harness runtime are unchanged.

The native app uses a narrow Linux/Wayland patch to receive bounded local-file drag-and-drop through winit's existing connection, seats and calloop queue. No foreign-display lifetime sidecar, second window, browser, polling thread or model request is involved. Keep unchanged upstream files intact; document each patched file and verifier below before shipping. Removing the Cargo patch also removes native Wayland drop support; unpatched 0.30.13 does not generate these file-drop events on Wayland.

## Patch inventory

| File | Owned change |
|---|---|
| `Cargo.toml` | Explicit existing `rustix` filesystem feature for nonblocking descriptor flags |
| `src/platform_impl/linux/common/xkb/compose.rs` | Three equivalent `Option::filter` locale checks satisfy Rust 1.99's `manual_filter` lint; locale precedence and empty-value handling unchanged |
| `src/platform_impl/linux/wayland/data_device.rs` | Bounded local URI parser, COPY/drop admission, nonblocking pipe/deadline ownership; bounded pre-enter action capture and SCTK snapshot reconciliation; thirteen production-module tests |
| `src/platform_impl/linux/wayland/mod.rs` | Register the data-device module |
| `src/platform_impl/linux/wayland/state.rs` | Bind the optional manager and own transfer state on the existing queue |
| `src/platform_impl/linux/wayland/seat/mod.rs` | Create/drop devices for initial/hotplugged seats; cancel removed-seat transfers |
| `src/platform_impl/linux/wayland/event_loop/mod.rs` | Cancel target transfers before removing a closed window |

The parent app workspace includes this package so `cargo test -p winit --lib data_device` executes its actual production-module tests. Its upstream development dependencies are test-only; native release features still come from Iced. No public winit event/API or session format changes. Preserve the upstream license and baseline hashes; replacing the source requires reviewing every listed file.

## Verification

The App's standard drop event admission and upload/review tests remain independent from protocol transfer tests. Parser/transfer tests, native window paint, compositor protocol delivery and physical file-manager interaction establish different facts. Codec/picker features do not depend on the Wayland patch.
