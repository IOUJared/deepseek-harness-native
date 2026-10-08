#!/usr/bin/env node
/** Private Linux desktop owner. Only framed inherited pipes carry native commands and credentials. */
import { createRequire } from 'node:module';
import { createCodexBridge } from './codex.mjs';
import { chmod, lstat, mkdir, mkdtemp, readFile, realpath, rm, writeFile } from 'node:fs/promises';
import { dirname, isAbsolute, join, relative, resolve, sep } from 'node:path';
import { homedir, tmpdir } from 'node:os';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { MAX_COMMAND_BYTES, parseCommand as parseBaselineCommand, lineReceiver, interceptDiagnostics,
  discoverRuntime, readyUrl, inspectTasks, addWorkspace } from './supervisor.mjs';

export const DESKTOP_BUNDLES = Object.freeze(['@deepseek-ai/dsh-base', '@deepseek-ai/dsh-web-app']);
/** Owner overlay remains the last non-telemetry layer even after profile reloads. */
export const DESKTOP_PATCH = `- id: desktop-product-telemetry\n  disabled: true\n- id: product-analytics\n  disabled: true\n- id: ui-sidebar-browser\n  disabled: true\n`;
export const OPTIONAL_PAYLOADS = Object.freeze({
  office: Object.freeze({ status: 'unavailable', reason: 'not-configured' }),
  browser: Object.freeze({ status: 'disabled', reason: 'linux-carrier-required' }),
});
const record = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const requestIdValid = value => typeof value === 'string' && /^[A-Za-z0-9_-]{1,128}$/.test(value);
const opaqueIdValid = value => typeof value === 'string' && value.length > 0 && value.length <= 512 && !/[\x00-\x20\x7f]/.test(value);
const discoveryIdValid = value => typeof value === 'string' && Buffer.byteLength(value) > 0 && Buffer.byteLength(value) <= 512 && !/[\p{White_Space}\p{Cc}\p{Cs}]/u.test(value);
const exact = (value, fields) => Object.keys(value).sort().join(',') === [...fields].sort().join(',');
const metadataValid = value => typeof value.locale === 'string' && /^[A-Za-z]{2,8}(?:[-_][A-Za-z0-9]{1,8})*$/.test(value.locale)
  && value.locale.length <= 64 && Number.isInteger(value.timezoneOffsetSeconds) && Math.abs(value.timezoneOffsetSeconds) <= 86400;

/** Reject arbitrary service calls, extra keys, client version spoofing, and invalid secret syntax. */
export function parseForkCommand(line) {
  if (Buffer.byteLength(line) > MAX_COMMAND_BYTES) throw new Error('Invalid native command');
  const value = JSON.parse(line);
  if (!record(value)) throw new Error('Invalid native command');
  if (['shutdown', 'inspect', 'open-workspace'].includes(value.type)) return parseBaselineCommand(line);
  if (!requestIdValid(value.requestId)) throw new Error('Invalid native command');
  const serial = number => Number.isSafeInteger(number) && number > 0;
  if (value.type === 'codex-cancel' && exact(value, ['type', 'requestId', 'attempt']) && serial(value.attempt)) return value;
  if (value.type === 'codex-enable-models' && exact(value, ['type', 'requestId', 'expectedRevision']) && Number.isSafeInteger(value.expectedRevision) && value.expectedRevision >= 0) return value;
  if (value.type === 'codex-callback' && exact(value, ['type', 'requestId', 'attempt', 'prompt', 'response'])
      && serial(value.attempt) && serial(value.prompt) && typeof value.response === 'string' && value.response.length <= 16384 && /^[\x21-\x7e]+$/.test(value.response)) return value;
  if (['codex-status', 'codex-start', 'onboarding-read', 'account-state', 'account-subscribe', 'account-unsubscribe'].includes(value.type)
      && exact(value, ['type', 'requestId'])) return value;
  if (['account-start', 'account-signout'].includes(value.type)
      && exact(value, ['type', 'requestId', 'locale', 'timezoneOffsetSeconds']) && metadataValid(value)) return value;
  if (value.type === 'account-cancel' && exact(value, ['type', 'requestId', 'attemptId']) && opaqueIdValid(value.attemptId)) return value;
  if (value.type === 'cancel-generation' && exact(value, ['type', 'requestId', 'sessionId']) && opaqueIdValid(value.sessionId)) return value;
  if (value.type === 'discover-session-agent' && exact(value, ['type', 'requestId', 'sessionId']) && discoveryIdValid(value.sessionId)) return value;
  if (value.type === 'save-api-key' && exact(value, ['type', 'requestId', 'apiKey'])
      && typeof value.apiKey === 'string' && value.apiKey.length <= 16384 && /^[\x21-\x7e]+$/.test(value.apiKey)) return value;
  throw new Error('Invalid native command');
}

