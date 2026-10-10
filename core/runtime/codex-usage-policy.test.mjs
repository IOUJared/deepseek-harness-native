import test from 'node:test';
import assert from 'node:assert/strict';
import { CodexUsagePolicyError, projectCodexUsageDisplay as display,
  decideCodexUsageAlert as alert, decideCodexResetRetry as retry } from './codex-usage-policy.mjs';

// Entirely synthetic PUBLIC DTOs: no runtime, account APIs, credentials or clocks.
const context = { accountOpaqueId: 'fake-public-account', accountGeneration: 3,
  nowUnixSeconds: 1000, modelAlias: 'fake-model' };
const spec = { maxSnapshotAgeSeconds: 60, maxHistorySamples: 8, minRateIntervalSeconds: 10,
  maxRateIntervalSeconds: 300, maxRetryWaitSeconds: 600, maxRetryAttempts: 1, earlyWarningSeconds: 30 };
const prefs = { composerDisplay: 'percent', usageAlert: 'percent20', customRemainingPercent: 20,
  retryAfterReset: false, experimentalQuota: true };
const beta = { ...prefs, experimentalQuota: true };
const retryPrefs = { ...beta, retryAfterReset: true };
const windowDTO = (usedPercent = 80, patch = {}) => ({ kind: 'primary', usedPercent,
  windowDurationMins: 300, resetsAtUnixSeconds: 1200, ...patch });
const bucketDTO = (windows = [windowDTO()], patch = {}) => ({ bucketId: 'fake-limit',
  limitId: 'fake-limit', limitName: 'Fake public limit', modelAlias: null, plan: 'plus', windows, ...patch });
const snapshot = (usedPercent = 80, patch = {}) => ({ accountOpaqueId: context.accountOpaqueId,
  accountGeneration: context.accountGeneration, observedAtUnixSeconds: context.nowUnixSeconds,
  identityBinding: 'ownedTransport', ordinaryUsageAllowed: null,
  buckets: [bucketDTO([windowDTO(usedPercent)])], ...patch });
const intent = (patch = {}) => ({ requestId: 'fake-request', currentRequestId: 'fake-request',
  accountOpaqueId: context.accountOpaqueId, accountGeneration: context.accountGeneration, modelAlias: context.modelAlias,
  failureKind: 'quotaDenied', failedBeforeOutput: true, cancelled: false, attempts: 0, resetAtUnixSeconds: 1000,
  requestDeadlineUnixSeconds: 1600, ...patch });
const callDisplay = (value = snapshot(), p = prefs, c = context, s = spec, h = []) => display(value, c, p, s, h);
const callAlert = (value = snapshot(), p = prefs, h = [], previous = null, c = context, s = spec) => alert(value, c, p, s, h, previous);
const callRetry = (value = snapshot(20, { ordinaryUsageAllowed: true }), i = intent(), c = context, p = retryPrefs, s = spec) => retry(value, c, p, s, i);
const fixedError = code => error => {
  assert.ok(error instanceof CodexUsagePolicyError);
  assert.equal(error.code, code);
  assert.equal(error.message, code);
  assert.equal(Object.hasOwn(error, 'cause'), false);
  return true;
};

