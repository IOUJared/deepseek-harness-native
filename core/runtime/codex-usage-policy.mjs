/** Pure policies over sanitized public CodexQuotaDTOs. No I/O, state or execution. */
/** @typedef {import('./codex-quota.mjs').CodexQuotaDTO} CodexQuotaDTO */
/** modelAlias is the provider-owned quota-model mapping for the request, not a UI
 * label/catalog route. Null is unknown and cannot select specific-model capacity.
 * @typedef {{accountOpaqueId:string,accountGeneration:number,nowUnixSeconds:number,modelAlias:string|null}} CodexUsageContext */
/** @typedef {{composerDisplay:'off'|'percent'|'progressBar'|'runway',usageAlert:'off'|'percent20'|'earlyWarning'|'custom',customRemainingPercent:number,retryAfterReset:boolean,experimentalQuota:boolean}} CodexUsagePreferences */
/** @typedef {{maxSnapshotAgeSeconds:number,maxHistorySamples:number,minRateIntervalSeconds:number,maxRateIntervalSeconds:number,maxRetryWaitSeconds:number,maxRetryAttempts:number,earlyWarningSeconds:number}} CodexUsagePolicySpec */
/** @typedef {{requestId:string,currentRequestId:string,accountOpaqueId:string,accountGeneration:number,modelAlias:string|null,failureKind:'quotaDenied'|'other'|'indeterminate',failedBeforeOutput:boolean,cancelled:boolean,attempts:number,resetAtUnixSeconds:number|null,requestDeadlineUnixSeconds:number}} CodexResetRetryIntent */
/** @typedef {'InvalidPolicySpec'|'InvalidContext'|'InvalidPreferences'|'InvalidSnapshot'|'InvalidHistory'|'InvalidAlertState'|'InvalidRetryIntent'} CodexUsagePolicyErrorCode */
/** @typedef {{accountOpaqueId:string,accountGeneration:number,observedAtUnixSeconds:number,bucketId:string|null,limitId:string|null,limitName:string|null,modelAlias:string|null,plan:string|null,kind:'primary'|'secondary',usedPercent:number,windowDurationMins:number|null,resetsAtUnixSeconds:number|null}} CodexUsageWindow */
/** @typedef {{mode:'percent'|'progressBar'|'runway',status:'known'|'unknown',window:CodexUsageWindow|null,reason?:string,remainingPercent?:number,remainingSeconds?:number,estimate?:true,beta?:true}} CodexUsageDisplay */
/** @typedef {{accountOpaqueId:string,accountGeneration:number,bucketId:string|null,kind:'primary'|'secondary',resetsAtUnixSeconds:number|null}} CodexUsageAlertScope */
/** @typedef {{scope:CodexUsageAlertScope,settingsFingerprint:string,belowThreshold:boolean}} CodexUsageAlertMarker */
/** @typedef {{mode:'percent20'|'custom'|'earlyWarning',window:CodexUsageWindow,remainingPercent?:number,thresholdRemainingPercent?:number,remainingSeconds?:number,estimate?:true,beta?:true}} CodexUsageAlertEvent */
/** @typedef {{decision:'off'|'blocked'|'wait'|'observe'|'retry',reason:string,waitUntilUnixSeconds:number|null,requestDeadlineUnixSeconds:number|null}} CodexResetRetryDecision */

const ERROR_CODES = ['InvalidPolicySpec', 'InvalidContext', 'InvalidPreferences', 'InvalidSnapshot',
  'InvalidHistory', 'InvalidAlertState', 'InvalidRetryIntent'];
const SPEC_KEYS = ['maxSnapshotAgeSeconds', 'maxHistorySamples', 'minRateIntervalSeconds',
  'maxRateIntervalSeconds', 'maxRetryWaitSeconds', 'maxRetryAttempts', 'earlyWarningSeconds'];
