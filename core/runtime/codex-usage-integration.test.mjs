/** PUBLIC in-memory integration; no runtime, file storage, account or provider is started. */
import test from 'node:test';
import assert from 'node:assert/strict';
import { projectCodexQuota } from './codex-quota.mjs';
import { createCodexUsagePreferencesStore } from './codex-usage-preferences.mjs';
import { projectCodexUsageDisplay as display, decideCodexUsageAlert as alert,
  decideCodexResetRetry as retry } from './codex-usage-policy.mjs';

const context = { accountOpaqueId: 'PUBLIC-account', accountGeneration: 4,
  modelAlias: 'PUBLIC-model', nowUnixSeconds: 1000 };
const projectionSpec = { maxInputBytes: 8192, maxOutputBytes: 4096, maxBuckets: 8,
  maxWindows: 16, maxEmailBytes: 320, maxStringBytes: 256 };
const policySpec = { maxSnapshotAgeSeconds: 60, maxHistorySamples: 8, minRateIntervalSeconds: 10,
  maxRateIntervalSeconds: 300, maxRetryWaitSeconds: 600, maxRetryAttempts: 1, earlyWarningSeconds: 30 };
const enabled = { composerDisplay: 'progressBar', usageAlert: 'percent20', customRemainingPercent: 20,
  retryAfterReset: true, experimentalQuota: true };
function quota(usedPercent, permission = null, observedAtUnixSeconds = 1000) {
  return projectCodexQuota({ accountId: 'PUBLIC-private-issuer', ordinaryUsageAllowed: permission,
    rateLimits: { limitId: 'PUBLIC-limit', normalModelSlug: 'PUBLIC-model',
      primary: { usedPercent, windowDurationMins: 30, resetsAt: 1000 } } },
  { accountOpaqueId: context.accountOpaqueId, accountGeneration: context.accountGeneration,
    expectedIssuerAccountId: 'PUBLIC-private-issuer', observedAtUnixSeconds }, projectionSpec);
}
function fixture() {
  let durable = null;
  let receipt = 'committed';
  let calls = 0;
  const store = createCodexUsagePreferencesStore({ maxPendingUpdates: 4, maxDocumentBytes: 4096,
    readDocument: () => structuredClone(durable),
    commitDocument: ({ expectedRevision, document }) => {
      calls++;
      if (receipt !== 'committed') return { status: receipt };
      if ((durable?.revision ?? 0) !== expectedRevision) return { status: 'conflict' };
      durable = structuredClone(document);
      return { status: 'committed' };
    } });
  return { store, calls: () => calls, setReceipt: value => { receipt = value; } };
}
const intent = { requestId: 'PUBLIC-request', currentRequestId: 'PUBLIC-request',
  accountOpaqueId: context.accountOpaqueId, accountGeneration: context.accountGeneration,
  modelAlias: context.modelAlias, failureKind: 'quotaDenied', failedBeforeOutput: true, cancelled: false, attempts: 0,
  resetAtUnixSeconds: 1000, requestDeadlineUnixSeconds: 1600 };

test('unwritten preferences disable projected quota, notifications and retries', async () => {
  const { store } = fixture();
  try {
    const state = await store.load();
    assert.equal(state.persisted, false);
    assert.equal(state.revision, 0);
    assert.equal(display(quota(90), context, state.preferences, policySpec, []), null);
    assert.deepEqual(alert(quota(90), context, state.preferences, policySpec, [], null),
      { event: null, nextMarker: null });
    assert.equal(retry(quota(20, true), context, state.preferences, policySpec, intent).decision, 'off');
  } finally { await store.dispose(); }
});

