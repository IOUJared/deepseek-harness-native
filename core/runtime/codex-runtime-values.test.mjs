// PUBLIC synthetic values only; production helpers own validation and projection.
import test from 'node:test';
import assert from 'node:assert/strict';
import { inspect } from 'node:util';
import {
  validateCodexRuntimeValues, projectCodexRuntimeValues, projectCodexRuntimeBaseValues, codexRuntimePathOps,
  validCodexRuntimeRevision, runtimeUnavailable,
} from './codex-runtime-values.mjs';

const fields = [
  'transport', 'cacheRetention', 'timeoutMs', 'websocketConnectTimeoutMs',
  'streamIdleTimeoutMs', 'requestImagePixelBudget', 'requestImageMaxBytes',
  'maxRequestImageBytes',
];
const numericFields = fields.slice(2);
const transports = [null, 'sse', 'websocket', 'websocket-cached', 'auto'];
const retentions = [null, 'none', 'short', 'long'];
const values = (changes = {}) => ({ ...Object.fromEntries(fields.map((key) => [key, null])), ...changes });
const bytes = (value) => Buffer.byteLength(JSON.stringify(value), 'utf8');
const unavailable = (error) => {
  assert.ok(error instanceof Error);
  assert.equal(error.name, 'CodexRuntimeError');
  assert.equal(error.code, 'CODEX_RUNTIME_UNAVAILABLE');
  assert.equal(error.message, error.code);
  assert.equal(Object.hasOwn(error, 'cause'), false);
  assert.ok(Object.isFrozen(error));
  return true;
};

test('schema defaults materialize even when inherited route is absent, without hardcoded defaults', () => {
  const refs = { 0: { type: 'object', dict: { providers: 1 } }, 1: { type: 'dict', inner: 2 }, 2: { type: 'object', dict: {} } };
  for (const [index, field] of fields.entries()) { refs[2].dict[field] = index + 3; refs[index + 3] = { meta: {} }; }
  refs[refs[2].dict.streamIdleTimeoutMs].meta.default = 12345.5;
  refs[refs[2].dict.requestImagePixelBudget].meta.default = 1024;
  const schema = { uid: 0, refs };
  const result = projectCodexRuntimeBaseValues(schema, { timeoutMs: 0 });
  assert.equal(result.timeoutMs, 0); assert.equal(result.streamIdleTimeoutMs, 12345.5);
  assert.equal(result.requestImagePixelBudget, 1024); assert.equal(result.transport, null);
  assert.equal(Object.isFrozen(result), true);
  assert.equal(projectCodexRuntimeBaseValues(schema, { streamIdleTimeoutMs: 0.5 }).streamIdleTimeoutMs, 0.5);
  assert.throws(() => projectCodexRuntimeBaseValues({}, undefined), unavailable);
  refs[refs[2].dict.requestImagePixelBudget].meta.default = -1;
  assert.throws(() => projectCodexRuntimeBaseValues(schema, undefined), unavailable);
});
test('natural timeout zero and fractional idle timeout retain public schema behavior', () => {
  const result = validateCodexRuntimeValues(values({ timeoutMs: -0, websocketConnectTimeoutMs: 0, streamIdleTimeoutMs: 0.5 }));
  assert.equal(Object.is(result.timeoutMs, 0), true); assert.equal(result.streamIdleTimeoutMs, 0.5);
  assert.throws(() => validateCodexRuntimeValues(values({ streamIdleTimeoutMs: 2147483648 })), unavailable);
});

// A second Settings model would not verify these production helpers.
test('all inheritance fields normalize to an exact detached immutable DTO', () => {
  const input = values();
  const first = validateCodexRuntimeValues(input);
  const second = validateCodexRuntimeValues(input);
  assert.deepEqual(first, input);
  assert.deepEqual(Reflect.ownKeys(first), fields);
  assert.notEqual(first, input);
  assert.notEqual(first, second);
  assert.ok(Object.isFrozen(first));
  input.timeoutMs = 123;
  assert.equal(first.timeoutMs, null);
  assert.throws(() => { first.timeoutMs = 1; }, TypeError);
  assert.deepEqual(validateCodexRuntimeValues(Object.assign(Object.create(null), values())), values());
});

