import assert from 'node:assert/strict';
import test from 'node:test';
import { createCodexBridge, codexAccountsMetadata, validCodexAccountId, validCodexLabel } from './codex.mjs';

const ID = 'account-11111111-1111-4111-8111-111111111111';
const OTHER = 'account-22222222-2222-4222-9222-222222222222';
const row = (opaqueId = ID, label = 'Personal') => ({ opaqueId, label, email: null, plan: null });
const tick = () => new Promise(resolve => setImmediate(resolve));
const gate = () => { let resolve, reject; const promise = new Promise((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; };
const url = () => 'https://auth.openai.com/oauth/authorize?' + new URLSearchParams({ response_type: 'code', client_id: 'app_EMoamEEZ73f0CkXaXp7hrann', redirect_uri: 'http://localhost:1455/auth/callback', scope: 'openid profile email offline_access', code_challenge: 'A'.repeat(43), code_challenge_method: 'S256', state: 'a'.repeat(32), id_token_add_organizations: 'true', codex_cli_simplified_flow: 'true', originator: 'pi' });
function fixture({ accounts = [], selected = null, login, select } = {}) {
  let state = { accounts, selected, epoch: 0, legacy: 'unadopted' };
  const calls = [], events = [], secrets = [];
  const check = expected => { if (expected !== state.epoch) throw Object.assign(new Error('private provider diagnostic'), { code: 'CODEX_EPOCH_CONFLICT' }); };
  const service = {
    describe: async () => structuredClone(state),
    beginLogin: async request => {
      calls.push(['add', request.label, request.expectedEpoch]); check(request.expectedEpoch);
      if (login) return login(request, commit);
      return commit(request.label);
    },
    cancelLogin: async () => { calls.push(['cancel']); },
    select: async (id, expected) => { calls.push(['select', id, expected]); check(expected); if (select) await select(); state = { ...state, selected: id, epoch: state.epoch + 1 }; },
    signOut: async (id, expected) => { calls.push(['signout', id, expected]); check(expected); state = { ...state, accounts: state.accounts.filter(a => a.opaqueId !== id), selected: state.selected === id ? null : state.selected, epoch: state.epoch + 1 }; },
    signOutAll: async expected => { calls.push(['signout-all', expected]); check(expected); state = { ...state, accounts: [], selected: null, epoch: state.epoch + 1 }; },
  };
  const commit = label => {
    state = { ...state, accounts: [...state.accounts, row(ID, label)] };
    return { account: row(ID, label), epoch: state.epoch };
  };
  const forbidden = () => { throw new Error('legacy authorization/grant must not be accessed'); };
  const services = { codexAccounts: service, credentials: { describeRecord: forbidden }, authorization: { describe: forbidden, begin: forbidden, cancel: forbidden } };
  const bridge = createCodexBridge({ ctx: { get: key => services[key] }, emit: packet => events.push(packet), addSecret: secret => secrets.push(secret) });
  return { bridge, service, services, calls, events, secrets, state: () => state, setState: value => { state = value; } };
}

test('managed Add is nonselected and keeps the exact commit receipt, not a later describe epoch', async t => {
  const f = fixture({ login: async (request, commit) => { const receipt = commit(request.label); f.setState({ ...f.state(), epoch: 9 }); return receipt; } });
  t.after(() => f.bridge.stop());
  const start = await f.bridge.handle({ type: 'codex-add', label: 'Personal', expectedEpoch: 0 });
  await tick();
  const status = await f.bridge.handle({ type: 'codex-status' });
  assert.equal(status.phase, 'authorized'); assert.equal(status.credentialStored, false);
  const accounts = await f.bridge.handle({ type: 'codex-accounts-status' });
  assert.equal(accounts.selected, null); assert.equal(accounts.epoch, 9);
  assert.deepEqual(accounts.added, { accountId: ID, epoch: 0, attempt: start.attempt });
  await assert.rejects(f.bridge.handle({ type: 'codex-select', accountId: ID, expectedEpoch: accounts.added.epoch }), { code: 'CODEX_EPOCH_CONFLICT' });
  assert.equal(f.calls.some(call => call[0] === 'select'), false);
});

test('managed Connect preserves one click but requires explicit exact-receipt selection', async t => {
  const f = fixture(); t.after(() => f.bridge.stop());
  await f.bridge.handle({ type: 'codex-start' }); await tick();
  assert.deepEqual(f.calls[0], ['add', 'ChatGPT account', 0]);
  const accounts = await f.bridge.handle({ type: 'codex-accounts-status' });
  assert.equal(accounts.selected, null);
  const selected = await f.bridge.handle({ type: 'codex-select', accountId: accounts.added.accountId, expectedEpoch: accounts.added.epoch });
  assert.equal(selected.selected, ID);
  assert.equal((await f.bridge.handle({ type: 'codex-status' })).credentialStored, true);
});

test('switch and local logout never choose another account, adopt legacy, or revoke issuer grants', async t => {
  const f = fixture({ accounts: [row(), row(OTHER, 'Work')], selected: ID }); t.after(() => f.bridge.stop());
  await f.bridge.handle({ type: 'codex-select', accountId: OTHER, expectedEpoch: 0 });
  let value = await f.bridge.handle({ type: 'codex-signout', accountId: OTHER, expectedEpoch: 1 });
  assert.equal(value.selected, null); assert.deepEqual(value.accounts, [row()]); assert.equal(value.legacy, 'unadopted');
  value = await f.bridge.handle({ type: 'codex-signout-all', expectedEpoch: 2 });
  assert.equal(value.selected, null); assert.equal(value.accounts.length, 0); assert.equal(value.legacy, 'unadopted');
  assert.deepEqual(f.calls, [['select', OTHER, 0], ['signout', OTHER, 1], ['signout-all', 2]]);
});

test('serialized mutations recheck epochs after queued selection commits', async t => {
  const barrier = gate();
  const f = fixture({ accounts: [row(), row(OTHER, 'Work')], selected: ID, select: () => barrier.promise }); t.after(() => f.bridge.stop());
  const first = f.bridge.handle({ type: 'codex-select', accountId: OTHER, expectedEpoch: 0 }); await tick();
  const next = f.bridge.handle({ type: 'codex-signout-all', expectedEpoch: 0 });
  const denial = assert.rejects(next, { code: 'CODEX_EPOCH_CONFLICT' });
  await assert.rejects(f.bridge.handle({ type: 'codex-accounts-status' }));
  barrier.resolve(); await first; await denial;
  assert.equal(f.state().selected, OTHER); assert.equal(f.state().accounts.length, 2);
  assert.equal(f.calls.some(call => call[0] === 'signout-all'), false);
});

test('unknown selection is sticky until explicit nonmutating reconciliation; no retry is invented', async t => {
  const f = fixture({ accounts: [row()], select: () => { throw new Error('PUBLIC_FAKE_TOKEN provider diagnostic'); } }); t.after(() => f.bridge.stop());
  await assert.rejects(f.bridge.handle({ type: 'codex-select', accountId: ID, expectedEpoch: 0 }), { message: 'Codex unavailable' });
  assert.equal((await f.bridge.handle({ type: 'codex-status' })).retryBlocked, true);
  await assert.rejects(f.bridge.handle({ type: 'codex-signout-all', expectedEpoch: 0 }));
  assert.deepEqual(f.calls, [['select', ID, 0]]);
  await f.bridge.handle({ type: 'codex-accounts-status' });
  assert.equal((await f.bridge.handle({ type: 'codex-status' })).retryBlocked, false);
  await f.bridge.handle({ type: 'codex-signout-all', expectedEpoch: 0 });
  assert.equal(JSON.stringify(f.events).includes('PUBLIC_FAKE_TOKEN'), false);
});

test('cancel reaches in-flight managed login outside the mutation lane and ignores late success', async t => {
  const barrier = gate(); let request;
  const f = fixture({ login: async (input, commit) => { request = input; await barrier.promise; return commit(input.label); } }); t.after(() => f.bridge.stop());
  const start = await f.bridge.handle({ type: 'codex-add', label: 'Personal', expectedEpoch: 0 }); await tick();
  await f.bridge.handle({ type: 'codex-cancel', attempt: start.attempt }); await tick();
  assert.equal(request.signal.aborted, true); assert.deepEqual(f.calls.at(-1), ['cancel']);
  await assert.rejects(f.bridge.handle({ type: 'codex-accounts-status' }));
  barrier.resolve(); await tick();
  assert.equal((await f.bridge.handle({ type: 'codex-status' })).phase, 'indeterminate');
  const value = await f.bridge.handle({ type: 'codex-accounts-status' });
  assert.equal(value.added, null); assert.equal(value.selected, null); assert.equal(value.accounts.length, 1);
  assert.equal(f.calls.some(call => ['select', 'signout', 'signout-all'].includes(call[0])), false);
});

test('managed browser/callback prompt ownership remains consuming and attempt-scoped', async t => {
  const f = fixture({ login: async (request, commit) => {
    request.interaction.notify({ url: url() });
    const answer = await request.interaction.prompt({ kind: 'secret', signal: request.signal, message: 'Complete login in your browser, or paste the authorization code / redirect URL here:', placeholder: 'http://localhost:1455/auth/callback' });
    assert.equal(answer, 'PUBLIC_FAKE_CODE'); return commit(request.label);
  } }); t.after(() => f.bridge.stop());
  const start = await f.bridge.handle({ type: 'codex-add', label: 'Personal', expectedEpoch: 0 }); await tick();
  const waiting = await f.bridge.handle({ type: 'codex-status' });
  assert.equal(waiting.browserAvailable, true);
  assert.deepEqual(await f.bridge.handle({ type: 'codex-callback', attempt: start.attempt, prompt: waiting.prompt + 1, response: 'PUBLIC_FAKE_CODE' }), { ok: false });
  const callback = { type: 'codex-callback', attempt: start.attempt, prompt: waiting.prompt, response: 'PUBLIC_FAKE_CODE' };
  assert.deepEqual(await f.bridge.handle(callback), { ok: true }); assert.deepEqual(await f.bridge.handle(callback), { ok: false });
  await tick();
  assert.equal((await f.bridge.handle({ type: 'codex-status' })).phase, 'authorized');
  assert.equal(JSON.stringify(f.events.map(event => event.value)).includes('PUBLIC_FAKE_CODE'), false);
});

test('stop drains managed login and cancel before returning and suppresses late metadata', async () => {
  const barrier = gate(); let request;
  const f = fixture({ login: async input => { request = input; return barrier.promise; } });
  await f.bridge.handle({ type: 'codex-add', label: 'Personal', expectedEpoch: 0 }); await tick();
  let done = false; const stopped = f.bridge.stop().then(() => { done = true; }); await tick();
  assert.equal(done, false); assert.equal(request.signal.aborted, true); assert.deepEqual(f.calls.at(-1), ['cancel']);
  const count = f.events.length; barrier.resolve({ account: row(), epoch: 1 }); await stopped; await tick();
  assert.equal(f.events.length, count); assert.equal(done, true);
});

test('managed service loss never falls back to legacy grants or authorization', async t => {
  const f = fixture(); t.after(() => f.bridge.stop()); await f.bridge.handle({ type: 'codex-status' });
  delete f.services.codexAccounts;
  await assert.rejects(f.bridge.handle({ type: 'codex-status' })); await assert.rejects(f.bridge.handle({ type: 'codex-start' }));
});

test('metadata allowlist rejects token fields, issuer ids, fabricated email/plan, duplicates and unbounded rows', () => {
  const value = { accounts: [row()], selected: ID, epoch: 0, legacy: 'absent' };
  assert.deepEqual(codexAccountsMetadata(value), value);
  for (const changed of [ { ...value, token: 'PUBLIC_FAKE_TOKEN' }, { ...value, accounts: [{ ...row(), token: 'PUBLIC_FAKE_TOKEN' }] }, { ...value, accounts: [row('issuer-id')] }, { ...value, accounts: [{ ...row(), email: 'fake@example.test' }] }, { ...value, accounts: [{ ...row(), plan: 'Pro' }] }, { ...value, accounts: [row(), row()] }, { ...value, accounts: Array(129).fill(row()) }, { ...value, selected: OTHER }, { ...value, epoch: Number.MAX_SAFE_INTEGER + 1 } ]) assert.throws(() => codexAccountsMetadata(changed), { message: 'Codex unavailable' });
  assert.equal(validCodexAccountId(ID), true); assert.equal(validCodexAccountId(ID.toUpperCase()), false);
  assert.equal(validCodexLabel('😀'.repeat(128)), true);
  for (const label of ['', ' ', '\u00a0', 'a\nb', 'x\u202Ey', '\u200B', '😀'.repeat(129), '\ud800']) assert.equal(validCodexLabel(label), false);
});

test('managed route activation retains revision CAS and requires selected metadata rather than legacy grant presence', async t => {
  const f = fixture({ accounts: [row()] }); t.after(() => f.bridge.stop());
  let revision = 0, count = 0, configured = false;
  const value = { nativeCodexAccounts: true, providers: {} };
  f.services.llm = { listConfigurableProviders: () => [{ provider: 'openai-codex', settingsNs: 'llm-pi-ai', settingsPath: ['providers', 'openai-codex'] }], listProviders: () => configured ? [{ id: 'openai-codex' }] : [] };
  f.services.settings = { writable: true, describe: () => [{ ns: 'llm-pi-ai', revision, value }], mutate: async (ns, ops, expected) => {
    assert.equal(ns, 'llm-pi-ai'); assert.equal(expected, revision);
    assert.deepEqual(ops, [{ op: 'set', path: ['providers', 'openai-codex'], value: {} }]);
    count++; value.providers['openai-codex'] = {}; revision++; configured = true;
  } };
  await assert.rejects(f.bridge.handle({ type: 'codex-enable-models', expectedRevision: 0 }));
  await f.bridge.handle({ type: 'codex-select', accountId: ID, expectedEpoch: 0 });
  await assert.rejects(f.bridge.handle({ type: 'codex-enable-models', expectedRevision: 99 }));
  const status = await f.bridge.handle({ type: 'codex-enable-models', expectedRevision: 0 });
  assert.equal(status.settingsRevision, 1); assert.equal(status.routeConfigured, true); assert.equal(value.nativeCodexAccounts, true);
  await f.bridge.handle({ type: 'codex-enable-models', expectedRevision: 1 }); assert.equal(count, 1);
});

test('128 accounts are allowed only when the complete metadata fits the private frame budget', () => {
  const accounts = Array.from({ length: 128 }, (_, i) => row('account-' + i.toString(16).padStart(8, '0') + '-1111-4111-8111-111111111111'));
  assert.equal(codexAccountsMetadata({ accounts, selected: null, epoch: 0, legacy: 'absent' }).accounts.length, 128);
  const oversized = accounts.map(account => ({ ...account, label: '😀'.repeat(128) }));
  assert.throws(() => codexAccountsMetadata({ accounts: oversized, selected: null, epoch: 0, legacy: 'absent' }), { message: 'Codex unavailable' });
  for (const epoch of [false, true, 0.5, Number.MAX_SAFE_INTEGER + 1]) assert.throws(() => codexAccountsMetadata({ accounts: [], selected: null, epoch, legacy: 'absent' }));
});

test('legacy absent-service flow remains one-click, grant-based, and does not fabricate accounts', async t => {
  let began = 0; let configured = false;
  const services = { credentials: { describeRecord: async () => ({ configured, kind: 'grant' }) }, authorization: { describe: () => ({ methods: [{ id: 'oauth' }] }), begin: async () => { began++; configured = true; return { status: 'authorized' }; }, cancel: () => {} } };
  const bridge = createCodexBridge({ ctx: { get: key => services[key] }, emit: () => {} }); t.after(() => bridge.stop());
  await bridge.handle({ type: 'codex-start' }); await tick();
  assert.equal(began, 1); assert.equal((await bridge.handle({ type: 'codex-status' })).credentialStored, true);
  assert.deepEqual(await bridge.handle({ type: 'codex-accounts-status' }), { mode: 'legacy', accounts: [], selected: null, epoch: null, legacy: 'absent', added: null });
  await assert.rejects(bridge.handle({ type: 'codex-add', label: 'Personal', expectedEpoch: 0 }));
});
