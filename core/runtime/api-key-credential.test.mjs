/**
 * The native API-key save decision. Only an accepted key reaches the credential
 * store, and only through the reference the live `llm-deepseek` settings
 * namespace names. An unusable key or a namespace without that reference must
 * write nothing, so a refused save never overwrites a stored credential.
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import { saveApiKey } from './fork-supervisor.mjs';

const PUBLIC_KEY = 'PUBLIC-synthetic-deepseek-key';
const REF = 'DEEPSEEK_API_KEY';

/** Mirrors the Host's upstream header-value validation: printable ASCII only. */
const normalizeApiKey = (raw) => {
  const value = typeof raw === 'string' ? raw.trim() : '';
  if (value.length === 0) return { ok: false, reason: 'empty' };
  if (![...value].every(character => character >= '\u0021' && character <= '\u007e')) {
    return { ok: false, reason: 'unusable' };
  }
  return { ok: true, value };
};
const credentialRef = ref => `ref:${ref}`;

function context(namespaces = [{ ns: 'llm-deepseek', value: { apiKeyEnv: REF } }]) {
  const writes = [];
  const describes = [];
  const ctx = {
    get(name) {
      if (name === 'settings') {
        return {
          describe(options) {
            describes.push(options);
            return namespaces;
          },
        };
      }
      if (name === 'credentials') {
        return {
          async set(reference, value) {
            writes.push([reference, value]);
          },
        };
      }
      return undefined;
    },
  };
  return { ctx, writes, describes };
}

test('an accepted key is written to the reference the DeepSeek namespace names', async () => {
  const { ctx, writes, describes } = context();
  assert.deepEqual(await saveApiKey(ctx, ` ${PUBLIC_KEY}\n`, { credentialRef, normalizeApiKey }), { ok: true });
  assert.deepEqual(writes, [[`ref:${REF}`, PUBLIC_KEY]]);
  // Secret values are never requested from the settings service.
  assert.deepEqual(describes, [{ redactSecrets: true }]);
});

test('a namespace without an apiKeyEnv reference refuses without writing', async () => {
  for (const namespaces of [
    [],
    [{ ns: 'llm-deepseek', value: {} }],
    [{ ns: 'llm-deepseek', value: { apiKeyEnv: 7 } }],
    [{ ns: 'other-provider', value: { apiKeyEnv: REF } }],
  ]) {
    const { ctx, writes } = context(namespaces);
    assert.deepEqual(await saveApiKey(ctx, PUBLIC_KEY, { credentialRef, normalizeApiKey }), { ok: false });
    assert.deepEqual(writes, []);
  }
});

test('an unusable key refuses without writing or echoing the value', async () => {
  for (const raw of ['', '   ', 'PUBLIC\nsecond-line', 'PUBLIC_世界']) {
    const { ctx, writes } = context();
    const outcome = await saveApiKey(ctx, raw, { credentialRef, normalizeApiKey });
    assert.deepEqual(outcome, { ok: false });
    assert.deepEqual(writes, []);
    assert.equal(JSON.stringify(outcome).includes('PUBLIC'), false);
  }
});

test('a failing credential write surfaces as a rejection rather than a false success', async () => {
  const ctx = {
    get(name) {
      if (name === 'settings') return { describe: () => [{ ns: 'llm-deepseek', value: { apiKeyEnv: REF } }] };
      if (name === 'credentials') return { set: async () => { throw new Error('PUBLIC_CREDENTIAL_STORE_FAILURE'); } };
      return undefined;
    },
  };
  await assert.rejects(saveApiKey(ctx, PUBLIC_KEY, { credentialRef, normalizeApiKey }));
});

test('an unavailable settings or credentials service refuses instead of writing', async () => {
  // No settings service at all is a composition error, not a refusal.
  await assert.rejects(saveApiKey({ get: () => undefined }, PUBLIC_KEY, { credentialRef, normalizeApiKey }));
  // Settings present but no namespace is an ordinary refusal.
  const settingsOnly = { get: name => (name === 'settings' ? { describe: () => [] } : undefined) };
  assert.deepEqual(await saveApiKey(settingsOnly, PUBLIC_KEY, { credentialRef, normalizeApiKey }), { ok: false });
  // A named reference without a credentials service is a composition error too.
  const noCredentials = { get: name => (name === 'settings' ? { describe: () => [{ ns: 'llm-deepseek', value: { apiKeyEnv: REF } }] } : undefined) };
  await assert.rejects(saveApiKey(noCredentials, PUBLIC_KEY, { credentialRef, normalizeApiKey }));
});
