# Native Host terminal panel

**More → Terminal** is a client-only view of the Host's `terminal/*` service. The app never
spawns a shell or child process; the Host owns every PTY and its process range.

## Flow

1. **Open** (More → *Terminal · Host shell*) issues one read: `terminal/list` for the selected
   Session's retained terminals, then best-effort `terminal/shells` and `terminal/environment`
   for the registered Agent. Opening produces no shell, terminal or other Host mutation.
2. **Select** one listed terminal attaches `terminal/follow` with a fresh attachment id and
   exclusive input control (an older attachment becomes read-only). The first frame is a
   complete bounded screen, then ordered output and metadata changes.
3. **New terminal…** reviews a discovered shell profile and the fixed `80×24` request, then
   sends one `terminal/create` for a caller-generated `[\w-]{1,128}` identity.
4. **Close…** reviews one exact terminal, then sends one `terminal/close`.
5. **Keys** reach the shell while the panel is open. The reference pipes its terminal's key handler
   straight to the Host (`ui-sidebar-terminal/src/client/terminal.tsx:101,138`), and native does the
   same: `terminal_keys::bytes` maps each press to the bytes it stands for — named keys to their
   escape sequences, Ctrl+letter to its control byte, a printable character as itself — and
   `Action::Key` sends them through the same guarded write path as the button. An unmapped key sends
   nothing and falls through, so Escape and the app's shortcuts still work (rounds 460).
6. **Write** sends the typed input line verbatim, with no terminator. The line exists for input a
   key mapping cannot express (a paste, a long command); Enter is a key, so it arrives as `\r`
   through step 5 rather than through this button. The line clears only after the Host accepts it.
7. **Take control** is a control, not an instruction: another attachment holding the terminal makes
   this view read-only, and the sentence sits beside a **Take control** button that re-attaches
   (`locales.ts:35`, round 457).
8. **The shell choice is remembered** across launches as the `layout.terminalShell` preference, so a
   reopened create review starts on the shell you last chose (rounds 461–462).
9. **Rename** opens a review on the title the Host reports, refuses a title that is empty, over 120
   characters, control-bearing or unchanged, and installs the new title only from the Host's ACK
   (round 539, `terminal-controller/src/index.ts` `rename`).
10. **Resize** opens a review on the size the Host reports, refuses zero, out-of-bounds or unchanged
    dimensions, and adopts the acknowledged size only from the Host's ACK (round 540).

Left out of this slice: `terminal/retain` (which only means something with docked terminal tabs).

**The screen is a native emulator core, not a text panel (rounds 552–559).** It owns a grid as tall and
as wide as the Host's own terminal and applies the rules a shell depends on: `\r` redraws in place,
`CSI K`/`J` erase, `CSI H`/`f`/`A`–`D`/`G`/`d` address, `CSI L`/`M`/`P`/`@` insert and delete rows and
characters, `CSI m` colours the cells (drawn through `design::ansi_ink`), wide and fullwidth glyphs take
two columns, a glyph past the last column wraps, the bottom row scrolls into scrollback (2,000 lines),
and `CSI ? 1049 h`/`l` swaps to the alternate screen a full-screen program draws in and restores the
shell on exit, and `CSI t;b r` sets a scroll region whose rows scroll alone while the rows above and
below stay put (round 561), and `CSI ? 7 h`/`l` turns autowrap on or off, so a program that overwrites
its own last column keeps that column instead of wrapping (round 562), `SGR 4`/`21`/`24` underline a run
(round 564), and `SGR 40`–`47`/`100`–`107`/`49` paint a cell's background through the same palette
(round 565). The one remaining limitation is a *text* one rather than a protocol one: a combining mark
takes no column of its own and is dropped, because one character per cell cannot compose it. The panel is a monochrome-per-cell renderer of that
screen, not a second emulator.

**The output does not fill the pane, and here is exactly why (rounds 567–568).** The panel is one
`scrollable` over the whole body — `scrollable(container(container(body).max_width(...).padding(24)))` in
`ui.rs::terminal_view` — so the output box cannot take a `Fill` height: a Fill child inside a shrink
column inside a scrollable has nothing to fill, and round 567's capture shows it collapsing to its
content (`shots/347-terminal-fill.png`). Filling the pane like the reference's terminal requires
splitting the panel into two regions: a scrolling header (title, authority copy, shell, terminal list,
notices) and a fixed bottom region where the **output** is the only `Fill` child above the input row.
That restructure is the recorded next step (specified to the line in the round-572 note), because a
half-restructured panel is worse at short window heights than the box is.

**What the box does instead (rounds 571 and 575).** It is no longer a fixed height: on a tall window it
takes a share of the window — `clamp(window * 0.42, 240, 640)` — and under 700px it gives the header and
footer room first — `clamp(window - 420, 60, 240)` — so the input row never falls below the fold. A
520-high window shows the input row again (`shots/350-terminal-short.png`); the Write button and the
notice can still require a scroll there, which is what pinning the footer in the rewrite fixes. A tall
window shows about 385px of screen (`shots/349-terminal-share.png`).

