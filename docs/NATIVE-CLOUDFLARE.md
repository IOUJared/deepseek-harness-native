# Cloudflare models and browser tools

Optional Workers AI and Browser Run integration in Settings → Cloudflare. The native Iced interface and existing session model remain unchanged. No Worker deployment, Agents SDK remote execution, AI Gateway, paid fallback or automatic model selection is included.

## Setup

1. Use a Cloudflare **Workers Free** account. Check the account's plan in the dashboard before enabling either feature.
2. Copy the intended account's 32-character lowercase hexadecimal ID. Choose [browser OAuth](<NATIVE-CLOUDFLARE-OAUTH.md>) or an account-scoped API token (Workers AI Read + Edit for models; Browser Run Edit for browser tools).
3. Open Settings → Cloudflare and explicitly read local configuration. Save the account ID with both features off, then configure/connect browser authentication or enter the API token. A blank token keeps the selected credential mode; entering a new token explicitly selects API-token mode.
4. Select models and/or browser tools, confirm Workers Free, review and confirm persistence. Changing the account ID clears the free-plan confirmation; confirm it again for the new account.
5. Close Settings, open the model selector and select a Cloudflare model for the conversation. Opening the selector refreshes the catalog; saving configuration does not send a prompt, validate credentials or switch a session's model.

| Capability | Purpose | Workers Free allocation |
|---|---|---|
| Workers AI | Curated text-only model route | 10,000 neurons/day, account-wide |
| Browser Run Markdown | Render a public webpage and extract text | Shares 10 browser minutes/day |
| Browser Run screenshot | Capture a public webpage as PNG | Shares the same browser allocation |

Workers AI limits reset at 00:00 UTC; Cloudflare enforces the Browser Run daily allocation. The free-plan confirmation is a user assertion, **not verified billing information**. A Workers Paid account can incur charges, including overages on otherwise free-compatible models. This client cannot enforce account-wide spending or detect usage from other applications. Keep the account on Workers Free; Cloudflare enforces its free-plan limits. No local quota estimate is presented as a billing guarantee.

## Privacy and persistence

- Only the fixed Cloudflare API origin receives model/browser bearer credentials. Browser sign-in additionally uses fixed Cloudflare dashboard OAuth endpoints; no custom remote endpoints are exposed.
- API tokens use `DSH_NATIVE_CLOUDFLARE_TOKEN`; OAuth uses the Host-owned `cloudflare/oauth` grant record with serialized refresh. Configuration stores public registration fields and credential mode, never access/refresh tokens. Default storage uses private directories/files, not encryption; trusted custom providers may store elsewhere.
- Local metadata exposes account ID, enable flags, selected credential presence, public registration fields, authorization phase and revisions—not token values, quota, account plan or live credential validity.
- A token save and configuration edit are separate writes. A timeout/error can occur after either commit; no write is automatically retried. Explicit Read configuration observes current flags and presence, but cannot prove which token was saved.
- Disable both removes these model/tool registrations; it does not revoke the API token, erase credentials, undo admitted requests or change other providers. A session using Cloudflare then needs another explicitly selected model; no replacement is chosen automatically.
- Model prompts and enabled browser requests are sent to Cloudflare. Extracted web content is untrusted data. Browser tools do not carry local browser cookies or accept HTTP authentication options. Requested URLs, including any query parameters, are sent to Cloudflare and logged in tool history.
- Screenshots are stored as durable file attachments, not vision input. The curated models are text-only. Native chat currently shows file metadata with preview unavailable; this integration does not add inline screenshot viewing or a generated-file download control. Referenced files are included by existing [session ZIP export](<NATIVE-SESSION-EXPORT.md>), subject to its size limits; exports also contain potentially sensitive conversation data.

## Verification limits

Production checks cover the private parser, fixed credential reference, settings CAS, concurrent admission, stale owners and shutdown. Rust checks cover secret framing, metadata decoding and native reducer/worker controls; backend checks own model/tool registration and bounded network results. Mocked network behavior does not establish real token permissions, live model/tool compatibility, Cloudflare billing status, physical input or deployed binary availability.

## Sources

[Workers AI pricing](https://developers.cloudflare.com/workers-ai/platform/pricing/) · [REST token setup](https://developers.cloudflare.com/workers-ai/get-started/rest-api/) · [OpenAI-compatible endpoint](https://developers.cloudflare.com/workers-ai/configuration/open-ai-compatibility/) · [Browser Run pricing](https://developers.cloudflare.com/browser-run/pricing/) · [Markdown API](https://developers.cloudflare.com/browser-run/quick-actions/markdown-endpoint/). Checked October 10, 2026; pricing and permissions may change.