async function canonicalFuture(path) {
  try { return await realpath(path); } catch (error) {
    if (error.code !== 'ENOENT') throw error;
    const parent = dirname(path);
    if (parent === path) throw error;
    return join(await canonicalFuture(parent), path.slice(parent.length + (parent === sep ? 0 : 1)));
  }
}
const within = (path, root) => path === root || (!relative(root, path).startsWith(`..${sep}`) && relative(root, path) !== '..' && !isAbsolute(relative(root, path)));

/** Ignore inherited DSH_HOME and refuse aliases of it, ~/.dsh, or a containing directory. */
export async function resolveIsolatedHome(env = process.env) {
  const userHome = resolve(env.HOME || homedir());
  const dataHome = env.XDG_DATA_HOME || join(userHome, '.local', 'share');
  const requested = env.DSH_TAURI_HOME ?? join(dataHome, 'deepseek-harness-tauri', 'dsh');
  if (!isAbsolute(dataHome) || !isAbsolute(requested) || /[\x00-\x1f\x7f]/.test(requested)) throw new Error('Invalid isolated Harness home');
  const home = await canonicalFuture(resolve(requested));
  const forbidden = [join(userHome, '.dsh'), ...(env.DSH_HOME ? [resolve(env.DSH_HOME)] : [])];
  for (const path of forbidden) {
    const canonical = await canonicalFuture(path);
    if (within(home, canonical) || within(canonical, home)) throw new Error('Harness home is not isolated');
  }
  if (home === sep || home === userHome || home === await canonicalFuture(resolve(dataHome))) throw new Error('Invalid isolated Harness home');
  return home;
}

async function versionedExport(require, name, version) {
  const manifest = JSON.parse(await readFile(require.resolve(`${name}/package.json`), 'utf8'));
  if (manifest.version !== version) throw new Error('Mismatched fork runtime versions');
  return require.resolve(name);
}

/** Initialize with upstream helpers; required bundled anchors may not be silently skipped. */
export async function initializeDesktopProfile(appBoot, home, installAnchor, version) {
  const dir = appBoot.resolveProfileDir('desktop', home);
  if (await canonicalFuture(dir) !== dir) throw new Error('Desktop profile is not isolated');
  await mkdir(dir, { recursive: true, mode: 0o700 });
  if (await realpath(dir) !== dir) throw new Error('Desktop profile changed during initialization');
  for (const name of ['package.json', 'cordis.yml', 'cordis.patch.yml', 'pnpm-workspace.yaml']) {
    let info;
    try { info = await lstat(join(dir, name)); } catch (error) { if (error.code !== 'ENOENT') throw error; }
    if (info && !info.isFile()) throw new Error('Invalid desktop profile file');
  }
  appBoot.initProfile(dir, DESKTOP_BUNDLES);
  const profile = appBoot.loadProfileDirectory('dsh-tauri', dir, installAnchor);
  if (profile.name !== 'desktop' || profile.skippedBundles.length !== 0
      || profile.layers.length !== DESKTOP_BUNDLES.length
      || profile.layers.some((layer, index) => layer.packageName !== DESKTOP_BUNDLES[index])) throw new Error('Invalid desktop profile bundles');
  for (const layer of profile.layers) {
    const manifest = JSON.parse(await readFile(join(layer.packageDir, 'package.json'), 'utf8'));
    if (manifest.version !== version) throw new Error('Mismatched fork bundle versions');
  }
  return { profile, installAnchor };
}

