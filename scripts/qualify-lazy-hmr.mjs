/** Real supported alpha base+web/native supervisor; authenticated loopback SSE consumers, no artifact mutation or model request. */
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import fs from 'node:fs';
import { chmod, lstat, mkdir, readFile, readdir, realpath, writeFile } from 'node:fs/promises';
import http from 'node:http';
import { createHash } from 'node:crypto';
import { createRequire, syncBuiltinESMExports } from 'node:module';
import { join, resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { startForkSupervisor, loadForkRuntime } from '../core/runtime/fork-supervisor.mjs';
import { interceptDiagnostics } from '../core/runtime/supervisor.mjs';

const output = process.env.DSH_NATIVE_HMR_EVIDENCE;
const stage = process.env.DSH_NATIVE_HMR_STAGE;
assert.ok(output && resolve(output) === output && process.env.DSH_TAURI_HOME === join(output, 'harness'), 'private-output-required');
assert.ok(['baseline', 'after'].includes(stage), 'explicit-stage-required');
assert.ok(process.env.DSH_TAURI_RUNTIME && resolve(process.env.DSH_TAURI_RUNTIME) === process.env.DSH_TAURI_RUNTIME, 'explicit-runtime-required');
assert.equal(process.env.DSH_TAURI_EXPECTED_VERSION, '0.2.1-alpha.1', 'exact-alpha-required');
assert.equal(process.platform, 'linux', 'linux-required');
for (const key of Object.keys(process.env)) assert.ok(!/API_?KEY|AUTH|TOKEN|PASSWORD|SECRET|NODE_OPTIONS/i.test(key), 'credential-free-environment-required');
await mkdir(output, { recursive: true, mode: 0o700 });
assert.equal(await realpath(output), output, 'canonical-output-required');
assert.ok(!(await lstat(output)).isSymbolicLink(), 'canonical-output-required');
await chmod(output, 0o700);
try { await lstat(join(output, 'harness')); throw new Error('fresh-home-required'); }
catch (error) { assert.equal(error.code, 'ENOENT', 'fresh-home-required'); }

const writes = { stdout: process.stdout.write, stderr: process.stderr.write };
// Keep even redacted readiness origins and third-party diagnostic text out of this driver output.
const privateDiagnosticSink = (_chunk, encoding, callback) => {
  const done = typeof encoding === 'function' ? encoding : callback;
  if (done) queueMicrotask(done);
  return true;
};
process.stdout.write = privateDiagnosticSink;
process.stderr.write = privateDiagnosticSink;
const diagnostics = interceptDiagnostics(process.stdout, process.stderr);
const report = {
  status: 'failed', stage, phase: 'boot', runtimeVersion: null, hmrArtifactSha256: null,
  collectorSha256: createHash('sha256').update(await readFile(new URL(import.meta.url))).digest('hex'),
  scope: 'actual built alpha supported runProfile/resolvedProfile, unchanged base+web bundles and source native supervisor; scripted loopback SSE, no rendered browser/Rust/UI',
  statScope: 'exact artifactBaseline paths from the actual loaded ClientModuleRegistry; HMR caller/callback-attributed statSync only, not root filesystem or all Host activity',
  metricScope: 'whole current Node process (profile + native helper + this instrumentation) and Linux /proc descendants observed through its task children; RSS sum double-counts shared pages, not PSS; no exited-child CPU or escaped/unobserved descendants',
  metricOverhead: 'real delegate instrumentation and boundary /proc reads included; no continuous peak sampling, forced GC or UI overall claim',
  snapshots: {}, windows: {}, checks: {}, fatal: false, stopped: false,
  externalFetches: 0, modelRequests: 0, scriptedSignIn: false, scriptedCredentialSave: false,
  artifactMutation: false, realBrowserRendered: false, processWideNetworkTrace: false,
};
const original = {
  statSync: fs.statSync, setInterval: globalThis.setInterval, clearInterval: globalThis.clearInterval,
  writeHead: http.ServerResponse.prototype.writeHead, fetch: globalThis.fetch,
};
const artifactPaths = new Set();
const hmrTimers = new Set();
const serverConsumers = new Set();
const clients = new Set();
const counters = { artifactStats: 0, hmrArtifactStats: 0, pollerTicks: 0, timerStarts: 0, timerStops: 0,
  graphs: 0, rebuiltFrames: 0, serverConsumersOpened: 0, serverConsumersClosed: 0 };
let runtime, controller, hmrPath, ready, cookie, afterBoot = false, inHmrTick = false, consumerWake;
let removeGraphListener, removeRebuiltListener;
const hostEvents = { graphChanges: 0, rebuilt: 0 };
const input = new EventEmitter(); input.pause = () => {};
function hmrCaller() { return hmrPath !== undefined && new Error().stack?.includes(hmrPath) === true; }
fs.statSync = function (...args) {
  if (afterBoot && artifactPaths.has(args[0])) {
    counters.artifactStats++;
    if (inHmrTick || hmrCaller()) counters.hmrArtifactStats++;
  }
  return Reflect.apply(original.statSync, this, args);
};
syncBuiltinESMExports();
globalThis.setInterval = function (callback, milliseconds, ...args) {
  const hmr = hmrCaller();
  const delegated = hmr ? function (...parameters) {
    counters.pollerTicks++;
    const previous = inHmrTick; inHmrTick = true;
    try { return Reflect.apply(callback, this, parameters); }
    finally { inHmrTick = previous; }
  } : callback;
  const timer = Reflect.apply(original.setInterval, this, [delegated, milliseconds, ...args]);
  if (hmr) { counters.timerStarts++; hmrTimers.add(timer); }
  return timer;
};
globalThis.clearInterval = function (timer) {
  if (hmrTimers.delete(timer)) counters.timerStops++;
  return Reflect.apply(original.clearInterval, this, [timer]);
};
http.ServerResponse.prototype.writeHead = function (...args) {
  if (cookie && this.req.method === 'GET' && this.req.url === '/plugins/events' && this.req.headers.cookie === cookie && args[0] === 200) {
    serverConsumers.add(this); counters.serverConsumersOpened++;
    this.once('close', () => {
      if (serverConsumers.delete(this)) counters.serverConsumersClosed++;
      if (serverConsumers.size === 0 && consumerWake) { const wake = consumerWake; consumerWake = undefined; wake(); }
    });
  }
  return Reflect.apply(original.writeHead, this, args);
};
globalThis.fetch = async (request, options) => {
  const url = new URL(typeof request === 'string' ? request : request instanceof URL ? request.href : request.url);
  if (url.hostname !== '127.0.0.1') { report.externalFetches++; throw new Error('external-network-denied'); }
  return original.fetch(request, options);
};

async function bounded(promise, milliseconds = 15_000) {
  let timer;
  try { return await Promise.race([promise, new Promise((_, reject) => { timer = setTimeout(() => reject(new Error('fixed-deadline')), milliseconds); })]); }
  finally { clearTimeout(timer); }
}
async function processIdentity(pid) {
  const text = await readFile(`/proc/${pid}/stat`, 'utf8');
  const fields = text.slice(text.lastIndexOf(')') + 2).trim().split(/\s+/);
  return { pid, parentPid: Number(fields[1]), startTimeTicks: fields[19], cpuUserTicks: Number(fields[11]), cpuSystemTicks: Number(fields[12]) };
}
async function processes() {
  const queue = [{ pid: process.pid, parentPid: null }]; const found = new Map(); let missed = 0;
  while (queue.length) {
    const { pid, parentPid } = queue.shift();
    if (found.has(pid)) continue;
    try {
      const before = await processIdentity(pid);
      if (parentPid !== null) {
        const parent = await processIdentity(parentPid);
        if (before.parentPid !== parentPid || parent.startTimeTicks !== found.get(parentPid)?.startTimeTicks) { missed++; continue; }
      }
      const status = await readFile(`/proc/${pid}/status`, 'utf8');
      const rssKiB = Number(status.match(/^VmRSS:\s+(\d+)\s+kB$/m)?.[1] ?? 0);
      const after = await processIdentity(pid);
      if (before.startTimeTicks !== after.startTimeTicks || before.parentPid !== after.parentPid) { missed++; continue; }
      found.set(pid, { ...after, rssBytes: rssKiB * 1024 });
      for (const task of await readdir(`/proc/${pid}/task`)) {
        try {
          const children = await readFile(`/proc/${pid}/task/${task}/children`, 'utf8');
          for (const child of children.trim().split(/\s+/)) if (/^\d+$/.test(child)) queue.push({ pid: Number(child), parentPid: pid });
        } catch (_error) { missed++; }
      }
    } catch (_error) { missed++; }
  }
  return { rootPid: process.pid, processes: [...found.values()], observedProcesses: found.size,
    observedTreeRssBytes: [...found.values()].reduce((total, entry) => total + entry.rssBytes, 0), missedRacingReads: missed };
}
async function capture() {
  return { ...counters, ...hostEvents, liveHmrTimers: hmrTimers.size, actualServerConsumers: serverConsumers.size,
    heap: process.memoryUsage(), processTree: await processes() };
}
async function window(name, milliseconds) {
  const before = await capture();
  const cpuBefore = process.cpuUsage(); const start = process.hrtime.bigint();
  await delay(milliseconds);
  const elapsedMs = Number(process.hrtime.bigint() - start) / 1_000_000;
  const cpu = process.cpuUsage(cpuBefore);
  const after = await capture();
  const deltas = Object.fromEntries(Object.keys(counters).map(key => [key, after[key] - before[key]]));
  report.windows[name] = { requestedMs: milliseconds, elapsedMs, currentNodeCpuUserUs: cpu.user,
    currentNodeCpuSystemUs: cpu.system, currentNodeCpuPercentOneCore: (cpu.user + cpu.system) / (elapsedMs * 10),
    counters: deltas, before, after };
  return report.windows[name];
}
function ownedUrl(url) {
  const result = new URL(url);
  assert.equal(result.protocol, 'http:'); assert.equal(result.hostname, '127.0.0.1');
  assert.ok(result.port && Number(result.port) > 0); return result;
}
async function requestOnce(url, headers = {}, method = 'GET') {
  const destination = ownedUrl(url);
  return bounded(new Promise((done, reject) => {
    const request = http.request(destination, { headers, method, agent: false }, response => {
      const status = response.statusCode;
      const responseHeaders = response.headers;
      response.resume(); response.once('end', () => done({ status, headers: responseHeaders }));
      response.once('error', () => reject(new Error('loopback-response-failed')));
    });
    request.once('error', () => reject(new Error('loopback-request-failed')));
    request.setTimeout(10_000, () => request.destroy(new Error('loopback-deadline')));
    request.end();
  }));
}
async function browser() {
  const url = ownedUrl(new URL('/plugins/events', ready).href);
  let response;
  const request = http.request(url, { method: 'GET', agent: false,
    headers: { cookie, accept: 'text/event-stream', 'user-agent': 'PUBLIC-HMR-qualification-SSE-consumer' } });
  clients.add(request);
  const opening = new Promise((done, reject) => {
    request.once('response', incoming => {
      response = incoming;
      if (incoming.statusCode !== 200 || !incoming.headers['content-type']?.startsWith('text/event-stream')) { reject(new Error('sse-refused')); return; }
      let pending = ''; let opened = false;
      incoming.setEncoding('utf8');
      incoming.on('data', text => {
        pending += text;
        if (pending.length > 4 * 1024 * 1024) { reject(new Error('sse-bound')); request.destroy(); return; }
        let at;
        while ((at = pending.indexOf('\n\n')) >= 0) {
          const frame = pending.slice(0, at); pending = pending.slice(at + 2);
          if (!frame.startsWith('data: ')) continue;
          try {
            const value = JSON.parse(frame.slice(6));
            if (value.type === 'rebuilt') counters.rebuiltFrames++;
            if (value.type !== 'graph') continue;
            counters.graphs++;
            if (!opened) { opened = true; done(value.graph); }
          } catch (_error) { reject(new Error('sse-invalid')); }
        }
      });
      incoming.once('error', () => { if (!opened) reject(new Error('sse-response-failed')); });
      incoming.once('end', () => { if (!opened) reject(new Error('sse-ended')); });
    });
    request.once('error', () => reject(new Error('sse-request-failed')));
  });
  request.end();
  const graph = await bounded(opening);
  const close = () => { response?.destroy(); request.destroy(); clients.delete(request); };
  return { graph, close };
}
function graphCurrent(graph) {
  assert.deepEqual(graph, runtime.ctx.get('clientModules').graph());
  for (const row of graph.entries) {
    const baseline = runtime.ctx.get('clientModules').artifactBaseline(row.id);
    if (!baseline) continue;
    const stat = original.statSync(baseline.path);
    for (const field of ['mtimeMs', 'ctimeMs', 'size']) assert.equal(baseline[field], stat[field]);
  }
  return true;
}
try {
  controller = startForkSupervisor({ input, env: process.env,
    send(packet) {
      if (packet.type === 'ready') { ready = ownedUrl(packet.url).href; report.runtimeVersion = packet.version; }
      if (packet.type === 'fatal') report.fatal = true;
      if (packet.type === 'shutdown-complete') report.stopped = true;
    },
    addSecret: value => diagnostics.addSecret(value), onStopped() {}, onFatal() { report.fatal = true; },
    loadRuntime: async env => {
      const installed = await loadForkRuntime(env);
      const require = createRequire(join(env.DSH_TAURI_RUNTIME, 'package.json'));
      const webRequire = createRequire(require.resolve('@deepseek-ai/dsh-web-app/package.json'));
      hmrPath = await realpath(webRequire.resolve('@deepseek-ai/dsh-client-hmr'));
      report.hmrArtifactSha256 = createHash('sha256').update(await readFile(hmrPath)).digest('hex');
      const run = installed.runProfile;
      return { ...installed, runProfile: async options => {
        runtime = await run(options);
        const modules = runtime.ctx.get('clientModules'); assert.ok(modules);
        for (const row of modules.graph().entries) {
          const baseline = modules.artifactBaseline(row.id);
          if (baseline) artifactPaths.add(baseline.path);
        }
        const llm = runtime.ctx.get('llm'); assert.ok(llm);
        const stream = llm.stream;
        llm.stream = function (...args) { report.modelRequests++; return Reflect.apply(stream, this, args); };
        removeGraphListener = modules.onGraphChanged(() => { hostEvents.graphChanges++; });
        removeRebuiltListener = modules.onRebuilt(() => { hostEvents.rebuilt++; });
        afterBoot = true;
        return runtime;
      } };
    } });
  await bounded(controller.boot, 30_000);
  assert.ok(ready && !report.fatal); assert.equal(report.runtimeVersion, '0.2.1-alpha.1');
  report.artifactPathCount = artifactPaths.size; assert.ok(artifactPaths.size > 0);
  report.snapshots.ready = await capture();
  report.phase = 'zero-browser-idle';
  const idle = await window('zeroBrowserIdle', 5_000);
  report.checks.zeroBrowserIdleArtifactStatsAbsent = idle.counters.hmrArtifactStats === 0;
  report.checks.zeroBrowserIdleTimerAbsent = idle.after.liveHmrTimers === 0;
  assert.equal(idle.after.actualServerConsumers, 0);
  if (stage === 'after') { assert.equal(idle.counters.hmrArtifactStats, 0); assert.equal(idle.after.liveHmrTimers, 0); }
  else { assert.ok(idle.counters.hmrArtifactStats > 0); assert.equal(idle.after.liveHmrTimers, 1); }

  report.phase = 'browser-cookie-handshake';
  const handshake = await requestOnce(ready);
  assert.equal(handshake.status, 303);
  cookie = handshake.headers['set-cookie']?.find(value => value.startsWith('dsh-auth-'))?.split(';')[0];
  assert.ok(cookie); diagnostics.addSecret(cookie); diagnostics.addSecret(cookie.slice(cookie.indexOf('=') + 1));
  report.checks.browserCookieHandshake = true;
  report.phase = 'first-browser';
  const first = await browser();
  report.checks.firstGraphCurrentAndMetadataMatches = graphCurrent(first.graph);
  report.snapshots.firstBrowser = await capture();
  assert.equal(hmrTimers.size, 1); assert.equal(serverConsumers.size, 1);
  const one = await window('oneBrowser', 1_000); assert.ok(one.counters.hmrArtifactStats > 0);
  report.phase = 'second-browser';
  const timerStarts = counters.timerStarts;
  const second = await browser();
  report.checks.secondGraphCurrentAndMetadataMatches = graphCurrent(second.graph);
  report.checks.secondConsumerSharesPoller = counters.timerStarts === timerStarts && hmrTimers.size === 1;
  assert.ok(report.checks.secondConsumerSharesPoller); assert.equal(serverConsumers.size, 2);
  await window('twoBrowsers', 1_000);
  report.phase = 'last-browser-close';
  first.close(); second.close();
  if (serverConsumers.size > 0) await bounded(new Promise(done => { consumerWake = done; }));
  report.snapshots.lastDisconnect = await capture();
  report.phase = 'zero-browser-idle-after-close';
  const closed = await window('zeroBrowserAfterClose', 5_000);
  report.checks.lastCloseArtifactStatsAbsent = closed.counters.hmrArtifactStats === 0;
  report.checks.lastCloseTimerAbsent = closed.after.liveHmrTimers === 0;
  if (stage === 'after') { assert.equal(closed.counters.hmrArtifactStats, 0); assert.equal(closed.after.liveHmrTimers, 0); }
  else { assert.ok(closed.counters.hmrArtifactStats > 0); assert.equal(closed.after.liveHmrTimers, 1); }
  const graph = runtime.ctx.get('clientModules').graph();
  const resource = graph.batches[0]?.url ?? graph.entries[0]?.url;
  assert.ok(resource);
  const artifact = await requestOnce(new URL(resource, ready).href, { cookie }, 'HEAD');
  report.checks.artifactAvailableWithoutConsumer = artifact.status === 200; assert.ok(report.checks.artifactAvailableWithoutConsumer);
  report.phase = 'shutdown';
  await bounded(controller.shutdown(), 15_000);
  assert.equal(hmrTimers.size, 0); assert.equal(report.fatal, false); assert.equal(report.stopped, true);
  assert.equal(report.modelRequests, 0); assert.equal(report.externalFetches, 0);
  report.snapshots.stopped = await capture();
  report.status = 'passed'; report.phase = 'complete';
} catch (_error) {
  // Fixed phase only: do not publish exception strings, paths, readiness URL, cookies, profile graph or raw diagnostics.
} finally {
  for (const client of clients) client.destroy(); clients.clear();
  try { await bounded(controller?.shutdown() ?? Promise.resolve(), 15_000); }
  catch (_error) { report.status = 'failed'; }
  removeGraphListener?.(); removeRebuiltListener?.();
  fs.statSync = original.statSync; syncBuiltinESMExports();
  globalThis.setInterval = original.setInterval; globalThis.clearInterval = original.clearInterval;
  http.ServerResponse.prototype.writeHead = original.writeHead; globalThis.fetch = original.fetch;
  await writeFile(join(output, 'hmr.json'), JSON.stringify(report, null, 2) + '\n', { mode: 0o600, flag: 'wx' });
  diagnostics.restore();
  process.stdout.write = writes.stdout; process.stderr.write = writes.stderr;
  process.exitCode = report.status === 'passed' ? 0 : 1;
}
