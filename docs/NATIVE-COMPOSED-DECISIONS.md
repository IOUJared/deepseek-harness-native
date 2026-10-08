# Composed native live decisions

## Developer fixture scope

- [Example](<../app/examples/decisions_composed_smoke.rs>) uses actual App/worker/reducers/renderer and full same-version public alpha profile, plus a [fixture Loader module](<../app/examples/support/decision_host_fixture.mjs>) and fixture-local approval `ask`. No fabricated events/Agent IDs or replacement Cordis tree.
- [Runner](<../scripts/qualify-composed-decisions.py>): fresh canonical private evidence directory, isolated HOME/XDG/data, cleared environment, pidfd-owned cleanup, scales 1/1.25. Private loopback authentication still exists; no real account/API credential.
- Physical business messages are dropped; fixed scripts drive actions. Own-renderer screenshots are not desktop/input/IME/accessibility or compositor fractional-scale qualification.

## Real request path

A real returned root Agent wakes once with PUBLIC input. A rejecting pre-step guard holds one driver-owned turn:

| Request | Required settlement |
|---|---|
| Nonexistent fixture operation approval | Allow once; real approval audit pair, no tool execution |
| Ordinary question | Multi-selection, PUBLIC custom text, explicit skip |
| Second question | Native question-only Cancel → `ASK_CANCELLED` |
| Final question | Host signal → root Cancel + `ASK_ABORTED`; no native reply or parent-turn abort |

Gateway delivery → exact native `EventDelivery` → worker key/validation → actual App Interaction dispatch. Each phase separately waits for matching native ACK **and Host promise settlement**. Agent identity comes from the returned object, not roster.

After rejection: one closed turn, matching approval pair, **zero admitted steps/request headers/tools/assistant messages**; await idle/dispose. Observation disconnect alone is not business cancellation.

## Parent-run results

Ignored [parent evidence](<../app/evidence/parent-composed-decisions-qualification.json>) is **local-only**: both scales passed three ACKs/four settlements, native Wayland, exact audit/root/roster, disposal and graceful zero exit/two observed processes/no survivors. Three Rust/five Python guards passed; **273 shared tests were filtered, not rerun**.

Four inspected pending-request frames: scale-1 controls readable; scale-1.25 question footer extended below viewport. No entered-answer paint/physical scrolling claim. [Later layout qualification](<NATIVE-DECISION-LAYOUT.md>) fixes controls with newer pinned binaries; historical captures remain unchanged.

## Qualification and limits

Runner requires correlation, PNG/privacy/bounds, independent settlement, closed turn and owned cleanup—not script/PNG presence alone. Full composition still mounts account/Codex; prompt assembly precedes pre-step. Zero admitted fixture steps is **not process-wide network/model/account tracing**. Reject/Cancel approvals, tools, timed/continued UI, multi-client/reconnect, physical input and parity remain separate gates. [Ownership](<NATIVE-DECISION-OWNERSHIP.md>) / [claims](<NATIVE-TIMED-QUESTIONS.md>).
