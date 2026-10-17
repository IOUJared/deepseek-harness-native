# Session prompt: finish the feature-complete native clone

Paste everything below into a fresh session started in
`/home/jared/Documents/deepseek-harness/default-workspace`.

---

## Load first

`deepseek-harness-native/AGENTS.md` governs this repository. Before frontend work load
`.agents/skills/native-ui-parity/SKILL.md` (layout porting; build on `docs/NATIVE-UI-PARITY.md`) and
`.agents/skills/impeccable-rust/SKILL.md` (verification discipline; follow `docs/VERIFICATION.md`).
Assign every failure to an existing production verifier; add one only for a documented gap.

## Mission

Take the Rust/Iced clone of the DeepSeek Harness web UI to **feature complete**, in this repository
(`deepseek-harness-native`, branch `main`), by cloning the reference implementation in
`../deepseek-harness-linux` feature by feature.

Rules of engagement, in force for the whole session:

- **Native only**: Rust + Iced + wgpu. No webview, no Electron, no new dependencies without explicit
  approval. Reuse the existing backend, transport and Core; never fork a second wire path.
- **Strict 1-to-1**: when the reference and the native app differ, the reference wins. Do not offer
  options or "improvements"; close the gap that is proven.
- **Local only**: one commit per item, imperative subject. **Never push, publish, deploy or open a
  PR.** The repository has remotes configured; leave them untouched.
- **Honest claims**: say what was verified and how. "Compiles", "tests pass", "captured" and "pressed
  it and it worked" are four different claims.

## Where the truth lives

- `docs/NATIVE-UI-PARITY.md` — **start here**. Its first block is the current state, the next step and
  the four rules earned the hard way. The rest is a per-item record with `file:line` citations,
  measured before/after values, deliberate differences and remainders.
- `docs/FEATURE-PARITY.md` — the older, broader feature inventory (many items are done; check the
  parity doc before treating anything here as open).
- `../deepseek-harness-linux` — the reference. Read the artifact that *defines* a fact (a generated
  descriptor, a locale file, a component) before writing a name, a string or a shape.
- Captures: `~/.local/share/dsh-macos-shell-reference/shots/` (numbered), mirrored but git-ignored in
  `app/evidence/`.

## Verification gates (capture every exit status; never read status from output text)

```bash
# app suites — fixture feature and default profile BOTH matter
cargo test --manifest-path app/Cargo.toml --locked --offline --jobs 2 \
  --features public-layout-fixture --bin dsh-native-app -- --test-threads=2
cargo test --manifest-path app/Cargo.toml --locked --offline --jobs 2 \
  --bin dsh-native-app -- --test-threads=2
# transport, core, JS runtime
cargo test --manifest-path transport/Cargo.toml --locked --offline -j2
cargo test --manifest-path core/Cargo.toml --locked --offline -j2 --lib
node --test $(python3 -c "import glob;print(' '.join(sorted(glob.glob('core/runtime/*.test.mjs'))))")
# formatting debt (must be 0; never edit its baseline)
python3 -B scripts/format-debt.py
```

Baseline at the time of writing: fixture **1134 passed**, default **1127**, transport **17 + 82 + 7**,
core **112**, JS **419 / 0**, format debt **0**, tree clean, 343 commits.

Hard rules around the gates:

1. **Capture the exit code** — `cmd | tail` returns `tail`'s status. `FIX=$?` or refuse the commit.
2. **Run both app profiles.** A change that only the fixture profile accepts is a configuration, not a
   change. One round shipped a red default build because only the fixture suite was run.
3. **Format before committing, not after** — and never run `cargo fmt` between two scripts that must
   agree on the same text.
4. **Gate the commit on all of them**: `if [ $FC -ne 0 ] || [ $DC -ne 0 ] || [ $FMT -ne 0 ]; then refuse; fi`.

## Capture recipe

The fixture is a self-checking binary; the shell script around it can fail for its own reasons.

```bash
mkdir -p /tmp/dsh-shots
EXTRA="--settings true --shortcuts true" bash /tmp/dsh-capture.sh NAME MODE 3
# when the script reports "all attempts failed", run the binary directly FIRST:
./app/target/release/examples/layout_smoke --mode settings-general \
  --screenshot /tmp/dsh-shots/x.png --report /tmp/dsh-shots/x.json --shortcuts true
# then read the report's booleans: passed / allTextCollected / paintVisibilityQualified /
# expectedControlCountsMatch / presetStatesMatch
```

- Build with `--features public-layout-fixture --example layout_smoke`, and **check the build exit
  before capturing** — a failed build leaves the previous binary in place and the capture silently
  photographs it.
