// PUBLIC, in-memory durable-CAS fixtures only; never read or write user settings.
import test from 'node:test';
import assert from 'node:assert/strict';
import { inspect } from 'node:util';
import {
  defaultCodexUsagePreferences,
  validateCodexUsagePreferences,
  CodexUsagePreferencesError,
  createCodexUsagePreferencesStore,
} from './codex-usage-preferences.mjs';

const LIMIT = 4096;
const defaults = () => ({
  composerDisplay: 'off', usageAlert: 'off', customRemainingPercent: 20,
  retryAfterReset: false, experimentalQuota: false,
});
const prefs = (changes = {}) => ({ ...defaults(), ...changes });
const doc = (revision = 0, preferences = prefs()) => ({ version: 1, revision, preferences });
const clone = (value) => value === null ? null : JSON.parse(JSON.stringify(value));
const utf8 = (value) => new TextEncoder().encode(value);
const deferred = () => {
  let resolve;
  const promise = new Promise((done) => { resolve = done; });
  return { promise, resolve };
};

function hasCode(code) {
  return (error) => {
    assert.ok(error instanceof CodexUsagePreferencesError);
    assert.equal(error.code, code);
    assert.equal(Object.hasOwn(error, 'cause'), false);
    assert.ok(Object.isFrozen(error));
    return true;
  };
}

function durableFixture(initial = null, overrides = {}) {
  let durable = clone(initial);
  let reads = 0;
  const calls = [];
  const readDocument = () => {
    reads += 1;
    return clone(durable);
  };
  const commitDocument = ({ expectedRevision, document }) => {
    calls.push({ expectedRevision, document });
    if ((durable?.revision ?? 0) !== expectedRevision) return { status: 'conflict' };
    durable = clone(document);
    return { status: 'committed' };
  };
  const store = createCodexUsagePreferencesStore({
    readDocument, commitDocument, maxPendingUpdates: 4, maxDocumentBytes: LIMIT,
    ...overrides,
  });
  return {
    store, calls,
    read: () => clone(durable),
    externalWrite: (value) => { durable = clone(value); },
    reads: () => reads,
    readDocument, commitDocument,
  };
}

function storeForRead(value, maxDocumentBytes = LIMIT) {
  let commits = 0;
  const store = createCodexUsagePreferencesStore({
    readDocument: () => value,
    commitDocument: () => { commits += 1; return { status: 'committed' }; },
    maxPendingUpdates: 1, maxDocumentBytes,
  });
  return { store, commits: () => commits };
}

test('defaults are exact, newly allocated and immutable', () => {
  const first = defaultCodexUsagePreferences();
  const second = defaultCodexUsagePreferences();
  assert.deepEqual(first, defaults());
  assert.notEqual(first, second);
  assert.ok(Object.isFrozen(first));
  assert.throws(() => { first.retryAfterReset = true; }, TypeError);
});

test('all enum combinations, fractional boundaries and strict booleans validate', () => {
  for (const composerDisplay of ['off', 'percent', 'progressBar', 'runway']) {
    for (const usageAlert of ['off', 'percent20', 'earlyWarning', 'custom']) {
      for (const customRemainingPercent of [0, 0.125, 20.75, 99.999, 100]) {
        for (const enabled of [false, true]) {
          const input = prefs({ composerDisplay, usageAlert, customRemainingPercent,
            retryAfterReset: enabled, experimentalQuota: enabled });
          const output = validateCodexUsagePreferences(input);
          assert.deepEqual(output, input);
          assert.notEqual(output, input);
          assert.ok(Object.isFrozen(output));
        }
      }
    }
  }
});

