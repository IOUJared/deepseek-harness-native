# Native Linux feature status

Development port, **not Windows/macOS parity**. Source audits/scripted fixtures are not live official-desktop comparisons. [Qualification](<NATIVE-APP-QUALIFICATION.md>) · [Run instructions](<../app/README.md>) · [Web UI parity mapping](<NATIVE-UI-PARITY.md>).

| Area | Implemented / evidence | Remaining |
| --- | --- | --- |
| Native window | Iced 0.14/wgpu; owned Wayland, inspected app scales 1/1.25 | Compositor fractional scaling, resize/input/IME/CJK/accessibility |
| Lifecycle/authentication | Isolated alpha Host; private cookie/control/HTTP/WS; pidfd-owned teardown | Broader load/shutdown; adversarial containment needs cgroup ownership |
| Registry | Real creation/feeds; [reviewed pin/unpin/archive/restore](<NATIVE-REGISTRY-CONTROLS.md>), six scripted ACKs plus ordered global feeds | Active-work/cross-client/input; rename/fork/workspace UX/import; no session-delete BFF |
| ZIP export | [Review/Confirm, 4 MiB, exclusive 0600 save](<NATIVE-SESSION-EXPORT.md>); two Host runs, independent ZIP checks/no-overwrite | Rich/large exports, sync/shutdown/input; sensitive root attachments/live flush; no rollback/extraction/retry/full identity proof |
| History/presentation | Bounded typed follow/page; [native Markdown/code/search and supplied thoughts](<NATIVE-CHAT-FEATURES.md>), [compact tool layout](<NATIVE-CONTROL-LAYOUT.md>), [quiet disclosed records](<NATIVE-RECORDS-PROJECTION.md>) with the **Chat/Trajectory** split, inline tool-row disclosure, tool calls under Thoughts, [content-aware geometry](<NATIVE-CHAT-GEOMETRY.md>) and [bounded Details](<NATIVE-FORMATTED-DETAILS.md>) | Real model streams/representative scrolling, inline diffs/path navigation, grouped tool runs; not Web clone |
| Models/Send/Stop | [Owned startup/picker catalog reads](<NATIVE-CATALOG-OWNERSHIP.md>), explicit model/reasoning/request controls and typed prompt/cancel; [native DeepSeek page](<NATIVE-DEEPSEEK.md>) (secure key, route models, live `baseURL`/`thinking`/`reasoningEffort`) | Routes other than the backend's DeepSeek catalog; catalog reads may contact providers; live generation unverified |
| API key | Masked consuming Review/Confirm; [sticky unknown-save block](<NATIVE-SAVE-UNCERTAINTY.md>); [real PUBLIC-dummy App→provider persistence](<NATIVE-COMPOSED-SETTINGS.md>) | Physical input/real-key validity; trusted custom paths not confined; ACK/metadata is not validation |
| ChatGPT/GPT | [One-click Codex browser flow](<NATIVE-CODEX.md>), masked fallback, revision-CAS route activation; [managed accounts/select/local sign-out](<NATIVE-MANAGED-CODEX.md>) with PUBLIC Loader checks | Real OAuth/identity/entitlement/generation; device login and remote authorization revocation |
| Codex runtime/usage | [Native transport/cache/timeouts/image-input controls](<NATIVE-CODEX-USAGE-POLICY.md#remaining-supported-controls>), shared retained drafts, explicit CAS save/reconciliation; actual Rust pipe and frozen native paint/release; local usage preferences persist | Official quota/reset observation, actual alerts/reset retry, image generation/sketch; live-account/input and cross-platform qualification |
| Legacy account | [Explicit opt-in observation](<NATIVE-LAZY-ACCOUNT.md>), owned unsubscribe/shutdown | Shared services still mount; isolated guest/official Linux provider support; no platform spoofing |
| Decisions | Native approval/untimed UI; [owned tickets](<NATIVE-DECISION-OWNERSHIP.md>): 46 worker/23 interaction tests, 285 shared tests/seven transport races; [actual Gateway fixtures](<NATIVE-COMPOSED-DECISIONS.md>), [fixed chrome](<NATIVE-DECISION-LAYOUT.md>) | Reject/Cancel/tool execution/reconnect/multi-client/input; [timed transport](<NATIVE-TIMED-QUESTIONS.md>) (15 fake-wire/eight Host tests) does not qualify timed/continued UI |
| Agent authority | Distinct AgentId/SessionId; [exact registered identity read](<NATIVE-AGENT-DISCOVERY.md>) | Point observation, not lease; broader selection/shutdown/subagent controls |
| Files | [4 MiB/eight-job upload](<NATIVE-FILE-UPLOAD.md>), [ACK-loss storage proof](<NATIVE-UPLOAD-UNCERTAINTY.md>), [bounded intake](<NATIVE-FILE-INTAKE.md>), [bounded 4-file prompt, 4 files × 4 MiB](<NATIVE-FILE-PROMPT.md>), [actual App/Host admission](<NATIVE-COMPOSED-FILES.md>); [native picker/PNG/JPEG preview and 4-file chip row](<NATIVE-ATTACHMENT-UX.md>) | Physical Wayland drops, model image interpretation/output; real model/history retirement/input; frozen ACK-loss recovery/read-only admission checks need broader crash/concurrency qualification; cold upload may resume; no storage rollback |
| Settings/plugins/skills/MCP | Recognized policy/tool/plugin records; unknown required events disable Send | Broader native editing/secret/preset/management compatibility |
| Terminal/browser/Office/docs | Backend audit only, no native guest | Linux providers and lazy isolated guest lifecycle/resources |
| Goals/todos/subagents/schedules | Bounded known read-only records | Dedicated controls/live qualification |
| Desktop integration | Own native window; [native Wayland clipboard paste](<NATIVE-CLIPBOARD.md>) | Open-with/notifications/file associations/tray/shortcuts; X11 paste |
| Install/update | [Frozen external-runtime development archives](<NATIVE-DEVELOPMENT-PACKAGE.md>); extracted launchers qualified | Not installer; Host/Node closure, older Linux, licenses, integration/update/rollback; Windows/macOS [boundary mapped](<NATIVE-PLATFORM-PORT.md>), nothing built or verified off Linux |
| Performance | [Startup mapping, 8 s and 60 s owned-tree idle, and 4,084-row reflow measured on the r2 release](<NATIVE-PERFORMANCE.md>) (60 s idle 0.47% of one core, 336 MiB RSS, 264 MiB PSS; later reflows 5.54 ms median) | Input and presented-frame latency, VRAM, live-host long chat, sustained/other-hardware repeats; no universal toolkit superiority |

## Acceptance policy

- Buttons, accepted RPCs, fixtures and backend capabilities alone are not completion.
- Explicit consent for approvals/answers, keys, model selection/prompts, policy changes and sign-in; catalog reads can run on startup/picker opening; no automatic retry after indeterminate mutation.
- Tests use PUBLIC fixtures/private homes/owned processes, not real credentials/installed profiles. **Evidence/archive links are local ignored artifacts, unavailable on GitHub—not downloads.** Frozen archives never inherit later source behavior.
- Unsupported timed/continued questions stay unsupported without owned Host claims; never guess deadlines or derive Agent authority from Session IDs.
- Lazy guests never become mandatory primary webviews or receive generic privileged execution/raw native cookies.
- Breaking development API: question methods require actual registered **AgentId**, not SessionId; wire field names unchanged.
