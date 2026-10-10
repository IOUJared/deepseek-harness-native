import test from 'node:test';
import assert from 'node:assert/strict';
import { createCodexBridge } from './codex.mjs';
import { projectCodexRuntimeValues } from './codex-runtime-values.mjs';
import { parseForkCommand } from './fork-supervisor.mjs';

const empty = () => ({ ...projectCodexRuntimeValues(undefined) });
const code = expected => error => error.code === expected && error.message === expected;
const deferred = () => { let resolve; const promise = new Promise(done => { resolve = done; }); return { promise, resolve }; };
function fixture() {
  const state = { revision: 4, base: { transport: 'sse', timeoutMs: 2000 }, profile: {
    transport: 'auto', cacheRetention: 'short', models: [{ id: 'PUBLIC-model' }],
    headers: { 'PUBLIC-preserved': 'fixture' }, apiKeyEnv: 'PUBLIC_REFERENCE',
  }, calls: [], hook: null };
  const fields = Object.keys(empty());
  const schema = { uid: 0, refs: { 0: { type: 'object', dict: { providers: 1 } },
    1: { type: 'dict', inner: 2 }, 2: { type: 'object', dict: Object.fromEntries(fields.map((field, index) => [field, index + 3])) },
    ...Object.fromEntries(fields.map((_field, index) => [index + 3, { meta: {} }])) } };
  const llm = {
    listConfigurableProviders: () => [{ provider: 'openai-codex', settingsNs: 'PUBLIC-provider', settingsPath: ['providers', 'openai-codex'] }],
    listProviders: () => [],
  };
  const settings = {
    writable: true,
    describe(options) {
      assert.deepEqual(options, { redactSecrets: true });
      return [{ ns: 'PUBLIC-provider', revision: state.revision, schema,
        value: { providers: { 'openai-codex': state.profile } },
        base: { providers: { 'openai-codex': state.base } },
        user: { providers: { 'openai-codex': state.profile } } }];
    },
    async mutate(ns, ops, expected) {
      state.calls.push({ ns, ops, expected });
      if (state.hook) await state.hook();
      if (expected !== state.revision) throw Object.assign(new Error('PUBLIC conflict'), { code: 'SETTINGS_CONFLICT' });
      const before = JSON.stringify(state.profile);
      for (const op of ops) {
        assert.deepEqual(op.path.slice(0, 2), ['providers', 'openai-codex']);
        const field = op.path[2];
        if (op.op === 'set') state.profile[field] = op.value;
        else if (Object.hasOwn(state.base, field)) state.profile[field] = state.base[field];
        else delete state.profile[field];
      }
      if (JSON.stringify(state.profile) !== before) state.revision++;
    },
  };
  const services = { llm, settings };
  const bridge = createCodexBridge({ ctx: { get: name => services[name] }, emit() {}, addSecret() {} });
  return { state, services, bridge };
}
const save = (bridge, values, expectedRevision = 4) => bridge.handle({ type: 'codex-runtime-save', expectedRevision, values });