test('validation rejects coercion, non-finite values, missing keys and extras', () => {
  for (const value of [undefined, null, [], true, 'PUBLIC', 1]) {
    assert.throws(() => validateCodexUsagePreferences(value), hasCode('INVALID_PREFERENCES'));
  }
  for (const customRemainingPercent of [-1, 100.01, NaN, Infinity, -Infinity, '20', null]) {
    assert.throws(() => validateCodexUsagePreferences(prefs({ customRemainingPercent })),
      hasCode('INVALID_PREFERENCES'));
  }
  for (const key of ['retryAfterReset', 'experimentalQuota']) {
    for (const value of [0, 1, 'false', null, undefined]) {
      assert.throws(() => validateCodexUsagePreferences(prefs({ [key]: value })),
        hasCode('INVALID_PREFERENCES'));
    }
  }
  for (const key of ['composerDisplay', 'usageAlert']) {
    assert.throws(() => validateCodexUsagePreferences(prefs({ [key]: 'PUBLIC-秘密-🔐' })),
      hasCode('INVALID_PREFERENCES'));
  }
  for (const key of Object.keys(defaults())) {
    const missing = prefs();
    delete missing[key];
    assert.throws(() => validateCodexUsagePreferences(missing), hasCode('INVALID_PREFERENCES'));
  }
  assert.throws(() => validateCodexUsagePreferences(prefs({ extra: 'PUBLIC' })),
    hasCode('INVALID_PREFERENCES'));
  const symbol = prefs();
  symbol[Symbol('PUBLIC')] = true;
  assert.throws(() => validateCodexUsagePreferences(symbol), hasCode('INVALID_PREFERENCES'));
});

test('validation does not invoke accessors, toJSON or inherited fields', () => {
  let invoked = 0;
  const accessor = prefs();
  Object.defineProperty(accessor, 'composerDisplay', {
    enumerable: true, get() { invoked += 1; throw new Error('PUBLIC'); },
  });
  const serialized = prefs({ toJSON() { invoked += 1; throw new Error('PUBLIC'); } });
  const inherited = Object.create(defaults());
  const hidden = prefs();
  Object.defineProperty(hidden, 'usageAlert', { enumerable: false });
  for (const input of [accessor, serialized, inherited, hidden]) {
    assert.throws(() => validateCodexUsagePreferences(input), hasCode('INVALID_PREFERENCES'));
  }
  assert.equal(invoked, 0);
  const nullPrototype = Object.assign(Object.create(null), defaults());
  assert.deepEqual(validateCodexUsagePreferences(nullPrototype), defaults());
  const hostile = new Proxy({}, { getPrototypeOf() { throw new Error('PUBLIC-秘密'); } });
  assert.throws(() => validateCodexUsagePreferences(hostile), hasCode('INVALID_PREFERENCES'));
});

test('safe diagnostics never retain Unicode inputs, thrown text or constructor causes', async () => {
  const payload = 'PUBLIC-秘密-🔐\nraw text';
  const invalid = prefs({ composerDisplay: payload });
  let error;
  try { validateCodexUsagePreferences(invalid); } catch (caught) { error = caught; }
  assert.ok(error);
  for (const text of [error.message, String(error), error.stack, JSON.stringify(error), inspect(error)]) {
    assert.ok(!text.includes(payload));
    assert.ok(!text.includes('秘密'));
  }
  const unknown = new CodexUsagePreferencesError(payload, { cause: new Error(payload) });
  assert.equal(unknown.code, 'INTERNAL_ERROR');
  assert.equal(Object.hasOwn(unknown, 'cause'), false);
  const fixture = durableFixture(null, { readDocument() { throw new Error(payload); } });
  await assert.rejects(fixture.store.load(), hasCode('READ_FAILED'));
});

