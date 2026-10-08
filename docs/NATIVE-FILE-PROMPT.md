# Native single-file draft and prompt admission

**+ File path → absolute Linux path → Review → Upload → Send.** One local slot, **4 MiB** maximum, using [intake](<NATIVE-FILE-INTAKE.md>)/[upload](<NATIVE-FILE-UPLOAD.md>). No OS chooser, new dependency or primary webview. Wayland drag/drop is unsupported in pinned Iced core 0.14.0; no XWayland fallback.

## Local review and private authority

- [Draft](<../app/src/attachment_draft.rs>) holds path input, tickets/name/size/outcomes—never bodies/receipts. Opening/revision fences allow current edits but reject old Upload/Cancel consent after changes. Validation performs no I/O.
- [App](<../app/src/ui.rs>) shows storage-before-Send/no-deletion warnings. Exact-ticket Remove must reach the worker; queue failure cannot silently erase authority.
- File-only or text+file uses normal Send/model controls. Editing/rejected/unknown/pending file state blocks Send, never silently omits the file.
- Stale Remove cannot discard a newer file. Selection/reload/follow loss hides authority; admitted reads stay reserved until completion/disconnection. Remove/cancel remains available during model restrictions, not inactive modal/dispatched prompt. Matching upload completion may settle behind Settings.

## One explicit prompt attempt

- [Worker](<../app/src/attachments.rs>) alone inserts/consumes private receipts: exact epoch/generation/ticket/Session, ordinary v4 header, enabled Queue mode, caller text ≤32 KiB/eight parts. Generic Prompt cannot inject files; header correlation is not an Agent lease.
- **Never restore a dispatched receipt**, including error/drop/negative ACK/timeout. Slot remains reserved until exact request completion. Closing/delivery loss wins before polling; cancellation after dispatch is **Unknown, not rollback**.
- Matching ACK clears only unchanged submitted draft. Known-not-sent refusal can retain Ready for another explicit attempt; uncertain Send retains text/metadata and blocks Send until Remove. No automatic retry. Foreign/text-only ACKs cannot settle files; serial exhaustion fails closed.
- Durable file summaries show name/size and “preview unavailable,” never receipt/digest/raw objects.

## What Alpha acceptance means

- [Prompt](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/api/session-controller/src/commands.ts#L311-L376>) binds the durable reference, queues/steers and returns `accepted:true`: **inbox admission, not generation/history**. Binding rollback does not undo queued/storage effects.
- [Host binding](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/client/file-upload/src/index.ts#L150-L167>) is reusable until retirement; native policy is stricter. Duplicate detection is pending inbox/history, not permanent exactly-once storage. Same-ID retry is unsafe after claim/rejection/ACK loss.
- Matching user-message history retires receipts; disposal retires maps. Neither deletes bytes. Local Remove is not Host retirement; accumulated Host storage exceeds the local-slot guarantee.

## Verification

**Local evidence, ignored on GitHub:** [manifest](<../app/evidence/parent-native-file-prompt-qualification.json>), [worker proof](<../app/evidence/native-file-prompt-02/native-prompt-qualification.json>), [PUBLIC paint](<../app/evidence/native-file-draft-paint-01/capture-summary.json>).

- **416 default/418 feature tests**, all-target check; 15 draft/eight UI/four receipt/three cancellation/two summary groups. Preserved failures: concurrent scroll compile mismatch; test wrongly counted Settings metadata as file work.
- Three uploads/prompts: **5003-byte text+file, empty file-only, 4 MiB text+file**; three forged-file/three consumed-ticket refusals; independent bytes/SHA-256/modes/hardlinks.
- [Fixture](<../app/examples/support/file_prompt_host_fixture.mjs>)/[worker example](<../app/examples/file_prompt_smoke.rs>) use fresh private public-profile composition; owned root rejects every pre-step. Three balanced blocked turns, real inbox/claims/ACKs and graceful cleanup; zero scoped model/tools/user-message work, not network tracing.
- Three validator/four storage-oracle groups; four PUBLIC renderer states without Host/physical input.
- [Composed App proof](<NATIVE-COMPOSED-FILES.md>) adds real scales 1/1.25. [Upload uncertainty](<NATIVE-UPLOAD-UNCERTAINTY.md>) is not prompt ACK-loss/retry proof.
- Unqualified: real model/history retirement, physical input, chooser/previews, kernel-I/O shutdown delays, release/distribution/performance and parity. Final-symlink refusal is not confinement/atomic snapshot.
