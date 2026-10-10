/**
 * Pure private-Host projections of Codex 0.160.1 account/read and
 * account/rateLimits/read result objects, not JSON-RPC envelopes or REST bodies.
 * Pinned v2 GetAccountResponse/GetAccountRateLimitsResponse embed the Account,
 * RateLimitSnapshot and RateLimitWindow definitions. No auth/route validation,
 * I/O, persistence, prior-snapshot merging or freshness policy lives here.
 */
import { types } from 'node:util';

/** @typedef {'InvalidProjectionSpec'|'InvalidBinding'|'InvalidWireResult'|'ProjectionLimitExceeded'|'IdentityMismatch'} ProjectionErrorCode */
/** @typedef {{maxInputBytes:number,maxOutputBytes:number,maxBuckets:number,maxWindows:number,maxEmailBytes:number,maxStringBytes:number}} CodexProjectionSpec */
/** @typedef {{accountOpaqueId:string,accountGeneration:number,expectedIssuerAccountId:string|null,observedAtUnixSeconds:number}} CodexProjectionBinding */
/** @typedef {{accountOpaqueId:string,accountGeneration:number,observedAtUnixSeconds:number,status:'chatgpt'|'notChatGPT',accountMode:'chatgpt'|'apiKey'|'amazonBedrock'|'signedOut',requiresOpenaiAuth:boolean,email:string|null,plan:string|null}} CodexAccountDTO */
/** @typedef {{kind:'primary'|'secondary',usedPercent:number,windowDurationMins:number|null,resetsAtUnixSeconds:number|null}} CodexQuotaWindowDTO */
/** @typedef {{bucketId:string|null,limitId:string|null,limitName:string|null,modelAlias:string|null,plan:string|null,windows:ReadonlyArray<Readonly<CodexQuotaWindowDTO>>}} CodexQuotaBucketDTO */
/** @typedef {{accountOpaqueId:string,accountGeneration:number,observedAtUnixSeconds:number,identityBinding:'issuerMatched'|'ownedTransport',ordinaryUsageAllowed:boolean|null,buckets:ReadonlyArray<Readonly<CodexQuotaBucketDTO>>}} CodexQuotaDTO */

const SPEC_KEYS = ['maxInputBytes', 'maxOutputBytes', 'maxBuckets', 'maxWindows', 'maxEmailBytes', 'maxStringBytes'];
// Hard security ceilings, not deployment defaults: every tuning field is required.
const CEILINGS = { maxInputBytes: 16 * 1024 * 1024, maxOutputBytes: 65536,
  maxBuckets: 1024, maxWindows: 2048, maxEmailBytes: 4096, maxStringBytes: 4096 };
const PLANS = new Set(['free', 'go', 'plus', 'pro', 'prolite', 'promax', 'team',
  'self_serve_business_prolite', 'self_serve_business_usage_based', 'business', 'ent26',
  'enterprise_cbp_automation', 'enterprise_cbp_usage_based', 'enterprise', 'edu', 'edu_plus', 'edu_pro', 'unknown']);
const REACHED = new Set(['rate_limit_reached', 'workspace_owner_credits_depleted',
  'workspace_member_credits_depleted', 'workspace_owner_usage_limit_reached', 'workspace_member_usage_limit_reached']);
const own = (value, key) => Object.hasOwn(value, key);
const record = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const integer = value => Number.isSafeInteger(value);
const unsafeLabel = /[\p{Cc}\p{Cf}\p{Cs}\p{Zl}\p{Zp}]/u;
function jwtLabel(value) {
  // Reject JWT-shaped public text, including non-canonical/unsigned headers.
  const jsonObject = segment => {
    try { return record(JSON.parse(Buffer.from(segment, 'base64url').toString('utf8'))); }
    catch { return false; }
  };
  for (const run of value.match(/[A-Za-z0-9_.-]+/gu) ?? []) {
    const parts = run.split('.');
    for (let index = 0; index + 2 < parts.length; index++) {
      if (jsonObject(parts[index]) && jsonObject(parts[index + 1])) return true;
    }
  }
  return false;
}

/** Fixed code only; never retains input, issuer identity, payload or cause. */
export class CodexProjectionError extends Error {
  /** @param {ProjectionErrorCode} code */
  constructor(code) {
    // Also constrain JS callers of the exported constructor.
    const safe = ['InvalidProjectionSpec', 'InvalidBinding', 'InvalidWireResult',
      'ProjectionLimitExceeded', 'IdentityMismatch'].includes(code) ? code : 'InvalidWireResult';
    super(safe);
    this.name = 'CodexProjectionError';
    /** @type {ProjectionErrorCode} */
    this.code = safe;
  }
}
const fail = code => { throw new CodexProjectionError(code); };
const requireWire = condition => { if (!condition) fail('InvalidWireResult'); };