test('factory is inert and requires explicit bounded ports and limits', () => {
  const fixture = durableFixture();
  assert.equal(fixture.reads(), 0);
  assert.equal(fixture.calls.length, 0);
  const options = {
    readDocument: () => null, commitDocument: () => ({ status: 'committed' }),
    maxPendingUpdates: 1, maxDocumentBytes: LIMIT,
  };
  for (const key of Object.keys(options)) {
    const missing = { ...options };
    delete missing[key];
    assert.throws(() => createCodexUsagePreferencesStore(missing), hasCode('INVALID_OPTIONS'));
  }
  for (const key of ['maxPendingUpdates', 'maxDocumentBytes']) {
    for (const value of [0, -1, 0.5, '1', NaN, Infinity, Number.MAX_SAFE_INTEGER + 1]) {
      assert.throws(() => createCodexUsagePreferencesStore({ ...options, [key]: value }),
        hasCode('INVALID_OPTIONS'));
    }
  }
  assert.throws(() => createCodexUsagePreferencesStore({ ...options, maxPendingUpdates: 129 }),
    hasCode('INVALID_OPTIONS'));
  assert.throws(() => createCodexUsagePreferencesStore({ ...options, maxDocumentBytes: 65_537 }),
    hasCode('INVALID_OPTIONS'));
  assert.ok(createCodexUsagePreferencesStore({ ...options,
    maxPendingUpdates: 128, maxDocumentBytes: 65_536 }));
  assert.throws(() => createCodexUsagePreferencesStore({ ...options, file: 'PUBLIC' }),
    hasCode('INVALID_OPTIONS'));
  assert.ok(Object.isFrozen(fixture.store));
  assert.equal(JSON.stringify(fixture.store), '"[CodexUsagePreferencesStore]"');
  assert.equal(inspect(fixture.store), '[CodexUsagePreferencesStore]');
});

test('null read is explicit unwritten defaults and never fabricated persistence', async () => {
  const fixture = durableFixture();
  const state = await fixture.store.load();
  assert.deepEqual(state, { version: 1, revision: 0, preferences: defaults(), persisted: false });
  assert.ok(Object.isFrozen(state));
  assert.ok(Object.isFrozen(state.preferences));
  assert.equal(fixture.read(), null);
  assert.equal(fixture.calls.length, 0);
  assert.equal(fixture.reads(), 1);
});

test('successful CAS persists exactly the document and acknowledges a new immutable snapshot', async () => {
  const fixture = durableFixture();
  const input = prefs({ composerDisplay: 'runway', customRemainingPercent: 20.125,
    retryAfterReset: true, experimentalQuota: true });
  const state = await fixture.store.update(0, input);
  assert.deepEqual(state, { ...doc(1, input), persisted: true });
  assert.deepEqual(fixture.read(), doc(1, input));
  assert.deepEqual(Object.keys(fixture.calls[0]), ['expectedRevision', 'document']);
  assert.deepEqual(Object.keys(fixture.calls[0].document), ['version', 'revision', 'preferences']);
  assert.ok(Object.isFrozen(state));
  assert.ok(Object.isFrozen(state.preferences));
  assert.ok(Object.isFrozen(fixture.calls[0].document));
  assert.deepEqual(await fixture.store.load(), state);
});

test('same expected revision concurrent updates allow exactly one success', async () => {
  const fixture = durableFixture();
  const results = await Promise.allSettled([
    fixture.store.update(0, prefs({ composerDisplay: 'percent' })),
    fixture.store.update(0, prefs({ composerDisplay: 'runway' })),
  ]);
  assert.equal(results[0].status, 'fulfilled');
  assert.equal(results[1].status, 'rejected');
  hasCode('CONFLICT')(results[1].reason);
  assert.equal(fixture.calls.length, 1);
  assert.equal(fixture.reads(), 2);
  assert.equal(fixture.read().preferences.composerDisplay, 'percent');
});

test('outside writer between read and CAS produces conflict without false mutation', async () => {
  let fixture;
  const outside = doc(1, prefs({ usageAlert: 'earlyWarning' }));
  fixture = durableFixture(null, {
    commitDocument(request) {
      fixture.externalWrite(outside);
      return fixture.commitDocument(request);
    },
  });
  await assert.rejects(fixture.store.update(0, prefs({ usageAlert: 'custom' })), hasCode('CONFLICT'));
  assert.deepEqual(fixture.read(), outside);
  assert.deepEqual(await fixture.store.load(), { ...outside, persisted: true });
});

