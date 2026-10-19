# Session prompt: keep coding the native clone to 1-to-1

Paste everything below the line into a fresh session started in
`/home/jared/Documents/deepseek-harness/default-workspace`.

---

## Mission

Take the Rust/Iced clone of the DeepSeek Harness web UI to a **strict 1-to-1 clone**: same layout,
same behaviour, same copy, genuinely native rendering. The reference implementation is the sibling
checkout `../deepseek-harness-linux`; the clone is `deepseek-harness-native`, branch `main`, local
commits only.

**Work autonomously and do not ask questions.** Decide from the reference source, the existing
documents and the code. When something genuinely needs the user (a file, a charged provider turn,
hardware, a policy), write it into `docs/NATIVE-UI-PARITY.md`'s user-dependent list with the exact
step, then keep working on everything that does not depend on it. Never stop to wait, never end a
turn with a question, and never ask for permission to continue.

Start by creating a goal (`create_goal`) with this objective so the work continues across rounds:
"Close the remaining native-clone gaps one commit at a time, reference-first, with tests, DP-2
captures and full gates, recording user-only blockers in the docs instead of asking." Keep it active
until the definition of done below is true.

## Load first

- `AGENTS.md` in the repository — the governing rules.
- `.agents/skills/native-ui-parity/SKILL.md` and `.agents/skills/impeccable-rust/SKILL.md`.
- `docs/NATIVE-UI-PARITY.md` — the top status block, "Reference surface coverage", the open items, and
  the latest rounds at the end (they are the current state; this prompt's numbers are only a snapshot).
- `docs/NATIVE-CHAT-GEOMETRY.md`, `docs/NATIVE-LIVE-VERIFICATION.md`, `docs/VERIFICATION.md`.
- `git log --oneline -30` and `git status`.

## Current state (verify, do not trust)

HEAD `f72a4f5`, tree clean, ~430 local commits ahead of origin, nothing pushed. Captures archived
through `shots/231-steer-all.png`. Working gates at that commit: app fixture **1409 passed / 1
ignored**, app default **1394 / 1**, transport **56 + 17 + 91 + 7**, core `--lib` **115**, JS **419
pass / 0 fail**, format debt **0 files / 0 hunks**, `--suite clippy` and `--suite examples` passed. The
app **bin** carries 21 pre-existing dead-code warnings: record the count, never claim it
warning-free.

Two recorded environment facts, not defects to "fix": `--suite test` fails `binary-freshness` when a
present `app/target/release/dsh-native-app` predates the sources (do not rebuild to silence it — record
it), and `wayland-drops` cannot fetch `as-raw-xcb-connection v1.0.1` offline.

## Hard rules

- **Native only**: Rust + Iced + wgpu. No webview, no Electron, no new dependencies (the approved set
  is in `Cargo.lock`: `zeroize`, `accesskit`, `oauth2`, `keyring`, `libc`). Reuse `dsh-native-transport`
  and the existing `/api/<method>` BFF; never fork a second wire path; never rebuild or fork the Host.
- **Strict 1-to-1**: the reference wins. No options, no "improvements", no re-designing. A deliberate
  difference is allowed only with a reason written in the parity doc.
- **Local only**: one commit per item, imperative subject. Never push, publish, deploy or open a PR.
- **Serialize builds**: `--locked --offline --jobs 2`, never concurrent with another build.
- `cargo fmt --manifest-path <crate>/Cargo.toml` before committing; **never `cargo clippy --fix`**;
  never edit `scripts/format-debt-baseline.json`; never reformat files you did not touch.
- Honest claims: "compiles", "tests pass", "captured", "pressed it and it worked" are four different
  claims. A green suite proves nothing about padding — look at the capture.
- Destructive actions keep their confirmation visible; a button must state exactly what it persists.

## Gates (capture every exit status; never read status from output text)

```bash
cd /home/jared/Documents/deepseek-harness/default-workspace/deepseek-harness-native
cargo test --manifest-path app/Cargo.toml --locked --offline --jobs 2 \
  --features public-layout-fixture --bin dsh-native-app -- --test-threads=2   # F
cargo test --manifest-path app/Cargo.toml --locked --offline --jobs 2 \
  --bin dsh-native-app -- --test-threads=2                                     # D
cargo test --manifest-path transport/Cargo.toml --locked --offline --jobs 2 -- --test-threads=2
cargo test --manifest-path core/Cargo.toml --locked --offline --jobs 2 --lib -- --test-threads=2
node --test core/runtime/*.test.mjs
python3 -B scripts/format-debt.py
python3 -B scripts/verify-native.py --suite clippy      # when source or examples change
python3 -B scripts/verify-native.py --suite examples    # when an example changes
```

Rules: capture the exit code (`cmd | tail` returns `tail`'s status); run **both** app profiles; format
before committing; gate the commit on every status.

## Per-item workflow

1. **Reference source first**: read the artifact that defines the fact (generated descriptor, locale
   file, component, host service) — never a name, string or shape from memory or from our own docs.
2. **Transport validation** when a wire is involved: exact request body against the descriptor; a live
   probe (below) or a fake-wire test.
3. **App state → UI**: the control, the reducer, the disabled/refusal states, the failure path.
4. **Test**: press the real `Message`/`Action` (state-direct tests hide the bugs this repo keeps
   finding); smallest production verifier that pins it.
5. **DP-2 capture** for anything visual: build the release fixture, capture, **look at the image**,
   archive it, and give it the next shot number.
6. **Docs note** in `docs/NATIVE-UI-PARITY.md` (round section: what was wrong, what changed, evidence,
   battery table) and correct any stale claim the round touched.
7. **Commit** (imperative subject), then re-run the gates you touched.

## Standing work loop (repeat; never idle)

**A. Audit one subsystem, read-only, with a subagent.** Give it the coverage taxonomy: every action
variant, every state field a draw site reads, every Command/Event pair; per item the construction site,
the handling arm, any wildcard before it, the outcome, and the failure path. Flag only (A) constructed
but never handled, (B) handled but invisible/stuck, (C) unreachable, (D) documented claim vs code.
Exclude style, vendor code and anything already listed as unqualified. It must return findings with
`file:line` and the smallest test. **Do not duplicate its reading in your own context.**

**B. Fix the smallest finding, one commit each,** in this order: silent states first (a control that
does nothing, a refusal dropped, a flag that cannot clear), then wrong words (a notice or doc that
lies), then sizeable missing behaviour.

**C. When a subsystem's audit is closed, take the next unqualified surface.** Audited so far:
settings/management/sidebar (rounds 327-333), composer controls (334-336/341-342), terminal panel
(337-340/343-344), approvals/questions (347-348), schedules (345-346/349-351), MCP panel (353).
Not yet audited this way: the session sidebar and workspace browser, the details/work panel and
deliverables, Codex settings/runtime, Cloudflare, plugins, the model/composer catalogs, goals/todos.

**D. When the reference specifies something the native app lacks, build it.** The path form and the
edit pair landed (round 354: `relativizeToCwd`/`abbreviateHomePath`, `shots/158-…`). Read what the
reference's path link opens before building it: `openFile` addresses the path and calls
`sidebarRight.openResource`, i.e. the **right Sidebar document preview**, and its own test asserts
the desktop opener is not called. Native's right Sidebar is a recorded deliberate difference, so
the path stays display-only unless that difference is revisited. The remaining member of this family
is the **inline diff body**, which the reference's `DiffBlock` defines.

**E. When `reference.png` appears, run the visual diff.** The native side is already captured at the
reference's exact size (`shots/171-chat-reference-size-folded.png`, 1980×1319) and the measured native rects
and comparison order are in
`docs/NATIVE-CHAT-GEOMETRY.md#reference-comparison-at-19801319-blocked-on-the-file`. Compare, fix the
concrete gaps, capture each fix, record the deltas.

## Live Host probe (free reads only)

A closed Host can be booted exactly as the app boots it, without charging a model turn: the pinned
runtime is `../deepseek-harness-linux/apps/cli` (version `0.2.1-alpha.1`), booted with
`loadForkRuntime` + `runProfile` from `core/runtime/`, an isolated `HOME` and `DSH_TAURI_HOME`, and
`DSH_TAURI_RUNTIME` pointing at that `apps/cli` directory. `/tmp/dsh-live8/probe*.mjs` are working
examples (settings/roster probes). A live pass never substitutes for the fake-wire test and vice
versa; [record](NATIVE-LIVE-VERIFICATION.md) what ran and what did not.

## Capture recipe

```bash
cargo build --release --manifest-path app/Cargo.toml --locked --offline --jobs 2 \
  --features public-layout-fixture --example layout_smoke      # check this exit FIRST, warning-free
mkdir -p /tmp/dsh-shots
EXTRA="--settings true --shortcuts true" bash /tmp/dsh-capture.sh NAME MODE 3
python3 -c "import json;d=json.load(open('/tmp/dsh-shots/NAME.json'));print(d.get('status'),d.get('layoutObservation',{}).get('passed'))"
```

- `/tmp/dsh-capture.sh` captures the fixture's own buffer **and** the physical DP-2 window; recreate it
  if missing. DP-2 is 2560×1440 at scale 1.
- Flags must be registered in `examples/layout_smoke.rs` or the fixture **refuses** them.
- Modes are presets: `chat-transcript`, `settings-*`, `sidebar-*`, `plan-mode*`, `queued-idle`,
  `mcp`, `schedules`, `workflow-run`, … see `layout_fixture.rs`'s preset list.
- Archive to `~/.local/share/dsh-macos-shell-reference/shots/<n>-name.png` (+ `-dp2.png`, `.json`)
  and mirror the buffer into `app/evidence/`. Next number is **245** (shot 245 is the newest).
- When the fixture's check disagrees with a correct picture, read the check's inputs before touching
  the picture — twice the fix was one line in the *expectation*.

## User-only blockers (record; never wait)

- `reference.png` at `/home/jared/Documents/deepseek-harness/default-workspace/reference.png`
  (expected 1980×1319, sha256 `b8e164f5…`). Without it no pixel-level 1-to-1 claim may be made.
- One authorized **charged** DeepSeek turn for the live model/feedback/schedule pass.
- A real window-edge drag below 520×600, and one close-timing run against the 10/5/5 s budgets.
- The remote-image fetch policy.
- Optional: one `cargo fetch` of `as-raw-xcb-connection v1.0.1`.

Record these in the parity doc's list with their exact step and what each would establish. They are
never a reason to stop.

## Working method that actually works here

- **Read before editing.** Every structural failure in this repo's history came from an anchor written
  from memory. Read the exact current text, or locate a short unique anchor.
- **Abort-safe scripts**: assert every anchor first, write once at the end.
- **Prefer new files and pure functions**; `app/src/ui.rs` is ~19k lines — re-read the production
  anchor immediately before editing, because the fixture is a second configuration that will happily
  accept what the release build refuses.
- **A control that does nothing is worse than no control.** Wildcard arms have silently swallowed new
  actions here more than once: wire the App interception or do not ship the button.
- **One commit per item**, imperative subject, docs corrected in the same round — including "this
  earlier claim was wrong".
- **Abort a build you cannot finish**: a stale artifact is worse than no evidence.

## Definition of done

Every capability the reference's UI exposes has a native equivalent that is either **captured** (graded
fixture run + DP-2 image) or **tested** (test named), every deliberate difference is recorded with its
reason, both app profiles and all suites are green with captured exit codes, and the only open items
are the user-only ones above. Write the closing table into the parity doc, and only then mark the goal
complete.
