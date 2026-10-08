# Native title-control presentation

## Behavior

- [Filter](<../app/src/transcript_presentation.rs>) hides exact `session/title`/`session/title-llm-request` metadata in Conversation and expanded Session records—not user/assistant text mentioning those names.
- Raw records/export data remain intact; unknown events, cancellation, warnings and Send gates stay unchanged.
- [UI](<../app/src/ui.rs>) applies title baseline/newer admitted live updates to heading, sidebar, search and secondary label. LLM-request payloads never become titles or native model requests.
- Replay/stale generations/gaps/malformed values publish no title. Newer roster wins; removal retires local title.
- Display bounds: **1,024 UTF-8 bytes/session, 4,096 entries**; raw source preserved. No Host/browser/credential/clipboard/desktop operation.

## Qualification and distribution

**Local evidence, ignored on GitHub:** [qualification](<../app/evidence/title-control-ui-0img3pzt/qualification.json>), [retained binary](<../app/evidence/control-layout-3hpjyr1r/compiled-snapshot/dsh-native-app>).

- Eight title tests; **492/499/501/503** default/feature/layout/scroll regressions; all-target/release checks, **31 PUBLIC captures** and default keyless startup.
- Both new frames inspected: updated heading/sidebar, no title JSON in either chat mode.
- Binary: **16,068,400 bytes**, SHA-256 `0d39ed51f3e1e784e8e260b20bf48479a081da35e86cc57ab39ee26274c35a12`.
- [Frozen ac91 development archive](<NATIVE-DEVELOPMENT-PACKAGE.md>) **does not contain this change**; old binaries never inherit later source behavior.
- Read-only fixtures do not qualify physical input, live title-model generation or parity.