test('outside writer before next update is reread rather than overwritten from cached state', async () => {
  const fixture = durableFixture();
  await fixture.store.update(0, prefs());
  const outside = doc(2, prefs({ usageAlert: 'earlyWarning' }));
  fixture.externalWrite(outside);
  await assert.rejects(fixture.store.update(1, prefs()), hasCode('CONFLICT'));
  assert.equal(fixture.calls.length, 1);
  assert.deepEqual(await fixture.store.load(), { ...outside, persisted: true });
});

test('queued and in-flight updates reserve budget before any asynchronous read', async () => {
  const gate = deferred();
  const started = deferred();
  let fixture;
  fixture = durableFixture(null, {
    maxPendingUpdates: 2,
    async readDocument() { started.resolve(); await gate.promise; return fixture.read(); },
  });
  const first = fixture.store.update(0, prefs({ usageAlert: 'percent20' }));
  const second = fixture.store.update(1, prefs({ usageAlert: 'earlyWarning' }));
  await assert.rejects(fixture.store.update(2, prefs()), hasCode('QUEUE_FULL'));
  await started.promise;
  await assert.rejects(fixture.store.update(2, prefs()), hasCode('QUEUE_FULL'));
  assert.equal(fixture.calls.length, 0);
  gate.resolve();
  const results = await Promise.all([first, second]);
  assert.deepEqual(results.map((state) => state.revision), [1, 2]);
  const third = await fixture.store.update(2, prefs());
  assert.equal(third.revision, 3);
});

test('admitted preferences are copied before queued caller mutation', async () => {
  const gate = deferred();
  let fixture;
  fixture = durableFixture(null, {
    async readDocument() { await gate.promise; return fixture.read(); },
  });
  const input = prefs({ customRemainingPercent: 18.75 });
  const result = fixture.store.update(0, input);
  input.customRemainingPercent = 99;
  input.extra = 'PUBLIC';
  gate.resolve();
  assert.equal((await result).preferences.customRemainingPercent, 18.75);
  assert.equal(fixture.read().preferences.customRemainingPercent, 18.75);
});

test('port request and nested document cannot be mutated by the external writer', async () => {
  let fixture;
  fixture = durableFixture(null, {
    commitDocument(request) {
      assert.ok(Object.isFrozen(request));
      assert.throws(() => { request.expectedRevision = 9; }, TypeError);
      assert.throws(() => { request.document.revision = 9; }, TypeError);
      assert.throws(() => { request.document.preferences.retryAfterReset = true; }, TypeError);
      return fixture.commitDocument(request);
    },
  });
  assert.equal((await fixture.store.update(0, prefs())).revision, 1);
});

test('loads serialize behind admitted commits and return fresh disk facts', async () => {
  const started = deferred();
  const gate = deferred();
  let fixture;
  fixture = durableFixture(null, {
    async commitDocument(request) {
      started.resolve();
      await gate.promise;
      return fixture.commitDocument(request);
    },
  });
  const updating = fixture.store.update(0, prefs());
  await started.promise;
  const loading = fixture.store.load();
  assert.equal(fixture.reads(), 1);
  gate.resolve();
  const state = await updating;
  assert.deepEqual(await loading, state);
  fixture.externalWrite(doc(2, prefs({ composerDisplay: 'progressBar' })));
  const refreshed = await fixture.store.load();
  assert.equal(refreshed.revision, 2);
  assert.equal(refreshed.preferences.composerDisplay, 'progressBar');
  assert.notEqual(refreshed, state);
});

test('read failures never fall back, commit or poison later read recovery', async () => {
  let readable = false;
  const fixture = durableFixture(doc(7), {
    readDocument() {
      if (!readable) return Promise.reject(new Error('PUBLIC-秘密'));
      return doc(7);
    },
    maxPendingUpdates: 1,
  });
  await assert.rejects(fixture.store.load(), hasCode('READ_FAILED'));
  await assert.rejects(fixture.store.update(7, prefs()), hasCode('READ_FAILED'));
  assert.equal(fixture.calls.length, 0);
  readable = true;
  assert.equal((await fixture.store.load()).revision, 7);
  assert.equal((await fixture.store.update(7, prefs())).revision, 8);
});