test('every transport/cache choice and positive safe integer boundary validates', () => {
  for (const transport of transports) {
    for (const cacheRetention of retentions) {
      for (const number of [null, 1, 2, Number.MAX_SAFE_INTEGER - 1, Number.MAX_SAFE_INTEGER]) {
        const input = values({ transport, cacheRetention,
          ...Object.fromEntries(numericFields.map((field) => [field, field === 'streamIdleTimeoutMs' && number !== null ? Math.min(number, 2147483647) : number])) });
        assert.deepEqual(validateCodexRuntimeValues(input), input);
        assert.deepEqual(projectCodexRuntimeValues(input), input);
        const ops = codexRuntimePathOps(input);
        assert.equal(ops.length, 8);
        for (const [index, field] of fields.entries()) {
          assert.deepEqual(ops[index], input[field] === null
            ? { op: 'unset', path: ['providers', 'openai-codex', field] }
            : { op: 'set', path: ['providers', 'openai-codex', field], value: input[field] });
        }
      }
    }
  }
});

test('invalid record types fail closed across validation, projection and operations', () => {
  for (const input of [null, [], true, false, 1, 'PUBLIC', () => {}, new Date(), new Map()]) {
    for (const helper of [validateCodexRuntimeValues, projectCodexRuntimeValues, codexRuntimePathOps]) {
      assert.throws(() => helper(input), unavailable);
    }
  }
  assert.throws(() => validateCodexRuntimeValues(undefined), unavailable);
  assert.throws(() => codexRuntimePathOps(undefined), unavailable);
});

test('all missing, unknown, credential and addressing fields are rejected', () => {
  for (const field of fields) {
    const input = values();
    delete input[field];
    assert.throws(() => validateCodexRuntimeValues(input), unavailable);
    assert.throws(() => codexRuntimePathOps(input), unavailable);
  }
  for (const key of ['unknown', 'apiKey', 'apiKeyEnv', 'credentials', 'headers', 'baseURL',
    'namespace', 'ns', 'path', 'provider', '__proto__', 'constructor', 'toJSON', Symbol('PUBLIC')]) {
    const input = values();
    Object.defineProperty(input, key, { value: 'PUBLIC-SECRET', enumerable: true });
    assert.throws(() => validateCodexRuntimeValues(input), unavailable);
    assert.throws(() => codexRuntimePathOps(input), unavailable);
  }
  const input = values();
  Object.defineProperty(input, 'secret', { value: 'PUBLIC-SECRET', enumerable: false });
  assert.throws(() => validateCodexRuntimeValues(input), unavailable);
});

test('numeric fields enforce alpha-specific zero, fractional and image-integer limits', () => {
  const invalid = [undefined, 0, -0, -1, 0.5, 1.5, NaN, Infinity, -Infinity,
    Number.MAX_SAFE_INTEGER + 1, '1', true, false, 1n, {}, new Number(1), [], Symbol('PUBLIC')];
  for (const field of numericFields) {
    for (const number of invalid) {
      if (['timeoutMs', 'websocketConnectTimeoutMs'].includes(field) && Object.is(number, 0)) continue;
      if (['timeoutMs', 'websocketConnectTimeoutMs'].includes(field) && Object.is(number, -0)) continue;
      if (field === 'streamIdleTimeoutMs' && [0.5, 1.5].includes(number)) continue;
      assert.throws(() => validateCodexRuntimeValues(values({ [field]: number })), unavailable);
      assert.throws(() => projectCodexRuntimeValues({ [field]: number }), unavailable);
      assert.throws(() => codexRuntimePathOps(values({ [field]: number })), unavailable);
    }
  }
});

