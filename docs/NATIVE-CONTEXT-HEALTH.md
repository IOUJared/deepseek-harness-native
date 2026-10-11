# Native context health, context management and the loop guard

How the native app reports the three harness components that
[An Empirical Study of Harness Design for Coding Agents](https://arxiv.org/abs/2609.20804)
(arXiv:2609.20804) isolates: context management, planning, and the action space.
The study's headline result is that context management pays off mainly by
**preventing overflow**, not by improving reasoning: over its four models, an
unmanaged context overflowed on **78.7%** of SWE-Bench Verified tasks, while every
managed tier overflowed on **zero** tasks at every budget. This page covers what
the app shows, and what it deliberately does not show.

Presentation only. Nothing here sends a prompt, changes a Host policy, exposes a
settings write, or weakens a Send gate.

## Context readout

The composer's single line is the Host's own reading, tiered:

| Tier | When | Composers style |
|---|---|---|
| Normal | below the compaction threshold | muted |
| Tight | above 80% of the reported window | text colour |
| Critical | at or past the window | danger colour, plus a visible warning line |

- The line is `Context ~used / capacity · N%`. It is **approximate** and labelled so.
- `Tight` is keyed to the mounted compaction threshold (`thresholdRatio: 0.8` in the
  base bundle), not to the study's own `0.6`/`0.85` pair: the app reports the policy
  this harness runs, and invents no other.
- Occupancy the window cannot contain is treated as a malformed report. The two
  counts are shown and no percentage, tier or warning is derived from them.
- A last-request total, a pending model switch, a changed route or a stale
  projection produce no tier and no warning.
- Only this line and its disclosure come from the projections; a session with no
  usage report shows `Usage unavailable` exactly as before.

`▸ Context policy` expands four bounded entries: the approximation, the harness's
staged elision-then-summarization policy, that predefined tools stay available,
and — only at the window limit — why continuing is risky. Long entries wrap; a
clipped warning is not a warning.

## Context management in Trajectory

The harness already stages rule-based elision **before** LLM summarization, which
is the study's best-performing ordering, but it is invisible on the wire. Two rows
name it, using the study's vocabulary:

| Row | Source event | Reported |
|---|---|---|
| `Context elided · N records → stubs` | `compaction/prune` | record count and the logged `shadowedTokenCount` |
| `Context summarized · seqs A–B` | `compaction/summary` | shadowed range and the same logged token count |
| `Compaction failed · <error>` | `compaction/end` with `error` | red, like every recorded error |

`compaction/start` and `compaction/end` without an error are transaction brackets
and produce no row, exactly as `turn/end` without a notice does. Detail opens the
recorded payload, which stays authoritative; the summary body never reaches the
row. Every line is bounded, and a malformed payload contributes nothing.

The study also finds the recall mechanism (`recall_event`) is unused in 56.3% of
its matched settings and does not improve accuracy over elision alone, so this app
exposes no recovery control: spill locators stay what the tool result already says.

## Loop guard

`repeat-tool-reminder` reminds the model after 3, 5 and 8 identical calls. Its
reminder is a `user/message` marked by its producer source, and measures a loop
the app could not count for itself — the app therefore labels the reminder rather
than recounting anything:

`⟳ Loop guard · bash × 8` — the tool and run length are the guard's own
`source.summary`, and an unrecognised summary degrades to the bare label.

## Standing plan

The study's planning scaffold appends the stored plan to each request **without
storing it in history**, so the plan is state rather than conversation. The
composer reads the newest `todo/write` and shows one muted chip:

`Plan · 1/4 · next: <item>` · `Plan · 4/4 · complete` · `Plan · 3/7` (nothing in
progress). Unlike the cumulative **Todos** section behind *More → Work records* —
which keeps every item ever written — this is the model's current list, so an item
a later write dropped disappears. No write is ever retained, and a session that has
never written a plan shows no chip.

## Plan mode

Plan mode is the harness's soft guidance, rendered as the `plan:policy` prompt
section while it is active. The Host publishes it to clients as exactly
`{ active, pending }`, and the composer shows it in accent beside the plan chip:

| Host state | Chip |
|---|---|
| active, settled | `Plan mode` |
| active, a selection still waiting to be appended | `Plan mode · applying` |
| inactive, a selection still waiting | `Leaving plan mode · applying` |
| inactive, settled | no chip |

The distinction matters: a selection is only appended at the next accepted in-turn
pre-step, so guidance stays in force — or is not yet in force — until then.

The chip is **actionable where the Host supports it**, and read-only otherwise. Its
offer comes from whether the Host published the `plan` projection at all: a
deployment without the plan-mode plugin publishes no key, one that mounts it
publishes the key even while inactive. Pressing it asks for the *opposite of the
committed state*, so `Applying` requests `/plan` and `Active` or `Leaving` request
`/plan off`; `Leaving` is still active and must not be asked to enter. The mode you
see afterwards is the Host's projection, never the request that was sent, so an
accepted command that changed nothing reads as unchanged. A press in flight is
inert, and a reply for a different session or a different line is ignored as stale
rather than clearing that state.


## Chat stays clean

Compaction, the loop guard and the plan chip are harness bookkeeping. Chat
presents only the conversation; Trajectory keeps every record and every labelled
row. The retained window is unchanged, and an unrecognised event kind still stays
visible by policy.

`Known internals` are excluded from Chat by the **kind** the reducer folded, not by
the label it chose. `chat_visible(kind, role)` checks the vocabulary list before any
role label, so a bookkeeping kind cannot join Chat whatever a caller calls it, while
a message-form kind keeps the decision its folding branch made from the event's own
producer.

Two real leaks came from the earlier label-only rule: `session-log-deepseek/delivery-accepted`
and the system prompt both rendered as JSON in Chat, because a listed kind is asked
about by *name* and a display label answers a different question. `every_known_kind_is_classified_deliberately_in_chat`
now pins the whole vocabulary so a third leak fails a test instead of reaching the UI.

## What real recorded sessions show

The render frames are synthetic, but the parsers are additionally pinned against a
**real recorded alpha session** in `app/fixtures/real-session-context.json`: 88
events covering the payload shapes a Host actually sends — `request/context`
carrying the window, `assistant/message` carrying usage buckets, injected user
messages, spliced inbox messages, tool rows and delivery bookkeeping. Its content
is replaced; its kinds, field names, sequence order and numbers are the recorded
ones. `a_real_recorded_window_projects_its_own_reported_occupancy` asserts that the
window projects its own reported occupancy (7486 of 272000 tokens, 2%), that the
recorded bookkeeping kinds stay out of Chat while the recorded human and tool
turns stay, and that every recorded tool row labels through the real argument
shapes.

Measured across the 14 recorded sessions in this workspace: the highest occupancy
was **2.8%** of the reported window against a compaction threshold of **80%**, and
**no session contains a single `compaction/*` or `plan/mode` event**. That is why
compaction, the loop guard and plan mode have no live evidence here: they have
never occurred on this machine, not merely gone unobserved.

The compaction rows are additionally pinned to a **recorded payload** rather than to
my reading of the type declarations:
`the_recorded_compaction_shape_labels_as_documented` carries the
`compaction/summary` event field-for-field from a session the backend's own snapshot
suite recorded against a real summarization call
(`deepseek-harness-linux/snapshots/session/compaction-summary-headroom/session.v4.jsonl`),
and asserts that it labels as `Context summarized · seqs 8–9` with `~375 tokens
reclaimed`, that its summary body stays in the record, and that the recorded
`compaction/start` and `compaction/end` brackets produce no row. If that shape
moves, the test fails here instead of in a user's session.

`compaction/prune` has never been recorded anywhere, so its shape is pinned from
the producer instead (`compaction-tool-result-pruner/src/index.ts`, the append
beside the replacement `tool/result`). It shares `shadowedRange`, `shadowedSeqs`
and `shadowedTokenCount` with the summary event, which is why one parser reads both.

Plan mode is not entered through a prompt: `/plan` as prompt text would be ordinary
conversation. The Host executes commands through its own surface, so the app invokes
`commands/execute`, which the transport also validates. Follow delivery stays
passive — the offer is read from the projection a frame already carries, so knowing
whether this Host mounts plan mode costs no round trip, and the app dispatches
nothing while appending history.

## Not qualified

- No live Host, provider or model produced these frames. `layout_smoke` renders
  PUBLIC synthetic records: `context-pressure`, `context-pressure-risk`,
  `context-management`, `context-management-chat`, `plan-chip` and the
  `plan-mode` states.
- The real recorded window above is replayed through the fold; it is not a Host
  run. It exercises the parsers against real shapes, not a live connection.
- The tier thresholds are read from this build's mounted policy. A deployment that
  changes `thresholdRatio` or the pruner budgets changes the copy's accuracy, not
  the code's behaviour: nothing here parses a threshold from the wire.
- The study's own measurements are on its four models and two benchmarks. They are
  cited as its findings, never as measurements of this app.