test('thrown write, including external safe errors, is indeterminate and never retried', async () => {
  for (const thrown of [new Error('PUBLIC-秘密'), new CodexUsagePreferencesError('CONFLICT'), 'PUBLIC']) {
    let attempts = 0;
    const fixture = durableFixture(null, {
      commitDocument() { attempts += 1; throw thrown; },
    });
    await assert.rejects(fixture.store.update(0, prefs()), hasCode('INDETERMINATE'));
    assert.equal(attempts, 1);
    assert.deepEqual(await fixture.store.load(), {
      version: 1, revision: 0, preferences: defaults(), persisted: false,
    });
    assert.equal(attempts, 1);
  }
});

test('write-then-failure is not success; explicit later load resolves durable fact', async () => {
  let fixture;
  let attempts = 0;
  fixture = durableFixture(null, {
    commitDocument(request) {
      attempts += 1;
      fixture.commitDocument(request);
      return Promise.reject(new Error('PUBLIC-ambiguous'));
    },
    maxPendingUpdates: 1,
  });
  await assert.rejects(fixture.store.update(0, prefs({ usageAlert: 'custom' })),
    hasCode('INDETERMINATE'));
  const recovered = await fixture.store.load();
  assert.equal(recovered.revision, 1);
  assert.equal(recovered.persisted, true);
  assert.equal(recovered.preferences.usageAlert, 'custom');
  await assert.rejects(fixture.store.update(0, prefs()), hasCode('CONFLICT'));
  assert.equal(attempts, 1);
});

test('only exact committed receipts acknowledge persistence', async () => {
  let accessorCalls = 0;
  const accessor = { get status() { accessorCalls += 1; return 'committed'; } };
  const symbol = { status: 'committed', [Symbol('PUBLIC')]: 'PUBLIC' };
  const receipts = [undefined, null, true, 'committed', [], {},
    { status: 'indeterminate' }, { status: 'PUBLIC-秘密' },
    { status: 'committed', extra: 'PUBLIC' }, accessor, symbol,
    Object.create({ status: 'committed' }),
    new Proxy({}, { getPrototypeOf() { throw new Error('PUBLIC'); } }),
  ];
  for (const receipt of receipts) {
    let fixture;
    fixture = durableFixture(null, {
      commitDocument(request) { fixture.commitDocument(request); return receipt; },
    });
    await assert.rejects(fixture.store.update(0, prefs()), hasCode('INDETERMINATE'));
    assert.equal((await fixture.store.load()).revision, 1);
    assert.equal(fixture.calls.length, 1);
  }
  assert.equal(accessorCalls, 0);
});

test('explicit indeterminate without a write leaves only disk observation authoritative', async () => {
  const fixture = durableFixture(doc(3), {
    commitDocument: () => ({ status: 'indeterminate' }),
  });
  await assert.rejects(fixture.store.update(3, prefs()), hasCode('INDETERMINATE'));
  assert.equal((await fixture.store.load()).revision, 3);
  assert.equal(fixture.read().revision, 3);
});

test('documents accept DTO, complete JSON text and strict UTF-8 bytes', async () => {
  const initial = doc(8, prefs({ customRemainingPercent: 19.625 }));
  for (const raw of [initial, JSON.stringify(initial), utf8(JSON.stringify(initial))]) {
    const { store } = storeForRead(raw);
    const snapshot = await store.load();
    assert.deepEqual(snapshot, { ...initial, persisted: true });
    assert.ok(Object.isFrozen(snapshot.preferences));
    assert.notEqual(snapshot.preferences, initial.preferences);
  }
});