test('runtime read projects only fixed nonsecret fields and preserves inherited values', async () => {
  const { bridge } = fixture();
  const value = await bridge.handle({ type: 'codex-runtime-read' });
  assert.equal(value.values.transport, 'auto'); assert.equal(value.baseValues.transport, 'sse');
  assert.equal(value.configured, true); assert.equal(value.writable, true);
  assert.equal(JSON.stringify(value).includes('PUBLIC_REFERENCE'), false);
  assert.equal(JSON.stringify(value).includes('PUBLIC-preserved'), false);
  await bridge.stop();
});
test('read preserves an explicit pin equal to base while another runtime field changes', async () => {
  const { bridge, state } = fixture(); state.profile.transport = state.base.transport;
  const before = await bridge.handle({ type: 'codex-runtime-read' });
  assert.equal(before.values.transport, before.baseValues.transport);
  assert.equal(before.overrides.transport, 'sse');
  const after = await save(bridge, { ...before.overrides, cacheRetention: 'long' });
  assert.equal(after.overrides.transport, 'sse');
  assert.deepEqual(state.calls[0].ops[0], { op: 'set', path: ['providers', 'openai-codex', 'transport'], value: 'sse' });
  await bridge.stop();
});
test('fixed path mutation preserves unrelated fields and inheritance returns public base', async () => {
  const { bridge, state } = fixture();
  const original = { models: state.profile.models, headers: state.profile.headers, apiKeyEnv: state.profile.apiKeyEnv };
  const value = await save(bridge, { ...empty(), cacheRetention: 'long', streamIdleTimeoutMs: 18000 });
  assert.equal(value.revision, 5); assert.equal(value.values.transport, 'sse');
  assert.equal(value.values.timeoutMs, 2000); assert.equal(value.values.cacheRetention, 'long');
  for (const field of Object.keys(original)) assert.deepEqual(state.profile[field], original[field]);
  assert.equal(state.calls.length, 1); assert.equal(state.calls[0].ops.length, 8);
  await bridge.stop();
});
test('stale revision and invalid caller fields never invoke Settings mutation', async () => {
  const { bridge, state } = fixture();
  await assert.rejects(save(bridge, empty(), 3), code('CODEX_RUNTIME_CONFLICT'));
  await assert.rejects(save(bridge, { ...empty(), headers: {} }), code('CODEX_RUNTIME_UNAVAILABLE'));
  assert.equal(state.calls.length, 0); await bridge.stop();
});
test('input revision and values are captured before queued execution', async () => {
  const { bridge, state } = fixture(); const values = { ...empty(), transport: 'websocket' };
  const packet = { type: 'codex-runtime-save', expectedRevision: 4, values };
  const pending = bridge.handle(packet); packet.expectedRevision = 99; values.transport = 'sse';
  const result = await pending; assert.equal(result.values.transport, 'websocket');
  assert.equal(state.calls[0].expected, 4); await bridge.stop();
});
test('public Settings conflict remains conflict, other invoked failures are indeterminate', async () => {
  for (const [failure, expected] of [[{ code: 'SETTINGS_CONFLICT' }, 'CODEX_RUNTIME_CONFLICT'], [{}, 'CODEX_RUNTIME_INDETERMINATE']]) {
    const { bridge, state } = fixture(); state.hook = () => { throw Object.assign(new Error('PUBLIC detail must not escape'), failure); };
    await assert.rejects(save(bridge, empty()), code(expected)); assert.equal(state.calls.length, 1); await bridge.stop();
  }
});
test('stop drains invoked mutation but cannot acknowledge it as a live-owner save', async () => {
  const { bridge, state } = fixture(); const gate = deferred(); state.hook = () => gate.promise;
  const pending = save(bridge, empty()); await Promise.resolve(); await Promise.resolve();
  assert.equal(bridge.busy, true); let settled = false; const stop = bridge.stop().then(() => { settled = true; });
  await Promise.resolve(); assert.equal(settled, false); gate.resolve();
  await assert.rejects(pending, code('CODEX_RUNTIME_INDETERMINATE')); await stop; assert.equal(settled, true);
});
test('owner replacement and later revision jump cannot become successful acknowledgments', async () => {
  for (const kind of ['owner', 'revision']) {
    const { bridge, state, services } = fixture();
    state.hook = () => { if (kind === 'owner') services.settings = { ...services.settings }; else state.revision += 2; };
    await assert.rejects(save(bridge, empty()), code(kind === 'owner' ? 'CODEX_RUNTIME_INDETERMINATE' : 'CODEX_RUNTIME_CONFLICT'));
    await bridge.stop();
  }
});
test('runtime read and a second mutation are refused while one write owns the lane', async () => {
  const { bridge, state } = fixture(); const gate = deferred(); state.hook = () => gate.promise;
  const pending = save(bridge, empty()); await Promise.resolve();
  await assert.rejects(bridge.handle({ type: 'codex-runtime-read' }), code('CODEX_RUNTIME_UNAVAILABLE'));
  await assert.rejects(save(bridge, empty()), code('CODEX_RUNTIME_UNAVAILABLE'));
  gate.resolve(); await pending; assert.equal(state.calls.length, 1); await bridge.stop();
});
test('parser accepts only fixed runtime packets, captures values and rejects addressing extras', () => {
  const packet = { type: 'codex-runtime-save', requestId: 'PUBLIC-request', expectedRevision: 0, values: empty() };
  assert.equal(Object.isFrozen(parseForkCommand(JSON.stringify(packet)).values), true);
  assert.deepEqual(parseForkCommand(JSON.stringify({ type: 'codex-runtime-read', requestId: 'PUBLIC-read' })), { type: 'codex-runtime-read', requestId: 'PUBLIC-read' });
  for (const invalid of [{ ...packet, ns: 'other' }, { ...packet, expectedRevision: '0' }, { ...packet, values: { ...empty(), timeoutMs: -1 } }]) {
    assert.throws(() => parseForkCommand(JSON.stringify(invalid)), /Invalid native command/);
  }
});
