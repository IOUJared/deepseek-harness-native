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
> ### First actions, in order
> 1. `git log --oneline -8` and `git status` — the tree must be clean; read the last three rounds in
>    `docs/NATIVE-UI-PARITY.md` to learn the current state.
> 2. Measure the baselines rather than trusting this file:
>    - fixture: `cargo test --manifest-path app/Cargo.toml --locked --offline --jobs 2 --features public-layout-fixture --bin dsh-native-app -- --test-threads=2`
>    - default: the same without `--features`
>    - at handover: **1486** fixture / **1447** default, 1 ignored each.
> 3. Load `.agents/skills/native-ui-parity/SKILL.md` (and `.agents/skills/impeccable-rust/SKILL.md` for
>    Rust changes) — `AGENTS.md` makes both mandatory for this work. Then read `docs/SESSION-PROMPT.md`
>    (capture conventions, next shot number), `docs/VERIFICATION.md`, and the coverage table at the end
>    of `docs/NATIVE-UI-PARITY.md`.
>
> ### The loop, one item per round, one commit per item
> 1. **Prove the gap first.** Either reuse a recorded finding or run a read-only audit (a subagent that
>    only reads, no cargo, no writes) over one surface, asking for (b) different and (c) absent findings
>    with `file:line` on **both** sides, and for a list of what is already documented so it is not
>    re-reported.
> 2. Implement against the reference's behaviour, not its pixels alone. Name deliberate differences.
> 3. Add a test named as a lowercase sentence that fails if the label/rule coincides by accident.
> 4. Capture a DP-2 frame when anything visual changed: build the release `layout_smoke` example, run
>    `/tmp/dsh-capture.sh NAME PRESET BURST`, inspect with `read_image`, archive as
>    `shots/<next>-<name>.png` + `-dp2.png` + `.json` and mirror into `app/evidence/<feature>-<date>/`.
>    **Inspect the frame before committing** — a green suite cannot see an element that vanished
>    (round 107) or a duplicated label (round 109).
> 5. Run the full battery: fixture, default, transport (59+17+91+7), core `--lib` (115), JS
>    (`node --test core/runtime/*.test.mjs`, 419), `scripts/format-debt.py` (0), `verify-native.py
>    --suite clippy` and `--suite examples`, and report the app bin warning count (21).
> 6. Record the round in `docs/NATIVE-UI-PARITY.md` with a battery table and honest notes — including
>    mistakes made inside the round.
> 7. Bump the shot counter in `docs/SESSION-PROMPT.md` and **verify the line actually changed**; a
>    reworded anchor silently failed for thirteen rounds once.
>
> ### Hard rules
> - Never `cargo clippy --fix`, never reformat, never edit `scripts/format-debt-baseline.json`.
> - Builds are `--locked --offline --jobs 2`, never concurrent with another agent's build.
> - No new dependencies without explicit approval; `app/vendor/winit` is a pinned patch — never file
>   findings against it.
> - A new fixture preset needs registration in four places (PRESETS list, size arm, seed block, match
>   arm) plus draining queued commands, or the fixture test fails.
> - Keep destructive actions guarded and confirmations visible; buttons say exactly what they persist.
> - Report passed versus unrun checks honestly; do not claim a visual match without a matching-state
>   frame (same window size, same sidebar state, same conversation data).
>
> ### Live backlog at handover (verify each before working it)
> 1. **Header reminders chip** — the reference registers an icon-only clock with "{count} reminders" and a
>    popover (`ui-schedule/src/client/index.ts:208-216`, `order: -5`); native's only entry is the sidebar
>    row. Not yet built.
> 2. **Split (side-by-side) diff view** — named deliberate second layout; wrapping and the `Wrap`/`No
>    wrap` toggle exist, split does not.
> 3. **Context meter ring** — the reference's 14px ring trigger; native draws the occupancy text. This
>    needs iced's `canvas` feature (a sub-crate absent from the lockfile) → **user approval required**.
> 4. **Pixel comparison against `reference.png`** — the screenshot is now in the workspace root and
>    hash-matching; round 502 measured the bubble line (22px, fixed). Recorded and still open: native's
>    message rows carry a 14px horizontal pad the reference does not (reference text at lane +1, native
>    +19), and a pixel-level diff needs a **matching-state preset** (collapsed sidebar, no header,
>    scrolled) before any 1-to-1 claim.
> 5. **Blocked on the Host, not on work**: settings' "Open configuration file" and the account
>    balance/usage rows (no RPC serves them); session-level feedback (native transport has only
>    per-message routes — `/feedback <text>` keeps the Host command path); terminal screen emulation and
>    fit-to-pane resize (no emulator by design); upload byte progress (the transport reports none); image
>    thumbnails anywhere (native draws attachment chips by design).
> 6. **User-only**: `canvas` approval, image fetch policy (https / local-only / neither), one window-edge
>    drag below 520×600 with a close-timing run, and the control-stream worker hop.
>
> ### Etiquette
> Be terse. Prefer numbered lists over tables in replies. Challenge your own claims; walk back anything
> unsupported. If a frame contradicts the suite, the frame wins and the round says so.

---

## Notes for whoever maintains this file

- Refresh the baselines, shot number and backlog after every verification sweep; a stale handover costs the
  next session a round of re-discovery.
- The last sweep that mattered (round 129) found two stale coverage rows rather than code drift — the
  coverage table is the part most likely to rot.
