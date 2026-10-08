#!/usr/bin/env node
/** Keyless built-fork smoke. Only metadata is persisted; all homes and IPC belong to a unique test root.
 * Run with plain Node 26. No UI engine, browser automation, account mutations, or model requests.
 * Baseline supervisor is imported in place so its adjacent utilities retain their real resolution.
 */
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdir, mkdtemp, chmod, realpath, lstat, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { PREFIX, lineReceiver, interceptDiagnostics } from '../../deepseek-harness-tauri/runtime/supervisor.mjs';
import { startForkSupervisor, loadForkRuntime, resolveIsolatedHome } from '../../deepseek-harness-tauri/runtime/fork-supervisor.mjs';

const SCRIPT = fileURLToPath(import.meta.url);
const NATIVE = resolve(dirname(SCRIPT), '..');
const WORKSPACE = resolve(NATIVE, '..');
const RUNTIME = join(WORKSPACE, 'deepseek-harness-linux/apps/cli');
const VERSION = '0.2.1-alpha.1';
const TIMEOUT = { boot: 90000, command: 15000, http: 10000, shutdown: 20000, exit: 10000 };
const DIAGNOSTIC = 'DSH_SMOKE_DIAGNOSTIC:';
const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const safeCode = value => typeof value === 'string' && /^[A-Z][A-Z0-9_]{0,79}$/.test(value) ? value : 'UNCLASSIFIED';

