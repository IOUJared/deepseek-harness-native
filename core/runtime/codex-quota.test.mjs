import assert from 'node:assert/strict';
import test from 'node:test';
import { CodexProjectionError, projectCodexAccount, projectCodexQuota } from './codex-quota.mjs';

// PUBLIC synthetic fixtures only. No files, environment, CLI or live accounts.
const PRIVATE_ISSUER = 'PUBLIC-private-issuer-fixture';
const PRIVATE_TOKEN = 'PUBLIC_FIXTURE_TOKEN_NEVER_REAL';
const PRIVATE_EMAIL = 'private-grant-only@example.invalid';
const PUBLIC_EMAIL = 'public-fixture@example.invalid';
const PUBLIC_JWT = 'eyJwdWJsaWMiOnRydWV9.eyJmaXh0dXJlIjp0cnVlfQ.UFVCTElDX0ZJWFRVUkVfU0lHTkFUVVJF';
const PUBLIC_NONCANONICAL_JWT = `${Buffer.from('{\n"alg":"none"\n}').toString('base64url')}.${Buffer.from('{\n"public":true\n}').toString('base64url')}.`;
const BINDING = Object.freeze({ accountOpaqueId: 'opaque-public-fixture', accountGeneration: 7,
  expectedIssuerAccountId: PRIVATE_ISSUER, observedAtUnixSeconds: 1791430800 });
const SPEC = Object.freeze({ maxInputBytes: 16384, maxOutputBytes: 8192,
  maxBuckets: 12, maxWindows: 24, maxEmailBytes: 128, maxStringBytes: 128 });
const account = () => ({ requiresOpenaiAuth: true, account: { type: 'chatgpt', email: PUBLIC_EMAIL, planType: 'plus' } });
const snapshot = () => ({ limitId: 'codex', limitName: 'Codex', normalModelSlug: 'gpt-fixture', planType: 'plus',
  primary: { usedPercent: 12, windowDurationMins: 43, resetsAt: 1791430900 },
  secondary: { usedPercent: 142, windowDurationMins: null, resetsAt: null } });
const quota = () => ({ accountId: PRIVATE_ISSUER, ordinaryUsageAllowed: false, rateLimits: snapshot() });
const projectAccount = (value, spec = SPEC, binding = BINDING) => projectCodexAccount(value, binding, spec);
const projectQuota = (value, spec = SPEC, binding = BINDING) => projectCodexQuota(value, binding, spec);
const encodingBytes = value => Buffer.byteLength(JSON.stringify(value), 'utf8');
function fails(callback, code) {
  assert.throws(callback, error => {
    assert(error instanceof CodexProjectionError);
    assert.equal(error.code, code);
    assert.equal(error.message, code);
    assert.equal(error.name, 'CodexProjectionError');
    assert.deepEqual(Object.keys(error).sort(), ['code', 'name']);
    for (const secret of [PRIVATE_ISSUER, PRIVATE_TOKEN, PRIVATE_EMAIL, PUBLIC_JWT]) {
      assert(!JSON.stringify(error).includes(secret));
      assert(!String(error).includes(secret));
      assert(!error.stack.includes(secret));
    }
    assert(!('cause' in error));
    return true;
  });
}
function noPrivate(dto) {
  const encoded = JSON.stringify(dto);
  for (const secret of [PRIVATE_ISSUER, PRIVATE_TOKEN, PRIVATE_EMAIL, PUBLIC_JWT]) assert(!encoded.includes(secret));
}

