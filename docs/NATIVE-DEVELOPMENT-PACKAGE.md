# Native Linux development package

## Scope

- **External-runtime development archive**, not installer/AppImage/update channel or standalone Host. Native Iced/wgpu; no Electron/mandatory primary webview.
- Requires built isolated alpha CLI `0.2.1-alpha.1`; installed RC is incompatible. Local archives do not certify model/account/tools/plugins offline.
- pnpm 11.7 offline/ignore-scripts deployments failed: missing `@agentclientprotocol/sdk@1.4.0` metadata (bare CLI), LibreOffice-kit metadata (full closure). No downloads/scripts; partial staging excluded. Unfrozen/hoisted flag changes are not proof of closure.
- Release gates: required peers, one Cordis identity, materialized vendor links/assets/native modules, self-contained Host/Node and closure-specific licensing.

## Run

Extract into a new owned directory; retain root launcher/manifest/executable layout.

| Requirement | Limit |
| --- | --- |
| Python | 3.11+ |
| Node | Major 26; only 26.10.0 on this Arch x86_64 host qualified |
| glibc | 2.44 required by inspected ELF; older refused, other systems unqualified |
| Display/dependencies | Native Wayland; system fonts/GPU/Vulkan/EGL/libraries/tools; no XWayland fallback |
| Interpreter | Fixed `/usr/bin/node`; optional trusted `--node /absolute/node-executable`, no PATH/fallback |

```sh
./launch.py --workspace /absolute/existing/workspace \
  --runtime /absolute/isolated-alpha-fork/apps/cli --check

./launch.py --workspace /absolute/existing/workspace \
  --runtime /absolute/isolated-alpha-fork/apps/cli
```

- `--check` validates manifests/digest/paths/marker and **executes selected Node's version command**, including explicit selection. No data/lock/GUI/Host; prerequisites-valid is not complete dependency/startup proof. Manifest digest is consistency, not signature/authenticity.
- Canonical executable-regular-file validation/version parsing is not authentication/sandbox/exec-time inode binding. Core/tools retain `PATH=/usr/bin:/bin`; Node libraries/Host remain external.
- User home: explicit `--user-home` or HOME. Storage: build-SHA-qualified `.local/share` directory (existing parent), or `--data-home /absolute/new-container`.
- Refuses foreign/build-mismatched containers, symlink/nonprivate children and overlap with package/runtime/default `.dsh`/inherited DSH_HOME. Containers/Host 0700; marker/lock 0600. Failure may leave storage; **no deletion, rollback, migration or old-profile adoption**. Not arbitrary-profile confinement.
- Nonblocking advisory lock survives exec and possible Node inheritance; surviving inheritor may retain it. No resident Python supervisor. Directory-fd/inode checks detect ordinary acquisition replacement, not later same-UID races; preserve active storage.
- Only selected locale/Wayland/session environment forwarded. DSH_HOME is exclusion only; no keys/cookies/NODE_OPTIONS/LD_PRELOAD/arbitrary DSH/X11. Trusted custom hooks may have effects; no network/adversarial guarantee.

## Package and source contents

- Executable/launcher/external-runtime manifest/instructions; native app/Core/transport source, examples/fixtures, Cargo manifests/lock, MIT notices and available cached Rust license texts.
- Excludes npm runtime, browser/Python/Office/model sidecars, credentials/profiles/history, evidence, Cargo cache/targets.
- License inventory is a lockfile/target **superset**, not exact linked closure/legal certification. External runtime/Office licensing remains separate.

## Relocated keyless qualification

```sh
./launch.py --runtime /absolute/isolated-alpha-fork/apps/cli \
  --workspace /absolute/owned-empty-0700-workspace \
  --user-home /absolute/owned-empty-0700-user-home \
  --data-home /absolute/new-private-native-container \
  --qualify-evidence /absolute/owned-empty-0700-evidence --scale 1.25
```

