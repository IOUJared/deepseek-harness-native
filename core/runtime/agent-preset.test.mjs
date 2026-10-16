import assert from 'node:assert/strict';
import test from 'node:test';
import { boundedRoster, createAgentPresetBridge, validPresetId } from './agent-preset.mjs';

const row = (id, extra = {}) => ({ id, isDefault: false, ...extra });
const bridgeWith = service => createAgentPresetBridge({ ctx: { get: () => service } });

test('the roster projects only the five rendered fields', async () => {
  const bridge = bridgeWith({
    remoteExportList: async () => ({
      presets: [
        row('implementer', { isDefault: true, name: 'Implementer', description: 'Writes code' }),
        row('reviewer', { name: 'Reviewer', secret: 'PUBLIC_FAKE_SECRET', nested: { deep: true } }),
        row('broken-one', { broken: 'activation failed', extra: 1 }),
      ],
      secret: 'PUBLIC_FAKE_SECRET',
    }),
  });
  const result = await bridge.handle({ type: 'agent-presets' });
  assert.deepEqual(result, {
    ok: true,
    roster: {
      presets: [
        { id: 'implementer', isDefault: true, name: 'Implementer', description: 'Writes code' },
        { id: 'reviewer', isDefault: false, name: 'Reviewer' },
        { id: 'broken-one', isDefault: false, broken: 'activation failed' },
      ],
    },
  });
  assert.ok(!JSON.stringify(result).includes('PUBLIC_FAKE_SECRET'));
});

test('a malformed roster fails closed rather than presenting a partial one', () => {
  assert.equal(boundedRoster(null), undefined);
  assert.equal(boundedRoster({}), undefined);
  assert.equal(boundedRoster({ presets: [] }), undefined);
  assert.equal(boundedRoster({ presets: [{ isDefault: true }] }), undefined);
  assert.equal(boundedRoster({ presets: [row('a', { isDefault: true, name: 'bad\nname' })] }).presets[0].name, undefined);
  assert.equal(boundedRoster({ presets: [row('a', { id: undefined, isDefault: true })] }), undefined);
  // A roster with no default cannot tell a client which preset a new session gets.
  assert.equal(boundedRoster({ presets: [row('a'), row('b')] }), undefined);
  const many = Array.from({ length: 200 }, (_, index) => row(`p${index}`, { isDefault: index === 0 }));
  assert.equal(boundedRoster({ presets: many }).presets.length, 64);
  assert.equal(validPresetId('implementer'), true);
  assert.equal(validPresetId(''), false);
  assert.equal(validPresetId('x'.repeat(129)), false);
  assert.equal(validPresetId('bad\u0000id'), false);
});

test('a missing, malformed or failing service is unavailable rather than fatal', async () => {
  assert.equal((await createAgentPresetBridge({ ctx: { get: () => undefined } }).handle({ type: 'agent-presets' })).ok, false);
  assert.equal((await createAgentPresetBridge({ ctx: { get: () => ({ remoteExportList: 1 }) } }).handle({ type: 'agent-presets' })).ok, false);
  const throwing = bridgeWith({ remoteExportList: async () => { throw new Error('PUBLIC_FAKE_SECRET'); } });
  const failed = await throwing.handle({ type: 'agent-presets' });
  assert.deepEqual(failed, { ok: false, error: { code: 'unavailable', message: 'agent presets are unavailable' } });
  assert.ok(!JSON.stringify(failed).includes('PUBLIC_FAKE_SECRET'));
  const malformed = bridgeWith({ remoteExportList: async () => ({ presets: [{ isDefault: true }] }) });
  assert.deepEqual(await malformed.handle({ type: 'agent-presets' }), { ok: false, error: { code: 'invalid', message: 'agent roster is malformed' } });
  // Unknown commands are refused, never silently accepted.
  assert.equal((await throwing.handle({ type: 'agent-preset-resolve' })).ok, false);
  assert.equal((await throwing.handle(undefined)).ok, false);
});