/** Errors are classified, never serialized: arbitrary messages/stacks can contain credentials. */
function errorMetadata(error, phase) {
  const message = typeof error === 'string' ? error : typeof error?.message === 'string' ? error.message : '';
  const packageName = /(@deepseek-ai\/[a-z0-9-]+)(?:[/\s'".]|$)/.exec(message)?.[1];
  const categories = [
    [/Cannot find (?:package|module)|module not found/i, 'missing-module'],
    [/failed to import/i, 'import-failed'],
    [/SyntaxError|Unexpected (?:token|identifier)/, 'syntax-error'],
    [/Unsupported.*APIs|is not a function|does not provide an export named/i, 'unsupported-api'],
    [/Mismatched|Unexpected.*version/i, 'version-mismatch'],
    [/inactive|unresolved|activation|dependency/i, 'profile-activation'],
    [/isolated|Invalid.*home/i, 'home-isolation'],
    [/\b(?:EACCES|EPERM)\b|permission denied|sandbox: file access denied/i, 'permission-denied'],
    [/ENOENT|no such file/i, 'missing-file'],
  ];
  const api = /does not provide an export named ['"]([A-Za-z_$][\w$]*)['"]/.exec(message)?.[1]
    ?? /\b([A-Za-z_$][\w.$]*) is not a function/.exec(message)?.[1];
  const metadata = { phase, code: safeCode(error?.code ?? /\b(EACCES|EPERM|ENOENT|ERR_[A-Z_]+)\b/.exec(message)?.[1]),
    category: categories.find(([test]) => test.test(message))?.[1] ?? 'runtime-error',
    ...(packageName ? { package: packageName } : {}), ...(api ? { api } : {}) };
  if (Array.isArray(error?.entries)) metadata.entries = error.entries.slice(0, 100).map(entry => ({
    id: /^[\w-]{1,128}$/.test(entry.id ?? '') ? entry.id : 'redacted',
    module: /^@deepseek-ai\/[a-z0-9-]+(?:\/[a-z0-9-]+)*$/.test(entry.module ?? '') ? entry.module : 'redacted',
    required: entry.required === true, outcome: entry.outcome?.kind === 'pending' ? 'pending' : 'failed',
    ...(entry.outcome?.kind === 'pending' ? { missing: (entry.outcome.missing ?? []).filter(name => /^[A-Za-z][\w]{0,79}$/.test(name)).slice(0, 30) }
      : { error: errorMetadata(entry.outcome?.error, 'plugin') }),
  }));
  return metadata;
}

/** Worker uses the unchanged supervisor and its real public profile boot. */
async function worker() {
  const diagnostics = interceptDiagnostics(process.stdout, process.stderr);
  const report = (error, phase) => {
    const { entries, ...summary } = errorMetadata(error, phase);
    process.stderr.write(`${DIAGNOSTIC}${JSON.stringify(summary)}\n`);
    for (const entry of entries ?? []) process.stderr.write(`${DIAGNOSTIC}${JSON.stringify({ phase: 'plugin', ...entry })}\n`);
    // Import failures have no Fiber; their original errors live in StartupError.startup.messages.
    const seen = new Set();
    let count = 0;
    const visit = value => {
      if (count >= 100 || value === undefined || seen.has(value)) return;
      seen.add(value); count++;
      const { entries: _entries, ...row } = errorMetadata(value, 'cause');
      if (row.category !== 'runtime-error' || row.code !== 'UNCLASSIFIED' || row.api)
        process.stderr.write(`${DIAGNOSTIC}${JSON.stringify(row)}\n`);
      if (object(value)) {
        visit(value.cause);
        for (const nested of value.errors ?? []) visit(nested);
        for (const log of value.startup?.messages ?? []) for (const arg of log.args ?? []) visit(arg);
      }
    };
    visit(error);
  };
  startForkSupervisor({
    send: packet => diagnostics.send(packet), addSecret: secret => diagnostics.addSecret(secret),
    loadRuntime: async env => {
      let installed;
      try { installed = await loadForkRuntime(env); }
      catch (error) { report(error, 'load-runtime'); throw error; }
      process.stderr.write(`${DIAGNOSTIC}${JSON.stringify({ phase: 'load-runtime-success', versionMatches: installed.version === VERSION,
        profile: 'desktop', bundledAnchorsValidated: true })}\n`);
      const runProfile = installed.runProfile;
      return { ...installed, runProfile: async options => {
        try { return await runProfile(options); }
        catch (error) { report(error, 'run-profile'); throw error; }
      } };
    },
  });
}

/** Private memory-only cookie jar; no header, token, or complete HTTP body reaches the report. */
class PrivateJar {
  #cookie;
  accept(response) {
    const cookies = response.headers.getSetCookie();
    assert.equal(cookies.length, 1, 'exchange-cookie-count');
    const cookie = cookies[0];
    assert.match(cookie, /;\s*HttpOnly(?:;|$)/i, 'exchange-http-only');
    assert.match(cookie, /;\s*SameSite=Strict(?:;|$)/i, 'exchange-same-site');
    assert.match(cookie, /;\s*Path=\/(?:;|$)/i, 'exchange-path');
    assert.doesNotMatch(cookie, /;\s*Secure(?:;|$)/i, 'loopback-cookie-security');
    this.#cookie = cookie.split(';', 1)[0];
  }
  headers() { assert.ok(this.#cookie, 'private-cookie-present'); return { cookie: this.#cookie }; }
  clear() { this.#cookie = undefined; }
}

async function smoke() {
  assert.equal(process.platform, 'linux', 'linux-required');
  assert.equal(Number(process.versions.node.split('.')[0]), 26, 'node-26-required');
  process.umask(0o077);
  const parent = join(NATIVE, 'smoke-home');
  await mkdir(parent, { recursive: true, mode: 0o700 });
  assert.equal(await realpath(parent), parent, 'smoke-home-must-not-be-an-alias');
  const root = await mkdtemp(join(parent, 'fork-backend-'));
  await chmod(root, 0o700);
  assert.equal(await realpath(root), root, 'test-root-must-not-be-an-alias');
  const paths = Object.fromEntries(['harness', 'previous', 'config', 'cache', 'data', 'runtime', 'tmp', 'user-home', 'agents', 'workspace', 'workspace-two']
    .map(name => [name, join(root, name)]));
  await Promise.all(Object.values(paths).map(path => mkdir(path, { mode: 0o700 })));
  // Allowlist, rather than redaction alone, excludes inherited DSH/XDG, proxies, secrets, and Node preload hooks.
  const env = Object.fromEntries(['PATH', 'LANG', 'LC_ALL', 'TZ'].filter(key => process.env[key] !== undefined)
    .map(key => [key, process.env[key]]));
  Object.assign(env, { HOME: paths['user-home'], XDG_CONFIG_HOME: paths.config, XDG_CACHE_HOME: paths.cache,
    XDG_DATA_HOME: paths.data, XDG_RUNTIME_DIR: paths.runtime, TMPDIR: paths.tmp, TMP: paths.tmp, TEMP: paths.tmp,
    DSH_AGENTS_HOME: paths.agents, DSH_HOME: paths.previous, DSH_TAURI_HOME: paths.harness,
    DSH_TAURI_RUNTIME: RUNTIME, DSH_TAURI_EXPECTED_VERSION: VERSION, DSH_TELEMETRY_DISABLED: '1',
    NODE_NO_WARNINGS: '1' });
  assert.equal(await resolveIsolatedHome(env), paths.harness, 'distinct-home-alias-guard');
  const resultPath = join(root, 'result.json');
  const eventsPath = join(root, 'protocol-metadata.jsonl');
  const report = { status: 'running', node: process.version, expectedVersion: VERSION, runtime: RUNTIME,
    supervisor: join(WORKSPACE, 'deepseek-harness-tauri/runtime/fork-supervisor.mjs'), root,
    isolation: { harnessHome: paths.harness, previousHome: paths.previous, cwd: paths.workspace, inheritedEnvironment: 'allowlisted',
      distinctHomeGuard: true, cachesAndIPC: 'test-root-only' },
    capabilities: { office: 'unavailable', browser: 'disabled', nativeUIEngine: 'none', fullParity: false },
    modelCalls: 'none-issued', accountMutations: 'none-issued', checks: [], startupErrors: [],
    diagnostics: { lines: 0, runtimeErrorsSuppressed: 0, unframedStdout: 0 },
    shutdown: { complete: false, exitObserved: false, forcedSignals: [], groupGone: false } };
  const events = [];
  const jar = new PrivateJar();
  const started = performance.now();
  const pending = new Set();
  let child, exit, failed, shutdownComplete = false, ready;
  let requestNumber = 0;
  let stopping = false;
  const timed = (promise, ms, name) => {
    let timer;
    return Promise.race([promise, new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error(`${name}-timeout`)), ms);
    })]).finally(() => clearTimeout(timer));
  };
  const note = (name, metadata = {}) => report.checks.push({ name, ...metadata });
  const failWaiters = error => {
    failed ??= error;
    for (const waiter of [...pending]) { pending.delete(waiter); clearTimeout(waiter.timer); waiter.reject(error); }
  };
  const waitPacket = (test, ms, name) => {
    if (failed) return Promise.reject(failed);
    if (exit) return Promise.reject(new Error(`${name}-process-exited`));
    return new Promise((resolveWait, reject) => {
      const waiter = { test, resolve: resolveWait, reject };
      waiter.timer = setTimeout(() => { pending.delete(waiter); reject(new Error(`${name}-timeout`)); }, ms);
      pending.add(waiter);
    });
  };
  const summarize = packet => {
    const types = ['ready', 'inspection', 'platform-session', 'native-result', 'account-state', 'request-error',
      'fatal', 'workspace-added', 'account-subscription-error', 'shutdown-complete'];
    const row = { elapsedMs: Math.round(performance.now() - started), type: types.includes(packet.type) ? packet.type : 'unknown' };
    if (typeof packet.requestId === 'string' && /^[A-Za-z0-9_-]{1,128}$/.test(packet.requestId)) row.requestId = packet.requestId;
    switch (packet.type) {
      case 'ready': row.versionMatches = packet.version === VERSION; row.profileMatches = packet.profile === 'desktop'; break;
      case 'inspection': Object.assign(row, {
        activeTasks: Number.isSafeInteger(packet.activeTasks) ? packet.activeTasks : null,
        scheduledTasks: Number.isSafeInteger(packet.scheduledTasks) ? packet.scheduledTasks : null,
        unknown: packet.unknown !== false }); break;
      case 'platform-session': row.present = packet.session !== null; break;
      case 'native-result': row.command = ['onboarding-read', 'account-state', 'account-subscribe', 'account-unsubscribe'].includes(packet.command)
        ? packet.command : 'unknown'; row.success = true; break;
      case 'account-state': row.signedOut = packet.state?.status === 'signed-out'; break;
      case 'request-error': row.error = ['Harness is still starting', 'Native request already pending', 'Native request failed'].includes(packet.message)
        ? packet.message : 'redacted'; break;
      case 'fatal': row.error = ['Harness startup failed', 'Harness shutdown failed', 'Invalid native command'].includes(packet.message)
        ? packet.message : 'redacted'; break;
    }
    return row;
  };
  const send = packet => {
    assert.ok(child && !exit && !child.stdin.destroyed, 'owned-process-input');
    child.stdin.write(`${JSON.stringify(packet)}\n`);
  };
  const command = async (type, fields = {}) => {
    const requestId = `smoke-${++requestNumber}`;
    const response = waitPacket(packet => packet.requestId === requestId, TIMEOUT.command, type);
    send({ type, requestId, ...fields });
    const packet = await response;
    assert.notEqual(packet.type, 'request-error', `${type}-request-error`);
    return packet;
  };
  const http = async (url, options = {}) => {
    // Native authentication is strictly same-origin loopback; redirects are NEVER followed.
    assert.equal(new URL(url).origin, new URL(ready.url).origin, 'same-origin-http');
    const response = await fetch(url, { ...options, redirect: 'manual', signal: AbortSignal.timeout(TIMEOUT.http) });
    const body = await timed((async () => {
      const reader = response.body?.getReader();
      if (!reader) return '';
      let bytes = 0;
      const chunks = [];
      try {
        while (true) {
          const chunk = await reader.read();
          if (chunk.done) break;
          bytes += chunk.value.byteLength;
          if (bytes > 256 * 1024) throw new Error('http-response-too-large');
          chunks.push(Buffer.from(chunk.value));
        }
        return Buffer.concat(chunks).toString('utf8');
      } finally { await reader.cancel(); }
    })(), TIMEOUT.http, 'http-body');
    return { response, body };
  };
  const groupExists = () => {
    if (!child?.pid) return false;
    try { process.kill(-child.pid, 0); return true; }
    catch (error) { if (error.code === 'ESRCH') return false; throw error; }
  };
  const signalGroup = signal => {
    assert.ok(child?.pid > 1 && child.pid !== process.pid, 'owned-detached-process-group');
    if (!groupExists()) return;
    process.kill(-child.pid, signal); // Only the session created by this exact spawn is owned.
    report.shutdown.forcedSignals.push(signal);
  };
  let exitPromise, closePromise;
  const onInterrupt = () => { report.interrupted = true; failWaiters(new Error('runner-interrupted')); };
  process.on('SIGINT', onInterrupt);
  process.on('SIGTERM', onInterrupt);
  const stop = async () => {
    if (stopping) return;
    stopping = true;
    if (!child?.pid) return;
    if (!exit && !shutdownComplete && !child.stdin.destroyed) {
      try {
        const completion = waitPacket(packet => packet.type === 'shutdown-complete', TIMEOUT.shutdown, 'shutdown');
        send({ type: 'shutdown' });
        await completion;
      } catch (_error) { /* Independent exit and cleanup results are recorded below. */ }
    }
    if (!exit) {
      try { await timed(exitPromise, TIMEOUT.exit, 'exit'); }
      catch (_error) {
        signalGroup('SIGTERM');
        try { await timed(exitPromise, 5000, 'terminate'); }
        catch (_error) { signalGroup('SIGKILL'); await timed(exitPromise, 5000, 'kill'); }
      }
    }
    // Descendants, if any, remain our co-owned test group even after the leader exits.
    if (groupExists()) {
      signalGroup('SIGKILL');
      const deadline = performance.now() + 5000;
      while (groupExists() && performance.now() < deadline) await new Promise(resolveWait => setTimeout(resolveWait, 50));
    }
    report.shutdown.groupGone = !groupExists();
    // Drain already-written metadata, but success still requires the independently observed exit.
    await timed(closePromise, 5000, 'owned-pipe-close');
    report.shutdown.complete = shutdownComplete;
    report.shutdown.exitObserved = Boolean(exit);
    if (exit) Object.assign(report.shutdown, exit);
    child.stdin.destroy();
    child.stdout.destroy();
    child.stderr.destroy();
    jar.clear();
  };
  try {
    child = spawn(process.execPath, [SCRIPT, '--worker'], { cwd: paths.workspace, env,
      detached: true, stdio: ['pipe', 'pipe', 'pipe'] });
    closePromise = new Promise(resolveClose => child.once('close', resolveClose));
    exitPromise = new Promise(resolveExit => child.once('exit', (code, signal) => {
      exit = { code, signal }; resolveExit(exit);
      failWaiters(new Error('owned-process-exited'));
    }));
    child.once('error', () => failWaiters(new Error('owned-process-spawn-error')));
    child.stdin.on('error', () => { if (!stopping) failWaiters(new Error('owned-process-input-error')); });
    const receiver = lineReceiver(line => {
      if (!line.startsWith(PREFIX)) { report.diagnostics.unframedStdout++; return; }
      let packet;
      try { packet = JSON.parse(line.slice(PREFIX.length)); }
      catch (_error) { failWaiters(new Error('invalid-protocol-json')); return; }
      if (!object(packet) || typeof packet.type !== 'string') { failWaiters(new Error('invalid-protocol-packet')); return; }
      if (events.length >= 10000) { failWaiters(new Error('protocol-event-limit')); return; }
      events.push(summarize(packet));
      if (packet.type === 'shutdown-complete') shutdownComplete = true;
      if (packet.type === 'fatal') { failWaiters(new Error(`supervisor-${summarize(packet).error}`)); return; }
      for (const waiter of [...pending]) {
        if (waiter.test(packet)) { pending.delete(waiter); clearTimeout(waiter.timer); waiter.resolve(packet); }
      }
    }, () => failWaiters(new Error('oversized-private-protocol')));
    child.stdout.on('data', chunk => receiver.write(chunk));
    // EOF is never readiness or successful shutdown. Only shutdown-complete + process exit qualify.
    child.stdout.on('end', () => { receiver.end(); });
    const diagnosticReceiver = lineReceiver(line => {
      report.diagnostics.lines++;
      if (line.includes('runtime error diagnostic suppressed')) report.diagnostics.runtimeErrorsSuppressed++;
      const index = line.indexOf(DIAGNOSTIC);
      if (index < 0) return;
      try {
        const metadata = JSON.parse(line.slice(index + DIAGNOSTIC.length));
        if (metadata.phase === 'load-runtime-success') note('built-fork-bootstrap', {
          versionMatches: metadata.versionMatches === true, profile: 'desktop', bundledAnchorsValidated: metadata.bundledAnchorsValidated === true });
        if (metadata.phase === 'plugin') {
          report.startupErrors.push({ phase: 'plugin', id: metadata.id, module: metadata.module,
            required: metadata.required, outcome: metadata.outcome,
            ...(metadata.missing ? { missing: metadata.missing } : {}), ...(metadata.error ? { error: metadata.error } : {}) });
        }
        if (['load-runtime', 'run-profile', 'cause'].includes(metadata.phase)) report.startupErrors.push({
          phase: metadata.phase, code: safeCode(metadata.code),
          category: ['missing-module', 'import-failed', 'syntax-error', 'unsupported-api', 'version-mismatch', 'profile-activation', 'home-isolation', 'permission-denied', 'missing-file', 'runtime-error'].includes(metadata.category) ? metadata.category : 'redacted',
          ...(/^@deepseek-ai\/[a-z0-9-]+$/.test(metadata.package ?? '') ? { package: metadata.package } : {}),
          ...(/^[A-Za-z_$][\w.$]{0,120}$/.test(metadata.api ?? '') ? { api: metadata.api } : {}),
          ...(Array.isArray(metadata.entries) ? { entries: metadata.entries } : {}) });
      } catch (_error) { /* Unstructured diagnostics are counted but never retained. */ }
    }, () => { report.diagnostics.lines++; });
    child.stderr.on('data', chunk => diagnosticReceiver.write(chunk));
    ready = await waitPacket(packet => packet.type === 'ready', TIMEOUT.boot, 'boot');
    const url = new URL(ready.url);
    assert.equal(url.protocol, 'http:', 'loopback-protocol');
    assert.equal(url.hostname, '127.0.0.1', 'loopback-host');
    assert.equal(url.pathname, '/', 'root-launch-path');
    assert.equal(url.username + url.password, '', 'no-url-credentials');
    assert.match(url.searchParams.get('token') ?? '', /^[A-Za-z0-9_-]{43}$/, 'launch-token-format');
    assert.equal(ready.version, VERSION, 'built-fork-version');
    assert.equal(ready.profile, 'desktop', 'desktop-profile');
    assert.equal(ready.optionalPayloads?.office?.status, 'unavailable', 'office-unavailable');
    assert.equal(ready.optionalPayloads?.browser?.status, 'disabled', 'browser-disabled');
    note('ready', { version: ready.version, profile: ready.profile, elapsedMs: Math.round(performance.now() - started) });

    const inspected = await command('inspect');
    assert.equal(inspected.type, 'inspection', 'inspection-packet');
    assert.equal(inspected.activeTasks, 0, 'zero-active-tasks');
    assert.equal(inspected.scheduledTasks, 0, 'zero-scheduled-tasks');
    assert.equal(inspected.unknown, false, 'task-inspection-known');
    note('inspect', { activeTasks: 0, scheduledTasks: 0, unknown: false });
    const onboarding = await command('onboarding-read');
    assert.equal(onboarding.type, 'native-result', 'onboarding-packet');
    assert.equal(onboarding.value?.loggedIn, false, 'not-logged-in');
    assert.equal(onboarding.value?.hasApiKey, false, 'no-api-key');
    note('onboarding-read', { loggedIn: false, hasApiKey: false, writable: onboarding.value.writable });
    const account = await command('account-state');
    assert.equal(account.value?.status, 'signed-out', 'native-account-signed-out');
    assert.equal(account.value?.attempt, null, 'no-account-attempt');
    note('account-state', { status: 'signed-out', attemptPresent: false });
    const subscribed = await command('account-subscribe');
    assert.equal(subscribed.value?.status, 'signed-out', 'subscription-snapshot');
    const unsubscribed = await command('account-unsubscribe');
    assert.equal(unsubscribed.value?.subscribed, false, 'subscription-disposed');
    note('account-subscription-lifecycle', { snapshotReceived: true, unsubscribed: true,
      scope: 'snapshot-and-unsubscribe-ack; no account mutation or forced update' });
    for (const path of [paths.workspace, paths['workspace-two']]) {
      assert.equal((await command('open-workspace', { path })).type, 'workspace-added', 'workspace-added-packet');
    }
    note('open-workspace', { existingDirectoriesRegistered: 2 });
    const rpcId = 'native-smoke-account';
    const envelope = { type: 'client-request', rpcId, method: 'account/getState', payload: { args: {} } };
    const endpoint = new URL('/api/account/getState', url.origin);
    const headers = { 'content-type': 'application/json' };
    const unauthenticated = await http(endpoint, { method: 'POST', headers, body: JSON.stringify(envelope) });
    assert.equal(unauthenticated.response.status, 401, 'unauthenticated-account-denied');
    note('http-unauthenticated-account', { status: 401 });
    const exchange = await http(ready.url);
    assert.equal(exchange.response.status, 303, 'launch-token-exchange-status');
    assert.equal(exchange.response.headers.get('location'), './', 'clean-token-redirect');
    jar.accept(exchange.response);
    note('launch-token-cookie-exchange', { status: 303, redirectsFollowed: false, httpOnly: true, sameSite: 'Strict', jar: 'memory-only' });
    const authenticated = await http(endpoint, { method: 'POST', headers: { ...headers, ...jar.headers() }, body: JSON.stringify(envelope) });
    assert.equal(authenticated.response.status, 200, 'authenticated-account-http-status');
    const response = JSON.parse(authenticated.body);
    assert.equal(response.type, 'server-response', 'remote-response-type');
    assert.equal(response.rpcId, rpcId, 'remote-rpc-id-echo');
    assert.equal(response.result?.ok, true, 'remote-account-result');
    assert.equal(response.result.value?.status, 'signed-out', 'remote-account-signed-out');
    assert.equal(response.result.value?.attempt, null, 'remote-no-account-attempt');
    note('native-authenticated-account-http', { status: 200, envelope: 'client-request', method: 'account/getState', accountStatus: 'signed-out' });
    // HEAD bounds the optional web-shell observation without retaining HTML or bootstrap content.
    const index = await http(new URL('/', url.origin), { method: 'HEAD', headers: jar.headers() });
    note('optional-web-shell', { status: index.response.status, htmlQualified: false,
      requirement: 'not-required-for-native-primary-ui', observation: index.response.status === 404 ? 'static-index-unavailable' : 'head-only; HTML and client bundles not validated' });
    const finalInspection = await command('inspect');
    assert.deepEqual([finalInspection.activeTasks, finalInspection.scheduledTasks, finalInspection.unknown], [0, 0, false], 'final-inspection-idle');
    note('final-inspect', { activeTasks: 0, scheduledTasks: 0, unknown: false });
    await stop();
    assert.ok(report.shutdown.complete, 'shutdown-complete-required');
    assert.ok(report.shutdown.exitObserved, 'process-exit-required-not-stdout-eof');
    assert.equal(report.shutdown.code, 0, 'graceful-exit-code');
    assert.equal(report.shutdown.signal, null, 'no-exit-signal');
    assert.deepEqual(report.shutdown.forcedSignals, [], 'no-forced-cleanup');
    assert.ok(report.shutdown.groupGone, 'owned-process-group-gone');
    assert.equal(report.diagnostics.unframedStdout, 0, 'private-protocol-only-stdout');
    assert.ok((await lstat(paths.previous)).isDirectory(), 'previous-home-preserved');
    assert.notEqual(report.interrupted, true, 'runner-not-interrupted');
    report.status = 'passed';
  } catch (error) {
    report.status = 'failed';
    // Assertions include potentially secret actual/expected values, so NEVER serialize the assertion object.
    report.failure = { check: /^[a-z0-9 -]+$/i.test(error?.message ?? '') ? error.message : 'check-failed-redacted',
      ...errorMetadata(error, 'smoke') };
  } finally {
    try { await stop(); }
    catch (error) { report.status = 'failed'; report.cleanupError = errorMetadata(error, 'cleanup'); }
    process.removeListener('SIGINT', onInterrupt);
    process.removeListener('SIGTERM', onInterrupt);
    report.elapsedMs = Math.round(performance.now() - started);
    await writeFile(eventsPath, events.map(row => JSON.stringify(row)).join('\n') + '\n', { flag: 'wx', mode: 0o600 });
    await writeFile(resultPath, JSON.stringify(report, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
    console.log(JSON.stringify({ status: report.status, resultPath, eventsPath,
      checks: report.checks.length, startupErrorCount: report.startupErrors.length,
      startupErrorExamples: report.startupErrors.filter(row => row.phase === 'cause' && row.code !== 'UNCLASSIFIED').slice(0, 8),
      failure: report.failure, shutdown: report.shutdown }));
    process.exitCode = report.status === 'passed' ? 0 : 1;
  }
}

if (process.argv[2] === '--worker') await worker();
else {
  try { assert.equal(process.argv.length, 2, 'no-runtime-overrides-supported'); await smoke(); }
  catch (error) { console.error(JSON.stringify(errorMetadata(error, 'runner'))); process.exitCode = 1; }
}