// Exact public surface, metadata only, no validation/route/credential claims.
test('module exports only two pure projections and fixed-code error type', async () => {
  const exports = await import('./codex-quota.mjs');
  assert.deepEqual(Object.keys(exports).sort(), ['CodexProjectionError', 'projectCodexAccount', 'projectCodexQuota']);
});
test('official account metadata projects one fresh bounded observation', () => {
  assert.deepEqual(projectAccount(account()), { accountOpaqueId: BINDING.accountOpaqueId,
    accountGeneration: 7, observedAtUnixSeconds: BINDING.observedAtUnixSeconds,
    status: 'chatgpt', accountMode: 'chatgpt', requiresOpenaiAuth: true, email: PUBLIC_EMAIL, plan: 'plus' });
});
test('ChatGPT null email and unknown official plan remain unknown metadata', () => {
  const result = account();
  result.account.email = null;
  result.account.planType = 'unknown';
  assert.equal(projectAccount(result).email, null);
  assert.equal(projectAccount(result).plan, 'unknown');
});
for (const mode of ['apiKey', 'amazonBedrock', 'signedOut', 'missing']) {
  test(`account ${mode} returns typed notChatGPT, no invented usage`, () => {
    const result = { requiresOpenaiAuth: false };
    if (mode !== 'missing') result.account = mode === 'signedOut' ? null : { type: mode };
    const dto = projectAccount(result);
    assert.equal(dto.status, 'notChatGPT');
    assert.equal(dto.accountMode, mode === 'missing' ? 'signedOut' : mode);
    assert.equal(dto.email, null);
    assert.equal(dto.plan, null);
    assert.equal(dto.requiresOpenaiAuth, false);
    assert(!('usage' in dto));
    assert(!('credentialValidated' in dto));
    assert(!('routeActive' in dto));
  });
}
test('Bedrock optional credential flag validates but never becomes authorization', () => {
  const result = { requiresOpenaiAuth: false, account: { type: 'amazonBedrock', usesCodexManagedCredentials: true } };
  assert.equal(projectAccount(result).accountMode, 'amazonBedrock');
  result.account.usesCodexManagedCredentials = null;
  fails(() => projectAccount(result), 'InvalidWireResult');
});
for (const [name, result] of [
  ['required auth missing', { account: null }], ['auth wrong type', { requiresOpenaiAuth: null }],
  ['unknown account mode', { requiresOpenaiAuth: true, account: { type: 'oauth' } }],
  ['account wrong type', { requiresOpenaiAuth: true, account: 'chatgpt' }],
  ['ChatGPT email missing', { requiresOpenaiAuth: true, account: { type: 'chatgpt', planType: 'plus' } }],
  ['ChatGPT plan missing', { requiresOpenaiAuth: true, account: { type: 'chatgpt', email: null } }],
  ['ChatGPT plan null', { requiresOpenaiAuth: true, account: { type: 'chatgpt', email: null, planType: null } }],
  ['ChatGPT future plan', { requiresOpenaiAuth: true, account: { type: 'chatgpt', email: null, planType: 'future' } }],
  ['ChatGPT email numeric', { requiresOpenaiAuth: true, account: { type: 'chatgpt', email: 123, planType: 'plus' } }],
]) test(`account rejects ${name}`, () => fails(() => projectAccount(result), 'InvalidWireResult'));
test('official enum includes newer plans without invented interpretation', () => {
  const result = account();
  for (const value of ['free', 'go', 'prolite', 'promax', 'self_serve_business_prolite', 'ent26',
    'enterprise_cbp_usage_based', 'edu_plus', 'edu_pro']) {
    result.account.planType = value;
    assert.equal(projectAccount(result).plan, value);
  }
});

