// Real public alpha profile: does the composed Host publish the `turnOutline` projection, and does a
// freshly created blank session carry it? No prompt, credential, catalog or network action is
// scripted. The frame every client reads is built from this same registry snapshot
// (`packages/api/session-controller/src/control.ts:62-82`), so a key here is a key native receives.
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { createRequire } from 'node:module';
import { randomUUID } from 'node:crypto';
import { writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { startForkSupervisor, loadForkRuntime } from '../core/runtime/fork-supervisor.mjs';
import { interceptDiagnostics } from '../core/runtime/supervisor.mjs';

const output = process.env.DSH_NATIVE_TURNOUTLINE_EVIDENCE;
if (!output || resolve(output) !== output || process.env.DSH_TAURI_HOME !== join(output, 'harness')) throw new Error('private-output-required');
const diagnostics = interceptDiagnostics(process.stdout, process.stderr);
const input = new EventEmitter(); input.pause = () => {};
let controller, ctx, phase = 'boot', status = 'failed', fatal = false;
const facts = { registryPresent: false, keyPublished: false, blankSessionIsEmpty: false, keysAreClientVisible: false };
let keys = [];
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
  keys = Object.keys(snapshot.values).sort();
  const outline = snapshot.values.turnOutline;
  facts.keyPublished = Object.prototype.hasOwnProperty.call(snapshot.values, 'turnOutline');
  facts.blankSessionIsEmpty = Array.isArray(outline) && outline.length === 0;
  // The registry exposes client-visible units only; a host-only unit would be absent here by design.
  facts.keysAreClientVisible = keys.length > 0;
  phase = 'shutdown';
  await controller.shutdown(); assert.equal(fatal, false);
  status = 'passed'; phase = 'complete';
} catch (_error) {
  // Only the fixed phase is reported; no credential, profile or exception body.
} finally {
  try { await controller?.shutdown(); } catch (_error) { status = 'failed'; }
  await writeFile(join(output, 'turn-outline.json'), `${JSON.stringify({ status, phase, fatal, keys, facts,
    runtime: '0.2.1-alpha.1', providerSubstituted: false, modelPromptsRequested: false, catalogRequested: false,
    scope: 'Real alpha base+web profile: the composed session projection registry and one blank session snapshot; not native transport, GUI or a model run',
  }, null, 2)}\n`, { mode: 0o600, flag: 'wx' });
  diagnostics.restore(); process.exitCode = status === 'passed' ? 0 : 1;
}
