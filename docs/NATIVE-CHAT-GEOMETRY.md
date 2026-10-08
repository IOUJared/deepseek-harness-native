# Native content-aware chat geometry

- Bounded plaintext previews use pinned Iced 0.14 native paragraphs: **DejaVu Sans 15px**, advanced shaping, Word wrap, **1.3 line height / 19.5px**.
- Content-sized right user bubbles; unboxed left assistants. Fixed **24px** read-only Details header; raw slot **87–145px**, four complete lines/**78px** body cap, not earlier partial 75px cap. Stored records stay unchanged.
- One measured height vector drives virtualization/spacers/rows/prepend. Tools **40px**; opaque metadata **160px**.
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

## Remaining limits

- Preview: **four source lines / 240 code points / four displayed lines**, not full-message height/Markdown. Long unbreakable tokens may clip; Details preserves bounded display source. [Formatted Details](<NATIVE-FORMATTED-DETAILS.md>) is optional/basic.
- Inline diffs, underlined relative paths and real edit counts remain unsupported.
- [Reader anchors](<NATIVE-READER-ANCHORS.md>) separately address durable reflow/neighbor fallback, not partial/replaced text identity. Retention fails closed.
- Live resize/input/IME/accessibility, compositor fractional scaling, real models/credentials and full parity remain unqualified.