// Every enum branch, raw usage, actual window metadata and model selection.
test('unwritten default preference DTO stays off/20/no retry/no Beta', () => {
  const defaults = { composerDisplay: 'off', usageAlert: 'off', customRemainingPercent: 20,
    retryAfterReset: false, experimentalQuota: false };
  assert.equal(display(undefined, context, defaults, spec, undefined), null);
  assert.deepEqual(alert(undefined, context, defaults, spec, undefined, null), { event: null, nextMarker: null });
  assert.equal(retry(undefined, context, defaults, spec, undefined).decision, 'off');
});
test('off display performs no hidden snapshot/history observation', () => {
  assert.equal(callDisplay(undefined, { ...prefs, composerDisplay: 'off' }, context, spec, undefined), null);
});
test('Beta revocation blocks all modes and clears alert state without reading cached quota', () => {
  const previous = callAlert(snapshot(90)).nextMarker;
  for (const mode of ['percent', 'progressBar', 'runway']) {
    assert.deepEqual(callDisplay(undefined, { ...prefs, composerDisplay: mode, experimentalQuota: false }, context, spec, undefined),
      { mode, status: 'unknown', reason: 'disabled', window: null });
  }
  for (const mode of ['percent20', 'custom', 'earlyWarning']) {
    assert.deepEqual(alert(undefined, context, { ...prefs, usageAlert: mode, experimentalQuota: false }, spec, undefined, previous),
      { event: null, nextMarker: null });
  }
  assert.equal(callDisplay(snapshot(), { ...prefs, composerDisplay: 'off', experimentalQuota: false }), null);
  assert.equal(callRetry(undefined, undefined, context, { ...retryPrefs, experimentalQuota: false }).decision, 'off');
});
test('percent retains >100 raw usage and does not invent remaining or timing', () => {
  const result = callDisplay(snapshot(121, { buckets: [bucketDTO([windowDTO(121,
    { windowDurationMins: null, resetsAtUnixSeconds: null })])] }));
  assert.equal(result.status, 'known');
  assert.equal(result.window.usedPercent, 121);
  assert.equal(result.window.windowDurationMins, null);
  assert.equal(result.window.resetsAtUnixSeconds, null);
  assert.equal(Object.hasOwn(result, 'remainingPercent'), false);
  assert.equal(Object.hasOwn(result, 'remainingSeconds'), false);
});
test('progress bar clamps remaining while preserving raw usage', () => {
  for (const [used, left] of [[0, 100], [12.5, 87.5], [100, 0], [121, 0]]) {
    const result = callDisplay(snapshot(used), { ...prefs, composerDisplay: 'progressBar' });
    assert.equal(result.remainingPercent, left);
    assert.equal(result.window.usedPercent, used);
  }
});
test('most constrained primary/secondary across exact model and general buckets', () => {
  const buckets = [bucketDTO([windowDTO(99)], { bucketId: 'unrelated', modelAlias: 'other-model' }),
    bucketDTO([windowDTO(30), windowDTO(77, { kind: 'secondary', windowDurationMins: 12345 })], { bucketId: 'general' }),
    bucketDTO([windowDTO(88)], { bucketId: 'matching', modelAlias: 'fake-model' })];
  const input = snapshot(0, { buckets });
  assert.equal(callDisplay(input).window.bucketId, 'matching');
  const general = callDisplay(input, prefs, { ...context, modelAlias: null });
  assert.equal(general.window.bucketId, 'general');
  assert.equal(general.window.kind, 'secondary');
  assert.equal(general.window.windowDurationMins, 12345);
});
test('model match is exact, unrelated-only and empty windows are unknown', () => {
  for (const buckets of [[], [bucketDTO([])], [bucketDTO([windowDTO()], { modelAlias: 'FAKE-MODEL' })]]) {
    assert.deepEqual(callDisplay(snapshot(0, { buckets })),
      { mode: 'percent', status: 'unknown', reason: 'noRelevantWindow', window: null });
  }
});
test('tie selection is stable and UTF-8 public labels survive without private payload copies', () => {
  const input = snapshot(0, { buckets: [bucketDTO([windowDTO()], { limitName: '合成 β quota' }),
    bucketDTO([windowDTO()], { bucketId: 'second' })] });
  const result = callDisplay(input);
  assert.equal(result.window.limitName, '合成 β quota');
  assert.equal(result.window.bucketId, 'fake-limit');
  assert.notEqual(result.window, input.buckets[0].windows[0]);
  assert.equal(Object.hasOwn(result, 'buckets'), false);
});
test('freshness and identity boundaries produce unknown without values', () => {
  const cases = [[null, 'missing'], [snapshot(80, { observedAtUnixSeconds: 939 }), 'stale'],
    [snapshot(80, { observedAtUnixSeconds: 1001 }), 'future'],
    [snapshot(80, { accountOpaqueId: 'different-account' }), 'identityMismatch'],
    [snapshot(80, { accountGeneration: 4 }), 'identityMismatch']];
  for (const [value, reason] of cases) {
    assert.deepEqual(callDisplay(value), { mode: 'percent', status: 'unknown', reason, window: null });
    assert.equal(callAlert(value).event, null);
  }
  assert.equal(callDisplay(snapshot(80, { observedAtUnixSeconds: 940 })).status, 'known');
  assert.equal(callDisplay(snapshot(80, { observedAtUnixSeconds: 1000 })).status, 'known');
});

