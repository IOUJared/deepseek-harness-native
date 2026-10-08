# DeepSeek Harness Native

Native Linux client built with **Rust + Iced**, styled in **Rosé Pine**. No Electron or primary webview. **Development build—not a finished release.**

## Status

| Available | Still needed |
|---|---|
| Chat, tool activity, session navigation | Live model/login verification |
| Approvals, questions, settings | Physical input and accessibility testing |
| Session export, pin/archive controls | Rich attachments, terminal and plugin panels |
| Native Wayland rendering | Installers, updates, Windows/macOS parity |

## Build

Clone these repositories into sibling directories:

- [Native client](https://github.com/IOUJared/deepseek-harness-native)
- [Compatible backend](https://github.com/IOUJared/deepseek-harness-linux)

Build the backend using its instructions, then:

```sh
cargo build --locked --release -j2 --manifest-path deepseek-harness-native/app/Cargo.toml
```

| Requirement | Version |
|---|---|
| Rust | 1.99 |
| Backend | 0.2.1-alpha.1 |
| Node | 26; 26.10 tested |
| Tested desktop | Linux x86_64, Wayland, glibc 2.44 |

[Run instructions](<app/README.md>) · [Feature status](<docs/FEATURE-PARITY.md>) · [Packaging limits](<docs/NATIVE-DEVELOPMENT-PACKAGE.md>)

## CI and downloads

[GitHub Actions](https://github.com/IOUJared/deepseek-harness-native/actions) checks pushes and pull requests. Each successful `main` push publishes a [development prerelease](https://github.com/IOUJared/deepseek-harness-native/releases) such as `v0.1.0-dev.1`, with a Linux executable archive and checksum. The compatible backend and Node remain separate. [Checks and release policy](<docs/CI-RELEASES.md>).

## Components

| Component | Purpose |
|---|---|
| [App](<app/README.md>) | Native interface |
| [Core](<core/README.md>) | Owns the private Node Host |
| [Transport](<transport/README.md>) | Typed, authenticated HTTP/WebSocket |

## Performance

| Synthetic scrolling, scale 1 | RSS | CPU, one core |
|---|---:|---:|
| Iced GPU | 74.48 MiB | 3.92% |
| Slint GPU | 96.02 MiB | 10.42% |

[Measured workload](<benchmarks/results-comparison-03/MEASUREMENTS.md>); not full-app results or a universal toolkit ranking.

## Limits

- Separate backend required; not self-contained or generally portable yet.
- Existing Harness profiles and credentials stay separate.
- Evidence, screenshots, private homes and binaries are local-only; historical evidence links may not resolve on GitHub.
- Packaging still needs the original local Cargo cache; clean-machine packaging is not verified.
- [MIT](<LICENSE>); dependency redistribution requirements remain separate.
