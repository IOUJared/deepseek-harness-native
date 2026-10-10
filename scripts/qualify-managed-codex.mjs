/** Real unchanged base+web alpha profile and native bridge; PUBLIC stored OAuth fixtures only. */
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { chmod, mkdir, readFile, realpath, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { startForkSupervisor, loadForkRuntime } from '../core/runtime/fork-supervisor.mjs';
import { interceptDiagnostics } from '../core/runtime/supervisor.mjs';

const output = process.env.DSH_NATIVE_CODEX_EVIDENCE;
if (!output || resolve(output) !== output || process.env.DSH_TAURI_HOME !== join(output, 'harness')) throw new Error('private-output-required');
const stage = process.env.DSH_NATIVE_CODEX_STAGE;
if (!['initial', 'restored'].includes(stage)) throw new Error('explicit-stage-required');
await mkdir(output, { recursive: true, mode: 0o700 });
if (await realpath(output) !== output) throw new Error('canonical-output-required');
const home = join(output, 'harness');
await mkdir(join(home, 'codex-accounts'), { recursive: true, mode: 0o700 });
await chmod(home, 0o700);
const a = { opaqueId: 'account-11111111-1111-4111-8111-111111111111', label: 'PUBLIC first', email: null, plan: null };
const b = { opaqueId: 'account-22222222-2222-4222-8222-222222222222', label: 'PUBLIC second', email: null, plan: null };
const grant = id => ({ kind: 'grant', payload: { type: 'oauth', access: `PUBLIC-access-${id}`, refresh: `PUBLIC-refresh-${id}`,
  expires: 4_102_444_800_000, accountId: `PUBLIC-issuer-${id}`, extra: { untouched: ['PUBLIC-extra', null] } } });
const seed = { version: 1, manifest: { accounts: [a, b], selected: null, epoch: 0 }, records: [a, b].map((account, i) => ({
  key: `llm-pi-ai-codex-accounts/${account.opaqueId}`, record: grant(String(i)),
})) };
if (stage === 'initial') await writeFile(join(home, 'codex-accounts', 'accounts.v1.json'), JSON.stringify(seed) + '\n', { flag: 'wx', mode: 0o600 });
const originalFetch = globalThis.fetch;
let externalFetches = 0;
globalThis.fetch = async (input, init) => {
  const url = new URL(typeof input === 'string' ? input : input instanceof URL ? input.href : input.url);
  if (!['127.0.0.1', 'localhost', '[::1]'].includes(url.hostname)) {
    externalFetches++;
    throw new Error('PUBLIC external network denied');
  }
  return originalFetch(input, init);
};
const diagnostics = interceptDiagnostics(process.stdout, process.stderr);
const report = { status: 'failed', stage, phase: 'boot', composedBoots: 0, commands: [], publicFrames: 0, externalFetches: 0,
  generationRequests: 0, oauthLogins: 0, issuerRevocation: false, sdkGrantSeeding: 'PUBLIC synthetic stored grants, no real sign-in',
  scope: 'built alpha supported runProfile/resolvedProfile, unchanged full base+web bundles, native initializer and private bridge; no Rust/App physical input' };
const publicFrames = [];
const runtimeValues = { transport: 'sse', cacheRetention: 'none', timeoutMs: 17000,
  websocketConnectTimeoutMs: 9000, streamIdleTimeoutMs: 32000,
  requestImagePixelBudget: 1048576, requestImageMaxBytes: 524288, maxRequestImageBytes: 8388608 };
function runtimeDescriptor() {
  const rows = runtime.ctx.get('llm').listConfigurableProviders().filter(row => row.provider === 'openai-codex');
  assert.equal(rows.length, 1);
  assert.deepEqual(rows[0].settingsPath, ['providers', 'openai-codex']);
  const settings = runtime.ctx.get('settings');
  const descriptor = settings.describe({ redactSecrets: true }).find(row => row.ns === rows[0].settingsNs);
  assert.ok(descriptor);
  return { settings, descriptor, row: rows[0] };
}
let controller;
let runtime;
let next = 0;
const controls = new Map();
let currentInput;
function send(packet) {
  if (packet.type === 'fatal') {
    for (const pending of controls.values()) pending.reject(new Error('PUBLIC native fatal'));
    controls.clear();
    return;
  }
  if (packet.type !== 'ready') publicFrames.push(packet);
  if (!packet.requestId) return;
  const pending = controls.get(packet.requestId);
  if (!pending) return;
  controls.delete(packet.requestId);
  clearTimeout(pending.timer);
  if (packet.type === 'native-result') pending.resolve(packet.value);
  else pending.reject(Object.assign(new Error('PUBLIC native refusal'), {
    code: ['CODEX_RUNTIME_UNAVAILABLE', 'CODEX_RUNTIME_CONFLICT', 'CODEX_RUNTIME_INDETERMINATE'].includes(packet.code) ? packet.code : 'PUBLIC_REFUSAL',
  }));
}
function command(type, fields = {}) {
  const requestId = `public-codex-${++next}`;
  report.commands.push(type);
  const done = new Promise((resolveDone, reject) => {
    const timer = setTimeout(() => { controls.delete(requestId); reject(new Error('PUBLIC command timeout')); }, 15_000);
    controls.set(requestId, { resolve: resolveDone, reject, timer });
  });
  currentInput.emit('data', JSON.stringify({ type, requestId, ...fields }) + '\n');
  return done;
}
async function boot() {
  currentInput = new EventEmitter();
  currentInput.pause = () => {};
  controller = startForkSupervisor({ input: currentInput, send, env: process.env,
    addSecret: value => diagnostics.addSecret(value),
    loadRuntime: async env => {
      const installed = await loadForkRuntime(env);
      const run = installed.runProfile;
      return { ...installed, runProfile: async options => {
        runtime = await run(options);
        const llm = runtime.ctx.get('llm');
        const stream = llm.stream;
        llm.stream = function (...args) { report.generationRequests++; return Reflect.apply(stream, this, args); };
        return runtime;
      } };
    } });
  await controller.boot;
  assert.ok(runtime.ctx.get('codexAccounts'));
  report.composedBoots++;
}
try {
  await boot();
  if (stage === 'initial') {
  report.phase = 'root-description';
  const description = await runtime.ctx.get('codexAccounts').describe();
  report.rootDescription = { accountCount: Array.isArray(description.accounts) ? description.accounts.length : null,
    epoch: description.epoch, selectedNull: description.selected === null,
    expectedFields: ['accounts', 'selected', 'epoch', 'legacy'].every(key => Object.hasOwn(description, key)),
    totalFields: Object.keys(description).length };
  report.phase = 'usage-preferences-first-save';
  const unwritten = await command('codex-usage-load');
  assert.equal(unwritten.revision, 0);
  assert.equal(unwritten.persisted, false);
  assert.equal(unwritten.preferences.experimentalQuota, false);
  const preferences = { ...unwritten.preferences, composerDisplay: 'progressBar',
    usageAlert: 'custom', customRemainingPercent: 12.5 };
  const saved = await command('codex-usage-save', { expectedRevision: 0, preferences });
  assert.equal(saved.version, 1);
  assert.equal(saved.revision, 1);
  assert.equal(saved.persisted, true);
  assert.deepEqual(saved.preferences, preferences);
  await assert.rejects(command('codex-usage-save', { expectedRevision: 0, preferences }));
  assert.deepEqual(await command('codex-usage-load'), saved);
  report.usagePreferences = { persisted: true, revision: 1, betaConsent: false, staleSaveRejected: true };
  report.phase = 'initial-roster';
  const initial = await command('codex-accounts-status');
  report.initial = { mode: initial.mode, epoch: initial.epoch, accountCount: Array.isArray(initial.accounts) ? initial.accounts.length : null,
    firstLabelMatches: initial.accounts?.[0]?.label === a.label, selectedNull: initial.selected === null };
  assert.equal(initial.mode, 'managed');
  assert.equal(initial.epoch, 0);
  assert.equal(initial.selected, null);
  assert.deepEqual(initial.accounts, [a, b]);
  report.phase = 'select-and-route-cas';
  const selected = await command('codex-select', { accountId: a.opaqueId, expectedEpoch: 0 });
  assert.equal(selected.selected, a.opaqueId);
  assert.equal(selected.epoch, 1);
  const status = await command('codex-status');
  assert.equal(status.credentialStored, true);
  const enabled = await command('codex-enable-models', { expectedRevision: status.settingsRevision });
  assert.equal(enabled.routeConfigured, true);
  report.phase = 'runtime-config-public-mutation';
  const fixtureTarget = runtimeDescriptor();
  await fixtureTarget.settings.mutate(fixtureTarget.row.settingsNs,
    [{ op: 'set', path: ['providers', 'openai-codex', 'displayName'], value: 'PUBLIC preserved route label' }], fixtureTarget.descriptor.revision);
  const runtimeBefore = await command('codex-runtime-read');
  assert.equal(runtimeBefore.configured, true);
  assert.equal(runtimeBefore.writable, true);
  const runtimeSaved = await command('codex-runtime-save', { expectedRevision: runtimeBefore.revision, values: runtimeValues });
  assert.deepEqual(runtimeSaved.values, runtimeValues);
  assert.ok(runtimeSaved.revision > runtimeBefore.revision);
  await assert.rejects(command('codex-runtime-save', { expectedRevision: runtimeBefore.revision, values: runtimeValues }));
  assert.deepEqual(await command('codex-runtime-read'), runtimeSaved);
  assert.equal(runtimeDescriptor().descriptor.value.providers['openai-codex'].displayName, 'PUBLIC preserved route label');
  report.runtimeConfig = { saved: true, staleSaveRejected: true, unrelatedFieldPreserved: true, fieldCount: 8 };
  const patch = await readFile(join(home, 'profiles', 'desktop', 'cordis.patch.yml'), 'utf8');
  assert.match(patch, /nativeCodexAccounts:\s*true/);
  assert.match(patch, /openai-codex:/);
  assert.ok(!patch.includes('PUBLIC-access') && !patch.includes('PUBLIC-refresh'));
  report.phase = 'stale-epoch';
  await assert.rejects(command('codex-select', { accountId: b.opaqueId, expectedEpoch: 0 }));
  assert.equal((await command('codex-accounts-status')).selected, a.opaqueId);
  report.phase = 'switch-and-restart';
  const switched = await command('codex-select', { accountId: b.opaqueId, expectedEpoch: 1 });
  report.switched = { selectedMatches: switched.selected === b.opaqueId, epoch: switched.epoch };
  assert.equal(switched.selected, b.opaqueId);
  assert.equal(switched.epoch, 2);
  } else {
  report.phase = 'runtime-config-restoration-and-inheritance';
  const runtimeRestored = await command('codex-runtime-read');
  assert.deepEqual(runtimeRestored.values, runtimeValues);
  assert.equal(runtimeDescriptor().descriptor.value.providers['openai-codex'].displayName, 'PUBLIC preserved route label');
  const inherit = Object.fromEntries(Object.keys(runtimeValues).map(field => [field, null]));
  let runtimeReset;
  try {
    runtimeReset = await command('codex-runtime-save', { expectedRevision: runtimeRestored.revision, values: inherit });
  } catch (error) {
    report.runtimeResetDiagnostic = { refusal: error.code, before: runtimeRestored, after: await command('codex-runtime-read') };
    throw error;
  }
  assert.deepEqual(runtimeReset.values, runtimeReset.baseValues);
  assert.equal(runtimeDescriptor().descriptor.value.providers['openai-codex'].displayName, 'PUBLIC preserved route label');
  report.phase = 'runtime-explicit-default-pin';
  assert.ok(runtimeReset.baseValues.requestImagePixelBudget > 0);
  const pinValues = { ...runtimeReset.overrides, requestImagePixelBudget: runtimeReset.baseValues.requestImagePixelBudget };
  const pinned = await command('codex-runtime-save', { expectedRevision: runtimeReset.revision, values: pinValues });
  assert.equal(pinned.overrides.requestImagePixelBudget, pinned.baseValues.requestImagePixelBudget);
  const sibling = await command('codex-runtime-save', { expectedRevision: pinned.revision,
    values: { ...pinned.overrides, cacheRetention: 'long' } });
  assert.equal(sibling.overrides.requestImagePixelBudget, pinned.overrides.requestImagePixelBudget);
  assert.equal(runtimeDescriptor().descriptor.value.providers['openai-codex'].displayName, 'PUBLIC preserved route label');
  report.runtimeConfig = { restored: true, inheritedReset: true, explicitDefaultPinPreserved: true, unrelatedFieldPreserved: true, fieldCount: 8 };
  report.phase = 'usage-preferences-restoration';
  const preferences = await command('codex-usage-load');
  assert.equal(preferences.revision, 1);
  assert.equal(preferences.persisted, true);
  assert.equal(preferences.preferences.composerDisplay, 'progressBar');
  assert.equal(preferences.preferences.usageAlert, 'custom');
  assert.equal(preferences.preferences.customRemainingPercent, 12.5);
  assert.equal(preferences.preferences.experimentalQuota, false);
  report.usagePreferences = { restored: true, revision: 1, betaConsent: false };
  report.phase = 'restart-restoration';
  const restored = await command('codex-accounts-status');
  assert.equal(restored.selected, b.opaqueId);
  assert.equal(restored.epoch, 2);
  assert.deepEqual(restored.accounts, [a, b]);
  report.phase = 'local-logout-no-fallback';
  const signedOut = await command('codex-signout', { accountId: b.opaqueId, expectedEpoch: 2 });
  assert.equal(signedOut.selected, null);
  assert.deepEqual(signedOut.accounts, [a]);
  assert.equal((await command('codex-status')).credentialStored, false);
  const cleared = await command('codex-signout-all', { expectedEpoch: signedOut.epoch });
  assert.deepEqual(cleared.accounts, []);
  assert.equal(cleared.selected, null);
  const stored = JSON.parse(await readFile(join(home, 'codex-accounts', 'accounts.v1.json'), 'utf8'));
  assert.deepEqual(stored.manifest.accounts, []);
  assert.deepEqual(stored.records, []);
  }
  report.phase = 'privacy-and-shutdown';
  await controller.shutdown();
  controller = undefined;
  assert.equal(report.generationRequests, 0);
  assert.equal(externalFetches, 0);
  const frames = JSON.stringify(publicFrames);
  for (const secret of ['PUBLIC-access-', 'PUBLIC-refresh-', 'PUBLIC-issuer-']) assert.ok(!frames.includes(secret));
  report.status = 'passed';
  report.phase = 'complete';
} catch (_error) {
  // Record a fixed stage only, never upstream stacks, grants, browser URLs or cookies.
} finally {
  try { await controller?.shutdown(); } catch (_error) { report.status = 'failed'; }
  for (const pending of controls.values()) clearTimeout(pending.timer);
  controls.clear();
  globalThis.fetch = originalFetch;
  report.externalFetches = externalFetches;
  report.publicFrames = publicFrames.length;
  await writeFile(join(output, `qualification-${stage}.json`), JSON.stringify(report, null, 2) + '\n', { mode: 0o600, flag: 'wx' });
  diagnostics.restore();
  process.exitCode = report.status === 'passed' ? 0 : 1;
}
