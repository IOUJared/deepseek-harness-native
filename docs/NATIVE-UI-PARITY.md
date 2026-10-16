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
| Permission chip | control-row chip, `height: 28px`, `padding: 0 4px 0 8px`, `13px/20px` `label-secondary`, a 14px glyph, dimmed while locked | **landed** — `native-permission-control` 28.0 × 137.9 at x 380, and its menu `native-permission-menu` 200.0 × 90.7; the pick sends `/permission <preset>` |
| Permission select | control-row chip: `height: 28px`, `padding: 0 4px 0 8px`, transparent, `radius-sm`, `13px/20px` weight 500, `label-secondary`, `max-width: 220px`, a 14px glyph on the trigger and 16px in the rows, hover `interactive-bg-hover`, disabled `label-dimmed`; menu rows plus an Auto-review preset with a badge and a risk confirmation before full access | **not implemented** — no permission projection state; recorded below with the package's parts |
| Shutdown budgets | DMG shell: graceful 10 s → SIGTERM 5 s → SIGKILL 5 s (`apps/desktop/src/main.ts:3790-3807`) | native 3 s / 1 s / 1 s (`StopPolicy::default`), documented in `NATIVE-CLOSE.md#budgets-against-the-dmg-shell`; aligning needs a measured Host unwind, not the shell's figures |
| Referenced-session summary | under a user bubble: `Referenced session · labels` at 13px/18px tertiary, labels from the following `session-reference` context record, joined with `, ` | `DisplayRow::references` + `REFERENCE_LINE`/`reference_line` and the row's geometry; `chat-references` preset |
| Markdown image | unresolvable source renders the alt as tertiary italic; a failed load adds the destination when there is no alt (`imageAlt`, `radius-md` block) | `Span::alt` + `render.tsx:640,653`; no loading or decoding pipeline yet |
| Markdown table | `th`/`td` `padding: 10px 16px` on the body ladder and `4px 8px` compact, first cell flush, last cell flush right, a rule under every row, `13px/22px` body and `13px/20px` compact, fills under four columns and scrolls at a `100px` floor above it | `TABLE_PAD_*`/`TABLE_MIN_COLUMN`/`TABLE_FILL_COLUMNS`/`TABLE_LINE` + the shared `table_view`; `Block::Table` |
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

## Open items

Everything below is proven against the reference and deliberately not landed yet, with
the one step that would settle each. Nothing here is a measurement anyone can read off
the CSS alone; each needs a decision, a live run, or a feature-sized build.

1. **Markdown images — the resolvable branch.** Native renders the alt fallback
   (`render.tsx:640,653`) and documents it; a picture the reference would show appears as
   its alt text. Needs a decode plus texture path and a deliberate fetch policy: are
   `https://` URLs fetched, are local paths read, or neither? That policy is the user's
   call, and it decides whether the feature is safe to build at all.
2. **Permission select — the mutation is a conversation command, so the chip mirrors the
   plan chip.** Corrected again, and this one shrinks the feature: the package's own header
   says the picker "submits the `/permission <preset>` command line, so both surfaces" agree,
   and that "`/permission <preset>` still switches directly" (`ui-permission-presets/src/client/index.ts:3-11`).
   The Host therefore owns a `/permission` command exactly as it owns `/plan`, and native
   already submits composer lines for the plan chip (`app/src/plan_control.rs:44-54`). So the
   chip needs **no new bridge, Core protocol or app plumbing to switch a preset** — it sends
   `/permission <preset>` through the composer. The landed bridge, Core family and typed
   client remain the *other* path the reference also uses (the remote `set`), and the
   catalog read is still the one remote call; the previous note follows for that part.
   (Original note, superseded for the mutation:)
   `core/runtime/permission.mjs` now answers `permission-catalog` and `permission-set` for
   the Host's `permissionPresets` service: the catalog is reduced to the three fields a
   client renders (`value`, `name`, an optional `description`) with the Host's own
   "missing label falls back to the preset key" rule, a set carries a bounded session and
   preset name and surfaces a refusal as a bounded error, and nothing else on the wire
   ever reaches it. Its `core/runtime/permission.test.mjs` joins the existing
   `node --test core/runtime/*.test.mjs` gate (`scripts/verify-native.py:65`), which now
   reports **414 pass / 0 fail** with the four new cases included.
   The two commands are now **routed**: `fork-supervisor.mjs` imports the bridge, builds it
   beside the Codex and Cloudflare bridges, validates them in `parseForkCommand` with
   `exact` key sets plus `validPermissionSession`/`validPermissionName`, and dispatches them
   in its `switch`. Verified by importing the supervisor directly — a valid
   `permission-catalog` and a `permission-set {session, name}` are accepted, while a missing
   name, a 129-character name and an extra key are each refused — on top of the full
   `node --test core/runtime/*.test.mjs` gate at **414 pass / 0 fail**.
   Still to come, in this order, with the files each touches:
   (a) **done** — `core/src/lib.rs` carries `Command::PermissionCatalog` and
   `Command::PermissionSet { session, name }` plus the two `PublicReply` variants, and
   `core/src/protocol.rs` maps their names, refuses a set whose session or name is
   unbounded before it reaches the wire, and projects both replies;
   (b) **done** — `core/src/permission.rs` is the typed client: `catalog(value)` keeps only
   the three rendered fields, falls back to the preset key for a missing label (the Host's
   own rule), refuses a catalog whose `defaultPreset` names nothing selectable, and
   `set_ack` accepts only `ok: true`. Its four tests join the Core suite, which reports
   **108 passed / 0 failed**;
   (c) **done** — the catalog read is wired end to end: `Command::PermissionCatalogRead`
   maps the Core's `PublicReply::PermissionCatalog` into `Event::PermissionCatalog`
   (`app/src/worker.rs:2187-2188` is the shape), the app keeps the reply in
   `permission_catalog`, the chip asks for it when its menu first opens, and the menu lists
   the Host's own `options` with the design set only as the stand-in before a reply.
   Verified with a fixture-seeded Host catalog (`--permission-catalog true`): the capture
   shows the menu offering **"Ask every time"** — a name that exists only in the seeded
   reply, replacing the design set's "Read only" — with the current preset in full ink.
   Two corrections to earlier estimates are worth keeping: the delivery is the plain worker
   event pattern, and no sub-effect module was required.
   The **Auto badge** is landed with it: when the Host's catalog carries the registered Auto
   preset, the menu row shows the reference's own `EXP` copy beside it
   (`ui-permission-presets/src/client/locales.ts:54`), and `PresetOption`'s full shape was
   checked against the Host's declaration (`api-catalog.ts:6293-6295`: `value`, `name`, an
   optional `description`) — so the Core projection drops nothing the badge would need.
   Captured with the fixture catalog extended by an Auto option
   (`shots/69-permission-auto-badge-dp2.png`), which shows `Auto` with `EXP` dimmed beside
   it below `Full access`. (Superseded note follows.)
   The worker/app plumbing, whose delivery shape is now pinned too: a worker arm
   spawns the request and emits an `Event`, exactly as `app/src/worker.rs:2187` does for
   `Command::Inspect` (`backend.request(CoreCommand::Inspect, …)` mapped into
   `Event::Inspection`), so the catalog read is a `Command` variant, an arm in that shape
   mapping `PublicReply::PermissionCatalog`, an `Event` variant, the app's handler and its
   state — no sub-effect module is strictly required, since plain variants like
   `AccountSubscribe` and `PluginsRead` already exist (`app/src/worker.rs:48-70`).
   Corrected estimate: this is a **feature module, not three
   edits** — the app's `Command` enum (`app/src/worker.rs:48-70`) is a taxonomy whose every
   variant wraps that feature's own `Effect` enum (`crate::codex::Effect`,
   `crate::settings::cloudflare::Effect`), so permission needs its own effect type, its own
   worker handler that calls `backend.request(CoreCommand::PermissionCatalog, timeout)` and
   projects the reply the way `app/src/codex_worker.rs:308-320` does for
   `CodexRuntimeRead`/`Save`, its own app message and its own state to hold the catalog the
   chip will read. An earlier revision of this entry called it three edits, and reading the
   enum showed that was optimistic. The app already compiles against the new Core variants;
   after (c) lands, (d) is the first visible change in this feature;
   (d) the chip, menu, Auto-review badge and confirmation guard, at the geometry below, with
   a DP-2 capture — and, given the `/permission` command, the menu can offer the presets the
   Host already advertises to the command menu rather than waiting on the catalog read. Two facts gathered for it: the reference's three permission glyphs are
   SVGs with no DejaVu equivalent, so the chip needs a text stand-in the way the plan chip
   uses `☑` — checked against DejaVuSans, `◈` (U+25C8), `▣` (U+25A3) and `⎔` (U+2394) are
   covered while `⚿` (U+26BF) is not; and the chip's *disabled* form is a faithful state to
   start from rather than a placeholder, since the reference renders exactly it when the
   control is locked (`disabled={locked || leaving}`, `color: label-dimmed`). The one thing
   still unlocated is the composer's control-row composition site: the model chip is built
   at `app/src/model_controls.rs:343-355` and placed from `ui.rs`.
   The first two are Core-side and user-independent; the last needs a fixture flag once the
   chip exists.
   (Original note follows.)
   The wiring, read from the Codex bridge: a bridge is a `core/runtime/<name>.mjs` exporting
   a factory (`createCodexBridge`) whose `switch (command.type)` answers fixed commands
   (`codex.mjs:322-324`); it is embedded in the Core binary by adding a
   `("<name>.mjs", include_str!("../runtime/<name>.mjs"))` entry to the list in
   `core/src/lib.rs:561-565`; and its test is a JS module driven from a `core/src/*_tests.rs`
   file, which imports the factory (`codex_tests.rs:120` imports `createCodexBridge`). A
   permission bridge therefore needs exactly three things: the `.mjs` calling
   `ctx.permissionPresets.catalog()` and `.set(session, name)`, its `include_str!` line, and
   a `permission_tests.rs` driving both commands against a fake ctx (the catalogue's
   ok/error and pending/failure shape is in `catalog.ts:134-148`).
   (Corrected: an earlier revision called this gate "outside the app crate" and that was
   wrong — native owns its bridge scripts, `core/runtime/*.mjs`, and `codex.mjs:322-324` is
   the pattern to copy: a `switch (command.type)` over fixed commands such as
   `codex-runtime-read` and `codex-runtime-save`.) The Host owns permissions as *remote API
   methods*, not a conversation command:
   `permissionPresets.catalog(): PermissionCatalog` and
   `permissionPresets.set(session, name)` — "record a changed preset, then update each
   changed knob through its own setter"
   (`packages/extensions/tool-cordis/src/api-catalog.ts:1666-1703`). That is why the plan
   chip needed no such bridge: the same catalog lists plan mode as owning "the `/plan`
   command" (`:1706-1716`), which native can send through the composer
   (`app/src/plan_control.rs:44-54`). Permissions have no equivalent command, so the chip,
   menu, badge and confirmation guard cannot be built until the runtime bridge exposes
   those two methods. The native-side projection of the current preset is already landed
   (`Transcript::permission_preset`), so the UI work that remains is genuinely small once
   the bridge exists.