/** Require an explicit built fork package; never discover or reuse an installed CLI. */
export async function loadForkRuntime(env = process.env) {
  if (process.platform !== 'linux') throw new Error('Linux fork runtime required');
  if (!env.DSH_TAURI_RUNTIME || !isAbsolute(env.DSH_TAURI_RUNTIME)) throw new Error('Explicit fork runtime required');
  const installation = await discoverRuntime({ DSH_TAURI_RUNTIME: env.DSH_TAURI_RUNTIME });
  if (env.DSH_TAURI_EXPECTED_VERSION && installation.version !== env.DSH_TAURI_EXPECTED_VERSION) throw new Error('Unexpected fork runtime version');
  const home = await resolveIsolatedHome(env);
  await mkdir(home, { recursive: true, mode: 0o700 });
  if (await realpath(home) !== home) throw new Error('Harness home changed during initialization');
  await chmod(home, 0o700);
  // Bootstrap variables are committed before importing a single upstream module.
  process.env.DSH_HOME = home;
  process.env.DSH_TELEMETRY_DISABLED = '1';
  process.env.DSH_CLIENT_VERSION = installation.version;
  const appBootPath = await versionedExport(installation.require, '@deepseek-ai/dsh-app-boot', installation.version);
  const appRequire = createRequire(appBootPath);
  const baseDir = installation.require.resolve('@deepseek-ai/dsh-base/package.json');
  const baseRequire = createRequire(baseDir);
  const credentialsRequire = createRequire(baseRequire.resolve('@deepseek-ai/dsh-credentials-local/package.json'));
  const credentialsPath = await versionedExport(credentialsRequire, '@deepseek-ai/dsh-credentials', installation.version);
  const llmPath = await versionedExport(baseRequire, '@deepseek-ai/dsh-llm', installation.version);
  const [boot, appBoot, credentials, llm] = await Promise.all([
    import(pathToFileURL(installation.profileBoot).href), import(pathToFileURL(appBootPath).href),
    import(pathToFileURL(credentialsPath).href), import(pathToFileURL(llmPath).href),
  ]);
  if (typeof boot.runProfile !== 'function' || typeof appBoot.initProfile !== 'function'
      || typeof credentials.credentialRef !== 'function' || typeof llm.normalizeApiKey !== 'function') throw new Error('Unsupported fork APIs');
  // Verify the app-boot resolver resolves inside this same runtime, not ambient global packages.
  await versionedExport(appRequire, '@deepseek-ai/dsh-home-paths', installation.version);
  const resolvedProfile = await initializeDesktopProfile(appBoot, home, installation.anchor, installation.version);
  const overlayDir = await mkdtemp(join(tmpdir(), 'dsh-tauri-owner-'));
  const canonicalOverlay = await realpath(overlayDir);
  const patchPath = join(canonicalOverlay, 'owner.patch.yml');
  const cleanup = async () => {
    if (await realpath(overlayDir) !== canonicalOverlay || dirname(canonicalOverlay) !== await realpath(tmpdir())
        || !canonicalOverlay.split(sep).at(-1).startsWith('dsh-tauri-owner-')) throw new Error('Invalid overlay cleanup target');
    await rm(canonicalOverlay, { recursive: true });
  };
  try {
    await chmod(canonicalOverlay, 0o700);
    await writeFile(patchPath, DESKTOP_PATCH, { flag: 'wx', mode: 0o600 });
    const environment = appBoot.loadLayeredEnv('dsh-tauri', home, () => process.stderr.write('[harness] environment diagnostic suppressed\n'));
    return { version: installation.version, environment, resolvedProfile, patchFiles: [patchPath],
      runProfile: boot.runProfile, credentialRef: credentials.credentialRef, normalizeApiKey: llm.normalizeApiKey,
      optionalPayloads: OPTIONAL_PAYLOADS, cleanup };
  } catch (error) {
    try { await cleanup(); } catch (cleanupError) { throw new AggregateError([error, cleanupError], 'Fork initialization cleanup failed'); }
    throw error;
  }
}

const service = (ctx, name) => typeof ctx.get === 'function' ? ctx.get(name) : ctx[name];
const requireService = (ctx, name) => {
  const found = service(ctx, name);
  if (!found) throw new Error('Required fork service unavailable');
  return found;
};

