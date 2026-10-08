// Test-only instance barriers; unchanged public profile, real HTTP/storage/Agent factory, no prompt.
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { createRequire } from 'node:module';
import { randomUUID, createHash } from 'node:crypto';
import { writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { startForkSupervisor, loadForkRuntime } from '../core/runtime/fork-supervisor.mjs';
import { interceptDiagnostics } from '../core/runtime/supervisor.mjs';

const output = process.env.DSH_NATIVE_UPLOAD_UNCERTAINTY_EVIDENCE;
if (!output || resolve(output) !== output || process.env.DSH_TAURI_HOME !== join(output, 'harness')) throw new Error('private-output-required');
const failurePoint = process.argv[2] ?? 'none';
assert.ok(['none', 'ack-held', 'store-held'].includes(failurePoint));
const cleanup = { handlersJoined: false, descriptorsRestored: false, observerDisposersRan: false, ownedHandlesDisposed: false, registryReturnedToBaseline: false, hostShutdown: false, drainTimedOut: false };
const diagnostics = interceptDiagnostics(process.stdout, process.stderr);
const input = new EventEmitter(); input.pause = () => {};
const handles = [], subjects = new Set(), removers = [], restorers = [], gates = [], clients = [], operations = [], bridges = [];
const counts = { turnStarts: 0, turnEnds: 0, stepStarts: 0, stepEnds: 0, requestHeaders: 0, toolCalls: 0, toolResults: 0, assistantMessages: 0, userMessages: 0, unexpectedWake: 0 };
const facts = { authenticatedRawRoute: false, storedBeforeAbort: false, stagedBeforeAbort: false, clientAbortedBeforeAck: false,
  serverObservedDisconnect: false, receiptRetainedAfterAckLoss: false, foreignResolveRefused: false, foreignBindingRefused: false,
  foreignRetirementHarmless: false, rollbackRetainedUnbound: false, rollbackRestoredPrior: false, committedRetirementExact: false,
  oldSessionDisposalRetiredReceipt: false, sameIdReplacementDifferentSession: false, sameIdReplacementRefusedOldReceipt: false,
  storedBeforeAgentDisposal: false, disposedBeforeStorePromiseReturned: false, lateDisposalRejectedNoReceipt: false,
  handlersJoined: false, descriptorsRestored: false, registryReturnedToBaseline: false };
const files = [];
let controller, ctx, launch, uploads, attachments, originalSymbol, phase = 'boot', status = 'failed', fatal = false;
let baseline = 0, bridgeActive = 0, bridgeComplete = 0, bridgeErrors = 0, methodActive = 0, injectionReached = false;
const tracks = new Map();
function gate() {
  const entered = Promise.withResolvers(), released = Promise.withResolvers();
  const result = { entered: entered.promise, release: () => released.resolve(), async hold(value) { entered.resolve(value); await released.promise; } };
  gates.push(result); return result;
}
async function bounded(promise, milliseconds = 6000) {
  let timer;
  try { return await Promise.race([promise, new Promise((_resolve, reject) => { timer = setTimeout(() => reject(new Error('owned-phase-timeout')), milliseconds); })]); }
  finally { clearTimeout(timer); }
}
function abortObserved(signal) {
  assert.ok(signal instanceof AbortSignal);
  if (signal.aborted) return Promise.resolve();
  return new Promise(done => signal.addEventListener('abort', done, { once: true }));
}
function patch(service, key, wrapper) {
  const descriptor = Object.getOwnPropertyDescriptor(service, key), original = service[key];
  assert.equal(typeof original, 'function');
  Object.defineProperty(service, key, { configurable: true, writable: true, value: wrapper(original) });
  restorers.push(() => {
    if (descriptor) Object.defineProperty(service, key, descriptor); else assert.equal(Reflect.deleteProperty(service, key), true);
    assert.equal(service[key], original);
    assert.deepEqual(Object.getOwnPropertyDescriptor(service, key), descriptor);
  });
}
function fixture(caseName) {
  const [count, step] = { 'ack-loss': [5003, 43], scope: [6117, 71], 'late-disposal': [7109, 29] }[caseName];
  return Buffer.from(Array.from({ length: count }, (_, index) => (index * step + 19) % 256));
}
function recordFile(caseName, ref) {
  const bytes = fixture(caseName);
  assert.equal(ref.attachmentId, `sha256:${createHash('sha256').update(bytes).digest('hex')}`);
  assert.equal(ref.bytes, bytes.length); assert.equal(ref.name, `PUBLIC ${caseName}.bin`);
  files.push({ case: caseName, file: ref });
}
function setup(agentCtx, subject) {
  subjects.add(subject.session);
  agentCtx.on('agent/pre-step', ({ agent }, next) => {
    if (agent !== subject) return next(); counts.unexpectedWake++; return { kind: 'reject' };
  }, { prepend: true });
}
async function fetchPhase(sessionId, caseName, cookie, origin) {
  const url = new URL('/api/session/uploadFileBinary', origin);
  url.searchParams.set('sessionId', sessionId); url.searchParams.set('name', `PUBLIC ${caseName}.bin`);
  const done = Promise.withResolvers();
  const track = { done: done.promise, finish: done.resolve, seen: 0 };
  tracks.set(url.pathname + url.search, track); bridges.push(track);
  const client = new AbortController(); clients.push(client);
  const operation = fetch(url, { method: 'POST', redirect: 'manual', headers: {
    Cookie: cookie, Origin: origin, 'Content-Type': 'application/octet-stream',
  }, body: fixture(caseName), signal: client.signal }).then(async response => {
    assert.equal(response.status, 200); assert.equal(response.headers.get('cache-control'), 'no-store');
    assert.ok(response.headers.get('content-type')?.startsWith('application/json'));
    const bytes = Buffer.from(await response.arrayBuffer()); assert.ok(bytes.length <= 16384);
    return { kind: 'reply', value: JSON.parse(bytes) };
  }, error => ({ kind: 'failed', aborted: error.name === 'AbortError' }));
  // Every operation is rejection-observed immediately, including decoding/assertion failures.
  const settled = operation.then(value => value, () => ({ kind: 'failed', aborted: false }));
  operations.push(settled); return { client, result: settled, track };
}
try {
  controller = startForkSupervisor({ input, env: process.env, addSecret: value => diagnostics.addSecret(value), onStopped() {},
    onFatal() { fatal = true; }, send(packet) { if (packet.type === 'ready') launch = packet.url; if (packet.type === 'fatal') fatal = true; },
    loadRuntime: async env => {
      const runtime = await loadForkRuntime(env), runProfile = runtime.runProfile;
      return { ...runtime, runProfile: async options => { const loaded = await runProfile(options); ctx = loaded.ctx; return loaded; } };
    },
  });
  await bounded(controller.boot, 15000); assert.equal(fatal, false);
  const require = createRequire(join(process.env.DSH_TAURI_RUNTIME, 'package.json'));
  const { SessionId } = await import(pathToFileURL(require.resolve('@deepseek-ai/dsh-session')).href);
  const { symbols } = await import(pathToFileURL(require.resolve('@deepseek-ai/cordis')).href);
  originalSymbol = symbols.original;
  const concrete = service => service[originalSymbol] ?? service;
  uploads = concrete(ctx.get('fileUploads')); attachments = concrete(ctx.get('attachments'));
  const agents = ctx.get('agents'); baseline = agents.list().length;
  removers.push(ctx.on('session/event', (session, event) => {
    if (!subjects.has(session)) return;
    const key = { 'turn/start': 'turnStarts', 'turn/end': 'turnEnds', 'step/start': 'stepStarts', 'step/end': 'stepEnds',
      'request/header': 'requestHeaders', 'tool/call': 'toolCalls', 'tool/result': 'toolResults',
      'assistant/message': 'assistantMessages', 'user/message': 'userMessages' }[event.type];
    if (key) counts[key]++;
  }));
  removers.push(ctx.on('connection/request', async (request, _response, next) => {
    const track = tracks.get(request.url);
    if (!track) return next();
    track.seen++; bridgeActive++;
    try { await next(); } catch (error) { bridgeErrors++; throw error; }
    finally { bridgeActive--; bridgeComplete++; track.finish(); }
  }, { prepend: true }));
  const first = await agents.create({ sessionId: SessionId(randomUUID()), meta: { cwd: join(output, 'workspace') }, setup }); handles.push(first);
  const foreign = await agents.create({ sessionId: SessionId(randomUUID()), meta: { cwd: join(output, 'workspace') }, setup }); handles.push(foreign);
  assert.equal(agents.get(first.agent.id), first.agent); assert.equal(agents.get(foreign.agent.id), foreign.agent);
  assert.notEqual(first.agent.session, foreign.agent.session);
  phase = 'private-authentication';
  const origin = new URL(launch).origin;
  assert.equal(new URL(origin).hostname, '127.0.0.1');
  const exchange = await fetch(launch, { redirect: 'manual' });
  assert.equal(exchange.status, 303); assert.equal(exchange.headers.get('location'), './');
  const cookie = exchange.headers.get('set-cookie')?.split(';', 1)[0]; assert.ok(cookie);
  diagnostics.addSecret(cookie); await exchange.arrayBuffer(); launch = undefined;
  const ack = gate(), store = gate();
  patch(uploads, 'uploadStream', original => async function(request) {
    methodActive++;
    try {
      const value = await Reflect.apply(original, this, [request]);
      if (request.name === 'PUBLIC ack-loss.bin') {
        const disconnected = abortObserved(request.signal);
        await ack.hold({ value, signal: request.signal, disconnected });
      }
      return value;
    } finally { methodActive--; }
  });
  patch(attachments, 'saveFileStream', original => async function(request) {
    methodActive++;
    try {
      const value = await Reflect.apply(original, this, [request]);
      if (request.name === 'PUBLIC late-disposal.bin') await store.hold(value);
      return value;
    } finally { methodActive--; }
  });
  phase = 'ack-lost-after-real-staging';
  const lost = await fetchPhase(first.agent.session.id, 'ack-loss', cookie, origin);
  const committed = await bounded(ack.entered);
  recordFile('ack-loss', committed.value.file); facts.storedBeforeAbort = true;
  assert.deepEqual(uploads.resolve(first.agent, committed.value.receiptId), committed.value.file); facts.stagedBeforeAbort = true;
  lost.client.abort();
  assert.deepEqual(await bounded(lost.result), { kind: 'failed', aborted: true }); facts.clientAbortedBeforeAck = true;
  await bounded(committed.disconnected); assert.equal(committed.signal.aborted, true); facts.serverObservedDisconnect = true;
  if (failurePoint === 'ack-held') { injectionReached = true; throw new Error('intentional-owned-assertion-failure'); }
  ack.release(); await bounded(lost.track.done); assert.equal(lost.track.seen, 1);
  assert.deepEqual(uploads.resolve(first.agent, committed.value.receiptId), committed.value.file); facts.receiptRetainedAfterAckLoss = true;
  facts.authenticatedRawRoute = true;
  phase = 'actual-session-receipt-binding-no-prompt';
  const receipt = committed.value.receiptId;
  assert.equal(uploads.resolve(foreign.agent, receipt), undefined); facts.foreignResolveRefused = true;
  assert.throws(() => uploads.bindPrompt(foreign.agent, [receipt], 'PUBLIC_foreign'), error => error.code === 'session/attachment-invalid' && error.details.reason === 'FILE_NOT_STAGED'); facts.foreignBindingRefused = true;
  uploads.retirePrompt(foreign.agent, 'PUBLIC_foreign'); assert.ok(uploads.resolve(first.agent, receipt)); facts.foreignRetirementHarmless = true;
  const unbound = uploads.bindPrompt(first.agent, [receipt], 'PUBLIC_unbound'); unbound[Symbol.dispose]();
  uploads.retirePrompt(first.agent, 'PUBLIC_unbound'); assert.ok(uploads.resolve(first.agent, receipt)); facts.rollbackRetainedUnbound = true;
  const prior = uploads.bindPrompt(first.agent, [receipt], 'PUBLIC_prior'); prior.commit(); prior[Symbol.dispose]();
  const temporary = uploads.bindPrompt(first.agent, [receipt], 'PUBLIC_temporary'); temporary[Symbol.dispose]();
  uploads.retirePrompt(first.agent, 'PUBLIC_temporary'); assert.ok(uploads.resolve(first.agent, receipt)); facts.rollbackRestoredPrior = true;
  uploads.retirePrompt(first.agent, 'PUBLIC_prior'); assert.equal(uploads.resolve(first.agent, receipt), undefined); facts.committedRetirementExact = true;
  const normal = await fetchPhase(first.agent.session.id, 'scope', cookie, origin);
  const normalResult = await bounded(normal.result); await bounded(normal.track.done);
  assert.equal(normalResult.kind, 'reply'); assert.equal(normalResult.value.ok, true);
  const scopeValue = normalResult.value.value; recordFile('scope', scopeValue.file);
  assert.ok(uploads.resolve(first.agent, scopeValue.receiptId));
  phase = 'dispose-resume-same-id-exact-session-scope';
  const previousSession = first.agent.session, previousId = previousSession.id;
  await first.dispose(); assert.equal(agents.get(first.agent.id), undefined);
  assert.equal(uploads.resolve(first.agent, scopeValue.receiptId), undefined); facts.oldSessionDisposalRetiredReceipt = true;
  const replacement = await agents.resume({ resumeSessionId: previousId, setup }); handles.push(replacement);
  assert.equal(replacement.agent.session.id, previousId); assert.notEqual(replacement.agent.session, previousSession);
  assert.equal(agents.get(replacement.agent.id), replacement.agent); facts.sameIdReplacementDifferentSession = true;
  assert.equal(uploads.resolve(replacement.agent, scopeValue.receiptId), undefined); facts.sameIdReplacementRefusedOldReceipt = true;
  phase = 'agent-disposed-after-real-save-before-upload-commit';
  const late = await fetchPhase(foreign.agent.session.id, 'late-disposal', cookie, origin);
  const persisted = await bounded(store.entered); recordFile('late-disposal', persisted); facts.storedBeforeAgentDisposal = true;
  if (failurePoint === 'store-held') { injectionReached = true; throw new Error('intentional-owned-assertion-failure'); }
  await foreign.dispose(); assert.equal(agents.get(foreign.agent.id), undefined); facts.disposedBeforeStorePromiseReturned = true;
  store.release();
  const rejected = await bounded(late.result); await bounded(late.track.done);
  assert.equal(rejected.kind, 'reply'); assert.equal(rejected.value.ok, false);
  assert.equal(rejected.value.error.code, 'session/not-found'); assert.equal(rejected.value.error.details.sessionId, foreign.agent.id);
  assert.equal('value' in rejected.value, false); facts.lateDisposalRejectedNoReceipt = true;
  phase = 'joined-restoration-and-shutdown';
  await Promise.all(operations); await bounded(Promise.all(bridges.map(item => item.done)));
  assert.equal(bridgeActive, 0); assert.equal(bridgeComplete, 3); assert.equal(bridgeErrors, 0); assert.equal(methodActive, 0);
  facts.handlersJoined = true;
  for (const handle of handles) await bounded(handle.dispose());
  assert.equal(agents.list().length, baseline); facts.registryReturnedToBaseline = true;
  for (const count of Object.values(counts)) assert.equal(count, 0);
  status = 'passed'; phase = 'complete';
} catch (_error) { /* Fixed phase only; no private request, cookie, paths or error text are published. */ }
finally {
  for (const client of clients) client.abort();
  for (const hold of gates) hold.release();
  try {
    await bounded(Promise.all(operations)); await bounded(Promise.all(bridges.filter(item => item.seen > 0).map(item => item.done)));
    cleanup.handlersJoined = bridgeActive === 0 && methodActive === 0;
    if (!cleanup.handlersJoined) status = 'failed';
  } catch (_error) { cleanup.drainTimedOut = true; status = 'failed'; }
  cleanup.descriptorsRestored = true;
  for (const restore of restorers) { try { restore(); } catch (_error) { cleanup.descriptorsRestored = false; status = 'failed'; } }
  cleanup.observerDisposersRan = true;
  while (removers.length) { try { removers.pop()(); } catch (_error) { cleanup.observerDisposersRan = false; status = 'failed'; } }
  cleanup.ownedHandlesDisposed = true;
  for (const handle of handles) { try { await bounded(handle.dispose()); } catch (_error) { cleanup.ownedHandlesDisposed = false; status = 'failed'; } }
  try { cleanup.registryReturnedToBaseline = !!ctx && ctx.get('agents').list().length === baseline; }
  catch (_error) { cleanup.registryReturnedToBaseline = false; }
  if (!cleanup.registryReturnedToBaseline) status = 'failed';
  try { if (controller) { await bounded(controller.shutdown()); cleanup.hostShutdown = controller.cancelled; } }
  catch (_error) { status = 'failed'; }
  if (!cleanup.hostShutdown || fatal) status = 'failed';
  facts.descriptorsRestored = cleanup.descriptorsRestored;
  if (status === 'passed' && !Object.values(facts).every(value => value === true)) status = 'failed';
  try { await writeFile(join(output, 'barriers.json'), `${JSON.stringify({ status, phase, fatal, facts, counts, files, cleanup, failurePoint, injectionReached,
    bridge: { active: bridgeActive, complete: bridgeComplete, errors: bridgeErrors, methodActive },
    scriptedModelPromptOrCatalog: false, scriptedSignInOrCredentialSave: false, providerSubstituted: false,
    testOnlyInstanceBarriers: true, processWideNetworkTrace: false, rustSdkExercised: false, uiExercised: false,
    scope: 'actual public alpha Loader/HTTP/default storage/Agent factory; test-only post-real-save/staging gates and direct receipt transaction calls, no prompt delivery',
  }, null, 2)}\n`, { mode: 0o600, flag: 'wx' }); }
  finally { diagnostics.restore(); process.exitCode = status === 'passed' ? 0 : 1; }
}
