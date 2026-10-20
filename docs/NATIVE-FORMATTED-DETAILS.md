# Bounded native assistant Details

Opened assistant Details offers **Formatted / Source**; Source is initial. Native text/spans/scrollable code only—not browser/HTML/images/executable syntax. Conversation retains its four-line preview and [bounded geometry/cache](<NATIVE-HISTORY-PERFORMANCE.md>).

## Presentation and authority

- Supports headings/paragraphs/emphasis/strikethrough/inline code, ordered/unordered lists with continuation and task boxes, quotes and lists in either nesting order, rules, fenced/indented code, tables, footnotes, math (drawn as its source), literal raw HTML and inert visible destinations.
- Code preserves parser text/final newline, normalizes CRLF, uses monospace/no wrap/horizontal scroll. Fence label ≤48 code points; Source preserves display-source representation.
- Exact retained `assistant/message`/`assistant/attempt` provenance required—not role label. Live Details is a frozen non-durable snapshot, never auto-refreshed.
- Source is bounded reducer projection, possibly redacted/summarized/truncated reasoning/tool text—not raw wire or guaranteed final reply. Record/timestamp metadata stays separate.
- Read-only/disconnected presentation never changes history/geometry/follow/Send authority or executes commands.
- [Lifecycle](<NATIVE-DETAIL-LIFECYCLE.md>): generation/global consumed-opener fences; Back/mode/Escape require current visible panel stamp. Generic dismissal/covers reject queued actions. Exhaustion preserves last readable snapshot/Back. Same-generation content resolves at admission—not per-record revision lease; frozen archives do not inherit later fixes.

## Bounds and whole-source fallback

| [Parser](<../app/src/formatted_details.rs>) limit | Maximum |
| --- | ---: |
| Input, checked before parser construction | 32 KiB |
| Blocks / globally coalesced spans / open tags | 128 / 1,024 / 16 |
| Generated UTF-8, including amplification/markers/code | 64 KiB |
| Parsed snapshots | One, not all 4,096 rows |

- **Budget** overflow (bytes/blocks/spans/nesting) returns **whole unchanged Source + reason**, never
  partial formatting. Since rounds 527–538 there is no structural refusal left: raw HTML is drawn
  literally (`raw_html_stays_literal_text_in_the_flow`), a link's or image's title is dropped rather
  than failing the source, headings, rules, tables and fenced blocks keep their container's rule and
  indent (`a_heading_inside_a_container_keeps_its_rule_and_indent`,
  `a_rule_inside_a_container_keeps_its_rule_and_indent`,
  `a_table_inside_a_container_keeps_its_rule_and_indent`,
  `a_code_block_inside_a_container_keeps_its_rule_and_indent`), a quote and a list compose in the
  order they opened
  (`mixed_quote_list_ancestries_keep_their_opening_order_instead_of_collapsing`), and an image nested
  in an image joins the outer alt run (`an_image_inside_an_image_joins_the_outer_alt_run`).
- An image still renders its alt text rather than decoding: no URI/file opener, image decode/fetch,
  network or execution. Separate [Copy source](<NATIVE-DETAIL-COPY.md>) explicitly copies frozen display
  source. Not full Markdown parity (math has no typesetter; a task item draws a box glyph).

## Qualified evidence

**Local evidence, ignored on GitHub:** [qualification](<../app/evidence/formatted-details-paint-_6jwwdmh/qualification.json>), inspected [wide](<../app/evidence/formatted-details-paint-_6jwwdmh/assistant-details-formatted/own-window.png>)/[narrow](<../app/evidence/formatted-details-paint-_6jwwdmh/assistant-details-narrow/own-window.png>)/[Source](<../app/evidence/formatted-details-paint-_6jwwdmh/assistant-details-source/own-window.png>)/[fallback](<../app/evidence/formatted-details-paint-_6jwwdmh/assistant-details-fallback/own-window.png>).

- Four actual-App PUBLIC Wayland captures, exact contained mode controls, zero business effects; no Host/worker/clipboard executor.
- Probe **13,570,320 bytes**, SHA `0d9dcf23d764e541ac82b773308179a7f5b1df38148c640d8f8f43482f206ee9`; stable source pins, not reproducible full closure.
- **465/472/474/476** [default](<../app/evidence/native-formatted-details-ao75np6t/final-default-tests.log>)/[feature](<../app/evidence/native-formatted-details-ao75np6t/final-feature-tests.log>)/[layout](<../app/evidence/native-formatted-details-ao75np6t/final-layout-tests.log>)/[scroll](<../app/evidence/native-formatted-details-ao75np6t/final-scroll-tests.log>) regressions; 21 parser/eight admission tests; [all-target check](<../app/evidence/native-formatted-details-ao75np6t/final-all-targets-check.log>). Scoped formatting passed, not blanket shared-App formatting.
- Cached `pulldown-cmark =0.13.4`, default features disabled, adds only parser/`unicase 2.10.0`; [lock delta](<../app/evidence/native-formatted-details-ao75np6t/lock-delta.json>) preserves existing entries. No fresh documentation claim.

From app directory, shared offline cache, `CARGO_BUILD_JOBS=2`:

```sh
cargo test --offline --locked --features public-layout-fixture --example layout_smoke
cargo build --offline --locked --release --features public-layout-fixture --example layout_smoke
python3 qualify_formatted_details.py
```

[Runner](<../app/qualify_formatted_details.py>) freezes the probe, uses private credential-free homes and captures only its renderer.

## Remaining scope

Physical mode/code scrolling, selection/copy, IME/accessibility, compositor fractional scaling and integrated resources/latency remain unqualified. Main-chat Markdown now shares this parser's full construct set (rounds 527–538); relative-path
navigation/edit counts remain incomplete. Comparisons are
drawn as diff rows with hunk headers, colour bars, fills, monospace and two numbered gutters
since rounds 482 and 483; a wrap toggle exists since round 499 and a side-by-side layout since round 505. [Package refresh](<NATIVE-DEVELOPMENT-PACKAGE.md>) separately qualifies default/relocated startup, not parity; older artifacts and installed GUI/profile/desktop stay unchanged.
