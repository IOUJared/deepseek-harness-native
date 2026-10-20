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
> - 666: **the Session-row hover preview** — 800ms dwell, 200ms grace, a non-modal beside overlay that
>   never captures input outside the card; frame `365`.
> - 667: the created-task card follows its catalog task (`Deleted` after deletion, current title/rule
>   while present); frame `366`.
> - 668: that card now renders on the **Turn tail** outside the process fold, not inside the call's seat;
>   frame `367`.
> - 669: a combining mark **composes onto its base cell** (bounded at 16 per cell); frame `368`.
> - 670: the round-551 "what is left" tables were re-audited against source and replaced; the handover
>   was refreshed. Nothing in the old blocked table was still blocked except the canvas-dial approval.
>
> **Current remainder** (see `docs/NATIVE-UI-PARITY.md`, "What is left"): terminal `terminal/retain` +
> a docked tab strip; the `schedule_create` expanded-details row; the workspace "Open in app" header
> control. Blocked on the user: the canvas-dial dependency approval and the deferred live
> `schedule/changed` observation. Baselines: fixture **1600** / default **1544**, 21 warnings, battery
> 17/17 (`verification-526912410ae0`).

---

## Notes for whoever maintains this file

- Refresh the baselines, shot number and backlog after every verification sweep; a stale handover costs the
  next session a round of re-discovery.
- Round 670 found the remainder tables a full clone-generation out of date (image thumbnails, session
  feedback and the terminal split were still listed as blocked). Re-read every "blocked/unbuilt" row
  against source before trusting it; the tables are the part most likely to rot.