test('quota preserves actual primary/secondary windows and >100 raw numeric display value', () => {
  const dto = projectQuota(quota());
  assert.deepEqual(dto, { accountOpaqueId: BINDING.accountOpaqueId, accountGeneration: 7,
    observedAtUnixSeconds: BINDING.observedAtUnixSeconds, identityBinding: 'issuerMatched', ordinaryUsageAllowed: false,
    buckets: [{ bucketId: null, limitId: 'codex', limitName: 'Codex', modelAlias: 'gpt-fixture', plan: 'plus',
      windows: [{ kind: 'primary', usedPercent: 12, windowDurationMins: 43, resetsAtUnixSeconds: 1791430900 },
        { kind: 'secondary', usedPercent: 142, windowDurationMins: null, resetsAtUnixSeconds: null }] }] });
  noPrivate(dto);
});
test('finite fractional usage and zero usage remain raw numbers', () => {
  const result = quota();
  result.rateLimits.primary.usedPercent = 0;
  result.rateLimits.secondary.usedPercent = 100.125;
  assert.deepEqual(projectQuota(result).buckets[0].windows.map(value => value.usedPercent), [0, 100.125]);
});
test('empty snapshot has unknown metadata and no fabricated windows', () => {
  assert.deepEqual(projectQuota({ rateLimits: {} }).buckets, [{ bucketId: null, limitId: null,
    limitName: null, modelAlias: null, plan: null, windows: [] }]);
});
test('nullable fields and missing optional fields both project explicit null', () => {
  const result = { accountId: null, ordinaryUsageAllowed: null,
    rateLimits: { limitId: null, limitName: null, normalModelSlug: null, planType: null,
      primary: { usedPercent: 9 }, secondary: null }, rateLimitsByLimitId: null };
  const dto = projectQuota(result);
  assert.equal(dto.identityBinding, 'ownedTransport');
  assert.equal(dto.ordinaryUsageAllowed, null);
  assert.deepEqual(dto.buckets[0].windows, [{ kind: 'primary', usedPercent: 9,
    windowDurationMins: null, resetsAtUnixSeconds: null }]);
});
for (const permission of [true, false, null, 'missing']) {
  test(`ordinaryUsageAllowed ${permission} is never inferred from usage/reset`, () => {
    const result = { rateLimits: { primary: { usedPercent: 0, resetsAt: 0 }, secondary: { usedPercent: 9999 } } };
    if (permission !== 'missing') result.ordinaryUsageAllowed = permission;
    assert.equal(projectQuota(result).ordinaryUsageAllowed, permission === 'missing' ? null : permission);
  });
}
for (const id of [null, 'missing']) {
  test(`quota ${id} accountId explicitly uses transport ownership not issuer verification`, () => {
    const result = quota();
    if (id === 'missing') delete result.accountId;
    else result.accountId = id;
    assert.equal(projectQuota(result).identityBinding, 'ownedTransport');
  });
}
test('non-null issuer mismatch is typed, never contains either private identity', () => {
  const result = quota();
  result.accountId = `${PRIVATE_ISSUER}-wrong`;
  fails(() => projectQuota(result), 'IdentityMismatch');
});
test('unknown expected issuer cannot independently validate non-null server identity', () => {
  fails(() => projectQuota(quota(), SPEC, { ...BINDING, expectedIssuerAccountId: null }), 'IdentityMismatch');
  assert.equal(projectQuota({ rateLimits: {} }, SPEC, { ...BINDING, expectedIssuerAccountId: null }).identityBinding, 'ownedTransport');
});
test('multiple map buckets preserve order, IDs, metadata and all actual windows', () => {
  const legacy = snapshot();
  const second = { limitId: null, limitName: null, normalModelSlug: null, planType: null,
    primary: null, secondary: { usedPercent: 35, windowDurationMins: 9, resetsAt: 0 } };
  const result = { rateLimits: legacy, rateLimitsByLimitId: { codex: structuredClone(legacy), image: second } };
  const dto = projectQuota(result);
  assert.equal(dto.buckets.length, 2);
  assert.deepEqual(dto.buckets.map(value => value.bucketId), ['codex', 'image']);
  assert.equal(dto.buckets[1].limitId, null);
  assert.equal(dto.buckets[1].modelAlias, null);
  assert.equal(dto.buckets[1].windows[0].kind, 'secondary');
  assert.equal(dto.buckets.reduce((sum, value) => sum + value.windows.length, 0), 3);
});
test('divergent map/legacy windows are retained rather than silently overwritten', () => {
  const result = quota();
  const map = snapshot();
  map.secondary.usedPercent = 200;
  result.rateLimitsByLimitId = { codex: map };
  const dto = projectQuota(result);
  assert.equal(dto.buckets.length, 2);
  assert.deepEqual(dto.buckets.map(value => value.windows[1].usedPercent), [200, 142]);
});
test('matching snapshot under a different map key does not deduplicate legacy by guess', () => {
  const result = quota();
  result.rateLimitsByLimitId = { other: snapshot() };
  assert.equal(projectQuota(result).buckets.length, 2);
});
test('map snapshot ID stays null when absent; map key is separately preserved', () => {
  const dto = projectQuota({ rateLimits: {}, rateLimitsByLimitId: { mapped: { primary: { usedPercent: 30 } } } });
  assert.equal(dto.buckets[0].bucketId, 'mapped');
  assert.equal(dto.buckets[0].limitId, null);
});
test('deduplicated legacy does not consume emitted bucket/window bounds', () => {
  const result = quota();
  result.rateLimitsByLimitId = { codex: snapshot() };
  assert.equal(projectQuota(result, { ...SPEC, maxBuckets: 1, maxWindows: 2 }).buckets.length, 1);
});
for (const [name, result] of [
  ['required legacy missing', {}], ['legacy null', { rateLimits: null }], ['legacy array', { rateLimits: [] }],
  ['map array', { rateLimits: {}, rateLimitsByLimitId: [] }],
  ['map value null', { rateLimits: {}, rateLimitsByLimitId: { codex: null } }],
  ['issuer numeric', { rateLimits: {}, accountId: 1 }],
  ['permission string', { rateLimits: {}, ordinaryUsageAllowed: 'true' }],
  ['window missing usage', { rateLimits: { primary: {} } }],
  ['window boolean', { rateLimits: { secondary: false } }],
  ['plan future', { rateLimits: { planType: 'future' } }],
  ['label numeric', { rateLimits: { limitName: 1 } }],
  ['alias array', { rateLimits: { normalModelSlug: [] } }],
]) test(`quota rejects ${name}`, () => fails(() => projectQuota(result), 'InvalidWireResult'));
for (const usedPercent of [-1, NaN, Infinity, -Infinity, null, '12', undefined]) {
  test(`usage rejects ${String(usedPercent)}`, () => fails(() => projectQuota({ rateLimits: { primary: { usedPercent } } }), 'InvalidWireResult'));
}
for (const [field, values] of [['windowDurationMins', [0, -1, 0.5, Number.MAX_SAFE_INTEGER + 1, '1']],
  ['resetsAt', [-1, 0.5, Number.MAX_SAFE_INTEGER + 1, '1']]]) {
  for (const value of values) test(`window rejects ${field} ${value}`, () => {
    fails(() => projectQuota({ rateLimits: { primary: { usedPercent: 0, [field]: value } } }), 'InvalidWireResult');
  });
}
test('safe integer limits for actual duration/reset survive without invented units', () => {
  const dto = projectQuota({ rateLimits: { primary: { usedPercent: Number.MAX_VALUE,
    windowDurationMins: Number.MAX_SAFE_INTEGER, resetsAt: Number.MAX_SAFE_INTEGER } } });
  assert.equal(dto.buckets[0].windows[0].usedPercent, Number.MAX_VALUE);
  assert.equal(dto.buckets[0].windows[0].windowDurationMins, Number.MAX_SAFE_INTEGER);
});