3. **Permission select (original note).** The chip, its menu, the Auto-review preset with its badge, the
   risk confirmation before full access, and the Host command that changes the preset.
   Recorded with its exact geometry and package parts in the verification log below; it is
   several items, each with its own capture, not one.
3. **Shutdown budgets.** Native stops a Host in 3 s / 1 s / 1 s where the DMG shell waits
   10 s / 5 s / 5 s (`NATIVE-CLOSE.md#budgets-against-the-dmg-shell`). Settling it needs a
   live run: start against the real backend, begin a turn, close the window, and time from
   the close request to the shutdown acknowledgement. Until that number exists the native
   budget stays as it is.
4. **The window floor, by hand.** The DMG's own window options say `min 520x600`
   (`main.ts`) and native now uses those figures, but nothing programmatic can drag a
   window below its floor: fixture windows are `resizable: false` and Hyprland's Lua
   `window.resize` is mouse-driven. Drag a real window edge inwards and confirm it stops
   at 520x600.
5. **Inline code chips.** The reference draws an inline code run as a chip:
   `background-color: markdown-inline-code`, `border: 0.5px solid border-l1`,
   `border-radius: radius-sm`, `padding: 0 5px`, at `0.875em` mono
   (`MarkdownText.module.css:163-174`). Native renders the run in the mono family and
   `FOAM` but with no chip: Iced's `rich_text` spans carry no background, and the only
   alternative is rendering each inline code run as its own widget, which would need an
   inline-flow layout the toolkit does not provide. Blocked on toolkit capability rather
   than on a decision; a chip would need either a per-span background in Iced or a
   text-layout rework that preserves wrapping inside a paragraph.
6. **The reference screenshot.** The supplied 1980x1319 PNG
   (`sha256:b8e164f5…`) is not in this workspace, so first-line offsets, bubble-side gaps
   and tool-row rhythm stay CSS-derived. Placing it under
   `/home/jared/Documents/deepseek-harness/default-workspace/` would let those be measured
   from pixels instead.

