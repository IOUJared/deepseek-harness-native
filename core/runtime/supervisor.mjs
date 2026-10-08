#!/usr/bin/env node
/** Private native-parent adapter. No HTTP routes, plugins, or profile overlays. */
import { createRequire } from 'node:module';
import { realpath, readFile, stat } from 'node:fs/promises';
import { dirname, delimiter, isAbsolute, join, resolve } from 'node:path';
import { homedir } from 'node:os';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { StringDecoder } from 'node:string_decoder';

export const PREFIX = 'DSH_TAURI:';
export const MAX_COMMAND_BYTES = 64 * 1024;
export const MAX_DIAGNOSTIC_BYTES = 4096;
const MAX_ITEMS = 10000;
const execFileAsync = promisify(execFile);
const requestIdValid = id => typeof id === 'string' && /^[A-Za-z0-9_-]{1,128}$/.test(id);

/** Reject extra fields: this pipe is not an arbitrary runtime invocation surface. */
export function parseCommand(line) {
  if (Buffer.byteLength(line) > MAX_COMMAND_BYTES) throw new Error('Invalid native command');
  const value = JSON.parse(line);
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Invalid native command');
  const keys = Object.keys(value).sort().join(',');
  if (value.type === 'shutdown' && keys === 'type') return value;
  if (value.type === 'inspect' && keys === 'requestId,type' && requestIdValid(value.requestId)) return value;
  if (value.type === 'open-workspace' && keys === 'path,requestId,type' && requestIdValid(value.requestId)
      && typeof value.path === 'string' && value.path.length > 0 && value.path.length <= 32768
      && isAbsolute(value.path) && !/[\x00-\x1f\x7f]/.test(value.path)) return value;
  throw new Error('Invalid native command');
}

/** Complete bounded lines only: oversized lines are dropped, never split across secrets. */
export function lineReceiver(onLine, onOversize, limit = MAX_COMMAND_BYTES) {
  const decoder = new StringDecoder('utf8');
  let pending = '';
  let dropping = false;
  const feedText = text => {
    for (const part of text.split(/(?<=\n)/)) {
      if (!part) continue;
      const ended = part.endsWith('\n');
      if (!dropping) {
        if (Buffer.byteLength(pending) + Buffer.byteLength(part) > limit) {
          pending = '';
          dropping = true;
          onOversize();
        } else pending += part;
      }
      if (ended) {
        if (!dropping) onLine(pending.replace(/\r?\n$/, ''));
        pending = '';
        dropping = false;
      }
    }
  };
  return {
    write(chunk) { feedText(decoder.write(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk))); },
    end() {
      feedText(decoder.end());
      // Commands require newline framing; diagnostics may flush their final bounded line.
      return !dropping && pending ? pending : undefined;
    },
  };
}