test('enum fields reject unknown strings and coercible or absent explicit values', () => {
  for (const field of fields.slice(0, 2)) {
    for (const value of [undefined, '', 'PUBLIC-秘密-🔐', 'SSE', 'inherit', 1, false,
      new String(field === 'transport' ? 'sse' : 'short'), {}, []]) {
      assert.throws(() => validateCodexRuntimeValues(values({ [field]: value })), unavailable);
      assert.throws(() => projectCodexRuntimeValues({ [field]: value }), unavailable);
    }
  }
  assert.throws(() => validateCodexRuntimeValues(values({ transport: 'short' })), unavailable);
  assert.throws(() => validateCodexRuntimeValues(values({ cacheRetention: 'auto' })), unavailable);
});

test('projection defaults only absent fields and independently captures partial profiles', () => {
  assert.deepEqual(projectCodexRuntimeValues(), values());
  assert.deepEqual(projectCodexRuntimeValues(undefined), values());
  assert.deepEqual(projectCodexRuntimeValues({}), values());
  for (const field of fields) {
    const profile = { [field]: field === 'transport' ? 'auto' : field === 'cacheRetention' ? 'long' : 1 };
    const result = projectCodexRuntimeValues(profile);
    assert.deepEqual(result, values(profile));
    assert.deepEqual(Reflect.ownKeys(result), fields);
    assert.ok(Object.isFrozen(result));
    const captured = result[field];
    profile[field] = null;
    assert.equal(result[field], captured);
    assert.notEqual(result, projectCodexRuntimeValues(profile));
  }
});

test('projection ignores secret extras, symbols, hidden extras and serialization hooks', () => {
  let reads = 0;
  const secret = 'PUBLIC-秘密-🔐-not-a-real-credential';
  const profile = { transport: 'websocket', timeoutMs: 1, apiKeyEnv: secret,
    headers: { Authorization: secret }, models: [{ apiKey: secret }], baseURL: secret };
  for (const key of ['credentials', 'toJSON', '__proto__', Symbol('PUBLIC')]) {
    Object.defineProperty(profile, key, {
      enumerable: true, get() { reads += 1; throw new Error(secret); },
    });
  }
  Object.defineProperty(profile, 'hiddenSecret', { value: secret });
  const result = projectCodexRuntimeValues(profile);
  assert.deepEqual(result, values({ transport: 'websocket', timeoutMs: 1 }));
  assert.equal(reads, 0);
  assert.ok(!JSON.stringify(result).includes(secret));
  assert.deepEqual(Reflect.ownKeys(result), fields);
  const requestedDescriptors = [];
  const proxy = new Proxy(profile, {
    ownKeys() { throw new Error(secret); },
    getOwnPropertyDescriptor(target, key) {
      requestedDescriptors.push(key);
      return Reflect.getOwnPropertyDescriptor(target, key);
    },
  });
  assert.deepEqual(projectCodexRuntimeValues(proxy), result);
  assert.deepEqual(requestedDescriptors, fields);
});

test('known accessors, hidden and inherited fields are not accepted as values', () => {
  let reads = 0;
  for (const field of fields) {
    const accessor = values();
    Object.defineProperty(accessor, field, {
      enumerable: true, get() { reads += 1; throw new Error('PUBLIC-SECRET'); },
    });
    const hidden = values();
    Object.defineProperty(hidden, field, { enumerable: false });
    for (const input of [accessor, hidden]) {
      assert.throws(() => validateCodexRuntimeValues(input), unavailable);
      assert.throws(() => projectCodexRuntimeValues(input), unavailable);
    }
  }
  assert.equal(reads, 0);
  const inherited = Object.create(values());
  assert.throws(() => validateCodexRuntimeValues(inherited), unavailable);
  assert.throws(() => projectCodexRuntimeValues(inherited), unavailable);
});

test('reflection failures never escape with arbitrary text or cause', () => {
  const secret = 'PUBLIC-秘密-🔐';
  for (const trap of ['getPrototypeOf', 'ownKeys', 'getOwnPropertyDescriptor']) {
    const input = new Proxy(values(), { [trap]() { throw new Error(secret); } });
    const helpers = trap === 'ownKeys'
      ? [validateCodexRuntimeValues, codexRuntimePathOps]
      : [validateCodexRuntimeValues, projectCodexRuntimeValues, codexRuntimePathOps];
    for (const helper of helpers) {
      assert.throws(() => helper(input), (error) => {
        unavailable(error);
        for (const text of [error.message, String(error), error.stack, JSON.stringify(error), inspect(error)]) {
          assert.ok(!text.includes(secret));
        }
        return true;
      });
    }
  }
  const { proxy, revoke } = Proxy.revocable(values(), {});
  revoke();
  for (const helper of [validateCodexRuntimeValues, projectCodexRuntimeValues, codexRuntimePathOps]) {
    assert.throws(() => helper(proxy), unavailable);
  }
});

