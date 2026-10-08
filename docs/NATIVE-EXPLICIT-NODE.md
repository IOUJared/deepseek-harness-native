# Native explicit Node interpreter

Optional `--node ABS_NODE_EXECUTABLE` selects the native app's interpreter. Omission keeps `/usr/bin/node`; no PATH discovery or installed-runtime fallback. Same-version alpha Host, native/user homes and workspace remain separately required—not bundled-runtime/installer support.

## Validation and startup

- [Parser](<../app/src/config.rs>): one absolute path; no parent traversal/ASCII controls; spaces/Unicode preserved; duplicate/missing values rejected without path echoes. Parsing performs no execution; `--help` starts no GUI/Host.
- [Core](<../core/src/options.rs>) canonicalizes and requires an executable regular file. [Spawn](<../core/src/lib.rs>) uses that absolute executable, a cleared environment and fixed `PATH=/usr/bin:/bin`.
- Selection does not redirect downstream PATH tools or bundle libraries/assets/Host. Validation is not authentication, Node-version certification or immutable exec-time inode binding. Core adds no version policy.

## Qualification

- [Qualifier](<../scripts/qualify-explicit-node.py>) copies system Node into fresh private evidence, checks identical bytes/strict observed v26, then runs a spaces/Unicode path.
- Retained pidfd-owned descendant path/device/inode and PID/start-time checks are point observations, not atomic binding/adversarial containment.
- [Pure tests](<../scripts/test_qualify_explicit_node.py>) cover mismatch/reused identity/read errors without processes/live `/proc`.

**Local evidence, ignored on GitHub:** [record](<../app/evidence/explicit-node-app-9geecs7m/qualification.json>), [29 frames/default startup](<../app/evidence/control-layout-952lk4vy/qualification.json>), private runs [scale 1](<../app/evidence/explicit-node-app-9geecs7m/private-node-scale1/result.json>)/[1.25](<../app/evidence/explicit-node-app-9geecs7m/private-node-scale125/result.json>).

- Five parser/five observation tests; **484/491/493/495** default/feature/layout/scroll regressions; all-target check.
- Private starts observe **v26.10.0**, matching executable identity, real blank session/fold, native Wayland, zero prompts/catalog requests and graceful exit/no observed survivors. Both own frames inspected.

## Distribution limits

- [Development archive](<NATIVE-DEVELOPMENT-PACKAGE.md>) supports app/launcher selection; ba14 remains frozen without it.
- Launcher revalidates canonical paths, executes the trusted selection for its Node-26 probe and forwards it exactly. Rejection never falls back; this is not executable authentication or new Core policy.
- **Local evidence:** [package runs](<../app/evidence/package-node-copy-refresh-qpwmgtsn/qualification.json>) cover default/private starts at both app scales, not compositor fractional scaling.
- Unfinished: runtime/library/license closure, self-contained packaging, general Linux portability, physical input, representative performance, real account/model flows and parity. Installed runtime/profile/GUI/desktop remains unchanged.