const CONTEXT_KEYS = ['accountOpaqueId', 'accountGeneration', 'nowUnixSeconds', 'modelAlias'];
const PREF_KEYS = ['composerDisplay', 'usageAlert', 'customRemainingPercent', 'retryAfterReset', 'experimentalQuota'];
const INTENT_KEYS = ['requestId', 'currentRequestId', 'accountOpaqueId', 'accountGeneration', 'modelAlias',
  'failureKind', 'failedBeforeOutput', 'cancelled', 'attempts', 'resetAtUnixSeconds', 'requestDeadlineUnixSeconds'];
const record = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const integer = value => Number.isSafeInteger(value) && value >= 0;
const text = value => typeof value === 'string' && value.length > 0 && value.length <= 4096;
const nullableText = value => value === null || (typeof value === 'string' && value.length <= 4096);
const nullableTime = value => value === null || integer(value);
const exact = (value, keys) => record(value) && Object.keys(value).length === keys.length
  && keys.every(key => Object.hasOwn(value, key));
const remaining = used => Math.max(0, Math.min(100, 100 - used));

/** Fixed code only; no supplied value, payload or cause is retained. */
export class CodexUsagePolicyError extends Error {
  /** @param {CodexUsagePolicyErrorCode} code */
  constructor(code) {
    const safe = ERROR_CODES.includes(code) ? code : 'InvalidSnapshot';
    super(safe);
    this.name = 'CodexUsagePolicyError';
    this.code = safe;
  }
}
const fail = code => { throw new CodexUsagePolicyError(code); };
const requireValue = (condition, code) => { if (!condition) fail(code); };
function guarded(callback, code) {
  try { return callback(); }
  catch (error) {
    if (error instanceof CodexUsagePolicyError) throw error;
    fail(code);
  }
}
function options(context, prefs, spec) {
  requireValue(exact(spec, SPEC_KEYS) && SPEC_KEYS.every(key => integer(spec[key]) && spec[key] > 0)
    && spec.maxRateIntervalSeconds >= spec.minRateIntervalSeconds, 'InvalidPolicySpec');
  requireValue(exact(context, CONTEXT_KEYS) && text(context.accountOpaqueId)
    && integer(context.accountGeneration) && integer(context.nowUnixSeconds)
    && nullableText(context.modelAlias), 'InvalidContext');
  requireValue(exact(prefs, PREF_KEYS) && ['off', 'percent', 'progressBar', 'runway'].includes(prefs.composerDisplay)
    && ['off', 'percent20', 'earlyWarning', 'custom'].includes(prefs.usageAlert)
    && typeof prefs.customRemainingPercent === 'number' && Number.isFinite(prefs.customRemainingPercent)
    && prefs.customRemainingPercent >= 0 && prefs.customRemainingPercent <= 100
    && typeof prefs.retryAfterReset === 'boolean' && typeof prefs.experimentalQuota === 'boolean', 'InvalidPreferences');
}
// DTOs come from the trusted projector, not a second hostile-wire parser. Validate
// policy-relevant values without cloning private inputs or calling a projector.
function snapshotDTO(snapshot, code = 'InvalidSnapshot') {
  if (snapshot === null) return;
  requireValue(record(snapshot) && text(snapshot.accountOpaqueId) && integer(snapshot.accountGeneration)
    && integer(snapshot.observedAtUnixSeconds) && (snapshot.ordinaryUsageAllowed === null
      || typeof snapshot.ordinaryUsageAllowed === 'boolean') && Array.isArray(snapshot.buckets), code);
  for (const bucket of snapshot.buckets) {
    requireValue(record(bucket) && ['bucketId', 'limitId', 'limitName', 'modelAlias', 'plan']
      .every(key => nullableText(bucket[key])) && Array.isArray(bucket.windows), code);
    const kinds = new Set();
    for (const window of bucket.windows) {
      requireValue(record(window) && ['primary', 'secondary'].includes(window.kind) && !kinds.has(window.kind)
        && typeof window.usedPercent === 'number' && Number.isFinite(window.usedPercent) && window.usedPercent >= 0
        && (window.windowDurationMins === null || (integer(window.windowDurationMins) && window.windowDurationMins > 0))
        && nullableTime(window.resetsAtUnixSeconds), code);
      kinds.add(window.kind);
    }
  }
}
function bound(snapshot, context) {
  return snapshot !== null && snapshot.accountOpaqueId === context.accountOpaqueId
    && snapshot.accountGeneration === context.accountGeneration;
}
function freshness(snapshot, context, spec) {
  if (snapshot === null) return 'missing';
  if (!bound(snapshot, context)) return 'identityMismatch';
  if (snapshot.observedAtUnixSeconds > context.nowUnixSeconds) return 'future';
  if (context.nowUnixSeconds - snapshot.observedAtUnixSeconds > spec.maxSnapshotAgeSeconds) return 'stale';
  return null;
}
function relevant(bucket, context) {
  return bucket.modelAlias === null || (context.modelAlias !== null && bucket.modelAlias === context.modelAlias);
}
function constrained(snapshot, context) {
  let selected = null;
  for (const bucket of snapshot.buckets) {
    if (!relevant(bucket, context)) continue;
    for (const window of bucket.windows) {
      if (selected === null || window.usedPercent > selected.window.usedPercent) selected = { bucket, window };
    }
  }
  return selected;
}
function publicWindow(snapshot, selected) {
  const { bucket, window } = selected;
  return { accountOpaqueId: snapshot.accountOpaqueId, accountGeneration: snapshot.accountGeneration,
    observedAtUnixSeconds: snapshot.observedAtUnixSeconds, bucketId: bucket.bucketId,
    limitId: bucket.limitId, limitName: bucket.limitName, modelAlias: bucket.modelAlias, plan: bucket.plan,
    kind: window.kind, usedPercent: window.usedPercent, windowDurationMins: window.windowDurationMins,
    resetsAtUnixSeconds: window.resetsAtUnixSeconds };
}
const unknownDisplay = (mode, reason) => ({ mode, status: 'unknown', reason, window: null });
function historyWindow(sample, context, selected) {
  if (!bound(sample, context)) return null;
  const matches = sample.buckets.filter(bucket => bucket.bucketId === selected.bucket.bucketId
    && bucket.modelAlias === selected.bucket.modelAlias && bucket.limitId === selected.bucket.limitId);
  if (matches.length !== 1) return null;
  const window = matches[0].windows.find(item => item.kind === selected.window.kind);
  return window && window.resetsAtUnixSeconds === selected.window.resetsAtUnixSeconds
    && window.windowDurationMins === selected.window.windowDurationMins ? window : null;
}
function runway(snapshot, context, spec, history, selected) {
  requireValue(Array.isArray(history) && history.length <= spec.maxHistorySamples, 'InvalidHistory');
  for (const sample of history) {
    requireValue(sample !== null, 'InvalidHistory');
    snapshotDTO(sample, 'InvalidHistory');
  }
  // Unknown reset identity or duration cannot support a same-window rate estimate.
  if (selected.window.resetsAtUnixSeconds === null || selected.window.windowDurationMins === null
    || historyWindow(snapshot, context, selected) === null) return { status: 'unknown', reason: 'unsupportedWindow' };
  const samples = [];
  let lastTime = -1;
  for (const sample of history) {
    if (sample.observedAtUnixSeconds <= lastTime || sample.observedAtUnixSeconds > snapshot.observedAtUnixSeconds) {
      return { status: 'unknown', reason: 'nonMonotonicHistory' };
    }
    lastTime = sample.observedAtUnixSeconds;
    const window = historyWindow(sample, context, selected);
    if (window === null) { samples.length = 0; continue; }
    samples.push({ time: sample.observedAtUnixSeconds, used: window.usedPercent });
  }
  const current = { time: snapshot.observedAtUnixSeconds, used: selected.window.usedPercent };
  const last = samples.at(-1);
  if (last?.time === current.time) {
    if (last.used !== current.used) return { status: 'unknown', reason: 'conflictingHistory' };
  } else samples.push(current);
  if (samples.length < 2) return { status: 'unknown', reason: 'insufficientHistory' };
  // A regression invalidates the current uninterrupted window's history.
  for (let index = 1; index < samples.length; index++) {
    if (samples[index].used < samples[index - 1].used) return { status: 'unknown', reason: 'usageRegression' };
  }
  const before = samples.at(-2);
  const elapsed = current.time - before.time;
  const increase = current.used - before.used;
  if (elapsed < spec.minRateIntervalSeconds || elapsed > spec.maxRateIntervalSeconds || increase <= 0) {
    return { status: 'unknown', reason: 'unsupportedRate' };
  }
  const rate = increase / elapsed;
  const seconds = Math.ceil(remaining(current.used) / rate);
  if (!Number.isFinite(rate) || rate <= 0 || !integer(seconds)) return { status: 'unknown', reason: 'nonFiniteEstimate' };
  return { status: 'known', remainingSeconds: seconds, estimate: true, beta: true };
}