- Modes are prefixed: a settings surface is `settings-*` (`settings-general`); the fixture derives its
  expectations from the mode name (`preset.starts_with("settings-")`).
- Flags must be registered in the fixture's accepted list or they are **refused**, not ignored.
- The window is captured on DP-2 at 2400×1330; crop with PIL and *look* at the image. Padding and
  layout changes have no measurement contract — a green suite proves nothing about them.
- When the fixture's check disagrees with a correct picture, read the check's inputs before touching
  the picture. Twice the fix was one line in the *expectation*.

## State of play

**Done, tested and captured:** permission select (chip, Host menu, `/permission` line, guarded
full-access confirmation, default row); agent-preset roster and selection (header label, hero seat,
seat list, staged pick, locked announcement); background jobs end to end (transport, mux, worker,
counted header action, panel, row dots, two-click kill, per-job output stream, expansion with the
reference's "… earlier output dropped …" copy); and the shortcuts editor **complete in behaviour**
(store with revision guard, catalog, resolver, `Ctrl+/` trigger, sorted rows, "Modified", Remove,
Restore default, customized count, Host commands group, guarded restore-all, capture-a-key).

**Tested but not wired** (grep for callers before claiming a surface exists): `catalog_rows` is wired,
but the ten-ish `COPY_*` constants are only partly used; `search_index` has no caller.

**Next, in order, smallest first:**

1. **A fixture flag for the armed *capture* state** — the branch compiles, the pure half
   (`keybindings::binding_from`) has nine test cases. Two edits: a setter on `App`, a flag.
2. **Keys as data** — a `Message::KeyPressed(key, modifiers)` routed by `update` through the whole key
   chain with live state, so a stored binding can **replace** a hardcoded one. Not a store-first
   subscription: that swallows `Ctrl+V` and `Ctrl+C`, which the paste/clipboard links own. The
   fallback route already carries *unclaimed* presses as data, so this is unifying the chain, not
   inventing a mechanism.
3. **Rename the stale `fixed.` prefix** on this app's five ids (established cosmetic in the parity
   doc). Do it with a scoped transform and run both profiles.
4. **Then the rest of the reference's feature set**, in the order `docs/FEATURE-PARITY.md` and the
   parity doc's Open items name. If the reference has a feature the native app lacks and it is not
   listed, that is a finding: size it before building it.

**Needs the user (do not guess, ask for the exact step):** the reference screenshot at
`/home/jared/Documents/deepseek-harness/default-workspace/reference.png` (absent; expected sha256
`b8e164f5…`, 1980×1319); a real window-edge drag to confirm the 520×600 floor (programmatic resizes
below the minimum are blocked); one live close-timing run against the shell's 10/5/5 s budgets; and the
remote-image fetch policy.

**Blocked by the toolkit:** inline-code chips — Iced's `rich_text` has no span backgrounds. Do not
fake it with spacing.

## Working method that has actually worked here

- **Read before editing.** Every structural failure in this repository's history came from an anchor
  written from memory: a reformatted variant list, a moved match arm, a private type crossing a module
  boundary. Read the exact current text, or locate the line by a *short* unique anchor.
- **Write abort-safe scripts**: assert every anchor first, write once at the end. A missed anchor then
  costs nothing.
- **Prefer new files and pure functions** for new behaviour; they compile-check and test cheaply. Edits
  to `app/src/ui.rs` (~11k lines) need their production anchor read immediately before the edit — the
  fixture is a second configuration that will happily accept what the release build refuses.
- **A control that does nothing is worse than no control.** `Settings::update` has a wildcard arm, so a
  new `Action` variant is silently inert: wire the App interception (`Message::Settings(...)`) or do
  not ship the button.
- **Destructive actions keep their confirmation visible** — the confirmation names what will change and
  Cancel is a first-class control.
- **Remove dead-but-tested code's ambiguity in the document**, not by deleting it: say which functions
  are capabilities and which are shipped surfaces.
- **When you are wrong, say so in a commit** and correct the document the same round. The parity doc
  carries twelve such corrections; they are why it can be trusted.

## Definition of done

Feature complete means: every capability the reference's UI exposes has a native equivalent that is
either **captured** (with a graded fixture run) or **tested** (with the test named), every deliberate
difference is recorded with its reason in `docs/NATIVE-UI-PARITY.md`, both app profiles and all four
suites are green with captured exit codes, and the only open items are the ones that need the user's
hands, the toolkit, or a decision that is theirs to make.

Write that closing table into the parity doc when you get there, and do not mark the goal complete
before it is true.