// Validate ignored known wire structures, but never export them.
test('valid known credits/reset/spend fields are validated then discarded', () => {
  const result = quota();
  result.rateLimits.credits = { hasCredits: true, unlimited: false, balance: PRIVATE_TOKEN };
  result.rateLimits.individualLimit = { limit: PRIVATE_TOKEN, used: PRIVATE_TOKEN, remainingPercent: 44, resetsAt: 1 };
  result.rateLimits.rateLimitReachedType = 'workspace_owner_usage_limit_reached';
  result.rateLimits.spendControlReached = true;
  result.rateLimitResetCredits = { availableCount: 2, credits: [{ id: PRIVATE_TOKEN, grantedAt: 1,
    expiresAt: null, resetType: 'codexRateLimits', status: 'available', title: PRIVATE_EMAIL, description: PRIVATE_TOKEN }] };
  const dto = projectQuota(result);
  assert(!('credits' in dto.buckets[0]));
  assert(!('rateLimitResetCredits' in dto));
  noPrivate(dto);
});
for (const [field, value] of [['credits', {}], ['credits', { hasCredits: true, unlimited: null }],
  ['credits', { hasCredits: true, unlimited: false, balance: 0 }], ['individualLimit', {}],
  ['individualLimit', { limit: '1', used: '2', remainingPercent: 1.5, resetsAt: 1 }],
  ['rateLimitReachedType', 'future'], ['spendControlReached', 'false']]) {
  test(`ignored snapshot ${field} invalid ${JSON.stringify(value)} fails wire validation`, () => {
    fails(() => projectQuota({ rateLimits: { [field]: value } }), 'InvalidWireResult');
  });
}
for (const value of [{}, { availableCount: '1' }, { availableCount: 1, credits: {} },
  { availableCount: 1, credits: [{ grantedAt: 1, resetType: 'codexRateLimits', status: 'available' }] },
  { availableCount: 1, credits: [{ id: 'x', grantedAt: 1, resetType: 'future', status: 'available' }] },
  { availableCount: 1, credits: [{ id: 'x', grantedAt: 1, resetType: 'unknown', status: 'future' }] },
  { availableCount: 1, credits: [{ id: 'x', grantedAt: 1, resetType: 'unknown', status: 'unknown', expiresAt: 0.5 }] }]) {
  test(`ignored reset credits invalid ${JSON.stringify(value)} fails wire validation`, () => {
    fails(() => projectQuota({ rateLimits: {}, rateLimitResetCredits: value }), 'InvalidWireResult');
  });
}
test('nullable optional credit structures are allowed without fabricated credit state', () => {
  noPrivate(projectQuota({ rateLimits: { credits: null, individualLimit: null, rateLimitReachedType: null,
    spendControlReached: null }, rateLimitResetCredits: { availableCount: 0, credits: null } }));
});

