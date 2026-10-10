# Native chat visibility

- Host-confirmed inbox input appears as **You · sent** before request preparation. Claiming it does not erase it; a matching append-origin user message takes over by message ID, never by text. Multiple inputs in one insertion share one grouped bubble.
- Chat preserves append-origin conversation and tool activity. Model-context replacements are validated separately and appear only in Session records; they cannot erase human history.
- Assistant text and backend-supplied reasoning stream into chat, then hand off once to the durable response. No hidden reasoning is requested or fabricated.
- Preparation errors, interrupted/stopped responses and output limits remain visible. Authentication failures use generic text because provider messages can contain credentials.
- Exact title and model-selection control events remain absent from both chat modes. Empty paginated views retain Older messages and Reload.

This is presentation only: no prompt replay, queue execution, cancellation, model selection or Send authority is inferred. The retained history window remains bounded; Older messages loads earlier records. Cancellation diagnostics stay visible beside retained submitted input.

## Checks

Synthetic [reducer tests](<../app/src/chat_visibility_tests.rs>), [follow/view tests](<../app/src/chat_flow_ui_tests.rs>) and a [recorded snapshot](<../app/fixtures/chat-visibility.json>) cover delayed user publication, preparation failure, ID handoff, grouped input, supplied reasoning/output, context replacement and authentication-message suppression. Native PUBLIC render fixtures start no Host or model. These checks do not establish a live provider's availability.
