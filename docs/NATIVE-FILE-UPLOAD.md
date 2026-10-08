# Native bounded file upload

Isolated `0.2.1-alpha.1` backend intake—not attachment UI or desktop parity.

## API and ownership

- [`FileUpload`](<../transport/src/upload.rs>): consuming `Vec<u8>`, empty/arbitrary bytes, **4 MiB** maximum. Optional leaf: 1–1024 UTF-8 bytes; no controls, separators, `.`/`..`. No path/read/decode/Clone/serialization/body getter; Debug reports size. Copies are not wiped.
- `NativeClient::upload_file(&SessionId, FileUpload)` POSTs only `/api/session/uploadFileBinary?sessionId=…&name=…`: percent-encoded query, octet-stream, exact length, private Cookie/Origin. No multipart/base64/redirect/arbitrary endpoint/AgentId; separate from JSON RPC limits.
- **Eight** generation-owned queued/active workers share the RPC semaphore. `QueueFull` is pre-send; owned bodies ≤32 MiB, excluding caller/unpolled/carrier/kernel copies—not process/Host quotas.
- One absolute deadline covers scheduling through complete decoding. Caller drop aborts; owner close/invalidation cancels/drains parked upload workers; stale completed results are refused. Other RPC/export futures still require polling/drop before close.
- Receipts: ≤16 KiB/128 JSON values or smaller configured limits; exact fields/typed IDs/name/byte count. Errors never expose remote diagnostics/URLs. [`FileUploadValue`](<../transport/src/dto.rs#L641-L653>) contains sensitive receipt authority/metadata; do not publish/log indiscriminately. Caller retains Session/view ticket/epoch; replies echo neither Session nor Agent.

## Exact alpha behavior

- [Route](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/client/file-upload/src/http-route.ts>)/[FileUploads](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/client/file-upload/src/index.ts>) checks live registry, otherwise may **resume a cold ordinary Agent**. Cancellation does not reach that resolver; discovery is not an admission lease.
- Live subagents are refused; cold resolver additionally refuses owned children. No MIME/image/model validation or stock streaming cap; 4 MiB is native policy, not Host-wide.
- Receipt staging is exact-Session-object-owned, not prompt admission/history/generation. Job limits do not cap accumulated receipts/storage.
- [Default local provider](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/attachment/attachment-local/src/file-store.ts>) preserves content-addressed bytes, sanitized names and hardlink aliases. Failure/abort/replacement/lost ACK can leave storage: **no rollback, automatic retry or deletion**. [Uncertainty qualification](<NATIVE-UPLOAD-UNCERTAINTY.md>) proves those scoped outcomes.

## Evidence and limits

**Local evidence, ignored on GitHub:** SDK/storage reports [02](<../app/evidence/file-upload-sdk-02/result.json>)/[02 storage](<../app/evidence/file-upload-sdk-02/upload-qualification.json>), [03](<../app/evidence/file-upload-sdk-03/result.json>)/[03 storage](<../app/evidence/file-upload-sdk-03/upload-qualification.json>), [manifest](<../app/evidence/parent-file-upload-qualification.json>).

- Two real Host epochs: live/cold intake, unknown-target refusal, five fresh receipts, two clean exits. Independent complete bytes/digests/empty/4 MiB/name/dedup/hardlinks, 0400 files/0700 directories; default-provider scope only.
- Snapshots contain policy metadata plus cold-resume `session/end-seed`, not network/credential tracing.
- [Wire fixtures](<../transport/tests/support/upload_wire.rs>) cover bounds/deadlines/cancellation/carrier closure/stale results; [denial tests](<../scripts/test_qualify_file_upload.py>) cover misleading metadata/modes/bytes/symlinks.
- Later [intake](<NATIVE-FILE-INTAKE.md>), [prompt drafts](<NATIVE-FILE-PROMPT.md>) and [control layout](<NATIVE-CONTROL-LAYOUT.md>) have separate qualification; historical packages inherit none automatically.
- Pending: OS chooser/drop, images/previews/output files, real model consumption/history retirement, physical input and integrated performance.
