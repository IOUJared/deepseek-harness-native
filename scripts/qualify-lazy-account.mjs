// Real public alpha profile; counters begin after profile boot, before native binding.
// Only local state/onboarding/subscribe/unsubscribe: no sign-in, key save, models or network action scripted.
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { startForkSupervisor, loadForkRuntime } from '../core/runtime/fork-supervisor.mjs';
import { interceptDiagnostics } from '../core/runtime/supervisor.mjs';

const output = process.env.DSH_NATIVE_ACCOUNT_EVIDENCE;
if (!output || resolve(output) !== output || process.env.DSH_TAURI_HOME !== join(output, 'harness')) throw new Error('private-output-required');
const diagnostics = interceptDiagnostics(process.stdout, process.stderr);
const input = new EventEmitter(); input.pause = () => {};
const counts = { watch: 0, state: 0, session: 0, live: 0, maxLive: 0, ended: 0 };
const controls = new Map();
const privateWaits = new Map();
function privateCount(count) {
  if (events.platform >= count) return Promise.resolve();
  return new Promise(resolve => privateWaits.set(count, resolve));
}
const events = { account: 0, platform: 0, nonnullPlatform: 0, observationErrors: 0 };
let next = 0, controller, service, originals, phase = 'boot', status = 'failed', fatal = false;
const snapshots = {};
const tick = () => new Promise(done => setImmediate(done));
function send(packet) {
  if (packet.type === 'fatal') { fatal = true; for (const control of controls.values()) control.reject(new Error('native-fatal')); }
  if (packet.type === 'account-state') events.account++;
  if (packet.type === 'platform-session') {
    events.platform++; if (packet.session) events.nonnullPlatform++;
    const wake = privateWaits.get(events.platform);
    if (wake) { privateWaits.delete(events.platform); wake(); }
  }
  if (packet.type === 'account-subscription-error') events.observationErrors++;
  if (packet.requestId) {
    const control = controls.get(packet.requestId);
    if (control) {
      controls.delete(packet.requestId);
      if (packet.type === 'native-result') control.resolve(packet.value);
      else control.reject(new Error('native-request-failed'));
    }
  }
}
function command(type) {
  const requestId = `public-native-audit-${++next}`;
  const promise = new Promise((resolve, reject) => controls.set(requestId, { resolve, reject }));
  input.emit('data', `${JSON.stringify({ type, requestId })}\n`);
  return promise;
}
function capture() { return { ...counts, accountEvents: events.account, platformEvents: events.platform }; }
try {
  controller = startForkSupervisor({ input, send, env: process.env, addSecret: value => diagnostics.addSecret(value),
    onStopped() {}, onFatal() { fatal = true; },
    loadRuntime: async env => {
      const installed = await loadForkRuntime(env);
      const originalRun = installed.runProfile;
      return { ...installed, runProfile: async request => {
        const runtime = await originalRun(request);
        service = runtime.ctx.get('deepseekAccount');
        assert.ok(service);
        originals = { watch: service.watch, getState: service.getState, getPlatformSession: service.getPlatformSession };
        service.getState = function (...args) { counts.state++; return Reflect.apply(originals.getState, this, args); };
        service.getPlatformSession = function (...args) { counts.session++; return Reflect.apply(originals.getPlatformSession, this, args); };
        service.watch = async function* (...args) {
          counts.watch++; counts.live++; counts.maxLive = Math.max(counts.maxLive, counts.live);
          try { yield* Reflect.apply(originals.watch, this, args); }
          finally { counts.live--; counts.ended++; }
        };
        return runtime;
      } };
    } });
  await controller.boot; assert.equal(fatal, false); await tick();
  snapshots.ready = capture();
  assert.deepEqual(counts, { watch: 0, state: 0, session: 0, live: 0, maxLive: 0, ended: 0 });
  phase = 'local-metadata';
  const onboarding = await command('onboarding-read');
  assert.equal(onboarding.loggedIn, false); assert.equal(onboarding.hasApiKey, false); assert.equal(onboarding.writable, true);
  const state = await command('account-state'); assert.equal(state.status, 'signed-out');
  snapshots.metadata = capture();
  assert.equal(counts.watch, 0); assert.equal(counts.session, 0); assert.equal(events.platform, 0);
  phase = 'explicit-subscribe';
  const first = await command('account-subscribe'); assert.equal(first.status, 'signed-out');
  await privateCount(1);
  assert.equal(counts.watch, 1); assert.equal(counts.session, 1); assert.equal(events.account, 1);
  await command('account-subscribe'); assert.equal(counts.watch, 1);
  snapshots.subscribed = capture();
  phase = 'explicit-unsubscribe';
  assert.deepEqual(await command('account-unsubscribe'), { subscribed: false });
  assert.equal(counts.live, 0); assert.equal(counts.ended, 1);
  snapshots.unsubscribed = capture(); await tick();
  assert.deepEqual(capture(), snapshots.unsubscribed);
  phase = 'explicit-resubscribe';
  await command('account-subscribe');
  await privateCount(snapshots.unsubscribed.platformEvents + 1);
  assert.equal(counts.watch, 2); assert.equal(counts.session, 2); assert.equal(counts.maxLive, 1);
  phase = 'shutdown'; await controller.shutdown();
  assert.equal(counts.live, 0); assert.equal(counts.ended, 2);
  assert.equal(events.nonnullPlatform, 0); assert.equal(events.observationErrors, 0); assert.equal(fatal, false);
  snapshots.stopped = capture(); status = 'passed'; phase = 'complete';
} catch (_error) {
  // Fixed stage only: no source exception, auth grant, token, URL, cookie or profile body.
} finally {
  try { await controller?.shutdown(); } catch (_error) { status = 'failed'; }
  if (service && originals) for (const [name, method] of Object.entries(originals)) service[name] = method;
  const report = { status, phase, scope: 'actual public base+web alpha Loader/Cordis/native supervisor; counters attached after profile boot, not whole-runtime or Rust-worker trace',
    instrumentation: 'same loaded account instance, method delegates preserve arguments/this and iterator return; no provider substituted',
    snapshots, counts, events, fatal, scriptedSignIn: false, scriptedModelOrCatalog: false, scriptedCredentialSave: false,
    providerHotReplacementQualified: false, processWideNetworkTrace: false };
  await writeFile(join(output, 'account.json'), `${JSON.stringify(report, null, 2)}\n`, { mode: 0o600, flag: 'wx' });
  diagnostics.restore(); process.exitCode = status === 'passed' ? 0 : 1;
}
