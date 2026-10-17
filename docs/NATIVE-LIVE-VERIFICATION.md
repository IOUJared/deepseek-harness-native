# Live Host verification

Two live passes and a read-only wire audit of the native `@` / schedule / feedback / workspace /
picker paths and the remaining non-model commands against a **running Host**, not a fixture. The
first pass found three wire defects, all fixed in `c9dd30e`, and one composition limit that changes
what the workspace picker can honestly claim; the second exercised the queue, goals, attachment,
terminal, registry, model-selection, cancel and command paths and found no defects; the audit found
two more divergences (`agentPresets/select`, `job/follow`), both fixed.

## What ran

- Runtime: the pinned sibling checkout `deepseek-harness-linux` at `c6044cc50b` (only generated
  docs were modified in that worktree), CLI `@deepseek-ai/dsh` **0.2.1-alpha.1**, Node
  **v26.11.1**.
- Composition: the exact desktop boot the app's fork supervisor performs —
  `loadForkRuntime` from `core/runtime/fork-supervisor.mjs` (isolated home, desktop profile,
  `dsh-base` + `dsh-web-app`, owner patch) then `runProfile` with
  `--no-open --host 127.0.0.1 --port 0`.
- Environment: `env -i HOME=<temp> PATH=/usr/bin:/bin`, `DSH_TAURI_HOME`, `DSH_TAURI_RUNTIME`,
  `DSH_TAURI_EXPECTED_VERSION`, `DSH_TELEMETRY_DISABLED=1` — the same cleared environment Core
  gives the Host, in particular **no `DISPLAY`/`WAYLAND_DISPLAY`**.
- Access: the printed ready URL's launch token was exchanged for the `dsh-auth-*` cookie with
  redirects disabled; every call was a `POST /api/<namespace>/<method>` ClientRPC envelope with the
  exact `Origin`. The temp home, workspace and cookie were private and were not the user's profile.
- An earlier probe also ran the shipped `web` profile with the inherited desktop environment. It
  agreed on the request-shape results below and differed only on the picker capability; it did not
  run the page/fork/goal/plugin/search probes.

## Observed results

| Call, exact body | Result |
| --- | --- |
| `session/list` `{"_request":{}}` | `{"items":[]}` |
| `session/create` `{"request":{"cwd":"…"}}` | `{sessionId, agentPreset:"standard"}` |
| `session/rename` `{"request":{"sessionId","title"}}` | `{title:"Alpha live session", seq:3}` |
| `fileReferences/list` `{"agentId","query":"main"}` | `[{"path":"src/main.rs","kind":"file"}]` (root query lists directories; a path prefix lists that level) |
| `sessionReferenceResolver/candidates` `{"agentId","query":""}` | one named session with `label/displayTitle/cwd/sameWorkspace/createdAt/mention` |
| `messageFeedback/list` `{"request":{"sessionId"}}` | `{"ok":true,"value":{"items":[]}}` |
| `messageFeedback/put` absent note/category, `ifVersion:null` | business `target-not-found` for a message with no assistant event — the wire is accepted |
| `messageFeedback/put` with `"note":null`/`"category":null` | **`gateway/input-invalid`** — fixed in `c9dd30e` |
| `messageFeedback/delete` `{"request":{"sessionId","messageId","ifVersion"}}` | `{"ok":true,"value":{"absent":true}}` |
| `schedule/catalog` `{}` | `[]` (empty isolated profile) |
| `schedule/history` with `"before":null` | **`gateway/input-invalid`** — fixed in `c9dd30e` |
| `schedule/history` with `before` absent, then a cursor string | `{"id":"missing","code":"schedule_not_found"}` both ways |
| `schedule/delete` `{"request":{"sessionId","id":"missing"}}` | `{"id":"missing","deleted":false,"code":"schedule_not_found"}` |
| `schedule/update` with a full one-shot `expected`, title-only and timed | `{"id":"missing","updated":false,"code":"schedule_not_found"}` — request shape accepted |
| `directoryPicker/list` `{}` | browse listing: `path`, `home`, `crumbs[]`, `entries[]`, `truncated` |
| `directoryPicker/list` `{"path":null}` | **`gateway/input-invalid`** — fixed in `c9dd30e` |
| `directoryPicker/list` `{"path":"/missing"}` | `directory-picker/unreadable` |
| `directoryPicker/pick` `{}` | `directory-picker/unavailable` (`the composed picker serves "browse"`) |
| `workspace/create` `{"request":{"path":"…"}}` | `{workspace:{workspaceId,path,title,sessionIds,createdAt,updatedAt}, created:true}` |
| `session/openWorkspacePath` missing file | `gateway/internal`, `path open failed` (business failure, no open) |
| `session/fork` before any completed turn | `session/fork-unavailable` with the session id |
| `session/page` with `address`/`throughSeq` | three seeded `permission/preset`, `sandbox/mode`, `approval/policy` events |
| `session/projections` | `{asOfSeq, values:{title, goal, tokenUsage, contextPressure, …}}` |
| `session/search` | `gateway/internal`: the desktop profile configures the session-query index with `openAt: "never"` |
| `goals/get` | `{"ok":true}` with no value (no current goal) |
| `commands/list` `{"agentId"}` | six commands (`compact`, `export`, `feedback`, `goal`, `permission`, `plan`) |
| `pluginInventory/list` `{}` / `settings/describe` `{}` | populated; `{"_request":{}}` is refused for both |
| `account/getState` `{}` | `signed-out` with the usage/top-up links |
| `session/modelCatalog` `{}` | the DeepSeek group with `deepseek-flash` and `deepseek-v4-pro` |