test('unknown nested secrets, errors, issuer/grant metadata and JWTs are discarded', () => {
  const privateFields = { access_token: PRIVATE_TOKEN, id_token: PUBLIC_JWT, email: PRIVATE_EMAIL,
    issuerAccountId: PRIVATE_ISSUER, error: { message: PRIVATE_TOKEN }, authenticatedAccountGrant: { token: PRIVATE_TOKEN } };
  const result = account();
  Object.assign(result, privateFields);
  Object.assign(result.account, { authenticatedAccountGrant: privateFields, jwt: PUBLIC_JWT });
  const dto = projectAccount(result);
  noPrivate(dto);
  assert.equal(dto.email, PUBLIC_EMAIL);
  const usage = quota();
  Object.assign(usage, privateFields, { rateLimitUpsell: privateFields });
  Object.assign(usage.rateLimits, privateFields);
  Object.assign(usage.rateLimits.primary, privateFields);
  noPrivate(projectQuota(usage));
});
for (const label of ['bad\u0000label', 'bad\u001flabel', 'bad\u007flabel', 'bad\u0085label',
  'bad\u202elabel', 'bad\u2066label', 'bad\u200blabel', 'bad\u2028label', 'bad\u2029label', 'bad\ud800label', PUBLIC_JWT, PUBLIC_NONCANONICAL_JWT]) {
  test(`public control/bidi/JWT label ${JSON.stringify(label)} fails closed not truncates`, () => {
    fails(() => projectQuota({ rateLimits: { limitName: label } }), 'InvalidWireResult');
    const result = account();
    result.account.email = label;
    fails(() => projectAccount(result), 'InvalidWireResult');
  });
}
test('private issuer cannot be echoed from a known public label or email', () => {
  fails(() => projectQuota({ rateLimits: { limitName: `prefix ${PRIVATE_ISSUER} suffix` } }), 'InvalidWireResult');
  const result = account();
  result.account.email = `${PRIVATE_ISSUER}@example.invalid`;
  fails(() => projectAccount(result), 'InvalidWireResult');
});
test('bounded printable Unicode and combining sequences are preserved, not normalized', () => {
  const result = { rateLimits: { limitName: '配額é e\u0301 🧪', normalModelSlug: '模型-é', limitId: '' } };
  const dto = projectQuota(result);
  assert.equal(dto.buckets[0].limitName, result.rateLimits.limitName);
  assert.equal(dto.buckets[0].modelAlias, result.rateLimits.normalModelSlug);
  assert.equal(dto.buckets[0].limitId, '');
});
for (const [name, value] of [
  ['nested poisoned prototype', Object.create({ token: PRIVATE_TOKEN })],
  ['nested explicit prototype key', JSON.parse('{"__proto__":{"token":"PUBLIC_FIXTURE_TOKEN_NEVER_REAL"}}')],
  ['nested constructor key', { constructor: { token: PRIVATE_TOKEN } }],
  ['nested prototype key', { prototype: { token: PRIVATE_TOKEN } }],
  ['nested undefined', { hidden: undefined }], ['nested bigint', { hidden: 1n }],
  ['nested function', { hidden: () => PRIVATE_TOKEN }], ['nested non-JSON Date', new Date(0)],
  ['nested Map', new Map()], ['nested NaN', { hidden: NaN }],
  ['nested symbol key', { [Symbol('secret')]: PRIVATE_TOKEN }],
  ['nested sparse array', new Array(2)],
]) test(`whole boundary rejects ${name} even on ignored fields`, () => {
  fails(() => projectQuota({ rateLimits: {}, ignored: value }), 'InvalidWireResult');
  fails(() => projectAccount({ ...account(), ignored: value }), 'InvalidWireResult');
});
test('getters, proxies and toJSON callbacks are never invoked', () => {
  let calls = 0;
  const getter = {};
  Object.defineProperty(getter, 'token', { enumerable: true, get() { calls++; throw new Error(PRIVATE_TOKEN); } });
  const proxy = new Proxy({}, { ownKeys() { calls++; throw new Error(PRIVATE_TOKEN); },
    getPrototypeOf() { calls++; throw new Error(PRIVATE_TOKEN); } });
  const toJSON = { toJSON() { calls++; throw new Error(PRIVATE_TOKEN); } };
  for (const ignored of [getter, proxy, toJSON]) fails(() => projectQuota({ rateLimits: {}, ignored }), 'InvalidWireResult');
  assert.equal(calls, 0);
});
test('cycle/depth/non-enumerable/extra-array-key cases fail closed', () => {
  const cycle = {};
  cycle.self = cycle;
  let deep = {};
  for (let i = 0; i < 35; i++) deep = { nested: deep };
  const nonEnumerable = {};
  Object.defineProperty(nonEnumerable, 'token', { value: PRIVATE_TOKEN });
  const extraArrayKey = [];
  extraArrayKey.secret = PRIVATE_TOKEN;
  for (const ignored of [cycle, deep, nonEnumerable, extraArrayKey]) {
    fails(() => projectQuota({ rateLimits: {}, ignored }), 'InvalidWireResult');
  }
});
test('null-prototype data records can be projected without prototype dependence', () => {
  const result = Object.assign(Object.create(null), { rateLimits: Object.create(null) });
  assert.equal(projectQuota(result).buckets[0].windows.length, 0);
});

