# Upload commit uncertainty and receipt scope

Actual isolated `0.2.1-alpha.1` public-profile qualification, using the unchanged Loader composition, private HTTP authentication, real Agent factory and default local attachment provider. This extends [bounded upload qualification](<NATIVE-FILE-UPLOAD.md>); it does not add native attachment UI or modify shipping Host services.

## Qualified behavior

The [real Host report](<../app/evidence/upload-uncertainty-02/result.json>) and [independent storage report](<../app/evidence/upload-uncertainty-02/uncertainty-qualification.json>) establish:

- **Acknowledgment loss after storage/staging:** a test-only instance wrapper delegates the complete real upload, then holds its return. The client aborts before receiving a receipt; the actual bridge Request signal independently observes disconnect before release. The original receiving Agent still resolves the staged receipt. Canonical bytes and named hardlink alias remain.
- **Disposal after storage, before staging:** a separate wrapper delegates real `saveFileStream`, then holds its completed result. The owned receiving Agent is fully disposed/unregistered before release. Actual raw HTTP responds `ok:false`, `session/not-found`, without a receipt; stored canonical/alias bytes remain. Error message/details are not published by the qualifier.
- **Session-owned authority:** another actual ordinary root cannot resolve or bind the owner's receipt; foreign retirement leaves it intact. Uncommitted binding disposal restores unbound state; temporary binding disposal restores an earlier committed binding. Retiring the temporary request does not delete the receipt; retiring the committed request does. Original Session disposal retires another receipt, and a genuine same-ID resumed Agent has a different Session object and cannot use it.

Receipt tests call `bindPrompt`, guard commit/dispose and `retirePrompt` directly. They do **not** submit a prompt, exercise actual inbox admission or prove queue/history-driven retirement. The runtime behavior above accompanies, but does not independently distinguish every possible implementation of, the source-audited exact-Session WeakMap.

## Cleanup and evidence refusal

[Node qualification](<../scripts/qualify-upload-uncertainty.mjs>) unwraps same-runtime Cordis traceable instances and modifies only their own method descriptors; neither provider replacement nor prototype/global edits occur. Exact descriptors are restored from a retained inventory. A test-only `connection/request` observer awaits the actual delegated bridge continuation: local fetch rejection alone is not Host handler quiescence. Clients are aborted and every gate released before bounded joins, descriptor restoration, observer retirement, owned Agent disposal, registry-baseline verification and Host shutdown. Unchanged production webserver shutdown closes sockets but does not itself join detached HTTP handlers; this test's explicit joins must not be attributed to shipping shutdown behavior.

Actual [ACK-held failure](<../app/evidence/upload-uncertainty-ack-denial-01/uncertainty-qualification.json>) and [store-held failure](<../app/evidence/upload-uncertainty-store-denial-01/uncertainty-qualification.json>) intentionally throw while a real operation is parked. Both child/owned reports remain **failed** with exit 1. Separate parent acceptance requires the exact injection reached/phase, zero admitted work remaining, restored descriptors, disposed handles, registry baseline and zero parent force/timeout/identity-pin failures; only the refusal-and-cleanup qualification passes. These are not successful-upload evidence.

[Python rejection tests](<../scripts/test_qualify_upload_uncertainty.py>) reject wrong booleans/counter types, missing facts, incomplete cleanup, substituted providers, invented surface claims, fixture mismatch/order and injected failures without their exact marker. The shared [storage oracle](<../scripts/qualify-file-upload.py>) verifies complete bytes, lengths/digests, 0400 regular canonical/alias files, hardlink identity and 0700 directories from the owned output through the versioned store. Symlinks at `harness`, `attachments` and leaf paths are refused; this is not adversarial race-proof descriptor-relative filesystem confinement.

The [fresh Rust SDK regression](<../app/evidence/file-upload-sdk-03/result.json>) and [five-case storage regression](<../app/evidence/file-upload-sdk-03/upload-qualification.json>) pass after oracle extraction/hardening. [Parent frozen evidence](<../app/evidence/parent-upload-uncertainty-qualification.json>) records exact source/artifact/report hashes and checks.

## Remaining limits

No scripted prompt/model catalog, sign-in or credential save occurs, and owned subject event/pre-step counters remain zero. Required shared account/credential services still mount; these counters are not process-wide provider/network/credential tracing. Process cleanup is observed PID/start-time/pidfd ownership, not total adversarial descendant containment. The barriers exercise post-real-operation timing in the default provider, not arbitrary providers or every partial-write/cold-resolver cancellation race; the new barrier runs use Node HTTP, not the Rust SDK or GUI.

Treat an admitted upload failure/cancellation/expired acknowledgment as potentially committed: no automatic retry, rollback, object deletion or reuse across Host/Session epochs. Native chooser, draft/selection/epoch binding, image intake/preview, prompt delivery and physical input remain pending. This work does not replace the separately qualified [native UI layout](<NATIVE-CONTROL-LAYOUT.md>) or refresh historical development packages.
