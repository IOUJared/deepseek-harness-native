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
>    - at handover: **1559** fixture / **1503** default, 1 ignored each (round 556).
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
> 1. **The rail's fades and the card's animations** — the reference scrolls a long ladder behind 24px
>    mask fades (native shows a 42-mark window instead) and animates the card in over 120ms with a
>    140ms vertical follow (native draws it at rest).
> 2. **A flaky examples test** — `single_instance::tests::a_second_launch_reaches_the_owner_that_holds_the_lock`
>    failed once in round 521 ("the owner answers on its socket"), then passed the re-run and six
>    further full runs in round 522. The cause is not established; the observation is retained.
> 3. **The turn control's counts** — the reference publishes `data-turn-process-messages`,
>    `-tool-calls` and `-subagents` for the control's accessible state
>    (`TurnProcessNodeView.tsx:38-41`); native's label states only the duration or end state.
> 4. **Hover ink** — the reference lifts a seat's title from tertiary to secondary ink on hover; native
>    keeps one ink and only swaps the leading glyph (round 516).
> 3. **`reference.png` measurements are paired, not pixel-diffed.** The frame was read as "collapsed
>    rail, no header" through round 509; round 510 measured every band in its left 520px and found no
>    ink, so it is a **crop**. A pixel diff needs a fixture preset that reproduces a crop — or a
>    cropped capture of the native frame at the same offsets.
> 4. **Context meter ring** — the reference's 14px ring trigger; native draws the occupancy text. This
>    needs iced's `canvas` feature (a sub-crate absent from the lockfile) → **user approval required**.
> 5. **Blocked on the Host, not on work**: settings' "Open configuration file" and the account
>    balance/usage rows (no RPC serves them); session-level feedback (native transport has only
>    per-message routes — `/feedback <text>` keeps the Host command path); terminal screen emulation and
>    fit-to-pane resize (no emulator by design); upload byte progress (the transport reports none); image
>    thumbnails anywhere (native draws attachment chips by design).
> 6. **User-only**: `canvas` approval, image fetch policy (https / local-only / neither), one window-edge
>    drag below 520×600 with a close-timing run, and the control-stream worker hop.
>
> Closed since the last handover: the header reminders chip (round 503), the split diff view (round
> 505), the comparison layout's persistence (round 509), the message rows' horizontal and vertical
> insets (rounds 504, 510), the group seat's 24px blank band (round 511), the folded-by-default seat
> with its expanded title gap (round 512), a settled turn's process control (round 513), the
> turn-process 16px flow rung (round 514), the seat's activity-to-chevron leading glyph (round 516),
> the transcript's turn rail over the loaded window (round 517), the rail's outline half with paging
> to an unloaded Turn (round 518), the rail's hover preview card (round 519), a paging mark's pulse
> with a windowed long ladder (round 520), the Turn usage accounting dialog (round 521), the
> `turnOutline` publication checked against the real Host (round 522,
> `scripts/qualify-turn-outline.py`), the frame-wide quota banner (round 523), the terminal
> panel's failed-discovery state with its audit list corrected (round 524), a workflow run's
> interrupted state (round 525), a Turn's usage as the sum of its attempts (round 526), and the
> reply renderer's struck runs and task items (round 527), raw HTML kept literal instead of failing
> the reply (round 528), link/image titles tolerated (round 529), a quote and a list composing in
> their opening order (round 530), one continuous rule across a quote's paragraphs (round 531), and
> a code block's one-second `Copied` confirmation (round 532), and a code card that wraps by default
> with a per-block toggle (round 533), and a heading that keeps its container's rule and indent
> (round 534), a thematic rule that keeps its container's inset (round 535), and a table that keeps
> its container's rule and fills the quoted column (round 536), and a fenced block that keeps its
> container's rule (round 537), and the renderer's last structural refusal — an image inside an
> image — closed (round 538), so no plain Markdown falls back to text now, and the terminal's rename
> review with its Host route (round 539), a reviewed resize control for the PTY (round 540), and the
> schedule-changed catalog reload (round 541), and the client projections checked against the composed
> Host (round 542, `scripts/qualify-client-projections.py`), and a renderer doc sweep that restored a
> lost heading-step test (round 543), and the wrapped code card's height rule measured and pinned
> (round 544), a repo-wide documentation citation sweep (round 545), and the `@` menu's breadcrumb
> trail (round 546), and `@` session rows labelled `Subagents`/`Sessions` as the reference does
> (round 547), the terminal panel's copy generated from its action list (round 548), and the
> single-instance owner-startup window closed (round 550; the one historical flake is still
> unexplained but not reproducible in 68 runs), and carriage-return redraw in the terminal panel
> (round 552), the erase sequences a `clear` sends (round 553), cursor addressing with a scrolling
> grid and scrollback (round 554), the ANSI colours a shell prints (round 555), and two-column wide
> glyphs (round 556).
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
