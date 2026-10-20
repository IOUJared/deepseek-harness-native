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
> - 681: **installs carry a request id**, a lost reply is recovered through `waitForInstall` (not a
>   retry), and `cancelInstall` states cancelled/too-late/not-running; frame `379`.
> - 682: **a failed install's pending package scripts are approved and retried**; frame `380`.
> - 683: **the install's Host phase, per-registry attempt and a bounded pnpm log tail are shown**;
>   frame `381`.
> - 684: **the final walk** found no unbuilt surface and corrected three stale composer claims.
> - 685: **the context meter draws the reference's 14px ring** on iced's newly approved `canvas`
>   feature; frame `382`. The live `schedule/changed` attempt is recorded: the Web UI schedule lives on
>   a different Host than the app's isolated one, and the Host has no create route.
>
> **No unbuilt reference surface remains.** The only unobserved item is live `schedule/changed` arrival
> in the app's own Host (unit-tested; needs an agent turn that creates a schedule there), and the
> deferred schedule observation is no longer a user action — it is an environment limit recorded in
> `docs/NATIVE-LIVE-VERIFICATION.md`. Baselines: fixture **1616** / default **1560**, 21 warnings,
> battery 17/17 (`verification-0f98d0df5758`).

---

## Notes for whoever maintains this file

- Refresh the baselines, shot number and backlog after every verification sweep; a stale handover costs the
  next session a round of re-discovery.
- Round 670 found the remainder tables a full clone-generation out of date (image thumbnails, session
  feedback and the terminal split were still listed as blocked). Re-read every "blocked/unbuilt" row
  against source before trusting it; the tables are the part most likely to rot.
