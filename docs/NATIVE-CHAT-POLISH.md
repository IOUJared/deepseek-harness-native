# Native chat polish

- Content-sized, right-aligned neutral user bubbles; unboxed assistant replies.
- No repeated role header on ordinary messages. Live and interrupted responses keep an explicit status line; interrupted status is red.
- Copy, Edit and Details share a small muted action row. Truncated user previews show Read more. Full reply formatting and reasoning disclosures are unchanged.
- Composer uses a shorter editor and one control row: model, Attach, Stop and Send. Usage and keyboard hints sit quietly below it. Healthy draft-save status no longer occupies a row.
- Draft failures, loading, uncertain submissions, read-only warnings, attachment review and consent/cancellation remain visible. Enter/Shift+Enter/Ctrl+Enter and explicit file-only Send are unchanged.

## Geometry and verification

`chat_geometry` supplies one shared header/padding contract for native measurement, virtual row spacers and search offsets: routine body top 18px; live/interrupted body top 49px. The App adds the 22px action row/gap. Body width, font, wrap and tool-row geometry are unchanged.

Owners: native geometry/cache tests, transcript spacer/page-anchor tests, search/reflow and input reducer tests, plus PUBLIC `layout_smoke` screenshots. These checks do not establish physical keyboard/IME input or live provider execution. Concurrent integration changes must be qualified separately from a chat-only frozen snapshot.
