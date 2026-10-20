# Handover prompt — native clone rounds

Paste the block below into a new session to continue this work. It assumes the same machine, the same
two checkouts, and the same standing permissions (local commits allowed, no push).

---

## The prompt

> Continue the native UI clone of the DeepSeek Harness web UI in
> `/home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-native` (branch `main`,
> local commits only, no push, no PR, no publish).
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
> - 623–625: the cursor's index was proved by test (`display_paint` returns rows **and** cursor from one
>   pass; `display_styled` delegates and still trims for text) after the frame exposed a dual-source bug.
> - 626: **the cursor is painted** — `shots/353-terminal-cursor.png` shows the block; fixture 1581, default
>   1525, 21 warnings.
> - 627: full battery 16/18 with both release artifacts rebuilt — `wayland-drops` (hardware) and
>   `release-guards` (leftover entry for a ported stage) only.
> - Terminal screen: protocol complete plus ECH, scroll up/down, cursor save/restore (both spellings),
>   scrollback erase and bold cancellation (605–610), all under a fifteen-case bounded-join guard.
> - Only app item left: the **terminal panel three-builder rewrite** (`terminal_header` / `terminal_output` /
>   `terminal_footer`), specified in the round-572 note and unbuilt.

---

## Notes for whoever maintains this file

- Refresh the baselines, shot number and backlog after every verification sweep; a stale handover costs the
  next session a round of re-discovery.
- The last sweep that mattered (round 129) found two stale coverage rows rather than code drift — the
  coverage table is the part most likely to rot.