Resize is a reviewed control in the panel (round 540) rather than the reference's automatic fit-to-pane:
a create requests `80×24`, clamped to the limits the Host advertised, and a later resize replaces that
request with the size the operator confirms.

## Authority and consent

- Copy in the panel states plainly that **the shell runs on the Host with the Host's authority**,
  that this app is a client, and that input is interpreted by the Host shell, not normalized here.
- Create, close, rename, resize and write are mutating. Each is an explicit **Review → Confirm**
  (write is the explicit **Write** action) and the app holds no optimistic state:
  - A create adds nothing until the Host ACK returns the **exact requested identity**.
  - A close removes nothing until the Host confirms it.
  - A rename changes no title until the Host acknowledges the reviewed one.
  - A resize changes no size until the Host acknowledges the reviewed dimensions.
  - A write clears the typed line only after the Host accepts it.
- Every one of them uses the same owned-ticket/epoch fencing as a *session* rename or fork — epoch,
  session generation, target and panel serial checked on every admission — and shares the app's
  single management flight, so a terminal write cannot race a session rename, fork, export or
  registry mutation (and those cannot race it).
- No automatic retry. A refusal, timeout or disconnect is surfaced as an indeterminate outcome
  (`CreatingIndeterminate`, `ClosingIndeterminate`, `RenamingIndeterminate`,
  `ResizingIndeterminate`, `WriteIndeterminate`) and requires a new explicit action.
- The panel's own copy for a Session without a registered Agent is **generated from the list of
  mutating actions** (`terminal::MUTATING_ACTIONS`), so it cannot go on naming only the older ones
  (round 548).

## Bounds

- Output is kept twice: the raw bytes bounded as follows, and the screen the emulator core builds from
  them (above). The sanitized text strips CSI/OSC payloads, so a plain copy of the output never carries
  escape sequences; the screen applies those payloads instead (rounds 552–559).
- The retained bytes live in the panel only: at most **400 chunks / 128 KiB**, newest kept, and the
  panel renders only the newest **32 KiB** of text and at most **400 rows** of it (one rich-text row per
  line, so a long scrollback cannot cost a paint per line, round 566). The screen keeps **2,000** lines
  of scrollback and counts what it drops; the viewport is not anchored across a redraw that drops front
  rows. Every discarded chunk and every byte an oversized chunk loses is counted and
  shown.
- One write is bounded by the smaller of the Host's `environment.maxInputBytes` and the
  transport's 256 KiB outbound cap — the typed line and a single keystroke's bytes alike; a create's
  dimensions are `80×24` bounded by the host environment's `maxCols`/`maxRows` (round 343).
- A keystroke never edits the typed line: the reference's terminal has no line for a key to land in,
  so `Action::Key` leaves the field untouched (round 460).
- A failed shell-profile read is not an empty discovery: `Loaded.shells_failed` and
  `Terminal::shells_failed()` carry the difference, and the SHELL section says
  "The Host's shell profiles could not be read; create uses the Host default shell. Re-read the panel
  to try again." in place of "No shell profile was discovered…" (round 524).
- The panel is an in-shell body (like Work records): it hides the composer and Chat/Trajectory
  tabs while open and never touches the chat transcript, scroll anchor or virtualisation.

## Not verified

No live Host terminal was created, attached, written to or closed. Shell discovery, real PTY
stream framing, actual `WebTerminalInfo` values and Host refusal codes were exercised only
against a local fake BFF fixture. The alpha `desktop` profile is expected to mount the service
(`core/runtime/fork-supervisor.mjs` layers `@deepseek-ai/dsh-web-app`, whose
`cordis.patch.yml` mounts `terminal-controller` unconditionally), but that is a source reading,
not a composed-profile or runtime observation.

**Every quantity a sequence parses is bounded (rounds 588–596).** The screen is fed bytes from the Host, so
a parsed number is untrusted. A tab stop is clamped to the last column
(`a_tab_past_the_last_column_does_not_spin`); `CSI N L`/`M`/`P`/`@` clamp their count to the grid's height
or the screen's width, and to 1,024 before the Host states a size (`bound every count` in round 592);
`CSI r;c H`, `G` and `C` clamp the cursor to the screen through one `clamp_column` (round 593); an unbounded
grid grows on insert and shrinks on delete rather than dropping a row
(`inserting_a_line_before_a_size_arrives_keeps_every_row`); and `CSI 1 J` blanks only up to and including
the cursor, leaving the rest of the row (`erasing_to_the_cursor_keeps_the_text_after_it`). A thirteen-case
harness runs each hostile shape in its own thread with a two-second join
(`adversarial_escape_input_terminates_and_renders`), so a new non-terminating input fails a test instead of
hanging the suite — which is how the tab loop was found.
