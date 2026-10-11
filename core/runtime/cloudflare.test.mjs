import test from 'node:test';
import assert from 'node:assert/strict';
import { createCloudflareBridge, validCloudflareAuth, validCloudflareBrowserUrl } from './cloudflare.mjs';
import { parseForkCommand } from './fork-supervisor.mjs';
const ACCOUNT = '0123456789abcdef0123456789abcdef';
const TOKEN = 'PUBLIC_SYNTHETIC_CLOUDFLARE_TOKEN';
const save = (extra = {}) => ({ type: 'cloudflare-save', requestId: 'PUBLIC', expectedRevision: 0,
  accountId: ACCOUNT, modelsEnabled: true, browserEnabled: true, freePlanConfirmed: true, apiToken: TOKEN, ...extra });
function fixture() {
  let revision = 0;
  let configured = false;
  let config = { accountId: '', modelsEnabled: false, browserEnabled: false, freePlanConfirmed: false, apiKeyEnv: 'DSH_NATIVE_CLOUDFLARE_TOKEN',
    authMode: 'api-token', oauthClientId: '', oauthScopes: [], oauthCallbackPort: 8977 };
  const calls = [];
  const secrets = [];
  const credentials = {
    async describe(ref) { calls.push(['describe', ref]); return { configured, writable: true }; },
    async set(ref, token) { calls.push(['set', ref]); assert.equal(token, TOKEN); configured = true; },
  };
  const settings = {
    writable: true,
    describe(options) { assert.deepEqual(options, { redactSecrets: true }); return [{ ns: 'cloudflare', revision, value: { ...config } }]; },
    async mutate(ns, ops, expected) {
      calls.push(['mutate', ns]); assert.equal(ns, 'cloudflare');
      if (expected !== revision) throw Object.assign(new Error('PUBLIC'), { code: 'SETTINGS_CONFLICT' });
      const next = { ...config };
      for (const op of ops) { assert.equal(op.op, 'set'); assert.equal(op.path.length, 1); next[op.path[0]] = op.value; }
      if (JSON.stringify(next) !== JSON.stringify(config)) revision++;
      config = next;
    },
  };
  const frames = [];
  let listener;
  let status = { authMode: 'api-token', setupConfigured: false, credentialStored: false, credentialWritable: true,
    phase: 'idle', attemptId: null, retryBlocked: false, errorCode: null, authEpoch: 0, sequence: 0 };
  const auth = {
    subscribe(fn) { listener = fn; return () => { listener = null; }; },
    async describe() {
      const info = await credentials.describe('DSH_NATIVE_CLOUDFLARE_TOKEN');
      return { ...status, authMode: config.authMode, setupConfigured: !!config.oauthClientId && !!config.oauthScopes.length,
        credentialStored: config.authMode === 'api-token' ? info.configured : status.credentialStored, credentialWritable: info.writable };
    },
    async beginLogin() { throw new Error('PUBLIC no fixture login'); },
    async cancelLogin() { status = { ...status, phase: 'cancelled', sequence: status.sequence + 1 }; listener?.(status); return status; },
    async disconnect() { throw new Error('PUBLIC no fixture disconnect'); },
  };
  const services = { settings, credentials, cloudflareAuth: auth };
  const bridge = createCloudflareBridge({ ctx: { get: name => services[name] }, credentialRef: value => value,
    addSecret: token => secrets.push(token), emit: frame => frames.push(frame) });
  return { bridge, settings, credentials, auth, calls, secrets, services, frames, change() { revision++; }, config: () => config,
    setConfig(values) { config = { ...config, ...values }; revision++; },
    status(values) { status = { ...status, ...values }; listener?.(status); return status; } };
}
test('private parser accepts only exact fixed Cloudflare operations', () => {
  for (const command of [save(), save({ apiToken: null }), save({ accountId: '', modelsEnabled: false, browserEnabled: false, freePlanConfirmed: false }), { type: 'cloudflare-read', requestId: 'PUBLIC' }, { type: 'cloudflare-disable', requestId: 'PUBLIC', expectedRevision: 0 }]) {
    assert.deepEqual(parseForkCommand(JSON.stringify(command)), command);
  }
  for (const command of [save({ extra: 1 }), save({ accountId: 'PUBLIC' }), save({ accountId: '' }), save({ apiToken: '' }), save({ apiToken: '\nPUBLIC' }), save({ apiToken: 'x'.repeat(16385) }), save({ modelsEnabled: 1 }), save({ freePlanConfirmed: false }), save({ expectedRevision: Number.MAX_SAFE_INTEGER }), save({ expectedRevision: -1 }), save({ requestId: ' ' }), { type: 'cloudflare-read', requestId: 'PUBLIC', apiToken: TOKEN }, { type: 'cloudflare-disable', requestId: 'PUBLIC', expectedRevision: 0, modelsEnabled: true }]) assert.throws(() => parseForkCommand(JSON.stringify(command)));
});
test('read returns local metadata only; save persists fixed credential and CAS flags', async () => {
  const f = fixture();
  const initial = await f.bridge.handle({ type: 'cloudflare-read' });
  assert.deepEqual(initial, { accountId: '', modelsEnabled: false, browserEnabled: false, freePlanConfirmed: false, tokenConfigured: false, writable: true, revision: 0,
    authStatus: { authMode: 'api-token', setupConfigured: false, credentialStored: false, credentialWritable: true, phase: 'idle', attemptId: null,
      retryBlocked: false, errorCode: null, authEpoch: 0, sequence: 0 }, oauthSetup: { clientId: '', scopes: [], callbackPort: 8977 } });
  assert.ok(f.calls.every(([kind]) => kind === 'describe'));
  const saved = await f.bridge.handle(save());
  assert.equal(saved.revision, 1); assert.equal(saved.tokenConfigured, true);
  assert.equal(saved.accountId, ACCOUNT); assert.ok(!JSON.stringify(saved).includes(TOKEN));
  assert.deepEqual(f.secrets, [TOKEN]);
  assert.deepEqual(f.calls.filter(([kind]) => kind !== 'describe'), [['set', 'DSH_NATIVE_CLOUDFLARE_TOKEN'], ['mutate', 'cloudflare']]);
  const unchanged = await f.bridge.handle(save({ expectedRevision: 1, apiToken: null }));
  assert.equal(unchanged.revision, 1);
  const disabled = await f.bridge.handle({ type: 'cloudflare-disable', expectedRevision: 1 });
  assert.equal(disabled.revision, 2); assert.equal(disabled.modelsEnabled, false); assert.equal(disabled.browserEnabled, false); assert.equal(disabled.tokenConfigured, true);
});
test('conflict is refused before token persistence', async () => {
  const f = fixture(); f.change();
  await assert.rejects(f.bridge.handle(save()), { code: 'CLOUDFLARE_CONFLICT' });
  assert.equal(f.calls.length, 0);
});
test('enabling without a saved or new token is refused', async () => {
  const f = fixture();
  await assert.rejects(f.bridge.handle(save({ apiToken: null })), { code: 'CLOUDFLARE_UNAVAILABLE' });
  assert.ok(f.calls.every(([kind]) => kind === 'describe'));
});
test('credential may persist before config failure; no automatic retry; explicit read reconciles flags only', async () => {
  const f = fixture();
  const mutate = f.settings.mutate;
  f.settings.mutate = async () => { throw new Error('PUBLIC failure'); };
  await assert.rejects(f.bridge.handle(save()), { code: 'CLOUDFLARE_INDETERMINATE' });
  await assert.rejects(f.bridge.handle(save()), { code: 'CLOUDFLARE_UNAVAILABLE' });
  assert.equal(f.calls.filter(([kind]) => kind === 'set').length, 1);
  const reconciled = await f.bridge.handle({ type: 'cloudflare-read' });
  assert.equal(reconciled.modelsEnabled, false); assert.equal(reconciled.tokenConfigured, true); assert.equal(reconciled.writable, true);
  f.settings.mutate = mutate;
  await f.bridge.handle(save({ apiToken: null }));
  assert.equal(f.calls.filter(([kind]) => kind === 'set').length, 1);
});
test('service replacement during credential admission cannot mutate the new owner', async () => {
  const f = fixture();
  f.credentials.describe = async () => { f.services.settings = { ...f.settings }; return { configured: true, writable: true }; };
  await assert.rejects(f.bridge.handle(save()), { code: 'CLOUDFLARE_UNAVAILABLE' });
  assert.equal(f.calls.length, 0);
});
test('read and mutation admission cannot overlap', async () => {
  const f = fixture(); let finish;
  f.credentials.describe = () => new Promise(resolve => { finish = resolve; });
  const read = f.bridge.handle({ type: 'cloudflare-read' });
  await assert.rejects(f.bridge.handle(save()), { code: 'CLOUDFLARE_UNAVAILABLE' });
  finish({ configured: false, writable: true }); await read;
});
test('stop revokes pending credential write before flags and awaits settlement', async () => {
  const f = fixture(); let finish;
  let admitted;
  const admission = new Promise(resolve => { admitted = resolve; });
  f.credentials.set = () => { admitted(); return new Promise(resolve => { finish = resolve; }); };
  const write = f.bridge.handle(save());
  await admission;
  let stopped = false;
  const stop = f.bridge.stop().then(() => { stopped = true; });
  await Promise.resolve(); assert.equal(stopped, false);
  finish();
  await assert.rejects(write, { code: 'CLOUDFLARE_INDETERMINATE' }); await stop;
  assert.equal(f.config().modelsEnabled, false);
  await assert.rejects(f.bridge.handle({ type: 'cloudflare-read' }), { code: 'CLOUDFLARE_UNAVAILABLE' });
});
test('read-only credential storage cannot block disabling or unchanged-token configuration', async () => {
  const f = fixture();
  await f.bridge.handle(save());
  f.credentials.describe = async () => ({ configured: true, writable: false });
  const meta = await f.bridge.handle({ type: 'cloudflare-read' });
  assert.equal(meta.writable, true);
  await assert.rejects(f.bridge.handle(save({ expectedRevision: meta.revision })), { code: 'CLOUDFLARE_UNAVAILABLE' });
  const disabled = await f.bridge.handle({ type: 'cloudflare-disable', expectedRevision: meta.revision });
  assert.equal(disabled.modelsEnabled, false); assert.equal(disabled.browserEnabled, false);
  const enabled = await f.bridge.handle(save({ expectedRevision: disabled.revision, apiToken: null }));
  assert.equal(enabled.modelsEnabled, true);
});
test('invalid mounted config refuses local metadata', async () => {
  const f = fixture();
  f.settings.describe = () => [{ ns: 'cloudflare', revision: 0, value: { apiKeyEnv: 'PUBLIC_OTHER_REF' } }];
  await assert.rejects(f.bridge.handle({ type: 'cloudflare-read' }), { code: 'CLOUDFLARE_UNAVAILABLE' });
  assert.equal(f.calls.length, 0);
});