- Opt-in creates one PUBLIC blank session, no prompts/catalog requests; own renderer only, eight-second graceful-stop request. Reports/PNGs create-new 0600.
- Explicit `--smoke-evidence-root` requires existing owned 0700 destination; omission preserves source-local restriction. No automatic output to old checkout.
- Scripted startup/lifecycle/paint, not physical input/IME, accounts/generation, all settings/exports/files or parity. Kernel/filesystem delays lack universal shutdown bound.
- Launcher/qualifier records actual Node patch/accepted major; accepted patches are not all certified. Output bounds apply after buffered capture, not adversarial allocation protection.

## Qualified artifact

**All archive/evidence links here are local ignored artifacts, unavailable on GitHub—not public downloads.** Each archive is frozen; later source/title changes are not included automatically.

| Artifact | Exact identity |
| --- | --- |
| [ac91 local archive](<../app/evidence/package-node-copy-refresh-qpwmgtsn/archive/dsh-native-linux-x86_64-development-0.1.0-ac91006577d1.tar.gz>) | `dsh-native-linux-x86_64-development-0.1.0-ac91006577d1.tar.gz`; **7,210,903 bytes**, SHA-256 `04ea1520cf0fede110a0340d74d72e16f8bdaabca0244b668f9da5b9f33276de` |
| Executable | **16,047,104 bytes**, SHA-256 `ac91006577d151b01b384da88c6bedc40f38053f3bc7248d911dcb9aea1d7bd5` |

- Includes [Copy](<NATIVE-DETAIL-COPY.md>)/[explicit Node](<NATIVE-EXPLICIT-NODE.md>)/fenced Details/layout/cache; **not later title projection**.
- [Parent](<../app/evidence/package-node-copy-refresh-qpwmgtsn/qualification.json>)/[UI certificate](<../app/evidence/package-node-copy-refresh-qpwmgtsn/native-ui-qualification.json>): **29 PUBLIC frames**, default startup; **484/491/493/495** default/feature/layout/scroll tests. Copy tasks not executed.
- [74 package guards + five Node observations](<../app/evidence/package-node-copy-refresh-qpwmgtsn/package-guard-tests.log>); canonical control/non-UTF8 paths now revalidated.
- [Extraction](<../app/evidence/package-node-copy-refresh-qpwmgtsn/relocated package with spaces/extraction-result.json>): **1,141 members/21,536,823 expanded bytes/121 native files**, exact staged→tar→extracted bytes/permissions. Links resolve; [offline all-target check](<../app/evidence/package-node-copy-refresh-qpwmgtsn/extracted-all-targets-check.log>) uses shared cache, not reproducible compiler/Host closure.
- Extracted checks [default](<../app/evidence/package-node-copy-refresh-qpwmgtsn/extracted-prerequisites-default.json>)/[explicit](<../app/evidence/package-node-copy-refresh-qpwmgtsn/extracted-prerequisites-explicit.json>) observe v26.10.0 without GUI/data/Host.
- Four real launches: default [1](<../app/evidence/package-node-copy-refresh-qpwmgtsn/scale1/result.json>)/[1.25](<../app/evidence/package-node-copy-refresh-qpwmgtsn/scale125/result.json>), private Node [1](<../app/evidence/package-node-copy-refresh-qpwmgtsn/private-node-scale1/result.json>)/[1.25](<../app/evidence/package-node-copy-refresh-qpwmgtsn/private-node-scale125/result.json>). Spaces/Unicode copy matched owned path/inode; blank session/fold, native Wayland, no prompts/catalogs, zero exit/no observed survivors, private markers/released locks.
- Four **1272×1402** frames inspected; default/private match by scale. [Default paint](<../app/evidence/package-node-copy-refresh-qpwmgtsn/scale1/evidence/own-window.png>)/[private enlarged paint](<../app/evidence/package-node-copy-refresh-qpwmgtsn/private-node-scale125/evidence/own-window.png>): readable Rosé Pine/one Settings, no observed clipping. Not physical input/compositor fractional scaling.
- Scoped formatting passes; shared-App formatting still fails. Licenses: **412 superset packages/354 copied texts/58 without texts**; legal review incomplete.
- Eight-second observations: mapping **324–340ms, not Host-ready**; RSS **334,936–339,020 KiB**, PSS **253,771–281,916 KiB**; CPU 0% sampled across 16 warm samples/run. Interpreter sharing/sampling differ; no matched improvement/long-chat claim.