// Deployment limits and public DTO validation: no numeric/boolean coercion.
test('every deployment bound is required positive safe integer', () => {
  for (const key of Object.keys(spec)) {
    for (const value of [undefined, 0, -1, 0.5, '10', Infinity, NaN, Number.MAX_SAFE_INTEGER + 1]) {
      assert.throws(() => callDisplay(snapshot(), prefs, context, { ...spec, [key]: value }), fixedError('InvalidPolicySpec'));
    }
    const missing = { ...spec }; delete missing[key];
    assert.throws(() => callDisplay(snapshot(), prefs, context, missing), fixedError('InvalidPolicySpec'));
  }
  assert.throws(() => callDisplay(snapshot(), prefs, context, { ...spec, unexpected: 1 }), fixedError('InvalidPolicySpec'));
  assert.throws(() => callDisplay(snapshot(), prefs, context, { ...spec, minRateIntervalSeconds: 301 }), fixedError('InvalidPolicySpec'));
  assert.equal(callDisplay(snapshot(), prefs, context,
    { ...spec, minRateIntervalSeconds: 10, maxRateIntervalSeconds: 10 }).status, 'known');
});
test('invalid context and preferences give fixed codes only', () => {
  for (const patch of [{ accountOpaqueId: '' }, { accountGeneration: '3' }, { accountGeneration: -1 },
    { nowUnixSeconds: 1000.5 }, { modelAlias: 1 }, { extra: 'fake-secret-never-echo' }]) {
    assert.throws(() => callDisplay(snapshot(), prefs, { ...context, ...patch }), fixedError('InvalidContext'));
  }
  for (const patch of [{ composerDisplay: 'bogus' }, { usageAlert: 'bogus' }, { customRemainingPercent: -1 },
    { customRemainingPercent: 101 }, { customRemainingPercent: NaN }, { customRemainingPercent: '20' },
    { retryAfterReset: 1 }, { experimentalQuota: 'true' }, { extra: 'fake-secret-never-echo' }]) {
    assert.throws(() => callDisplay(snapshot(), { ...prefs, ...patch }), fixedError('InvalidPreferences'));
  }
  for (const value of [0, 100, 0.5]) {
    assert.equal(callDisplay(snapshot(), { ...prefs, customRemainingPercent: value }).status, 'known');
  }
  const error = new CodexUsagePolicyError('fake-secret-never-echo');
  assert.equal(error.code, 'InvalidSnapshot');
  assert.equal(error.message, 'InvalidSnapshot');
});
test('malformed relevant public DTO fields reject without coercion', () => {
  const inputs = [undefined, {}, snapshot(80, { ordinaryUsageAllowed: 'true' }),
    snapshot(80, { observedAtUnixSeconds: '1000' }), snapshot(80, { buckets: null }),
    snapshot(80, { buckets: [bucketDTO([windowDTO(-1)])] }),
    snapshot(80, { buckets: [bucketDTO([windowDTO(Infinity)])] }),
    snapshot(80, { buckets: [bucketDTO([windowDTO('80')])] }),
    snapshot(80, { buckets: [bucketDTO([windowDTO(80, { kind: 'weekly' })])] }),
    snapshot(80, { buckets: [bucketDTO([windowDTO(80, { windowDurationMins: 0 })])] }),
    snapshot(80, { buckets: [bucketDTO([windowDTO(80, { resetsAtUnixSeconds: -1 })])] }),
    snapshot(80, { buckets: [bucketDTO([windowDTO(), windowDTO()])] })];
  for (const input of inputs) {
    assert.throws(() => display(input, context, prefs, spec, []), fixedError('InvalidSnapshot'));
  }
});

