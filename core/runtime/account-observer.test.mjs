import test from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { createAccountObserver, startForkSupervisor } from './fork-supervisor.mjs';

const tick = () => new Promise(resolve => setImmediate(resolve));
const deferred = () => { let resolve; const promise = new Promise(done => { resolve = done; }); return { promise, resolve }; };
const snapshot = () => ({ status: 'signed-out', links: { usageUrl: 'https://platform.example/usage', topUpUrl: 'https://platform.example/topup' }, attempt: null });
function account({ session = async () => null, state = async () => snapshot(), ended = false, failed = false } = {}) {
  const calls = { watch: 0, state: 0, session: 0, ended: 0 };
  let wake;
  const queue = [snapshot()];
  const service = {
    async getState() { calls.state++; return state(); },
    async getPlatformSession() { calls.session++; return session(); },
    async *watch(signal) {
      calls.watch++;
      const abort = () => wake?.();
      signal.addEventListener('abort', abort);
      try {
        if (failed) throw new Error('PUBLIC_FIXTURE_ERROR_MARKER');
        while (!signal.aborted) {
          if (queue.length) yield queue.shift();
          else if (ended) return;
          else await new Promise(resolve => { wake = resolve; });
        }
      } finally { calls.ended++; signal.removeEventListener('abort', abort); }
    },
  };
  return { service, calls, push(value) { queue.push(value); wake?.(); } };
}
function observer() {
  const packets = [], secrets = [];
  const controller = createAccountObserver({ send: packet => packets.push(packet), addSecret: secret => secrets.push(secret) });
  return { controller, packets, secrets };
}
function supervisor(fixtureAccount = account()) {
  const input = new EventEmitter(); input.pause = () => {};
  const packets = [], secrets = [], disposers = [];
  let inject;
  const ctx = {
    webServer: { host: '127.0.0.1', port: 34567 },
    connection: { authenticatedUrl: base => `${base}/?token=PUBLIC_FIXTURE_LAUNCH_TOKEN` },
    deepseekAccount: fixtureAccount.service,
    settings: { describe: () => [{ ns: 'llm-deepseek', value: { apiKeyEnv: 'PUBLIC_FIXTURE_KEY_REF' } }] },
    credentials: { describe: async () => ({ configured: false, writable: true }) },
    llm: { listConfigurableProviders: () => [] },
    get(name) { return this[name]; },
    inject(deps, callback) {
      assert.deepEqual(deps, ['deepseekAccount']);
      inject = callback;
      callback({ deepseekAccount: this.deepseekAccount, effect: setup => disposers.push(setup()) });
      return Promise.resolve();
    },
  };
  const installed = {
    version: '0.2.1-alpha.1', environment: {}, resolvedProfile: {}, patchFiles: [], optionalPayloads: [],
    credentialRef: value => value,
    runProfile: async request => {
      assert.equal(request.profile, 'desktop');
      return { ctx, shutdown: { shutdown: async () => { for (const dispose of disposers.splice(0)) await dispose(); } } };
    },
  };
  const controller = startForkSupervisor({ input, send: packet => packets.push(packet), addSecret: secret => secrets.push(secret),
    loadRuntime: async () => installed, env: {}, onStopped: () => {}, onFatal: () => {} });
  return { controller, packets, secrets, command(value) { input.emit('data', `${JSON.stringify(value)}\n`); },
    async replace(replacement) {
      for (const dispose of disposers.splice(0)) await dispose();
      ctx.deepseekAccount = replacement.service;
      inject({ deepseekAccount: replacement.service, effect: setup => disposers.push(setup()) });
    } };
}

test('native supervisor boot and one-shot onboarding do not start observation or mirror Platform credentials', { timeout: 2000 }, async () => {
  const provider = account();
  const f = supervisor(provider);
  try {
    await f.controller.boot;
    assert.equal(f.packets.filter(packet => packet.type === 'ready').length, 1);
    assert.deepEqual(provider.calls, { watch: 0, state: 0, session: 0, ended: 0 });
    assert.deepEqual(f.secrets, ['PUBLIC_FIXTURE_LAUNCH_TOKEN']);
    f.command({ type: 'onboarding-read', requestId: 'read1' });
    await tick();
    assert.equal(f.packets.find(packet => packet.requestId === 'read1')?.value.loggedIn, false);
    assert.equal(provider.calls.state, 1);
    assert.equal(provider.calls.watch, 0);
    assert.equal(provider.calls.session, 0);
    assert.equal(f.packets.some(packet => packet.type === 'platform-session'), false);
  } finally { await f.controller.shutdown(); }
});

