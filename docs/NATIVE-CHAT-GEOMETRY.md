# Native content-aware chat geometry

- Bounded plaintext previews use pinned Iced 0.14 native paragraphs: **DejaVu Sans 14px** body (13px secondary), advanced shaping, Word wrap, and an **absolute 24px line** (`design::LINE_BODY`, measured by `design::body_line`). An earlier revision of this line said 15px/19.5px, which the code never drew.
- Content-sized right user bubbles; unboxed left assistants. Fixed **24px** read-only Details header; raw slot **87–145px**, four complete lines/**96px** body cap (4 × `LINE_BODY`), not the earlier partial 75px or 78px caps. Stored records stay unchanged.
- One measured height vector drives virtualization/spacers/rows/prepend. Tools **36px** (24px line + 12px row); opaque metadata **156px**.
- An expanded tool row reserves what it renders. `TOOL_BODY_GAP` (4px) and `BODY_INSET` (28px) in [chat_geometry.rs](<../app/src/chat_geometry.rs>) are now the single owners: the view previously rendered the body at a 16px inset while the reservation measured at 28px, so when the body wrapped to the same line count the measured height came out 4px short of the rendered one and the row's bottom was clipped. Measure and render read the same two constants.
- An **answerless** assistant reply (thinking or a recorded tool call, no answer) measures to its thoughts disclosure alone: the 20px empty-body minimum and the 22px action row are both omitted, because the view renders no body frame and no Copy/Details for it.
- [Cache](<../app/src/chat_geometry.rs>): **4097 entries** (4096 durable + live), clears on selection, invalidates role/preview changes. [Later refinement](<NATIVE-HISTORY-PERFORMANCE.md>) stores two recent scalar widths per shared preview, LRU eviction; user/assistant caps **640/800px**, tool/opaque actual widths. Diagnostics omit text. Dynamic fonts would require cache recreation.

## Qualification scope

Ignored [parent qualification](<../app/evidence/control-layout-lkwnfxtd/qualification.json>) is **local-only, not a GitHub download**.

| Historical check | Scope |
|---|---|
| Standalone probe | Six native-font/cache tests, no App/Host |
| Synthetic release microbenchmark | 4096 rows: ~112.4ms cold incl. font initialization; 0.376ms mean warm, ten passes |
| App tests | 360 default / 362 feature / 364 fixture; all-target check/builds |
| Actual-App capture | 20 PUBLIC frames + keyless lifecycle; changed frames inspected, 14 unchanged byte-identical |

Microbenchmark excludes App excerpt/widget layout; **not startup/scroll/CPU/memory performance**. Stable widths avoid shaping; histories >128 rows snap sidebar rather than per-animation reflow. Continuous resize still recomputes.

Earlier [development package](<NATIVE-DEVELOPMENT-PACKAGE.md>) has its own frozen identity; captures do not qualify attachment effects or physical input.

## Reference comparison at 1980×1319 (blocked on the file)

The user's reference screenshot is **not in the workspace**, so visual parity is unverified. The
expected artifact is a 1980×1319 PNG with sha256
`b8e164f53665c2e85bad4b326c4c0b6decff0f1fc37d7f93fb64a0cbff4759ce` at
`/home/jared/Documents/deepseek-harness/default-workspace/reference.png`. To place it, the user can
copy it there and verify it with:

```sh
sha256sum /home/jared/Documents/deepseek-harness/default-workspace/reference.png
```

**The native side is captured at exactly that size already**, so the comparison is a diff and not a
re-capture: `shots/171-chat-reference-size-folded.png` (DP-2 alongside, fixture report
`status: passed`) is the chat fixture at `--width 1980 --height 1319`, re-taken after round 371
folded each tool result onto its call. `shots/157-chat-reference-size.png` is the same frame before
the fold (six activity rows instead of three) and stays as history. The rects below were measured
from the 157 frame; none of them is a tool row, so the fold did not move them. The product frame is `y=32, 1980×1319`; the 32px
band above it is the fixture's own PUBLIC banner, not product chrome. Measured native rects from
that run:

| Element | Rect (logical px) |
| --- | --- |
| Frame | x 0, y 32, 1980×1319 |
| Sidebar / header split | header starts x 260 |
| Header | x 260, 1720×75 |
| Header crumbs | x 280, y 43, 198×28 |
| Content lane handles | x 626 and x 1604, 10 wide, y 136.3, h 1095.7 |
| Composer controls row | y 1293, h 28 (attach, permission, model) |
| Stop | x 1448.95, 51.28×34 |
| Send | x 1512.23, 75.77×34 |
| Settings entry | x 12, y 1309, 236×36 |

**What to compare, in this order** (each is a user-stated requirement, so a mismatch is a defect,
not a preference):

1. user bubble: right inset from the lane edge, content width, and the gap to the next row;
2. assistant text: the first line's x and y relative to the lane, and the absence of any box;
3. tool activity: single muted line, icon column, name, underlined **relative** path, and the
   `+N −N` edit pair where the reference shows one;
4. the action row under an assistant reply (branch, like/dislike, details, time) and its rhythm;
5. the composer card: width against the lane, its 32px overhang, control row height and baseline.

**Known open divergences that do not need the image.** The reference's own source defines the
underlined file path and the `+N −N` edit pair, and both are now drawn: the path is
`relativizeToCwd` then `abbreviateHomePath` of the recorded argument (round 354, `shots/158-…`),
and the totals follow the reference's hover/expanded cue rules. Two things from that family remain:
the reference's path link opens the right Sidebar's document preview, which this clone replaces with
the details panel, so the native path is display-only, and the edit row's inline diff card is drawn
but carries none of the reference's code toolbar (see "Remaining limits" below).
Their *exact* rhythm stays unverified until `reference.png` lands.

## Remaining limits

- Preview: **four source lines / 240 code points / four displayed lines**, not full-message height/Markdown. Long unbreakable tokens may clip; Details preserves bounded display source. [Formatted Details](<NATIVE-FORMATTED-DETAILS.md>) is optional/basic.
- The inline diff card is drawn for an `edit`/`write` call (round 362): path header, added and
  removed rows with the reference's 3px bar and tint, `⋯` between hunks and a 9-row cap with its
  fold. The reference's card also carries its code toolbar (language, wrap, copy), which native's
  expanded tool body does not draw. The underlined workspace-relative path and the edit `+N −N`
  pair are drawn (round 354); the path is not a separate open link.
- [Reader anchors](<NATIVE-READER-ANCHORS.md>) separately address durable reflow/neighbor fallback, not partial/replaced text identity. Retention fails closed.
- Live resize/input/IME/accessibility, compositor fractional scaling, real models/credentials and full parity remain unqualified.


## Rows must reserve, and paint, a line (rounds 391–392)

Two failures made a Chat row invisible while every test stayed green, and both are now rules.

**Reservation.** A row is drawn only inside the scroll projection's height range, so a geometry of
zero removes it from the frame entirely. `every_preset_row_reserves_its_own_line` now builds every
PUBLIC preset, walks each Chat row, and asserts a positive height and width — the class of bug is
checked, not just the two rows that had it.

**Paint.** A `Fill`-height spacer inside a shrink-to-fit `Row`
(`Space::new().width(Length::Fill)`) collapses the row's own height, so the row reserves its space and
paints nothing. The command row hit this in round 391: the same text as a plain label painted, while
the row composition drew empty, and neither forcing the spacer's height nor splitting the disabled
button changed a pixel. Rows that carry one line of text therefore use a single label; a row built
from parts must set its own `.height(...)`, as the tool row does. A capture is the only check for
this — geometry assertions cannot see it.
