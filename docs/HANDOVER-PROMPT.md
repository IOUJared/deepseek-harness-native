# Handover prompt — native clone rounds

Paste the block below into a new session to continue this work. It assumes the same machine, the same
two checkouts, and the same standing permissions (local commits allowed, no push).

---

## The prompt

> Continue the native UI clone of the DeepSeek Harness web UI in
> `/home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-native` (branch `main`;
> parity commits and pushes to `main` are authorized — no PR, no publish).
>
> **Goal** (create it with the goal tool, `max_goal_rounds` 200): *Close the remaining native-clone gaps
> one commit at a time, reference-first, with tests, DP-2 captures and full gates, recording user-only
> blockers in the docs instead of asking.*
>
> Work autonomously. Never end a turn with a question; never stop to wait for approval. When something
> only the user can provide blocks an item, record it in `docs/NATIVE-UI-PARITY.md` and move to the next.
>
> **Reference**: `/home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-linux`
> (pinned `c6044cc50b`). Cite it as `file:line` in code comments and round notes.
>
> ### Last five rounds, so the next session starts oriented
> - 669: a combining mark **composes onto its base cell** (bounded at 16 per cell); frame `368`.
> - 670: the round-551 "what is left" tables were re-audited against source and replaced.
> - 671: the **workspace "Open in app"** header split button over the two plain Host routes, with the
>   remembered choice persisted; frame `369`.
> - 672: a created task's expanded row states `Scheduled for` / `Repeat` / `State`; frame `370`.
> - 673: the terminal panel **docks its terminals as tabs** above the output, re-following on a switch
>   instead of holding `terminal/retain` streams; frame `371`.
>
> **Current remainder:** no unbuilt reference surface remains. Blocked on the user: the canvas-dial
> dependency approval (the context meter stays a text reading) and the deferred live `schedule/changed`
> observation (the arm is built and unit-tested). Baselines: fixture **1605** / default **1549**, 21
> warnings, battery 17/17 (`verification-ca228a0e2dfe`).

---

## Notes for whoever maintains this file

- Refresh the baselines, shot number and backlog after every verification sweep; a stale handover costs the
  next session a round of re-discovery.
- Round 670 found the remainder tables a full clone-generation out of date (image thumbnails, session
  feedback and the terminal split were still listed as blocked). Re-read every "blocked/unbuilt" row
  against source before trusting it; the tables are the part most likely to rot.