/** Reject executable/non-JSON/prototype input before reading business fields.
 * Count the exact UTF-8 JSON encoding, including unknown fields and escapes;
 * oversize input is rejected, never truncated. Accessors/toJSON are not invoked.
 */
function jsonBoundary(value, maxBytes, errorCode) {
  let bytes = 0;
  let nodes = 0;
  const ancestors = new Set();
  const add = amount => {
    bytes += amount;
    if (bytes > maxBytes) fail(errorCode);
  };
  const stringBytes = text => {
    if (text.length > maxBytes) fail(errorCode);
    return Buffer.byteLength(JSON.stringify(text), 'utf8');
  };
  function visit(item, depth) {
    if (++nodes > 65536 || depth > 32) fail(errorCode);
    if (item === null || typeof item === 'boolean') return add(item === null ? 4 : item ? 4 : 5);
    if (typeof item === 'string') return add(stringBytes(item));
    if (typeof item === 'number' && Number.isFinite(item)) return add(Buffer.byteLength(JSON.stringify(item), 'utf8'));
    if (typeof item !== 'object' || types.isProxy(item) || ancestors.has(item)) fail(errorCode);
    const array = Array.isArray(item);
    const proto = Object.getPrototypeOf(item);
    if (array ? proto !== Array.prototype : proto !== Object.prototype && proto !== null) fail(errorCode);
    const keys = Reflect.ownKeys(item);
    if (keys.length > 65536) fail(errorCode);
    ancestors.add(item);
    add(2);
    if (array) {
      const length = Object.getOwnPropertyDescriptor(item, 'length').value;
      if (keys.length !== length + 1) fail(errorCode);
      for (let index = 0; index < length; index++) {
        const descriptor = Object.getOwnPropertyDescriptor(item, String(index));
        if (!descriptor || !descriptor.enumerable || !own(descriptor, 'value')) fail(errorCode);
        if (index) add(1);
        visit(descriptor.value, depth + 1);
      }
    } else {
      for (let index = 0; index < keys.length; index++) {
        const key = keys[index];
        if (typeof key !== 'string' || ['__proto__', 'prototype', 'constructor'].includes(key)) fail(errorCode);
        const descriptor = Object.getOwnPropertyDescriptor(item, key);
        if (!descriptor.enumerable || !own(descriptor, 'value')) fail(errorCode);
        if (index) add(1);
        add(stringBytes(key) + 1);
        visit(descriptor.value, depth + 1);
      }
    }
    ancestors.delete(item);
  }
  visit(value, 0);
}

function options(binding, spec) {
  jsonBoundary(spec, 1024, 'InvalidProjectionSpec');
  if (!record(spec) || Object.keys(spec).length !== SPEC_KEYS.length
    || SPEC_KEYS.some(key => !own(spec, key) || !integer(spec[key]) || spec[key] < 1 || spec[key] > CEILINGS[key])) {
    fail('InvalidProjectionSpec');
  }
  jsonBoundary(binding, 16384, 'InvalidBinding');
  const fields = ['accountOpaqueId', 'accountGeneration', 'expectedIssuerAccountId', 'observedAtUnixSeconds'];
  if (!record(binding) || Object.keys(binding).length !== fields.length || fields.some(key => !own(binding, key))
    || typeof binding.accountOpaqueId !== 'string'
    || !/^[A-Za-z0-9][A-Za-z0-9._:-]*$/.test(binding.accountOpaqueId)
    || jwtLabel(binding.accountOpaqueId)
    || Buffer.byteLength(binding.accountOpaqueId, 'utf8') > spec.maxStringBytes
    || !integer(binding.accountGeneration) || binding.accountGeneration < 0
    || !integer(binding.observedAtUnixSeconds) || binding.observedAtUnixSeconds < 0
    || (binding.expectedIssuerAccountId !== null && (typeof binding.expectedIssuerAccountId !== 'string'
      || binding.expectedIssuerAccountId.length === 0 || unsafeLabel.test(binding.expectedIssuerAccountId)
      || Buffer.byteLength(binding.expectedIssuerAccountId, 'utf8') > 4096))
    || binding.accountOpaqueId === binding.expectedIssuerAccountId) fail('InvalidBinding');
}

function observation(binding) {
  return { accountOpaqueId: binding.accountOpaqueId, accountGeneration: binding.accountGeneration,
    observedAtUnixSeconds: binding.observedAtUnixSeconds };
}