test('documents reject corruption, missing/extra fields and unsupported versions loudly', async () => {
  const malformed = [undefined, {}, [], 'null', 'false', '42', '{',
    JSON.stringify(doc()).slice(0, -1),
    { revision: 0, preferences: prefs() },
    { ...doc(), extra: 'PUBLIC-秘密' },
    doc(-1), doc(0.5), doc(Number.MAX_SAFE_INTEGER + 1),
    doc(0, prefs({ extra: 'PUBLIC' })), doc(0, prefs({ retryAfterReset: 1 })),
    doc(0, prefs({ composerDisplay: '秘密' })),
    new Proxy({}, { getPrototypeOf() { throw new Error('PUBLIC'); } }),
  ];
  for (const raw of malformed) {
    const { store, commits } = storeForRead(raw);
    await assert.rejects(store.load(), hasCode('INVALID_DOCUMENT'));
    await assert.rejects(store.update(0, prefs()), hasCode('INVALID_DOCUMENT'));
    assert.equal(commits(), 0);
  }
  for (const version of [0, 2, '1', null, false]) {
    const { store, commits } = storeForRead({ ...doc(), version });
    await assert.rejects(store.load(), hasCode('UNSUPPORTED_VERSION'));
    await assert.rejects(store.update(0, prefs()), hasCode('UNSUPPORTED_VERSION'));
    assert.equal(commits(), 0);
  }
});

test('a hostile thrown object cannot escape sanitized document diagnostics', async () => {
  const unsafe = new Proxy({}, {
    getPrototypeOf() { throw new Error('PUBLIC-秘密-raw error'); },
  });
  const raw = new Proxy({}, { getPrototypeOf() { throw unsafe; } });
  const { store } = storeForRead(raw);
  await assert.rejects(store.load(), hasCode('INVALID_DOCUMENT'));
  await assert.rejects(store.update(0, prefs()), hasCode('INVALID_DOCUMENT'));
});

test('read DTO mutations never alter an already returned snapshot', async () => {
  const raw = doc(3, prefs({ customRemainingPercent: 25.5 }));
  const { store } = storeForRead(raw);
  const snapshot = await store.load();
  raw.revision = 4;
  raw.preferences.customRemainingPercent = 99;
  assert.equal(snapshot.revision, 3);
  assert.equal(snapshot.preferences.customRemainingPercent, 25.5);
  assert.throws(() => { snapshot.preferences.customRemainingPercent = 1; }, TypeError);
  assert.equal((await store.load()).revision, 4);
});

test('strict UTF-8 rejects truncated, overlong and invalid byte sequences', async () => {
  for (const raw of [Uint8Array.of(0xc3), Uint8Array.of(0xc0, 0xaf),
    Uint8Array.of(0xff), Uint8Array.of(0xed, 0xa0, 0x80),
    new Uint8Array([...utf8(JSON.stringify(doc())), 0xff])]) {
    await assert.rejects(storeForRead(raw).store.load(), hasCode('INVALID_DOCUMENT'));
  }
  await assert.rejects(storeForRead('\ud800').store.load(), hasCode('INVALID_DOCUMENT'));
  await assert.rejects(storeForRead('\udc00').store.load(), hasCode('INVALID_DOCUMENT'));
});

test('byte limit covers complete input, multibyte whitespace/content and generated document', async () => {
  const text = JSON.stringify(doc());
  const size = utf8(text).byteLength;
  assert.equal((await storeForRead(text, size).store.load()).revision, 0);
  for (const raw of [doc(), text, utf8(text)]) {
    await assert.rejects(storeForRead(raw, size - 1).store.load(), hasCode('DOCUMENT_TOO_LARGE'));
  }
  await assert.rejects(storeForRead(`${text} `, size).store.load(), hasCode('DOCUMENT_TOO_LARGE'));
  const unicode = JSON.stringify({ ...doc(), extra: '秘密🔐' });
  assert.ok(utf8(unicode).byteLength > unicode.length);
  await assert.rejects(storeForRead(unicode, unicode.length).store.load(),
    hasCode('DOCUMENT_TOO_LARGE'));
  const fixture = durableFixture(null, { maxDocumentBytes: size - 1 });
  await assert.rejects(fixture.store.update(0, prefs()), hasCode('DOCUMENT_TOO_LARGE'));
  assert.equal(fixture.reads(), 0);
  assert.equal(fixture.calls.length, 0);
});

