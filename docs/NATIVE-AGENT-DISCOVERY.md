# Native registered-Agent discovery

- With Core `transport`: `Backend::request(Command::DiscoverSessionAgent(SessionId), timeout)` → `PublicReply::AgentDiscovery(AgentDiscovery {session_id,agent_id:Option<AgentId>})`.
- Private control, **not Gateway RPC**. [Controller](<../core/runtime/fork-supervisor.mjs>) scans live registration and checks the exact object. No create/resume/prompt/cancel/follow/account/credential operation.
- Missing → None; ambiguity/stale membership/unavailable service/unsupported ID → failure, never guessing. IDs: nonempty Unicode scalars, **≤512 UTF-8 bytes**, no whitespace/control; complete Core frame **≤64 KiB**.
- [Decoder](<../core/src/agent_discovery.rs>) accepts only sessionId/nullable agentId; request/command correlation plus echoed-session validation. AgentId and SessionId are distinct metadata, **not credentials, leases or grants**.

## Lifetime and use

- Run on worker; result is one synchronous registration observation. Fence Host epoch, selection generation and request ticket; discard stale/timeouts/disconnects.
- Never resume a missing Agent automatically. Later Gateway operations re-resolve identity; disposal/replacement may immediately invalidate it.
- Concrete alpha registry enforces equal Agent/session bytes. Differing-ID adapter tests check semantics, **not valid differing-ID alpha registration**. `agentAvailable` is not identity; history follow can activate.

## Development SDK upgrade

Transport-enabled `Command`/`PublicReply` add variants; update exhaustive matches. Default non-transport Core does not expose discovery. No dependency, alpha RPC or persistence change; UI integrations remain separately qualified.

## Qualification

Ignored [parent manifest](<../app/evidence/parent-agent-discovery-qualification.json>) is **local-only**.

| Historical checks | Result |
|---|---|
| Node | Four discovery + 18 observer cases = 22 |
| Core/App | 56 transport Core + six privacy doctests; 50 default Core; 285 App; all-target checks |
| Qualifier denial tests | Eight |
| Real registry/factory | Absence→registered exact object→repeat→post-disposal absence; no substituted provider |
| Typed SDK | Explicit separate BFF creation, identity/echo/repeat, history/roster/control, stop/revocation; no prompt/catalog |
| Production regression | Eight-second keyless Wayland smoke at scale 1.25, graceful stop |

First Node run's nonexistent `turn/step` counter is superseded by real step/start/end counts. Counters/snapshots are not process-lifetime network/model tracing; SDK did not qualify post-disposal absence. Runner source pins cover listed inputs, not complete dependency closure. Physical input, files/previews, subagent/continued controls, performance and standalone installation remain unfinished; old packages do not inherit changes.
