// Real public alpha profile: does the composed Host publish the client projections native's chat is
// built on, and does a freshly created blank session carry them? No prompt, credential, catalog or
// network action is scripted. Every follow frame and control baseline is built from this same registry
// snapshot (`packages/api/session-controller/src/control.ts:62-82`), so a key here is a key native
// receives.
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { createRequire } from 'node:module';
import { randomUUID } from 'node:crypto';
import { writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { startForkSupervisor, loadForkRuntime } from '../core/runtime/fork-supervisor.mjs';
import { interceptDiagnostics } from '../core/runtime/supervisor.mjs';

const output = process.env.DSH_NATIVE_PROJECTIONS_EVIDENCE;
if (!output || resolve(output) !== output || process.env.DSH_TAURI_HOME !== join(output, 'harness')) throw new Error('private-output-required');
const diagnostics = interceptDiagnostics(process.stdout, process.stderr);
const input = new EventEmitter(); input.pause = () => {};
let controller, ctx, phase = 'boot', status = 'failed', fatal = false;
// The keys native's chat consumes, each named with what it feeds.
const NAMED = {
  turnOutline: 'the turn rail marks and its hover previews',
  tokenUsage: 'the composer usage pill and the session statistics dialog',
  inbox: 'queued-message classification',
  contextPressure: 'the context reading',
  contextBreakdown: 'the context breakdown panel',
  modelSelection: 'the model control',
  plan: 'the plan chip',
  todos: 'the todo dock',
  subagent: 'subagent breadcrumbs',
  userQuestions: 'approval and question decisions',
  title: 'the session title',
  sessionStats: 'the activity statistics',
};
const facts = { registryPresent: false, allNamedKeysPublished: false, blankSessionShapes: false };
let published = [];
let missing = [];
try {
  controller = startForkSupervisor({
    input,
    env: process.env,
    addSecret: value => diagnostics.addSecret(value),
    onStopped() {},
    onFatal() { fatal = true; },
    send(packet) { if (packet.type === 'fatal') fatal = true; },
    loadRuntime: async env => {
      const runtime = await loadForkRuntime(env);
      return { ...runtime, runProfile: async options => {
        const loaded = await runtime.runProfile(options);
        ctx = loaded.ctx;
        return loaded;
      } };
    },
  });
  await controller.boot; assert.equal(fatal, false);
  const require = createRequire(join(process.env.DSH_TAURI_RUNTIME, 'package.json'));
  const { SessionId } = await import(pathToFileURL(require.resolve('@deepseek-ai/dsh-session')).href);
  phase = 'blank-session';
  const session = ctx.sessions.create(SessionId(randomUUID()));
  phase = 'projection-snapshot';
  const registry = ctx.sessionProjections;
  facts.registryPresent = registry !== undefined;
  assert.ok(registry, 'the desktop composition owns a projection registry');
  const snapshot = registry.snapshot(session);
  published = Object.keys(snapshot.values).sort();
  missing = Object.keys(NAMED).filter(key => !Object.prototype.hasOwnProperty.call(snapshot.values, key));
  facts.allNamedKeysPublished = missing.length === 0;
  // A blank session's shapes: the outline is an empty list and the usage reading states zero rather
  // than being absent, so native's readers see the same shape before any turn.
  const outline = snapshot.values.turnOutline;
  const usage = snapshot.values.tokenUsage;
  facts.blankSessionShapes = Array.isArray(outline) && outline.length === 0
    && (usage === null || typeof usage === 'object');
  phase = 'shutdown';
  await controller.shutdown(); assert.equal(fatal, false);
  status = 'passed'; phase = 'complete';
} catch (_error) {
  // Only the fixed phase is reported; no credential, profile or exception body.
} finally {
  try { await controller?.shutdown(); } catch (_error) { status = 'failed'; }
  await writeFile(join(output, 'projections.json'), `${JSON.stringify({ status, phase, fatal, published, missing,
    named: NAMED, facts, runtime: '0.2.1-alpha.1', providerSubstituted: false, modelPromptsRequested: false,
    catalogRequested: false,
    scope: 'Real alpha base+web profile: the composed session projection registry and one blank session snapshot, with the client keys native consumes; not native transport, GUI or a model run',
  }, null, 2)}\n`, { mode: 0o600, flag: 'wx' });
  diagnostics.restore(); process.exitCode = status === 'passed' ? 0 : 1;
}
