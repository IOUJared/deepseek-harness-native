import assert from 'node:assert/strict';
import test from 'node:test';
import { parseForkCommand } from './fork-supervisor.mjs';
import { defaultCodexUsagePreferences } from './codex-usage-preferences.mjs';

const ID = 'account-11111111-1111-4111-8111-111111111111';
const parse = command => parseForkCommand(JSON.stringify({ requestId: 'PUBLIC_1', ...command }));
const accepted = [
  { type: 'codex-accounts-status' },
  { type: 'codex-add', label: 'Personal', expectedEpoch: 0 },
  { type: 'codex-add', label: ' PUBLIC label ', expectedEpoch: 0 },
  { type: 'codex-add', label: '😀'.repeat(128), expectedEpoch: Number.MAX_SAFE_INTEGER },
  { type: 'codex-select', accountId: ID, expectedEpoch: 1 },
  { type: 'codex-signout', accountId: ID, expectedEpoch: 2 },
  { type: 'codex-signout-all', expectedEpoch: 3 },
];

test('actual private parser admits only exact managed controls and preserves legacy fixed packets', () => {
  for (const packet of accepted) assert.deepEqual(parse(packet), { requestId: 'PUBLIC_1', ...packet });
  for (const type of ['codex-status', 'codex-start']) assert.deepEqual(parse({ type }), { requestId: 'PUBLIC_1', type });
  assert.deepEqual(parse({ type: 'codex-cancel', attempt: 1 }), { requestId: 'PUBLIC_1', type: 'codex-cancel', attempt: 1 });
});

test('every managed parser acceptance rejects extra authority, omissions, invalid epochs and credential-shaped ids', () => {
  for (const packet of accepted) {
    for (const key of ['token', 'grant', 'issuerId', 'service', 'callback', 'url']) assert.throws(() => parse({ ...packet, [key]: 'PUBLIC_FAKE_SECRET' }), { message: 'Invalid native command' });
    for (const key of Object.keys(packet)) { const incomplete = { ...packet }; delete incomplete[key]; assert.throws(() => parse(incomplete)); }
    if ('expectedEpoch' in packet) for (const expectedEpoch of [-1, 0.5, Number.MAX_SAFE_INTEGER + 1, null, '0', Infinity]) assert.throws(() => parse({ ...packet, expectedEpoch }));
    if ('accountId' in packet) for (const accountId of ['', 'issuer-account', 'PUBLIC_FAKE_TOKEN', ID.toUpperCase(), ID.replace('-4111-', '-1111-'), ID.replace('-8111-', '-7111-'), ID + '\n', 1]) assert.throws(() => parse({ ...packet, accountId }));
    if ('label' in packet) for (const label of ['', ' ', '\u00a0', 'a\nb', 'a\u202Eb', 'x'.repeat(129), '😀'.repeat(129), '\ud800', null, 1]) assert.throws(() => parse({ ...packet, label }));
  }
});

test('actual private parser accepts exact usage preference commands and preserves fractional thresholds', () => {
  const preferences = { ...defaultCodexUsagePreferences(), composerDisplay: 'progressBar',
    usageAlert: 'custom', customRemainingPercent: 12.25 };
  assert.deepEqual(parse({ type: 'codex-usage-load' }), { type: 'codex-usage-load', requestId: 'PUBLIC_1' });
  for (const expectedRevision of [0, 1, Number.MAX_SAFE_INTEGER]) {
    assert.deepEqual(parse({ type: 'codex-usage-save', expectedRevision, preferences }),
      { type: 'codex-usage-save', requestId: 'PUBLIC_1', expectedRevision, preferences });
  }
});

test('usage parser rejects extra authority, missing preferences, malformed enums and unsafe revisions', () => {
  const preferences = defaultCodexUsagePreferences();
  const packet = { type: 'codex-usage-save', expectedRevision: 0, preferences };
  for (const extra of [{ token: 'PUBLIC_FAKE_SECRET' }, { service: 'llm' }, { resetTime: 1 }, { accountId: ID }]) {
    assert.throws(() => parse({ ...packet, ...extra }));
    assert.throws(() => parse({ type: 'codex-usage-load', ...extra }));
  }
  for (const expectedRevision of [-1, 0.5, Number.MAX_SAFE_INTEGER + 1, '0', null, Infinity]) assert.throws(() => parse({ ...packet, expectedRevision }));
  for (const bad of [null, [], {}, { ...preferences, composerDisplay: 'bar' }, { ...preferences, usageAlert: 'always' },
    { ...preferences, customRemainingPercent: -0.1 }, { ...preferences, customRemainingPercent: 100.1 },
    { ...preferences, customRemainingPercent: '20' }, { ...preferences, retryAfterReset: 1 },
    { ...preferences, experimentalQuota: null }, { ...preferences, token: 'PUBLIC_FAKE_SECRET' }]) {
    assert.throws(() => parse({ ...packet, preferences: bad }), { message: 'Invalid native command' });
  }
  for (const key of Object.keys(preferences)) {
    const incomplete = { ...preferences }; delete incomplete[key];
    assert.throws(() => parse({ ...packet, preferences: incomplete }));
  }
});

test('managed cancellation cannot carry destructive authority or stale arbitrary control payloads', () => {
  for (const extra of [{ accountId: ID }, { expectedEpoch: 0 }, { signOut: true }, { revoke: true }, { label: 'Personal' }]) assert.throws(() => parse({ type: 'codex-cancel', attempt: 1, ...extra }));
  for (const attempt of [0, -1, Number.MAX_SAFE_INTEGER + 1, null, '1']) assert.throws(() => parse({ type: 'codex-cancel', attempt }));
  assert.throws(() => parse({ type: 'codex-start', label: 'Personal' }));
  assert.throws(() => parse({ type: 'codex-accounts-status', expectedEpoch: 0 }));
});
