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
| `lg` | 16 | — |
| `xl` | 20 | `RADIUS_XL` 20 |
| `panel` | 28 | `RADIUS_PANEL` 20 (intentional, tighter) |

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
- **Content width:** native now follows the web clamp (`680…920`); the web's drag width handles remain unqualified native work.
- **Tool-row path affordance:** web uses a grey dotted underline on a clickable link; native underlines in `MUTED` (Iced has no dotted underline).
- **Tool-row disclosure:** the web expands a tool row in place; native now does the same (`aria-expanded` equivalent: the `▸`/`▾` chevron). An expandable row toggles its recorded body inline and never opens the detail panel; a row with nothing recorded keeps no affordance. The body is the text this app already retained for the row (`tool/call` name + arguments, `tool/result` content), rendered inert in mono 13 px, word-wrapped and capped at 320 px. The detail panel remains for assistant replies and raw records.
- **Edit diff stat:** the web derives `+N -M` from the tool result's parsed diff; native derives it from the `edit` call's contiguous `old_string`→`new_string` line diff (bounded to 256 lines per side, counts only, content never surfaced).
- **Message measure:** the unboxed assistant column follows the shared content column; the user bubble keeps a fixed 640 px cap (≈70.2% of the widest column) so resizing still reuses cached user-row geometry. Assistant rows re-measure on resize, as the web reflows.
- **Flow gap:** native uses one 12 px gap (the web's assistant-response value) instead of the web's adjacency-dependent 6/12/16; each row's height carries its own half so virtual row heights stay independent.
- **Header:** native title row and status/decision rows differ from the web tab strip; seam aligned.
- **Chat / Trajectory:** the web splits Chat from a Trajectory log; native now does the same with one tab strip above the conversation. **Chat** presents the conversation (human messages, assistant replies, tool activity, turn notices) and nothing else: the system prompt, developer injects, model-only context, the `Injected context` row and every known bookkeeping or telemetry kind (`session-log-*`, `command/*`, `todo/write`, `subagent/*`, …) are Trajectory-only. **Trajectory** shows every retained record in the same lane and the same scroll model, and still filters the control-only kinds (`session/title`, `session/title-llm-request`, `model/selection`).
  Retention is untouched: Trajectory never discards a record, only presents it elsewhere. The exclusion is a **known list rather than an allowlist**, so an event kind this build does not recognize stays visible in Chat — a future required diagnostic must never be silently dropped (`future/required` is the standing case). A `user/message` is human history when its source kind is `user` **or carries no kind at all**; every real producer declares its kind on the wire (a recorded session shows the human prompt as `kind: "user"` and the context injects as `runtime-context`, `time-context` and `skill-catalog`), so an unlabelled message is kept rather than risk hiding the user's own text.
- **Composer:** native editor now auto-grows with the draft (logical lines at 19.5 px, capped at 14, or 8 in a short window); it does not model wrapped visual lines, has no masked fade band, and keeps explicit Send/Stop controls.

Remaining unqualified native work is unchanged: physical input/IME, CJK, accessibility, compositor fractional scaling, live provider execution.

## Verification owners

Native geometry/cache tests, `layout_smoke` PUBLIC fixtures, App reducer tests, and `docs/FEATURE-PARITY.md` status. Source-only comparison is not visual parity; rendered capture status is recorded in the handoff.