test('invalid revisions and overflow are rejected without ports or reservations', async () => {
  const fixture = durableFixture(doc(Number.MAX_SAFE_INTEGER), { maxPendingUpdates: 1 });
  for (const revision of [-1, 0.5, NaN, Infinity, '0', null, Number.MAX_SAFE_INTEGER + 1]) {
    await assert.rejects(fixture.store.update(revision, prefs()), hasCode('INVALID_REVISION'));
  }
  await assert.rejects(fixture.store.update(Number.MAX_SAFE_INTEGER, prefs()),
    hasCode('REVISION_OVERFLOW'));
  await assert.rejects(fixture.store.update(0, prefs({ extra: 'PUBLIC' })),
    hasCode('INVALID_PREFERENCES'));
  assert.equal(fixture.reads(), 0);
  assert.equal(fixture.calls.length, 0);
  assert.equal((await fixture.store.load()).revision, Number.MAX_SAFE_INTEGER);
});

test('total operation ceiling bounds held read plus queued loads and disposal drains only that read', async () => {
  const gate = deferred();
  const started = deferred();
  let reads = 0;
  const fixture = durableFixture(null, {
    maxPendingUpdates: 1,
    async readDocument() {
      reads += 1;
      started.resolve();
      await gate.promise;
      return null;
    },
  });
  const first = fixture.store.load();
  await started.promise;
  const queued = Array.from({ length: 127 }, () =>
    assert.rejects(fixture.store.load(), hasCode('CLOSED')));
  await assert.rejects(fixture.store.load(), hasCode('QUEUE_FULL'));
  await assert.rejects(fixture.store.update(0, prefs()), hasCode('QUEUE_FULL'));
  assert.equal(reads, 1);
  let disposed = false;
  const disposal = fixture.store.dispose();
  disposal.then(() => { disposed = true; });
  await Promise.all(queued);
  assert.equal(disposed, false);
  assert.equal(reads, 1);
  gate.resolve();
  assert.deepEqual(await first, {
    version: 1, revision: 0, preferences: defaults(), persisted: false,
  });
  await disposal;
  assert.equal(disposed, true);
  assert.equal(reads, 1);
  assert.equal(fixture.calls.length, 0);
});

test('dispose before work starts cancels every queued operation without touching ports', async () => {
  const fixture = durableFixture();
  const first = assert.rejects(fixture.store.update(0, prefs()), hasCode('CLOSED'));
  const second = assert.rejects(fixture.store.update(1, prefs()), hasCode('CLOSED'));
  const loading = assert.rejects(fixture.store.load(), hasCode('CLOSED'));
  const disposing = fixture.store.dispose();
  assert.equal(fixture.store.dispose(), disposing);
  await Promise.all([first, second, loading, disposing]);
  assert.equal(fixture.reads(), 0);
  assert.equal(fixture.calls.length, 0);
  await assert.rejects(fixture.store.load(), hasCode('CLOSED'));
  await assert.rejects(fixture.store.update(0, prefs()), hasCode('CLOSED'));
  await assert.rejects(fixture.store.update('PUBLIC', null), hasCode('CLOSED'));
  assert.equal(fixture.reads(), 0);
});