for (const key of Object.keys(SPEC)) {
  test(`projection spec requires explicit ${key}`, () => {
    const spec = { ...SPEC };
    delete spec[key];
    fails(() => projectQuota(quota(), spec), 'InvalidProjectionSpec');
  });
  test(`projection spec rejects invalid ${key} types and security ceiling`, () => {
    for (const value of [null, 0, -1, 0.5, '12', Infinity, Number.MAX_SAFE_INTEGER]) {
      fails(() => projectAccount(account(), { ...SPEC, [key]: value }), 'InvalidProjectionSpec');
    }
  });
}
test('projection spec rejects extras and non-JSON/prototype options', () => {
  fails(() => projectQuota(quota(), { ...SPEC, defaults: true }), 'InvalidProjectionSpec');
  fails(() => projectQuota(quota(), Object.assign(Object.create({}), SPEC)), 'InvalidProjectionSpec');
});
for (const key of Object.keys(BINDING)) {
  test(`binding requires explicit ${key}`, () => {
    const binding = { ...BINDING };
    delete binding[key];
    fails(() => projectQuota(quota(), SPEC, binding), 'InvalidBinding');
  });
}
for (const [field, values] of [['accountGeneration', [-1, 1.5, Number.MAX_SAFE_INTEGER + 1, '1']],
  ['observedAtUnixSeconds', [-1, 1.5, Number.MAX_SAFE_INTEGER + 1, '1']],
  ['accountOpaqueId', ['', PRIVATE_ISSUER, PUBLIC_JWT, PUBLIC_NONCANONICAL_JWT, PRIVATE_EMAIL, 'bad\u202eid', 'bad\n'] ],
  ['expectedIssuerAccountId', ['', 1, 'bad\n', 'bad\u202eid']]]) {
  test(`binding rejects invalid ${field}`, () => {
    for (const value of values) fails(() => projectQuota(quota(), SPEC, { ...BINDING, [field]: value }), 'InvalidBinding');
  });
}
test('binding rejects unknown secret-bearing extra fields', () => {
  fails(() => projectAccount(account(), SPEC, { ...BINDING, token: PRIVATE_TOKEN }), 'InvalidBinding');
});
test('input byte bound counts complete unknown metadata, UTF-8 and JSON escapes exactly', () => {
  const result = { rateLimits: {}, ignored: { message: 'é配🧪\n"\\', token: PRIVATE_TOKEN } };
  const bytes = encodingBytes(result);
  assert.equal(projectQuota(result, { ...SPEC, maxInputBytes: bytes }).buckets.length, 1);
  fails(() => projectQuota(result, { ...SPEC, maxInputBytes: bytes - 1 }), 'InvalidWireResult');
});
test('oversize ignored body and oversize array fail closed, not partial projection', () => {
  fails(() => projectQuota({ rateLimits: {}, ignored: PRIVATE_TOKEN.repeat(1000) }), 'InvalidWireResult');
  fails(() => projectAccount({ ...account(), ignored: new Array(20000).fill(PRIVATE_TOKEN) }), 'InvalidWireResult');
});
test('public UTF-8 label bound uses bytes rather than characters, inclusive exact edge', () => {
  const label = '配🧪é'.repeat(4); // 36 bytes, more than the public opaque ID.
  const result = { rateLimits: { limitName: label } };
  assert.equal(projectQuota(result, { ...SPEC, maxStringBytes: Buffer.byteLength(label) }).buckets[0].limitName, label);
  fails(() => projectQuota(result, { ...SPEC, maxStringBytes: Buffer.byteLength(label) - 1 }), 'ProjectionLimitExceeded');
});
test('official metadata email UTF-8 bound is independent and inclusive', () => {
  const result = account();
  result.account.email = '配額@example.invalid';
  const length = Buffer.byteLength(result.account.email);
  assert.equal(projectAccount(result, { ...SPEC, maxEmailBytes: length }).email, result.account.email);
  fails(() => projectAccount(result, { ...SPEC, maxEmailBytes: length - 1 }), 'ProjectionLimitExceeded');
});
test('aggregate output UTF-8 bound is exact including observation and all fields', () => {
  const result = { rateLimits: { limitName: '配額🧪' } };
  const dto = projectQuota(result);
  const bytes = encodingBytes(dto);
  assert.deepEqual(projectQuota(result, { ...SPEC, maxOutputBytes: bytes }), dto);
  fails(() => projectQuota(result, { ...SPEC, maxOutputBytes: bytes - 1 }), 'ProjectionLimitExceeded');
  const accountBytes = encodingBytes(projectAccount(account()));
  assert.equal(encodingBytes(projectAccount(account(), { ...SPEC, maxOutputBytes: accountBytes })), accountBytes);
  fails(() => projectAccount(account(), { ...SPEC, maxOutputBytes: accountBytes - 1 }), 'ProjectionLimitExceeded');
});
test('bucket and total actual-window bounds are inclusive, no silent truncation', () => {
  const result = quota();
  result.rateLimitsByLimitId = { image: { primary: { usedPercent: 12 }, secondary: { usedPercent: 14 } } };
  assert.equal(projectQuota(result, { ...SPEC, maxBuckets: 2, maxWindows: 4 }).buckets.length, 2);
  fails(() => projectQuota(result, { ...SPEC, maxBuckets: 1 }), 'ProjectionLimitExceeded');
  fails(() => projectQuota(result, { ...SPEC, maxWindows: 3 }), 'ProjectionLimitExceeded');
});
test('map key and public opaque ID are also bounded dynamic strings', () => {
  fails(() => projectQuota({ rateLimits: {}, rateLimitsByLimitId: { ['配'.repeat(50)]: {} } }), 'ProjectionLimitExceeded');
  fails(() => projectQuota(quota(), SPEC, { ...BINDING, accountOpaqueId: 'a'.repeat(129) }), 'InvalidBinding');
});
test('unknown RPC/error envelope is not accepted as a business result', () => {
  fails(() => projectQuota({ jsonrpc: '2.0', id: 1, result: quota() }), 'InvalidWireResult');
  fails(() => projectAccount({ jsonrpc: '2.0', id: 1, error: { message: PRIVATE_TOKEN } }), 'InvalidWireResult');
});
test('error constructor constrains malicious raw input to fixed safe code', () => {
  const error = new CodexProjectionError(PRIVATE_TOKEN);
  assert.equal(error.code, 'InvalidWireResult');
  noPrivate(error);
});
test('each result is fresh, generation/timestamp-bound, deeply immutable and detached', () => {
  const raw = quota();
  const first = projectQuota(raw);
  const second = projectQuota(raw, SPEC, { ...BINDING, accountGeneration: 8, observedAtUnixSeconds: 1791430900 });
  assert.notEqual(first, second);
  assert.notEqual(first.buckets, second.buckets);
  assert.notEqual(first.buckets[0], second.buckets[0]);
  assert.notEqual(first.buckets[0].windows[0], second.buckets[0].windows[0]);
  assert.equal(first.accountGeneration, 7);
  assert.equal(second.accountGeneration, 8);
  assert.equal(second.observedAtUnixSeconds, 1791430900);
  for (const value of [first, first.buckets, first.buckets[0], first.buckets[0].windows, first.buckets[0].windows[0]]) {
    assert(Object.isFrozen(value));
  }
  raw.rateLimits.primary.usedPercent = 99;
  assert.equal(first.buckets[0].windows[0].usedPercent, 12);
  assert.throws(() => { first.buckets[0].windows[0].usedPercent = 33; }, TypeError);
  assert(Object.isFrozen(projectAccount(account())));
});
test('fresh sparse read never merges old permission/name/plan/model/window state', () => {
  projectQuota(quota());
  const next = projectQuota({ rateLimits: { primary: { usedPercent: 0 } } });
  assert.equal(next.ordinaryUsageAllowed, null);
  assert.equal(next.buckets[0].limitName, null);
  assert.equal(next.buckets[0].modelAlias, null);
  assert.equal(next.buckets[0].plan, null);
  assert.deepEqual(next.buckets[0].windows, [{ kind: 'primary', usedPercent: 0, windowDurationMins: null, resetsAtUnixSeconds: null }]);
});
test('projections do not mutate input, binding or explicit spec', () => {
  const result = quota();
  const before = structuredClone(result);
  projectQuota(result);
  assert.deepEqual(result, before);
  const metadata = account();
  const metadataBefore = structuredClone(metadata);
  projectAccount(metadata);
  assert.deepEqual(metadata, metadataBefore);
  assert.deepEqual(BINDING, { accountOpaqueId: 'opaque-public-fixture', accountGeneration: 7,
    expectedIssuerAccountId: PRIVATE_ISSUER, observedAtUnixSeconds: 1791430800 });
});