### Previous frozen lifecycle refresh

| Artifact | Exact identity |
| --- | --- |
| [ba14 local archive](<../app/evidence/package-lifecycle-refresh-_20f978m/archive/dsh-native-linux-x86_64-development-0.1.0-ba141829f8e7.tar.gz>) | `dsh-native-linux-x86_64-development-0.1.0-ba141829f8e7.tar.gz`; **7,207,527 bytes**, SHA-256 `4ea14de27a69c2b75e17d75fd92cd679abe7b150c88913c192a51a7fd83b685a` |
| Executable | **16,044,688 bytes**, SHA-256 `ba141829f8e7f50fb936fa182b0d6442e3b458710d257ea527119a1b3b02aaeb` |

- [Lifecycle fix](<NATIVE-DETAIL-LIFECYCLE.md>)/[Node observation](<NATIVE-NODE-OBSERVATION.md>), **no Copy/Node override/title fix**.
- [Parent](<../app/evidence/package-lifecycle-refresh-_20f978m/qualification.json>)/[UI](<../app/evidence/package-lifecycle-refresh-_20f978m/native-ui-qualification.json>)/[paint comparison](<../app/evidence/package-lifecycle-refresh-_20f978m/paint-comparison.json>): 29 unchanged PUBLIC frames + inspected keyless startup; **474/481/483/485 tests**, 17 Details cases.
- [66 guards](<../app/evidence/package-lifecycle-refresh-_20f978m/package-guard-tests.log>); [extraction](<../app/evidence/package-lifecycle-refresh-_20f978m/relocated package with spaces/extraction-result.json>): **1,141/21,522,812 bytes/121 source hashes**. [All-target check](<../app/evidence/package-lifecycle-refresh-_20f978m/extracted-all-targets-check.log>)/[prerequisites](<../app/evidence/package-lifecycle-refresh-_20f978m/extracted-prerequisites.json>) passed.
- Real runs [1](<../app/evidence/package-lifecycle-refresh-_20f978m/scale1/result.json>)/[1.25](<../app/evidence/package-lifecycle-refresh-_20f978m/scale125/result.json>); inspected [1](<../app/evidence/package-lifecycle-refresh-_20f978m/scale1/evidence/own-window.png>)/[1.25](<../app/evidence/package-lifecycle-refresh-_20f978m/scale125/evidence/own-window.png>) **1272×697** frames, not prior-frame equivalence.
- [Shared formatting failed](<../app/evidence/package-lifecycle-refresh-_20f978m/shared-format-check.log>)/[four owned files passed](<../app/evidence/package-lifecycle-refresh-_20f978m/owned-format-check.log>); license inventory **412/354/58**.
- Mapping ≈**327/328ms**; RSS **334,490/336,032**, PSS **246,493/248,236 KiB**; CPU 0% sampled, 16 warm samples/run—not Host-ready/matched performance.

### Previous frozen formatting refresh

| Artifact | Exact identity |
| --- | --- |
| [33f local archive](<../app/evidence/package-current-native-7duwm_xn/archive/dsh-native-linux-x86_64-development-0.1.0-33f79d25f020.tar.gz>) | `dsh-native-linux-x86_64-development-0.1.0-33f79d25f020.tar.gz`; **7,204,163 bytes**, SHA-256 `538706e73d073ebfea3759dff2cb21a043a2a193136efd2dff182f6af8358229` |
| Executable | **16,042,432 bytes**, SHA-256 `33f79d25f020f84a7ed65174417d489f0b161c6e503880353e299d7a16e3cf5d` |

