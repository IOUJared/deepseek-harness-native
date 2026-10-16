# Native UI parity — web reference

Extracted measurements and web→native mapping for the first main-screen pass. Authoritative method: [native-ui-parity skill](<../.agents/skills/native-ui-parity/SKILL.md>). Build on this; do not re-derive by eye.

Pinned web reference: `IOUJared/deepseek-harness-linux`, branch `native-linux`, commit `c6044cc50bc4db0a2b57d1e02d0b53bfeda4590c`.

## Palette differences (first pass: keep native Rosé Pine)

| Role | Web token (dark) | Web value | Native |
|---|---|---|---|
| Base background | `--dsw-alias-bg-base` | `#151517` (neutral-bluish-950) | `BASE` `#191724` |
| Surface / layer | `--dsw-alias-bg-layer-2` | `#2c2c2e` | `SURFACE` `#1f1d2e` |
| Overlay | `--dsw-alias-bg-overlay` | `#616669` | `OVERLAY` `#26233a` |
| Primary text | `--dsw-alias-label-primary` | `#f0f2f5` | `TEXT` `#e0def4` |
| Secondary text | `--dsw-alias-label-secondary` | `#c3c6c9` | `MUTED` `#908caa` |
| Brand / accent | `--dsw-static-deepseek-*` | blue `#5a86fe` | `ACCENT` `#c4a7e7` (foil) |
| Error | `--dsw-alias-state-error-primary` | `#f25a5a` | `DANGER` `#eb6f92` |

Web uses a neutral-bluish scale with a business-blue accent; native keeps standard dark Rosé Pine. First pass preserves the native palette by design.

## Radius scale

| Web token | Value | Native constant (design.rs) |
|---|---|---|
| `xs` | 4 | — |
| `sm` | 8 | `RADIUS_SM` 9 |
| `md` | 12 | `RADIUS_MD` 12 |
| `lg` | 16 | `RADIUS_LG` 18 |
| `xl` | 20 | `RADIUS_XL` 20 |
| `panel` | 28 | `RADIUS_XL` 20 (the panel reuses xl; intentional, tighter) |

Native rounds to whole values and uses a tighter panel radius; see remaining differences.

## Typography

| Web token | Size / line-height / weight | Native role |
|---|---|---|
| `markdown-base` | 14 / 24 / 400 | 15 px DejaVu Sans body |
| `xs-13` | 13 / 20 / 400 | 13 px rows/buttons |
| `xxs-12` | 12 / 18 / 400 | 12 px meta/tool rows |
| `xxxs-11` | 11 / 14 / 400 | 11 px hints/status |
| `code` / `code-block` | 12 / 19 · 11 / 19 | 13 px DejaVu Sans Mono |
| `l-20` / `xl-24` | 20/28 · 24/32 | 18/22/25 px headings |

Native font stack (DejaVu Sans) and 14/15 px base are retained; the web 16 px base and system font stack are a documented difference.

## Main-screen dimensions

| Surface | Web value | Native |
|---|---|---|
| Sidebar column padding | `6px 12px` | `design::SIDEBAR_PAD` 6/12 |
| Session cell | 32 h · `0 8px` pad · radius md · title 14/20 clipped | `SESSION_ROW_PAD` 6/8 + `SESSION_TITLE_LINE` 20 (32 h), radius md |
| Panel row | `min-height: 36` · `7px 8px` pad · radius md | `PANEL_ROW_HEIGHT`/`PANEL_ROW_PAD`, radius md |
| New-session row | 38 h · 8/12 pad · radius md · elevated fill + 0.5px border | `NEW_SESSION_*` + `design::elevated`, 2px inset, 12px below |
| Sidebar search | 30 h expanded · `0.5px` l4 hairline · radius md · 13/18 text | `design::search_field` + `SEARCH_*` |
| Browser section header | 36 h · gap 4 · 4px trailing margin · 28px icon controls | `SECTION_HEADER_*` box; label and text stand-ins keep native's hint tier |
| Sidebar rail | 56 wide · `18px 10px 6px` seat · 36×36 controls · 12px rhythm | `design::RAIL`/`RAIL_PAD`/`RAIL_CONTROL`/`RAIL_RHYTHM`; the plus and gear are verified DejaVu glyphs |
| Conversation header | 76 min-h · bottom 0.5px seam · 30px title row · tabs 13/16 + 2px bar | padding `10px 28px 0 20px` (tabs) / `… 10px` (no tabs) + 1px seam; `HEADER_*`/`TAB_*` carry the rows (band 77) |
| Header crumbs | `4px` gap · crumb `4px 8px` · radius md · cap 220px · 14/20 (subagent 12/18) · 14/20 caption `/` | `CRUMB_*`; the trail walks `parentSessionId` while the origin is a subagent |
| Chat content width | `clamp(680, 64%, 920)` | `design::chat_width(pane)` clamp, bounded by the padded pane |
| Window box | main 1280×820 · min 520×600 | `design::WINDOW_*`/`WINDOW_MIN_*`, pinned by `the_window_box_matches_the_reference` |
| User bubble | `padding: 10px 16px` · radius xl · stack cap `min(content × 0.702, 82%)` | `USER_BUBBLE_PAD`/`_INSET`/`_RADIUS`/`_SHARE`; the cap follows the live lane |
| Code block | `radius-lg` · banner `9px 14px` at 11/18 · `pre` `padding: 16px` · code `11px/19px` · banner one step over the block | `CODE_FONT`/`CODE_LINE`/`CODE_PAD`/`CODE_BANNER_*`/`CODE_CHROME`; both renderers and the offset path read them |
| Blockquote | `border-left: 2px solid label-caption` · `padding-left: 14px` · `margin: 16px 0 0` | `QUOTE_BAR`/`QUOTE_PAD`/`QUOTE_INSET`; a filled rule whose height is the paragraph's own measured height |
| Editable context menu | Undo · Redo · — · Cut · Copy · Paste · — · Select all, each enabled from the edit flags, no accelerator labels, 180px panel | `editor_menu::entries`/`view` + a 100-step editor history; `native-editor-menu` rect |
| Search offset | measured with the drawn block heights, collapsed gaps and the ladder, for the answer and the reasoning alike | `document_first_offset` + `document_first_offset_walks_drawn_heights_and_gaps` |
| Referenced-session summary | under a user bubble: `Referenced session · labels` at 13px/18px tertiary, labels from the following `session-reference` context record, joined with `, ` | `DisplayRow::references` + `REFERENCE_LINE`/`reference_line` and the row's geometry; `chat-references` preset |
| Markdown image | unresolvable source renders the alt as tertiary italic; a failed load adds the destination when there is no alt (`imageAlt`, `radius-md` block) | `Span::alt` + `render.tsx:640,653`; no loading or decoding pipeline yet |
| Markdown table | `th`/`td` `padding: 10px 16px`, first cell flush, last cell flush right, a rule under every row, `13px/22px`, fills under four columns and scrolls at a `100px` floor above it | `TABLE_PAD_*`/`TABLE_MIN_COLUMN`/`TABLE_FILL_COLUMNS`/`TABLE_LINE` + the shared `table_view`; `Block::Table` |
| Compact markdown | Reasoning body + trajectory detail use the secondary ladder: `p`/`ul`/`ol`/`blockquote` `4px 0`, `li` `2px`, headings `8px 0 4px` at the inherited size/600, `hr` `8px 0`, code `4px 0` in the surrounding colour, quote padding `8px` | `Ladder::Compact` + `COMPACT_*`; the reasoning body is parsed into the same block model |
| Markdown rhythm | per-block margins, CSS-collapsed: `p` 16/16 · `h1–h3` 32/16 · `h4–h6` 16/16 · `li` 6 inside, list 16 at its edges · nested list 4 · `hr` 32/32 · code 16/11 · `h4–h6`+list 8 | `block_margins`/`document_gap` over `BLOCK_GAP`/`RULE_GAP`/`LIST_ITEM_GAP`/`NESTED_LIST_GAP`/`CODE_LAST_GAP`/`TIGHT_HEADING_GAP`; both renderers and the offset walk read them |
| Width handles | 10px strip at lane ± 24 · `2px` glow `4px` in, `16px` core, hover/drag only | `WIDTH_HANDLE_*` + `HANDLE_GLOW_*`, hover/drag state, `native-width-handle-*` rects |
| Jump to latest | `34px` round floating button · slot `bottom: 16px` · right edge on the column (or `32px` in) · only while not following the tail | `TO_BOTTOM_*` + `native-to-bottom` rect; `design::LINE` hover, no shadow |
| Scrollbars | `5px` bar · invisible rail · `scrollbar-bg-l1` thumb, `hover-l1` on hover, declared for every element | `SCROLLBAR_WIDTH` + `design::scrollbar`/`scrollable_style` on all 18 scrollers; `LINE`/`MUTED` as the l1 pair |
| Load earlier | centered row · button `4px 12px` · radius sm · `12px` secondary on the hover-solid fill · disabled while loading | `design::fill_solid` + `native-load-older` rect; the duplicate left native's own bar |
| User bubble max-width | `min(70.2% · content, 82%)` | content-sized, fixed 640 cap (≈70.2% of the widest column) |
| Assistant text | unboxed, full content column | unboxed, spans the shared content column |
| Message flow gap | 6 process / 12 response / 16 turn | `design::FLOW_GAP`/`RESPONSE_GAP` ladder 6/12; the 16px `turn-process` row is Trajectory-only |
| Tool activity | compact unboxed single muted line | compact unboxed single muted line, expands in place |
| Tool-row file path | grey **dotted** underline, clickable | solid `MUTED` underline (Read/Edit/Write) |
| Edit diff stat | `+N -M`, code font 11px, 10px left margin, muted until hover/expanded, dropped on error | same call-argument totals; `DIFF_STAT_*` + ASCII `-`, cued colours, dropped when the row failed |
| Composer card | panel 28 · `padding-top: 8` · `gap: 12` · input-major + l2 hairline | `RADIUS_XL` 20; `COMPOSER_CARD_*` + `design::elevated_surface` |
| Composer input | `min-height: 36` · `4px 8px 0 14px` · 14-line text cap 336 | `COMPOSER_INPUT_*`, one-line floor 36, 14 lines (8 in a short window) |
| Composer control row | `2px 8px 6px` · gap 12 | `COMPOSER_ROW_PAD`/`COMPOSER_ROW_GAP` |
| Composer model chip | `.select` 28 h · `0 20px 0 8px` · radius sm · transparent · 13/20 | `COMPOSER_CHIP_*` + `design::ghost`; in the trailing group |
| Composer plan chip | 28 h · `0 8px` · `gap: 4` · 14px glyph slot · radius sm · business tertiary/primary · 6% hover mix | `PLAN_CHIP_*`; in the leading group while plan mode is on, dimmed while a toggle is in flight |
| Send / stop control | 34×34 circle · info fill · icon glyph | 34 h · `COMPOSER_CONTROL_HEIGHT`; native keeps the words and its fills |
| Composer seat | `.root` `0 16px 4px` · dock strip 4px · card = content + 32 | `COMPOSER_SEAT_PAD`/`COMPOSER_DOCK_GAP`/`COMPOSER_CARD_OVERHANG` |
| Empty-state hero | headline + composer card centered as one stack, `gap: 12`; hero composer floor `52` | `HERO_GAP`/`HERO_INPUT_MIN`; `hero_pane` centers both, `COMPOSER_HERO_SEAT_PAD` drops the bottom pad |
| Hero workspace chip | 28 min-h · `0 8px` · `gap: 4` · radius sm · 16px glyph + 12px chevron · row `0 16px 0 20px` + `margin-top: 4` | `HERO_CHIP_*`/`HERO_ROW_*`; opens native's sidebar workspace list |
| Composer attachment rail | `padding: 2px 10px 0` · `-6px` pull · 10px gap · 64px items · 240×64 cards · 18px round remove 6px in | `ATTACH_RAIL_*`; the card's first child, above the draft |
| Hero headline | mark + `Into the Unknown` in one centered row, `column-gap: 10`, `26/32` wt500 | `HERO_HEADLINE_GAP`/`HERO_TITLE_*`; the session-less state keeps native's own line |
| Composer placeholder | `placeholder.hero` in the hero, `placeholder.default` docked | state-owned string in `composer()` |
| Transcript side inset | `calc(clearance + 16px)` = 32 per side | `design::TRANSCRIPT_INSET`; `chat_lane` = `min(column − 64, chat_cap)` |
| Menu | 13 px · blur 40 · hairline | native panel, no blur |

