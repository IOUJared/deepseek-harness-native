# Native reader anchors

[Anchors](<../app/src/scroll_anchor.rs>) / [operations](<../app/src/scroll_operation.rs>): plaintext-free geometry, not authority. Uses [chat measurements](<NATIVE-CHAT-GEOMETRY.md>).

## Identity and legal extent

- Capture first intersecting durable row's sequence/intra-row displacement from native measured heights. Restore in **display order**, not numerical seq; replacements can reorder.
- Removed row → first surviving old successor, else predecessor, preserving legal viewport-relative top. No survivor → zero. Shrinking rows/native clamp can prevent exact alignment; same seq does not prove same text.
- Retention fails closed at **4096 records / 16 MiB**, never evicts. Observe post-page/follow projection even on errors; pagination uses opening cut.
- Live preview contributes extent, **not durable identity**. Reusable local `u64::MAX` is not attempt authority; no text matching/partial-to-commit mapping. Capture omits partial shaping; cached live geometry survives.

## Timing and stale actions

- Pinned Iced rebuilds after message-batch drain, then operates on actual widget bounds. Restore clamps rebuilt content/viewport, preserves horizontal offset and returns metadata only; no sleeps/latch.
- Checked geometry revision/selection generation reject old feedback; fresh checked operation serial rejects superseded callbacks. Hidden/remounted panes rotate IDs. Counter exhaustion disables restoration.
- Compare target using actual notification extent; fitting content may be expanded to viewport height. Notification provenance is unknown: target match is a hint, not input proof.
- Reader intent is not silently changed to following near bottom. Differing current scroll can update intent; explicit Latest follows actual bottom.

## Evidence and remaining limits

Ignored evidence is **local-only**, not GitHub downloads.

| Historical evidence | Scope |
|---|---|
| [Corrected standalone operation](<../app/evidence/scroll-anchors-wnhh7qb3/native-operation-corrected.json>) | Native offsets 320/396/0px; callbacks [1,1,1], stale zero. Initial failure wrongly expected fitting content 144px rather than 260px |
| Tests | 16 anchor/operation + eight App model tests; checkpoint 416 default, later 416/418/420 default/feature/fixture |
| [Frozen capture](<../app/evidence/control-layout-sh5wjnfj/qualification.json>) | 20 byte-identical PUBLIC App frames and keyless lifecycle; **not restoration-pipeline execution** |
| [Later App integration](<NATIVE-APP-SCROLL-INTEGRATION.md>) | Real view/update/restore with synthetic frames; key112/intra12; no Host/input |

Counts include concurrent work, not compiler closure. Input/IME/accessibility, live-Host restoration/generation, arbitrary footers, partial identity and rich Markdown/diff parity remain unqualified. Every selected/page frame measures full durable projection, even no-ops; integrated costs need measurement. Archive stays frozen.