`fileReferences/list` caches one per-session index: files created after the first query are
invisible until a tool result invalidates it or a new session is created. The observed empty
answer was that cache, not the query semantics; a fresh session returned the file.

## Second pass — the non-model commands

A fresh isolated desktop-composed Host, one created session, and the exact native request bodies:

| Call, exact body | Result |
| --- | --- |
| `session/promptAdmission` `{"request":{"sessionId","requestId"}}` | `{"state":"unknown"}` |
| `session/updateQueue` remove a missing item | business `session/queue-item-not-found` |
| `session/updateQueue` edit a missing item with one text block | the same business refusal |
| `goals/get` | `{"ok":true}` with no value (no current goal) |
| `goals/edit` / `pause` / `resume` / `clear` with `ref:{id,revision}` | `gateway/internal`, `no current goal` — the ref shape is accepted |
| `session/attachment` with an unreferenced id | `session/attachment-invalid`, `ATTACHMENT_NOT_REFERENCED` |
| `terminal/list` | `[]` |
| `terminal/shells` | `zsh` and `bash` with `args:["-i"]` |
| `terminal/environment` | `cwd`, `maxInputBytes:65536`, `maxCols:500`, `maxRows:200`, `scrollback:1000` |
| `workspace/pinSession` then `unpinSession` | `{pinnedSessionIds:[id]}` then `[]` |
| `workspace/archiveSession` then `unarchiveSession` | `{archivedSessionIds:[id]}` then `[]` |
| `session/selectModel` `{sessionId,provider,model}` | `{selected:{provider,model,reasoningEffort:"high"}}` |
| `session/cancel` on an idle session | `{accepted:true}` |
| `commands/execute` flat `{agentId,line,submittedAttachments}` for `/plan` and `/plan off` | `{commandId,result:{kind:"success",text}}` |

No reply diverged from the native DTOs. The optional `reasoningEffort` on the selection is the
Host's committed default, and our type keeps it optional. These are shape and business-error
observations, not a claim that a queued turn, goal mutation or terminal session succeeded.

## Findings

1. **`messageFeedback/put` sent `null` for absent optional strings.** A plain rating or a note
   without a category is the common path, and it failed at the Host boundary. Fixed by omitting
   `note`/`category`; `ifVersion` stays present as the required `null`-or-string field.
2. **`schedule/history` sent `"before":null` on the first page.** Every first page failed. Fixed by
   omitting the cursor; the fake wire now distinguishes the first page from a cursor page.
3. **`directoryPicker/list` sent `"path":null` for the Host home.** The root listing failed. Fixed
   by omitting `path`, matching the reference client's `(path: string | undefined)` signature.
4. **The Host chooser cannot work in this app's composition.** Core clears `DISPLAY`, so the
   picker-auto resolver serves **browse**; `directoryPicker/pick` answers
   `directory-picker/unavailable`. The in-app folder list is the usable path. This does not
   invalidate the `pick` implementation, but the **Browse…** control's live outcome was the
   documented failure status. The app now learns the capability: a failed pick states the error and
   opens the in-app list, a successful listing marks browse, and later **Browse…** presses go
   straight to the list. A successful pick or cancel keeps the native path. The first press in a
   browse Host still performs one failing chooser call because the wire advertises no capability.

## Audit pass — two more wire divergences

A read-only audit of `transport/src` against the generated `typert.host.js` descriptors and the
reference client types found two real divergences, both fixed:

5. **`agentPresets/select` sent a wrapped body instead of flat arguments.** The descriptor declares
   exactly `agentId` (the lookup parameter) and `agentPreset`; the native sent
   `{"request":{"sessionId",…}}`. Confirmed live on a fresh Host: the flat body returned the
   committed preset (`"ptc"`, then `"standard"`), while the wrapped body was refused with
   `gateway/arguments-invalid` — *missing `agentId`, `agentPreset`; unexpected `request`*. Every
   preset pick was failing before this fix.