test('dispose rejects queued updates immediately and drains an actual in-flight commit', async () => {
  const gate = deferred();
  const started = deferred();
  let commits = 0;
  let fixture;
  fixture = durableFixture(null, {
    maxPendingUpdates: 2,
    async commitDocument(request) {
      commits += 1;
      started.resolve();
      await gate.promise;
      return fixture.commitDocument(request);
    },
  });
  const first = fixture.store.update(0, prefs({ usageAlert: 'custom' }));
  await started.promise;
  const second = assert.rejects(fixture.store.update(1, prefs()), hasCode('CLOSED'));
  const loading = assert.rejects(fixture.store.load(), hasCode('CLOSED'));
  let disposed = false;
  const disposal = fixture.store.dispose();
  disposal.then(() => { disposed = true; });
  assert.equal(fixture.store.dispose(), disposal);
  await Promise.all([second, loading]);
  await assert.rejects(fixture.store.update(1, prefs()), hasCode('CLOSED'));
  await assert.rejects(fixture.store.load(), hasCode('CLOSED'));
  assert.equal(disposed, false);
  assert.equal(commits, 1);
  assert.equal(fixture.reads(), 1);
  assert.equal(fixture.read(), null);
  gate.resolve();
  const committed = await first;
  await disposal;
  assert.equal(committed.revision, 1);
  assert.equal(committed.persisted, true);
  assert.equal(fixture.read().preferences.usageAlert, 'custom');
  assert.equal(disposed, true);
  assert.equal(commits, 1);
  assert.equal(fixture.calls.length, 1);
  assert.equal(fixture.reads(), 1);
});

test('dispose does not detach a failing commit or hide its indeterminate outcome', async () => {
  const gate = deferred();
  const started = deferred();
  const fixture = durableFixture(null, {
    async commitDocument() {
      started.resolve();
      await gate.promise;
      throw new Error('PUBLIC-write-may-have-happened');
    },
  });
  const updating = assert.rejects(fixture.store.update(0, prefs()), hasCode('INDETERMINATE'));
  await started.promise;
  let disposed = false;
  const disposal = fixture.store.dispose();
  disposal.then(() => { disposed = true; });
  await Promise.resolve();
  assert.equal(disposed, false);
  gate.resolve();
  await updating;
  await disposal;
  assert.equal(disposed, true);
  assert.equal(fixture.calls.length, 0);
});

test('dispose during active read waits its actual completion and prevents a new commit', async () => {
  const gate = deferred();
  const started = deferred();
  let reads = 0;
  const fixture = durableFixture(null, {
    async readDocument() {
      reads += 1;
      started.resolve();
      await gate.promise;
      return null;
    },
  });
  const updating = assert.rejects(fixture.store.update(0, prefs()), hasCode('CLOSED'));
  await started.promise;
  const queued = assert.rejects(fixture.store.update(0, prefs()), hasCode('CLOSED'));
  let disposed = false;
  const disposal = fixture.store.dispose();
  disposal.then(() => { disposed = true; });
  await queued;
  assert.equal(disposed, false);
  assert.equal(reads, 1);
  gate.resolve();
  await updating;
  await disposal;
  assert.equal(disposed, true);
  assert.equal(reads, 1);
  assert.equal(fixture.calls.length, 0);
});

test('dispose waits active load and preserves a failed read without default fallback', async () => {
  const gate = deferred();
  const started = deferred();
  const fixture = durableFixture(null, {
    async readDocument() {
      started.resolve();
      await gate.promise;
      throw new Error('PUBLIC-read-failed');
    },
  });
  const loading = assert.rejects(fixture.store.load(), hasCode('READ_FAILED'));
  await started.promise;
  let disposed = false;
  const disposal = fixture.store.dispose();
  disposal.then(() => { disposed = true; });
  await Promise.resolve();
  assert.equal(disposed, false);
  gate.resolve();
  await loading;
  await disposal;
  assert.equal(disposed, true);
  assert.equal(fixture.calls.length, 0);
});

test('a failure releases its reservation and does not stop later queued transactions', async () => {
  let fixture;
  let attempts = 0;
  fixture = durableFixture(null, {
    maxPendingUpdates: 2,
    commitDocument(request) {
      attempts += 1;
      if (attempts === 1) return { status: 'indeterminate' };
      return fixture.commitDocument(request);
    },
  });
  const first = fixture.store.update(0, prefs());
  const second = fixture.store.update(0, prefs({ usageAlert: 'custom' }));
  await assert.rejects(first, hasCode('INDETERMINATE'));
  assert.equal((await second).revision, 1);
  assert.equal((await fixture.store.update(1, prefs())).revision, 2);
});