/** OFF returns null without reading snapshot/history. Revoked experimentalQuota
 * blocks ALL usage display/alerts/retry, even cached observations. Percent retains
 * raw usage; only the progress bar adds clamped remainingPercent. Runway is a Beta estimate,
 * never a quota reset, permission, entitlement or restoration promise.
 * @param {CodexQuotaDTO|null} snapshot @param {CodexUsageContext} context
 * @param {CodexUsagePreferences} prefs @param {CodexUsagePolicySpec} spec
 * @param {ReadonlyArray<CodexQuotaDTO>} history Bounded chronological public samples, optionally including current.
 * @returns {CodexUsageDisplay|null} @throws {CodexUsagePolicyError}
 */
export function projectCodexUsageDisplay(snapshot, context, prefs, spec, history) {
  return guarded(() => {
    options(context, prefs, spec);
    if (prefs.composerDisplay === 'off') return null;
    if (!prefs.experimentalQuota) return unknownDisplay(prefs.composerDisplay, 'disabled');
    snapshotDTO(snapshot);
    const reason = freshness(snapshot, context, spec);
    if (reason) return unknownDisplay(prefs.composerDisplay, reason);
    const selected = constrained(snapshot, context);
    if (selected === null) return unknownDisplay(prefs.composerDisplay, 'noRelevantWindow');
    const window = publicWindow(snapshot, selected);
    if (prefs.composerDisplay === 'runway') {
      const estimate = runway(snapshot, context, spec, history, selected);
      return { mode: 'runway', ...estimate, window, estimate: true, beta: true };
    }
    return { mode: prefs.composerDisplay, status: 'known', window,
      ...(prefs.composerDisplay === 'progressBar' ? { remainingPercent: remaining(window.usedPercent) } : {}) };
  }, 'InvalidSnapshot');
}
function scope(snapshot, selected) {
  return { accountOpaqueId: snapshot.accountOpaqueId, accountGeneration: snapshot.accountGeneration,
    bucketId: selected.bucket.bucketId, kind: selected.window.kind, resetsAtUnixSeconds: selected.window.resetsAtUnixSeconds };
}
function alertMarker(marker) {
  if (marker === null) return;
  requireValue(exact(marker, ['scope', 'settingsFingerprint', 'belowThreshold'])
    && exact(marker.scope, ['accountOpaqueId', 'accountGeneration', 'bucketId', 'kind', 'resetsAtUnixSeconds'])
    && text(marker.scope.accountOpaqueId) && integer(marker.scope.accountGeneration)
    && nullableText(marker.scope.bucketId) && ['primary', 'secondary'].includes(marker.scope.kind)
    && nullableTime(marker.scope.resetsAtUnixSeconds) && text(marker.settingsFingerprint)
    && typeof marker.belowThreshold === 'boolean', 'InvalidAlertState');
}

