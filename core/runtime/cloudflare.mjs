/** Native Cloudflare controls. OAuth credentials remain in the owning Host service. */
const REF = 'DSH_NATIVE_CLOUDFLARE_TOKEN';
const NS = 'cloudflare';
const ORIGINAL = Symbol.for('cordis.original');
const same = (a, b) => !!a && !!b && (a[ORIGINAL] ?? a) === (b[ORIGINAL] ?? b);
const record = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const exact = (value, fields) => record(value) && Object.keys(value).sort().join(',') === [...fields].sort().join(',');
export const validCloudflareRevision = value => Number.isSafeInteger(value) && value >= 0 && value < Number.MAX_SAFE_INTEGER;
export const validCloudflareAccount = value => typeof value === 'string' && /^[a-f0-9]{32}$/.test(value);
export const validCloudflareToken = value => value === null || typeof value === 'string' && value.length > 0 && value.length <= 16384 && /^[\x21-\x7e]+$/.test(value);
export const validCloudflareAttempt = value => typeof value === 'string' && /^[\x21-\x7e]{1,128}$/.test(value);
export const validCloudflareClient = value => typeof value === 'string' && (value === '' || /^[\x21-\x7e]{1,256}$/.test(value));
export const validCloudflareScopes = value => Array.isArray(value) && value.length <= 32 && new Set(value).size === value.length
  && value.every(scope => typeof scope === 'string' && /^[\x21\x23-\x5b\x5d-\x7e]{1,256}$/.test(scope)) && value.join(' ').length <= 2048;
export const validCloudflarePort = value => Number.isInteger(value) && value >= 1024 && value <= 65535;
export const validCloudflareSetup = value => exact(value, ['clientId', 'scopes', 'callbackPort']) && validCloudflareClient(value.clientId)
  && validCloudflareScopes(value.scopes) && validCloudflarePort(value.callbackPort);
const phases = ['idle', 'starting', 'waiting-browser', 'exchanging', 'committing', 'connected', 'cancelled', 'failed', 'indeterminate'];
const errors = ['SETUP_REQUIRED', 'CALLBACK_UNAVAILABLE', 'OAUTH_DENIED', 'OAUTH_FAILED', 'RECONNECT_REQUIRED', 'CONFLICT', 'INDETERMINATE'];
const active = status => ['starting', 'waiting-browser', 'exchanging', 'committing'].includes(status.phase);
/** Closed private-wire metadata; no URLs, grants or provider error text. */
export const validCloudflareAuth = value => exact(value, ['authMode', 'setupConfigured', 'credentialStored', 'credentialWritable', 'phase', 'attemptId', 'retryBlocked', 'errorCode', 'authEpoch', 'sequence'])
  && ['api-token', 'oauth'].includes(value.authMode) && phases.includes(value.phase)
  && ['setupConfigured', 'credentialStored', 'credentialWritable', 'retryBlocked'].every(field => typeof value[field] === 'boolean')
  && (value.attemptId === null || validCloudflareAttempt(value.attemptId)) && (!active(value) || value.attemptId !== null)
  && (value.errorCode === null || errors.includes(value.errorCode)) && validCloudflareRevision(value.authEpoch) && validCloudflareRevision(value.sequence);
