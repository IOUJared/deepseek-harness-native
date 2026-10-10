# Native opt-out composition

The local preference path is now qualified through the **real native App → worker → Core private pipe → production usage bridge/store**, alongside the unchanged full base+web `0.2.1-alpha.1` profile. This adds verification, not quota capabilities or a new runtime/profile/API.

## Evidence

[Round-36 qualification](<../app/evidence/usage-composed-gvlojcr6/qualification.json>): **756 default-feature example tests + one ignored**, including three new fixture regressions, and a release build. Frozen prior 219 canonical native sources are unchanged; only the private example is added. Four Python supervision/metrics sources are independently frozen. Node: **26.11.1**.

| Actual native action | Verified result |
| --- | --- |
| Refresh a new owned PUBLIC legacy document | Version 1, revision 4, saved modes/flags, threshold 17.5 |
| Attempt enabling/threshold/no-op Save | No new operation ticket admitted |
| Explicit Off → Save | Matching real persisted revision-5 ACK through App/worker/Core/store |
| Refresh | Exact committed Off values; fractional threshold preserved |
| Graceful exit, independent process restart, Refresh | Same persisted document/revision; no reseed or second Save |

Both real native/full-alpha process trees stopped gracefully, with no observed survivors. The private document is mode 0600; the usage kernel lock was released before restart and after the second stop. The parent inspected the [initial](<../app/evidence/usage-composed-gvlojcr6/source-composed/app/evidence/full-alpha-public-optout/initial/own-window.png>) and [restored](<../app/evidence/usage-composed-gvlojcr6/source-composed/app/evidence/full-alpha-public-optout/restored/own-window.png>) own Wayland/wgpu frames: readable unavailable copy, Off values, revision 5, disabled unchanged Save and visible Refresh. Pixels match across independent processes; this is not physical-input or general scaling qualification.

## Fixture isolation

- Only new private PUBLIC state; no credentials, OAuth/account commands or model prompts. Counts are bounded source/native-command evidence, **not a network firewall or packet measurement**.
- The private adapter reserves an **unsent** local catalog flight before actual Ready. This suppresses unrelated startup catalog I/O without fabricating a catalog success or weakening Settings guards. The background “Loading models” belongs to this fixture state; normal startup behavior is unchanged.
- Settings opens on Codex before actual Ready, avoiding the default API-login metadata read. Normal non-smoke App/worker guards and real receipts remain active.
- Manual business messages are blocked; actual Iced draft-read completion is forwarded so shutdown cannot wait on a discarded receipt. Only the own renderer is captured.
- Existing identity/pidfd supervision is reused. A [PUBLIC dummy interruption check](<../app/evidence/usage-composed-gvlojcr6/supervision-interrupt.json>) sends SIGTERM to the owned qualifier with a detached two-process dummy tree: qualification fails as expected and bounded cleanup reaps both. This is one schedule, not a real-Host interruption campaign.
- Initial staging/compilation failures and an intermediate Python-only logging-source change remain recorded. Read-only review caught empty-output logging and interrupt-ownership bugs before real execution; both were corrected. Final native/helper/binary hashes are stable.

## Repeat in a new owned home

[Native example](<../app/examples/usage_composed_smoke.rs>) and [supervision adapter](<../scripts/qualify-composed-usage.py>):

```sh
cargo build --release --offline --locked --manifest-path app/Cargo.toml --example usage_composed_smoke
python3 -B scripts/qualify-composed-usage.py \
  --binary "$PWD/app/target/release/examples/usage_composed_smoke" \
  --runtime "$PWD/../deepseek-harness-linux/apps/cli" \
  --output "$PWD/app/evidence/usage-composed-NEW"
```

Use the existing Cargo cache/two-job serialization. Output must be new; for a frozen build it must lie beneath that frozen manifest’s private evidence directory. The adapter preserves the same owned HOME/XDG/profile across two processes.

## Limits

This verifies **local opt-out persistence**, not live quota, subscription buckets/resets, notifications, retry execution, auth, image generation, physical input, power loss or platform parity. SDK Settings/API-key/model/runtime/account flows have separate evidence. The [official-interface limits](<NATIVE-CODEX-INTERFACE-AUDIT.md>) and [readback/opt-out policy](<NATIVE-CODEX-USAGE-UPGRADE.md>) are unchanged. Installed runtime, profiles, accounts and desktop are untouched. The current 6ffa development archive remains unchanged; verifier-only work is not an installation or publication.
