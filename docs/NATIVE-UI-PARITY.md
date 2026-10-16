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
| Conversation header | 76 min-h · bottom 0.5px seam · title 30 row | padding [14,20] + seam |
| Chat content width | `clamp(680, 64%, 920)` | `design::chat_width(pane)` clamp, bounded by the padded pane |
| User bubble max-width | `min(70.2% · content, 82%)` | content-sized, fixed 640 cap (≈70.2% of the widest column) |
| Assistant text | unboxed, full content column | unboxed, spans the shared content column |
| Message flow gap | 6 process / 12 response / 16 turn | `design::FLOW_GAP`/`RESPONSE_GAP` ladder 6/12; the 16px `turn-process` row is Trajectory-only |
| Tool activity | compact unboxed single muted line | compact unboxed single muted line, expands in place |
| Tool-row file path | grey **dotted** underline, clickable | solid `MUTED` underline (Read/Edit/Write) |
| Edit diff stat | parsed-result diff `+N -M` after the summary | call-argument line diff `+N −M` after the target |
| Composer card radius | panel 28 | 20 |
| Composer text max-height | 336 (14×24) | auto-grows: 14 logical lines (297 px), 8 in a short window |
| Send button | 34×34 · pill | Send pill, 8/14 pad |
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
  brand row has no macOS titlebar strip, so its own height is unchanged. The
  RECENT section header and the search field keep their native boxes.