/** Fixed configuration fields and account identifier accepted over the private pipe. */
export function validCloudflareSave(value) {
  return exact(value, ['type', 'requestId', 'expectedRevision', 'accountId', 'modelsEnabled', 'browserEnabled', 'freePlanConfirmed', 'apiToken'])
    && validCloudflareRevision(value.expectedRevision) && (value.accountId === '' || validCloudflareAccount(value.accountId))
    && ['modelsEnabled', 'browserEnabled', 'freePlanConfirmed'].every(field => typeof value[field] === 'boolean')
    && (!(value.modelsEnabled || value.browserEnabled) || value.freePlanConfirmed && validCloudflareAccount(value.accountId))
    && validCloudflareToken(value.apiToken);
}
/** OAuth controls expose registration and metadata only, never callback codes or credentials. */
export function validCloudflareOAuthCommand(value) {
  if (value.type === 'cloudflare-setup-save') return exact(value, ['type', 'requestId', 'expectedRevision', 'clientId', 'scopes', 'callbackPort'])
    && validCloudflareRevision(value.expectedRevision) && validCloudflareSetup({ clientId: value.clientId, scopes: value.scopes, callbackPort: value.callbackPort });
  if (value.type === 'cloudflare-auth-mode-save') return exact(value, ['type', 'requestId', 'expectedRevision', 'authMode'])
    && validCloudflareRevision(value.expectedRevision) && ['api-token', 'oauth'].includes(value.authMode);
  if (value.type === 'cloudflare-oauth-start') return exact(value, ['type', 'requestId', 'expectedRevision']) && validCloudflareRevision(value.expectedRevision);
  if (value.type === 'cloudflare-oauth-cancel') return exact(value, ['type', 'requestId', 'attemptId']) && validCloudflareAttempt(value.attemptId);
  if (value.type === 'cloudflare-oauth-disconnect') return exact(value, ['type', 'requestId', 'expectedRevision', 'revoke'])
    && validCloudflareRevision(value.expectedRevision) && typeof value.revoke === 'boolean';
  return false;
}
const setupOf = config => ({ clientId: config.oauthClientId, scopes: [...config.oauthScopes], callbackPort: config.oauthCallbackPort });
/** Only the configured Cloudflare PKCE authorization request may become browser authority. */
export function validCloudflareBrowserUrl(value, setup) {
  if (typeof value !== 'string' || value.length > 8192 || /[\x00-\x20\x7f]/.test(value) || !validCloudflareSetup(setup) || !setup.clientId || !setup.scopes.length) return false;
  try {
    const url = new URL(value);
    const keys = [...url.searchParams.keys()];
    return url.origin === 'https://dash.cloudflare.com' && !url.username && !url.password && !url.hash && url.pathname === '/oauth2/auth'
      && keys.length === 7 && keys.sort().join(',') === ['client_id', 'redirect_uri', 'response_type', 'scope', 'state', 'code_challenge', 'code_challenge_method'].sort().join(',')
      && url.searchParams.get('client_id') === setup.clientId
      && url.searchParams.get('redirect_uri') === `http://127.0.0.1:${setup.callbackPort}/cloudflare/callback`
      && url.searchParams.get('response_type') === 'code' && url.searchParams.get('scope') === setup.scopes.join(' ')
      && url.searchParams.get('code_challenge_method') === 'S256'
      && /^[A-Za-z0-9_-]{43}$/.test(url.searchParams.get('code_challenge'))
      && /^[A-Za-z0-9_-]{32,128}$/.test(url.searchParams.get('state'));
  } catch (_error) { return false; }
}
const failure = code => Object.assign(new Error('Cloudflare configuration unavailable'), { code });
const unavailable = () => failure('CLOUDFLARE_UNAVAILABLE');
const mapped = error => error?.code === 'SETTINGS_CONFLICT' || error?.code === 'CLOUDFLARE_CONFLICT' || error?.code === 'CONFLICT'
  ? failure('CLOUDFLARE_CONFLICT') : error?.code === 'INDETERMINATE' || error?.code === 'CLOUDFLARE_INDETERMINATE' ? failure('CLOUDFLARE_INDETERMINATE') : unavailable();
