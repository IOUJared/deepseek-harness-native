import assert from 'node:assert/strict';
import nodeTest from 'node:test';
import { createRequire } from 'node:module';
import { pathToFileURL } from 'node:url';
import { promises as fs } from 'node:fs';
import { createCodexBridge } from './codex.mjs';

// Missing companion artifacts skip proxy checks; broken artifacts fail.
const companionManifest = new URL('../../../deepseek-harness-linux/apps/cli/package.json', import.meta.url);
let manifestPresent;
try { await fs.lstat(companionManifest); manifestPresent = true; }
catch (error) { if (error.code !== 'ENOENT') throw error; manifestPresent = false; }
let cordisPath;
if (manifestPresent) {
  const manifest = JSON.parse(await fs.readFile(companionManifest, 'utf8'));
  assert.equal(manifest.name, '@deepseek-ai/dsh');
  assert.equal(manifest.version, '0.2.1-alpha.1', 'Companion CLI must be the exact supported alpha');
  const require = createRequire(companionManifest);
  try { cordisPath = require.resolve('@deepseek-ai/cordis'); }
  catch (error) {
    if (error.code !== 'MODULE_NOT_FOUND' || !error.message.startsWith("Cannot find module '@deepseek-ai/cordis'")) throw error;
  }
}
const artifactSkip = cordisPath ? false : 'Requires built same-version alpha Cordis artifact; actual-proxy integration unrun';
if (artifactSkip && process.env.DSH_REQUIRE_ALPHA_TEST_ARTIFACTS === '1') throw new Error(artifactSkip);
const { Context, Service } = cordisPath ? await import(pathToFileURL(cordisPath).href) : {};
if (cordisPath) {
  assert.equal(typeof Context, 'function', 'Invalid Cordis Context export');
  assert.equal(typeof Service, 'function', 'Invalid Cordis Service export');
}
const test = (name, body) => nodeTest(name, { skip: artifactSkip }, body);
const ORIGINAL = Symbol.for('cordis.original');
const ID = 'account-11111111-1111-4111-8111-111111111111';
const row = label => ({ opaqueId: ID, label, email: null, plan: null });
const tick = () => new Promise(resolve => setImmediate(resolve));
const gate = () => { let resolve; const promise = new Promise(done => { resolve = done; }); return { resolve, promise }; };

async function provider(root, { login, describe } = {}) {
  let state = { accounts: [], selected: null, epoch: 0, legacy: 'absent' };
  const calls = [];
  let original;
  const fork = await root.plugin(owner => {
    original = new class extends Service {
      constructor() { super(owner, 'codexAccounts'); }
      async describe() { calls.push(['describe', this.ctx.bridgeCaller]); return describe ? describe(structuredClone(state)) : structuredClone(state); }
      async beginLogin(request) {
        calls.push(['add', this.ctx.bridgeCaller]);
        if (login) await login(request);
        const account = row(request.label); state = { ...state, accounts: [account] };
        return { account, epoch: state.epoch };
      }
      async cancelLogin() { calls.push(['cancel', this.ctx.bridgeCaller]); }
      async select(id, epoch) { assert.equal(epoch, state.epoch); calls.push(['select', this.ctx.bridgeCaller]); state = { ...state, selected: id, epoch: state.epoch + 1 }; }
      async signOut(id, epoch) { assert.equal(epoch, state.epoch); assert.equal(id, ID); calls.push(['signout', this.ctx.bridgeCaller]); state = { ...state, accounts: [], selected: null, epoch: state.epoch + 1 }; }
      async signOutAll(epoch) { assert.equal(epoch, state.epoch); calls.push(['signout-all', this.ctx.bridgeCaller]); state = { ...state, accounts: [], selected: null, epoch: state.epoch + 1 }; }
    }();
  });
  return { fork, original, calls };
}

async function fixture(options) {
  const root = new Context();
  const current = await provider(root, options);
  const caller = root.extend({ bridgeCaller: 'PUBLIC native caller' });
  const first = caller.get('codexAccounts'), second = caller.get('codexAccounts');
  assert.notEqual(first, second); assert.equal(first[ORIGINAL], current.original); assert.equal(second[ORIGINAL], current.original);
  const bridge = createCodexBridge({ ctx: caller, emit: () => {} });
  return { root, current, caller, bridge, async close() { await bridge.stop(); await root.fiber.dispose(); } };
}

test('actual Cordis fresh traceable proxies admit status, zero-epoch Add, selection and local logout through caller context', async t => {
  const f = await fixture(); t.after(() => f.close());
  assert.equal((await f.bridge.handle({ type: 'codex-accounts-status' })).epoch, 0);
  await f.bridge.handle({ type: 'codex-add', label: ' PUBLIC label ', expectedEpoch: 0 }); await tick();
  const added = await f.bridge.handle({ type: 'codex-accounts-status' });
  assert.equal(added.accounts[0].label, ' PUBLIC label '); assert.deepEqual(added.added, { accountId: ID, epoch: 0, attempt: 1 });
  const selected = await f.bridge.handle({ type: 'codex-select', accountId: ID, expectedEpoch: 0 }); assert.equal(selected.selected, ID);
  await f.bridge.handle({ type: 'codex-signout', accountId: ID, expectedEpoch: 1 });
  await f.bridge.handle({ type: 'codex-signout-all', expectedEpoch: 2 });
  assert.equal(f.current.calls.every(([, context]) => context === 'PUBLIC native caller'), true);
});

test('actual Cordis service replacement rejects an awaited roster even when labels and metadata match', async t => {
  const entered = gate(), release = gate();
  const f = await fixture({ describe: async value => { entered.resolve(); await release.promise; return value; } }); t.after(() => f.close());
  const pending = f.bridge.handle({ type: 'codex-accounts-status' });
  const rejected = assert.rejects(pending, { message: 'Codex unavailable' });
  await entered.promise; await f.current.fork.dispose();
  const replacement = await provider(f.root);
  assert.notEqual(replacement.original, f.current.original);
  release.resolve(); await rejected;
});

test('actual Cordis replacement makes an uncancelled late Add sticky without publishing an owned receipt', async t => {
  const entered = gate(), release = gate();
  const f = await fixture({ login: async request => { entered.resolve(request); await release.promise; } }); t.after(() => f.close());
  await f.bridge.handle({ type: 'codex-add', label: 'Personal', expectedEpoch: 0 }); await entered.promise;
  await f.current.fork.dispose(); await provider(f.root);
  release.resolve(); await tick();
  const status = await f.bridge.handle({ type: 'codex-status' });
  assert.equal(status.retryBlocked, true); assert.equal(status.phase, 'failed');
  assert.equal((await f.bridge.handle({ type: 'codex-accounts-status' })).added, null);
});

test('actual Cordis replacement cannot redirect cancellation or turn late Add into an owned receipt', async t => {
  const entered = gate(), release = gate();
  const f = await fixture({ login: async request => { entered.resolve(request); await release.promise; } }); t.after(() => f.close());
  const started = await f.bridge.handle({ type: 'codex-add', label: 'Personal', expectedEpoch: 0 });
  const request = await entered.promise;
  await f.current.fork.dispose(); const replacement = await provider(f.root);
  await f.bridge.handle({ type: 'codex-cancel', attempt: started.attempt }); await tick();
  assert.equal(request.signal.aborted, true);
  assert.equal(f.current.calls.filter(([kind]) => kind === 'cancel').length, 1);
  assert.equal(replacement.calls.filter(([kind]) => kind === 'cancel').length, 0);
  release.resolve(); await tick();
  assert.equal((await f.bridge.handle({ type: 'codex-accounts-status' })).added, null);
});
