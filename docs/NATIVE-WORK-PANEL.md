# Native read-only work records panel

More → **Work records** opens one compact, read-only panel for the recorded
goal, todo, subagent and schedule events of the selected conversation. It is not
a new tab: the entry sits in the existing **More** menu under **APPLICATION**,
beside *Information & diagnostics*, and Escape or **Back to conversation**
dismisses it exactly like the other auxiliary views.

## What it shows

| Section   | Source event                                                | Folded fields |
|-----------|-------------------------------------------------------------|---------------|
| Goals     | `goal/change` (`create`/`edit`/`pause`/`resume`/`complete`/`block`) | objective · phase, max rounds and rounds started, blocked code/message, last operation |
| Todos     | `todo/write`                                                | item text · pending / in progress / completed |
| Subagents | `subagent/catalog`, `subagent/descriptor`                   | child identity or label, mode, provider/agent route |
| Schedules | `schedule/change` (`create`)                                | type (`at` / `after` / `every`), timing, prompt |

The Todo section is cumulative: it keeps every item ever written, because a
`todo/write` that drops an item is still a record. The composer's standing-plan
chip is the opposite view — only the newest write, so an item the model dropped
disappears. [Context health](<NATIVE-CONTEXT-HEALTH.md>).

The raw events stay authoritative in the transcript; this panel is only a
projection. `goal/change` with `clear` and `schedule/change` with `delete`
remove the matching row, and repeated updates of one identity replace that row
instead of appending duplicates. Trajectory is unchanged and still shows every
record, including kinds this projection does not understand.

## Bounds

- **50 records per section** and **200 in total**; the total is `4 × 50`, so one
  busy section can fill the panel while the per-section bound still holds.
- Each section keeps its newest records. When a section is full, its own oldest
  record is dropped; when the panel total is full, the globally oldest record is
  dropped. Every drop is counted.
- Overflow is explicit, never silent: a section with drops shows
  `N older records omitted`, and the header states the bounds and the total
  omission count.
- Each rendered line is bounded to 512 UTF-8 bytes, a goal objective is
  additionally bounded to 90 bytes, and an entry keeps at most 8 lines.

These match the neighbouring retention style: the transcript keeps at most 4096
records / 16 MiB (`app/src/reducer.rs`), and this projection is rebuilt from the
same retained events.

## Read-only

The panel has no write, cancel, retry, delete or schedule-editing control, and
opening or closing it produces no command, effect, prompt or authority change;
the UI states this in the panel text. The `Work` message carries only `Open` and
`Close`, and the projection is rebuilt inside `Transcript::rebuild`, the same
fold that produces the Trajectory rows.

Malformed or unrecognized payloads contribute nothing to the panel and cannot
crash the fold. A section with no valid records is omitted and an entirely empty
projection shows one short line, never an empty box.

## Not qualified

- No live Host session has been exercised against this panel; the evidence is
  unit tests plus a synthetic `public-layout-fixture` capture registered as
  `work-panel` (900×2082 PNG, 32 synthetic records, 0 captured commands).
- Subagent steering, schedule editing/cancellation and goal control remain
  unavailable in this UI, as before; only the read-only records are shown.
- A descriptor carries no child id on the wire, so a `subagent/descriptor` row
  is keyed by its label (or provider for a one-shot child) and can appear beside
  the matching `subagent/catalog` row rather than merged with it.