/** Caller stores nextMarker and delivers event. First known low observation of a
 * new scope/settings arms once; recovery rearms, unknown preserves matching state.
 * No actual notification occurs. previousAlert must be null or a returned marker.
 * @param {CodexQuotaDTO|null} snapshot @param {CodexUsageContext} context
 * @param {CodexUsagePreferences} prefs @param {CodexUsagePolicySpec} spec
 * @param {ReadonlyArray<CodexQuotaDTO>} history @param {CodexUsageAlertMarker|null} previousAlert
 * @returns {{event:CodexUsageAlertEvent|null,nextMarker:CodexUsageAlertMarker|null}} @throws {CodexUsagePolicyError}
 */
export function decideCodexUsageAlert(snapshot, context, prefs, spec, history, previousAlert) {
  return guarded(() => {
    options(context, prefs, spec);
    if (prefs.usageAlert === 'off' || !prefs.experimentalQuota) return { event: null, nextMarker: null };
    alertMarker(previousAlert);
    snapshotDTO(snapshot);
    const retained = previousAlert !== null && previousAlert.scope.accountOpaqueId === context.accountOpaqueId
      && previousAlert.scope.accountGeneration === context.accountGeneration ? previousAlert : null;
    if (freshness(snapshot, context, spec)) return { event: null, nextMarker: retained };
    const selected = constrained(snapshot, context);
    if (selected === null) return { event: null, nextMarker: retained };
    const threshold = prefs.usageAlert === 'percent20' ? 20 : prefs.customRemainingPercent;
    const fingerprint = JSON.stringify(prefs.usageAlert === 'earlyWarning'
      ? [prefs.usageAlert, spec.earlyWarningSeconds, prefs.experimentalQuota]
      : [prefs.usageAlert, threshold]);
    let belowThreshold;
    let estimate = null;
    if (prefs.usageAlert === 'earlyWarning') {
      estimate = runway(snapshot, context, spec, history, selected);
      if (estimate.status !== 'known') return { event: null, nextMarker: retained };
      belowThreshold = estimate.remainingSeconds <= spec.earlyWarningSeconds;
    } else belowThreshold = remaining(selected.window.usedPercent) <= threshold;
    const nextMarker = { scope: scope(snapshot, selected), settingsFingerprint: fingerprint, belowThreshold };
    const same = retained !== null && retained.settingsFingerprint === fingerprint
      && Object.keys(nextMarker.scope).every(key => retained.scope[key] === nextMarker.scope[key]);
    const event = belowThreshold && (!same || !retained.belowThreshold)
      ? { mode: prefs.usageAlert, window: publicWindow(snapshot, selected),
        ...(estimate === null ? { remainingPercent: remaining(selected.window.usedPercent), thresholdRemainingPercent: threshold }
          : { remainingSeconds: estimate.remainingSeconds, estimate: true, beta: true }) } : null;
    return { event, nextMarker };
  }, 'InvalidAlertState');
}
const retryResult = (decision, reason, intent = null) => ({ decision, reason,
  waitUntilUnixSeconds: decision === 'wait' ? intent.resetAtUnixSeconds : null,
  requestDeadlineUnixSeconds: intent === null ? null : intent.requestDeadlineUnixSeconds });