test('committed preferences consume genuine sanitized fields without clamping raw usage', async () => {
  const { store } = fixture();
  try {
    const state = await store.update(0, enabled);
    assert.equal(state.persisted, true);
    assert.equal(state.revision, 1);
    const result = display(quota(121), context, state.preferences, policySpec, []);
    assert.equal(result.window.usedPercent, 121);
    assert.equal(result.remainingPercent, 0);
    assert.equal(result.window.windowDurationMins, 30);
    assert.equal(JSON.stringify(result).includes('PUBLIC-private-issuer'), false);
    const first = alert(quota(90), context, state.preferences, policySpec, [], null);
    assert.ok(first.event);
    assert.equal(alert(quota(91), context, state.preferences, policySpec, [], first.nextMarker).event, null);
  } finally { await store.dispose(); }
});

test('conflict preserves the last committed document without a false save acknowledgement', async () => {
  const f = fixture();
  try {
    await f.store.update(0, enabled);
    f.setReceipt('conflict');
    await assert.rejects(f.store.update(1, { ...enabled, experimentalQuota: false }),
      error => error.code === 'CONFLICT');
    const state = await f.store.load();
    assert.equal(state.revision, 1);
    assert.equal(state.preferences.experimentalQuota, true);
    assert.equal(display(quota(20), context, state.preferences, policySpec, []).status, 'known');
  } finally { await f.store.dispose(); }
});

// A synthetic consumer pattern, not a claim that native controls are wired.
test('local opt-out overrides stale saved consent when saving conflicts or is indeterminate', async () => {
  for (const [receipt, errorCode] of [['conflict', 'CONFLICT'], ['indeterminate', 'INDETERMINATE']]) {
    const f = fixture();
    try {
      const on = await f.store.update(0, enabled);
      const marker = alert(quota(90), context, on.preferences, policySpec, [], null).nextMarker;
      // Fence local permission before awaiting the save; failed persistence cannot restore it.
      const effectiveConsent = false;
      f.setReceipt(receipt);
      await assert.rejects(f.store.update(1, { ...enabled, experimentalQuota: false }),
        error => error.code === errorCode);
      const saved = await f.store.load();
      assert.equal(saved.preferences.experimentalQuota, true);
      const effectivePreferences = { ...saved.preferences,
        experimentalQuota: effectiveConsent && saved.preferences.experimentalQuota };
      assert.equal(display(quota(90), context, effectivePreferences, policySpec, []).reason, 'disabled');
      assert.deepEqual(alert(quota(90), context, effectivePreferences, policySpec, [], marker),
        { event: null, nextMarker: null });
      assert.equal(retry(quota(20, true), context, effectivePreferences, policySpec, intent).decision, 'off');
    } finally { await f.store.dispose(); }
  }
});

test('committed consent revocation hides cached observations and clears alert/retry state', async () => {
  const { store } = fixture();
  try {
    const on = await store.update(0, enabled);
    const marker = alert(quota(90), context, on.preferences, policySpec, [], null).nextMarker;
    const off = await store.update(1, { ...enabled, experimentalQuota: false });
    const result = display(quota(90), context, off.preferences, policySpec, []);
    assert.equal(result.status, 'unknown');
    assert.equal(result.reason, 'disabled');
    assert.equal(result.window, null);
    assert.deepEqual(alert(quota(90), context, off.preferences, policySpec, [], marker),
      { event: null, nextMarker: null });
    assert.equal(retry(quota(20, true), context, off.preferences, policySpec, intent).decision, 'off');
  } finally { await store.dispose(); }
});

test('persisted retry opt-in still requires fresh permission and immutable request/model binding', async () => {
  const { store } = fixture();
  try {
    const { preferences } = await store.update(0, enabled);
    assert.equal(retry(quota(20, null), context, preferences, policySpec, intent).decision, 'observe');
    assert.equal(retry(quota(20, true, 999), context, preferences, policySpec, intent).decision, 'observe');
    assert.equal(retry(quota(20, true), context, preferences, policySpec, intent).decision, 'retry');
    assert.equal(retry(quota(20, true), context, preferences, policySpec,
      { ...intent, modelAlias: 'PUBLIC-other-model' }).reason, 'modelMismatch');
    assert.equal(retry(quota(20, true), context, preferences, policySpec,
      { ...intent, failedBeforeOutput: false }).reason, 'partialOutput');
  } finally { await store.dispose(); }
});