function text(value, maxBytes) {
  requireWire(typeof value === 'string');
  if (Buffer.byteLength(value, 'utf8') > maxBytes) fail('ProjectionLimitExceeded');
  requireWire(!unsafeLabel.test(value) && !jwtLabel(value));
  return value;
}
function optional(value, key, valid) {
  if (own(value, key) && value[key] !== null) requireWire(valid(value[key]));
}
function plan(value) {
  requireWire(PLANS.has(value));
  return value;
}
function nullableText(value, key, spec) {
  return !own(value, key) || value[key] === null ? null : text(value[key], spec.maxStringBytes);
}
function nullablePlan(value) { return value == null ? null : plan(value); }
function validateCredits(value) {
  requireWire(record(value) && own(value, 'hasCredits') && own(value, 'unlimited')
    && typeof value.hasCredits === 'boolean' && typeof value.unlimited === 'boolean');
  optional(value, 'balance', item => typeof item === 'string');
}
function validateSpendControl(value) {
  requireWire(record(value) && ['limit', 'used', 'remainingPercent', 'resetsAt'].every(key => own(value, key))
    && typeof value.limit === 'string' && typeof value.used === 'string'
    && integer(value.remainingPercent) && integer(value.resetsAt));
}
function validateResetCredits(value) {
  requireWire(record(value) && own(value, 'availableCount') && integer(value.availableCount));
  if (!own(value, 'credits') || value.credits === null) return;
  requireWire(Array.isArray(value.credits));
  for (const credit of value.credits) {
    requireWire(record(credit) && ['id', 'grantedAt', 'resetType', 'status'].every(key => own(credit, key))
      && typeof credit.id === 'string' && integer(credit.grantedAt)
      && ['codexRateLimits', 'unknown'].includes(credit.resetType)
      && ['available', 'redeeming', 'redeemed', 'unknown'].includes(credit.status));
    optional(credit, 'expiresAt', integer);
    optional(credit, 'description', item => typeof item === 'string');
    optional(credit, 'title', item => typeof item === 'string');
  }
}
function window(value, kind) {
  requireWire(record(value) && own(value, 'usedPercent') && typeof value.usedPercent === 'number'
    && Number.isFinite(value.usedPercent) && value.usedPercent >= 0);
  // Numeric usage is deliberately preserved (including >100), never clamped.
  optional(value, 'windowDurationMins', item => integer(item) && item > 0);
  optional(value, 'resetsAt', item => integer(item) && item >= 0);
  return Object.freeze({ kind, usedPercent: value.usedPercent,
    windowDurationMins: value.windowDurationMins ?? null, resetsAtUnixSeconds: value.resetsAt ?? null });
}
function bucket(value, bucketId, spec) {
  requireWire(record(value));
  optional(value, 'credits', item => { validateCredits(item); return true; });
  optional(value, 'individualLimit', item => { validateSpendControl(item); return true; });
  optional(value, 'rateLimitReachedType', item => REACHED.has(item));
  optional(value, 'spendControlReached', item => typeof item === 'boolean');
  const windows = [];
  for (const kind of ['primary', 'secondary']) {
    if (own(value, kind) && value[kind] !== null) windows.push(window(value[kind], kind));
  }
  return Object.freeze({ bucketId, limitId: nullableText(value, 'limitId', spec),
    limitName: nullableText(value, 'limitName', spec), modelAlias: nullableText(value, 'normalModelSlug', spec),
    plan: !own(value, 'planType') || value.planType === null ? null : text(nullablePlan(value.planType), spec.maxStringBytes),
    windows: Object.freeze(windows) });
}
function finish(dto, spec, binding) {
  // Even a malicious known display field must not echo the private issuer ID.
  const noIssuer = value => {
    if (typeof value === 'string' && binding.expectedIssuerAccountId !== null
      && value.includes(binding.expectedIssuerAccountId)) fail('InvalidWireResult');
    if (value !== null && typeof value === 'object') Object.values(value).forEach(noIssuer);
  };
  noIssuer(dto);
  if (Buffer.byteLength(JSON.stringify(dto), 'utf8') > spec.maxOutputBytes) fail('ProjectionLimitExceeded');
  return Object.freeze(dto);
}
function guarded(callback) {
  try { return callback(); }
  catch (error) {
    if (error instanceof CodexProjectionError) throw error;
    fail('InvalidWireResult');
  }
}

