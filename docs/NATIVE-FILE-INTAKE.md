# Native selected-file intake foundation

Headless worker foundation—not chooser/composer UI/preview/prompt binding. Builds on [upload](<NATIVE-FILE-UPLOAD.md>) and [Host uncertainty](<NATIVE-UPLOAD-UNCERTAINTY.md>); later [prompt integration](<NATIVE-FILE-PROMPT.md>) is separate.

## Local intake

- [`SelectedFile`](<../app/src/file_intake.rs>): private consuming absolute Linux path, ≤4096 bytes/no NUL, literal UTF-8 leaf satisfying `FileUpload` naming. Construction performs no I/O; selection is not an inode lease.
- Admitted blocking worker opens `RDONLY|NONBLOCK|NOFOLLOW|CLOEXEC`, requires a regular file and checks size. Reads ≤64 KiB each, **4 MiB** total plus one-byte growth probe; empty/arbitrary bytes supported. Body moves privately into upload; outcomes never echo paths/OS errors.
- Cancellation checked before open, between reads, after intake and before upload. Final symlinks refused; parent symlinks remain possible. No confinement, atomic snapshot or hard regular-file kernel-I/O deadline; NONBLOCK only avoids FIFO open waits.

## Native worker ownership

- [Worker](<../app/src/worker.rs>)/[tickets](<../app/src/attachments.rs>) fence epoch, follow generation, nonzero monotonic serial and exact SessionId. Requires correlated ordinary v4 snapshot; Session lineage is not Agent ownership/live registry lease.
- One active operation or private staged receipt. UI receives only ticket/name/size/closed outcome. Exact discard cancels/removes local authority, **not Host receipt/storage**; foreign/replayed tickets refused. Refused serials do not advance admission.
- Selection/follow loss invalidates authority; cancelled slots remain reserved until the blocking closure joins. Stale success cannot restage.
- Files/tasks/one export share **eight-operation** admission—not a Host/prompt quota. Closures observe local/global/delivery cancellation; upload races them. Joining local work is not SDK/Host quiescence.
- Shutdown invalidates authority, requests Core/client stop, then joins non-abortable reads. Never detach, auto-retry/prompt/delete or claim rollback; unhealthy filesystems can delay shutdown.

## Evidence and regression

**Local evidence, ignored on GitHub:** [successful worker run](<../app/evidence/native-file-stage-02/result.json>), [independent storage](<../app/evidence/native-file-stage-02/native-stage-qualification.json>), [manifest](<../app/evidence/parent-native-file-intake-qualification.json>).

- Production worker/Core/Host: **0/5003/4 MiB** fixtures; pre-snapshot/stale generation refused; foreign discard blocks replacement three times. Full bytes/digests, 0400 files/0700 directories and hardlinks verified; refused fixture not published.
- [85 focused groups](<../app/evidence/file-intake-worker-tests-02.log>): 29 intake/six ticket/four snapshot-budget guards; [360 app tests](<../app/evidence/file-intake-native-tests-03.log>), [consumer check](<../app/evidence/file-intake-consumer-check-01.log>), [seven validator groups](<../app/evidence/file-intake-validator-tests-01.log>).
- Preserved failures: [obsolete 960px anchor](<../app/evidence/file-intake-native-tests-01.log>) corrected independently to 887px; [first run](<../app/evidence/native-file-stage-01/result.json>) referenced nonexistent home, corrected to `user` without recompilation.
- [Example](<../app/examples/file_stage_smoke.rs>)/[runner](<../scripts/qualify-native-file-stage.py>): unchanged supported public composition, owned normal cleanup. Zero scoped model/catalog events—not process-wide tracing/adversarial containment.
- No new release/paint qualification. Actual delayed reads, carrier cancellation, provider/cold-resolver/partial-write races and kernel-I/O delays remain unqualified.

## Chooser and next integration

- Cached rfd 0.17.2 was reviewed, **not adopted**: unjoined async-picker thread, possible Zenity fallback, unbounded/unwrap reader. Future selection must own cancellation/window lifetime; never use that reader.
- No new dependency/helper/MIME/decode/preview. Cached-source review is not latest-upstream verification.
- Later explicit path Review/Upload/Send has separate inbox/paint proof; OS chooser, previews and real model/history retirement remain pending.