const SETUP = { clientId: 'PUBLIC_OWN_DESKTOP_CLIENT', scopes: ['PUBLIC.product.read', 'offline_access'], callbackPort: 8977 };
function authorizationUrl(setup = SETUP) {
  const url = new URL('https://dash.cloudflare.com/oauth2/auth');
  for (const [key, value] of Object.entries({ client_id: setup.clientId, redirect_uri: `http://127.0.0.1:${setup.callbackPort}/cloudflare/callback`,
    response_type: 'code', scope: setup.scopes.join(' '), state: 'P'.repeat(43), code_challenge: 'C'.repeat(43), code_challenge_method: 'S256' })) url.searchParams.set(key, value);
  return url.href;
}
function configuredFixture() {
  const f = fixture();
  f.setConfig({ accountId: ACCOUNT, oauthClientId: SETUP.clientId, oauthScopes: SETUP.scopes, oauthCallbackPort: SETUP.callbackPort });
  return f;
}
test('OAuth parser closes setup and control fields without accepting secret values or redirect overrides', () => {
  const setup = { type: 'cloudflare-setup-save', requestId: 'PUBLIC', expectedRevision: 0, ...SETUP };
  const controls = [setup, { type: 'cloudflare-oauth-start', requestId: 'PUBLIC', expectedRevision: 0 },
    { type: 'cloudflare-oauth-cancel', requestId: 'PUBLIC', attemptId: 'PUBLIC_ATTEMPT' },
    { type: 'cloudflare-oauth-disconnect', requestId: 'PUBLIC', expectedRevision: 0, revoke: true },
    { type: 'cloudflare-auth-mode-save', requestId: 'PUBLIC', expectedRevision: 0, authMode: 'api-token' }];
  for (const value of controls) assert.deepEqual(parseForkCommand(JSON.stringify(value)), value);
  for (const value of [...controls.map(value => ({ ...value, clientSecret: TOKEN })), { ...setup, scopes: ['x', 'x'] },
    { ...setup, scopes: ['x y'] }, { ...setup, scopes: ['x\\y'] }, { ...setup, scopes: Array.from({ length: 33 }, (_, i) => `PUBLIC${i}`) },
    { ...setup, callbackPort: 1023 }, { ...setup, redirectUri: 'https://PUBLIC.invalid' },
    { ...controls[2], attemptId: '' }, { ...controls[3], revoke: 'true' }]) assert.throws(() => parseForkCommand(JSON.stringify(value)));
});
test('Cloudflare PKCE browser authority is tied to registered client, scopes and exact numeric loopback', () => {
  assert.equal(validCloudflareBrowserUrl(authorizationUrl(), SETUP), true);
  for (const [key, value] of Object.entries({ client_id: 'PUBLIC_OTHER', redirect_uri: 'http://localhost:8977/cloudflare/callback',
    response_type: 'token', scope: 'PUBLIC.admin.write', state: 'short', code_challenge_method: 'plain', code_challenge: 'bad' })) {
    const url = new URL(authorizationUrl()); url.searchParams.set(key, value);
    assert.equal(validCloudflareBrowserUrl(url.href, SETUP), false);
  }
  const duplicate = new URL(authorizationUrl()); duplicate.searchParams.append('state', 'P'.repeat(43));
  assert.equal(validCloudflareBrowserUrl(duplicate.href, SETUP), false);
  assert.equal(validCloudflareBrowserUrl(authorizationUrl().replace('dash.cloudflare.com', 'PUBLIC.invalid'), SETUP), false);
  assert.equal(validCloudflareBrowserUrl(`${authorizationUrl()}#PUBLIC`, SETUP), false);
});
test('OAuth setup saves only registration and neither enables features nor confirms Workers Free', async () => {
  const f = fixture();
  const result = await f.bridge.handle({ type: 'cloudflare-setup-save', expectedRevision: 0, ...SETUP });
  assert.deepEqual(result.oauthSetup, SETUP);
  assert.equal(result.authStatus.setupConfigured, true);
  assert.equal(result.modelsEnabled, false); assert.equal(result.browserEnabled, false); assert.equal(result.freePlanConfirmed, false);
  assert.equal(result.tokenConfigured, false);
  assert.equal(f.calls.some(([kind]) => kind === 'set'), false);
});
test('private browser notice is published once with setup authority and no credential in public snapshot', async () => {
  const f = configuredFixture();
  f.auth.beginLogin = async ({ expectedRevision, interaction }) => {
    assert.equal(expectedRevision, 1);
    f.status({ phase: 'waiting-browser', attemptId: 'PUBLIC_ATTEMPT', setupConfigured: true, sequence: 1 });
    interaction.notify({ message: 'PUBLIC', url: authorizationUrl() });
    return { attemptId: 'PUBLIC_ATTEMPT' };
  };
  const result = await f.bridge.handle({ type: 'cloudflare-oauth-start', expectedRevision: 1 });
  assert.equal(result.authStatus.phase, 'waiting-browser');
  assert.equal(JSON.stringify(result).includes('code_challenge'), false);
  const frames = f.frames.filter(frame => frame.browserUrl !== null);
  assert.equal(frames.length, 1); assert.deepEqual(frames[0].oauthSetup, SETUP);
  assert.deepEqual(f.secrets, [authorizationUrl()]);
  assert.equal(f.config().modelsEnabled, false); assert.equal(f.config().freePlanConfirmed, false);
  await f.bridge.stop();
});
test('reserved cancel settles a pending start receipt without waiting behind its slot', async () => {
  const f = configuredFixture();
  let entered, release;
  const admission = new Promise(resolve => { entered = resolve; });
  const bound = new Promise(resolve => { release = resolve; });
  f.auth.beginLogin = async () => {
    f.status({ phase: 'starting', attemptId: 'PUBLIC_ATTEMPT', sequence: 1 }); entered();
    await bound;
    return { attemptId: 'PUBLIC_ATTEMPT' };
  };
  f.auth.cancelLogin = async attempt => {
    assert.equal(attempt, 'PUBLIC_ATTEMPT');
    const status = f.status({ phase: 'cancelled', sequence: 2 }); release(); return status;
  };
  const start = f.bridge.handle({ type: 'cloudflare-oauth-start', expectedRevision: 1 });
  await admission;
  const cancelled = await f.bridge.handle({ type: 'cloudflare-oauth-cancel', attemptId: 'PUBLIC_ATTEMPT' });
  assert.equal(cancelled.authStatus.phase, 'cancelled');
  assert.equal((await start).authStatus.phase, 'cancelled');
  assert.equal(f.frames.some(frame => frame.browserUrl !== null), false);
});
test('settings writes cannot overlap live OAuth; API token selection is explicit and blank preserves OAuth', async () => {
  const f = configuredFixture();
  await f.bridge.handle({ type: 'cloudflare-read' });
  f.status({ phase: 'waiting-browser', attemptId: 'PUBLIC_ATTEMPT', sequence: 1 });
  await assert.rejects(f.bridge.handle(save({ expectedRevision: 1 })), { code: 'CLOUDFLARE_UNAVAILABLE' });
  assert.equal(f.calls.some(([kind]) => kind === 'set'), false);
  f.status({ phase: 'connected', credentialStored: true, sequence: 2 });
  f.setConfig({ authMode: 'oauth' });
  const keep = await f.bridge.handle(save({ expectedRevision: 2, apiToken: null }));
  assert.equal(keep.authStatus.authMode, 'oauth');
  const manual = await f.bridge.handle(save({ expectedRevision: keep.revision }));
  assert.equal(manual.authStatus.authMode, 'api-token');
});
test('explicit authentication mode switches preserve credentials and require feature review', async () => {
  const f = fixture();
  await f.bridge.handle(save());
  const browser = await f.bridge.handle({ type: 'cloudflare-auth-mode-save', expectedRevision: 1, authMode: 'oauth' });
  assert.equal(browser.authStatus.authMode, 'oauth');
  assert.equal(browser.modelsEnabled, false); assert.equal(browser.browserEnabled, false); assert.equal(browser.freePlanConfirmed, true);
  assert.equal(browser.tokenConfigured, false);
  const manual = await f.bridge.handle({ type: 'cloudflare-auth-mode-save', expectedRevision: browser.revision, authMode: 'api-token' });
  assert.equal(manual.tokenConfigured, true); assert.equal(manual.modelsEnabled, false); assert.equal(manual.browserEnabled, false);
  assert.equal(f.calls.filter(([kind]) => kind === 'set').length, 1);
});
test('issuer revocation failure is not reported as a successful complete sign-out', async () => {
  const f = fixture();
  f.auth.disconnect = async () => ({ local: 'removed', issuerRevocation: 'failed' });
  await assert.rejects(f.bridge.handle({ type: 'cloudflare-oauth-disconnect', expectedRevision: 0, revoke: true }), { code: 'CLOUDFLARE_INDETERMINATE' });
  await assert.rejects(f.bridge.handle(save()), { code: 'CLOUDFLARE_UNAVAILABLE' });
  const read = await f.bridge.handle({ type: 'cloudflare-read' });
  assert.equal(read.writable, true);
});
test('auth metadata rejects extra secrets and invalid active attempts', () => {
  const value = { authMode: 'oauth', setupConfigured: true, credentialStored: false, credentialWritable: true,
    phase: 'waiting-browser', attemptId: 'PUBLIC', retryBlocked: false, errorCode: null, authEpoch: 1, sequence: 2 };
  assert.equal(validCloudflareAuth(value), true);
  assert.equal(validCloudflareAuth({ ...value, accessToken: TOKEN }), false);
  assert.equal(validCloudflareAuth({ ...value, attemptId: null }), false);
  assert.equal(validCloudflareAuth({ ...value, phase: 'unknown' }), false);
  assert.equal(validCloudflareAuth({ ...value, sequence: -1 }), false);
});
