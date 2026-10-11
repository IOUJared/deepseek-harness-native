# Cloudflare browser sign-in

## One-time client registration

Native browser login needs your own Cloudflare OAuth client, not Wrangler's client identity. An account-private client works for members of your account and needs no domain verification. OAuth is available on Cloudflare's Free plan; Workers AI and Browser Run allocation rules remain separate.

1. Sign in to the [Cloudflare dashboard](https://dash.cloudflare.com/). Select your account, then **Manage Account → OAuth clients → Create client**. You need Super Administrator, Administrator or OAuth Client Write.
2. Name it **DeepSeek Native**. Leave visibility **private**.
3. Select response type `code`, grant types `authorization_code` and `refresh_token`, and token authentication method `none`. This desktop client uses PKCE `S256`; do not create or embed a client secret.
4. Register this exact callback URL:

   ```text
   http://127.0.0.1:8977/cloudflare/callback
   ```

   If Cloudflare rejects that loopback URL, do not substitute a production website callback; report the validation message first.
5. Select the permissions needed by the enabled products: **Workers AI Read + Edit** and/or **Browser Run Edit**, plus refresh/offline access as supported by your client registration. Do not grant account administration, billing changes or Worker deployment permissions just for this integration.
6. Keep the **Client ID** and the registered **OAuth scope IDs**. These are public configuration, not secrets. Scope IDs are Cloudflare's dot-delimited labels, not Wrangler's legacy colon-delimited scopes. The authenticated [scope-list API](https://developers.cloudflare.com/api/resources/iam/subresources/oauth_scopes/methods/list/) maps permission names to these IDs; do not guess them.

Never paste an API token, authorization code, refresh token or client secret into chat. The Client ID and scope IDs can be shared when configuring the desktop client.

## Native flow

1. Restart the matching rebuilt Native app and backend. Open **Settings → Cloudflare → Read configuration**.
2. Enter the account ID and save it with both product features off. Expand **OAuth client setup**, enter the Client ID, exact registered scope IDs separated by spaces and callback port `8977`, then **Save browser setup**. Saving setup selects browser authentication and disables both features; it does not erase a saved API token.
3. Click **Connect Cloudflare**, then sign in, complete MFA and approve the scopes in your normal browser. The app accepts only the matching local PKCE callback. Opening the browser is not authorization success.
4. After authorization, separately select models/browser tools, confirm Workers Free and review/confirm persistence. Model selection stays explicit.

Access/refresh credentials remain in the owning backend; the native interface receives status only. **Cancel** withdraws browser-opening authority and joins admitted work; it cannot undo an already issued or committed credential. Closing Settings stops further automatic opener steps, not an admitted backend write. Unknown outcomes require an explicit read, never automatic retry.

**Use saved API token** is an explicit mode change, not an OAuth-error fallback. Mode changes disable both features until reviewed again. **Disconnect** disables features and removes the local OAuth grant; issuer revocation is attempted separately and may fail. It does not erase a separately saved API token or log out your browser's Cloudflare session.

Browser sign-in does not verify the account plan, grant Workers AI/Browser Run permissions automatically, enable features or select a model. Keep the intended account on **Workers Free** and explicitly confirm it before enabling features. OAuth failures never automatically select a saved API token.

## Sources and limits

[Client registration and desktop PKCE](https://developers.cloudflare.com/fundamentals/oauth/create-an-oauth-client/) · [OAuth availability](https://developers.cloudflare.com/fundamentals/oauth/) · [Official endpoints](https://developers.cloudflare.com/fundamentals/oauth/integrate-with-cloudflare/). Checked October 10, 2026.

Build, native rendering, lifecycle tests and synthetic OAuth on the real supported profile passed; these do not establish live authentication. Your client has not been registered or authorized by this implementation yet. Exact live loopback acceptance, granted product scopes, refresh and revocation still need verification with that client. Private OAuth client visibility is distinct from a public/native protocol client using token authentication `none`; promoting visibility to public is permanent and requires verified domain ownership.