/** Credential metadata only; same provider/settings discovery used by native Desktop onboarding. */
export async function readOnboarding(ctx, credentialRef) {
  const namespaces = requireService(ctx, 'settings').describe({ redactSecrets: true });
  const official = namespaces.find(row => row.ns === 'llm-deepseek');
  const ref = official?.value?.apiKeyEnv;
  const refs = new Set(ref === undefined ? [] : [ref]);
  for (const provider of requireService(ctx, 'llm').listConfigurableProviders()) {
    let value = namespaces.find(row => row.ns === provider.settingsNs)?.value;
    for (const key of provider.settingsPath) value = value?.[key];
    if (typeof value?.apiKeyEnv === 'string') refs.add(value.apiKeyEnv);
  }
  const credentials = requireService(ctx, 'credentials');
  let hasApiKey = false;
  for (const candidate of refs) if ((await credentials.describe(credentialRef(candidate))).configured) hasApiKey = true;
  const writable = ref === undefined ? false : (await credentials.describe(credentialRef(ref))).writable;
  const localePreference = namespaces.find(row => row.ns === 'locale')?.value?.preference ?? null;
  return { loggedIn: (await requireService(ctx, 'deepseekAccount').getState()).status === 'credential-stored',
    hasApiKey, writable, localePreference };
}

/** Write only the official provider reference after upstream header-value validation. */
export async function saveApiKey(ctx, raw, { credentialRef, normalizeApiKey }) {
  const checked = normalizeApiKey(raw);
  if (!checked.ok) return { ok: false };
  const ref = requireService(ctx, 'settings').describe({ redactSecrets: true }).find(row => row.ns === 'llm-deepseek')?.value?.apiKeyEnv;
  if (typeof ref !== 'string') return { ok: false };
  await requireService(ctx, 'credentials').set(credentialRef(ref), checked.value);
  return { ok: true };
}

/** Cancel one live generation; completion means the entire Agent became idle, not one message settled. */
export async function cancelGeneration(ctx, sessionId) {
  const agent = requireService(ctx, 'agents').list().find(item => item.session.id === sessionId);
  if (!agent) return { cancelled: false };
  agent.cancel({ kind: 'hook', reason: 'desktop/cancel-generation' });
  await agent.whenIdle();
  return { cancelled: true };
}

/** Read one currently registered identity; never resume an Agent or infer its id from the request. */
export function discoverSessionAgent(ctx, sessionId) {
  if (!discoveryIdValid(sessionId)) throw new Error('Native agent discovery unavailable');
  const registry = requireService(ctx, 'agents');
  let found;
  for (const agent of registry.list()) {
    if (agent.session.id !== sessionId) continue;
    if (found || registry.get(agent.id) !== agent || !discoveryIdValid(agent.id)) throw new Error('Native agent discovery unavailable');
    found = agent;
  }
  return { sessionId, agentId: found?.id ?? null };
}

/** One opt-in native watch. Aborted provider reads remain owned until they settle; no automatic retry. */
export function createAccountObserver({ send, addSecret = () => {} }) {
  let disposed = false;
  let provider;
  let owner;
  const live = current => !disposed && owner === current && provider === current.provider && !current.lifetime.signal.aborted;
  const clear = current => {
    if (owner === current && !current.cleared) {
      current.cleared = true;
      send({ type: 'platform-session', session: null });
    }
  };
  const retire = current => {
    if (owner === current && current.watchEnded && current.reads.size === 0) owner = undefined;
  };
  const stop = current => {
    if (!current) return Promise.resolve();
    current.lifetime.abort();
    clear(current);
    return Promise.allSettled([current.done, ...current.reads]);
  };
  const watch = async current => {
    try {
      if (!live(current)) return;
      for await (const state of current.provider.account.watch(current.lifetime.signal)) {
        if (!live(current)) break;
        send({ type: 'account-state', state });
        const session = await current.provider.account.getPlatformSession();
        if (!live(current)) break;
        if (session) {
          addSecret(session.token);
          for (const value of Object.values(session.requestHeaders ?? {})) addSecret(value);
        }
        send({ type: 'platform-session', session });
      }
      if (live(current)) send({ type: 'account-subscription-error', message: 'Account subscription ended' });
    } catch (_error) {
      if (live(current)) send({ type: 'account-subscription-error', message: 'Account subscription unavailable' });
    } finally {
      current.lifetime.abort();
      clear(current);
      current.watchEnded = true;
      retire(current);
    }
  };
  return {
    // The lightweight Cordis binding is retained; provider loss revokes observation.
    attach(account) {
      if (disposed) throw new Error('Account observation closed');
      const binding = { account };
      provider = binding;
      if (owner) {
        send({ type: 'account-subscription-error', message: 'Account provider changed' });
        void stop(owner);
      }
      return async () => {
        const observed = owner?.provider === binding ? owner : undefined;
        if (observed && live(observed)) send({ type: 'account-subscription-error', message: 'Account provider unavailable' });
        if (provider === binding) provider = undefined;
        await stop(observed);
      };
    },
    async subscribe() {
      if (disposed || !provider || owner?.lifetime.signal.aborted) throw new Error('Account observation unavailable');
      if (!owner) {
        const current = { provider, lifetime: new AbortController(), cleared: false, watchEnded: false, reads: new Set() };
        owner = current;
        current.done = Promise.resolve().then(() => watch(current));
      }
      const current = owner;
      const read = Promise.resolve().then(() => {
        if (!live(current)) throw new Error('Account observation changed');
        return current.provider.account.getState();
      });
      current.reads.add(read);
      try {
        let state;
        try { state = await read; }
        finally { current.reads.delete(read); retire(current); }
        if (!live(current)) throw new Error('Account observation changed');
        return state;
      } catch (_error) {
        await stop(current);
        throw new Error('Account observation unavailable');
      }
    },
    async unsubscribe() {
      await stop(owner);
      return { subscribed: false };
    },
    async dispose() {
      disposed = true;
      provider = undefined;
      await stop(owner);
    },
  };
}

