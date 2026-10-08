// Full public profile/actual registered Agent; discovery never prepares or resumes a session.
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { createRequire } from 'node:module';
import { randomUUID } from 'node:crypto';
import { writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { startForkSupervisor, loadForkRuntime } from '../core/runtime/fork-supervisor.mjs';
import { interceptDiagnostics } from '../core/runtime/supervisor.mjs';
const output = process.env.DSH_NATIVE_AGENT_EVIDENCE;
if (!output || resolve(output) !== output || process.env.DSH_TAURI_HOME !== join(output, 'harness')) throw new Error('private-output-required');
const diagnostics = interceptDiagnostics(process.stdout, process.stderr);
const input = new EventEmitter(); input.pause = () => {};
const waiting = new Map(); let serial = 0, controller, ctx, handle, removeEvents, phase = 'boot', status = 'failed', fatal = false;
const counts = { turnStart: 0, turnEnd: 0, stepStarts: 0, stepEnds: 0, requestHeaders: 0, toolCalls: 0, toolResults: 0, assistantMessages: 0, unexpectedWake: 0 };
const facts = { absentBefore: false, actualRegisteredObject: false, matchedActualIdentity: false, unrelatedAbsent: false, unchangedByRepeatedRead: false, absentAfterDisposal: false, registryReturnedToBaseline: false };
function command(sessionId) {
  const requestId = `agent-read-${++serial}`;
  const promise = new Promise((resolve, reject) => waiting.set(requestId, { resolve, reject }));
  input.emit('data', `${JSON.stringify({ type: 'discover-session-agent', requestId, sessionId })}\n`);
  return promise;
}
try {
  controller = startForkSupervisor({ input, env: process.env, addSecret: value => diagnostics.addSecret(value),
    onStopped() {}, onFatal() { fatal = true; }, send(packet) {
      if (packet.type === 'fatal') { fatal = true; for (const waiter of waiting.values()) waiter.reject(new Error('native-fatal')); }
      const waiter = waiting.get(packet.requestId);
      if (waiter) { waiting.delete(packet.requestId); if (packet.type === 'native-result') waiter.resolve(packet.value); else waiter.reject(new Error('native-read-failed')); }
    },
    loadRuntime: async env => {
      const runtime = await loadForkRuntime(env); const runProfile = runtime.runProfile;
      return { ...runtime, runProfile: async options => { const loaded = await runProfile(options); ctx = loaded.ctx; return loaded; } };
    },
  });
  await controller.boot; assert.equal(fatal, false);
  const require = createRequire(join(process.env.DSH_TAURI_RUNTIME, 'package.json'));
  const { SessionId } = await import(pathToFileURL(require.resolve('@deepseek-ai/dsh-session')).href);
  const sessionId = SessionId(randomUUID()), unrelated = SessionId(randomUUID());
  const agents = ctx.get('agents'), baseline = agents.list().length;
  phase = 'unregistered-read';
  assert.deepEqual(await command(sessionId), { sessionId, agentId: null }); facts.absentBefore = true;
  assert.equal(agents.list().length, baseline);
  phase = 'create-owned-idle-root';
  handle = await agents.create({ sessionId, meta: { cwd: join(output, 'workspace') }, setup(agentCtx, subject) {
    agentCtx.on('agent/pre-step', ({ agent }, next) => {
      if (agent !== subject) return next(); counts.unexpectedWake++; return { kind: 'reject' };
    }, { prepend: true });
  } });
  removeEvents = ctx.on('session/event', (session, event) => {
    if (session !== handle.agent.session) return;
    const key = { 'turn/start': 'turnStart', 'turn/end': 'turnEnd', 'step/start': 'stepStarts', 'step/end': 'stepEnds', 'request/header': 'requestHeaders',
      'tool/call': 'toolCalls', 'tool/result': 'toolResults', 'assistant/message': 'assistantMessages' }[event.type];
    if (key) counts[key]++;
  });
  facts.actualRegisteredObject = agents.get(handle.agent.id) === handle.agent && agents.roots().includes(handle.agent);
  assert.equal(facts.actualRegisteredObject, true);
  phase = 'registered-read';
  const found = await command(sessionId);
  assert.equal(found.sessionId, handle.agent.session.id); assert.equal(found.agentId, handle.agent.id);
  facts.matchedActualIdentity = true;
  assert.deepEqual(await command(unrelated), { sessionId: unrelated, agentId: null }); facts.unrelatedAbsent = true;
  assert.deepEqual(await command(sessionId), found);
  assert.equal(agents.list().length, baseline + 1); facts.unchangedByRepeatedRead = true;
  phase = 'dispose-owned-root';
  await handle.dispose(); assert.notEqual(agents.get(handle.agent.id), handle.agent);
  assert.deepEqual(await command(sessionId), { sessionId, agentId: null }); facts.absentAfterDisposal = true;
  facts.registryReturnedToBaseline = agents.list().length === baseline; assert.equal(facts.registryReturnedToBaseline, true);
  for (const count of Object.values(counts)) assert.equal(count, 0);
  phase = 'shutdown'; await controller.shutdown(); assert.equal(fatal, false);
  status = 'passed'; phase = 'complete';
} catch (_error) { /* Fixed phase only; no profile/authorization/credential body or source exception printed. */ }
finally {
  try { if (handle) await handle.dispose(); } catch (_error) { status = 'failed'; }
  removeEvents?.();
  try { await controller?.shutdown(); } catch (_error) { status = 'failed'; }
  await writeFile(join(output, 'agent.json'), `${JSON.stringify({ status, phase, fatal, facts, counts,
    scope: 'actual full public alpha profile, AgentRegistry/loop factory and private supervisor read; not Rust framing/GUI or hostile provider testing',
    providerSubstituted: false, discoveryPromptOrResumeRequested: false, accountOrKeyMutationRequested: false, processWideNetworkTrace: false,
    countersScope: 'owned subject events after create returned; pre-step guard installed during setup counts any unexpected wake',
  }, null, 2)}\n`, { mode: 0o600, flag: 'wx' });
  diagnostics.restore(); process.exitCode = status === 'passed' ? 0 : 1;
}
