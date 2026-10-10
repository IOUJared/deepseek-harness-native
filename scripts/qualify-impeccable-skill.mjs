// Keyless full alpha profile and shipping preset skill loader; no replacement plugin tree.
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { createRequire } from 'node:module';
import { randomUUID, createHash } from 'node:crypto';
import { readFile, realpath, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { startForkSupervisor, loadForkRuntime } from '../core/runtime/fork-supervisor.mjs';
import { interceptDiagnostics } from '../core/runtime/supervisor.mjs';

const root = fileURLToPath(new URL('..', import.meta.url));
const output = process.env.DSH_NATIVE_SKILL_EVIDENCE;
if (!output || resolve(output) !== output || process.env.DSH_TAURI_HOME !== join(output, 'harness')) throw new Error('private-output-required');
const diagnostics = interceptDiagnostics(process.stdout, process.stderr);
const input = new EventEmitter(); input.pause = () => {};
let controller, ctx, removeEvents, handle, phase = 'boot', fatal = false, status = 'failed';
const facts = [];
const subjects = new Set();
const counts = { turnStarts: 0, stepStarts: 0, requests: 0, unexpectedWake: 0 };
try {
  controller = startForkSupervisor({ input, env: process.env,
    addSecret: value => diagnostics.addSecret(value), onStopped() {}, onFatal() { fatal = true; },
    send(packet) { if (packet.type === 'fatal') fatal = true; },
    loadRuntime: async env => {
      const runtime = await loadForkRuntime(env);
      return { ...runtime, runProfile: async options => {
        const loaded = await runtime.runProfile(options); ctx = loaded.ctx; return loaded;
      } };
    },
  });
  await controller.boot; assert.equal(fatal, false);
  const require = createRequire(join(process.env.DSH_TAURI_RUNTIME, 'package.json'));
  const { SessionId } = await import(pathToFileURL(require.resolve('@deepseek-ai/dsh-session')).href);
  const { ToolCallId } = await import(pathToFileURL(require.resolve('@deepseek-ai/dsh-llm')).href);
  removeEvents = ctx.on('session/event', (session, event) => {
    if (!subjects.has(session.id)) return;
    if (event.type === 'turn/start') counts.turnStarts++;
    if (event.type === 'step/start') counts.stepStarts++;
    if (event.type === 'request/header') counts.requests++;
  });
  const cases = [
    ['native-repository', root, 'impeccable-rust', join(root, '.agents/skills/impeccable-rust/SKILL.md')],
    ['desktop-workspace', resolve(root, '..'), 'impeccable-rust', join(root, '.agents/skills/impeccable-rust/SKILL.md')],
    ['backend-repository', resolve(root, '../deepseek-harness-linux'), 'harness-verification', resolve(root, '../deepseek-harness-linux/.agents/skills/harness-verification/SKILL.md')],
  ];
  for (const [name, cwd, skill, instruction] of cases) {
    phase = `${name}-create`;
    const sessionId = SessionId(randomUUID()); subjects.add(sessionId);
    handle = await ctx.agents.create({ sessionId, meta: { cwd }, async setup(agentCtx, subject) {
      await ctx.agentPresets.mount(agentCtx, 'standard');
      agentCtx.on('agent/pre-step', ({ agent }, next) => {
        if (agent !== subject) return next(); counts.unexpectedWake++; return { kind: 'reject' };
      }, { prepend: true });
    } });
    phase = `${name}-catalog`;
    const options = { cwd, scope: handle.agent };
    let snapshot = await ctx.skills.snapshot(options);
    if (!snapshot.complete) snapshot = await ctx.skills.snapshot(options);
    assert.equal(snapshot.complete, true);
    const summary = snapshot.skills.find(item => item.name === skill);
    assert.ok(summary); assert.equal(await realpath(summary.path), await realpath(instruction));
    phase = `${name}-body`;
    const body = await readFile(instruction, 'utf8');
    const expected = body.replace(/^---\r?\n[\s\S]*?\r?\n---\r?\n/, '').trim();
    const loaded = await ctx.skills.get(skill, options);
    assert.ok(loaded); assert.equal(loaded.content.trim(), expected);
    assert.equal(loaded.invocation.modelInvocable, true);
    phase = `${name}-tool`;
    const result = await ctx.tools.execute({ callId: ToolCallId(randomUUID()), name: 'skill',
      arguments: { name: skill }, signal: new AbortController().signal, agent: handle.agent });
    assert.equal(result.isError, false);
    const rendered = result.content.filter(part => part.type === 'text').map(part => part.text).join('\n');
    assert.ok(rendered.includes(`<skill_content name=\"${skill}\">`));
    assert.ok(rendered.includes(expected));
    assert.equal(loaded.resourceBase.kind, 'directory');
    assert.equal(await realpath(loaded.resourceBase.path), await realpath(resolve(instruction, '..')));
    facts.push({ name, skill, catalog: true, bodyLoaded: true, toolLoaded: true,
      instructionSha256: createHash('sha256').update(body).digest('hex') });
    phase = `${name}-dispose`; await handle.dispose(); handle = undefined;
  }
  for (const count of Object.values(counts)) assert.equal(count, 0);
  phase = 'shutdown'; await controller.shutdown(); assert.equal(fatal, false);
  status = 'passed'; phase = 'complete';
} catch (_error) { /* Only the fixed phase is reported; no credential/profile/exception body. */ }
finally {
  try { if (handle) await handle.dispose(); } catch (_error) { status = 'failed'; }
  removeEvents?.();
  try { await controller?.shutdown(); } catch (_error) { status = 'failed'; }
  await writeFile(join(output, 'skill.json'), `${JSON.stringify({ status, phase, fatal, facts, counts,
    runtime: '0.2.1-alpha.1', providerSubstituted: false, modelPromptsRequested: false,
    scope: 'Real alpha base+web profile, shipping standard preset scope, catalog/body/tool loading; not GUI or live model execution',
  }, null, 2)}\n`, { mode: 0o600, flag: 'wx' });
  diagnostics.restore(); process.exitCode = status === 'passed' ? 0 : 1;
}
