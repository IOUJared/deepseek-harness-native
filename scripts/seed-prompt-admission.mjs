/** PUBLIC cold-history fixture through the built supported native profile and real persistence writer. */
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { EventEmitter } from 'node:events';
import { createRequire } from 'node:module';
import { mkdir, realpath, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { startForkSupervisor, loadForkRuntime } from '../core/runtime/fork-supervisor.mjs';
import { interceptDiagnostics } from '../core/runtime/supervisor.mjs';

const output = process.env.DSH_NATIVE_PROMPT_EVIDENCE;
if (!output || resolve(output) !== output || process.env.DSH_TAURI_HOME !== join(output, 'harness')) throw new Error('private-output-required');
assert.equal(await realpath(output), output);
assert.equal(process.cwd(), join(output, 'workspace'));
const diagnostics = interceptDiagnostics(process.stdout, process.stderr);
const report = { status: 'failed', phase: 'boot', scope: 'built full native profile, real persistence create/append/flush; PUBLIC history only',
  sessionId: 'PUBLIC-prompt-admission-cold', requestId: 'PUBLIC-original-request', composedBoots: 0,
  generationRequests: 0, externalFetches: 0, targetAgentAbsentBeforeAndAfter: false, seededEvents: 0, stopped: false };
const originalFetch = globalThis.fetch;
globalThis.fetch = async (input, init) => {
  const url = new URL(typeof input === 'string' ? input : input instanceof URL ? input.href : input.url);
  if (!['127.0.0.1', 'localhost', '[::1]'].includes(url.hostname)) {
    report.externalFetches++;
    throw new Error('PUBLIC external network denied');
  }
  return originalFetch(input, init);
};
let controller;
let fatal = false;
try {
  const input = new EventEmitter();
  input.pause = () => {};
  controller = startForkSupervisor({ input, env: process.env,
    addSecret: value => diagnostics.addSecret(value),
    send: packet => {
      if (packet.type === 'fatal') fatal = true;
      if (packet.type === 'ready') {
        diagnostics.addSecret(packet.url);
        report.runtimeVersion = packet.version;
      }
    },
    loadRuntime: async env => {
      const installed = await loadForkRuntime(env);
      const require = createRequire(join(env.DSH_TAURI_RUNTIME, 'package.json'));
      const baseRequire = createRequire(require.resolve('@deepseek-ai/dsh-base/package.json'));
      const llm = await import(pathToFileURL(baseRequire.resolve('@deepseek-ai/dsh-llm')).href);
      const sessions = await import(pathToFileURL(baseRequire.resolve('@deepseek-ai/dsh-session')).href);
      const run = installed.runProfile;
      return { ...installed, runProfile: async options => {
        const runtime = await run(options);
        report.composedBoots++;
        const ctx = runtime.ctx;
        const model = ctx.get('llm');
        model.stream = () => { report.generationRequests++; throw new Error('PUBLIC no generation permitted'); };
        const agents = ctx.get('agents');
        assert.equal(agents.get(report.sessionId), undefined);
        report.phase = 'seed';
        const text = 'PUBLIC original intent';
        // Fixture encoding follows the documented v1 raw queue/text/UTC digest; no admission RPC is sent.
        const promptFingerprint = `v1:sha256:${createHash('sha256').update(JSON.stringify(['queue', [['text', text]], 'UTC'])).digest('hex')}`;
        const message = llm.createUserMessage({ content: [{ type: 'text', text }],
          source: { kind: 'user', rpcId: report.requestId, promptFingerprint } });
        const header = { id: report.sessionId, version: sessions.SESSION_FORMAT_VERSION, createdAt: 1,
          cwd: process.cwd(), isSeeded: false };
        const events = [
          { type: 'agent/inbox/spliced', seq: 0, time: 1, data: { target: 'next-turn', start: 0, inserted: [message] } },
          { type: 'agent/inbox/spliced', seq: 1, time: 2, data: { target: 'next-turn', start: 0, removedCount: 1, inserted: [] } },
        ];
        await using handle = await ctx.get('sessionPersistence').create(header);
        await handle.append(events);
        await handle.flush();
        report.seededEvents = events.length;
        report.sessionFormatVersion = sessions.SESSION_FORMAT_VERSION;
        assert.equal(agents.get(report.sessionId), undefined);
        assert.equal(ctx.get('sessions').get(report.sessionId), undefined);
        report.targetAgentAbsentBeforeAndAfter = true;
        return runtime;
      } };
    } });
  await controller.boot;
  assert.equal(fatal, false);
  assert.equal(report.runtimeVersion, '0.2.1-alpha.1');
  assert.equal(report.composedBoots, 1);
  assert.equal(report.seededEvents, 2);
  assert.equal(report.generationRequests, 0);
  assert.equal(report.externalFetches, 0);
  report.phase = 'complete';
  report.status = 'passed';
} catch {
  report.status = 'failed';
} finally {
  try { await controller?.shutdown(); report.stopped = true; }
  catch { report.status = 'failed'; report.phase = 'shutdown'; }
  globalThis.fetch = originalFetch;
  await mkdir(output, { recursive: true, mode: 0o700 });
  await writeFile(join(output, 'seed.json'), JSON.stringify(report, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
}
diagnostics.restore();
process.exitCode = report.status === 'passed' ? 0 : 1;
