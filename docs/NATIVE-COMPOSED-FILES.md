# Native composed file-flow qualification

Scripted production App → Worker → Core → isolated Alpha Host; actual uploads/ACKs, not simulated metadata. [Composer](<NATIVE-FILE-PROMPT.md>) remains explicit absolute-path Review/Upload/Send—not chooser. Installed profile, desktop, dependencies and production Alpha services stay unchanged.

## Actual consumer path

- [Driver](<../app/examples/file_composed_smoke.rs>) runs real App boot/update/subscription/view and worker in an own Iced/wgpu Wayland window. Helpers inspect readiness/consent/tickets/generated request ID; never inject state/receipts/ACKs.
- Physical business/editor/account/model messages are dropped; window/layout feedback remains forwarded.
- Script waits for real registry/root/ordinary v4 snapshot, rejects old Upload consent, stages with matching ticket/name/size, then uses production Send and matching real ACK.
- Independently waits for matching durable inbox/file claim/blocked turn before proceeding.
- Caller-file and Steer probes return NotSent without consuming Ready. Duplicate Send, stale Remove and replayed prior real ACK cannot clear newer drafts. Callback replay is not duplicate wire delivery or global exactly-once proof.

## Test-owned Host and storage evidence

- [Loader fixture](<../app/examples/support/file_prompt_host_fixture.mjs>) preserves full public base+web composition in fresh private homes. First valid insertion binds App request/process/generation/serial **1/2/3**; exact text/file references, emit identity and inbox removal are checked.
- Only owned registered root is intercepted; every pre-step rejected, unrelated Agents delegate. Requires three balanced blocked turns, zero model steps/request headers/tools/user/assistant messages and clean disposal/teardown.
- Admission is not generation, user-message history or receipt retirement; zero scoped counters are not network/credential tracing.
- [Runner](<../scripts/qualify-composed-files.py>) correlates private Host receipts and independently verifies **5003-byte text+file, empty file-only, 4 MiB text+file** bytes/SHA-256, hardlinks, 0400 files/0700 directories. Unsubmitted fixture absent; default-provider scope, **not rollback/deletion**.

## Qualified results and paint scope

**Local evidence, ignored on GitHub:** scales [1](<../app/evidence/native-composed-files-03/composed-files-qualification.json>)/[1.25](<../app/evidence/native-composed-files-04/composed-files-qualification.json>), [headless regression](<../app/evidence/native-file-prompt-03/native-prompt-qualification.json>), [manifest](<../app/evidence/parent-native-composed-files-qualification.json>).

- Each run: three stages/ACKs/claims, six probe/six duplicate-Send/three stale-consent refusals and two each stale-Remove/prior-ACK refusals.
- [424 driver tests](<../app/evidence/file-composed-tests-03.log>), including five wrapper guards; [13 denial groups](<../app/evidence/file-composed-validator-tests-01.log>); [consumer check](<../app/evidence/file-composed-consumer-check-02.log>).
- Six inspected own images: visible consent/Upload/Cancel, ready rows/Remove/Send, one Settings/Stop and PUBLIC editor glyphs; **1272×1402** physical buffer at both app scales. PNG-header validation and human paint inspection are separate.
- Earlier 01/02 captured no editor glyphs despite actual admission: screenshot followed editor rebuild/old Weak primitive. Cancellable silent capture delay fixed this fixture, not production input or atomic frame fencing.
- [Later record projection](<NATIVE-RECORDS-PROJECTION.md>) adds actual disclosure roundtrip/fourth screenshot; old raw-record captures/hashes stay frozen. ACK never invents a user bubble.

## Remaining limits

- Pending: chooser/drop, images/previews/downloads, physical input/accessibility, compositor fractional scaling/tiny geometry, real model/history retirement, prompt ACK-loss races and representative performance/install/update/parity.
- Reads/screenshot writes remain owned/joined; bad kernel I/O can delay shutdown. Debug fixtures do not qualify a fresh release or distribution.