/** Ownership listeners precede imports; shutdown suppresses all late command/subscription results. */
export function startForkSupervisor({ input = process.stdin, send, loadRuntime = loadForkRuntime, env = process.env,
  inspect = inspectTasks, openWorkspace = addWorkspace, cancel = cancelGeneration, addSecret = () => {},
  onStopped = () => { process.exitCode = 0; }, onFatal = () => { process.exitCode = 1; } } = {}) {
  if (typeof send !== 'function') throw new Error('Protocol sender required');
  let cancelled = false;
  let fatal = false;
  let installed;
  let runtime;
  let codex;
  let boot;
  let shutdownPromise;
  const pending = new Set();
  const busy = new Set();
  const emit = packet => { if (!cancelled) send(packet); };
  const accountObserver = createAccountObserver({ send: emit, addSecret });
  const fail = message => {
    if (!fatal) { fatal = true; send({ type: 'fatal', message }); }
    return shutdown();
  };
  const shutdown = () => {
    cancelled = true;
    if (shutdownPromise) return shutdownPromise;
    const accountStopped = accountObserver.dispose();
    return shutdownPromise = (async () => {
      try {
        try { await boot; } catch (_error) { /* Boot observer reports startup errors. */ }
        let disposalFailed = false;
        try { await codex?.stop(); } catch (_error) { disposalFailed = true; }
        try { if (runtime) await runtime.shutdown.shutdown(0); } catch (_error) { disposalFailed = true; }
        await accountStopped;
        await Promise.allSettled([...pending]);
        send({ type: 'platform-session', session: null });
        await installed?.cleanup?.();
        if (disposalFailed) throw new Error('Harness shutdown failed');
        send({ type: 'shutdown-complete' });
      } catch (_error) {
        if (!fatal) { fatal = true; send({ type: 'fatal', message: 'Harness shutdown failed' }); }
      } finally {
        for (const event of ['data', 'end', 'close', 'error']) input.removeListener(event, event === 'data' ? onData : onEnd);
        input.pause?.();
        if (fatal) onFatal(); else onStopped();
      }
    })();
  };
  const unknown = requestId => emit({ type: 'inspection', requestId, activeTasks: 0, scheduledTasks: 0, unknown: true });
  const handle = async command => {
    if (command.type === 'shutdown') { void shutdown(); return; }
    if (cancelled) return;
    if (!runtime) {
      if (command.type === 'inspect') unknown(command.requestId);
      else emit({ type: 'request-error', requestId: command.requestId, message: 'Harness is still starting' });
      return;
    }
    if (command.type === 'save-api-key' && codex?.busy) {
      emit({ type: 'request-error', requestId: command.requestId, message: 'Native request unavailable' }); return;
    }
    // Unsubscribe can revoke a pending subscribe; its owner denies replacement until reads retire.
    const lane = ['codex-start', 'codex-enable-models', 'account-start', 'account-signout', 'account-cancel', 'save-api-key'].includes(command.type) ? 'account-mutation' : command.type;
    if (busy.has(lane)) {
      if (command.type === 'inspect') unknown(command.requestId);
      else emit({ type: 'request-error', requestId: command.requestId, message: 'Native request already pending' });
      return;
    }
    busy.add(lane);
    try {
      const ctx = runtime.ctx;
      const client = () => ({ version: installed.version, locale: command.locale, timezoneOffsetSeconds: command.timezoneOffsetSeconds });
      const account = () => requireService(ctx, 'deepseekAccount');
      let value;
      switch (command.type) {
        case 'codex-status': case 'codex-start': case 'codex-cancel': case 'codex-enable-models': case 'codex-callback':
          value = await codex.handle(command); break;
        case 'inspect': emit({ type: 'inspection', requestId: command.requestId, ...await inspect(ctx) }); return;
        case 'open-workspace': await openWorkspace(ctx, command.path); emit({ type: 'workspace-added', requestId: command.requestId }); return;
        case 'onboarding-read': value = await readOnboarding(ctx, installed.credentialRef); break;
        case 'save-api-key': addSecret(command.apiKey); value = await saveApiKey(ctx, command.apiKey, installed); break;
        case 'account-state': value = await account().getState(); break;
        case 'account-start': value = await account().startSignIn(client(), new URL(readyUrl(ctx)).origin, 'desktop'); break;
        case 'account-cancel': value = await account().cancelSignIn(command.attemptId); break;
        case 'account-signout': value = await account().signOut(client()); break;
        case 'account-subscribe': value = await accountObserver.subscribe(); break;
        case 'account-unsubscribe': value = await accountObserver.unsubscribe(); break;
        case 'discover-session-agent': value = discoverSessionAgent(ctx, command.sessionId); break;
        case 'cancel-generation': value = await cancel(ctx, command.sessionId); break;
        default: throw new Error('Invalid native command');
      }
      emit({ type: 'native-result', requestId: command.requestId, command: command.type, value });
    } catch (_error) {
      if (command.type === 'inspect') unknown(command.requestId);
      else emit({ type: 'request-error', requestId: command.requestId, message: 'Native request failed' });
    } finally { busy.delete(lane); }
  };
  const receiver = lineReceiver(line => {
    if (cancelled) return;
    let command;
    try { command = parseForkCommand(line); } catch (_error) { void fail('Invalid native command'); return; }
    const task = handle(command);
    pending.add(task);
    void task.catch(() => fail('Native command failed')).finally(() => pending.delete(task));
  }, () => { void fail('Native command exceeds size limit'); });
  const onData = chunk => receiver.write(chunk);
  const onEnd = () => { void shutdown(); };
  input.on('data', onData);
  for (const event of ['end', 'close', 'error']) input.on(event, onEnd);
  boot = Promise.resolve().then(async () => {
    if (input.readableEnded || input.destroyed || cancelled) return;
    installed = await loadRuntime(env);
    if (cancelled) return;
    runtime = await installed.runProfile({ environment: installed.environment, profile: 'desktop',
      resolvedProfile: installed.resolvedProfile, patchFiles: installed.patchFiles,
      args: ['--no-open', '--host', '127.0.0.1', '--port', '0'] });
    if (cancelled) return;
    codex = createCodexBridge({ ctx: runtime.ctx, emit, addSecret });
    const url = readyUrl(runtime.ctx);
    requireService(runtime.ctx, 'deepseekAccount');
    for (const value of new URL(url).searchParams.values()) addSecret(value);
    await runtime.ctx.inject(['deepseekAccount'], accountCtx => {
      accountCtx.effect(() => accountObserver.attach(accountCtx.deepseekAccount));
    });
    emit({ type: 'ready', url, version: installed.version, profile: 'desktop', optionalPayloads: installed.optionalPayloads });
  });
  void boot.catch(() => { if (!cancelled) void fail('Harness startup failed'); });
  if (input.readableEnded || input.destroyed) void shutdown();
  return { boot, shutdown, get cancelled() { return cancelled; } };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const diagnostics = interceptDiagnostics(process.stdout, process.stderr);
  startForkSupervisor({ send: packet => diagnostics.send(packet), addSecret: secret => diagnostics.addSecret(secret) });
}
