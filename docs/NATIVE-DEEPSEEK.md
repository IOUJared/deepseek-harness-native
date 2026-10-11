# Native DeepSeek surface

Settings → **DeepSeek** (the page previously titled "Models") is the native front end for the backend's DeepSeek route.

## What it does

| Control | Behaviour |
|---|---|
| API key | Secure field. Type, **Paste** button, or Ctrl+V (see [clipboard](NATIVE-CLIPBOARD.md)). Review → Save. |
| Models | Lists the `(display name, model id)` rows the backend reports for its DeepSeek route, with the effective model marked `✓`. |
| Provider options | **Load**/**Refresh** reads the backend's `llm-deepseek` namespace and renders its supported nonsecret primitive fields (endpoint `baseURL`, `thinking`, `reasoningEffort`) through the shared live-settings editor: Begin → Edit → Review → Confirm writes one field against the loaded namespace revision. |

- The key is written through the existing `save_api_key` control request; nothing is saved until Review → Confirm, and a failed write stays indeterminate rather than retried.
- The key's credential reference is the backend `llm-deepseek` settings value `apiKeyEnv` (default `DEEPSEEK_API_KEY`). The app inlines no endpoint and no key.
- Rows come from the live catalog read. The page shows `Loading…`, `No DeepSeek models reported yet.` or the failed state instead of inventing a model.
- Provider options come from the live settings read. Before a read, the page shows only the load affordance; a namespace the backend does not describe renders nothing.
- Route matching is case-insensitive on the provider or the model id (`deepseek_route`), so a renamed route or an id-only row still appears.

The backend composes `@deepseek-ai/dsh-llm-deepseek-api-key` with the default catalog (`deepseek-flash` "DeepSeek-V41-Flash", `deepseek-v4-pro` "DeepSeek-V4-Pro"), `apiKeyEnv` `DEEPSEEK_API_KEY`, thinking `reasoningEffort` default `high`, and the public endpoint unless `$DEEPSEEK_BASE_URL` or `llm-deepseek.baseURL` overrides it.

## Ownership

- **Composer** owns the active model, reasoning effort and speed per session.
- **Settings → DeepSeek → Provider options** owns the `llm-deepseek` primitive fields (`baseURL`, `thinking`, `reasoningEffort`) as a focused view of the shared namespace editor.
- **Settings → Built-in plugins** still owns the whole namespace, including the `models` catalog array and the numeric limits, which are array entries or read-only here.

## Failure guidance

A failed turn is shown in the transcript. The provider's own text is never rendered for `AUTH`, `MISSING_CREDENTIAL` or `INVALID_CREDENTIAL`, because it can quote configuration or a rejected secret:

| Backend code | Native notice |
|---|---|
| `AUTH` | `Authentication failed. Check API login or Codex in Settings.` |
| `MISSING_CREDENTIAL` | `No API key stored for this model route. Add one in Settings → DeepSeek.` |
| `INVALID_CREDENTIAL` | `The stored API key was rejected. Re-enter it in Settings → DeepSeek.` |
| anything else | `Agent error (CODE): message` |

The backend's own text for the first two names the web Models page and the `DEEPSEEK_API_KEY` credential reference, so it is deliberately replaced on the native path.

## Verification

| Claim | Evidence |
|---|---|
| The credential write reaches the reference the live `llm-deepseek` namespace names, and a refused key writes nothing | `core/runtime/api-key-credential.test.mjs` (bridge suite) |
| The backend advertises `deepseek-official` models with no usable key | `packages/llm/llm-deepseek-api-key/tests/discovery.spec.ts` (2 passed, backend repo) |
| The real assembled CLI sends a DeepSeek request with its configured model and reasoning effort | `apps/cli/tests/profiles/headless/tests/headless.expected.e2e.ts` "keeps provider comments alive and sends DeepSeek defaults through the one-shot app" (17 passed, stubbed endpoint) |
| Missing and invalid credentials produce actionable guidance, not a crash | same spec: "logs actionable missing-credential guidance…" / "…invalid-credential guidance…" |
| The native transcript replaces those provider strings with a short native notice | `credential_failures_name_the_native_fix_instead_of_the_provider_message` |
| Paste, the page, the model list and the provider-options editor render and stay inert when they should | app unit suite |

## Limits

- Not implemented here: the `models` catalog array and numeric token/file limits (arrays and `token`-named fields stay with Built-in plugins), model selection from Settings, and account-route (`llm-deepseek-account`) login.
- **A live provider call is not verified.** The endpoints above use a stubbed DeepSeek-protocol server; no request has been sent to `api.deepseek.com`, and no real key has been used. Quota, provider-side rejection text and streaming against the live service remain untested here.