// Runway: actual same-window sample rates only, never fictional defaults.
const runwayPrefs = { ...beta, composerDisplay: 'runway' };
const prior = (used = 70, observedAtUnixSeconds = 990, patch = {}) => snapshot(used, { observedAtUnixSeconds, ...patch });
test('runway is experimental, explicitly estimated and separate from reset', () => {
  const result = callDisplay(snapshot(80), runwayPrefs, context, spec, [prior()]);
  assert.equal(result.status, 'known');
  assert.equal(result.remainingSeconds, 20);
  assert.equal(result.estimate, true);
  assert.equal(result.beta, true);
  assert.equal(result.window.resetsAtUnixSeconds, 1200);
  assert.notEqual(result.remainingSeconds, result.window.resetsAtUnixSeconds - context.nowUnixSeconds);
  assert.equal(Object.hasOwn(result, 'ordinaryUsageAllowed'), false);
  assert.equal(callDisplay(snapshot(), { ...prefs, composerDisplay: 'runway', experimentalQuota: false }, context, spec, undefined).reason,
    'disabled');
});
test('runway supports current in history without double counting', () => {
  assert.equal(callDisplay(snapshot(80), runwayPrefs, context, spec, [prior(), snapshot(80)]).remainingSeconds, 20);
  assert.equal(callDisplay(snapshot(121), runwayPrefs, context, spec, [prior(120)]).remainingSeconds, 0);
});
test('runway min/max elapsed boundaries are inclusive', () => {
  for (const elapsed of [10, 300]) {
    const result = callDisplay(snapshot(80), runwayPrefs, context, spec, [prior(70, 1000 - elapsed)]);
    assert.equal(result.remainingSeconds, 2 * elapsed);
  }
  for (const elapsed of [9, 301]) {
    assert.equal(callDisplay(snapshot(80), runwayPrefs, context, spec, [prior(70, 1000 - elapsed)]).status, 'unknown');
  }
});
test('runway insufficient, flat and regressing histories remain unknown', () => {
  const cases = [[], [snapshot(80)], [prior(80)], [prior(81)], [prior(60, 980), prior(50, 990)]];
  for (const h of cases) {
    const result = callDisplay(snapshot(80), runwayPrefs, context, spec, h);
    assert.equal(result.status, 'unknown');
    assert.equal(Object.hasOwn(result, 'remainingSeconds'), false);
  }
});
test('runway cannot bridge accounts, generations, bucket IDs, model or reset/duration identities', () => {
  const changed = [prior(70, 990, { accountOpaqueId: 'other-account' }),
    prior(70, 990, { accountGeneration: 2 }),
    prior(70, 990, { buckets: [bucketDTO([windowDTO(70)], { bucketId: 'different-limit' })] }),
    prior(70, 990, { buckets: [bucketDTO([windowDTO(70)], { modelAlias: 'fake-model' })] }),
    prior(70, 990, { buckets: [bucketDTO([windowDTO(70, { resetsAtUnixSeconds: 1100 })])] }),
    prior(70, 990, { buckets: [bucketDTO([windowDTO(70, { windowDurationMins: 600 })])] }),
    prior(70, 990, { buckets: [bucketDTO([windowDTO(70, { kind: 'secondary' })])] })];
  for (const sample of changed) {
    assert.equal(callDisplay(snapshot(80), runwayPrefs, context, spec, [sample]).status, 'unknown');
  }
  // A reset re-arms only after two actual observations in the new reset identity.
  const old = prior(95, 980, { buckets: [bucketDTO([windowDTO(95, { resetsAtUnixSeconds: 1100 })])] });
  assert.equal(callDisplay(snapshot(80), runwayPrefs, context, spec, [old, prior()]).status, 'known');
});
test('runway unknown reset/duration, ambiguous bucket and nonfinite estimates are unsupported', () => {
  for (const patch of [{ resetsAtUnixSeconds: null }, { windowDurationMins: null }]) {
    const value = snapshot(80, { buckets: [bucketDTO([windowDTO(80, patch)])] });
    assert.equal(callDisplay(value, runwayPrefs, context, spec, [prior()]).reason, 'unsupportedWindow');
  }
  const duplicate = snapshot(80, { buckets: [bucketDTO([windowDTO(80)]), bucketDTO([windowDTO(70)])] });
  assert.equal(callDisplay(duplicate, runwayPrefs, context, spec, [prior()]).status, 'unknown');
  const result = callDisplay(snapshot(Number.MIN_VALUE), runwayPrefs, context, spec, [prior(0)]);
  assert.equal(result.status, 'unknown');
  assert.equal(result.reason, 'nonFiniteEstimate');
});
test('runway requires latest chronological current snapshot and consistent current observation', () => {
  for (const h of [[prior(70, 1001)], [prior(70, 990), prior(75, 980)], [prior(70), prior(75)],
    [prior(), snapshot(79)]]) {
    assert.equal(callDisplay(snapshot(80), runwayPrefs, context, spec, h).status, 'unknown');
  }
  assert.equal(callDisplay(snapshot(80, { observedAtUnixSeconds: 939 }), runwayPrefs, context, spec, [prior()]).reason, 'stale');
});
test('bounded histories reject oversize, invalid and coercive input without truncation', () => {
  for (const h of [undefined, null, {}, [null], [prior(70, '990')], Array(spec.maxHistorySamples + 1).fill(prior())]) {
    assert.throws(() => display(snapshot(), context, runwayPrefs, spec, h), fixedError('InvalidHistory'));
  }
  const h = Array.from({ length: spec.maxHistorySamples }, (_, i) => prior(10 + i, 900 + 10 * i));
  assert.equal(callDisplay(snapshot(80), runwayPrefs, context, spec, h).status, 'known');
  // Non-rate modes do not inspect unused history.
  assert.equal(callDisplay(snapshot(), prefs, context, spec, undefined).status, 'known');
});

