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
> - 680: **an inspected spec installs** after one explicit Confirm, reporting bundle/version/restart or
>   failure and re-reading; frame `378`.
> - 681: **installs carry a request id**, a lost reply is recovered through `waitForInstall` (not a
>   retry), and `cancelInstall` states cancelled/too-late/not-running; frame `379`.
> - 682: **a failed install's pending package scripts are approved and retried**; frame `380`.
> - 683: **the install's Host phase, per-registry attempt and a bounded pnpm log tail are shown**;
>   frame `381`.
> - 684: **the final walk** enumerated every reference client slot and found no unbuilt surface;
>   the composer row's three stale claims were corrected and the round record is the only commit.
>
> **No unbuilt reference surface remains.** What stands is the user-only canvas context-ring approval
> (the context meter is a text reading), the deferred live `schedule/changed` observation, and the named
> deliberate differences (right Sidebar tabs, dockkit presets, locales, markdown remote images,
> web-only onboarding, session-level feedback dialog, workflow-run interrupted state, terminal
> fit-to-pane). Baselines: fixture **1615** / default **1559**, 21 warnings, battery 17/17
> (`verification-707c8098ff18`).

---

## Notes for whoever maintains this file

- Refresh the baselines, shot number and backlog after every verification sweep; a stale handover costs the
  next session a round of re-discovery.
- Round 670 found the remainder tables a full clone-generation out of date (image thumbnails, session
  feedback and the terminal split were still listed as blocked). Re-read every "blocked/unbuilt" row
  against source before trusting it; the tables are the part most likely to rot.