test('operations capture independent immutable arrays, entries and fixed paths', () => {
  const input = values({ transport: 'sse', timeoutMs: 123 });
  const first = codexRuntimePathOps(input);
  const second = codexRuntimePathOps(input);
  input.transport = 'auto';
  input.timeoutMs = 999;
  assert.equal(first[0].value, 'sse');
  assert.equal(first[2].value, 123);
  assert.ok(Object.isFrozen(first));
  assert.notEqual(first, second);
  assert.throws(() => first.push({}), TypeError);
  for (const [index, op] of first.entries()) {
    assert.ok(Object.isFrozen(op));
    assert.ok(Object.isFrozen(op.path));
    assert.notEqual(op, second[index]);
    assert.notEqual(op.path, second[index].path);
    assert.deepEqual(op.path, ['providers', 'openai-codex', fields[index]]);
    assert.deepEqual(Reflect.ownKeys(op), op.op === 'set' ? ['op', 'path', 'value'] : ['op', 'path']);
    assert.throws(() => { op.path[1] = 'PUBLIC-other-provider'; }, TypeError);
    assert.throws(() => { op.value = 999; }, TypeError);
  }
});

test('revision validation accepts exactly nonnegative safe numbers without coercion', () => {
  for (const revision of [0, -0, 1, Number.MAX_SAFE_INTEGER - 1, Number.MAX_SAFE_INTEGER]) {
    assert.equal(validCodexRuntimeRevision(revision), true);
  }
  for (const revision of [-1, 0.5, 1.5, Number.MAX_SAFE_INTEGER + 1, Infinity, NaN,
    undefined, null, true, false, '0', '1', 1n, [], {}, new Number(1), Symbol('PUBLIC')]) {
    assert.equal(validCodexRuntimeRevision(revision), false);
  }
});

test('diagnostics accept only closed codes and sanitize arbitrary prose without coercion', () => {
  for (const code of ['CODEX_RUNTIME_UNAVAILABLE', 'CODEX_RUNTIME_CONFLICT', 'CODEX_RUNTIME_INDETERMINATE']) {
    const error = runtimeUnavailable(code);
    assert.ok(error instanceof Error);
    assert.equal(error.code, code);
    assert.equal(error.message, code);
    assert.ok(Object.isFrozen(error));
    assert.equal(Object.hasOwn(error, 'cause'), false);
  }
  let coerced = 0;
  for (const code of [undefined, null, 'PUBLIC-秘密-🔐', '', 'CONFLICT', 'INDETERMINATE',
    1, {}, { toString() { coerced += 1; throw new Error('PUBLIC-SECRET'); } }]) {
    unavailable(runtimeUnavailable(code));
  }
  assert.equal(coerced, 0);
});

test('maximum field values and all enum combinations leave generous complete-result byte headroom', () => {
  for (const transport of transports) {
    for (const cacheRetention of retentions) {
      const captured = validateCodexRuntimeValues(values({ transport, cacheRetention,
        ...Object.fromEntries(numericFields.map((field) => [field, field === 'streamIdleTimeoutMs' ? 2147483647 : Number.MAX_SAFE_INTEGER])) }));
      const operations = codexRuntimePathOps(captured);
      assert.equal(Reflect.ownKeys(captured).length, 8);
      assert.equal(operations.length, 8);
      assert.ok(bytes(captured) < 512);
      assert.ok(bytes(operations) < 1536);
      assert.ok(bytes({ values: captured, operations, revision: Number.MAX_SAFE_INTEGER }) < 2048);
    }
  }
});