test('explicit repeated subscription owns one watch and unsubscribe awaits iterator retirement', { timeout: 2000 }, async () => {
  const provider = account(); const f = observer(); const detach = f.controller.attach(provider.service);
  try {
    await Promise.all([f.controller.subscribe(), f.controller.subscribe()]); await tick();
    assert.equal(provider.calls.watch, 1); assert.equal(provider.calls.session, 1);
    assert.equal(f.packets.filter(packet => packet.type === 'account-state').length, 1);
    await f.controller.unsubscribe();
    assert.equal(provider.calls.ended, 1);
    const count = f.packets.length;
    provider.push(snapshot()); await tick();
    assert.equal(f.packets.length, count);
    await f.controller.unsubscribe(); assert.equal(f.packets.length, count);
    await f.controller.subscribe(); await tick(); assert.equal(provider.calls.watch, 2);
    await detach();
    const after = f.packets.length;
    await tick(); assert.equal(f.packets.length, after);
    await assert.rejects(f.controller.subscribe());
  } finally { await f.controller.dispose(); }
});

test('cancellation before watch intake avoids provider iterator and session reads', { timeout: 2000 }, async () => {
  const provider = account(); const f = observer(); f.controller.attach(provider.service);
  try {
    const first = f.controller.subscribe();
    const ending = f.controller.unsubscribe();
    await assert.rejects(first); await ending;
    assert.equal(provider.calls.watch, 0); assert.equal(provider.calls.session, 0);
    assert.equal(provider.calls.state, 0);
    assert.equal(f.secrets.length, 0);
  } finally { await f.controller.dispose(); }
});

test('blocked unabortable session read remains owned and cannot publish late secrets or start a replacement', { timeout: 2000 }, async () => {
  const entered = deferred(), release = deferred();
  const old = account({ session: async () => { entered.resolve(); return release.promise; } });
  const fresh = account({ session: async () => ({ origin: 'https://platform.example', token: 'PUBLIC_NEW_FIXTURE_TOKEN', userId: null }) });
  const f = observer(); const oldDetach = f.controller.attach(old.service);
  let stopped = false;
  try {
    await f.controller.subscribe(); await entered.promise;
    const ending = f.controller.unsubscribe().then(() => { stopped = true; });
    assert.deepEqual(f.packets.at(-1), { type: 'platform-session', session: null });
    f.controller.attach(fresh.service);
    await assert.rejects(f.controller.subscribe()); await tick();
    assert.equal(stopped, false); assert.equal(fresh.calls.watch, 0);
    release.resolve({ origin: 'https://platform.example', token: 'PUBLIC_OLD_FIXTURE_TOKEN', userId: null, requestHeaders: { cookie: 'PUBLIC_OLD_FIXTURE_COOKIE' } });
    await ending;
    assert.equal(old.calls.ended, 1); assert.deepEqual(f.secrets, []);
    await f.controller.subscribe(); await tick();
    const current = f.packets.at(-1);
    assert.equal(current.session.token, 'PUBLIC_NEW_FIXTURE_TOKEN');
    await oldDetach(); assert.equal(f.packets.at(-1), current);
    assert.deepEqual(f.secrets, ['PUBLIC_NEW_FIXTURE_TOKEN']);
  } finally { release.resolve(null); await f.controller.dispose(); }
});

for (const failed of [false, true]) test(`ended=${!failed} failed=${failed} watch is optional unavailable without automatic restart`, { timeout: 2000 }, async () => {
  const provider = account({ ended: !failed, failed }); const f = observer(); f.controller.attach(provider.service);
  try {
    await f.controller.subscribe().catch(() => {}); await tick();
    assert.equal(provider.calls.watch, 1);
    assert.equal(f.packets.filter(packet => packet.type === 'account-subscription-error').length, 1);
    assert.equal(JSON.stringify(f.packets).includes('PUBLIC_FIXTURE_ERROR_MARKER'), false);
    await tick(); assert.equal(provider.calls.watch, 1);
    await f.controller.subscribe().catch(() => {}); await tick(); assert.equal(provider.calls.watch, 2);
  } finally { await f.controller.dispose(); }
});

