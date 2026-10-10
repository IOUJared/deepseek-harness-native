# Provider-backed request controls

| Native choice | Backend selection | Provider request |
|---|---|---|
| Provider default | Field omitted | Preserve existing provider/SDK behavior |
| Standard | `serviceTier: "default"` | `service_tier: "default"` |
| Fast · premium | `serviceTier: "priority"` | `service_tier: "priority"` |
| Concise / Balanced / Detailed | `textVerbosity: "low" / "medium" / "high"` | `text.verbosity` |

- Only catalog-advertised choices are interactive. The isolated backend checks exact model capabilities before selection or provider I/O; custom gateways and unrecognized models do not acquire capabilities from their names.
- Fast requests premium processing, not a latency guarantee. API pricing or ChatGPT quota consumption can increase; entitlement, workspace policy and provider downgrade still apply. See [OpenAI Fast mode](https://developers.openai.com/api/docs/guides/fast-mode) and [Codex speed](https://learn.chatgpt.com/docs/agent-configuration/speed).
- Output detail controls answer verbosity. It does not change reasoning effort, token limits, truncate UI text or replace the prompt.
- Selection applies to the next assembled request. The current request stays frozen; session selections and request headers retain the controls for replay/resume. Model-route changes clear inherited controls; effort changes retain them.
- The existing selection operation attempts to save the default asynchronously. Its ACK is not proof that the default reached disk. Choosing Provider default clears the explicit setting; it does not assert Standard speed or Medium output.
- Opening/browsing remains local. Native actions keep epoch, session-generation, opening and section checks; failures restore authoritative labels.

Implementation: [native request choices](<../app/src/model_request_controls.rs>), [popup routing](<../app/src/model_controls.rs>), [native wire DTOs](<../transport/src/dto.rs>). The pinned provider catalog may omit newer documented models; this feature does not invent account entitlement, add an authenticated model route or change the installed Web runtime.

Qualification uses captured requests and keyless composition tests, not charged provider calls or measured Fast latency. Native: 520 default / 527 fixture-enabled app tests and 98 transport tests; six visually inspected public frames, including small windows and an unloaded catalog. [Retained release and qualification](<../app/evidence/request-controls-o5ll5j2z/native-qualification.json>); [speed preview](<../app/evidence/request-controls-o5ll5j2z/native-speed-preview.png>). Renderer metadata is explicitly synthetic. [Backend qualification](<../app/evidence/request-controls-o5ll5j2z/backend-qualification.json>) records built artifacts, real Loader restart/restoration, captured serializers and both SDK replays; unrelated trajectory/Codex repository checks remain red. Changes are local, not installed or pushed.