- [Formatted Details](<NATIVE-FORMATTED-DETAILS.md>)/[two-width cache](<NATIVE-HISTORY-PERFORMANCE.md>); [parent](<../app/evidence/package-current-native-7duwm_xn/qualification.json>)/[UI](<../app/evidence/package-current-native-7duwm_xn/native-ui-qualification.json>)/[paint comparison](<../app/evidence/package-current-native-7duwm_xn/paint-comparison.json>): 29 PUBLIC frames, six changed + startup inspected/23 unchanged; **465/472/474/476 tests**.
- [60 guards](<../app/evidence/package-current-native-7duwm_xn/package-guard-tests.log>)/[extraction](<../app/evidence/package-current-native-7duwm_xn/relocated package with spaces/extraction-result.json>): **1,141/21,508,426 bytes/121 hashes**. Own-builder bounds after tar parsing, not adversarial streaming/signatures.
- [Extracted all-target check](<../app/evidence/package-current-native-7duwm_xn/extracted-all-targets-check.log>) uses shared cache; not reproduced binary or all composed examples; Rust deps not vendored.
- Runs [1](<../app/evidence/package-current-native-7duwm_xn/scale1/result.json>)/[1.25](<../app/evidence/package-current-native-7duwm_xn/scale125/result.json>), inspected paint [1](<../app/evidence/package-current-native-7duwm_xn/scale1/evidence/own-window.png>)/[1.25](<../app/evidence/package-current-native-7duwm_xn/scale125/evidence/own-window.png>).
- [Shared formatting failed](<../app/evidence/package-current-native-7duwm_xn/shared-format-check.log>)/[owned scoped passed](<../app/evidence/package-current-native-7duwm_xn/owned-format-check.log>); gate retains `formatCheckPassed:false`. Licenses **412/354/58**; observed Node patch or null, not hardcoded.
- Mapping ≈**328/331ms**; RSS **337,024/334,112**, PSS **255,663/253,629 KiB**; CPU 0% sampled/16 samples. Concurrent compilation—not uncontended benchmarks.

### Previous frozen UI refresh

- [e933 local archive](<../app/evidence/package-ui-refresh-nr_uq83v/archive/dsh-native-linux-x86_64-development-0.1.0-e93345bd6921.tar.gz>): `dsh-native-linux-x86_64-development-0.1.0-e93345bd6921.tar.gz`, **6,886,501 bytes**, SHA-256 `77a27a5da90aac858679cb4a2c0fe23c39a2ccebf5186c820af89372193e2660`; executable SHA `e93345bd6921e6a58b6dfe52fb8b02caa02ba082cb3b104f55db87cd0a57b19c`.
- [Parent](<../app/evidence/package-ui-refresh-nr_uq83v/qualification.json>): **314/316/318 tests, 33 guards**; **1,113 entries/20,423,284 expanded bytes/98 source hashes**, exact identities/permissions/no unsafe entries. Includes examples/fixtures; extracted feature compilation uses shared cache, not execution/reproducible backend closure.
- Spaces-path runs [1](<../app/evidence/package-ui-refresh-nr_uq83v/scale1/result.json>)/[1.25](<../app/evidence/package-ui-refresh-nr_uq83v/scale125/result.json>), inspected [1](<../app/evidence/package-ui-refresh-nr_uq83v/scale1/evidence/own-window.png>)/[1.25](<../app/evidence/package-ui-refresh-nr_uq83v/scale125/evidence/own-window.png>): system v26.10.0, blank fold/Wayland/clean exit/private locks; launcher execs. **External Host relocation unqualified**.
- Mapping ≈**322/326ms**; RSS **334,868/333,808**, PSS **254,149/252,750 KiB**; CPU 0% sampled/16 samples, not Host-ready/long-chat/toolkit comparison. License superset **410/352/58**.
- Older [cc53 archive](<../app/evidence/development-package-02/dsh-native-linux-x86_64-development-0.1.0-cc53f19aff0e.tar.gz>)/[qualification](<../app/evidence/parent-development-package-qualification.json>) remain frozen.
- Pending: self-contained runtime/addon/license closure, older Linux baseline, installer/integration/update/rollback, real input/accessibility/account/model flows, representative performance and parity. Installed runtime/data/desktop remain preserved.
