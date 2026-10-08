import test from 'node:test';
import assert from 'node:assert/strict';
import { createAccountObserver } from './fork-supervisor.mjs';
const tick = () => new Promise(resolve => setImmediate(resolve));
const deferred = () => { let resolve; const promise = new Promise(done => { resolve = done; }); return { promise, resolve }; };
const state = { status: 'credential-stored', links: { usageUrl: 'https://platform.example/usage', topUpUrl: 'https://platform.example/topup' }, attempt: null };

test('unsubscribe revokes immediately and retains held initial state read until actual settlement', { timeout: 2000 }, async () => {
  const read = deferred(); let stateSettled = false, stopped = false;
  const packets = [];
  const observer = createAccountObserver({ send(packet) { packets.push(packet); } });
  observer.attach({
    getState: async () => read.promise,
    getPlatformSession: async () => null,
    async *watch(signal) {
      yield state;
      if (!signal.aborted) await new Promise(resolve => signal.addEventListener('abort', resolve, { once: true }));
    },
  });
  const pending = observer.subscribe().finally(() => { stateSettled = true; });
  const rejected = assert.rejects(pending);
  try {
    await tick();
    const ending = observer.unsubscribe().then(() => { stopped = true; });
    assert.deepEqual(packets.at(-1), { type: 'platform-session', session: null });
    await tick();
    assert.equal(stateSettled, false); assert.equal(stopped, false);
    await assert.rejects(observer.subscribe());
    read.resolve(state); await rejected; await ending;
    assert.equal(stopped, true);
    assert.deepEqual(packets.at(-1), { type: 'platform-session', session: null });
  } finally { read.resolve(state); await observer.dispose(); }
});

test('token and every header are registered before private publication and provider failure clears that owner', { timeout: 2000 }, async () => {
  const registered = [], packets = [], fail = deferred();
  const session = { origin: 'https://platform.example', token: 'PUBLIC_FIXTURE_TOKEN', userId: null,
    requestHeaders: { cookie: 'PUBLIC_FIXTURE_COOKIE', 'x-fixture': 'PUBLIC_FIXTURE_HEADER' } };
  const observer = createAccountObserver({ addSecret: value => registered.push(value), send(packet) {
    if (packet.type === 'platform-session' && packet.session) assert.deepEqual(registered, [session.token, ...Object.values(session.requestHeaders)]);
    packets.push(packet);
  } });
  observer.attach({
    getState: async () => state,
    getPlatformSession: async () => session,
    async *watch() { yield state; await fail.promise; throw new Error('PUBLIC_ERROR_NEVER_FORWARDED'); },
  });
  try {
    await observer.subscribe(); await tick();
    assert.equal(packets.at(-1).session, session);
    fail.resolve(); await tick();
    assert.deepEqual(packets.at(-1), { type: 'platform-session', session: null });
    assert.equal(packets.filter(packet => packet.type === 'account-subscription-error').length, 1);
    assert.equal(JSON.stringify(packets).includes('PUBLIC_ERROR_NEVER_FORWARDED'), false);
  } finally { fail.resolve(); await observer.dispose(); }
});