/** Redact credentials before truncation, including URLs, structured keys, and ANSI escapes. */
export function redactDiagnostic(value, secrets = []) {
  let text = String(value).replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, '').replace(/[\x00-\x08\x0b-\x1f\x7f]/g, '');
  for (const secret of secrets) if (typeof secret === 'string' && secret.length >= 4) text = text.split(secret).join('[REDACTED]');
  text = text
    .replace(/(https?:\/\/)[^\s/]+@/gi, '$1[REDACTED]@')
    .replace(/(https?:\/\/[^\s?#]+)[?#][^\s]*/gi, '$1?[REDACTED]')
    .replace(/\bBearer\s+[^\s,;"']+/gi, 'Bearer [REDACTED]')
    .replace(/(["']?(?:[\w-]*(?:token|password|passwd|secret|api[_-]?key|authorization)[\w-]*)["']?\s*[:=]\s*)(?:"[^"\n]*"|'[^'\n]*'|[^\s,;}]+)/gi, '$1[REDACTED]')
    .replace(/\b(?:sk-[A-Za-z0-9_-]{8,}|eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+)\b/g, '[REDACTED]');
  return text.slice(0, MAX_DIAGNOSTIC_BYTES);
}

/** Runtime console output never reaches the protocol stdout channel. */
export function interceptDiagnostics(stdout, stderr, env = process.env) {
  const rawOut = stdout.write.bind(stdout);
  const rawErr = stderr.write.bind(stderr);
  const originalOut = stdout.write;
  const originalErr = stderr.write;
  let outputOpen = true;
  let errorOpen = true;
  const onOutputError = () => { outputOpen = false; };
  const onDiagnosticError = () => { errorOpen = false; };
  stdout.on('error', onOutputError);
  stderr.on('error', onDiagnosticError);
  const diagnostic = text => {
    // Runtime log floods cannot build an unbounded pending stderr queue.
    if (errorOpen && (stderr.writableLength ?? 0) < 64 * 1024) {
      try { rawErr(text); } catch { errorOpen = false; }
    }
  };
  const secrets = Object.entries(env).filter(([key]) => /token|password|passwd|secret|api_?key/i.test(key)).map(([, value]) => value);
  const emit = line => {
    // Avoid echoing arbitrary exception text / stack traces from third-party packages.
    if (/\b(?:Error|Exception|AggregateError)\b|^\s+at\s/.test(line)) {
      diagnostic('[harness] runtime error diagnostic suppressed\n');
      return;
    }
    diagnostic(`[harness] ${redactDiagnostic(line, secrets)}\n`);
  };
  const receivers = [stdout, stderr].map(() => lineReceiver(emit, () => diagnostic('[harness] oversized diagnostic suppressed\n'), MAX_DIAGNOSTIC_BYTES));
  [stdout, stderr].forEach((stream, i) => {
    stream.write = (chunk, encoding, callback) => {
      receivers[i].write(typeof chunk === 'string' ? Buffer.from(chunk, typeof encoding === 'string' ? encoding : 'utf8') : chunk);
      const done = typeof encoding === 'function' ? encoding : callback;
      if (done) queueMicrotask(done);
      return true;
    };
  });
  return {
    send(packet) {
      if (!outputOpen) return false;
      try { return rawOut(`${PREFIX}${JSON.stringify(packet)}\n`); } catch { outputOpen = false; return false; }
    },
    addSecret(secret) { if (typeof secret === 'string' && secret.length >= 4) secrets.push(secret); },
    restore() {
      receivers.forEach(receiver => { const tail = receiver.end(); if (tail) emit(tail); });
      stdout.write = originalOut;
      stderr.write = originalErr;
      stdout.removeListener('error', onOutputError);
      stderr.removeListener('error', onDiagnosticError);
    },
  };
}

async function packageAt(candidate) {
  let path;
  try { path = await realpath(candidate); } catch { return undefined; }
  if (!(await stat(path)).isDirectory()) path = dirname(path);
  for (let i = 0; i < 6; i++, path = dirname(path)) {
    try {
      const anchor = join(path, 'package.json');
      const metadata = JSON.parse(await readFile(anchor, 'utf8'));
      if (metadata.name === '@deepseek-ai/dsh') {
        const require = createRequire(anchor);
        return { anchor, require, version: metadata.version, profileBoot: require.resolve('@deepseek-ai/dsh/profile-boot') };
      }
    } catch { /* A non-package executable or parent directory is not an installation. */ }
  }
}

/** Explicit override is authoritative; otherwise use installed dsh, common install, then npm global. */
export async function discoverRuntime(env = process.env) {
  if (env.DSH_TAURI_RUNTIME !== undefined) {
    if (!env.DSH_TAURI_RUNTIME || !isAbsolute(env.DSH_TAURI_RUNTIME)) throw new Error('Invalid runtime installation override');
    const found = await packageAt(env.DSH_TAURI_RUNTIME);
    if (!found) throw new Error('Harness runtime installation not found');
    return found;
  }
  for (const entry of (env.PATH ?? '').split(delimiter).filter(Boolean)) {
    const found = await packageAt(join(entry, 'dsh'));
    if (found) return found;
  }
  const common = await packageAt(join(env.HOME || homedir(), '.local/share/deepseek-harness-cli/lib/node_modules/@deepseek-ai/dsh'));
  if (common) return common;
  try {
    const { stdout } = await execFileAsync('npm', ['root', '--global'], { env, timeout: 3000, maxBuffer: 16384 });
    const root = stdout.trim();
    if (isAbsolute(root)) {
      const found = await packageAt(join(root, '@deepseek-ai/dsh'));
      if (found) return found;
    }
  } catch { /* Final fixed failure below; never echo command errors or inherited secrets. */ }
  throw new Error('Harness runtime installation not found');
}

export async function loadInstalledRuntime(env = process.env) {
  const installation = await discoverRuntime(env);
  const boot = await import(pathToFileURL(installation.profileBoot).href);
  const appBoot = await import(pathToFileURL(installation.require.resolve('@deepseek-ai/dsh-app-boot')).href);
  if (typeof boot.runProfile !== 'function' || typeof appBoot.loadLayeredEnv !== 'function') throw new Error('Unsupported Harness runtime');
  return {
    runProfile: boot.runProfile,
    environment: appBoot.loadLayeredEnv('dsh-tauri', process.cwd(), () => process.stderr.write('[harness] environment diagnostic suppressed\n')),
    version: installation.version,
  };
}

function service(ctx, name) {
  try { return typeof ctx.get === 'function' ? ctx.get(name) : ctx[name]; } catch { return undefined; }
}

export function readyUrl(ctx) {
  const server = service(ctx, 'webServer');
  const connection = service(ctx, 'connection');
  if (!server || server.host !== '127.0.0.1' || !Number.isInteger(server.port) || server.port < 1 || server.port > 65535
      || typeof connection?.authenticatedUrl !== 'function') throw new Error('Loopback Harness Web runtime unavailable');
  const base = `http://127.0.0.1:${server.port}`;
  const url = connection.authenticatedUrl(base);
  const parsed = new URL(url);
  if (parsed.origin !== base || parsed.username || parsed.password || parsed.pathname !== '/' || typeof url !== 'string' || url.length > 16384)
    throw new Error('Invalid Harness Web URL');
  return url;
}

async function timeoutResult(work, timeoutMs, fallback) {
  let timer;
  try {
    return await Promise.race([
      Promise.resolve().then(work),
      new Promise(resolveTimeout => { timer = setTimeout(() => resolveTimeout(fallback()), timeoutMs); }),
    ]);
  } catch { return fallback(); } finally { clearTimeout(timer); }
}

/** Count public live Agent + Job registry work; ask activity providers for armed reminders. */
export async function inspectTasks(ctx, { timeoutMs = 1500 } = {}) {
  const state = { activeTasks: 0, scheduledTasks: 0, unknown: false };
  let expired = false;
  const result = () => ({ ...state });
  return timeoutResult(async () => {
    const sessions = new Set();
    const addSession = id => {
      if (sessions.size < MAX_ITEMS || sessions.has(id)) sessions.add(id);
      else state.unknown = true;
    };
    const agents = service(ctx, 'agents');
    const jobs = service(ctx, 'jobs');
    const persistence = service(ctx, 'sessionPersistence');
    let live = [];
    try {
      if (typeof agents?.list !== 'function') throw new Error();
      live = agents.list();
      if (!Array.isArray(live) || live.length > MAX_ITEMS) throw new Error();
      for (const agent of live) {
        if (typeof agent?.session?.id !== 'string') { state.unknown = true; continue; }
        addSession(agent.session.id);
        const nextTurn = agent.inbox?.nextTurn;
        const nextStep = agent.inbox?.nextStep;
        const inboxValid = Array.isArray(nextTurn) && Array.isArray(nextStep);
        if (!inboxValid) state.unknown = true;
        const queued = (Array.isArray(nextTurn) && nextTurn.length > 0) || (Array.isArray(nextStep) && nextStep.length > 0);
        if (agent.status === 'running' || queued) state.activeTasks++;
        if (agent.status !== 'running' && agent.status !== 'idle') state.unknown = true;
      }
    } catch { state.unknown = true; live = []; }
    const jobIds = new Set();
    let jobVisits = 0;
    try {
      if (typeof jobs?.list !== 'function') throw new Error();
      // list() without caller sees ONLY unowned jobs. Query every live owner as well.
      for (const owner of [undefined, ...sessions]) {
        const list = jobs.list(owner);
        if (!Array.isArray(list) || list.length > MAX_ITEMS) throw new Error();
        for (const job of list) {
          if (++jobVisits > MAX_ITEMS) throw new Error();
          if (typeof job?.id !== 'string') { state.unknown = true; continue; }
          if (job.owner !== undefined && typeof job.owner !== 'string') state.unknown = true;
          if (typeof job.owner === 'string') addSession(job.owner);
          if (jobIds.has(job.id)) continue;
          jobIds.add(job.id);
          if (job.status === 'running' || job.status === 'stopping') state.activeTasks++;
          else if (!['completed', 'killed', 'failed'].includes(job.status)) state.unknown = true;
        }
      }
    } catch { state.unknown = true; }
    try {
      if (typeof persistence?.list !== 'function') throw new Error();
      const stored = await persistence.list();
      if (!Array.isArray(stored) || stored.length > MAX_ITEMS) throw new Error();
      for (const item of stored) {
        if (typeof item?.header?.id === 'string') addSession(item.header.id);
        else state.unknown = true;
      }
    } catch { state.unknown = true; }
    if (expired) return { ...state, unknown: true };
    if (typeof ctx.waterfall !== 'function') state.unknown = true;
    else {
      const ids = [...sessions];
      const scheduled = new Set();
      let activityVisits = 0;
      let next = 0;
      await Promise.all(Array.from({ length: Math.min(8, ids.length) }, async () => {
        while (!expired && next < ids.length) {
          const sessionId = ids[next++];
          try {
            const activity = await ctx.waterfall('workspace/session-activity', { sessionId }, () => Promise.resolve([]));
            if (!Array.isArray(activity) || activity.length > MAX_ITEMS) throw new Error();
            for (const row of activity) {
              if (++activityVisits > MAX_ITEMS) { expired = true; throw new Error(); }
              if (row?.kind === 'schedule') {
                if (!Array.isArray(row.items) || row.items.length === 0 || row.items.length > MAX_ITEMS) { state.unknown = true; continue; }
                for (const item of row.items) {
                  if (++activityVisits > MAX_ITEMS) { expired = true; throw new Error(); }
                  if (typeof item?.id !== 'string') { state.unknown = true; continue; }
                  scheduled.add(item.id);
                }
                state.scheduledTasks = scheduled.size;
              } else if (!['turn', 'job', 'subagent'].includes(row?.kind)) state.unknown = true;
            }
          } catch { state.unknown = true; }
        }
      }));
    }
    return result();
  }, timeoutMs, () => { expired = true; return { ...state, unknown: true }; });
}

export async function addWorkspace(ctx, path, fs = { realpath, stat }) {
  if (typeof path !== 'string' || !isAbsolute(path) || !path || /[\x00-\x1f\x7f]/.test(path)) throw new Error('Invalid workspace folder');
  const canonical = await fs.realpath(path);
  if (!isAbsolute(canonical) || !(await fs.stat(canonical)).isDirectory()) throw new Error('Invalid workspace folder');
  const registry = service(ctx, 'workspaceRegistry');
  if (typeof registry?.create !== 'function') throw new Error('Workspace registration unavailable');
  await registry.create(canonical);
}

/** Install ownership listeners synchronously, before importing or booting Harness. */
export function startSupervisor({ input = process.stdin, send, loadRuntime = loadInstalledRuntime, env = process.env,
  inspect = inspectTasks, openWorkspace = addWorkspace, onStopped = () => { process.exitCode = 0; }, onFatal = () => { process.exitCode = 1; } } = {}) {
  if (typeof send !== 'function') throw new Error('Protocol sender required');
  let cancelled = false;
  let fatal = false;
  let runtime;
  let shutdownPromise;
  let inspectPending = false;
  let workspacePending = false;
  let bootPromise;
  const fail = message => {
    if (!fatal) { fatal = true; send({ type: 'fatal', message }); }
    return shutdown();
  };
  const shutdown = () => {
    cancelled = true;
    return shutdownPromise ??= (async () => {
      try {
        // Readiness validation can reject after runProfile returned a live tree.
        // A failed ready packet must still dispose that returned runtime.
        try { await bootPromise; } catch { /* Startup failure is reported by the boot observer. */ }
        if (runtime) {
          await runtime.shutdown.shutdown(0);
          send({ type: 'shutdown-complete' });
        }
      } catch {
        if (!fatal) { fatal = true; send({ type: 'fatal', message: 'Harness shutdown failed' }); }
      } finally {
        input.removeListener('data', onData);
        input.removeListener('end', onEnd);
        input.removeListener('close', onEnd);
        input.removeListener('error', onEnd);
        input.pause?.();
        // Apply final status after Harness shutdown(0) sets its own process.exitCode.
        if (fatal) onFatal(); else onStopped();
      }
    })();
  };
  const handle = async command => {
    if (command.type === 'shutdown') return shutdown();
    if (cancelled) return;
    if (!runtime) {
      if (command.type === 'inspect') send({ type: 'inspection', requestId: command.requestId, activeTasks: 0, scheduledTasks: 0, unknown: true });
      else send({ type: 'request-error', requestId: command.requestId, message: 'Harness is still starting' });
      return;
    }
    if (command.type === 'inspect') {
      if (inspectPending) { send({ type: 'inspection', requestId: command.requestId, activeTasks: 0, scheduledTasks: 0, unknown: true }); return; }
      inspectPending = true;
      try {
        const result = await inspect(runtime.ctx);
        if (!cancelled) send({ type: 'inspection', requestId: command.requestId, ...result });
      } catch {
        if (!cancelled) send({ type: 'inspection', requestId: command.requestId, activeTasks: 0, scheduledTasks: 0, unknown: true });
      } finally { inspectPending = false; }
    } else {
      if (workspacePending) { send({ type: 'request-error', requestId: command.requestId, message: 'Workspace request already pending' }); return; }
      workspacePending = true;
      try {
        await openWorkspace(runtime.ctx, command.path);
        if (!cancelled) send({ type: 'workspace-added', requestId: command.requestId });
      } catch {
        if (!cancelled) send({ type: 'request-error', requestId: command.requestId, message: 'Unable to add workspace folder' });
      } finally { workspacePending = false; }
    }
  };
  const receiver = lineReceiver(line => {
    if (cancelled) return;
    let command;
    try { command = parseCommand(line); } catch { void fail('Invalid native command'); return; }
    void handle(command).catch(() => fail('Native command failed'));
  }, () => { void fail('Native command exceeds size limit'); });
  const onData = chunk => receiver.write(chunk);
  const onEnd = () => { void shutdown(); };
  input.on('data', onData);
  input.on('end', onEnd);
  input.on('close', onEnd);
  input.on('error', onEnd);
  // A microtask defers even injected loaders until ownership listeners and bootPromise exist.
  bootPromise = Promise.resolve().then(async () => {
    if (input.readableEnded || input.destroyed) cancelled = true;
    const installed = await loadRuntime(env);
    runtime = await installed.runProfile({ environment: installed.environment, profile: env.DSH_TAURI_PROFILE || 'web',
      patchFiles: [], args: ['--no-open', '--host', '127.0.0.1', '--port', '0'] });
    if (!cancelled) send({ type: 'ready', url: readyUrl(runtime.ctx), version: installed.version });
  });
  void bootPromise.catch(() => {
    if (cancelled && !runtime) return;
    void fail('Harness startup failed');
  });
  if (input.readableEnded || input.destroyed) void shutdown();
  return { boot: bootPromise, shutdown, get cancelled() { return cancelled; } };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const diagnostics = interceptDiagnostics(process.stdout, process.stderr);
  startSupervisor({ send: packet => {
    if (packet.type === 'ready') {
      const url = new URL(packet.url);
      for (const value of url.searchParams.values()) diagnostics.addSecret(value);
    }
    diagnostics.send(packet);
  } });
}
