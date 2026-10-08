# Native session ZIP export

## Scope

- **Manage → Export session ZIP… → absolute new destination → Review → Confirm**; dimmed/inert native modal. No automatic path, overwrite, extraction, opener, descendants, deletion, prompt or retry.
- [Controller](<../app/src/exporter.rs>) fences epoch/ordinary Session/generation/panel/editor/attempt; consuming submission. Close clears drafts, **not admitted exports or rollback**. Results expose metadata, not bytes/paths.
- [Worker](<../app/src/worker.rs>): one export within eight-operation budget; excludes key/plugin/Codex/registry writes. **4 MiB** consumer ceiling versus transport maximum 32 MiB.
- Shutdown revokes backend, cancels/awaits downloads and joins separately owned blocking saves before Stopped; never aborts/detaches them. Admitted saves may finish after Close; no universal syscall cutoff/deadline.
- [Authenticated download](<../transport/src/export.rs>), [consuming writer](<../app/src/export_file.rs>): Rustix 1.1.5 safe openat, unchanged existing dependency versions.

## Actual alpha route and contents

- [GET/HEAD `/api/session.export`](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/session-query/session-log-export/src/index.ts#L87-L168>), **not RPC**; exact opaque ID, `includeDescendants=false`, private Cookie/Origin and [operator admission](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/client/connection/src/index.ts#L144-L155>). No launch token, redirects or Content-Disposition filename adoption.
- Root-only still includes canonical `session.v4.jsonl` **and referenced root images/files**. [Serialization](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/session-query/session-log-export/src/archive.ts#L110-L168>) preserves paths/relationships/events: potentially personal/credential-sensitive, not redacted. Host descendants can include ordinary forks, not just subagents.
- [Live flush](<https://github.com/IOUJared/deepseek-harness-linux/blob/native-linux/packages/session-query/session-log-export/src/archive.ts#L80-L92>) may update durable logs before GET/**HEAD**. Cancellation cannot undo it; HEAD is not side-effect-free preflight.
- Default exporter does not resume/prompt, but custom providers/flush listeners are trusted; no packet-trace guarantee.
- Requires sessionQuery/persistence/attachments supplied by unchanged same-version base+web composition. Missing session usually 404; missing services/read errors fixed 500; attachment failure after 200 must not produce Saved.

## Download and ZIP admission

- Deadline covers queue/full transfer; advertised/chunk lengths bounded; revocation rejects queued/in-flight/stale work. Native limits do **not** bound Host root materialization/compression/custom-provider work.
- [Local cached fflate 0.8.3 source—not on GitHub](<../../deepseek-harness-linux/node_modules/.pnpm/fflate@0.8.3/node_modules/fflate/esm/index.mjs#L2081-L2195>): classic ZIP/DEFLATE 8, streaming bit 3/signed descriptors, no ZIP64; local/central DOS timestamps may differ.
- Conservative contiguous local/descriptor/central/EOCD/size/name checks refuse unsupported archives. **No decompression**: structure proves neither DEFLATE/CRC/expansion nor Session identity/digest/sensitivity/extraction safety.
- Non-Clone/nonserializable archive exposes metadata only; owned buffer wiped on release, not allocator/carrier/kernel/Host/writer/swap copies.

## Native file creation

- Exact absolute `.zip`; no trim/append, relative/overlong/control/dot/dotdot/repeated-separator paths. Existing ancestors held with DIRECTORY/NOFOLLOW; no mkdir. Final CREATE|EXCL|NOFOLLOW and exact **0600** precede writes.
- Held parent inode survives rename, not pathname stability/arbitrary confinement. Saved requires exact accounting plus file/directory sync.
- **Failure after creation is indeterminate:** empty/partial/complete private file may remain. No deletion, rollback, reused consent or automatic retry; unhealthy filesystem calls may block.

## Qualification and remaining work

**Local evidence, ignored on GitHub:** [foundation](<../app/evidence/parent-export-foundation-qualification.json>), [parent](<../app/evidence/parent-export-qualification.json>), scales [1](<../app/evidence/export-composed-01/result.json>)/[1.25](<../app/evidence/export-composed-02-scale-125/result.json>).

- 61 transport tests (11 unit/17 HTTP/30 existing/three doctests), seven filesystem tests; later **221 fixture tests** (217 shared/43 export-related + four guards). Earlier main suite 207; runner 12 guards/46 tests. Final export formatting/all-target check passed.
- [Actual-App fixture](<../app/examples/export_composed_smoke.rs>)/[runner](<../scripts/qualify-composed-export.py>): real Saved, independent 0600 ZIP, exact second NotCreated with bytes/inode/timestamps unchanged, graceful zero exit/no observed survivors.
- Independent classic ZIP/DEFLATE EOF/CRC/UTF-8 JSONL/v4-header checks: **449/455 compressed bytes, 563/573 root bytes, four lines**. Four review/saved frames inspected; warnings/confirmation visible.
- Runtime proof belongs to `741b3012…`, not later overwritten `7c514eb7…`; parent pins both. Scripted proof is not physical input, compositor scaling, rich attachments, full semantics/identity or parity.
- Pending: chooser, rich/large exports, sync failures/shutdown-during-save, unhealthy filesystems, accessibility/input and install/update. Revocation cannot undo live flush or admitted saves.