test('provider disposal invalidates exact follow and requires a new explicit subscription', { timeout: 2000 }, async () => {
  const first = account(), fresh = account(); const f = supervisor(first);
  try {
    await f.controller.boot;
    f.command({ type: 'account-subscribe', requestId: 'sub1' }); await tick();
    assert.equal(first.calls.watch, 1);
    await f.replace(fresh); await tick();
    assert.equal(first.calls.ended, 1); assert.equal(fresh.calls.watch, 0);
    assert.equal(f.packets.some(packet => packet.type === 'account-subscription-error'), true);
    f.command({ type: 'account-subscribe', requestId: 'sub2' }); await tick();
    assert.equal(fresh.calls.watch, 1);
    assert.equal(f.packets.find(packet => packet.requestId === 'sub2')?.type, 'native-result');
  } finally { await f.controller.shutdown(); }
});

test('retiring owner rejects resubscribe during blocked unsubscribe and shutdown joins the held owner', { timeout: 2000 }, async () => {
  const entered = deferred(), release = deferred();
  const provider = account({ session: async () => { entered.resolve(); return release.promise; } });
  const f = supervisor(provider);
  try {
    await f.controller.boot;
    f.command({ type: 'account-subscribe', requestId: 'sub1' }); await entered.promise; await tick();
    f.command({ type: 'account-unsubscribe', requestId: 'end1' });
    f.command({ type: 'account-subscribe', requestId: 'sub2' }); await tick();
    assert.equal(f.packets.find(packet => packet.requestId === 'sub2')?.type, 'request-error');
    assert.equal(provider.calls.watch, 1);
    let joined = false;
    const ending = f.controller.shutdown().then(() => { joined = true; });
    await tick(); assert.equal(joined, false);
    const before = f.packets.length;
    release.resolve({ origin: 'https://platform.example', token: 'PUBLIC_LATE_FIXTURE_TOKEN', userId: null });
    await ending;
    assert.equal(provider.calls.ended, 1);
    assert.equal(f.secrets.includes('PUBLIC_LATE_FIXTURE_TOKEN'), false);
    assert.deepEqual(f.packets.slice(before).map(packet => packet.type), ['platform-session', 'shutdown-complete']);
    assert.equal(f.packets.some(packet => packet.requestId === 'end1'), false);
  } finally { release.resolve(null); await f.controller.shutdown(); }
});

test('priority unsubscribe revokes a pending initial status read without early ACK or stale subscribe result', { timeout: 2000 }, async () => {
  const entered = deferred(), release = deferred();
  const provider = account({ state: async () => { entered.resolve(); return release.promise; } });
  const f = supervisor(provider);
  try {
    await f.controller.boot;
    f.command({ type: 'account-subscribe', requestId: 'held-sub' });
    await entered.promise; await tick();
    f.command({ type: 'account-unsubscribe', requestId: 'priority-end' });
    assert.deepEqual(f.packets.at(-1), { type: 'platform-session', session: null });
    await tick();
    assert.equal(provider.calls.ended, 1);
    assert.equal(f.packets.some(packet => packet.requestId === 'priority-end'), false);
    f.command({ type: 'account-subscribe', requestId: 'replacement' }); await tick();
    assert.equal(f.packets.find(packet => packet.requestId === 'replacement')?.type, 'request-error');
    assert.equal(provider.calls.watch, 1); assert.equal(provider.calls.state, 1);
    release.resolve(snapshot()); await tick();
    assert.equal(f.packets.find(packet => packet.requestId === 'held-sub')?.type, 'request-error');
    const ended = f.packets.find(packet => packet.requestId === 'priority-end');
    assert.equal(ended?.type, 'native-result'); assert.deepEqual(ended.value, { subscribed: false });
    f.command({ type: 'account-subscribe', requestId: 'fresh-sub' }); await tick();
    assert.equal(f.packets.find(packet => packet.requestId === 'fresh-sub')?.type, 'native-result');
    assert.equal(provider.calls.watch, 2);
  } finally { release.resolve(snapshot()); await f.controller.shutdown(); }
});
