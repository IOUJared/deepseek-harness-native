import test from 'node:test';
import assert from 'node:assert/strict';
import { inspect } from 'node:util';
import { promises as fs } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createCodexUsageBridge, CodexUsageBridgeError } from './codex-usage-bridge.mjs';
import { createCodexUsagePreferencesStore, defaultCodexUsagePreferences } from './codex-usage-preferences.mjs';
import { openCodexUsageStorage } from './codex-usage-storage.mjs';
// Missing companion artifacts skip only real-flock checks; broken artifacts fail.
async function artifactExists(url) {
  try { await fs.lstat(url); return true; }
  catch (error) { if (error.code === 'ENOENT') return false; throw error; }
}
const companionManifest = new URL('../../../deepseek-harness-linux/apps/cli/package.json', import.meta.url);
const manifestPresent = await artifactExists(companionManifest);
if (manifestPresent) {
  const manifest = JSON.parse(await fs.readFile(companionManifest, 'utf8'));
  assert.equal(manifest.name, '@deepseek-ai/dsh');
  assert.equal(manifest.version, '0.2.1-alpha.1', 'Companion CLI must be the exact supported alpha');
}
const flockURL = new URL('../../../deepseek-harness-linux/native/system/packages/entry/lib/flock.js', import.meta.url);
const flockPresent = await artifactExists(flockURL);
assert.ok(!flockPresent || manifestPresent, 'Present flock artifact requires its companion CLI manifest');
const artifactSkip = flockPresent ? false : 'Requires built same-version alpha flock artifact; real-flock integration unrun';
if (artifactSkip && process.env.DSH_REQUIRE_ALPHA_TEST_ARTIFACTS === '1') throw new Error(artifactSkip);
const { tryLockExclusive } = flockPresent ? await import(flockURL.href) : {};
if (flockPresent) assert.equal(typeof tryLockExclusive, 'function', 'Invalid flock artifact export');

const defaults = defaultCodexUsagePreferences();
const load = { type: 'codex-usage-load', requestId: 'PUBLIC-load' };
const save = (revision, changes = {}) => ({ type: 'codex-usage-save', requestId: 'PUBLIC-save',
  expectedRevision: revision, preferences: { ...defaults, ...changes } });
const errorCode = code => error => error.code === code;
const deferred = () => { let resolve; const promise = new Promise(done => { resolve = done; }); return { promise, resolve }; };
function memory() {
  let document = null, writes = 0, reads = 0, outcome = 'committed';
  const owner = createCodexUsagePreferencesStore({ maxPendingUpdates: 4, maxDocumentBytes: 4096,
    readDocument: async () => { reads++; return document; },
    commitDocument: async packet => {
      writes++;
      if (outcome === 'committed') document = packet.document;
      return { status: outcome };
    } });
  return { owner, get writes() { return writes; }, get reads() { return reads; },
    setOutcome(value) { outcome = value; }, seed(value) { document = value; } };
}

test('lazy shared opening and detached preference input; committed roundtrip has no other I/O', async () => {
  const f = memory(), entered = deferred(), release = deferred(); let openings = 0;
  const bridge = createCodexUsageBridge({ openStorage: async () => { openings++; entered.resolve(); await release.promise; return f.owner; } });
  assert.equal(openings, 0);
  const first = bridge.handle(load);
  const command = save(0, { composerDisplay: 'runway', customRemainingPercent: 12.5 });
  const second = bridge.handle(command);
  await entered.promise;
  command.preferences.composerDisplay = 'off'; command.expectedRevision = 88;
  release.resolve();
  assert.equal((await first).persisted, false);
  const saved = await second;
  assert.equal(saved.revision, 1); assert.equal(saved.preferences.composerDisplay, 'runway');
  assert.equal(saved.preferences.customRemainingPercent, 12.5); assert.equal(openings, 1);
  assert.deepEqual(await bridge.handle(load), saved);
  assert.equal(f.writes, 1);
  await bridge.stop();
});

test('known conflict and indeterminate saves return fixed codes without replay', async () => {
  const f = memory(), bridge = createCodexUsageBridge({ openStorage: async () => f.owner });
  try {
    await bridge.handle(save(0));
    await assert.rejects(bridge.handle(save(0)), errorCode('CODEX_USAGE_CONFLICT'));
    assert.equal(f.writes, 1);
    f.setOutcome('indeterminate');
    await assert.rejects(bridge.handle(save(1)), errorCode('CODEX_USAGE_INDETERMINATE'));
    assert.equal(f.writes, 2);
    assert.equal((await bridge.handle(load)).revision, 1);
    assert.equal(f.writes, 2);
  } finally { await bridge.stop(); }
});

test('revision overflow and corrupted/unsupported documents remain unavailable, not a write ACK', async () => {
  const f = memory(), bridge = createCodexUsageBridge({ openStorage: async () => f.owner });
  try {
    f.seed({ version: 1, revision: Number.MAX_SAFE_INTEGER, preferences: defaults });
    await assert.rejects(bridge.handle(save(Number.MAX_SAFE_INTEGER)), errorCode('CODEX_USAGE_UNAVAILABLE'));
    assert.equal(f.writes, 0);
    for (const value of ['PUBLIC_INVALID_JSON', { version: 2, revision: 1, preferences: defaults }]) {
      f.seed(value);
      await assert.rejects(bridge.handle(load), errorCode('CODEX_USAGE_UNAVAILABLE'));
      await assert.rejects(bridge.handle(save(1)), errorCode('CODEX_USAGE_UNAVAILABLE'));
    }
    assert.equal(f.writes, 0);
  } finally { await bridge.stop(); }
});