// Alerts return caller-owned state/events, never notify or repeatedly emit.
test('off alert clears state without snapshot/history observation', () => {
  assert.deepEqual(alert(undefined, context, { ...prefs, usageAlert: 'off' }, spec, undefined, undefined),
    { event: null, nextMarker: null });
});
test('20 percent means remaining, inclusive threshold, one event then recovery rearm', () => {
  const above = callAlert(snapshot(79));
  assert.equal(above.event, null);
  const crossed = callAlert(snapshot(80), prefs, [], above.nextMarker);
  assert.equal(crossed.event.remainingPercent, 20);
  assert.equal(crossed.event.thresholdRemainingPercent, 20);
  assert.equal(callAlert(snapshot(90), prefs, [], crossed.nextMarker).event, null);
  const recovered = callAlert(snapshot(60), prefs, [], crossed.nextMarker);
  assert.equal(recovered.event, null);
  assert.equal(recovered.nextMarker.belowThreshold, false);
  assert.notEqual(callAlert(snapshot(121), prefs, [], recovered.nextMarker).event, null);
});
test('custom thresholds 0/100/fraction and settings changes arm a new marker', () => {
  for (const [threshold, used, fires] of [[0, 99, false], [0, 100, true], [100, 0, true], [15.5, 84.5, true]]) {
    const result = callAlert(snapshot(used), { ...prefs, usageAlert: 'custom', customRemainingPercent: threshold });
    assert.equal(result.event !== null, fires);
  }
  const first = callAlert(snapshot(90));
  assert.notEqual(callAlert(snapshot(90), { ...prefs, usageAlert: 'custom', customRemainingPercent: 15 }, [], first.nextMarker).event, null);
  const custom = callAlert(snapshot(90), { ...prefs, usageAlert: 'custom', customRemainingPercent: 15 });
  assert.notEqual(callAlert(snapshot(90), { ...prefs, usageAlert: 'custom', customRemainingPercent: 20 }, [], custom.nextMarker).event, null);
  // Changing unused custom threshold does not spam percent20.
  assert.equal(callAlert(snapshot(90), { ...prefs, customRemainingPercent: 99 }, [], first.nextMarker).event, null);
});
test('account/generation, window and reset scopes arm separately', () => {
  const first = callAlert(snapshot(90));
  for (const [value, c] of [[snapshot(90, { accountOpaqueId: 'other' }), { ...context, accountOpaqueId: 'other' }],
    [snapshot(90, { accountGeneration: 4 }), { ...context, accountGeneration: 4 }],
    [snapshot(90, { buckets: [bucketDTO([windowDTO(90)], { bucketId: 'other' })] }), context],
    [snapshot(90, { buckets: [bucketDTO([windowDTO(90, { kind: 'secondary' })])] }), context],
    [snapshot(90, { buckets: [bucketDTO([windowDTO(90, { resetsAtUnixSeconds: 1400 })])] }), context]]) {
    assert.notEqual(callAlert(value, prefs, [], first.nextMarker, c).event, null);
  }
});
test('unknown alert does not emit or rearm known low state; account switch clears it', () => {
  const first = callAlert(snapshot(90));
  const unknown = callAlert(null, prefs, [], first.nextMarker);
  assert.equal(unknown.event, null);
  assert.deepEqual(unknown.nextMarker, first.nextMarker);
  assert.equal(callAlert(snapshot(90), prefs, [], unknown.nextMarker).event, null);
  const switched = callAlert(null, prefs, [], first.nextMarker, { ...context, accountGeneration: 4 });
  assert.equal(switched.nextMarker, null);
});
test('marker property order is irrelevant and malformed marker has fixed error', () => {
  const first = callAlert(snapshot(90));
  const keys = Object.keys(first.nextMarker.scope).reverse();
  const reordered = { ...first.nextMarker, scope: Object.fromEntries(keys.map(key => [key, first.nextMarker.scope[key]])) };
  assert.equal(callAlert(snapshot(90), prefs, [], reordered).event, null);
  for (const marker of [undefined, {}, { ...first.nextMarker, belowThreshold: 'true' },
    { ...first.nextMarker, scope: { ...first.nextMarker.scope, resetsAtUnixSeconds: '1200' } }]) {
    assert.throws(() => alert(snapshot(), context, prefs, spec, [], marker), fixedError('InvalidAlertState'));
  }
});
test('early warning needs Beta known runway <= configured seconds, not reset clock', () => {
  const early = { ...beta, usageAlert: 'earlyWarning' };
  const first = callAlert(snapshot(80), early, [prior(70)]);
  assert.equal(first.event.remainingSeconds, 20);
  assert.equal(first.event.estimate, true);
  assert.equal(first.event.beta, true);
  assert.equal(callAlert(snapshot(80), early, [prior(70)], first.nextMarker).event, null);
  assert.equal(callAlert(snapshot(80), early, []).event, null);
  assert.equal(callAlert(snapshot(80), { ...prefs, usageAlert: 'earlyWarning', experimentalQuota: false }, undefined).event, null);
  assert.equal(callAlert(snapshot(80), early, [prior(75)]).event, null); // 40 > 30 seconds.
  const equality = callAlert(snapshot(70), early, [prior(60)]);
  assert.equal(equality.event.remainingSeconds, 30);
  const changedLimit = { ...spec, earlyWarningSeconds: 25 };
  assert.notEqual(callAlert(snapshot(80), early, [prior(70)], first.nextMarker, context, changedLimit).event, null);
});

