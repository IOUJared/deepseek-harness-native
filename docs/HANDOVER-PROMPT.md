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
> - 674: the **read-only plugin manager** reads the Host's profile availability and installed bundles;
>   frame `372`; the inventory projection now carries `managementAvailable`.
> - 675: **a bundle switches on/off through review → Confirm**, one write, the Host's application /
>   restart / failure copy, then a re-read; frame `373`.
> - 676: **a removable bundle uninstalls through its own confirmation**; frame `374`.
> - 677: **a typed install spec is inspected** into an accepted preview or a refusal; frame `375`.
> - 678: **the Add-plugin form reads the registry order and inspects against the chosen registry**;
>   frame `376`.
>
> **Current remainder:** the plugin manager's install (stages, build approvals, cancellation, the
> install request id), version exemptions and plugin-level (row) toggles. Blocked on the user: the
> canvas-dial dependency approval (context meter stays a text reading) and the deferred live
> `schedule/changed` observation. Baselines: fixture **1610** / default **1554**, 21 warnings, battery
> 17/17 (`verification-bef38b907176`).

---

## Notes for whoever maintains this file

- Refresh the baselines, shot number and backlog after every verification sweep; a stale handover costs the
  next session a round of re-discovery.
- Round 670 found the remainder tables a full clone-generation out of date (image thumbnails, session
  feedback and the terminal split were still listed as blocked). Re-read every "blocked/unbuilt" row
  against source before trusting it; the tables are the part most likely to rot.