test('failed factory remains sticky; invalid commands do not open storage; diagnostics are redacted', async () => {
  let openings = 0;
  const bridge = createCodexUsageBridge({ openStorage: async () => { openings++; throw new Error('PUBLIC_EXTERNAL_SECRET'); } });
  for (const command of [{ type: 'login' }, save(-1), save(0, { experimentalQuota: 'true' })]) {
    await assert.rejects(bridge.handle(command), errorCode('CODEX_USAGE_UNAVAILABLE'));
  }
  assert.equal(openings, 0);
  for (let i = 0; i < 2; i++) await assert.rejects(bridge.handle(load), error => {
    assert.equal(error.code, 'CODEX_USAGE_UNAVAILABLE');
    assert.ok(!JSON.stringify(error).includes('PUBLIC_EXTERNAL_SECRET'));
    assert.equal(Object.hasOwn(error, 'cause'), false); return true;
  });
  assert.equal(openings, 1);
  assert.equal(JSON.stringify(bridge), '"[CodexUsageBridge]"');
  assert.equal(inspect(bridge), '[CodexUsageBridge]');
  assert.equal(new CodexUsageBridgeError('PUBLIC_UNKNOWN').code, 'CODEX_USAGE_UNAVAILABLE');
  await bridge.stop();
});

test('stop before initialization settlement prevents a queued save and drains initialization', async () => {
  const f = memory(), entered = deferred(), release = deferred();
  const bridge = createCodexUsageBridge({ openStorage: async () => { entered.resolve(); await release.promise; return f.owner; } });
  const pending = bridge.handle(save(0));
  await entered.promise;
  let stopped = false;
  const stopping = bridge.stop(); assert.equal(bridge.stop(), stopping);
  void stopping.then(() => { stopped = true; });
  await assert.rejects(bridge.handle(load), errorCode('CODEX_USAGE_UNAVAILABLE'));
  assert.equal(stopped, false); release.resolve();
  await assert.rejects(pending, errorCode('CODEX_USAGE_UNAVAILABLE'));
  await stopping; assert.equal(stopped, true); assert.equal(f.writes, 0);
});

test('started commit and cleanup remain owned until actual terminal settlement', async () => {
  const entered = deferred(), release = deferred(); let committed = false;
  const owner = createCodexUsagePreferencesStore({ maxPendingUpdates: 4, maxDocumentBytes: 4096,
    readDocument: async () => null,
    commitDocument: async () => { entered.resolve(); await release.promise; committed = true; return { status: 'committed' }; } });
  const bridge = createCodexUsageBridge({ openStorage: async () => owner });
  const pending = bridge.handle(save(0)); await entered.promise;
  let stopped = false; const stopping = bridge.stop().then(() => { stopped = true; });
  await assert.rejects(bridge.handle(load), errorCode('CODEX_USAGE_UNAVAILABLE'));
  assert.equal(stopped, false); assert.equal(committed, false);
  release.resolve(); assert.equal((await pending).revision, 1); await stopping;
  assert.equal(stopped, true); assert.equal(committed, true);
});

test('complete initialization queue is bounded and stopped calls retire without starting writes', async () => {
  const entered = deferred(), release = deferred(), f = memory();
  const bridge = createCodexUsageBridge({ openStorage: async () => { entered.resolve(); await release.promise; return f.owner; } });
  const pending = Array.from({ length: 128 }, () => bridge.handle(load));
  await entered.promise;
  await assert.rejects(bridge.handle(load), errorCode('CODEX_USAGE_UNAVAILABLE'));
  const settled = Promise.allSettled(pending), stopping = bridge.stop();
  release.resolve();
  assert.ok((await settled).every(result => result.status === 'rejected'));
  await stopping; assert.equal(f.reads, 0); assert.equal(f.writes, 0);
});

test('real storage consumer restores mode and revision after a new bridge lifetime', { skip: artifactSkip }, async t => {
  const base = await fs.realpath(await fs.mkdtemp(join(tmpdir(), 'PUBLIC-usage-bridge-')));
  await fs.chmod(base, 0o700); const original = await fs.lstat(base);
  const directory = join(base, 'usage'); const owners = [];
  const openStorage = () => openCodexUsageStorage({ directory, lockExclusive: tryLockExclusive,
    maxPendingUpdates: 4, maxDocumentBytes: 4096 });
  t.after(async () => {
    for (const bridge of owners) await bridge.stop();
    assert.equal(resolve(base), base); assert.equal(await fs.realpath(base), base);
    const actual = await fs.lstat(base); assert.equal(actual.ino, original.ino); assert.equal(actual.dev, original.dev);
    await fs.rm(base, { recursive: true });
  });
  const first = createCodexUsageBridge({ openStorage }); owners.push(first);
  const saved = await first.handle(save(0, { composerDisplay: 'progressBar', usageAlert: 'custom', customRemainingPercent: 7.25 }));
  await first.stop();
  const next = createCodexUsageBridge({ openStorage }); owners.push(next);
  assert.deepEqual(await next.handle(load), saved);
});
