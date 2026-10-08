import test from 'node:test';
import assert from 'node:assert/strict';
import { createAccountObserver } from './fork-supervisor.mjs';
const tick = () => new Promise(resolve => setImmediate(resolve));
const deferred = () => { let resolve, reject; const promise = new Promise((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; };
const state = { status: 'signed-out', links: { usageUrl: 'https://platform.example/usage', topUpUrl: 'https://platform.example/topup' }, attempt: null };

for (const mode of ['unsubscribe', 'detach', 'dispose']) test(`${mode} joins all admitted status reads, including a late failure`, { timeout: 2000 }, async () => {
  const reads = [deferred(), deferred()]; let calls = 0, ended = false, joined = false;
  const packets = [];
  const observer = createAccountObserver({ send(packet) { packets.push(packet); } });
  const detach = observer.attach({
    getState: async () => reads[calls++].promise,
    getPlatformSession: async () => null,
    async *watch(signal) {
      try { yield state; if (!signal.aborted) await new Promise(resolve => signal.addEventListener('abort', resolve, { once: true })); }
      finally { ended = true; }
    },
  });
  const first = assert.rejects(observer.subscribe());
  const second = assert.rejects(observer.subscribe());
  try {
    await tick(); assert.equal(calls, 2);
    const stop = (mode === 'detach' ? detach() : mode === 'dispose' ? observer.dispose() : observer.unsubscribe()).then(() => { joined = true; });
    assert.deepEqual(packets.at(-1), { type: 'platform-session', session: null });
    await tick(); assert.equal(ended, true); assert.equal(joined, false);
    reads[0].resolve(state); await tick(); assert.equal(joined, false);
    await assert.rejects(observer.subscribe()); assert.equal(calls, 2);
    reads[1].reject(new Error('PUBLIC_FAILURE_NOT_PROJECTED'));
    await Promise.all([first, second, stop]); assert.equal(joined, true);
    assert.equal(JSON.stringify(packets).includes('PUBLIC_FAILURE_NOT_PROJECTED'), false);
    assert.deepEqual(packets.at(-1), { type: 'platform-session', session: null });
  } finally { for (const read of reads) read.resolve(state); await observer.dispose(); }
});

test('a rejected read revokes observation and waits its held peer without explicit stop', { timeout: 2000 }, async () => {
  const reads = [deferred(), deferred()]; let calls = 0, firstFinished = false;
  const packets = [];
  const observer = createAccountObserver({ send(packet) { packets.push(packet); } });
  observer.attach({
    getState: async () => reads[calls++].promise, getPlatformSession: async () => null,
    async *watch(signal) { yield state; if (!signal.aborted) await new Promise(resolve => signal.addEventListener('abort', resolve, { once: true })); },
  });
  const first = assert.rejects(observer.subscribe()).then(() => { firstFinished = true; });
  const second = assert.rejects(observer.subscribe());
  try {
    await tick(); reads[0].reject(new Error('PUBLIC_READ_FAILURE')); await tick();
    assert.deepEqual(packets.at(-1), { type: 'platform-session', session: null });
    assert.equal(firstFinished, false);
    await assert.rejects(observer.subscribe()); assert.equal(calls, 2);
    reads[1].resolve(state); await Promise.all([first, second]);
    assert.equal(firstFinished, true);
  } finally { for (const read of reads) read.resolve(state); await observer.dispose(); }
});

for (const failed of [false, true]) test(`natural watch termination failed=${failed} retains its held initial read`, { timeout: 2000 }, async () => {
  const read = deferred(); let calls = 0, settled = false;
  const packets = [];
  const observer = createAccountObserver({ send(packet) { packets.push(packet); } });
  observer.attach({
    getState: async () => { calls++; return read.promise; }, getPlatformSession: async () => null,
    async *watch() { yield state; if (failed) throw new Error('PUBLIC_WATCH_FAILURE'); },
  });
  const pending = assert.rejects(observer.subscribe()).then(() => { settled = true; });
  try {
    await tick(); assert.equal(settled, false);
    assert.equal(packets.filter(packet => packet.type === 'account-subscription-error').length, 1);
    await assert.rejects(observer.subscribe()); assert.equal(calls, 1);
    read.resolve(state); await pending;
    assert.deepEqual(packets.at(-1), { type: 'platform-session', session: null });
  } finally { read.resolve(state); await observer.dispose(); }
});

test('provider replacement cannot overtake a held initial read or publish its late state', { timeout: 2000 }, async () => {
  const read = deferred(); let oldCalls = 0, freshCalls = 0;
  const packets = [];
  const observer = createAccountObserver({ send(packet) { packets.push(packet); } });
  const watch = async function* (signal) { yield state; if (!signal.aborted) await new Promise(resolve => signal.addEventListener('abort', resolve, { once: true })); };
  const oldDetach = observer.attach({ getState: async () => { oldCalls++; return read.promise; }, getPlatformSession: async () => null, watch });
  const pending = assert.rejects(observer.subscribe());
  try {
    await tick();
    observer.attach({ getState: async () => { freshCalls++; return state; }, getPlatformSession: async () => null, watch });
    await assert.rejects(observer.subscribe()); assert.equal(freshCalls, 0);
    read.resolve(state); await pending;
    await observer.subscribe(); await tick();
    assert.equal(oldCalls, 1); assert.equal(freshCalls, 1);
    const count = packets.length;
    await oldDetach(); assert.equal(packets.length, count);
  } finally { read.resolve(state); await observer.dispose(); }
});
