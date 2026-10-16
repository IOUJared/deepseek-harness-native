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
| Sidebar inline padding | 12 | 14 |
| Session row | 36 h · 7/8 pad · radius md | 36-ish (11/10 pad) |
| New-session row | 38 h · 8/12 pad | 38-ish (11/10 pad) |
| Conversation header | 76 min-h · bottom 0.5px seam · title 30 row | padding [14,20] + seam |
| Chat content width | `clamp(680, 64%, 920)` | `design::chat_width(pane)` clamp, bounded by the padded pane |
| User bubble max-width | `min(70.2% · content, 82%)` | content-sized, fixed 640 cap (≈70.2% of the widest column) |
| Assistant text | unboxed, full content column | unboxed, spans the shared content column |
| Message flow gap | 6 process / 12 response / 16 turn | uniform 12 (6 px row padding each side) |
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
- **Transcript line-height (decision, not changed):** the web sets its line boxes in pixels — the assistant body to `calc(24px + var(--dsh-content-font-delta))` (`AssistantMarkdown.module.css:13`) and the thinking body to `calc(20px + var(--dsh-content-font-delta-secondary))` (`ReasoningRow.module.css:49`) — where native uses Iced's relative `LineHeight::default()`, which is `1.3` (`iced_core-0.14.0/src/text.rs:215-219`). At native's body size that is 19.5px against the web's 24px: about **23% tighter per line**, and the two cross over near the top of the range (native 26px at size 20, the web 25px). Adopting the web's line boxes would make every transcript row materially taller, so it is recorded here rather than changed unilaterally — the density is visible in every capture kept in `../shots/`.
- **API key shape:** the shell refuses a key that arrived wrapped in quotes or copied as an environment assignment, in its own words (`welcomeKeyInvalid`: "Enter the API key itself, without quotes, spaces, or an environment-variable assignment"). Native applied only the Core frame's rule — printable non-whitespace ASCII, shared with callbacks — which *accepts* `"sk-…"` and `DEEPSEEK_API_KEY=sk-…`, so either would have been saved verbatim as the key. `settings::key_shape_error` now refuses both shapes at Review and again at Confirm, with the shell's message. It is deliberately separate from `secret_input::Input::ascii`, because `=` is ordinary in a callback (`?code=…`, base64 padding). Whitespace needs no extra check: the shared rule already refuses every byte outside `0x21..=0x7e`.
- **Tool-row path affordance:** web uses a grey dotted underline on a clickable link; native underlines in `MUTED` (Iced has no dotted underline).
- **Tool-row disclosure:** the web expands a tool row in place; native does the same. The affordance now sits where the web's does — in the **leading** 16 px box ([`design::LEADING_BOX`]), which shows the tool's variant glyph at rest, `▸` once the pointer is over the row, and `▾` while it is open (`DisclosureRow.tsx` swaps the leading glyph into a chevron on hover and on open, and draws no trailing chevron). Hover comes from a `MouseArea` wrapped around the row's button; it hands the event to its content first and returns once the button captures, so the press still toggles. A row with nothing recorded keeps the plain variant glyph and starts no request. The body is the text this app already retained for the row (`tool/call` name + arguments, `tool/result` content), rendered inert in mono 13 px, word-wrapped and capped at 320 px. The detail panel remains for assistant replies and raw records. An **answerless** reply (thinking or a tool call with no answer) renders only that disclosure: no empty body and no Copy/Details action row. *The resting and open glyphs are confirmed by capture; the hover swap itself needs a pointer and is not visually verified.*
- **Chat / Trajectory ownership:** see [the records projection](<NATIVE-RECORDS-PROJECTION.md>) for exactly which roles and source kinds each tab presents.
- **Edit diff stat:** the web derives `+N -M` from the tool result's parsed diff; native derives it from the `edit` call's contiguous `old_string`→`new_string` line diff (bounded to 256 lines per side, counts only, content never surfaced).
- **Message measure:** the unboxed assistant column follows the shared content column; the user bubble keeps a fixed 640 px cap (≈70.2% of the widest column) so resizing still reuses cached user-row geometry. Assistant rows re-measure on resize, as the web reflows.
- **Flow gap:** native uses one 12 px gap (the web's assistant-response value) instead of the web's adjacency-dependent 6/12/16; each row's height carries its own half so virtual row heights stay independent.
- **Header:** native title row and status/decision rows differ from the web tab strip; seam aligned.
- **Chat / Trajectory:** the web splits Chat from a Trajectory log; native now does the same with one tab strip above the conversation. **Chat** presents the conversation (human messages, assistant replies, tool activity, turn notices) and nothing else: the system prompt, developer injects, model-only context, the `Injected context` row and every known bookkeeping or telemetry kind (`session-log-*`, `command/*`, `todo/write`, `subagent/*`, …) are Trajectory-only. **Trajectory** shows every retained record in the same lane and the same scroll model, and still filters the control-only kinds (`session/title`, `session/title-llm-request`, `model/selection`).
  Retention is untouched: Trajectory never discards a record, only presents it elsewhere. The exclusion is a **known list rather than an allowlist**, so an event kind this build does not recognize stays visible in Chat — a future required diagnostic must never be silently dropped (`future/required` is the standing case). A `user/message` is human history when its source kind is `user` **or carries no kind at all**; every real producer declares its kind on the wire (a recorded session shows the human prompt as `kind: "user"` and the context injects as `runtime-context`, `time-context` and `skill-catalog`), so an unlabelled message is kept rather than risk hiding the user's own text.
- **Composer:** native editor now auto-grows with the draft (logical lines at 19.5 px, capped at 14, or 8 in a short window); it does not model wrapped visual lines and keeps explicit Send/Stop controls. The web's **fade band** now exists: in the active phase, and only while the composer is on screen, a 36 px top-to-bottom transparent-to-base gradient overlays the bottom of the transcript through `Stack`, so it costs no layout height and cannot shorten the lane. *The gradient's direction is derived from `Radians::to_distance` and unit-tested; its rendered appearance has not been visually verified.*

Remaining unqualified native work is unchanged: physical input/IME, CJK, accessibility, compositor fractional scaling, live provider execution.

## Verification owners

Native geometry/cache tests, `layout_smoke` PUBLIC fixtures, App reducer tests, and `docs/FEATURE-PARITY.md` status. Source-only comparison is not visual parity; rendered capture status is recorded in the handoff.
