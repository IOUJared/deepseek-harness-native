# Bounded native assistant Details

The production native App now offers **Formatted / Source** in an assistant's opened Details. Source remains the initial presentation. Formatting uses native Iced text, rich spans and scrollable code—not a browser, HTML renderer, image loader or executable syntax engine. The main conversation remains the unchanged four-line preview, preserving the [bounded native geometry/cache](<NATIVE-HISTORY-PERFORMANCE.md>).

## Presentation and authority

- Supports headings, paragraphs, nested emphasis, inline code, pure ordered/unordered lists (including indented continuation paragraphs), pure quotes, rules, fenced/indented code and inert visible link destinations.
- Code preserves the CommonMark parser's text/final newline (including unclosed fences), uses a monospace native widget and horizontal scrollbar without wrapping. Core normalizes CRLF. Fence-info labels are visibly shortened to 48 code points; Source preserves their display-source representation.
- Formatting eligibility uses exact retained `assistant/message` or `assistant/attempt` provenance, not an arbitrary “Assistant” role label. Live assistant Details are explicitly a frozen non-durable snapshot.
- This is the **bounded reducer display source**, not full raw wire data or a guaranteed final reply: folds may include reasoning/tool-call text; source projection can redact, summarize or truncate raw data. Record/timestamp metadata remains outside the Markdown body. Opening and subsequent updates do not automatically refresh an opened snapshot.
- Unknown required records/error/send gates remain authoritative. Formatting neither updates chat geometry/scroll/follow state nor sends a command. The panel works as local read-only presentation while disconnected.
- Current source binds opener requests to the current generation and last consumed global panel-opening identity; consuming an opener retires its lease. Back, mode changes and the visible Details Escape subscription carry the current panel stamp and require effective visibility. Generic stateless dismissals never clear Details, including when closing sidebar/information covers. Settings, export, management, decisions, closing and shell covers reject queued local actions. Exhaustion disables future openings without wrapping or destroying the last readable snapshot/valid Back. This is not a per-record revision lease: same-generation source is resolved when a valid opener arrives. See [lifecycle evidence](<NATIVE-DETAIL-LIFECYCLE.md>); frozen delivered archives do not automatically acquire this later source fix.

## Bounds and whole-source fallback

The pure [parser plan](<../app/src/formatted_details.rs>) checks source length **before constructing the parser**: 32 KiB input, 128 blocks, 1,024 globally counted/coalesced spans, 16 open tags, and 64 KiB generated UTF-8 including destination amplification, markers, fence labels and code. Only one opened snapshot is parsed/retained, never the entire 4,096-row transcript.

Any unsupported structure or exceeded budget returns no document. The UI shows the unchanged opened display Source with a short reason, never a partially accepted document. HTML, images, tables, link titles, headings/rules within containers, quoted code and **mixed quote/list ancestry** use complete Source fallback. Pure quotes and lists may be siblings. Mixed ancestry was deliberately rejected after review: the bounded paragraph API cannot distinguish quote-containing-list from list-containing-quote.

Formatting installs no URI/file opener, image decode/fetch, clipboard action, network request or code execution. The separate explicit [Copy source control](<NATIVE-DETAIL-COPY.md>) copies the bounded frozen display source without interpreting the formatted body. CommonMark reference definitions are non-rendering, as normal; Source remains available. This is basic formatting, **not full Markdown parity**.

## Qualified evidence

[Final native qualification](<../app/evidence/formatted-details-paint-_6jwwdmh/qualification.json>) passed four PUBLIC actual-App owned-Wayland captures: Source, wide Formatted, narrow Formatted and unsupported-image/HTML Source fallback. All were directly inspected. The wide/narrow captures visibly retain list continuation membership and non-wrapping code with a horizontal scrollbar; both modes preserve access to content beyond the chat preview. Native operations measured exact single Formatted/Source control counts when available, contained control bounds and zero captured/dispatched business commands. Host/worker/business/clipboard executors were not started.

- [Wide formatted preview](<../app/evidence/formatted-details-paint-_6jwwdmh/assistant-details-formatted/own-window.png>) · [narrow preview](<../app/evidence/formatted-details-paint-_6jwwdmh/assistant-details-narrow/own-window.png>)
- [Source preview](<../app/evidence/formatted-details-paint-_6jwwdmh/assistant-details-source/own-window.png>) · [unsupported fallback](<../app/evidence/formatted-details-paint-_6jwwdmh/assistant-details-fallback/own-window.png>)
- Frozen scoped probe SHA-256 `0d9dcf23d764e541ac82b773308179a7f5b1df38148c640d8f8f43482f206ee9`, 13,570,320 bytes; source pins unchanged during capture. Observed pins do not establish full reproducible compiled-source/runtime closure.
- Initial formatting qualification source regressions: **465 default / 472 feature App / 474 layout example / 476 scroll example** tests, including **21 parser and 8 detail-admission tests**; all-target feature check passed. See [default log](<../app/evidence/native-formatted-details-ao75np6t/final-default-tests.log>), [feature log](<../app/evidence/native-formatted-details-ao75np6t/final-feature-tests.log>), [layout log](<../app/evidence/native-formatted-details-ao75np6t/final-layout-tests.log>), [scroll log](<../app/evidence/native-formatted-details-ao75np6t/final-scroll-tests.log>) and [all-target check](<../app/evidence/native-formatted-details-ao75np6t/final-all-targets-check.log>).
- Owned parser/panel/admission-test/example formatting passed; shared UI/layout/reducer files were not blanket-formatted. Earlier full-App format limitations remain separate.
- Direct cached `pulldown-cmark =0.13.4`, default features disabled, adds only parser and `unicase 2.10.0`; [lock-entry comparison](<../app/evidence/native-formatted-details-ao75np6t/lock-delta.json>) preserves every existing dependency entry. No HTML writer/image/highlighter dependency was enabled. Context7 quota was exhausted; this used pinned cached sources, not freshly retrieved documentation.

Reproduce from the native app directory, with the existing shared offline Cargo cache and `CARGO_BUILD_JOBS=2`:

```sh
cargo test --offline --locked --features public-layout-fixture --example layout_smoke
cargo build --offline --locked --release --features public-layout-fixture --example layout_smoke
python3 qualify_formatted_details.py
```

The [runner](<../app/qualify_formatted_details.py>) freezes the scoped probe and uses fresh private HOME/XDG directories, retained output and a credential-free display environment. It uses only its own renderer screenshots; it does not capture the desktop or change desktop settings.

## Remaining scope

Physical mode clicks/code-scroll interaction, keyboard selection/copy, IME/accessibility, fractional compositor scaling and integrated whole-app latency/resources are not qualified here. Main-chat full-content Markdown, rich inline diffs, relative-path navigation and real edit counts remain incomplete. The retained older default/keyless artifact and archive are unchanged. The subsequent [development-package refresh](<NATIVE-DEVELOPMENT-PACKAGE.md>) separately qualifies the new default release and relocated keyless launcher, including this source; this scoped Details probe itself is not whole-product or full-parity qualification. Installed Harness Web GUI/profile/runtime and desktop configuration were not modified.