## Intentional differences (first pass)

- **Palette:** Rosé Pine retained; web blue/neutral scale not ported.
- **Fonts:** DejaVu Sans (14/15 px) vs web system stack (16 px base); native code 13 px vs web 11 px code-block.
- **Radii:** native 9/12/18/20 rounded values; panel 20 instead of 28.
- **Content width:** native follows the web clamp (`680…920`) and also honours a stored preference with the web's own precedence — `design::chat_width_with` clamps it to `[640, max(640, column − 176)]`, the web's `CONTENT_MIN` and `CONTENT_EDGE_BUDGET`, and the value persists in a `native-layout.v1.json` namespace beside the drafts and sidebar documents. The web's **drag handles** now exist too: two `10px` handles sit `24px` outside the lane, overlaid with `Stack` so the lane and the scroll model are untouched, and drawn only in the active phase and only when the pane can hold both — the web's own rule. Dragging moves the width by twice the pointer delta (`design::chat_width_drag`), a press that never moves stores nothing, Escape abandons the gesture, and only a commit writes the file. The gesture follows the **window cursor** through a gated `iced::event::listen()` stream, because a handle's own `on_move` is local to its 10px box. *Source-derived and unit-tested; the rendered handle placement has not been visually verified.*
- **Expanded thinking body:** the web indents it under the disclosure *title* rather than its glyph — `ReasoningRow.module.css` `.thinkBody` uses `calc(22px + delta)`, documented there as the leading `16 + delta` plus a `6` gap. Native now applies the same indent through `design::CONTENT_INSET`, and — the part that mattered — measures the body, its `body_height`, and its first-match offset at the same narrower width. Previously the offset already assumed a 22 px indent the render did not draw, so the two disagreed by the indent; the fixture capture now shows the body indented and wrapping cleanly with nothing clipped.
- **Content size preference:** the web's setting drives `--dsh-content-font-delta` and `--dsh-content-font-delta-secondary`, and native keeps **both** because they diverge above the default: at 15 px the body gains a pixel while the secondary tier stays at 13. `design::content_delta` / `content_delta_secondary` derive them, `design::content_size` maps anything outside `11..=20` back to the default rather than clamping, and the preference persists in the layout namespace beside the width (`layout.contentFontSize`). The delta scales the transcript body and markdown sizes, the reasoning and mono tiers, the tool line (`24 + delta`), the leading box (`16 + delta`) and the reasoning row. **Both geometry caches carry the deltas and clear themselves when either changes**, so no measurement or prepared reply can survive a size change; that invalidation is what the `chat_geometry` test asserts. Control: a `Content text size` stepper in the More panel.
  - One residual: the fixed non-message rows (`chat_geometry`'s opaque-row `75`/`156`) still do not scale. `design::CONTENT_INSET` now does — the web writes that inset `calc(22px + var(--dsh-content-font-delta))` (`ReasoningRow.module.css:75`), so it tracks the body size the way the row heights do.
  - The fixture can render any offered size (`--content-size`), and the captures show 14 and 18: at 18 the bubble grows to fit its wrapped lines and the tool rows scale, nothing clipped. **The first scaled capture caught a real bug** — `chat_geometry` measured the user body at a hard-coded `15.0` while the render used the scaled size, which cut off the last wrapped line; the measurement now reads the same delta. The default render is byte-identical before and after that fix, which is what makes "the delta is the identity at 14" a checked claim rather than an assumption.
- **User action row:** the web keeps a user message's bubble and its action row as *siblings* in a right-aligned column — `MessageItem.module.css` `.userRow` wraps `.bubble`, and its own comment says the bubble and action row "align to the right". Native previously painted `Copy`/`Edit` **inside** the bubble's rounded box. Native now matches: the card style wraps the body only, and the actions sit below it, right-aligned to the bubble's edge. The gap is now the web's `6px` (`.userRow { gap: 6px }`; the `8px` belongs to the inner `.userStack`, which is a different box — an earlier note here said 8px and was wrong). `ACTION_GAP`, `ACTION_HEIGHT` and `ACTION_FOOTER` in `ui.rs` are read by both the geometry reservation and the view, so the reserved height cannot drift from what is drawn.
- **Transcript line boxes:** the web sets them in pixels, not as a ratio — body text is `calc(24px + var(--dsh-content-font-delta))` (`AssistantMarkdown.module.css:13`, `ToolRow.module.css:42`) and compact/secondary text is `calc(20px + var(--dsh-content-font-delta-secondary))` (`MarkdownText.module.css:334`, `ReasoningRow.module.css:49`) — where native used Iced's relative `LineHeight::default()`, which is `1.3`. At native's body size that was 19.5px against the web's 24px, about 23% tighter. `design::body_line` / `design::compact_line` now carry the boxes, and every transcript measurement and render reads one of them: the answer and user text, the reasoning body, markdown paragraphs, headings, lists and code, and the tool rows with their recorded bodies. Two derived values followed: the four-line preview cap (`78` was four lines of the old box, now `PREVIEW_LINES × LINE_BODY` = 96, with the floor at one line box) and `chat_geometry`'s preview measurement, which still used `1.3` and **clipped the last line of a four-line user bubble** until it read the same box. The detail panel keeps its own relative box.
- **User attachment spacing (structural, not a value):** the web's `.userStack { gap: 8px }` (`MessageItem.module.css:5-10`) separates an **attachment row** from the bubble — `MessageItem.tsx:194-221` puts `attachmentRow` (image or file cards) first and the bubble second inside that one stack. It is not a gap between text runs, so there is nothing to set while native renders an attachment as its own right-aligned row (the claimed-input/file bubble) instead of stacking it above the message text. Closing it means restructuring attachment placement to match, not changing a spacing constant. **Correction to the first version of this note**, which said an attachment is its own transcript row: `file_summary` (`reducer.rs:736`) feeds `content()`, and `content()` **joins every content part with `\n`** (`reducer.rs:769-770`), so an attachment and its message text are one row holding one joined string. The two lines visible in a capture are two lines of that string, not two rows. So native is already closer to the reference than the note claimed — the attachment *is* in the message's own bubble — and what actually differs is that the reference renders a distinct file card above the text with the `.userStack` 8px gap, where native has a plain `File · … · bytes [preview unavailable]` line joined by a newline. Closing that means keeping the parts distinct through the model (not joining in `content()`) and giving the attachment block an 8px gap, which is a reducer-plus-view change rather than a view merge.

Remaining unqualified native work is unchanged: physical input/IME, CJK, accessibility, compositor fractional scaling, live provider execution.

## Verification owners

Native geometry/cache tests, `layout_smoke` PUBLIC fixtures, App reducer tests, and `docs/FEATURE-PARITY.md` status. Source-only comparison is not visual parity; rendered capture status is recorded in the handoff.

## Verification log (append-only)

Entries here are appended, never inserted: earlier notes were lost because they
were anchored onto rows of a markdown table above and collapsed into a single
cell, so new findings go at the end of the file as plain paragraphs.

- **Assistant tail clock — verified.** `shots/27-assistant-tail-clock.png`: the
  assistant row reads `[copy] Details` then `18:00`, dim, at the reference's
  secondary-minus-one size (`MessageIconActions.module.css:23-37`).
- **Tabs-less header padding — uncovered, not merely unverified.** The branch
  from `ConversationRoot.module.css:27-30` never runs in any current fixture:
  `wide-welcome` still selects a session, so `transcript_tabs_visible()` is true
  and the band keeps `bottom: 0`. A preset with no selected session would
  exercise it; until then the branch has no coverage.
- **Conversation header merge — verified.** `shots/25-single-band-header.png`:
  the title and the `Chat`/`Trajectory` tabs share one band and one left edge.
  The merge is `header()` folding `transcript_tabs()` in at `ui.rs:4991`'s
  caller, with the tab strip's own horizontal padding dropped to zero.
- **Tabs-less header padding — now covered and verified.** A `no-session`
  fixture preset (`layout_fixture.rs`) leaves `app.selected` unset, which makes
  `transcript_tabs_visible()` false and exercises the
  `ConversationRoot.module.css:27-30` branch. `shots/28-no-session-header.png`
  shows the title alone with its 10px bottom padding above the seam, where
  before the preset existed the branch never ran.

### Open items and their dispositions

- **Attachment parts — closed and capture-verified.** `file_summary`
  (`reducer.rs:770`) fed `content()`, and `content()` joins every content part with
  a newline (`reducer.rs:881`), so an attachment and its message text were one row
  holding one joined string where the reference draws a distinct file card above the
  text with `.userStack`'s 8px gap (`MessageItem.tsx:194-221`).
  `user_content()` now splits a user message's parts — `DisplayRow` carries
  `FileCard { name, meta }` values in `files` — the view stacks the cards above the
  bubble, and the reserved height rides `design::attachment_height` at both measure
  sites. The planning bullets below it (the three sites, the fixture-first order,
  the exact fixture edit, the cheap route, the macro trap and the coverage
  distinction) are closed by the verification log's message-attachment entry, which
  keeps the trap and the coverage distinction as accurate history.
- **Assistant `Details ▸` — not a divergence.** The reference's action row is
  copy-and-branch icons with no labelled disclosure, so there is no reference
  value to copy; `Details ▸` is native's door to the detail panel, which is a
  deliberate native surface. Documented rather than changed.
- **Drawn action icons — blocked, two independent reasons.** `iced`'s `canvas`
  feature needs the lyon crates, which the offline cache does not hold, so it is a
  new dependency family; and composing two sheets from `pin`/`Stack` was tried and
  fails, because a widget's fill occludes rather than strokes. The DejaVu glyphs
  (`❐`, `✎`) stand.
- **Small radius — decided, not overlooked.** The web's `--dsw-radius-sm` is 8px
  against native's `RADIUS_SM` 9. It is part of native's documented radius ladder
  and is shared by every small-radius surface, so moving it is a cross-surface
  decision; no measurement depends on it.
- **Attachment gap — the three sites that must move together (located).** Drawing
  a user row's text happens at `ui.rs:7055-7060` through
  `conversation_content::plain_view`, but its height comes from two other places:
  the cached measurement at `ui.rs:6652`
  (`chat_geometry::geometry(row.key, role, &excerpt(&row.text), lane)`, whose
  human-row paragraph is measured in `chat_geometry` at the body line box) and the
  search-expanded override, which calls `plain_height(&row.text, ...)`. Any
  `File · …` block plus its 8px gap therefore has to be added in all three, exactly
  as the line-box and content-size changes were. Adding it to the view alone is
  the shape of bug that clipped a four-line bubble earlier in this work.
- **Attachment gap — do the fixture first.** The gap has **no fixture coverage**:
  the only attachment machinery in `layout_fixture.rs` is
  `public_attachment_fixture` (`:836`), which stages files in the *composer*,
  while the code that builds a `File · …` line is `content()`'s handling of a
  `"file"` content part (`reducer.rs:765`). No preset sends a message carrying
  such a part, so a capture cannot show the gap today. The order of work is
  therefore: add a preset whose user message carries a `"file"` part (the same
  move as the `no-session` preset that uncovered the tabs-less header), then make
  the three-site change, then capture.
- **Attachment fixture — the exact edit.** `public_tool_records()`
  (`layout_fixture.rs:420`) builds the conversation's first user message, whose
  content is already an array:
  `"content":[{"type":"text","text":"PUBLIC conversation · compare user, …"}]`
  (`:433`). Adding a second part
  `{"type":"file","attachment":{"name":"PUBLIC-note.txt","bytes":12}}` there is
  what produces a `File · …` line through `content()` (`reducer.rs:765`). The
  function takes no arguments and is shared by several presets, so gating it means
  threading the preset name into it (or adding a dedicated record builder) rather
  than editing the shared message, which would change every preset that includes
  it and the tests that count its rows.
- **Attachment coverage, precisely.** Two different things are true and the
  distinction matters: the *text* a file part produces is pinned by a unit test —
  `reducer.rs:903` asserts `File · PUBLIC.bin · 0 bytes [preview unavailable]` —
  while no fixture renders a message carrying a file part, so there is no capture
  coverage of the row's layout. Changing the join in `content()` (`:769-770`) will
  therefore fail that assertion by design, and the fixture work above is what adds
  the layout half.
- **Attachment fixture — the cheap route.** `public_tool_records()` is called from
  three places (`layout_fixture.rs:533`, `:636`, `:642`), so adding a parameter
  means editing all of them. Cheaper and smaller: keep it as a wrapper
  (`fn public_tool_records() -> Vec<Value> { public_tool_records_with(None) }`) and
  put the body in `public_tool_records_with(attachment: Option<&str>)`, so only the
  new preset's call site asks for the file part and the three existing call sites
  are untouched. The conditional then belongs at `:433`, where the first user
  message's `content` array is built. Two edits plus the preset registration
  (`PRESETS` and the `preset_size` arm, as `no-session` needed) instead of seven.
- **Attachment fixture — a trap, hit and recorded.** Inserting the conditional
  *inside* the `json!` of the first user message does not compile: a macro body
  cannot hold a `let`/`if` block (`unexpected end of macro invocation` at
  `layout_fixture.rs:438`). The parts have to be built *before* the macro and
  interpolated, e.g. collect `let mut parts = vec![json!({...text...})]`, push the
  file part when requested, then pass `"content": parts` — which means moving the
  existing `vec![event(0, "user/message", json!({…}))]` wrapper around it rather
  than only editing the array in place.
- **Message attachment card and its gap — landed at all three sites, capture-verified.**
  `shots/29-attachment-card-dp2.png` is a `grim -o DP-2` grab of the floating
  `layout_smoke` window cropped to the window the grab itself shows; that run's own
  frame is `shots/29-attachment-card.png`. The fixture reported the requested
  `1040x1040` frame measured as `1040x1072` with `layoutObservation.passed`, so
  the capture is of the layout under test and not of a tiled or stale window.
  The crop is taken at the frame the pixels show, not at `hyprctl`'s client rect:
  a freshly floated window animates, `hyprctl` reports the animation's target, and
  a crop at that rect silently included ~200px of desktop while the window was
  elsewhere — the first two attempts did exactly that and looked plausible. The
  detector finds the frame's own 1040x32 SURFACE banner strip carrying the PUBLIC
  banner text, and the chosen grab agrees with the fixture's own frame at zero
  offset.
  Measured from the compositor's pixels rather than from source, in that capture
  (which aligns with the fixture's own frame at zero offset): the card's box is
  `x 745..984`, `y 137..201` — 240 wide, 64 high — the bubble's right edge (`984`)
  is the card's right edge, the gap between them is exactly `202..208` (8px), and
  the name and meta lines occupy their own 22 and 15 pixel boxes
  (`MessageItem.module.css:320-364`). The preset `tool-activity-attachment` is the
  first fixture to send a user message carrying a `file` content part; before it,
  no preset could show this gap at all.
  Four changes had to move together, as the planning bullets above predicted:
  - `layout_fixture.rs`: `public_tool_records_with(attachment: Option<&str>)` with
    `public_tool_records()` left as the wrapper, parts built before the `json!`
    macro (the recorded trap), and the preset registered in `PRESETS` and the
    `preset_size` arm. The pre-existing `tool-activity` prefix assertions apply to
    it unchanged, and its `if preset.starts_with("tool-activity")` block now also
    pins that only this preset carries cards.
  - `reducer.rs`: `user_content()` splits a user message into body text plus
    `FileCard { name, meta }` values, with `file_extension`/`file_size_text`
    mirroring `FileTypeIcon.tsx:122-126` and `file-size.ts:8-15`; `DisplayRow`
    carries them in `files`. `content()` is deliberately **unchanged**, so a tool
    result or a system message keeps its `File · …` line and the assertion the
    notes expected to fail (`reducer.rs`'s
    `durable_file_only_content_exposes_only_bounded_name_size_...`) never had to
    break: the split is scoped to the one caller the reference gives cards to.
  - `ui.rs` `row_geometry`: the card block is added after the action footer, so it
    covers both the cached measurement and the search-expanded override. A
    file-only message draws no bubble, so it reserves `ROW_CHROME` instead of the
    bubble's 20px of vertical padding plus a text body.
  - `ui.rs` `attachment_cards()` and the human branch: one fixed 240x64 card per
    file, `.userStack`'s 8px gap, right-aligned, with the row wrapper widened to the
    card so a bubble narrower than 240 cannot clip it. View and both measurements
    read `design::attachment_height`; `design.rs` and `ui.rs` each pin it against
    the drawn stack, including the file-only case.
  Three intentional, pre-existing dispositions inside the card: the reference's 500
  name weight has no DejaVu face and stays Normal; its 28px `FileTypeIcon` is drawn
  SVG, so `▤` — the glyph the tool rows already use for a file — stands in at the
  card's own icon box; and Iced has no ellipsis, so the two one-line text boxes
  clip instead of ellipsizing.
- **Conversation flow gaps — the web's 6/12 ladder landed, capture-verified.**
  `ChatView.module.css:60-95` gives every flow sibling `margin-top: 6px`, an
  Assistant response row (and the row after it, the live `.running` status
  included) `12px`, and a `turn-process` header `16px`, where native put a
  symmetric 6px padding on every row and so sat two process rows 12px apart.
  `design::FLOW_GAP` and `design::RESPONSE_GAP` hold the values,
  `ui.rs::flow_gap_before` is the single owner of the ladder, and both readers use
  it: `row_geometries` reserves `gap − ROW_CHROME` on each row's height and the view
  draws the same value as that row's top padding, so a reserved height cannot
  disagree with the drawn one. The scroller takes the web's own
  `design::FLOW_INSET` — its `.scroll` padding is `16px …`
  (`ChatView.module.css:26-29`) — which is now where the first row's and the last
  row's clearance lives.
  `shots/30-flow-gap-dp2.png` is the real `grim -o DP-2` grab and
  `shots/30-flow-gap.png` that run's own frame (both align at zero offset; the
  fixture reported `1040x1072` measured with `layoutObservation.passed`). At the
  `tool-activity-attachment` preset, consecutive tool-row text tops measure 30px
  apart (24px line box + 6px gap) where the same measurement on the previous
  capture read 36px (24 + 12); the tool-row-to-assistant distance is unchanged at
  50px, so only the process rhythm moved; and the first row's attachment card
  starts 10px lower (fill top `138` → `148`) because its own 6px top padding became
  the scroller's 16px inset.
  A `turn-process` row would take 16px, but Chat renders none — they are
  Trajectory-only — so no native row maps to it; the constant says so at its
  definition. `ui.rs`'s `conversation_flow_gaps_follow_the_web_ladder` pins the
  ladder and the height adjustment, and the answerless-row test in
  `transcript_layout_tests.rs` now reads the same ladder rather than asserting the
  old symmetric padding.
- **Sidebar column and row metrics — landed, capture- and rect-verified.**
  The web's sidebar is one set of literal boxes: `.root { padding: 6px
  var(--dsh-sidebar-inline-padding) }` with the inline padding at `12px`
  (`SidebarRoot.module.css:8-10`), `.newSession { height: 38px; padding: 8px 12px;
  margin: 0 2px 12px; border: 0.5px solid var(--dsw-alias-border-l3);
  border-radius: var(--dsw-radius-md) }` with its icon and label centered
  (`:397-409, :470`), `.panelRow { min-height: 36px; padding: 7px 8px;
  border-radius: var(--dsw-radius-md) }` (`:493-505`), and a session cell of
  `height: 32px; padding: 0 8px` whose title is `14px/20px`, clipped with an
  ellipsis (`Rows.module.css:2-10, 110-118, 162-166`). Native had `[18, 14]`
  container padding, an `11/10`-padded New conversation row with no box, `8/10`
  and `10/10` panel rows, and `11/10` session rows with a 13px wrapping title.
  `design.rs` now carries those metrics as `SIDEBAR_PAD`, `NEW_SESSION_*`,
  `PANEL_ROW_*`, `SESSION_ROW_*` and `ROW_ACTION_HEIGHT`, and the view reads the
  same constants it reserves, so no measurement can drift from the render.
  Two pieces were more than padding:
  - The New Session bar is a real box now. `design::elevated` fills it with
    `OVERLAY` — native's elevation rung above the sidebar's `SURFACE`, which is
    where the web's `--dsw-alias-button-elevated-fill` sits — and draws a `0.5px`
    `LINE` hairline for `--dsw-alias-border-l3`; the ladder rounds l2 and l3 to the
    same hairline, and hover darkens to `BASE` in the direction
    `--dsw-alias-button-floating-hover` takes. Its content is centered and inset
    2px, as the web's `margin: 0 2px` does.
  - `design::navigation_row` is `navigation` at `RADIUS_MD`: the web's `.panelRow`,
    `.sessionRow` and `.projectRow` are all `radius-md`, where native's shared row
    surface rounds to `RADIUS_SM`. It is a separate style so the settings-category
    and plugin rows keep their own radius until their web counterparts are read.
  The session title is one clipped 20px line rather than a wrapped paragraph:
  the web never wraps a session title, and a wrapped one made the cell taller than
  the web's 32px. Iced has no ellipsis, so the title clips (the same disposition
  the file card records) instead of ending in `…`.
  Verified from two independent sources. The fixture's own `layoutObservation`
  records `native-settings-entry`: its rect was `x 14, width 232, height 35.6`
  before and is `x 12, width 236, height 36.0` after — the 12px column inset and
  the 36px panel-row height, both exact. The DP-2 grab
  (`shots/31-sidebar-metrics-dp2.png`, that run's frame `shots/31-sidebar-metrics.png`)
  measures the New conversation bar's hairline box at `y 76..113` = 38px, the
  selected session cell at `y 202..234` = 32px, and consecutive session-title text
  tops at a 32px pitch, so the rows are stacked on the web's cell with no gap.
  Not changed here, and still native's own composition: the sidebar's order
  (sessions, then the workspace seat, then Settings) differs from the web's
  brand → New Session → destination panel list → browser region → foot, and the
  brand row has no macOS titlebar strip, so its own height is unchanged.
- **Sidebar search and browser section header — landed, capture-verified.**
  The region above the session list has two literal boxes in the web: the
  expanded search is `height: 30px` with a `0.5px solid
  var(--dsw-alias-border-l4)` edge at `--dsw-radius-md` over a transparent fill,
  its input `13px/18px` (`WorkspaceBrowser.module.css:160-171, 214-222`), and
  `.sectionHeader` is `height: 36px` with `gap: 4px` and a 4px trailing margin
  (`:53-62`), its trailing controls `28px` squares (`:17-30`). Native had a
  `design::field` search (SURFACE fill, 1px `OVERLAY` border, radius 10,
  `12px` text, ~37px tall) and a bare header row with no height or spacing.
  `design::search_field` and the `SEARCH_*`/`SECTION_HEADER_*` constants now carry
  those values, and the header row takes the box, the 4px gaps and the 28px
  control height.
  **The first capture of this change caught a real bug**, which is why the box is
  drawn and measured rather than assumed: `TextInput::line_height(f32)` converts
  through `impl From<f32> for LineHeight`, which is `Relative`, so `line_height(18.0)`
  asked for an eighteen-line box and the field filled the region (`y 126..320+`,
  the placeholder centered in it). `iced::Pixels(SEARCH_LINE)` is the absolute
  18px box the web writes; with it the field's hairline box measures `y 127..156`
  = exactly 30px, the header sits `160..196` = 36px between its 4px gaps, and the
  session titles below keep a 32px pitch (`211, 243, 275`), so the previous
  item's cell is untouched. The fixture's `native-settings-entry` rect is also
  unchanged at `x 12 / width 236 / height 36.0`.
  Deliberately kept: native's `RECENT`/`ARCHIVED`/`TRASH` label and its
  `View archived`/`Trash` text stand-ins for the web's single `Sessions` label and
  28px icon buttons, so the label and those controls keep native's hint tier while
  their box takes the web's size. Native also keeps the search field mounted where
  the web expands it from a 28px icon control, and its "N conversations" footer is
  native's own line (the web has no such element).
- **Composer card surface and insets — landed, capture-verified.** The web's
  composer is the elevated input surface: `.card` fills with
  `--dsw-specific-input-major` — the same bluish-850 the user bubble and the
  attachment card use — draws the `0.5px` l2 elevation hairline, and pads only
  its top by `8px` with a `12px` gap before the control row
  (`InputBar.module.css:60-100`, `gradient-shadow-text.css:33-40`); `.input` is
  `min-height: 36px` with `padding: 4px 8px 0 14px` (`:169-176`), and `.row` is
  `padding: 2px 8px 6px` with a `12px` gap (`:251-260`). Native had a `SURFACE`
  card with `design::card`'s 1px same-as-fill stroke, a uniform 10px inset, `22px`
  of text inset inside the editor, a control row with a 7px gap and no padding,
  and a resting editor of three lines.
  `design::file_card` became `design::elevated_surface` (one owner for the token
  both surfaces share), `COMPOSER_CARD_*`, `COMPOSER_INPUT_*` and
  `COMPOSER_ROW_*` carry the figures, and `composer_editor_height` reads them:
  `rows × line + COMPOSER_INPUT_CHROME` floored at `COMPOSER_INPUT_MIN`, so one
  line is exactly the web's 36px and the fourteen-line text cap stays 336px. The
  short-window 8-row cap remains native's own adaptation.
  Verified on DP-2 (`shots/33-composer-surface-dp2.png`; that run's frame is
  `shots/33-composer-surface.png`, matching at a mean absolute difference of 5.8):
  the card's fill reads the compositor's OVERLAY (`(41,40,56)`) where it read
  SURFACE (`(32,30,46)`) before; its box measures `946..1042` = **97px**, against
  the web's own arithmetic `8 + 36 + 12 + 42 = 98` and 153px before; and the
  draft's first text row sits **17px** below the card's top where it sat 27px,
  which is the web's 8px card pad + 4px input pad + the glyph's leading inside the
  24px line box.
  Not changed here, and next: the control row's own order and control boxes (the
  web puts attach on the left and the model chip with it on the right, inside
  28px/34px circular controls), and the composer's outer padding (`InputBar`
  `.root` is `0 16px 4px` with a 4px dock inset, where native uses `[10, 20]`).
- **Composer trailing controls — landed, rect-verified.** The web's control row
  is `justify-content: space-between`: the left `.tools` group holds its own
  command circle and the mode selects, and the right `.trailing` group holds the
  model select beside the send control (`InputBar.tsx:419-486`). The model select
  is a `.select` chip — `height: 28px`, `padding: 0 20px 0 8px` for its 12px
  chevron, `--dsw-radius-sm`, transparent at rest with an
  `interactive-bg-hover` hover, `13px/20px` secondary text
  (`InputBar.module.css:329-352`) — and the send and stop controls are both
  `34px` boxes (`:369-395`). Native had the model chip on the *left* as a 20px
  radius OVERLAY pill with a `◇` glyph at 12px and `6/10` padding, and a row
  whose send (32.9px) and stop (31.6px) boxes were whatever their text produced.
  `COMPOSER_CHIP_*` and `COMPOSER_CONTROL_HEIGHT` now carry the figures, the chip
  is `design::ghost` (transparent at rest, `radius-sm`, hover fill) with a 12px
  chevron box and no `◇`, and the row reads attach · fill · chip · stop · send.
  Verified from the fixture's own rects for the recorded control ids, before →
  after: `native-model-controls` `27.6 h, x 318.0` → **`28.0 h, x 708.3`** (the
  web's chip height, moved into the trailing group), `native-file-open` `26.3 h,
  x 764.5` → **`28.0 h, x 318.0`** (the chip height, moved to the leading group),
  and `native-stop-control` `31.6 → 34.0`, `native-send-control` `32.9 → 34.0`.
  `shots/34-composer-trailing-dp2.png` (that run's frame
  `shots/34-composer-trailing.png`, matching at 5.5 mean absolute difference)
  shows the row's arrangement.
  Deliberately kept: native's `+ Attach` control, because the web's leading
  circle is its *command* menu (`InputBar.tsx:419-431`) and native's attach
  affordance has no counterpart box there — it keeps its label and takes the
  chip height; and the words on Send and Stop with native's own fills
  (`design::primary`, `design::danger`), because drawn glyphs are blocked and
  the words are the native affordance. The web's 34px *circles* are therefore
  34px *pills* here.
- **Composer seat and transcript inset — landed, capture- and rect-verified.**
  The web's composer sits in a seat of its own: `.root { padding: 0
  var(--dsh-composer-side-clearance) 4px }` with the clearance at `16px`, its
  dock strip `4px` under the card (`InputBar.module.css:9-20`), and the card
  capped at `calc(var(--dsh-chat-content-width) + 32px)`
  (`ConversationRoot.module.css:381-390`). The transcript inside the scroller is
  inset `calc(var(--dsh-composer-side-clearance) + 16px)` = 32px per side
  (`ChatView.module.css:26-29`). Native used a 20px inset for both, a `[10, 20]`
  seat, a 5px dock gap, and capped the composer at the same lane as the
  transcript — so the card overlapped the transcript exactly instead of
  overhanging it.
  `design::TRANSCRIPT_INSET` (32), `COMPOSER_SEAT_PAD` (0/16/4/16),
  `COMPOSER_DOCK_GAP` (4) and `COMPOSER_CARD_OVERHANG` (32) carry the figures,
  and `chat_lane` was split from a new `chat_cap`: the lane is
  `min(column − 64, chat_cap)`, the cap is the web's `--dsh-chat-content-width`
  (adaptive clamp or stored preference), and the composer's shell caps at
  `chat_cap + 32` inside the seat's 16px padding, so the card is drawn as
  `min(column − 32, cap + 32)` — the web's own expression.
  `layout_ui_tests::conversation_column_is_adaptive_and_fits_the_padded_pane` now
  pins both: at 760/1200/1980/2560 wide the lane is `min(pane − 64, cap)` and the
  drawn card is exactly `COMPOSER_CARD_OVERHANG` wider than the lane.
  Measured at the fixture's 1040-wide window, where the 680px cap binds and so the
  transcript must not move: the composer card's fill spans `x 310..987` = 678
  before and **`x 294..1003` = 710** after (the cap and cap + 32), while the
  transcript's own rows are pixel-identical (the attachment card `746..983` and
  the user bubble `601..984` in both grabs). The fixture's control rects move with
  the card's edges: `native-file-open` `318.0 → 302.0`, and
  `native-model-controls` `708.3 → 724.3`, `native-stop-control` `843.0 → 859.0`,
  `native-send-control` `906.2 → 922.2` — sixteen pixels each way.
  `shots/35-composer-seat-dp2.png` is the grab and `shots/35-composer-seat.png`
  that run's frame (matching at 4.6 mean absolute difference).
  One native remainder: the `[10, 20]` seat's 10px top gap is gone because the
  web's composer has no top padding — the transcript's own 16px scroller inset
  provides the separation, which is how the web's numbers add up.
- **Conversation header rows and tab strip — landed, capture-verified.** The web
  builds its 76px band out of rows: a `min-height: 30px` title row
  (`ConversationRoot.module.css:72-79`), a tab strip `10px` below it with a
  `36px` gap and an `8px` leading inset (`:193-201`), and per tab a `13px/16px`
  label with *no* horizontal padding, a `9px` under-pad and a `2px` bar that ends
  flush with the header's rule (`:203-231`). Native had a 32px title row (the
  `More ▾` control's own `[8, 10]` padding), a `[2, 0]`-padded row of 12px pill
  tabs with a 6px gap, an OVERLAY pill as the active state, and no bar.
  `HEADER_TITLE_ROW` (30), `HEADER_TABS_GAP` (10), `HEADER_TABS_INSET` (8),
  `TAB_GAP` (36), `TAB_LINE` (16), `TAB_PAD_BOTTOM` (9) and `TAB_BAR` (2) carry
  the figures, `design::bar` draws the strip, and each tab is a `Stack` whose
  base layer is the label column — a `Fill`-width bar inside a `Stack` resolves
  against the base's size, which is how the bar spans exactly the tab's text
  width without measuring it. **The first capture caught the alternative**: a
  plain `column![label, pad, bar]` let the `Fill` bar inflate the tab, and the
  two tabs split the strip and rendered centred and far apart.
  Verified from the fixture's own `native-header` rect and the grab: the band is
  **77.0** tall against 71.2 before (the web's rows sum to 75 plus the 2px bar,
  which the web overlaps onto its 0.5px rule; native's ends flush at the seam
  instead, hence 77 against 76), and the active tab's bar measures exactly 2px
  (`y 104..105`) spanning 30px — the `Chat` label's own width — with the seam at
  `y 106`. `shots/36-header-tabs-dp2.png` is the grab and
  `shots/36-header-tabs.png` that run's frame (matching at 4.9 mean absolute
  difference). Native keeps `design::ghost`'s hover surface on the tabs, which
  the web's plain transparent tab does not draw.
- **Edit diff stat — landed, capture-verified, and the table row corrected.** The
  table above used to say the web reads a *parsed-result* diff where native counts
  call-argument lines. That was wrong: `ui-tool/.../models/diff-card-model.ts`
  derives the hunks from the *call arguments* for `edit`/`write`/
  `str_replace_editor`, and only falls back to opaque result metadata
  (`narrowDiffs`) when the call itself does not parse. `ToolRow.tsx:185` then
  totals those hunks with `diffTotals`, so native's `activity::edit_totals`
  lineage already matches the reference's primary path. What actually differed
  was the rendering and the cue rules:
  - `ToolRow.module.css:64-77`: the stat sits `margin-left: 10px`, in the code
    font at `secondary − 2` = 11px, and the two counts are `+N` and an ASCII
    `-M` separated by a literal space. Native drew them at 10px in the body font
    with a `−` (U+2212) and a 3px gap, at the row's trailing edge.
  - The `+`/`-` pair takes `--dsw-alias-state-success-primary` /
    `--dsw-alias-state-error-primary` **only while the row is hovered or
    expanded**; at rest they stay muted. Native coloured them unconditionally.
  - `ToolRow.tsx:186-190`: `settledWithCue = state === 'error' || 'stopped'`
    drops the stat entirely for a failed call.
  `design::DIFF_STAT_LEFT` (10) and `DIFF_STAT_SIZE` (11) carry the figures, the
  stat uses the code font with an ASCII hyphen, and native's existing
  hover/expanded predicates (`activity_hovered`, `activity_is_expanded`) drive the
  cue colours while a failed row drops the stat.
  Coverage came first for the third rule: `layout_fixture` gained a
  `tool-activity-edit-error` preset (`public_tool_edit_error_records`, a
  post-processing rewrite of the shared records in the same shape
  `public_tool_call_records` already uses) that records the Edit result as an
  error, so the failed branch renders in a fixture instead of only in theory.
  Verified from three DP-2 grabs of the same preset: at rest the Edit rows' ink
  spans `x 948..976` in MUTED only; with `--hover-tool-row` the same span carries
  FOAM (`x 907..959`) and DANGER (`x 924..975`) pixels; and in the failed-edit
  preset the failed Edit row drops the stat and shows its recorded-error suffix
  (`x 903..976`, DANGER) while the call row above keeps its totals.
  `shots/37-diff-stat-dp2.png`, `shots/37-diff-stat-hover-dp2.png` and
  `shots/37-diff-stat-error-dp2.png` are those grabs
  (`shots/37-diff-stat.png` is the first run's own frame).
- **Empty-state hero — landed, capture-verified.** The reference's empty state is
  not a centered body above a docked composer: `ConversationContent.tsx:164-171`
  renders the `HeroShell` and the input bar inside one `composerStack`, and
  `HeroShell.module.css:5-21` centers that stack (`align-items: center`,
  `justify-content: center`, `height: 100%`) with a `12px` gap. The composer's own
  `.hero` variant raises the draft floor to `52px` — two 24px lines and the 4px
  top pad — instead of the docked 36px, and the hero keeps no bottom pad on the
  seat (`InputBar.module.css:26-30, 232-236`). Native centered a *separate*
  welcome scrollable and docked the composer under it, always on the 36px floor.
  `HERO_GAP`, `HERO_INPUT_MIN` and `COMPOSER_HERO_SEAT_PAD` carry the figures, a
  new `hero_active()` decides the state (the transcript is the active view and has
  no row to render, a live partial included), `welcome()` split into the hero
  content plus a centered wrapper for the no-composer case, and `hero_pane()`
  stacks the hero and the composer centred in the pane.
  Verified on DP-2 at the `wide-welcome` preset
  (`shots/38-hero-pane-dp2.png`, that run's own frame `shots/38-hero-pane.png`,
  matching at 3.0 mean absolute difference): the composer card's box moves from
  `y 551..703` — flush with the bottom of the 732px window — to `y 454..567`,
  directly under the hero title at `y 343..365`, and it grows from 97px docked to
  **112px**, which is the 52px hero floor in place of the 36px docked one. The
  fixture's own frame also fixed a capture-tooling bug found here: `dsh-pick.py`
  hard-coded the tool-activity frame's 1040x1072, so the 1040x732 welcome preset
  crashed the picker; it now takes the window size from the fixture's own frame.
- **Hero headline and composer copy — landed, capture-verified.** The reference's
  hero headline is the mark beside the title in one centered row with
  `column-gap: 10px`, the title at `26/32` weight 500 (figma 34:10411, per
  `HeroShell.module.css:33-60`), and its strings come from the locale sheet:
  `hero.headline` = "Into the Unknown" (`locales.ts:453`), with the composer
  placeholder state-owned — `placeholder.hero` = "Describe what you want to
  build, / commands, @ files or sessions" in the hero and `placeholder.default` =
  "Message or run a task, / commands, @ files or sessions" docked (`:20, 396-399`).
  Native drew a 30px (25 compact) headline over a second 14px hint line and used
  one placeholder, "Message DeepSeek Harness… or drop a file", in every state.
  `HERO_HEADLINE_GAP` (10), `HERO_TITLE_SIZE` (26) and `HERO_TITLE_LINE` (32) carry
  the figures; the headline is a single row and the hero-only placeholder is
  chosen by the `hero_active()` state.
  Verified on DP-2: the headline's ink band shrinks from `y 343..365` (23 rows at
  30px) to `y 336..355` (20 rows at 26px), the 14px hint band at `y 398..406` is
  gone, and the composer card — unchanged at 112..113px tall — re-centers from
  `y 455..566` to `y 408..520`. The docked grab reads "Message or run a task, /
  commands, @ files or sessions" and the hero grab reads "Describe what you want
  to build, / commands, @ files or sessions", with "Into the Unknown" beside the
  mark. `shots/39-hero-copy-dp2.png` (own frame `shots/39-hero-copy.png`) and
  `shots/39-docked-copy-dp2.png` are those grabs.
  Kept deliberately: a session-less native window has no reference counterpart
  (the web reaches the hero only with a session), so it keeps "A quiet space for
  your work" plus the guidance its blocked composer needs; the hero's "Preview"
  badge (`hero.preview`, `:454`) is not drawn; and the hero's workspace-chip row
  (`ConversationContent.tsx:112-133`, `.heroWorkspaceRow` with `margin-top: 4px`
  and `padding: 0 16px 0 20px`) has no native counterpart yet — native's workspace
  seat lives in the sidebar.
- **Sidebar rail — landed, rect- and pixel-verified.** The reference's rail spec is
  explicit in `SidebarRoot.module.css:34-37`: "36x36 control boxes centered in the
  56px rail (10px side padding), 12px vertical rhythm, 18px from the rail top to
  the whale's box". Native's rail was 68 wide with `padding: 18px 8px`, 8px between
  controls and text buttons whose boxes were whatever their padding produced.
  `design::RAIL` (56), `RAIL_PAD` (18/10/6/10), `RAIL_CONTROL` (36) and
  `RAIL_RHYTHM` (12) carry the figures, and the rail's controls take those boxes.
  Two labels cannot fit a 36px box — "+ New" wrapped and "Settings" clipped,
  because a single word clips rather than wraps — so the rail now draws the plus
  and the gear its reference icon buttons draw (`+` U+002B and `⚙` U+2699, both
  verified present in the shipped DejaVu Sans), with "Chats" still a word and the
  settings accessibility node unchanged.
  Verified from the fixture's own rects at the `tool-activity-narrow` preset (a
  1000px-breakpoint compact window, so the rail renders) and the grab:
  `native-settings-entry` **36.0 × 36.0 at x 10.0** (was a padded text button),
  `native-header` x **56.0** (the rail's own width; was 68), and in the grab the
  brand's filled box measures **rows 49..84 = 36 tall, cols 9..44 = 36 wide** —
  the banner's 32px plus the 18px rail seat, at the 10px side padding.
  `shots/40-sidebar-rail-dp2.png` is the grab and `shots/40-sidebar-rail.png` that
  run's own frame (matching at 5.5 mean absolute difference).
  Note for future captures: `tiny-welcome` renders at scale 1.25, so its own frame
  (950×750) cannot be diffed against a 1.0-scale grab; `tool-activity-narrow` is
  the scale-1.0 compact preset. The banner detector's 0.90 coverage floor also only
  held at a 1040px window (the pink title is fixed-width), so it now floors at
  0.75 with the title check carrying the identification.
- **Width handles — landed, rect- and pixel-verified.** The reference positions
  each handle absolutely: a strip `width: min(10px, …)` with its inner edge
  `24px` beyond the content column (`ConversationRoot.module.css:259-281`), and a
  `.widthHandle::after` glow of `2px` held `4px` in from that inner edge, whose
  gradient window is a `16px` solid core fading to transparent by `36px` either
  way, `opacity: 0` at rest and `1` on hover or while dragging, centred on the
  pointer's Y with a `50%` fallback (`:289-323`). Native had the strip's own
  geometry right — `WIDTH_HANDLE_WIDTH` 10 and `WIDTH_HANDLE_OFFSET` 24 — but drew
  no affordance at all.
  `design::HANDLE_GLOW_WIDTH` (2), `HANDLE_GLOW_INSET` (4) and `HANDLE_GLOW_CORE`
  (16) carry the figures, `Message::WidthHandleHover` with `MouseArea::on_enter` /
  `on_exit` lights the hovered side, the drag lights its own side, and each strip
  now carries a recorded id so the fixture reports its box.
  Verified at `tool-activity-attachment` (1040 wide): the fixture's rects put the
  strips at `x 276.0` and `x 1014.0`, both 10.0 wide — the lane's 310..990 edges
  minus and plus the 24px offset, exactly the reference's placement — and with
  `--width-handle-glow true` the grab carries a 2px bar in each strip (`x 279..280`
  and `1017..1018`) spanning 17 rows around the strip's vertical centre, which is
  the 16px core at the reference's 50% fallback; without the flag those columns
  hold no ink, matching the reference's resting `opacity: 0`.
  `shots/41-width-handle-glow-dp2.png` is the grab and
  `shots/41-width-handle-glow.png` that run's own frame.
  Iced has no gradient, so native draws the solid core and leaves the 36px fade
  out, and it follows the reference's own 50% fallback rather than publishing the
  pointer's Y every frame, so a drag's glow does not travel with the cursor. The
  hero state draws no handles: the reference renders width controls only inside
  the active phase, and its empty state has none.
- **Composer plan chip — landed, rect- and capture-verified.** The reference puts
  its plan-mode control in the control row's leading `.modes` group beside the
  attach control (`InputBar.tsx:442-450`) as a chip of `height: 28px`,
  `padding: 0 8px`, `gap: 4px`, `radius-sm`, a 14px glyph slot, a `13px/20px` label
  in the business-primary colour over the business-tertiary fill, a 6%-mixed fill
  on hover and `opacity: 0.6` while disabled or leaving
  (`PlanModeControl.module.css:4-40`). Native drew "Plan mode" as a 10px text
  button **below the card** in the metadata strip, with no chip geometry.
  `PLAN_CHIP_PAD`, `PLAN_CHIP_GAP`, `PLAN_CHIP_GLYPH`, the tertiary/primary pair
  (`PLAN_CHIP_FILL`, `PLAN_CHIP_HOVER`, `PLAN_CHIP_FILL_DIM`) and a
  `native-plan-control` id carry the figures; the chip rides the leading group, and
  the strip keeps only the in-flight line so a pending toggle stays visible.
  Verified at the `plan-mode` preset (760x560): the fixture's
  `native-plan-control` rect is **28.0 tall at `x 158.4`** — the row's 12px gap
  after `native-file-open` (80 + 66.4) — with the model chip and send control
  unchanged in the trailing group, and the grab shows the accent-tinted `☑ Plan`
  chip in the row with the standing `Plan · 1/2 · next: …` line below.
  `shots/42-plan-chip-dp2.png` (own frame `shots/42-plan-chip.png`) is that grab.
  The pending branch is fixture-covered too: a `--plan-pending` flag now holds a
  toggle in flight, and `shots/42-plan-chip-pending-dp2.png` shows the dimmed chip
  with the strip's `Plan mode` line.
  Remainders: the web's chip swaps a plan outline for a circled close while
  hovered, which native does not (its hover is the fill mix), and the reference's
  sibling *permission* select has no native counterpart at all.
- **Jump-to-latest control — landed, rect- and capture-verified.** The reference
  renders a `.toBottomSlot` 16px above the composer, right-aligned with
  `padding-right: max(clearance + 16px, (100% − content)/2)`, holding a `.toBottom`
  button of `34×34`, `border-radius: 100px`, the floating fill, the panel elevation
  shadow and the primary label ink, hovered one rung above that fill, and shown
  only while the transcript is *not* following its tail
  (`ChatView.module.css:223-273`, `ChatView.tsx:288-292`). Native had the state —
  `follow_bottom`, set from the viewport's relative offset — and the snap-to-end
  operation, but no control to return to the tail after scrolling up.
  `TO_BOTTOM_SIZE` (34), `TO_BOTTOM_INSET` (16) and `TO_BOTTOM_EDGE` (32) carry the
  figures, `Message::ScrollToBottom` re-arms `follow_bottom` and reuses
  `position_transcript`'s existing snap, and the control rides the transcript's
  overlay stack with a `native-to-bottom` id.
  Verified at `chat-transcript` (1040 wide): with `--scrolled-away true` the
  fixture's rect is **34.0 × 34.0 at `x 956.0`** — the content column's right edge
  is 990, so 990 − 34 = 956 exactly, which is `max(32, (pane − lane)/2) = 50` in
  from the pane — and the grab shows the round `↓` button 16px above the composer
  card; without the flag the same preset reports **no `native-to-bottom` rect**,
  matching the reference's following-tail rule. `shots/43-to-bottom-dp2.png` is the
  grab and `shots/43-to-bottom.png` that run's own frame.
  Native draws no shadow (its documented palette-only elevation stance), and its
  control follows the transcript's own box rather than the reference's sticky
  composer-height offset, which is the same 16px once the composer is on screen.
- **Header breadcrumb trail — landed, rect- and capture-verified.** The
  reference's header title is a breadcrumb trail: `ConversationSession.tsx:25-43`
  walks `parentId` while a session's origin is `subagent`, and `:73-104` renders
  each crumb in one `4px`-gapped row — ancestors as links that select that session,
  the current one as plain text, with a `14px/20px` caption `/` before every crumb
  but the first. The crumb box is `padding: 4px 8px`, `radius-md`,
  `max-width: 220px`, `14px/20px` tertiary text (primary for the current session),
  dropping to `12px/18px` for subagent crumbs
  (`ConversationRoot.module.css:102-146`). Native drew the title as an **18px**
  plain label truncated by character count, with no box and no ancestry.
  `CRUMB_GAP`, `CRUMB_PAD`, `CRUMB_MAX`, `CRUMB_SIZE`/`CRUMB_LINE` and
  `CRUMB_SUBAGENT_SIZE`/`CRUMB_SUBAGENT_LINE` carry the figures, `header_crumbs`
  builds the trail from the summaries the app already holds, and the row carries a
  `native-header-crumbs` id whose own box is the trail's intrinsic width.
  Verified on DP-2 at two presets: a new `subagent-trail` fixture (parent plus a
  `parentSessionId`/`origin: subagent` child, its metadata in the same
  `SessionSummary` DTO the app already deserializes) reports a crumbs box of
  **375.1 × 28.0**, and the single-crumb `conversation` preset **198.0 × 28.0** —
  both **28.0 tall**, which is the crumb's `4 + 20 + 4`, and the 177px difference is
  the ancestor crumb plus its separator. The grabs show
  `PUBLIC parent conversation / PUBLIC subagent child` with the ancestor in the
  tertiary colour at 14px and the current crumb smaller, and the single crumb
  alone. `shots/44-header-crumbs-dp2.png` (own frame `shots/44-header-crumbs.png`)
  and `shots/44-header-crumb-single-dp2.png` are those grabs.
  Remainders: native's palette has no `label-caption` tier, so the separator reuses
  `MUTED`, and Iced has no text ellipsis, so a long crumb is still shortened by
  character count before its 220px box clips it.
- **Hero workspace chip — landed, rect- and capture-verified.** The reference seats
  a workspace chip row between the hero headline and the composer card: the row is
  `display: flex` with a `4px` top margin and `padding: 0 16px 0 20px` inside the
  composer-card width (`ConversationRoot.module.css:520-529`), and the chip is a
  `min-height: 28px` transparent control — `padding: 0 8px`, `gap: 4px`,
  `radius-sm`, `13px/20px` primary text, a 16px folder glyph, the workspace title
  or the `Choose workspace` string, and a 12px chevron — that opens the workspace
  picker with `interactive-bg-hover` on hover (`HeroShell.module.css:152-186`,
  `EmptyHero.tsx:38-62`). Native's hero had no workspace seat at all.
  `HERO_CHIP_PAD`, `HERO_CHIP_GAP`, `HERO_CHIP_GLYPH`, `HERO_CHIP_CHEVRON`,
  `HERO_ROW_TOP` and `HERO_ROW_PAD` carry the figures, `hero_workspace_row` places
  the row at the card's width between the strip and the composer, and the chip
  carries a `native-hero-workspace` id.
  Verified on DP-2 at two hero presets: the chip's rect is **28.0 tall at `x 314.0`**
  in both — the composer card's left edge is 294, so 294 + 20 is exactly the
  reference's row inset — at 173.7 wide unlabelled and 172.2 wide labelled, and the
  grabs show `⌂ Choose workspace ⌄` and `⌂ PUBLIC workspace ⌄` above the card.
  `shots/45-hero-workspace-chip-dp2.png` (own frame `shots/45-hero-workspace-chip.png`)
  and `shots/45-hero-workspace-selected-dp2.png` are those grabs. The labelled case
  needed coverage, so the fixture gained a `hero-workspace-selected` preset that
  selects the fixture workspace while staying in the hero.
  Remainders: the chip opens native's sidebar workspace list rather than the
  reference's anchored menu, and DejaVu carries no folder outline, so the glyph is
  the house mark it does carry (the reference also swaps a closed folder for an open
  one when a workspace is chosen, which native does not).
- **Load-earlier control — landed, rect- and capture-verified.** The reference
  centers its history control at the top of the transcript column —
  `.older { display: flex; justify-content: center }` around a button with
  `border-radius: radius-sm`, `padding: 4px 12px`, `12px` secondary text on the
  `interactive-bg-hover-solid` fill, `opacity: 0.6` while disabled, and the label
  switching to the loading string (`ChatView.module.css:201-218`,
  `ChatView.tsx:249-256`, locale `chat.loadOlder` = "Load earlier"). Native had the
  action only in its own top control bar, as an 11px ghost text button beside
  Reload/Find/Latest.
  `design::fill_solid` carries the filled-at-rest style and the control now sits
  inside the lane, centered, with a `native-load-older` id; the duplicate left the
  top bar, which keeps its own Reload/Find/Latest affordances.
  Verified at a new `history-more` preset — the fixture's snapshot always wrote
  `hasMore: false`, so the control had no coverage at all: the rect is
  **23.6 tall × 95.3 at `x 602.4`**, which is the reference's `4 + 12×1.3 + 4` box
  centered in the 310..990 lane (650 − 95.3/2 = 602.4 exactly), and the grabs show
  the filled `Load earlier` button and, with `--page-pending true`, its dimmed
  `Loading…` form. `shots/46-load-earlier-dp2.png` (own frame
  `shots/46-load-earlier.png`) and `shots/46-load-earlier-pending-dp2.png` are
  those grabs.
  Remainder: the reference's zoomed-out `.hint` line for an opening transcript
  (13/18 tertiary) has no native counterpart, because native has no partial-open
  state to show it in.
- **Composer attachment rail — landed, rect- and capture-verified.** The reference
  lays staged attachments in a horizontal, `10px`-gapped row of `64px`-tall items
  inside `padding: 2px 10px 0` with a `-6px` bottom margin, at the card's first
  child above the draft (`ComposerAttachments.module.css:1-8`,
  `AttachmentRail.module.css:8-30`), each file a `240×64` card — a 28px glyph,
  a `14px/22px` name and a `12px/15px` meta line, `0 12px` padding, `gap: 10px`,
  `radius-lg` and a 0.5px hairline (`FileCard.module.css:1-40, 60-75`) — with an
  `18px` round remove control 6px from its top-right corner, revealed on hover in
  the reference (`FileCard.module.css:77-96`). Native rendered a **vertical column
  of 12px text rows** ("File · name · bytes" + a Remove button) *below* the draft,
  in its own file-controls block.
  `ATTACH_RAIL_PAD`, `ATTACH_RAIL_GAP`, `ATTACH_RAIL_ITEM_HEIGHT`,
  `ATTACH_RAIL_BOTTOM_PULL`, `ATTACH_RAIL_REMOVE_SIZE` and
  `ATTACH_RAIL_REMOVE_INSET` carry the figures, `reducer::attachment_meta` shares
  the reference's extension-and-size line with a message's cards, and `composer_rail`
  draws the row as the card's first child while native's own path editor, consent
  line, preview and notice stay below it.
  Verified at the `file-ready-two` preset (two staged files): the rail's rect is
  **66.0 × 510.0** — 64 + the 2px top pad tall, and `10 + 240 + 10 + 240 + 10` wide,
  which is the reference's arithmetic exactly — and the grab shows both cards with
  `BIN 4.9KB` / `TXT 2.0KB` metas and their round remove controls above the draft.
  `shots/47-composer-rail-dp2.png` is the grab and `shots/47-composer-rail.png` that
  run's own frame. The empty-state container this change left behind also had to
  stop emitting a zero-height box: the fixture records that container's rect, and a
  zero-height control fails its containment check, which the first capture caught.
  Remainders: the remove control is always visible (the reference reveals it on card
  hover, and Iced would need per-card hover state), the reference's 64×64 image
  thumbnail has no native counterpart because native's rail holds files, and the
  rail's edge arrows for horizontal overflow are not drawn.
- **User bubble box and cap — landed, capture-verified.** The reference's user
  bubble is `padding: 10px 16px` with `border-radius: var(--dsw-radius-xl)` inside a
  `.userStack` capped at `min(calc(var(--dsh-chat-content-width, 748px) * 0.702),
  82%)` — the figma 525px cap held as the 525/748 share of the *live* width axis,
  where the 82% half never binds (`MessageItem.module.css:10-40`). Native drew the
  bubble with `padding: [10, 14]`, `RADIUS_LG` and a **fixed** `min(lane − 8, 640)`
  cap, so on any column narrower than 920 its bubble stayed 640 wide.
  `USER_BUBBLE_PAD`, `USER_BUBBLE_INSET` (the 32px both sides the geometry
  reserves), `USER_BUBBLE_RADIUS` (`RADIUS_XL`), `USER_BUBBLE_SHARE` and
  `USER_STACK_SHARE` carry the figures; the measurement bound and the drawn box both
  read them, `MIN_LANE` rose to 47 (0.702 of the lane must cover the inset plus a
  pixel of body), and the human branch measures with the bubble's own inset instead
  of the assistant body's 28.
  Verified at a new `long-user-message` preset — the shared fixture's only user text
  is 22 characters, so nothing reached a cap before this: at the 1040-wide window the
  bubble's fill spans **465px**, inside the lane's 477.4 cap, where the previous
  fixed cap drew it ~640 (175px wider). `shots/48-user-bubble-cap-dp2.png` is the
  grab and `shots/48-user-bubble-cap.png` that run's own frame.
  Five existing tests pinned the old arithmetic and moved with the rule rather than
  around it: the 640 ceiling became the share, the user-cap test now asserts the cap
  *follows* the lane (it was named for ignoring it), the malformed-width floor became
  the bubble's own minimum, and the two scroll rewrap fixtures needed a longer PUBLIC
  row so the wide and narrow lanes still land on different line counts under the
  smaller cap.
- **Scrollbar skin — landed, capture-verified.** The reference declares one skin for
  every element: `--dsh-scrollbar-width: 5px`, an invisible rail, and the
  `scrollbar-bg-l1` / `scrollbar-hover-l1` thumb pair, with elevated surfaces
  rebinding to the `l2` pair on their own container
  (`ui-theme/src/styles/scrollbar.css:20-24`). Native used iced's default: a `10px`
  translucent grey bar.
  `design::SCROLLBAR_WIDTH` (5), `design::scrollbar()` (the `Scrollbar` width and
  scroller width) and `design::scrollable_style` (an invisible rail, a `2px`-radius
  scroller in `LINE`, `MUTED` while the bar is hovered or dragged, and a transparent
  autoscroll overlay) are now applied to **all 18** scrolling surfaces, which is what
  the global rule asks for; `LINE` and `MUTED` are this palette's two rungs over the
  surface, standing in for the `l1`/`hover-l1` pair.
  Verified at the `history-more` preset in a 420px-tall window, where the transcript
  must overflow for a bar to exist at all: the bar measures **5 columns** at
  `x 1034..1038` with a `(64,61,81)` thumb — `LINE` is `0x40,0x3d,0x52` — where the
  same grab under iced's default measured **10 columns** at `x 1030..1039` with a
  translucent grey thumb. `shots/49-scrollbar-dp2.png` is the grab and
  `shots/49-scrollbar.png` that run's own frame.
  This is the pass the previous round recorded as not-half-applied: the first attempt
  scripted the insertion and put the two calls inside each `scrollable(…)` call's
  parentheses, which the compiler caught, and the second run inserted them after the
  matching close paren. A surface's content box shrinks by the bar's width only while
  it actually overflows, which is also how the reference behaves.
- **Window box — landed, source-pinned.** The DMG's own window options are a
  `1280x820` main window with a `520x600` minimum (`PARITY-INVENTORY.md` §Windows,
  read from `main.js`). Native already created the window at 1280x820 but floored it
  at **760x560**, a value no reference carries.
  `design::WINDOW_MIN_WIDTH`/`WINDOW_MIN_HEIGHT` now hold the reference minimum,
  `main.rs` reads all four numbers from `design`, and a new
  `the_window_box_matches_the_reference` test pins both the constants and the two
  option lines so they cannot drift apart again.
  Verification limits, stated plainly: this is a window option the toolkit hands to
  the compositor, so nothing in the fixture surface can measure it — a fixture
  window has `resizable: false`, and Hyprland's Lua `window.resize` dispatcher is
  mouse-driven (`hl.bind(mainMod .. " + mouse:273", hl.dsp.window.resize(), { mouse
  = true })` in the shipped config), so driving a programmatic below-minimum resize
  is the same blocked input path the width-drag handles have. Confirming the clamp
  by hand needs a drag on a real window: shrink it and check it stops at 520x600.
- **Code block box — landed, capture-verified.** The reference's fenced code block is
  a `radius-lg` block with a banner of `padding: 9px 14px` at `11px/18px` and a
  `<pre>` of `padding: 16px` carrying the `11px/19px` code font
  (`ui-primitives/markdown/CodeBlock.module.css`, with
  `--dsw-font-markdown-code-block: 11px/19px` from `gradient-shadow-text.css:166`); the
  banner's fill is one step above the block's (`neutral-bluish-850` over `-900`, which
  this palette expresses as `LINE` over `OVERLAY`).
  Native had **three different sets of numbers for one element**: the transcript's
  renderer drew a 22px header, a 13px/24px code font and `padding(10)`/`padding(12)`
  inside a radius-9 card with a 5px gap; the details panel drew a label-only header at
  a relative `1.3` line height and a 13px font; and the first-match offset path
  hardcoded `49.0` of chrome. `CODE_FONT` (11), `CODE_LINE` (19), `CODE_PAD` (16),
  `CODE_BANNER_PAD` (9/14), `CODE_BANNER_LINE` (18), `CODE_BANNER_HEIGHT` (36),
  `CODE_CHROME` (68) and `design::code_line` now carry the figures, and both renderers
  and the offset path read them, so the block, its height and its scroll offsets
  cannot drift again.
  Verified at `assistant-details-formatted` (1040 wide): the code's line pitch measures
  **19** where the same grab measured **17** under the details panel's relative line
  height, the ink bands are **10..12** rows tall (an 11px font) where they were 13..14
  (13px), and the banner is a full-width strip spanning **646px** — the block's own
  width — in **34** rows. `shots/50-code-block-dp2.png` is the grab and
  `shots/50-code-block.png` that run's own frame. The `native_first_match_offsets…`
  test moved with the metrics: its bound is now body-relative, and its exact relation
  asserts the banner plus the pre's top padding, which is what caught the third copy.
  Remainders: the banner measures 34 rows against the nominal 36 because Iced renders
  the 11px label's line box as 16 rather than the requested 18, and the reference's
  `margin: 16px 0` / `margin-bottom: 11px` rhythm around a code block is not modelled
  — native spaces blocks through its own flow gaps.
- **Markdown block rhythm — partially landed; heading step verified, list items
  recorded.** The reference gives each markdown block its own margins rather than one
  uniform gap: `p { margin: 16px 0 }`, `h1`-`h3 { margin: 32px 0 16px }`, `h4`-`h6
  { margin: 16px 0 }`, lists `margin: 16px 0` with `padding-left: 18px` and
  `li:not(:first-child) { margin-top: 6px }`, and a list item's own first/last child
  margins are zeroed (`ui-primitives/markdown/MarkdownText.module.css:15-44,102-140`).
  Native has **three** different rhythms for that one document: the transcript's
  document view spaces blocks by `BLOCK_GAP` (16), the details panel spaced its body
  by **12**, and neither modelled a heading's taller margin.
  `design::HEADING_EXTRA_TOP` (32 − 16) plus a shared
  `conversation_content::block_extra_top(block, first)` now carry the heading step;
  the transcript's render loop, its `document_height` and its first-match offset walk
  all add it, and the details panel reads the same helper with `BLOCK_GAP` instead of
  its own 12. A new `heading_margins_step_up_like_the_reference` test pins the rule
  (`h1`-`h3` after another block, `h4`-`h6` and paragraphs unchanged) and the
  document height's arithmetic.
  Verified at `assistant-details-formatted`: the paragraph pitch between body bands
  measures **25/26/25** rows in the grab where the same preset measured **21/22/21**
  under the details panel's 12px spacing — the +4 the 16px rhythm asks for.
  `shots/51-markdown-rhythm-dp2.png` is the grab and `shots/51-markdown-rhythm.png`
  that run's own frame.
  Remainders, recorded rather than half-done: the reference's `li:not(:first-child)`
  6px item margin cannot be expressed while native's list items are sibling blocks in
  a fixed-spacing column, so native still spaces them by `BLOCK_GAP`; the code block's
  `margin-bottom: 11px` and the first block's own top margin are likewise not
  modelled. Closing those needs a per-block margin model in the document layout
  (heights, render and offsets together), which is a larger change than this pass.
  That model landed next — see below.
- **Per-block margin model — landed, capture-verified.** The reference gives every
  markdown block its own margins and lets CSS collapse them: `p` 16/16, `h1`-`h3`
  32/16, `h4`-`h6` 16/16, `ul`/`ol` 16/16 with `li:not(:first-child)` 6 inside and
  `li > ul/ol` 4, `hr` 32/32, a code block 16/16 but 11 while another block follows,
  and an `h4`-`h6` followed by a list tightened to 8 on both sides
  (`ui-primitives/markdown/MarkdownText.module.css:15-56,102-147`,
  `CodeBlock.module.css:22-33`). Native expressed all of that as one uniform
  `Column::spacing`, so list items were 16px apart instead of 6 and a code block never
  took its 11px.
  `conversation_content::block_margins` now returns each block's `(top, bottom)` with
  the neighbour rules (list runs, nesting depth, the tight heading) and
  `document_gap` collapses a pair to `max(bottom(i−1), top(i))`; the transcript's
  render loop, its `document_height` and its first-match offset walk read both, and the
  details panel does the same instead of holding its own spacing. A new
  `block_margins_follow_the_reference_rhythm` test pins every branch: paragraph,
  heading step, collapsed gaps, run edges, the 6px item, nesting at 4, the code
  block's 11, the `hr` pair and the tight heading.
  Verified at `assistant-details-formatted`: the ink gaps inside the fixture's ordered
  list measure **13** rows in the grab where the same preset measured **25** and **23**
  under the uniform 16px gap — the 10px the 16-to-6 item margin asks for — while the
  code block's own lines keep their 19px pitch. `shots/52-block-margins-dp2.png` is the
  grab and `shots/52-block-margins.png` that run's own frame.
  The first and last block margins were open at the time and are now settled **from
  source**: the same sheet zeroes them — `.markdown > *:first-child, .markdown
  p:first-child { margin-top: 0 !important }` and the matching `:last-child` rule
  (`MarkdownText.module.css:286-294`). Native therefore already matched the reference
  by giving the first block no leading margin and the document no trailing one, and no
  change was needed; the only reason it looked open was that the rule lives in the
  table section of the sheet, which had not been read yet.
- **Blockquote — landed, capture-verified.** The reference draws a blockquote as
  `border-left: 2px solid var(--dsw-alias-label-caption)` with `padding-left: 14px` and
  `margin: 16px 0 0` (`ui-primitives/markdown/MarkdownText.module.css:151-156`), so its
  rule sits on the block's left edge and its text starts 16px in.
  Native drew a `│` glyph in **`FOAM`** — a teal accent from the palette's decorative
  rung, not a caption colour — inside a 20px box, reserved 20px for it in both the
  height and the offset paths, and gave the paragraph the ordinary 16/16 paragraph
  margins. `QUOTE_BAR` (2), `QUOTE_PAD` (14) and `QUOTE_INSET` (16) now carry the
  figures, the rule is a `MUTED` filled container whose height comes from the same
  `inline_height` measurement that sizes the paragraph (stemming from the paragraph's
  own spans, so the rule spans exactly the text it marks), the two geometry sites
  reserve the reference inset, and `block_margins` gives a blockquote the 16/0 the
  sheet asks for. The details panel reads the same constants instead of its own glyph.
  Verified at `assistant-details-formatted`: the rule measures **2** columns at
  `x 325..326` — the panel's content edge — spanning both quoted lines in the grab,
  where the previous render put the glyph inside a 20px box.
  `shots/53-blockquote-dp2.png` is the grab and `shots/53-blockquote.png` that run's own
  frame.
  Note on the mechanism: Iced's `Border` has no per-side widths, so the rule is a filled
  container rather than a real left border, and its height is passed in from the
  measurement instead of stretching — that is why it is derived rather than `Fill`,
  which resolved to zero inside the row.
- **Compact markdown in the reasoning body — landed, capture-verified.** The reference
  renders its reasoning row with `variant="compact"` (`ReasoningRow.tsx:72`, and the
  trajectory table's detail at `TrajectoryTable.tsx:1571`), a second ladder in the same
  sheet: the body is the secondary `13px/20px` tertiary text, and
  `p`/`ul`/`ol`/`blockquote` are `margin: 4px 0`, `li:not(:first-child)` `2px`,
  headings `8px 0 4px` at `font: inherit; font-weight: 600`, `hr` `8px 0`, a code block
  `margin: 4px 0` with the surrounding text colour, and a blockquote's padding narrows
  to `8px` (`MarkdownText.module.css:330-383`).
  Native drew the reasoning body as plain text, so a fenced block, list or heading
  inside a thought appeared as raw syntax. The shared fixture's reasoning was one plain
  sentence, so nothing had ever reached the ladder; a new `chat-thoughts-markdown`
  preset gives the seq-11 reasoning a heading, a paragraph, an ordered list, a fenced
  rust block and a quote, and expands the row exactly as `chat-thoughts` does.
  `conversation_content::Ladder` now carries both ladders through `block_margins`,
  `document_gap`, `block_height`, `document_height` and `document_view`; the reasoning
  is parsed with the same `formatted::parse` as the answer, held beside it, measured at
  the compact ladder (falling back to the plain height when it does not parse), and
  drawn through `document_view` with `Ladder::Compact` under a tertiary text colour,
  with compact headings rebuilt as bold spans and compact code tokens recoloured to the
  surrounding text.
  Verified at that preset: the same 420px-tall grab shows `### PUBLIC compact heading`
  as a bold line at the body size, the paragraph's bold and inline-code runs resolved,
  the two ordered items 4px apart, and the fenced block as the reference's code box
  (banner, `Copy code`, 11px/19px content) instead of a literal fence.
  `shots/54-reasoning-markdown-dp2.png` is the after grab,
  `shots/54-reasoning-markdown.png` that run's own frame, and
  `shots/54-reasoning-markdown-before.png` the same preset before the change.
  Remainders: the reasoning's *search offset* still walks the plain text rather than the
  compact document, so a match inside formatted reasoning can sit a few pixels from its
  glyph (the row's own height and the drawn blocks are exact); the trajectory table's
  compact markdown is a native surface of its own and was not touched.
- **Reasoning search offset — landed, capture-verified.** The offset recorded above as a
  remainder walked the reasoning's plain text while the body drew a compact document.
  `conversation_content::document_first_offset` now holds one walk for both the answer
  and the reasoning — the same `block_height`, the same `document_gap` and the same
  `Ladder` the render uses — and the reasoning takes it whenever it parsed, keeping the
  plain-text walk only for the unparsed fallback. `compact_heading_spans` is shared by
  the measurement, the render and this walk so a compact heading is measured with the
  bold spans it is drawn with.
  The walk's arithmetic was wrong for two blocks even before the compact work, and the
  new `document_first_offset_walks_drawn_heights_and_gaps` test caught it: it added a
  `BLOCK_GAP` after every block (a leftover from the uniform-`Column::spacing` era,
  double-counting once round 24 moved the gaps inside each block's container) while
  never adding the *next* block's leading gap. It now advances by exactly
  `gap + block_height` per block and returns `y + gap + offset`, which is what the
  render reserves.
  Verified end to end with a new `chat-search-thoughts` preset, which is the first
  fixture that drives the real search path (`SearchInput` at a set serial) against
  formatted reasoning: the grab shows the search field at `1/1 messages` with the
  transcript scrolled so the tint sits exactly on `Second compact item` inside the
  compact list. `shots/55-reasoning-search-dp2.png` is the grab and
  `shots/55-reasoning-search.png` that run's own frame.
  Remaining: the trajectory table's compact markdown is a native surface of its own and
  was not touched.
- **Editable-field context menu — landed, capture-verified.** The DMG shell installs a
  right-click menu for editable fields
  (`apps/desktop/src/main.ts:276-300`): Undo, Redo, a separator, Cut, Copy, Paste, a
  separator, Select all — enabled from Chromium's edit flags, with `accelerator: ''`
  so no shortcut label is drawn, and **no Delete item** (that belongs to the shell's
  separate `application` popup at `:1018-1034`). Native had no right-click path at all.
  `app/src/editor_menu.rs` now carries the entry table (`entries`, with a test pinning
  the order, the two separators and the enablement) and the popup surface; the
  composer's editor is wrapped in `mouse_area(..).on_right_press(..)` inside
  `anchored_popup::anchored`, which keeps the anchor's own flow size and dismisses on an
  outside press. Iced's `MouseArea` only captures the events it has callbacks for, so
  left clicks, keys and the wheel still reach the editor. Undo and Redo drive a
  native editor-owned history of up to 100 steps (`record_editor_edit` /
  `set_editor_text`), like the web editor the shell dispatches those keys into; Cut and
  Copy write the selection through the same wayland-safe clipboard writer the settings
  fields use; Paste reuses the composer's existing asynchronous clipboard read; Select
  all is `text_editor::Action::SelectAll`.
  Verified with a new `--editor-menu true` fixture flag at the `conversation` preset:
  the observation records `native-editor-menu` at **180 × 177.4** (x 826, y 435.6) in a
  1040×700 window, and the grab shows the six items in the shell's order with the two
  separators, Undo/Redo dimmed with an empty history and Cut/Copy dimmed without a
  selection while Paste and Select all stay live.
  `shots/56-editor-menu-dp2.png` is the grab, `shots/56-editor-menu.png` that run's own
  frame, and a copy of the grab is under `app/evidence/`. A second test drives the
  history through the production `update`: two edits, two Undos, a Redo, Select all,
  then an edit that drops the redo branch.
  Deliberate differences: the popup anchors to the editor rather than the pointer
  (native's other menus anchor to their trigger), Paste and Select all are always
  enabled because Iced exposes no edit flags for them, the caret lands where
  `Content::with_text` puts it after an Undo (Iced has no text-plus-caret restore), and
  the same menu is not yet offered on the settings and rename text inputs.
- **Markdown tables — landed, capture-verified.** A GitHub table used to fail the whole
  parse: `Tag::Table` fell through `Builder::start`'s catch-all, `parse` returned
  `Failure::Unsupported`, and the entire assistant reply degraded to raw plain text —
  losing its headings, lists, code and links with it. The reference renders tables
  (`ui-primitives/markdown/MarkdownText.module.css:192-283`, tokens at
  `ui-theme/src/styles/gradient-shadow-text.css:117,124`).
  `formatted::Block::Table { header, rows, indent }` now holds a table's cells as inline
  runs (the head is the first row, `Options::ENABLE_TABLES` was already on), with the
  same bounds as every other block list: `CELLS` 64 per row, `ROWS` 128, and the shared
  byte/span charges. The geometry (`table_height`/`table_row_height`) measures each row
  as its tallest cell at that column's width, and one shared `table_view` draws it for
  the transcript, the reasoning and the details panel: `padding: 10px 16px` per cell,
  the first cell's left and the last cell's right padding trimmed, a one-pixel rule
  under every row standing in for the sheet's per-side `border-bottom`, `13px` text on
  the ladder's own line box (22px on the body ladder, 20px compact), a fill under four
  columns and the reference's `100px` column floor with a horizontal scroller — and the
  horizontal skin from the same scrollbar constants — above it. The search walk and
  `matches` cover cells too.
  Verified at a new `chat-table` preset: the grab shows a paragraph, a three-column
  table whose header and body rows carry their rules with the first column flush, the
  inline runs inside cells (`one` as a code chip, **bold** in bold) and a paragraph
  after it. `shots/57-markdown-table-dp2.png` is the grab, `shots/57-markdown-table.png`
  that run's own frame, and a copy is under `app/evidence/`. The parser test
  `tables_keep_their_header_inline_runs_and_bounds` pins the header/row/inline shape,
  the refusal of a table inside a list or quote (the flat model cannot keep
  provenance), the row cap, and the cell cap through a direct event stream. Both suites
  green (1107 fixture / 1100 app), format-debt 0, and the release fixture example
  builds.
  Deliberate differences: the sheet's `border-bottom` is 0.5px and native's rule is one
  pixel (Iced's `Border` has no per-side widths, so it is a filled strip inside the
  cell), header and body rules share `LINE` where the sheet steps `border-l3` over
  `border-l2`, the reference reveals a wide table's bar on hover where native always
  shows it, and the header's weight-500 face has no DejaVu equivalent, so it matches
  the body's face rather than a heavier bold. A table's own margin is zero on the body
  ladder (the sheet gives `.tableScroll` no margin) and 4px compact.
- **Markdown images — landed (fallback branch), capture-verified.** An image failed the
  whole parse for the same reason tables did, taking the entire reply's formatting with
  it. The reference has three image branches: an unresolvable source renders the alt as
  `imageAlt` tertiary italic (`render.tsx:640`), a failed load renders
  `{failed} · {alt || destination}` (`:653`), and a resolved source renders the
  `radius-md` box (`:654`, `MarkdownText.module.css:303-315`).
  Native implements the first branch and, when the alt is empty, the second branch's
  `|| destination` — so an image can no longer vanish or cost the message its
  formatting. `Tag::Image` now opens an alt buffer that collects the image's plain text
  (dropping its inline markup, exactly like an HTML alt attribute), and `Span` gained an
  `alt` flag so the run renders italic in `MUTED`/tertiary while staying inline inside
  its paragraph. A non-empty title is still refused rather than silently dropped, and
  the alt bytes are charged against the same text budget.
  Verified at a new `chat-image` preset: the grab shows an inline image's alt inside a
  sentence, a standalone image's alt on its own line, and a no-alt image showing its
  destination, all italic and tertiary, with the surrounding paragraphs intact.
  `shots/58-image-fallback-dp2.png` is the grab, `shots/58-image-fallback.png` that run's
  own frame, and a copy is under `app/evidence/`. The parser test
  `images_fall_back_to_their_alt_run_and_keep_the_rest_of_the_message` pins the alt
  run's shape, the stripped markup, the destination fallback and the title refusal.
  Blocked remainder, stated plainly: the *resolvable* branch has no counterpart. Iced
  would need a decode + texture path plus a deliberate policy for fetching
  `https://`/local paths, which is a feature of its own rather than a measurement; until
  then every image takes the unresolvable branch, so a picture the reference would show
  appears here as its alt text.
- **Referenced-session summary — landed, capture-verified.** A user turn that triggered
  a cross-session recall shows one line under its bubble: `.referenceSummary` at
  `color: label-tertiary; font-size: 13px; line-height: 18px + delta_secondary`
  (`ui-chat/src/client/chat/MessageItem.module.css:42-45`), rendered after the bubble
  inside `.userStack` and before the action row (`MessageItem.tsx:225-229`). The copy is
  `Referenced session · {labels}` with labels joined by `, `
  (`ui-chat/src/client/locale.ts:317-318`; the zh sheet uses `、`), and the labels come
  from the context record that *immediately follows* the user message:
  `source.references[].label` when `source.kind === 'session-reference'`
  (`conversation-nodes/event-projection.ts:83-87`).
  `DisplayRow` now carries those labels; the reducer attaches them to the last human row
  before pushing the record's own hidden row; `design::REFERENCE_LINE` (18) and
  `reference_line` hold the sheet's third line box beside the body's 24 and the compact
  20; and the row's geometry takes the summary's height as part of its cache identity, so
  a record arriving after the row was measured re-measures it. The view pushes the line
  into the same right-aligned stack as the bubble, which is what gives it the stack's
  8px gap.
  Verified at a new `chat-references` preset: the grab shows `Referenced session ·
  PUBLIC session alpha, PUBLIC session beta` under the user bubble, in tertiary 13px,
  above the action row, with the assistant reply below it.
  `shots/59-referenced-session-dp2.png` is the grab, `shots/59-referenced-session.png`
  that run's own frame, and a copy is under `app/evidence/`. The reducer test
  `a_session_reference_attaches_its_labels_to_the_message_it_follows` pins the
  attribution (the preceding human row only, never the hidden context row or a later
  message), that the record's own row stays out of Chat, and that a malformed reference
  list contributes nothing.
  Remainder, stated plainly: the geometry reserves **one** line. The reference measures
  the wrapped block, so a label list long enough to wrap can exceed the reserved height;
  in the fixture's grab the row had slack and both wrapped lines showed, but a three-line
  summary in a tight row would clip. Fixing it properly means passing the joined label
  text and the stack width into the measurement rather than a scalar.

