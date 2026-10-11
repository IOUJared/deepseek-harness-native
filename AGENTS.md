# Native Harness development

Preserve the native Iced/Wayland frontend, dark Rosé Pine, isolated alpha backend and installed RC runtime. Do not replace profiles, credentials, released sessions or unrelated concurrent changes.

For frontend, Core, transport and backend integration work, load [impeccable-rust](.agents/skills/impeccable-rust/SKILL.md) and follow [verification](docs/VERIFICATION.md). Assign affected failures to existing production tests; add another verifier only for a documented gap.

For porting the web GUI's layout into the native frontend, load [native-ui-parity](.agents/skills/native-ui-parity/SKILL.md) and build on [the extracted measurements](docs/NATIVE-UI-PARITY.md).

Serialize Cargo with two jobs and locked dependencies; use offline mode with a warmed cache. Never rewrite all shared Rust files to resolve local formatting debt.

Use only supported, exact-version backend profiles. Unit results do not establish real composition, native paint, physical input, OAuth/provider execution or feature parity. Record checks actually run, failures and limitations; preserve private evidence outside publication.

Some evidence only the user can produce: a credential, a GUI or hardware action, a provider quota, a live Host state, or the composed profile of a running app. When a task needs one, ask for that exact step — the command to run or the observation to report — and say what each answer would establish. Continue on everything that does not depend on it, and keep the blocked item explicit rather than guessing at its result. Waiting quietly, or reporting a blocker without the step that would clear it, both waste the one thing the user has that you do not.