// Reset retry: explicit consent plus experimental gate; no clock-only restoration.
test('retry needs both explicit enablement and experimentalQuota; disabled does not observe', () => {
  for (const p of [prefs, beta, { ...prefs, retryAfterReset: true, experimentalQuota: false }]) {
    assert.deepEqual(retry(undefined, context, p, spec, undefined), {
      decision: 'off', reason: 'disabled', waitUntilUnixSeconds: null, requestDeadlineUnixSeconds: null });
  }
});
test('retry validation never coerces intents or leaks values', () => {
  for (const patch of [{ attempts: -1 }, { attempts: '0' }, { cancelled: 0 }, { failedBeforeOutput: 'true' },
    { requestId: '' }, { currentRequestId: 1 }, { accountGeneration: '3' }, { resetAtUnixSeconds: '1000' },
    { modelAlias: undefined }, { modelAlias: 1 }, { modelAlias: false },
    { failureKind: undefined }, { failureKind: null }, { failureKind: 'quota_denied' },
    { failureKind: 'timeout' }, { failureKind: 'QUOTADENIED' }, { failureKind: 1 },
    { requestDeadlineUnixSeconds: Infinity }, { extra: 'fake-secret-never-echo' }]) {
    assert.throws(() => callRetry(snapshot(), intent(patch)), fixedError('InvalidRetryIntent'));
  }
  for (const key of ['resetAtUnixSeconds', 'modelAlias', 'failureKind']) {
    const missing = intent(); delete missing[key];
    assert.throws(() => callRetry(snapshot(), missing), fixedError('InvalidRetryIntent'));
  }
});
test('partial/tool/model output, cancellation, changed request/account, exhausted attempts and deadline block', () => {
  const cases = [[{ failedBeforeOutput: false }, 'partialOutput'], [{ cancelled: true }, 'cancelled'],
    [{ currentRequestId: 'different-request' }, 'requestMismatch'], [{ accountOpaqueId: 'other' }, 'identityMismatch'],
    [{ accountGeneration: 4 }, 'identityMismatch'], [{ attempts: 1 }, 'attemptLimit'],
    [{ attempts: 2 }, 'attemptLimit'], [{ requestDeadlineUnixSeconds: 1000 }, 'deadline'],
    [{ requestDeadlineUnixSeconds: 999 }, 'deadline']];
  for (const [patch, reason] of cases) {
    const result = callRetry(undefined, intent(patch));
    assert.equal(result.decision, 'blocked');
    assert.equal(result.reason, reason);
  }
  assert.equal(callRetry(undefined, intent({ cancelled: true, resetAtUnixSeconds: 1200 })).reason, 'cancelled');
});
test('zero output cannot authorize resending other or indeterminate failures', () => {
  for (const [failureKind, reason] of [['other', 'failureNotQuotaDenied'], ['indeterminate', 'indeterminateOperation']]) {
    for (const resetAtUnixSeconds of [1100, 1000, null]) {
      for (const quota of [undefined, null, snapshot(0, { ordinaryUsageAllowed: true })]) {
        const result = retry(quota, context, retryPrefs, spec,
          intent({ failureKind, failedBeforeOutput: true, resetAtUnixSeconds }));
        assert.equal(result.decision, 'blocked');
        assert.equal(result.reason, reason);
        assert.equal(result.waitUntilUnixSeconds, null);
      }
    }
  }
  // Only the caller's authoritative pre-execution quota denial admits safe policy states.
  assert.equal(callRetry(snapshot(0, { ordinaryUsageAllowed: true }), intent({ failureKind: 'quotaDenied' })).decision, 'retry');
  assert.equal(callRetry(null, intent({ failureKind: 'quotaDenied', resetAtUnixSeconds: 1100 })).decision, 'wait');
  assert.equal(callRetry(null, intent({ failureKind: 'quotaDenied', resetAtUnixSeconds: null })).decision, 'observe');
});
test('original immutable quota-model binding cannot change before wait, observe or retry', () => {
  for (const [original, current] of [['fake-model', 'other-model'], ['fake-model', null], [null, 'fake-model']]) {
    const c = { ...context, modelAlias: current };
    for (const reset of [1100, null, 1000]) {
      const result = retry(snapshot(20, { ordinaryUsageAllowed: true }), c, retryPrefs, spec,
        intent({ modelAlias: original, resetAtUnixSeconds: reset }));
      assert.equal(result.decision, 'blocked');
      assert.equal(result.reason, 'modelMismatch');
      assert.equal(result.waitUntilUnixSeconds, null);
    }
  }
  for (const alias of ['fake-model', null]) {
    const c = { ...context, modelAlias: alias };
    assert.equal(callRetry(snapshot(20, { ordinaryUsageAllowed: true }), intent({ modelAlias: alias }), c).decision, 'retry');
    assert.equal(callRetry(null, intent({ modelAlias: alias, resetAtUnixSeconds: 1100 }), c).decision, 'wait');
    assert.equal(callRetry(null, intent({ modelAlias: alias, resetAtUnixSeconds: null }), c).decision, 'observe');
  }
  // Unknown original/current mapping must not borrow a specific model's capacity.
  const specificOnly = snapshot(20, { ordinaryUsageAllowed: true,
    buckets: [bucketDTO([windowDTO(20)], { modelAlias: 'fake-model' })] });
  assert.equal(callRetry(specificOnly, intent({ modelAlias: null }), { ...context, modelAlias: null }).reason, 'noRelevantWindow');
});
test('before known reset return wait bounded by explicit deadline/horizon, without hidden observation', () => {
  const result = callRetry(undefined, intent({ resetAtUnixSeconds: 1100 }));
  assert.deepEqual(result, { decision: 'wait', reason: 'awaitReset', waitUntilUnixSeconds: 1100,
    requestDeadlineUnixSeconds: 1600 });
  assert.equal(callRetry(undefined, intent({ resetAtUnixSeconds: 1600, requestDeadlineUnixSeconds: 1601 })).decision, 'wait');
  for (const patch of [{ resetAtUnixSeconds: 1601, requestDeadlineUnixSeconds: 1700 },
    { resetAtUnixSeconds: 1200, requestDeadlineUnixSeconds: 1200 },
    { resetAtUnixSeconds: 1200, requestDeadlineUnixSeconds: 1100 }, { resetAtUnixSeconds: 399 }]) {
    assert.equal(callRetry(undefined, intent(patch)).reason, 'resetOutsideBounds');
  }
  assert.equal(callRetry(undefined, intent({ resetAtUnixSeconds: null })).reason, 'unknownReset');
});
test('at/past reset requires fresh post-reset explicit permission and actual relevant capacity', () => {
  assert.equal(callRetry().decision, 'retry');
  const laterContext = { ...context, nowUnixSeconds: 1001 };
  assert.equal(callRetry(snapshot(20, { observedAtUnixSeconds: 1001, ordinaryUsageAllowed: true }), intent(), laterContext).decision, 'retry');
  assert.equal(callRetry(snapshot(20, { ordinaryUsageAllowed: null })).reason, 'unknownPermission');
  assert.equal(callRetry(snapshot(20, { ordinaryUsageAllowed: false })).reason, 'usageDisallowed');
  for (const used of [100, 121]) {
    assert.equal(callRetry(snapshot(used, { ordinaryUsageAllowed: true })).reason, 'noCapacity');
  }
  const preReset = snapshot(20, { observedAtUnixSeconds: 999, ordinaryUsageAllowed: true });
  assert.equal(callRetry(preReset).reason, 'preResetObservation');
  assert.equal(callRetry(snapshot(20, { ordinaryUsageAllowed: true }), intent({ attempts: 1 })).decision, 'blocked');
});
test('stale/future/mismatched/missing quota observes rather than sending', () => {
  const cases = [[null, 'missing'], [snapshot(20, { observedAtUnixSeconds: 939 }), 'stale'],
    [snapshot(20, { observedAtUnixSeconds: 1001 }), 'future'],
    [snapshot(20, { accountGeneration: 4 }), 'identityMismatch'],
    [snapshot(20, { accountOpaqueId: 'other' }), 'identityMismatch']];
  for (const [value, reason] of cases) {
    const result = callRetry(value);
    assert.equal(result.decision, 'observe');
    assert.equal(result.reason, reason);
  }
  assert.equal(callRetry(snapshot(20, { observedAtUnixSeconds: 940, ordinaryUsageAllowed: true }),
    intent({ resetAtUnixSeconds: 940 })).decision, 'retry');
});
test('relevant capacity is all exact-model/general windows; wrong-model capacity is never a basis', () => {
  const denied = snapshot(0, { ordinaryUsageAllowed: true, buckets: [bucketDTO([windowDTO(10)]),
    bucketDTO([windowDTO(100, { kind: 'secondary' })], { bucketId: 'matching', modelAlias: 'fake-model' })] });
  assert.equal(callRetry(denied).reason, 'noCapacity');
  const unrelated = bucketDTO([windowDTO(121)], { bucketId: 'unrelated', modelAlias: 'other-model' });
  assert.equal(callRetry(snapshot(0, { ordinaryUsageAllowed: true, buckets: [bucketDTO([windowDTO(99.9)]), unrelated] })).decision, 'retry');
  assert.equal(callRetry(snapshot(0, { ordinaryUsageAllowed: true, buckets: [unrelated] })).reason, 'noRelevantWindow');
  assert.equal(callRetry(snapshot(0, { ordinaryUsageAllowed: true, buckets: [] })).decision, 'observe');
});
test('bounded retry horizon covers inclusive past edge and expired reset/deadline never grants permission', () => {
  assert.equal(callRetry(snapshot(20, { ordinaryUsageAllowed: true }), intent({ resetAtUnixSeconds: 400 })).decision, 'retry');
  assert.equal(callRetry(snapshot(20, { ordinaryUsageAllowed: true }), intent({ resetAtUnixSeconds: 399 })).decision, 'blocked');
  assert.equal(callRetry(snapshot(0, { ordinaryUsageAllowed: null }), intent({ resetAtUnixSeconds: 400 })).decision, 'observe');
  assert.equal(callRetry(snapshot(0, { ordinaryUsageAllowed: true }), intent({ requestDeadlineUnixSeconds: 1000 })).reason, 'deadline');
});
test('explicit one-second bounds and caller-selected retry attempt budgets are honored', () => {
  const narrow = { ...spec, maxSnapshotAgeSeconds: 1, minRateIntervalSeconds: 1,
    maxRateIntervalSeconds: 1, maxRetryWaitSeconds: 1, maxRetryAttempts: 2, earlyWarningSeconds: 1 };
  assert.equal(callDisplay(snapshot(80, { observedAtUnixSeconds: 999 }), prefs, context, narrow).status, 'known');
  assert.equal(callDisplay(snapshot(80, { observedAtUnixSeconds: 998 }), prefs, context, narrow).status, 'unknown');
  assert.equal(callDisplay(snapshot(99), runwayPrefs, context, narrow, [prior(98, 999)]).remainingSeconds, 1);
  assert.equal(callRetry(snapshot(20, { ordinaryUsageAllowed: true }), intent({ attempts: 1 }), context, retryPrefs, narrow).decision, 'retry');
  assert.equal(callRetry(snapshot(20, { ordinaryUsageAllowed: true }), intent({ attempts: 2 }), context, retryPrefs, narrow).reason, 'attemptLimit');
  assert.equal(callRetry(null, intent({ resetAtUnixSeconds: 1001 }), context, retryPrefs, narrow).decision, 'wait');
  assert.equal(callRetry(null, intent({ resetAtUnixSeconds: 1002 }), context, retryPrefs, narrow).reason, 'resetOutsideBounds');
});
test('pure functions do not mutate frozen input; retry decisions require caller consumption', () => {
  function freeze(value) {
    if (value && typeof value === 'object') { Object.values(value).forEach(freeze); Object.freeze(value); }
    return value;
  }
  const value = freeze(snapshot(90, { ordinaryUsageAllowed: true }));
  const c = freeze({ ...context }), p = freeze({ ...retryPrefs }), s = freeze({ ...spec }), i = freeze(intent());
  const h = freeze([prior(80)]);
  assert.equal(display(value, c, { ...p, composerDisplay: 'runway' }, s, h).status, 'known');
  const first = alert(value, c, p, s, h, null);
  freeze(first.nextMarker);
  assert.equal(alert(value, c, p, s, h, first.nextMarker).event, null);
  assert.deepEqual(retry(value, c, p, s, i), retry(value, c, p, s, i));
  assert.equal(retry(value, c, p, s, { ...i, attempts: 1 }).reason, 'attemptLimit');
});