/** Intent resetAtUnixSeconds must be the original trusted quota reset, not a
 * fabricated clock hint. Before reset: bounded wait. After reset: fresh explicit
 * permission AND capacity in every relevant window. Caller consumes/increments
 * attempts and revalidates cancellation, ID, immutable original model/account
 * binding and deadline before sending. Intent modelAlias is captured from the
 * original failed request, never refreshed from a changed selection. quotaDenied
 * MUST be an authoritatively known original quota denial before execution, never
 * inferred from error prose, zero output or missing usage. No-output timeout or
 * indeterminate outcome is not safe to resend. A pure decision cannot itself
 * provide exactly-once execution or send anything.
 * @param {CodexQuotaDTO|null} snapshot @param {CodexUsageContext} context
 * @param {CodexUsagePreferences} prefs @param {CodexUsagePolicySpec} spec
 * @param {CodexResetRetryIntent} retryIntent
 * @returns {CodexResetRetryDecision} @throws {CodexUsagePolicyError}
 */
export function decideCodexResetRetry(snapshot, context, prefs, spec, retryIntent) {
  return guarded(() => {
    options(context, prefs, spec);
    if (!prefs.retryAfterReset || !prefs.experimentalQuota) return retryResult('off', 'disabled');
    const intent = retryIntent;
    requireValue(exact(intent, INTENT_KEYS) && text(intent.requestId) && text(intent.currentRequestId)
      && text(intent.accountOpaqueId) && integer(intent.accountGeneration) && nullableText(intent.modelAlias)
      && integer(intent.attempts) && ['quotaDenied', 'other', 'indeterminate'].includes(intent.failureKind)
      && typeof intent.failedBeforeOutput === 'boolean' && typeof intent.cancelled === 'boolean'
      && nullableTime(intent.resetAtUnixSeconds) && integer(intent.requestDeadlineUnixSeconds), 'InvalidRetryIntent');
    if (intent.cancelled) return retryResult('blocked', 'cancelled', intent);
    if (intent.requestId !== intent.currentRequestId) return retryResult('blocked', 'requestMismatch', intent);
    if (intent.accountOpaqueId !== context.accountOpaqueId || intent.accountGeneration !== context.accountGeneration) {
      return retryResult('blocked', 'identityMismatch', intent);
    }
    if (intent.modelAlias !== context.modelAlias) return retryResult('blocked', 'modelMismatch', intent);
    if (intent.failureKind === 'other') return retryResult('blocked', 'failureNotQuotaDenied', intent);
    if (intent.failureKind === 'indeterminate') return retryResult('blocked', 'indeterminateOperation', intent);
    if (!intent.failedBeforeOutput) return retryResult('blocked', 'partialOutput', intent);
    if (intent.attempts >= spec.maxRetryAttempts) return retryResult('blocked', 'attemptLimit', intent);
    if (context.nowUnixSeconds >= intent.requestDeadlineUnixSeconds) return retryResult('blocked', 'deadline', intent);
    if (intent.resetAtUnixSeconds === null) return retryResult('observe', 'unknownReset', intent);
    if (Math.abs(intent.resetAtUnixSeconds - context.nowUnixSeconds) > spec.maxRetryWaitSeconds
      || intent.resetAtUnixSeconds >= intent.requestDeadlineUnixSeconds) return retryResult('blocked', 'resetOutsideBounds', intent);
    if (context.nowUnixSeconds < intent.resetAtUnixSeconds) return retryResult('wait', 'awaitReset', intent);
    snapshotDTO(snapshot);
    const reason = freshness(snapshot, context, spec);
    if (reason) return retryResult('observe', reason, intent);
    if (snapshot.observedAtUnixSeconds < intent.resetAtUnixSeconds) return retryResult('observe', 'preResetObservation', intent);
    if (snapshot.ordinaryUsageAllowed === null) return retryResult('observe', 'unknownPermission', intent);
    if (snapshot.ordinaryUsageAllowed === false) return retryResult('blocked', 'usageDisallowed', intent);
    const selected = constrained(snapshot, context);
    if (selected === null) return retryResult('observe', 'noRelevantWindow', intent);
    if (selected.window.usedPercent >= 100) return retryResult('blocked', 'noCapacity', intent);
    return retryResult('retry', 'freshPermissionAndCapacity', intent);
  }, 'InvalidRetryIntent');
}
