# Live Host verification

One pass of the native `@` / schedule / feedback / workspace / picker paths against a **running
Host**, not a fixture. It found three wire defects, all fixed in `c9dd30e`, and one composition
limit that changes what the workspace picker can honestly claim.

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

## Still unverified

- A schedule with a real record: creation happens through the agent's schedule tool, so catalog
  rows, title/instruction edits against `expected`, a timing change and history records need a live
  model turn.
- Feedback put against a real assistant message (needs the same turn).
- `directoryPicker/pick` success/cancel under a native composition (needs a Host that serves it and
  a GUI interaction).
- Desktop-open success (`session/openWorkspacePath` on an existing file launches the desktop
  handler; only the failure path was safe to run).
- OAuth and any charged model execution. Those remain separate claims.

## Gates after the fixes

Measured with `python3 -B scripts/verify-native.py --suite test` and the format-debt script on
2026-10-17 after `c9dd30e`:

| gate | result |
| --- | --- |
| app, default | **1203 passed**, 1 ignored |
| app, `public-layout-fixture` | **1210 passed**, 1 ignored |
| transport | 56 + 17 + **83** + 7 |
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
