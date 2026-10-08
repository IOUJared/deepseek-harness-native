# Production-App native scroll integration

App view/update/receive/restore against rebuilt widgets, not desktop parity. [Earlier anchors](<NATIVE-READER-ANCHORS.md>) tested model/standalone operation separately.

## Opt-in public rig

- `public-layout-fixture` [rig](<../app/src/scroll_fixture.rs>): production App without subscriptions/worker/Node/Host/business executor. PUBLIC metadata only; queued commands fail and drop; physical business input shielded.
- [Example](<../app/examples/scroll_app_smoke.rs>) executes App Tasks/real scroll feedback. Feature-only raw-container IDs expose independent slot bounds without geometry wrappers.
- Native relative top = `row.y - viewport.y - translation.y`; expected **−12px**, not independent text-clipping/input proof.

## Scripted phases

| Phase | Controlled action |
|---|---|
| Position | key112/intra12 in 80 rows100–179; actual initial offset zero |
| Reflow | Internal frame 1040×800→650×800; genuine paragraph rewrap, **not OS resize** |
| Prepend | PUBLIC rows88–99 via Page receive → 92; no page RPC/ACK |
| Hide | PUBLIC approval opens; transcript widgets absent; no reply |
| Remount | Worker-shaped Root Cancel restores reader |

Advance from current receipt + immediate native observation, never sleeps. **20s** deadline; optional **250ms** screenshot grace affects paint only. One receipt/active phase, none hidden; tolerance **1px**.

## Viewport-only feedback refinement

Changed viewport height plus previous offset's legal clamp preserves follow intent/serial; differing displacement supersedes operations. Three model regressions cover threshold/clamp/input. Notifications lack provenance: content-only or simultaneous input/geometry stays ambiguous; arbitrary footer changes need not immediately snap bottom.

## Large-history extension

[History qualification](<NATIVE-HISTORY-PERFORMANCE.md>) adds bounded `--rows`, `--cycles`, `--idle-ms`, private `--phase-log`; 4096 rows/mid-history key2142, equivalent geometry/paint and two-width caching. Not live-Host/physical scrolling.

## Status and limits

Ignored evidence is **local-only**: [corrected qualification](<../app/evidence/scroll-app-lcbt5m5o/qualification.json>) / [repeated qualification](<../app/evidence/scroll-app-aaincmvb/qualification.json>).

- Five phases passed: top −12px, zero offset error/commands, following=false; heights **87→106px**, offsets **1056→1284→2556px**, receipts **[1,1,1,0,1]**. Hidden widgets absent; final key112/intra12. Repeated paint byte-identical.
- Earlier failures remain failures: invented startup offset; uncompleted resize; compositor tiling **1280×1410**. Controlled frame avoids claiming negotiated resize.
- Latest tests: **426 default / 430 feature / 432 layout / 430 scroll / 16 anchor-operation**. Counts include concurrent work; shared-source formatter still had unrelated differences. Source pins are not full compiler closure.
- No real worker-ticket admission, RPC, generated text, physical input/IME/accessibility, continuous races, adversarial containment or install/update parity. Application/reported scale is not fractional compositor qualification.
