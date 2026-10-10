# Native shutdown

Close inspects activity, asks for confirmation when needed, then stops only the app-owned backend. Graceful completion requires a shutdown acknowledgement, successful root exit and no observed owned survivors. Zero observed survivors alone does not confirm graceful or complete containment.

[Local attachment reads](<NATIVE-ATTACHMENT-UX.md#local-work>) are canceled cooperatively and joined before exit, including Exit anyway. Backend stop does not detach a pending folder scan or preview decode; unhealthy filesystems can delay local completion.

## Shutdown repairs

- [Core](<../core/src/process.rs>) retries a shutdown frame blocked by a busy control writer within the original grace deadline. Partial/poisoned frames are never replayed.
- A root already exited at the final pre-signal check is not classified as forced merely because a deadline expired.
- The alpha [pi-ai adapter](<../../deepseek-harness-linux/packages/llm/llm-pi-ai/src/adapter.ts>) closes admission, aborts admitted streams and awaits SDK terminal results before retiring owned session resources. Cached Codex WebSockets and five-minute idle timers are cleaned for each owned ID; shared adapter owners retain their cache until the last owner closes.

The safety warning remains for genuinely uncertain stops. Cleanup never broad-signals unrelated processes or treats zero sampled descendants as complete containment. Existing binaries and frozen archives require replacement to acquire the Core repair; model cleanup also requires the compatible rebuilt alpha backend.

## Verification

[Core evidence](<../core/evidence/close-writer-retry-cn4hh0au>) covers two contention cases, 52 default/58 transport tests and six doctests in each configuration. The transient-contention regression failed on the old implementation and passes with retry; a permanently blocked writer remains bounded and reports uncertainty.

[Updated build qualification](<../app/evidence/close-model-refresh-syx9xgy8/qualification.json>) records 507 default app tests, an all-targets check and release build, 25 focused backend tests including real Loader unload, and a fresh blank native Wayland session exiting gracefully with known containment and no surviving observed descendants. [Retained native build](<../app/evidence/close-model-refresh-syx9xgy8/dsh-native-app>) requires the rebuilt compatible alpha backend; the old archive is unchanged. No real model/account request or physical close-button interaction was tested. SDK cleanup initiates socket closure but does not await its acknowledgement; an abort-ignoring SDK can still produce a genuine bounded-shutdown warning.
