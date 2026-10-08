# Native decision layout

## Fixed controls, scrollable request content

- [Production sheet](<../app/src/interactions.rs>): centered, parent-constrained **760×720** cap; outer/sheet padding **12/20** logical units. Header/navigation and terminal controls are fixed siblings of the Fill-height request scrollable.
- Long requests/full errors scroll; footer uses a short static status. Explicit question validation, worker/epoch/attempt fences and ACK retirement are unchanged.
- Close is local; question Cancel is business cancellation. Render/open/close emits no answer, claim, delegation or permission change. **Timed/continued UI is unsupported**.
- Native widget allocation requires finite parent limits and room for chrome; no arbitrary tiny-window/all-scale guarantee or compositor settings change.

## Parent results

Ignored evidence is **local-only, not GitHub downloads**: [qualification](<../app/evidence/parent-decision-layout-qualification.json>).

| Check | Historical result |
|---|---|
| Tests/builds | 277 App tests, all-target check, two offline locked release builds, six Python oracle tests |
| Real composed paint | Scales 1/1.25 and requested 608×448 parent at 1.25; six inspected frames with complete fixed controls |
| Each composed run | Three native ACKs/four Host settlements, exact root/roster, closed zero-step turn, disposal, graceful zero exit/two observed processes |
| Production lifecycle | One 20-second blank-profile run, scale 1.25; native Wayland/blank fold/clean exit |

Production late-tree medians (31 samples): **RSS 333,816 KiB / PSS 262,627 KiB / DRM VRAM 57,776 KiB**. CPU median/mean/max: **0/0.37/1.91%** of one core. Map proxy **~555ms**, not first-frame timing or measured layout improvement.

## Verification scope

- Null-renderer tests exercise shell slots with short/8192-unit bodies at **608×448, 760×560, 1000×800**; not font shaping/input/paint.
- [Real fixture](<../app/examples/decisions_composed_smoke.rs>) uses production widgets. `--viewport minimum` requests internal **608×448** constraints, not OS resize. Runner validates declarations/binary, **not independently measured effective bounds**; mapped window was 1272×1402. PNG presence alone cannot prove controls visible.
- [Earlier composed evidence](<NATIVE-COMPOSED-DECISIONS.md>) retains its clipped footer/older hashes; it does not inherit this fix.
- Physical input/scroll/focus/IME/accessibility, compositor fractional scaling, every approval/tool outcome and representative integrated performance remain unqualified. No new archive/installer.