/** One mutation owner; cancellation bypasses its occupied receipt slot and joins actual OAuth work. */
export function createCloudflareBridge({ ctx, credentialRef, addSecret = () => {}, emit = () => {} }) {
  let stopped = false;
  let pending = null;
  let cancelling = null;
  let uncertain = false;
  let watched = null;
  let unwatch = null;
  let latest = null;
  let notice = null;
  const target = () => {
    const settings = ctx.get('settings');
    const credentials = ctx.get('credentials');
    const auth = ctx.get('cloudflareAuth');
    const descriptor = settings?.describe({ redactSecrets: true }).find(row => row.ns === NS);
    const config = descriptor?.value;
    if (!credentials || !auth || !descriptor || !validCloudflareRevision(descriptor.revision) || !record(config)
        || config.apiKeyEnv !== REF || !(config.accountId === '' || validCloudflareAccount(config.accountId))
        || !['modelsEnabled', 'browserEnabled', 'freePlanConfirmed'].every(field => typeof config[field] === 'boolean')
        || !['api-token', 'oauth'].includes(config.authMode) || !validCloudflareSetup(setupOf(config))
        || (config.modelsEnabled || config.browserEnabled) && (!config.freePlanConfirmed || !validCloudflareAccount(config.accountId))) throw unavailable();
    return { settings, credentials, auth, descriptor, config };
  };
  const unchanged = (a, b) => same(a.settings, b.settings) && same(a.credentials, b.credentials) && same(a.auth, b.auth)
    && a.descriptor.revision === b.descriptor.revision;
  const publish = (owner, status, browserUrl = null) => {
    if (stopped || !validCloudflareAuth(status)) return;
    let current;
    try { current = target(); } catch (_error) { return; }
    if (!same(owner, current.auth) || status.authMode !== current.config.authMode) return;
    const oauthSetup = setupOf(current.config);
    if (browserUrl !== null && (status.phase !== 'waiting-browser' || status.retryBlocked || !validCloudflareBrowserUrl(browserUrl, oauthSetup))) return;
    emit({ type: 'cloudflare-auth-state', value: { ...status }, browserUrl, oauthSetup });
  };
  const observe = before => {
    if (same(watched, before.auth)) return;
    unwatch?.();
    watched = before.auth;
    latest = null;
    notice = null;
    const owner = watched;
    unwatch = owner.subscribe(status => {
      if (stopped || !same(watched, owner) || !validCloudflareAuth(status)) return;
      if (latest && (status.authEpoch < latest.authEpoch || status.authEpoch === latest.authEpoch && status.sequence < latest.sequence)) return;
      latest = { ...status };
      if (notice && same(notice.owner, owner) && status.phase === 'waiting-browser' && !status.retryBlocked) {
        const url = notice.url;
        notice = null;
        publish(owner, status, url);
      } else {
        if (status.phase !== 'starting' && status.phase !== 'waiting-browser') notice = null;
        publish(owner, status);
      }
    });
  };
  const snapshot = async (reconcile = false) => {
    if (stopped) throw unavailable();
    const before = target();
    observe(before);
    const authStatus = await before.auth.describe();
    const current = target();
    if (stopped || !unchanged(before, current) || !validCloudflareAuth(authStatus) || authStatus.authMode !== current.config.authMode) throw unavailable();
    return { accountId: current.config.accountId, modelsEnabled: current.config.modelsEnabled,
      browserEnabled: current.config.browserEnabled, freePlanConfirmed: current.config.freePlanConfirmed,
      tokenConfigured: authStatus.credentialStored, writable: current.settings.writable === true && (reconcile || !uncertain),
      revision: current.descriptor.revision, authStatus: { ...authStatus }, oauthSetup: setupOf(current.config) };
  };
  const mutate = async command => {
    if (stopped || pending || uncertain) throw unavailable();
    const before = target();
    observe(before);
    if (before.descriptor.revision !== command.expectedRevision) throw failure('CLOUDFLARE_CONFLICT');
    let invoked = false;
    try {
      const info = await before.credentials.describe(credentialRef(REF));
      const authStatus = await before.auth.describe();
      if (stopped || !unchanged(before, target()) || before.settings.writable !== true || !validCloudflareAuth(authStatus) || active(authStatus)) throw unavailable();
      if (command.type === 'cloudflare-save' && command.apiToken !== null) {
        if (info.writable !== true) throw unavailable();
        addSecret(command.apiToken);
        invoked = true;
        await before.credentials.set(credentialRef(REF), command.apiToken);
      }
      if (stopped || !unchanged(before, target())) throw failure(invoked ? 'CLOUDFLARE_INDETERMINATE' : 'CLOUDFLARE_CONFLICT');
      const disabling = command.type === 'cloudflare-disable';
      const configuring = command.type === 'cloudflare-setup-save';
      const selectingMode = command.type === 'cloudflare-auth-mode-save';
      const values = disabling ? { modelsEnabled: false, browserEnabled: false }
        : configuring ? { oauthClientId: command.clientId, oauthScopes: command.scopes, oauthCallbackPort: command.callbackPort,
          authMode: 'oauth', modelsEnabled: false, browserEnabled: false }
        : selectingMode ? { authMode: command.authMode, modelsEnabled: false, browserEnabled: false }
        : { accountId: command.accountId, modelsEnabled: command.modelsEnabled,
          browserEnabled: command.browserEnabled, freePlanConfirmed: command.freePlanConfirmed,
          ...(command.apiToken !== null ? { authMode: 'api-token' } : {}) };
      if (!disabling && !configuring && (values.modelsEnabled || values.browserEnabled) && !authStatus.credentialStored && command.apiToken === null) throw unavailable();
      invoked = true;
      await before.settings.mutate(NS, Object.entries(values).map(([field, value]) => ({ op: 'set', path: [field], value })), command.expectedRevision);
      const current = target();
      if (stopped || !same(before.settings, current.settings) || !same(before.credentials, current.credentials) || !same(before.auth, current.auth)
          || current.descriptor.revision < before.descriptor.revision || current.descriptor.revision > before.descriptor.revision + 1
          || Object.entries(values).some(([field, value]) => JSON.stringify(current.config[field]) !== JSON.stringify(value))) throw failure('CLOUDFLARE_INDETERMINATE');
      const result = await snapshot();
      if (!configuring && !selectingMode && !disabling && (result.modelsEnabled || result.browserEnabled) && !result.tokenConfigured) throw failure('CLOUDFLARE_INDETERMINATE');
      return result;
    } catch (error) {
      // Credentials and settings have separate durable commits; an admitted write cannot be retried.
      if (invoked) { uncertain = true; throw failure('CLOUDFLARE_INDETERMINATE'); }
      throw mapped(error);
    }
  };
  const start = async command => {
    const before = target();
    observe(before);
    if (before.descriptor.revision !== command.expectedRevision) throw failure('CLOUDFLARE_CONFLICT');
    const interaction = {
      notify(value) {
        if (stopped || !value?.url || !same(watched, before.auth)) return;
        let current;
        try { current = target(); } catch (_error) { return; }
        if (!same(before.auth, current.auth) || !unchanged(before, current) || !validCloudflareBrowserUrl(value.url, setupOf(current.config))) return;
        addSecret(value.url);
        notice = { owner: before.auth, url: value.url };
        if (latest?.phase === 'waiting-browser' && !latest.retryBlocked) {
          const url = notice.url;
          notice = null;
          publish(before.auth, latest, url);
        }
      },
      async prompt() { throw unavailable(); },
    };
    try {
      const receipt = await before.auth.beginLogin({ expectedRevision: command.expectedRevision, interaction });
      if (!validCloudflareAttempt(receipt?.attemptId)) throw unavailable();
      if (stopped || !same(before.auth, target().auth)) {
        await before.auth.cancelLogin(receipt.attemptId);
        throw unavailable();
      }
      return await snapshot();
    } catch (error) { notice = null; throw mapped(error); }
  };
  return {
    async handle(command) {
      // A reserved cancellation slot is independent of the pending start/read/write slot.
      if (command.type === 'cloudflare-oauth-cancel') {
        if (stopped || cancelling) throw unavailable();
        const before = target();
        const task = (async () => {
          notice = null;
          await before.auth.cancelLogin(command.attemptId);
          if (stopped || !same(before.auth, target().auth)) throw unavailable();
          return snapshot();
        })();
        cancelling = task;
        try { return await task; } catch (error) { throw mapped(error); }
        finally { if (cancelling === task) cancelling = null; }
      }
      if (stopped || pending || cancelling || command.type !== 'cloudflare-read' && uncertain) throw unavailable();
      let task;
      if (command.type === 'cloudflare-read') task = snapshot(true);
      else if (['cloudflare-save', 'cloudflare-disable', 'cloudflare-setup-save', 'cloudflare-auth-mode-save'].includes(command.type)) task = mutate(command);
      else if (command.type === 'cloudflare-oauth-start') task = start(command);
      else if (command.type === 'cloudflare-oauth-disconnect') task = (async () => {
        const before = target();
        observe(before);
        if (before.descriptor.revision !== command.expectedRevision) throw failure('CLOUDFLARE_CONFLICT');
        try {
          const result = await before.auth.disconnect({ expectedRevision: command.expectedRevision, revoke: command.revoke });
          if (stopped || !same(before.auth, target().auth) || result?.local !== 'removed') throw failure('CLOUDFLARE_INDETERMINATE');
          if (result.issuerRevocation === 'failed') { uncertain = true; throw failure('CLOUDFLARE_INDETERMINATE'); }
          return await snapshot();
        } catch (error) { uncertain = true; throw mapped(error); }
      })();
      else throw unavailable();
      pending = task;
      try {
        const result = await task;
        if (command.type === 'cloudflare-read') uncertain = false;
        return result;
      } finally { if (pending === task) pending = null; }
    },
    async stop() {
      stopped = true;
      notice = null;
      unwatch?.(); unwatch = null;
      const tasks = [pending, cancelling].filter(Boolean);
      if (latest && active(latest) && watched) tasks.push(watched.cancelLogin(latest.attemptId));
      await Promise.allSettled(tasks);
    },
  };
}
