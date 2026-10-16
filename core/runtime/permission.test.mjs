import assert from 'node:assert/strict';
import test from 'node:test';
import { boundedPermissionCatalog, createPermissionBridge, validPermissionName } from './permission.mjs';

const option = (value, name, description) => ({ value, name, ...(description === undefined ? {} : { description }) });
const catalog = () => ({
  options: [
    option('read-only', 'Read only', 'Ask before changing anything'),
    option('workspace-write', 'Workspace write'),
    option('full-access', 'Full access'),
  ],
  defaultOptions: [option('workspace-write', 'Workspace write')],
  defaultPreset: 'workspace-write',
});
const bridgeWith = service => createPermissionBridge({ ctx: { get: name => (name === 'permissionPresets' ? service : undefined) } });

test('the catalog projects only the three rendered fields', async () => {
  let calls = 0;
  const bridge = bridgeWith({
    catalog: async () => { calls++; return { ...catalog(), secret: 'PUBLIC_FAKE_SECRET', options: [{ ...option('read-only', 'Read only', 'Ask'), receiptId: 'PUBLIC_RECEIPT', extra: { nested: true } }, ...catalog().options.slice(1)] }; },
    set: async () => {},
  });
  const result = await bridge.handle({ type: 'permission-catalog' });
  assert.equal(calls, 1);
  assert.deepEqual(result, {
    ok: true,
    catalog: {
      options: [
        { value: 'read-only', name: 'Read only', description: 'Ask' },
        { value: 'workspace-write', name: 'Workspace write' },
        { value: 'full-access', name: 'Full access' },
      ],
      defaultOptions: [{ value: 'workspace-write', name: 'Workspace write' }],
      defaultPreset: 'workspace-write',
    },
  });
  assert.ok(!JSON.stringify(result).includes('PUBLIC_FAKE_SECRET'));
  assert.ok(!JSON.stringify(result).includes('PUBLIC_RECEIPT'));
});

test('an option without a usable name falls back to its value and unbounded text is refused', () => {
  assert.deepEqual(boundedPermissionCatalog({ options: [{ value: 'custom' }], defaultOptions: [], defaultPreset: 'custom' }),
    { options: [{ value: 'custom', name: 'custom' }], defaultOptions: [], defaultPreset: 'custom' });
  // The Host's own `optionOf` falls back to the preset key for a missing label, so a
  // control-character label is replaced rather than failing the whole catalog.
  assert.deepEqual(boundedPermissionCatalog({ options: [{ value: 'x', name: 'bad\nname' }] }).options,
    [{ value: 'x', name: 'x' }]);
  assert.equal(boundedPermissionCatalog({ options: [] }), undefined);
  assert.equal(boundedPermissionCatalog({ options: [{ name: 'no value' }] }), undefined);
  assert.equal(boundedPermissionCatalog(null), undefined);
  assert.equal(boundedPermissionCatalog({ options: 'nope' }), undefined);
  const many = Array.from({ length: 200 }, (_, index) => option(`p${index}`, `Preset ${index}`));
  assert.equal(boundedPermissionCatalog({ options: many }).options.length, 64);
  assert.equal(validPermissionName('workspace-write'), true);
  assert.equal(validPermissionName(''), false);
  assert.equal(validPermissionName('x'.repeat(129)), false);
  assert.equal(validPermissionName('bad\u0000name'), false);
});

test('a set carries the exact session and preset, and a refusal stays bounded', async () => {
  const seen = [];
  const bridge = bridgeWith({
    catalog: async () => catalog(),
    set: async (session, name) => { seen.push([session, name]); if (name === 'unknown') throw new Error('PUBLIC_FAKE_SECRET unknown preset'); },
  });
  assert.deepEqual(await bridge.handle({ type: 'permission-set', session: 'session-1', name: 'full-access' }), { ok: true });
  assert.deepEqual(seen, [['session-1', 'full-access']]);
  const refused = await bridge.handle({ type: 'permission-set', session: 'session-1', name: 'unknown' });
  assert.deepEqual(refused, { ok: false, error: { code: 'invalid', message: 'permission preset was refused' } });
  assert.ok(!JSON.stringify(refused).includes('PUBLIC_FAKE_SECRET'));
  // Bounded inputs only: a missing, empty or oversized field never reaches the Host.
  assert.equal((await bridge.handle({ type: 'permission-set', session: 'session-1' })).ok, false);
  assert.equal((await bridge.handle({ type: 'permission-set', name: 'full-access' })).ok, false);
  assert.equal((await bridge.handle({ type: 'permission-set', session: '', name: 'full-access' })).ok, false);
  assert.equal((await bridge.handle({ type: 'permission-set', session: 'session-1', name: 'x'.repeat(129) })).ok, false);
  assert.equal(seen.length, 2);
});

test('a missing, malformed or failing service is unavailable rather than fatal', async () => {
  assert.deepEqual(await createPermissionBridge({ ctx: { get: () => undefined } }).handle({ type: 'permission-catalog' }),
    { ok: false, error: { code: 'unavailable', message: 'permission presets are unavailable' } });
  assert.deepEqual(await createPermissionBridge({ ctx: { get: () => ({ catalog: 1, set: 1 }) } }).handle({ type: 'permission-set', session: 's', name: 'n' }),
    { ok: false, error: { code: 'unavailable', message: 'permission presets are unavailable' } });
  const throwing = bridgeWith({ catalog: async () => { throw new Error('PUBLIC_FAKE_SECRET'); }, set: async () => {} });
  assert.equal((await throwing.handle({ type: 'permission-catalog' })).ok, false);
  const malformed = bridgeWith({ catalog: async () => ({ options: [{ name: 'no value' }] }), set: async () => {} });
  assert.deepEqual(await malformed.handle({ type: 'permission-catalog' }),
    { ok: false, error: { code: 'invalid', message: 'permission catalog is malformed' } });
  // An unknown command is refused, never silently accepted.
  assert.equal((await throwing.handle({ type: 'permission-drop' })).ok, false);
  assert.equal((await throwing.handle(undefined)).ok, false);
});