7. **Permission default row in Settings — a second surface, not yet built.** The reference
   has *two* permission pickers, and only the composer one is done here. `PermissionRow.tsx`
   is "the default preset for subsequently created sessions", mounted in the
   `settings.general.item` slot, with its own `RiskConfirmation` and its own write path:
   `load()` fetches a descriptor and `select(preset)` persists through the host-backed
   settings preference (`PermissionRow.tsx:1-40`), returning `null` when the host exposes no
   permission settings and closing itself when the state is not writable. The composer chip
   covers current-session switches, so this is additive rather than a divergence in what is
   already there — but it is a real, proven surface native lacks.
   Sizing, corrected after checking rather than assumed: the row is **not** a settings-row
   edit over an existing read. Its injected face is `load()` plus `select(preset)` against
   the host-backed *permission settings* preference (`PermissionRow.tsx:21-40`), which the
   package registers against `remote.settings` and `remote.permissionPresets`
   (`ui-permission-presets/src/client/index.ts:61-68`). Native's existing generic settings
   path carries *plugin* namespaces (`app/src/worker.rs`, `PluginsRead`/`PluginSave`), so
   this needs new bridge and Core work of the same kind the catalog read needed — a settings
   descriptor read and a write — before any row exists. That is three slices, not one, and
   an earlier revision of this entry called it a mirror of the plugin path.
   The bridge half is **landed**: `permission.mjs` answers `permission-settings` and
   `permission-default-set`. It never guesses a namespace name — it scans `describe()`'s
   views for the one whose value carries `defaultPreset`, which is the descriptor the
   reference's own projection reads (`settings-store.ts:45-58`: `view.value.defaultPreset
   ?? catalog.defaultPreset`), and writes with one `set` at that view's revision so a
   concurrent change is refused rather than overwritten. Read-only deployments, missing or
   negative revisions and unbounded names are all refused before anything reaches the Host,
   and a refusal's own message never crosses the boundary. The row's option list needs no
   new call: it is the catalog's `defaultOptions`, already projected.
   Verified: `permission.test.mjs` grew to six tests, the full `node --test
   core/runtime/*.test.mjs` gate reports **416 pass / 0 fail**, and importing the supervisor
   directly accepts both new commands with exact key sets while refusing a missing revision,
   a negative revision, an empty name and an extra key.
   The Core half is **landed** too: `Command::PermissionSettings` and
   `Command::PermissionDefaultSet { name, expected_revision }` with their `name()` strings
   and `PublicReply` variants, an encode arm that refuses an unbounded name or a revision
   past the safe-integer bound before the wire, and `permission::settings(value)` /
   `default_ack(value)` projections that require all three descriptor fields and fail closed
   on a malformed or negative revision. Core suite **109 passed / 0 failed**; the app still
   compiles against the new reply variants.
   The app plumbing is **landed**: `Event::PermissionSettings` and
   `Event::PermissionDefaultSet` on the same worker-event pattern as the catalog read, a
   `PermissionDefaultPick(name)` message that writes at the descriptor's own revision and
   only when the descriptor is writable and no write is already in flight, and — after a
   successful write — a re-read of the descriptor rather than an assumption that the new
   value took, so a refusal leaves the old one in place and the row never lies about what
   the deployment holds.
   The row itself is **landed** and capture-verified: `RouteFacts` carries a
   `PermissionDefault` (current value, revision, writable, advertised options, saving), the
   General page renders "Permission for new sessions" with the descriptor's current value
   only when the deployment exposes one, opens the catalog's advertised presets, and maps a
   pick to the write that matches the descriptor's revision. The row is dimmed and inert
   while unwritable or saving, which is the reference's own rule
   (`PermissionRow.tsx:47-60`).
   Verified with the fixture standing in for both halves of the same Host read
   (`--settings true --permission-catalog true`): the grab shows `Permission for new
   sessions` with `Workspace write` — the descriptor's value under the catalog's own name —
   directly above `Context & tool policy` in the General page
   (`shots/70-permission-default-row-dp2.png`). The first two attempts framed the wrong page
   and then a modal whose descriptor had never been seeded; both were caught by looking
   rather than by assuming, which is why this entry says captured rather than built.
   Both suites green, format-debt 0.
   Open item 7 is therefore closed; what remains for the permission feature is one live run
   to confirm that picks actually land.
   Then: mirror the row in native's General page, then mirror the `settings.general.item` row in native's General page
   (whose rows are a `column!` of labelled disclosures, `app/src/settings.rs:1014-1045`),
   reusing the landed full-access confirmation, and capture it with a fixture-seeded state. Native's
   settings row should also honour the reference's own rules: hidden when the host exposes
   nothing, closed while `status === 'unavailable'` or unwritable, and busy during
   `loading`/`saving`/`confirmingFullAccess`.

8. **The General page's other registered rows — a survey, not yet a decision.** Seven
   packages register `settings.general.item` rows in the reference: `ui-settings-general`
   itself, `ui-chat`, `ui-conversation`, `ui-permission-presets` (**landed** above),
   `ui-settings-session-log`, `ui-shortcuts` and `ui-theme`. Native's General page carries
   the shortcuts disclosure already and a fixed dark Rosé Pine theme by the user's own
   preference, so neither of those is automatically a gap. What is unaudited is what
   `ui-settings-general`, `ui-chat`, `ui-conversation` and `ui-settings-session-log` each
   put in that slot, and whether each is a functional control native lacks or a row the
   user's "keep settings minimal — no metadata lines, revision numbers or field counts"
   preference should deliberately omit. This is recorded as a survey with the filter stated,
   because treating all seven as divergences would contradict that preference.
   **Audited, and only one of the seven is a live question.** Read directly:
   `ui-chat` and `ui-conversation` register **no** General row at all — they matched the
   earlier file scan through other slots, so two of the seven were false positives.
   `ui-settings-general` registers two: `developer-tools` (order 15) and `current-version`
   (order 100). Both are **deliberate omissions**: a version row is exactly the metadata the
   "no revision numbers or field counts" preference excludes, and a developer-tools row has
   no subject in an app with no webview and no embedded browser. `ui-theme` is a deliberately
   fixed theme, but `ui-shortcuts` is **not** covered and an earlier revision of this entry was
   wrong to say so: the reference's row (id `shortcuts`, order 16, `ShortcutsRow`) is an
   **editable keybinding editor**, not a list. Its `Editor.tsx` captures a candidate binding,
   saves it (`save({ type: 'set', id, binding })`) and handles staleness with a retry
   (`ui-shortcuts/src/client/Editor.tsx:57-71`); the same package registers a `shortcuts.open`
   command whose desktop defaults are `primary + Slash` on macOS and Windows alike, rendered
   through a `shell.overlay` (`ui-shortcuts/src/client/index.ts:42-52`). Native has a static
   shortcuts disclosure and hardcoded input handling, so matching this means a binding store,
   an editor with capture and conflict handling, the overlay and the trigger — a functional
   feature rather than a row, and the largest gap this survey has found. **Decided by the user
   (round 101): full editable rebinding is in scope** — the editor, its binding store and its
   conflict handling, exactly as the reference has them. It is authorized explicitly because it
   changes input behaviour the user has otherwise asked to preserve. Settling step: read
   `Editor.tsx` and the package's binding store end to end, then land the store before the
   editor, since a binding that cannot be persisted is not a rebinding.
   That leaves one genuine gap: `ui-settings-session-log` injects a row (order 90) whose
   component is `UploadRow`, alongside a `session-log-upload-toast` overlay — it uploads the
   session log for support. Native has the Trajectory tab but no upload affordance, and this
   is a *functional* feature rather than a layout match — but reading it corrects the first
   guess: this row is **not** an upload action. Its own header calls it the "General settings
   companion for the Host Session-log upload configuration"
   (`ui-settings-session-log/src/client/index.ts:1`), its face is `setEnabled` over an
   `UploadPreference` (`index.ts:26-37`, `upload-preference.ts:17-30`), and the upload itself
   is the Host's, not the row's. So it is an on/off preference write — the same shape as the
   permission default row landed above, through the same settings path — and turning it on
   is the user's own explicit act rather than something this row does on its behalf.
   The transport question is **answered, and it is the cheap path**: the row does not use
   the permission-style preference at all, it uses `ctx.configForms.get(namespace)` with
   `namespace = 'session-log-deepseek'` and a locale of `settings.sessionLog`
   (`index.ts:26-32`). Config forms are the same generic machinery native's plugin settings
   already read and write (`PluginsRead`/`PluginSave` over a `SettingsNamespaceView`), so
   this row reaches the **same wire family** native already speaks — but the mechanism is not
   free to reuse, and an earlier revision of this entry claimed it was. Native's config-forms
   read is scoped to the plugins panel by a `ReadTicket { epoch, panel, serial }`
   (`app/src/plugins.rs:16-21`) driven from that panel's state, so a General-page row cannot
   simply call it: it needs either that read generalised to a second panel identity or its
   own small read over the same namespace family. So the row is one slice — but one that
   includes a scoping change, not a pure UI addition.
   **Answered definitively, and the answer is the cheap one — with a precedent in this very
   tree.** The transport read is namespace-generic: `settings/describe` is called with an
   empty args object and every advertised namespace is projected with no allowlist or prefix
   filter (`transport/src/plugin.rs:279-320`), so `session-log-deepseek` reads like any other
   namespace as long as the Host advertises it as live. The `ReadTicket`'s `panel` is a
   **UI-controller identity, not an authority check** — the worker validates only the epoch
   (`app/src/worker.rs:1007`), and the controller drops a result whose panel does not match
   its own (`app/src/plugins.rs:471-477`). The precedent is the DeepSeek page: it reuses the
   *same* controller and ticket and renders one namespace, with its visibility widened to
   cover a second page — `visible: self.open && matches!(self.page, Page::Plugins |
   Page::ApiLogin)`, whose own comment says the page "renders the one `llm-deepseek`
   namespace, so it needs the same live-settings authority as the plugin panel"
   (`app/src/settings.rs:350-353`, spot-checked; the render call is at `:1266`). The write
   needs nothing new either: `PluginSave` already carries `expected_revision` and the
   transport re-validates field membership, value and writability
   (`transport/src/plugin.rs:547`, `transport/src/http.rs:667-692`).
   So the row is three app-side edits — add `Page::General` to that visibility, call
   `view_namespace` for `session-log-deepseek`, and render its boolean with `PluginSave` —
   and the earlier claim that it needed "a scoping change" was the third wrong guess in a
   row about the same fact.
   **Landed.** `plugin_context` now includes `Page::General` in its visibility, and the
   General page renders the `session-log-deepseek` namespace with the same
   `view_namespace` call the DeepSeek page uses. Verified so far in the state that matters
   most for honesty: with no Host advertising that namespace, the row renders **nothing at
   all** rather than a label for a setting that does not exist, which is the reference's own
   rule for a preference row whose host exposes nothing
   (`shots/71-session-log-row-absent-dp2.png` shows the General page with the permission row
   present and no session-log row). Its *present* state is **not** capture-verified: the
   fixture's backend advertises no namespaces, and faking the plugins controller's snapshot
   is not something this fixture can do, so the visible row rests on the DeepSeek page's
   identical precedent rather than on a grab. Stated as a blocker rather than claimed as
   parity, per this document's rule about unavailable rendering.
   The path is now narrowed another step: the config-forms read does **not** travel through
   the fork supervisor at all — `fork-supervisor.mjs` knows only the bridge commands native
   declares there, and `PluginSettings`-style commands appear nowhere in it — so the
   plugins panel's read is a **Core-level protocol family**, and its `ReadTicket` is the
   likely gate rather than the mechanism. That is good news for cost: if the Core's read is
   generic over a namespace, a General-page row needs a ticket of its own (a second panel
   identity) rather than a new command family.
   Its copy is read and recorded, so only the transport question is open: title **"Upload
   Session Log when using the official model API"**, description **"Help improve DeepSeek
   models and products."**, plus `Preference saved` and `Could not save preference` for the
   two outcomes (`ui-settings-session-log/src/client/locales.ts:1-9`). The row is a switch on
   `upload.value?.enabled === true` with a toast overlay for those two outcomes
   (`UploadRow.tsx:26-48`), and the upload itself is the Host's — so landing the row gives the
   user a control over a Host behaviour rather than moving any data on their behalf.
   Settling step: probe whether a General-page ticket is accepted — mint one and look at what
   the Core validates before answering. If it is, the row is an app-side addition with no
   bridge or Core change; if the panel identity is enforced, the change is to accept a second
   identity. Either way `UploadRow.tsx`'s copy and states are read first, then the row lands
   with its own capture. The correction is recorded because
   "native already has this machinery" has now twice proved only half true: the mechanism
   existed in both cases (config forms here, the `/permission` command line earlier), while
   the *scoping* and the *surface* did not.

9. **The conversation and shell slot inventory — the next audit surface.** The settings
   survey found two real gaps (the permission default row and the session-log preference), so
   the same method applied to the composer, conversation and shell families yields this
   inventory from the reference's own registrations: `shell.overlay` (20 registrations),
   `conversation.session.header.actions` (7), `conversation.input.overlay` (7),
   `conversation.hero.workspace.directoryFlow` (7), `conversation.composer` (6),
   `conversation.chat.turnTail` (6), `conversation.chat.node` (6),
   `conversation.session.header.utilities` (5), `shell.leading` (4), `conversation.view` (3),
   `conversation.trajectory.images` (3), `conversation.plan-review.actions` (3),
   `shell.quota-notice` (2), `shell.bottom` (2), `conversation.session.header.lineage` (2),
   `conversation.session.header.corner` (2), `conversation.message.images` (2) and
   `conversation.input.plan` (2).
   This is an inventory, not a divergence list: native deliberately diverges where the user's
   own layout preferences apply (its ChatGPT-desktop-inspired hierarchy, its Trajectory tab
   standing in for trajectory slots, its composer controls in fixed places), so each slot
   must be checked against native and against those preferences rather than matched
   wholesale. The method that worked for settings is the method here too: read what each
   registration actually renders, then land the functional gaps and record the deliberate
   omissions with their reason.
   **First slot read: `conversation.chat.turnTail`, and it holds three things.** `ui-plan`
   registers a tail row (`id: previewId`) — the plan *preview* inside the conversation, not
   the composer chip native already has; `ui-schedule` registers `schedule-created` (order 20)
   — a scheduled-task row; and `ui-deliverables` registers a third whose package name is its
   id. A fourth registration comes from `ui-chat` itself.
   Against native: the plan preview is a **deliberate difference** in placement, since the
   user's own preference puts the plan in a standing composer chip and the native plan
   projection already feeds it; the schedule row belongs to a **feature native does not
   implement at all** (scheduled tasks), which is an absence of capability rather than a
   missing control to draw; and the deliverables and `ui-chat` rows are **not yet read**, so
   neither is claimed either way. That is the shape of this audit: reading a slot produces
   named rows, and each becomes a deliberate difference, an unsupported feature, or the next
   thing to read.
   **Second read, and it names a real feature gap.** `ui-deliverables` puts a *deliverables*
   row at the turn tail: its injected face carries a changes **diff** (`diffs.state`,
   `loadChangesDiff(sessionId, seq, index)`), a changes **summary**
   (`summaries.state`, `loadChangesSummary(sessionId, seq)`), and a presented-file **opener**
   (`opener.state`, `openPresented(sessionId, seq, index, action, application)`), with a
   `deliverables.file.actions` child list and the whole thing gated on
   `ctx.configForms.developerTools.enabled`
   (`ui-deliverables/src/client/index.ts:60-74`). So the reference shows, after each turn, what
   that turn changed — files, a diff to inspect and a summary — where native shows per-tool
   rows with their own `+N −N` counts and an expandable detail, which is the part the user's
   own screenshot preference asked for and is why the two are not the same thing: native has
   the *counts*, not the *changes view*. That makes this the third category in this audit's
   taxonomy — a functional gap of feature size — rather than a placement difference, and it
   is recorded as such rather than quietly matched, since a diff viewer also decides what
   data the conversation holds.
   `ui-chat`'s own tail registration was **not located** in its package index this round, so
   it remains unread rather than assumed absent.
   **Third read: the session header's action band, and it carries a stated ordering
   convention.** The reference's own comments describe the band — "Static session context
   occupies the header's leading negative-order band", so `ui-agent-preset` registers
   `agent-preset` at `order: -10`, and "Background work follows the preset label in the
   header actions band", so `ui-jobs` registers `job-list` at `order: 20`
   (`ui-agent-preset/src/client/index.ts:216-220`, `ui-jobs/src/client/index.ts:36-42`).
   `ui-subagent` registers a third; `ui-sidebar-terminal`'s registration is **commented
   out** in the reference itself, so its absence is not a gap to close. So the header shows
   what the session *is* (its agent preset) ahead of what it is *doing* (background jobs and
   subagents) — an ordering rule worth matching even where the rows differ.
   Against native: whether it surfaces an agent preset, background jobs or subagents is not
   yet established, and each is a *harness capability* rather than decoration, so this read
   names three candidate gaps of feature size and claims none of them. The ordering rule
   itself is the cheap, faithful part.
   **First row read: it is a label, and it has a sibling.** `ui-agent-preset` registers two
   surfaces from one seat: `AgentPresetLabel` at `conversation.session.header.actions`
   (id `agent-preset`, order −10) and an `AgentPresetSeat` chip at
   `conversation.hero.agentPreset` — the **new-session** chip — both injected from a single
   seat object (`ui-agent-preset/src/client/index.ts:210-232`). So "the agent preset label"
   is really one label plus a hero chip, and the hero one only appears for a session that has
   not started.
   Native's transcript projects **no agent preset at all** today: `reducer.rs` carries only
   the permission preset (`:147-160`), so this row needs a projection first — the projection,
   then the header label, then the hero chip — rather than being the pure-UI addition the
   ordering "smallest first" implied.
   Where the value comes from is now known: the reference reads
   `session?.projectionValues?.agentPreset` (`ui-agent-preset/src/client/seat-store.ts:253`),
   a **session projection value** rather than a transcript record — the same family as the
   `permissions` projection the package's own header mentions. Native reaches the permission
   preset through a `permission/preset` record instead, so the next question is whether its
   transcript or session state already carries projection values; if it does, the projection
   is a small addition beside `permission_preset`, and if it does not, the read has to come
   from wherever native keeps session metadata.
   **Answered, and it is the easy half.** Native already parses projections: `Transcript`
   keeps a `projections: BTreeMap<String, serde_json::Value>` with an accessor
   (`app/src/reducer.rs:96-135`), filled from the snapshot's `projections.values`
   (`:324-329`), and the fixture exercises the same path for the plan and context-pressure
   values (`app/src/layout_fixture.rs:884-898`). So the agent preset's value is reachable
   without any new read — a typed accessor beside `permission_preset`, of the kind already
   written for the plan and permission presets.
   **Accessor landed.** `Transcript::agent_preset()` reads the Host's own projection, taking
   the string form or an object's `id`, and refusing empty, numeric or malformed values
   instead of producing an empty label; its test feeds real snapshot frames and asserts all
   seven cases, so the app suite is **1114 passed** with it. (The first attempt used a closure
   for the emptiness check and failed to compile on lifetime inference; the filter form
   replaced it.)
   **Label landed and capture-verified — on the second attempt, because the first put it on
   the wrong side of the trail.** Reading `ui-subagent`'s own comment settles the band's
   order: subagent lineage "leads the band, directly after the title crumbs", "ahead of Team
   navigation (-20) and the preset label (-10)" (`ui-subagent/src/client/index.ts:83-92`).
   Native keeps that lineage inside the crumb trail itself — the trail already marks a
   subagent crumb at its own size — so the preset label belongs *after* the trail, not before
   it. It now renders there: the grab reads `PUBLIC local conversation` then `implementer`
   (`shots/73-agent-preset-label-ordered-dp2.png`), where the earlier
   `shots/72-agent-preset-label-dp2.png` had the two the other way round. The first version
   was a divergence native *introduced* while matching the reference, which is exactly the
   kind a capture exists to catch. Since the projection carries no display name, the key stands in, as
   the permission chip does without its catalog. Verified with a new `agent-preset` fixture
   mode whose projection carries `{id: implementer, name: Implementer}`: the grab shows
   `implementer` ahead of `PUBLIC local conversation`
   (`shots/72-agent-preset-label-dp2.png`). The first capture attempt failed outright because
   the fixture refuses an unregistered mode, which is the fixture doing its job.
   The remaining half is **not** a cheap sibling, and reading native's hero shows why. The
   reference's hero chip is `conversation.hero.agentPreset`, and the point of that slot is the
   **new-session** default — a session that has not started has no `agentPreset` projection to
   read, so the chip's value comes from the deployment's roster rather than from the transcript
   the label above uses. Native's hero does exist and already carries a chip of the same shape
   — `native-hero-workspace` with the `design::HERO_CHIP_*` constants
   (`app/src/ui.rs:8742-8777`) — so the *drawing* is a small mirror of that row, but the
   *value* depends on the roster read.
   So both remaining pieces of this item — the hero chip and the label's display name — sit
   behind the same missing read: the agent-preset roster, which the package reaches through
   `remote.agentPresets` (`ui-agent-preset/src/client/index.ts:65`). That makes it the same
   three-step shape the permission catalog needed (bridge, Core family, app plumbing) rather
   than one more UI edit, and it is recorded as such before any of it is built. the reference turns the value into display text
   through the agent-preset registry and its `settings.agentPreset` locale
   (`ui-agent-preset/src/client/index.ts:65`, `locales.ts:127`), so native needs either that
   roster read or a fallback to the raw key — the same choice the permission chip already
   made and documented when its catalog is absent.
   **Decided by the user (round 101): all three rows are authorized work** — the agent preset
   label, background jobs and subagents, in that order, smallest first, with the ordering rule
   above governing their placement (what the session is, ahead of what it is doing).
   Settling step: check what native's session header renders today against that band, then
   locate `ui-chat`'s tail registration.
   **Also decided by the user (round 101): the changes view is a deliberate omission.** Native
   keeps its per-tool `+N −N` counts and its detail panel and does not grow a turn-tail diff,
   summary or presented-file opener, so the one feature gap the deliverables read found is
   closed with its reason recorded rather than matched quietly or left unclaimed.

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
  The one-line limitation recorded here first is now closed: `chat_geometry::
  reference_summary_height` measures the joined copy at `human_measure_width(lane)` — the
  same user-stack cap the bubble uses — so every wrapped line is reserved, and the copy
  itself comes from `reducer::reference_summary_text`, one function shared with the view.
  `reference_summary_height_measures_every_wrapped_line` pins it (nothing for no labels,
  more than two lines for a twelve-label list, never fewer lines in a wider lane), and the
  same `chat-references` preset now records `native-reference-summary` at
  **360 × 36** (x 626, y 204.3) in the observation — two 18px lines where the previous
  one-line model reserved half that. `shots/60-reference-summary-wrap-dp2.png` is that
  grab.

- **Permission select — recorded, not landed.** The reference ships a whole package
  (`packages/client/ui-permission-presets/src/client/PermissionSelect.tsx`), not one
  control. Its trigger is the control row's chip: `height: 28px`,
  `padding: 0 4px 0 8px`, `background: transparent`, `border-radius: radius-sm`,
  `font-size: 13px`/`line-height: 20px`/`font-weight: 500`, `color: label-secondary`,
  `max-width: 220px`, hover `interactive-bg-hover`, disabled `label-dimmed`, a 2px focus
  ring, and a 14px glyph on the trigger against 16px in the rows
  (`PermissionSelect.module.css:1-40`). The menu carries the Host's preset catalog
  (`PresetOption` name/description, host copy preserved), three design presets with their
  own glyphs — `read-only`, `workspace-write`, and full access — plus an **Auto review**
  preset that adds a badge and replaces the label and description with locale copy, and
  selecting full access passes through a `RiskConfirmation` step before it commits.
  Native keeps **no permission projection state** at all, so there is no chip, menu,
  confirmation or command to align yet. Its two inputs split in a way worth knowing before
  starting: the **current preset already arrives as a record** — `permission/preset` with
  `data.preset` (`"workspace-write"` in `fixtures/chat-visibility.json`, listed in
  `known.rs`, folded in reducer tests) — while the **catalog is a remote call, not a
  record**: the reference fetches it through `ctx.remote.permissionPresets.catalog()`
  with an ok/error result and its own pending/failure state
  (`ui-permission-presets/src/client/catalog.ts:134-148`). So landing this means a
  transport command wrapper for the catalog (the plan toggle's path is the nearest
  existing shape), a projection for `data.preset`, the chip beside the model and
  reasoning chips with the geometry above, the menu with its Auto-review badge, the
  confirmation guard before full access commits, and the command that changes the preset
  — several items, each with its own capture, rather than one.
  Two more facts from sizing that work. First, **native's command surface is the composer
  line, not a bespoke transport call**: the plan chip sends `/plan` or `/plan off` through
  the conversation (`app/src/plan_control.rs:44-54`, `Control::press`), so a permission
  chip has to find the equivalent. Second, searching the client packages finds **only the
  catalog read** — `permissionPresets.catalog()` is the sole `permissionPresets.` call
  anywhere under `packages/client` — so the mutation that changes a preset is *not* a
  client-side call and its wire shape is still unlocated. That is the first question to
  answer before writing the chip: how a preset change is expressed (a Host command in the
  command catalog, a conversation command, or a remote mutation like the catalog read),
  because that decides whether native can send it through the composer or needs a new
  transport path. **Answered:** the mutation is `permissionPresets.set(session, name)`, a
  remote Host API method alongside `catalog()`
  (`packages/extensions/tool-cordis/src/api-catalog.ts:1689-1703`), and plan mode's
  contrast is explicit in the same catalog — it owns "the `/plan` command"
  (`:1706-1716`), which is why the plan chip could be a composer line and this cannot. The
  feature is therefore bridge-gated, and the bridge is native's own (`core/runtime/*.mjs`, the `codex.mjs` fixed-command pattern), so it has to expose `catalog` and
  `set` before any chip is honest, and the native projection already landed is the only
  part that did not depend on them.

- **Compact table cells — landed, capture-verified.** The table landed with one padding
  pair for both ladders, but the sheet gives the compact one its own:
  `.compact :is(th, td) { padding: 4px 8px; font: inherit }`
  (`ui-primitives/markdown/MarkdownText.module.css:341-346`) against the body's
  `10px 16px`. `design::COMPACT_TABLE_PAD_V`/`_H` and `conversation_content::
  table_padding` now carry both, and the row height, the column width and the rendered
  cells all read that one function, so the measurement and the render move together.
  Verified in the reasoning body at the `chat-thoughts-markdown` preset, whose reasoning
  now carries a two-column table: the grab shows the header and body rules with the
  tighter cells, and `a_compact_table_row_is_shorter_than_a_body_row` pins the exact
  delta — `2 × (10 − 4)` of padding plus the ladder's line box, `22 − 20` — so a future
  change to either side fails the test rather than drifting.
  `shots/61-compact-table-dp2.png` is the grab and `shots/61-compact-table.png` that run's
  own frame.

- **Inline code chip — recorded, blocked by the toolkit.** The reference's inline code is
  a chip, not coloured text: `background-color: var(--dsw-alias-markdown-inline-code)`,
  `border: 0.5px solid var(--dsw-alias-border-l1)`, `border-radius: var(--dsw-radius-sm)`,
  `padding: 0 5px`, `font-size: 0.875em !important` in the mono family
  (`ui-primitives/markdown/MarkdownText.module.css:163-174`). Native draws the run in the
  mono family with the `FOAM` ink but no chip, because an Iced `rich_text` span cannot
  carry a background and the paragraph's wrapping lives in that one text widget; drawing
  a chip would mean splitting a paragraph into a flow of widgets, which the toolkit has
  no inline-flow layout for. Recorded with its values so that whichever comes first — a
  per-span background upstream or a text-layout rework here — has the measurement ready.

- **Permission chip, locked form — landed, capture-verified.** The reference's chip is a
  28px control-row control with `padding: 0 4px 0 8px`, `font-size: 13px`/`line-height:
  20px`/`font-weight: 500`, `color: label-secondary`, a 14px glyph, and a dimmed
  `label-dimmed` form while locked (`PermissionSelect.module.css:1-40`). Native now draws
  it from the Host's own `permission/preset` record — the projection landed earlier — as
  `native-permission-control`: `◈` (U+25C8, the one covered by DejaVuSans out of the
  candidates checked) plus the preset's display name, dimmed and inert, which is exactly
  the state the reference renders for `disabled={locked || leaving}`. Verified at the
  `chat-transcript` preset, whose fixture opens with `permission/preset: workspace-write`:
  the observation records the chip at **28.0 × 137.9** (x 380, y 874), matching the sheet's
  height, and the grab shows `◈ Workspace write` seated after `+ Attach` and before the
  trailing model/send group. `shots/62-permission-chip-dp2.png` is the grab and
  `shots/62-permission-chip.png` that run's own frame.
  Deliberate remainders: the menu is not built yet, so the chip is the locked form rather
  than the interactive one — it will submit `/permission <preset>` as a command line, the
  path the package's header names (`ui-permission-presets/src/client/index.ts:3-11`) and
  the same one the plan chip uses; the display name falls back to the preset key when the
  Host's catalog is absent (its own `optionOf` rule), and the Auto badge and the
  risk-confirmation step still follow.

- **Permission chip menu — landed, capture-verified; its command is wired but unverified
  against a live Host.** The chip is now the interactive control the reference renders
  (`disabled={locked || leaving}` was only its locked form): pressing it opens a 200px
  menu of the three design presets with the current one in full ink and the others dimmed,
  and picking one closes the menu and sends **`/permission <preset>`** through
  `Command::CommandsExecute { session_id, line }` — the exact path the plan chip uses for
  `/plan` (`app/src/ui.rs`, `Message::TogglePlanMode`), and the mutation the package's own
  header names (`ui-permission-presets/src/client/index.ts:3-11`). Verified at the
  `chat-transcript` preset with the new `--permission-menu true` fixture flag: the
  observation records the chip at **28.0 × 137.9** (x 380, y 874) and the menu at
  **200.0 × 90.7** (x 318, y 775), and the grab shows `Read only` / **`Workspace write`** /
  `Full access` above the chip's own `◈ Workspace write`.
  Honest limits: the pick's command line is wired to the same entry point the plan chip
  uses, but no live Host has been driven to confirm the switch takes effect — that needs a
  real session, which is the same category as the other unrun live checks in this
  document. The menu lists the three design presets rather than the Host's catalog (which
  still needs the remote read), the Auto-review badge and the risk-confirmation step are
  not built, and the display-name fallback to the preset key stands.

- **Full-access risk confirmation — landed, capture-verified.** The reference does not let a
  pick commit full access directly: it passes through `RiskConfirmation`
  (`PermissionSelect.tsx:198`, `PermissionRow.tsx:105`) with a title, a warning, an explicit
  acknowledgement and Cancel/Enable (`ui-permission-presets/src/client/locales.ts:34-38`).
  Native now carries the same guard: picking Full access opens that confirmation instead of
  submitting, the copy is the sheet's own verbatim warning, Enable stays inert until the
  acknowledgement is ticked, and Cancel or the chip's own press dismisses it. The preset
  reaches the session through the same `/permission <preset>` line as before.
  Verified at the `chat-transcript` preset with a new `--permission-confirm true` fixture
  flag: the observation records `native-permission-confirm` at **360.0 × 165.1**
  (x 158, y 701) above the chip, and the grab shows the title, the warning, the unticked
  acknowledgement and both buttons. `shots/64-permission-confirm-dp2.png` is the grab and
  `shots/64-permission-confirm.png` that run's own frame.
  The second remainder is closed: the commit button now carries the primary style **only**
  when the acknowledgement is ticked and a dimmed ghost style while it is not, so an inert
  button no longer looks enabled on a safety control. Both forms are captured at the same
  preset — `--permission-confirm true` for the unticked state and `--permission-confirm true
  --permission-acknowledged true` for the ticked one (`shots/65-permission-confirm-enabled.png`
  is the second); a new fixture setter carries the tick.
  The third remainder is closed too: the acknowledgement text now toggles the box on a
  press, the way a checkbox's own label does, because Iced's checkbox carries no label and
  the reference's acknowledgement behaves that way (`locales.ts:36`).
  The presentation remainder is closed as well: the confirmation is now the same **modal
  over a dimmed conversation** the reference renders, using `settings_overlay` — the helper
  the settings and exporter dialogs already share — instead of a panel anchored to the chip,
  with Escape and Cancel both dismissing it. `shots/66-permission-confirm-modal-dp2.png` is
  the grab, showing the scrim, the centred panel and the acknowledgement row.
  No remainders left in the guard itself.

- **Wide-table fixture coverage — landed; its padding match is measured but not yet
  verified visually.** `MarkdownText.module.css:208-210` keeps
  `padding-bottom: var(--dsh-scrollbar-width, 5px)` on a wide table so its hover-revealed
  bar has room below the last row, and native's horizontal branch carried no such padding.
  **Landed** as a five-pixel bottom pad on that branch alone, with the render inspected
  rather than inferred: the first attempt framed the paragraph below the table and was
  reverted rather than committed on an uninspected render, and the second capture frames the
  table itself — `shots/67-wide-table-pad-dp2.png` shows the five-column fixture table with
  its header rules, both rows, and cleared space before the following paragraph. The
  three-column path keeps its geometry, since the pad sits on the horizontal branch only.

- **Checked and dismissed: the wide table's bar reveal is not a CSS divergence.** The
  reference's `.md-table-wide` rules (`AssistantMarkdown.module.css:23-51`) cover the
  breakout width, the negative lead margin and the painted table's z-index; they carry no
  scrollbar rule at all, so the bar's dim-until-hover behaviour comes from platform overlay
  scrollbars rather than from the sheet. Native already models that distinction —
  `design::scrollable_style` separates a lit `Status::Hovered` rail from the resting one
  (`app/src/design.rs:835-845`) — so there is nothing here to match, and a capture cannot
  show a hover state anyway. Recorded so a later pass does not spend a round chasing it.

10. **The session projection keys the reference reads — and two native lacks.** The
   reference's client packages read exactly four session projection values: `agentPreset`
   (seven uses; **landed** above), `subagent` (the lineage the header trail already renders),
   `subagentTiming` (two uses) and `tokenUsage` (one). The last two are the interesting ones
   because they ride the snapshot native already parses into a projections map
   (`app/src/reducer.rs:96-135`), so each is an accessor beside `agent_preset` plus a test —
   the same small slice that item turned out to be — rather than a new bridge read.
   It also corrects the ordering of the three authorized header rows: **jobs are not a
   projection at all**, since `ui-jobs`' row does not appear among these keys, so background
   jobs need a read of their own and are in fact the largest of the three despite being
   listed second.
   Settling step: read the two consumers — whoever reads `subagentTiming` and `tokenUsage` —
   to learn what each value means and where it surfaces, then land the accessors and the
   surfaces they feed. Check first whether native's composer context readout already covers
   `tokenUsage` under its own `contextPressure` projection, since that would make one of the
   two a deliberate duplicate rather than a gap.

   **Both consumers read, and they point different ways.**
   `subagentTiming` feeds `SubagentHeaderLineage.tsx:83-84`, which uses
   `subagentTiming.lastTurnCompleted === true` (`:244`) — so the reference's lineage row, the
   same band native's crumb trail occupies, also carries each subagent's **completion state**.
   That is functional rather than decorative (a running subagent is not a finished one), it
   rides a projection native already parses, and it is cheap: an accessor beside
   `agent_preset` plus a marker on the subagent crumb.
   `tokenUsage` is **not** a session projection at all — it is derived per turn
   (`ui-chat/src/client/conversation-nodes/turn-tail.ts:90-99`, from a `turn/start` event) and
   rendered by `StatsPills.tsx:46`. Per-turn token accounting at the turn tail is exactly the
   kind of chat metadata the user's own rule excludes ("Chat should show only clean
   conversation content"), and native already states context budget in the composer, so this
   one is recorded as a **deliberate omission** rather than a gap — the second such call in
   this document, and the first where the exclusion comes from a stated preference rather than
   from an absent capability.
   Settling step: land the `subagentTiming` accessor and the crumb marker with a capture, and
   leave `tokenUsage` omitted with the reason above.

   **The marker is gated too, and reading it re-orders the queue.** The reference's lineage row
   derives each entry's state from two sources: its `activity` comes from the **catalog
   entry** (`entry.activity === 'running'`), and `completed` additionally requires the
   projection's `subagentTiming.lastTurnCompleted === true`; the three rendered states are
   running, completed and inactive, composed into a secondary line with the title and mode
   (`SubagentHeaderLineage.tsx:236-252`). So the marker needs the **roster read**, not just the
   projection — the same read the hero chip and the display name need.
   That makes the roster the **single unblocking item** for three pieces at once (hero chip,
   display name, subagent activity marker), which is a better ordering than the smallest-first
   list implied: one read unlocks three surfaces, whereas each of the three alone is blocked on
   it. Recorded before building any of them.

   **The roster's Host API is now pinned, so the bridge slice can be written mechanically.**
   `agentPresets` declares `@Remote('list') async remoteExportList(): Promise<AgentPresetRoster>`
   — "Read the selection roster" — alongside `list(): Promise<AgentPreset[]>` and
   `resolve(id?: string): Promise<AgentPreset>` ("Resolve an identity without starting an
   Agent"), plus `register` and `inspectCompositions`
   (`packages/extensions/tool-cordis/src/api-catalog.ts:133-165`). So one `list` read answers
   all three waiting surfaces — the hero chip's default, the label's display name and the
   lineage marker's activity — because the roster carries the identities, their labels and
   their activation state; `resolve` is only needed if a surface must name a preset the roster
   omits, and the reference itself treats an absent catalogue entry as a known-leaf case.
   Settling step: add the bridge command beside `permission.mjs`'s pair (same shape: bounded
   projection, `ok`/error envelope, tests in the package's own node suite), route it in the
   supervisor, then take the Core family and the app plumbing as the catalog did.

   **Bridge half landed.** `core/runtime/agent-preset.mjs` answers `agent-presets` for the
   Host's `agentPresets` service through its own `@Remote('list')` read
   (`api-catalog.ts:133-165`), projecting only the five fields `AgentPresetRow` declares —
   `id`, `isDefault`, an optional `name`, `description` and `broken` (`:4605-4611`) — bounded
   to 64 presets and 4 KiB each, with a roster that carries no default refused outright,
   because without one a client cannot say which preset a new session gets. Its
   `core/runtime/agent-preset.test.mjs` joins the node gate, which now reports **419 pass /
   0 fail**; the Core checks with the bridge embedded, and importing the supervisor accepts
   `agent-presets` with its exact key set while refusing an extra key and a missing
   `requestId`. Formatting caught the new `include_str!` block's indentation, as it did for
   the permission bridge, and the gate is back to 0.
   Settling step: the Core family and the app plumbing, then the three surfaces it unlocks —
   the hero chip, the label's display name and the subagent activity marker.

   **Core family landed.** `core/src/agent_preset.rs` holds the typed client:
   `AgentPresetRow`/`AgentPresetRoster` with the Host's own `isDefault` casing, a projection
   that keeps only the five declared fields, and a roster refused when empty, oversized, or
   missing a default. `Command::AgentPresets` and `PublicReply::AgentPresets` join the
   protocol with their `name()` and `public_result` arms. Core suite **112 passed** with the
   module's three tests, which cover the rendered-fields-only round trip (asserting the
   `isDefault` casing survives), the malformed shapes, and the id bounds.
   One mistake worth recording: the first attempt patched `protocol.rs` but its `lib.rs`
   anchors had been reordered by an earlier `cargo fmt`, so the crate did not compile until
   the anchors were re-derived. The tree was never left broken and never committed in that
   state; the lesson is to prefer anchors that survive formatting.
   Settling step: the app plumbing, then the hero chip, the label's display name and the
   subagent activity marker.

   **App plumbing landed.** `Event::AgentPresets` and `Command::AgentPresetsRead` carry the
   roster on the same worker-event pattern as the catalog and settings reads, `rejected()`
   answers the refusal, the late dispatch marks a bypass, the example's exhaustive classifier
   grew its arm, and the app holds the reply in `agent_roster`. Anchors were chosen
   line-based this round precisely because the previous round's exact-text anchors had been
   reordered by formatting; the script inserted by matching the dispatch line rather than
   quoting it, and every edit applied first time.
   No consumer reads `agent_roster` yet, so nothing sends the read either — the three surfaces
   are the next change, and wiring the request belongs with them rather than ahead of them,
   which is how the permission catalog's plumbing landed too.
   Settling step: wire the request and the first consumer — the hero chip's default, which is
   the roster's own `isDefault` row.

   **First payout, capture-verified.** The request now rides the app's existing proactive read
   (`Command::CommandsList`'s own site), and the header label prefers the roster's own name for
   the projected preset, falling back to the key until the roster answers — the same rule the
   permission chip uses without its catalog. Verified with a fixture-seeded roster beside the
   `agent-preset` projection: the label reads **`Implementer`** where the projection alone read
   `implementer` (`shots/74-agent-preset-name-dp2.png`), which is the roster travelling bridge →
   Core → worker → label as one path rather than four separate claims.
   Remaining from this read: the hero chip's `isDefault` row, and the subagent activity marker
   that needs the catalog entry's own `activity`.

   **A process failure worth recording in full.** The commit that landed this payout reported
   the suites as green, and three tests had in fact failed. The cause was my own command
   chain: `cargo test … | tail -3` returns the exit status of `tail`, not of `cargo`, so a
   failing suite could not stop the `&&` chain and the commit proceeded on a red tree. The
   failing tests were order-sensitive assertions in the command-stream arm where I had put the
   proactive roster read, which the new command shifted.
   The fix landed immediately and is the honest one: the request line was removed, both suites
   were re-run with the exit status captured in a variable rather than through a pipe, and the
   label's display name stands on the part the capture verified. **Every later commit in this
   session must capture `cargo`'s own status**; a piped `tail` is not a gate.
   So the roster's request site is still unwired, and it needs the three order-sensitive tests
   updated alongside it rather than before them — which is why the request is its own slice.

   **Verification audit, with exit statuses captured rather than piped** (the round-115 flaw
   made this the honest thing to do): `fixture` exit 0 / **1114 passed**; `default` exit 0 /
   **1107 passed**; `core` exit 0 / **112 passed**; `transport` exit 0 / **82 passed** plus
   **7 doctests**; the runtime node gate exit 0 / **419 pass, 0 fail**; the format gate exit 0
   / no new debt; tree clean. Transport's exact counts are recorded here because earlier
   reports of this session described it only as "4 test targets ok", which was true but less
   than it claimed to be.
   **Standing rule for the rest of this work:** any gate whose status gates a commit must have
   its exit code captured in a variable. A `cargo test … | tail` returns `tail`'s status and
   cannot fail a chain, which is how a red tree was committed once in this session.

   **The three failing tests were right, and the reason is a discipline native already
   documents.** `request_commands` carries its own comment: it asks the Host for a session's
   commands "once per composer session", and — explicitly — "the trigger is a user action -- a
   leading slash -- so no frame dispatch and no selection does this on its own"
   (`app/src/ui.rs:6214-6230`). My inserted roster read sat in that arm, so it fired on
   selection, which is exactly what those tests exist to prevent. They were not brittle
   order-assertions to be updated; they were the enforcement of a rule, and the correct
   reading of their failure is that the roster's request belongs somewhere else.
   The roster is session-independent, so the faithful trigger is a **one-time read when the
   app becomes ready** (or the first user action that could need a preset name), not a
   per-selection read beside the session's commands. That is the slice's real shape, and it is
   recorded rather than retried in the same place.

   **The epoch read was refused by the suites too, and that is the strongest signal yet.** Put
   it in `Event::Ready`'s epoch-change branch — the same branch that retires the epoch-scoped
   catalog read — and **15 `codex_ui_tests` failed**, every one of them an assertion about the
   exact command stream the app queues after a receipt. So native's discipline is stricter than
   the `request_commands` comment alone suggested: the app's tests hold that *no read is queued
   without a user action*, and a "one harmless read per epoch" is not exempt.
   The change was reverted without being committed (`1099 passed, 15 failed` → back to
   `1114 / 1107`, both exit 0), and the gate check that refused it is the round-115 rule doing
   its job for the first time.
   So the roster's trigger must be a **user action**, like every other read: the hero chip's own
   press and the General page's open are the two faithful candidates, because those are the
   moments a surface actually needs a preset name. That shrinks nothing and speeds nothing —
   but it is where this belongs, and both places are already the ones that would need the
   roster anyway.

   **Hero chip landed, and its press is the read's only trigger.** The hero's chip row now
   carries the reference's `conversation.hero.agentPreset` seat beside the workspace chip: a
   glyph, the roster's own default name and the chevron, dimmed until the roster answers, and
   it renders nothing rather than a guess when there is no roster — no session projection can
   supply this one, which is why the read exists. Its press is the trigger, because the app
   queues no read without a user action (the suites enforce that, twice over now), so the
   action and the need are the same event.
   Verified at the `hero-workspace-selected` fixture with a seeded roster: the grab shows
   `◈ Implementer ⌄` beside `⌂ PUBLIC workspace ⌄`
   (`shots/75-hero-preset-chip-dp2.png`). Both suites green with their exit codes captured.
   Remaining from this read: the selection list behind the chevron, which the roster now makes
   possible, and the subagent activity marker.

   **The seat's selection list landed, and it renders the roster's own states.** Behind the
   chevron the hero seat now opens a 220px card listing every advertised preset: the
   deployment's default in full ink, the others dim, and a declared preset whose activation
   failed in red and **unchoosable** — the reference keeps such a row visible precisely so a
   broken preset is not mistaken for an absent one
   (`ui-agent-preset/src/client/index.ts:210-232`, `AgentPresetRow.broken`).
   **A correction, because the first version of this entry was wrong.** It claimed a pick
   submits `/agent <id>` through the composer, and the code did exactly that. Reading the
   reference shows no such command exists: a pick is staged through the **remote API**,
   `ctx.remote.agentPresets.select(session.id, staged)`
   (`ui-agent-preset/src/client/seat-store.ts:230`). The invented composer line would have
   failed silently against a real Host, which is the worst failure mode available, so the arm
   now closes the list and does nothing, with the reason in the code. Wiring selection is its
   own slice — bridge, Core and app plumbing for `select`, as the roster read needed for
   `list` — and it is not claimed here.
   Verified at the hero fixture with a three-row roster — default, ordinary and broken: the
   grab shows `Implementer` in full ink, `Reviewer` dim and `Scout` red above the chip
   (`shots/76-agent-preset-seat-dp2.png`). Both suites green with exit codes captured.
   Remaining from this read: the subagent activity marker. Then jobs and team navigation.

   **Wire-path audit, prompted by the invented pick.** Every claim this document makes about a
   transport path is now checked against an implementation line rather than a doc comment.
   The permission pick passes: the reference's own picker calls
   `const result = await live.command(\`/permission ${preset}\`)`
   (`ui-permission-presets/src/client/index.ts:126`), the same string this app submits, so the
   chip's command line is real and was previously cited only to the package header
   (`:3-11`) — the implementation is the stronger citation and is recorded here.
   The agent pick failed the same audit and has been removed (`57f6bf3`), because
   `ctx.remote.agentPresets.select(session.id, staged)` is a remote call, not a command.
   The other paths this document asserts were cited from the start: the catalog read and its
   `set` (`api-catalog.ts:1666-1703`), the session-log namespace and its config-forms write
   (`ui-settings-session-log/src/client/index.ts:26-32`, `transport/src/plugin.rs:547`), the
   roster read (`api-catalog.ts:133-165`) and its row shape (`:4605-4611`).
   Standing rule, alongside the exit-status rule: **a path claim needs an implementation line,
   and a doc comment is not one.**

11. **Agent-preset selection — specified, not built.** The reference has two entry points over
   one store: `select(id)` stages and then **immediately applies**
   (`select(id)` = `stage(id)` + `apply()`), and `stage(id, introduce)` stages *without*
   applying for a flow that starts the receiving session afterwards — the settings creator
   entry — because "`select()`'s immediate apply would meet the still-current running session
   and drop the stage as unservable" (`ui-agent-preset/src/client/seat-store.ts:120-150`). The
   apply is the remote call `ctx.remote.agentPresets.select(session.id, staged)` (`:230`); a
   refusal is stored for the chip's announcement **and** returned to callers, and a `busy`
   store short-circuits to `undefined`.
   So the hero seat's pick needs: a staged preset with an `introduce` flag, an apply command,
   and a refusal path that is both shown and returnable. That is bridge, Core and app
   plumbing for one `select` command — the same three slices `list` needed — and the hero seat's
   menu is already built to call it, with its pick arm deliberately inert until it exists.
   Settling step: add the `select` bridge command beside `agent-presets`, then the Core family,
   then replace the inert arm with the staged pick plus its refusal announcement.

## Handoff summary (append-only)

Work landed this session, all of it local and unpushed, with every gate green
(fixture 1114, default 1107, Core 112, transport 82 + 7 doctests, runtime node 419,
format 0):

- **Permission select**, complete against the reference: the chip, the Host-driven menu with
  the `EXP` Auto badge, `/permission <preset>` as the mutation (a real command, cited to
  `ui-permission-presets/src/client/index.ts:126`), the full-access risk confirmation with the
  sheet's verbatim copy and acknowledgement gate, and the new-session default row in Settings —
  all captured.
- **Session-log preference row** on the General page, riding the config-forms path with
  `Page::General` added to that page's visibility, hidden entirely when the Host advertises no
  such namespace. Its *present* state is **not** capture-verified: the fixture's backend
  advertises no namespaces. Recorded as a blocker, not as parity.
- **Agent preset**: the roster read end to end (bridge, Core, plumbing) with three captured
  payouts — the header label's display name, the hero seat, and the seat's list rendering the
  roster's default, ordinary and broken states. Selection is item 11 above.
- **Corrections on record**: an invented `/agent` pick path, removed; the wide-table pad landed
  only after its capture was inspected; a red tree committed once because a piped `tail`
  swallowed a failing suite's status.

Two standing rules, both earned here:

1. **Any gate whose status gates a commit must have its exit code captured in a variable.**
   A `cargo test … | tail` returns `tail`'s status and cannot fail a chain.
2. **A path claim needs an implementation line.** A doc comment is not one — that is exactly
   how the invented `/agent` line survived a commit.

Still needing the user: the reference screenshot at
`/home/jared/Documents/deepseek-harness/default-workspace/reference.png`; a real window-edge
drag for the 520×600 floor; one live close-timing run for the shutdown budgets; and the image
fetch policy. Still blocked on the toolkit: the inline-code chip. Still unsupported and named
as such: scheduled tasks, the deliverables changes view (deliberately omitted), jobs, team
navigation, `subagentTiming`'s activity marker, and editable keybinding rebinding
(authorized, not started).

   **Selection's declaration found, in the registry rather than the catalog** — which is why the
   first probe missed it: the tool-cordis catalog lists the *host service*'s seven methods
   (`register`, `inspectCompositions`, `list`, `remoteExportList`, `resolve`, `readDocument`,
   `mount`, `api-catalog.ts:133-172`) and `select` is not among them, but the preset registry
   declares it: `@Remote('select') async select(agent: Agent, agentPreset: string):
   Promise<string>` — "Select a preset before a session starts its first turn"
   (`packages/preset/agent-preset-registry/src/index.ts:345-346`).
   Its semantics, read rather than inferred:
   - it **refuses a started session** with `RemoteError('agent-preset/locked', 'This session has
     already started', { sessionId, agentPreset })` when the `turnBoundary` projection shows an
     open turn or a completed one (`:347-352`) — an error *code* and message the chip announces,
     which is the refusal the client's store both stores and returns;
   - serialised per agent through a `switches` map, so concurrent picks queue rather than race
     (`:353-361`);
   - on success it **appends a session record** — `agent.session.append('agent-preset/selected',
     { agentPreset: preset.id })` (`:355`) — and returns the committed identity.
   Two consequences for this app, both concrete: the **projection side is a transcript record**
   exactly like `permission/preset`, so reading the selected preset needs an accessor of the kind
   already written twice, not new plumbing; and the **bridge side must resolve an `Agent`**, since
   the method takes one while the client's remote call passes a session id and lets the remote
   layer marshal it. Whether this app's bridge can resolve an agent by session id is the one open
   question, and it is the next probe rather than an assumption.
   Settling step: find how a bridge-side caller resolves an `Agent` (the Host's sessions service
   is the likely answer), then add `agent-preset-select` with the lock code surfaced, then
   replace the hero seat's inert arm and project `agent-preset/selected` for the current value.

   **The probe answers itself against the bridge route.** `select` takes an `Agent`, and `Agent`
   is imported as a live handle from `@deepseek-ai/dsh-agent`
   (`agent-preset-registry/src/index.ts:8`), so the *remote* layer is what marshals the client's
   `session.id` string into one — the registry's own decorator does that work, not its caller.
   A bridge inside the Host process has no such handle to offer, which means **selection should
   not be a bridge command at all**, and the earlier plan for it was shaped wrong.
   The right route is the one native's transport already uses for the other remote families: the
   decorator's name is the endpoint, so `@Remote('select')` is `agentPresets/select`, exactly as
   `@Remote('list')` on the same registry is read through a `settings/describe`-style endpoint
   and `pluginInventory/list` and `settings/mutate` are already wired
   (`transport/src/http.rs:217-218`, `:658-692`). So the slice is transport endpoint, then Core
   family, then the app — three pieces, but a different three from the ones item 11 first named,
   and no bridge change at all.
   Settling step: read one existing transport endpoint pair end to end (`pluginInventory/list`
   is the closest: a read with no arguments) and mirror its shape for `agentPresets/select`,
   carrying the `agentPreset/locked` code through as the refusal the seat announces.

   **The endpoint template is read end to end, so the slice is mechanical.** A transport call is
   one line plus a projection — `plugin_inventory` is
   `crate::plugin::project_inventory(self.invoke(Endpoint::PluginInventory, json!({})).await?)`
   (`transport/src/http.rs:658-660`) — with the endpoint's own name in the `Endpoint` table
   (`:218` for `pluginInventory/list`, beside `settings/describe`, `settings/mutate`,
   `terminal/create` and the rest at `:214-222`), and per-endpoint failure codes mapped where
   the reply is inspected (`:443-460`, e.g. `session/fork-unavailable`,
   `terminal/limit-reached`) — which is where an `agentPreset/locked` refusal belongs.
   So `agentPresets/select` needs exactly three edits to the transport: an `Endpoint` variant
   with its name, a `pub async fn agent_preset_select(…)` calling `invoke`,
   and an error arm. Then the Core family and the app's pick, as every other command has
   travelled.
   **One unknown remains, and it is not guessed:** the wire argument names. The client calls
   `remote.agentPresets.select(session.id, staged)` positionally, while the registry declares
   `select(agent: Agent, agentPreset: string)` — so the JSON keys are a marshalling detail of
   the remote layer, unread, and they must be read from a real request or from the marshaller
   before this endpoint is written. That is the last thing between this specification and a
   slice that can be built without inventing anything.

   **The argument names are settled from a generated artifact.** The registry ships a generated
   remote client declaration:
   `'agentPresets/select': (agentId: SessionId, agentPreset: string) => Promise<RemoteResult<string>>`
   (`packages/preset/agent-preset-registry/lib/typert.remote-client.d.ts:18`), beside a
   session-scoped variant `'agent:agentPresets/select': (agentPreset: string)` (`:24`) whose
   agent is implicit in the endpoint path. So the parameters this app must send are named
   `agentId` and `agentPreset`, from code generation rather than from a doc comment — which is
   the standard this document now holds itself to.
   One detail remains, and it has a settling artifact rather than a guess: native's transport
   documents that some endpoints refuse an unwrapped body as `gateway/arguments-invalid`
   (`transport/src/http.rs:554`) while others take a bare `{"sessionId": …}` (`:895`), so
   whether `agentPresets/select` wants `{"request": {…}}` is a per-endpoint fact. The reference
   ships an end-to-end test that waits on `**/api/agentPresets/select`
   (`apps/web/tests/agent-preset-selection.e2e.ts:455`), whose request body settles it.
   Settling step: read that request body, then write the three transport edits item 11 names.

   **Settled completely, from the generated host descriptor — and it changes the request's
   shape.** `lib/typert.host.js:77-118` declares the endpoint in full: service `agentPresets`,
   method `select`, `invocation: { kind: 'direct' }`, and
   `scope: { context: 'agent', wire: 'agentId' }` with the first parameter
   `{ name: 'agent', wire: 'agentId', source: 'lookup', lookup: 'agent' }` and the second
   `{ name: 'agentPreset', wire: 'agentPreset', source: 'json' }`.
   Read together with the generated client's `'agent:agentPresets/select': (agentPreset: string)`
   (`typert.remote-client.d.ts:24`), that means the **agent is not a body field at all**: it is
   the endpoint's *scope*, carried as `agentId`, and the body is just `{"agentPreset": …}`.
   So the earlier reading — that the body would be `{"agentId": …, "agentPreset": …}` — was
   wrong in the same small way as before, and the artifact settled it rather than a guess.
   The e2e test's named artifact turned out **not** to settle it: it waits on
   `**/api/agentPresets/select` without asserting the body
   (`apps/web/tests/agent-preset-selection.e2e.ts:453-456`), though it does confirm the staged
   semantics and the appended record. Recording that negative matters as much as the positive.
   No unknowns remain for this endpoint: three transport edits, the Core family, the app's pick,
   and the seat announces an `agentPreset/locked` refusal.
