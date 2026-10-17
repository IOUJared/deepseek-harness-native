# Native shutdown

Close inspects activity, asks for confirmation when needed, then stops only the app-owned backend. Graceful completion requires a shutdown acknowledgement, successful root exit and no observed owned survivors. Zero observed survivors alone does not confirm graceful or complete containment.

[Local attachment reads](<NATIVE-ATTACHMENT-UX.md#local-work>) are canceled cooperatively and joined before exit, including Exit anyway. Backend stop does not detach a pending folder scan or preview decode; unhealthy filesystems can delay local completion.

## Budgets against the DMG shell

The shell stops its Host in three phases and waits longer at each of them: a graceful
`shutdown` message with a **10 s** deadline, then SIGTERM with **5 s**, then SIGKILL with
**5 s**; its `requireGraceful` path additionally demands exit code 0 and a
`shutdown-complete` acknowledgement (`apps/desktop/src/main.ts:3790-3807`). Native's
`StopPolicy::default` uses **3 s / 1 s / 1 s** (`core/src/lib.rs:280-293`), and
`StopPolicy::valid` caps every phase at 10 s, so the shell's figures are reachable
without changing that guard.

Why the numbers differ is not documented anywhere in this repository, which is what makes
this a real open question rather than a settled one. The honest reading: the shell's
larger window is *evidence* that an Electron-plus-Node Host of that shape needs seconds
to unwind a turn, not proof that the Rust Core's Host does — a mid-turn Host may need
more than 3 s to reach a clean acknowledgement, and a Host that is already wedged will
not be helped by any window, only by a later phase firing. The cost of aligning is
directly user-visible: the worst case moves from 5 s to 20 s before a forced kill.

Settling it needs one fact this repository cannot produce: how long a real Host actually
takes to acknowledge a graceful stop mid-turn. The step is a live run — start the app
against the real backend, begin a turn, close the window, and time from the close request
to the process exit with an external timer. **The Core does not log phases**: it reports
only `PublicEvent::Exited` (and the App's one outcome line), so a phase-by-phase breakdown
needs an external observation around the process, not a Core log. Until that number
exists, the native budget stays as it is rather than being changed on the reference's
figures alone.

## Shutdown repairs

- [Core](<../core/src/process.rs>) retries a shutdown frame blocked by a busy control writer within the original grace deadline. Partial/poisoned frames are never replayed.
- A root already exited at the final pre-signal check is not classified as forced merely because a deadline expired.
- The alpha [pi-ai adapter](<../../deepseek-harness-linux/packages/llm/llm-pi-ai/src/adapter.ts>) closes admission, aborts admitted streams and awaits SDK terminal results before retiring owned session resources. Cached Codex WebSockets and five-minute idle timers are cleaned for each owned ID; shared adapter owners retain their cache until the last owner closes.

The safety warning remains for genuinely uncertain stops. Cleanup never broad-signals unrelated processes or treats zero sampled descendants as complete containment. Existing binaries and frozen archives require replacement to acquire the Core repair; model cleanup also requires the compatible rebuilt alpha backend.

## Verification

[Core evidence](<../core/evidence/close-writer-retry-cn4hh0au>) covers two contention cases, 52 default/58 transport tests and six doctests in each configuration. The transient-contention regression failed on the old implementation and passes with retry; a permanently blocked writer remains bounded and reports uncertainty.

[Updated build qualification](<../app/evidence/close-model-refresh-syx9xgy8/qualification.json>) records 507 default app tests, an all-targets check and release build, 25 focused backend tests including real Loader unload, and a fresh blank native Wayland session exiting gracefully with known containment and no surviving observed descendants. [Retained native build](<../app/evidence/close-model-refresh-syx9xgy8/dsh-native-app>) requires the rebuilt compatible alpha backend; the old archive is unchanged. No real model/account request or physical close-button interaction was tested. SDK cleanup initiates socket closure but does not await its acknowledgement; an abort-ignoring SDK can still produce a genuine bounded-shutdown warning.