/**
 * Metadata only: a ChatGPT account record is NOT validated credentials, model
 * entitlement or an activated route. Missing account/null means signed out;
 * apiKey/amazonBedrock yield typed notChatGPT without fabricated usage.
 * The only email admitted is this official account/read metadata field.
 * @param {unknown} result Private complete account/read result.
 * @param {CodexProjectionBinding} binding Host-owned opaque identity/generation.
 * @param {CodexProjectionSpec} spec Explicit required deployment bounds.
 * @returns {Readonly<CodexAccountDTO>}
 * @throws {CodexProjectionError} Fixed local code only.
 */
export function projectCodexAccount(result, binding, spec) {
  return guarded(() => {
    options(binding, spec);
    jsonBoundary(result, spec.maxInputBytes, 'InvalidWireResult');
    requireWire(record(result) && own(result, 'requiresOpenaiAuth') && typeof result.requiresOpenaiAuth === 'boolean');
    let accountMode = 'signedOut';
    let email = null;
    let accountPlan = null;
    if (own(result, 'account') && result.account !== null) {
      const account = result.account;
      requireWire(record(account) && own(account, 'type') && ['chatgpt', 'apiKey', 'amazonBedrock'].includes(account.type));
      accountMode = account.type;
      if (accountMode === 'chatgpt') {
        requireWire(own(account, 'email') && own(account, 'planType'));
        email = account.email === null ? null : text(account.email, spec.maxEmailBytes);
        accountPlan = text(plan(account.planType), spec.maxStringBytes);
      } else if (accountMode === 'amazonBedrock' && own(account, 'usesCodexManagedCredentials')) {
        requireWire(typeof account.usesCodexManagedCredentials === 'boolean');
      }
    }
    return finish({ ...observation(binding), status: accountMode === 'chatgpt' ? 'chatgpt' : 'notChatGPT',
      accountMode, requiresOpenaiAuth: result.requiresOpenaiAuth, email, plan: accountPlan }, spec, binding);
  });
}

/**
 * Fresh read, not a sparse notification merge: absent optional fields become
 * null/empty, never a recovered old value. Primary/secondary are actual window
 * roles, not assumed 5h/week durations. modelAlias is normalModelSlug, not a model
 * entitlement. Unknown server fields, credits and reset-credit details never
 * leave this boundary (known pinned fields are still validated).
 *
 * Map keys are preserved as bucketId; absent snapshot limitId remains null.
 * Append legacy unless the map contains its ID AND identical projected quota
 * fields/windows; divergent legacy observations are retained, not overwritten.
 * Non-null accountId must match the private expected issuer; null/absent uses
 * the owned transport binding, explicitly not independent issuer verification.
 * Ordinary permission is true/false/null exactly as supplied, never inferred.
 * @param {unknown} result Private complete account/rateLimits/read result.
 * @param {CodexProjectionBinding} binding Host-owned opaque identity/generation.
 * @param {CodexProjectionSpec} spec Explicit required deployment bounds.
 * @returns {Readonly<CodexQuotaDTO>}
 * @throws {CodexProjectionError} Fixed local code only.
 */
export function projectCodexQuota(result, binding, spec) {
  return guarded(() => {
    options(binding, spec);
    jsonBoundary(result, spec.maxInputBytes, 'InvalidWireResult');
    requireWire(record(result) && own(result, 'rateLimits'));
    optional(result, 'accountId', item => typeof item === 'string');
    optional(result, 'ordinaryUsageAllowed', item => typeof item === 'boolean');
    optional(result, 'rateLimitResetCredits', item => { validateResetCredits(item); return true; });
    const legacy = bucket(result.rateLimits, null, spec);
    const buckets = [];
    if (own(result, 'rateLimitsByLimitId') && result.rateLimitsByLimitId !== null) {
      requireWire(record(result.rateLimitsByLimitId));
      const entries = Object.entries(result.rateLimitsByLimitId);
      if (entries.length > spec.maxBuckets) fail('ProjectionLimitExceeded');
      for (const [id, snapshot] of entries) buckets.push(bucket(snapshot, text(id, spec.maxStringBytes), spec));
    }
    const duplicate = buckets.some(item => item.bucketId === legacy.limitId
      && JSON.stringify({ ...item, bucketId: null }) === JSON.stringify(legacy));
    if (!duplicate) buckets.push(legacy);
    if (buckets.length > spec.maxBuckets || buckets.reduce((sum, item) => sum + item.windows.length, 0) > spec.maxWindows) {
      fail('ProjectionLimitExceeded');
    }
    if (result.accountId != null && result.accountId !== binding.expectedIssuerAccountId) fail('IdentityMismatch');
    return finish({ ...observation(binding), identityBinding: result.accountId == null ? 'ownedTransport' : 'issuerMatched',
      ordinaryUsageAllowed: result.ordinaryUsageAllowed ?? null, buckets: Object.freeze(buckets) }, spec, binding);
  });
}