6. **`job/follow` sent the resume cursor as `offset`; the wire field is `from`.** The descriptor
   declares exactly `sessionId`, `jobId` and `from`, and the reference type names the cursor
   `from`. The Host's nested codec ignored the extra `offset` key rather than refusing it (a
   missing-job probe answered the same business error with either name), so a real resume would
   have restarted from the oldest byte instead of the requested cursor — a silent failure rather
   than a refusal. The request field is renamed and a fake-wire opening-envelope test pins it.

**Coverage.** Modules read: `dto.rs`, `http.rs` (complete), `mux.rs`, `lib.rs`, `event.rs`,
`upload.rs`, `terminal.rs`, `plugin.rs`, `prompt_admission.rs`, `question.rs`, `export.rs`
(container validation; tail unread). Reference read: session-controller, gateway, job-controller,
terminal-controller, settings-controller, workspace-controller (+directory-picker),
account-controller, plugin-inventory, commands, goal, schedule, message-feedback,
agent-preset-registry, session-reference, user-questions, session-log-export,
host/directory-picker, plus the `lib/typert.host.js` descriptors. **Not inspected:** `skills/list`,
`session/attachment`, `session/canOpenWorkspacePath`, `session/initializeDefaultModel`, workspace
delete/rename/insertBefore/insertSessionBefore, `directoryPicker/createDirectory`,
`terminal/rename|retain`, `workspaceFiles/*`, `llm/*`, `subagents/*`, `permissionPresets/catalog`,
`speech/*`, `dynamicCordisRunner/*`, `pluginManager/*`, account sign-in/watch/credentials,
`command-feedback`, and the export container tail.

## Third pass — the Core supervisor path

A temporary Core example started `Backend::start` against the pinned runtime with an isolated home
and requested the permission/preset families through the supervisor (not the BFF). Observed:

```
permission-catalog: ok options=3 default=workspace-write
permission-settings: ok current=workspace-write revision=0 writable=true
agent-presets: ok presets=4
permission-default-set: ok
stop graceful=true exited=true containmentUnknown=false
```

The same probe first returned `permission-settings: Protocol` twice: once because the Core's
`matches_reply` gate had no arm for the family (round 323), then because the bridge assumed the
BFF's wrapper shape and a stored `defaultPreset` instead of the service's array, its `writable`
property and the catalog fallback (round 324). The probe was deleted after the run.

## Still unverified

- A schedule with a real record: creation happens through the agent's schedule tool, so catalog
  rows, title/instruction edits against `expected`, a timing change and history records need a live
  model turn.
- Feedback put against a real assistant message (needs the same turn).
- `directoryPicker/pick` success/cancel under a native composition (the path/null/malformed reply
  shapes are fake-wire tested; a live chooser still needs a Host that serves it and a GUI
  interaction).
- Desktop-open success (`session/openWorkspacePath` on an existing file launches the desktop
  handler; only the failure path was safe to run).
- OAuth and any charged model execution. Those remain separate claims.

## Gates after the fixes

Measured with `python3 -B scripts/verify-native.py --suite test` and the format-debt script on
2026-10-17 after `c9dd30e`:

| gate | result |
| --- | --- |
| app, default | **1204 passed**, 1 ignored |
| app, `public-layout-fixture` | **1211 passed**, 1 ignored |
| transport | 56 + 17 + **86** + 7 |
| core (`--lib`, through the suite) | **112 passed** |
| JS runtime | **419 pass**, 0 fail |
| format debt | 0 files, 0 hunks |
| release `layout_smoke` build | warning-free |
| DP-2 capture | `shots/145-live-wire-navigation.png`, `status: passed` |

The suite's `scroll_app_smoke` extra was repaired after this pass: its module list had drifted from
the App and did not compile; it now runs 1214 passed / 1 ignored. `wayland-drops` still cannot fetch
`as-raw-xcb-connection v1.0.1` in `--offline` mode and remains an environment limit. Neither touches
this change. The release app **bin** reports 20 dead-code warnings in app code that this transport
change does not touch; the fixture example used for captures builds warning-free. `shots/145` is a
paint check of the unchanged navigation sheet, not wire evidence — the wire is proven by the live
Host and the fake-wire test.

## Reproduce

The runtime and the launch-token exchange are the app's own; no installed CLI or user profile is
involved. Keep the token, cookie and home private. The harness used here is not committed: it
imports `loadForkRuntime` and `runProfile` from `core/runtime`, writes the ready URL to a mode-0600
file, and exits on SIGTERM. Do not substitute the installed rc runtime — it is a different version.
