// Framework-neutral preferences only: no filesystem, provider, account or model work.
// The caller owns durable storage and must implement an atomic compare-and-swap.
const preferenceKeys = Object.freeze([
  'composerDisplay', 'usageAlert', 'customRemainingPercent',
  'retryAfterReset', 'experimentalQuota',
]);
const documentKeys = Object.freeze(['version', 'revision', 'preferences']);
const optionKeys = Object.freeze([
  'readDocument', 'commitDocument', 'maxPendingUpdates', 'maxDocumentBytes',
]);
const composerDisplays = new Set(['off', 'percent', 'progressBar', 'runway']);
const usageAlerts = new Set(['off', 'percent20', 'earlyWarning', 'custom']);
const messages = Object.freeze({
  INVALID_PREFERENCES: 'Invalid Codex usage preferences.',
  INVALID_OPTIONS: 'Invalid Codex usage store options.',
  INVALID_REVISION: 'Invalid Codex usage revision.',
  INVALID_DOCUMENT: 'Invalid Codex usage document.',
  UNSUPPORTED_VERSION: 'Unsupported Codex usage document version.',
  DOCUMENT_TOO_LARGE: 'Codex usage document exceeds its byte limit.',
  READ_FAILED: 'Codex usage document could not be read.',
  CONFLICT: 'Codex usage revision conflicts with durable storage.',
  INDETERMINATE: 'Codex usage persistence is indeterminate; load to reconcile.',
  REVISION_OVERFLOW: 'Codex usage revision cannot be incremented.',
  QUEUE_FULL: 'Codex usage operation queue is full.',
  CLOSED: 'Codex usage preferences store is closed.',
  INTERNAL_ERROR: 'Codex usage preferences operation failed.',
});
const inspect = Symbol.for('nodejs.util.inspect.custom');
const safeErrors = new WeakSet();
const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });

/** Safe, fixed diagnostics: never attach input, external text or a cause. */
export class CodexUsagePreferencesError extends Error {
  constructor(code) {
    const safeCode = typeof code === 'string' && Object.hasOwn(messages, code)
      ? code : 'INTERNAL_ERROR';
    super(messages[safeCode]);
    this.name = 'CodexUsagePreferencesError';
    this.code = safeCode;
    safeErrors.add(this);
    Object.freeze(this);
  }

  toJSON() {
    return { name: this.name, code: this.code };
  }

  [inspect]() {
    return `${this.name} [${this.code}]`;
  }
}

function fail(code) {
  throw new CodexUsagePreferencesError(code);
}

// Accept only exact own data properties of ordinary DTOs, never accessors,
// class instances, inherited fields, symbols, non-enumerable fields or extras.
function exactFields(value, keys, code) {
  try {
    if (value === null || typeof value !== 'object') fail(code);
    const prototype = Object.getPrototypeOf(value);
    if (prototype !== Object.prototype && prototype !== null) fail(code);
    const descriptors = Object.getOwnPropertyDescriptors(value);
    const actual = Reflect.ownKeys(descriptors);
    if (actual.length !== keys.length) fail(code);
    const fields = Object.create(null);
    for (const key of keys) {
      const descriptor = descriptors[key];
      if (!descriptor || !Object.hasOwn(descriptor, 'value') || !descriptor.enumerable) {
        fail(code);
      }
      fields[key] = descriptor.value;
    }
    return fields;
  } catch {
    fail(code);
  }
}

export function defaultCodexUsagePreferences() {
  return Object.freeze({
    composerDisplay: 'off',
    usageAlert: 'off',
    customRemainingPercent: 20,
    retryAfterReset: false,
    experimentalQuota: false,
  });
}

/** Return a new immutable five-field DTO; do not coerce or truncate numbers. */
export function validateCodexUsagePreferences(value) {
  const fields = exactFields(value, preferenceKeys, 'INVALID_PREFERENCES');
  if (!composerDisplays.has(fields.composerDisplay)
      || !usageAlerts.has(fields.usageAlert)
      || typeof fields.customRemainingPercent !== 'number'
      || !Number.isFinite(fields.customRemainingPercent)
      || fields.customRemainingPercent < 0 || fields.customRemainingPercent > 100
      || typeof fields.retryAfterReset !== 'boolean'
      || typeof fields.experimentalQuota !== 'boolean') {
    fail('INVALID_PREFERENCES');
  }
  return Object.freeze({
    composerDisplay: fields.composerDisplay,
    usageAlert: fields.usageAlert,
    customRemainingPercent: fields.customRemainingPercent,
    retryAfterReset: fields.retryAfterReset,
    experimentalQuota: fields.experimentalQuota,
  });
}

function isRevision(value) {
  return Number.isSafeInteger(value) && value >= 0;
}

function documentOf(revision, preferences) {
  return Object.freeze({ version: 1, revision, preferences });
}

function snapshotOf(document, persisted) {
  return Object.freeze({ ...document, persisted });
}

function checkBytes(text, maximum) {
  // UTF-8 requires at least as many bytes as UTF-16 code units. Bound obvious
  // oversize inputs before allocating their encoding.
  if (text.length > maximum) fail('DOCUMENT_TOO_LARGE');
  // Reject unpaired UTF-16 surrogates rather than silently replacing them.
  if (/[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/u.test(text)) {
    fail('INVALID_DOCUMENT');
  }
  if (encoder.encode(text).byteLength > maximum) fail('DOCUMENT_TOO_LARGE');
}

function validateDocument(value, maximum) {
  const fields = exactFields(value, documentKeys, 'INVALID_DOCUMENT');
  if (fields.version !== 1) fail('UNSUPPORTED_VERSION');
  if (!isRevision(fields.revision)) fail('INVALID_DOCUMENT');
  let preferences;
  try {
    preferences = validateCodexUsagePreferences(fields.preferences);
  } catch {
    fail('INVALID_DOCUMENT');
  }
  const document = documentOf(fields.revision, preferences);
  checkBytes(JSON.stringify(document), maximum);
  return document;
}

function decodeDocument(value, maximum) {
  if (value === null) {
    // Explicit unwritten state, not a successful persistence observation.
    return snapshotOf(documentOf(0, defaultCodexUsagePreferences()), false);
  }
  if (value instanceof Uint8Array) {
    if (value.byteLength > maximum) fail('DOCUMENT_TOO_LARGE');
    try {
      value = decoder.decode(value);
    } catch {
      fail('INVALID_DOCUMENT');
    }
  }
  if (typeof value === 'string') {
    checkBytes(value, maximum);
    try {
      value = JSON.parse(value);
    } catch {
      fail('INVALID_DOCUMENT');
    }
    // A JSON null is corruption, not the port's explicit missing-file null.
  }
  return snapshotOf(validateDocument(value, maximum), true);
}

/**
 * Inert factory, with required positive safe-integer limits (no defaults).
 * Security ceilings: queued + in-flight operations <= 128,
 * maxPendingUpdates <= 128; maxDocumentBytes <= 65,536.
 * Callers select their smaller update/document budgets explicitly.
 * readDocument(): null | exact document DTO | complete JSON string | UTF-8 bytes.
 * null alone means no durable document; JSON null is not an unwritten document.
 * commitDocument({ expectedRevision, document }): exactly one of
 *   { status: 'committed' } | { status: 'conflict' } | { status: 'indeterminate' }.
 * The port must CAS the authoritative revision atomically (unwritten is zero),
 * durably commit the whole document before acknowledging committed, and never
 * infer committed from an unknown write result. This owner cannot prove those
 * external guarantees. It never retries a commit, including an ambiguous one.
 * load()/update() resolve immutable { version, revision, preferences, persisted }.
 * update() rejects CONFLICT or INDETERMINATE instead of publishing false success.
 * All operations serialize; queued + in-flight updates reserve capacity at call
 * time. Inputs are copied at admission. load() always rereads, never caches.
 * dispose() immediately closes admission and rejects queued work with CLOSED;
 * it waits for active work, preventing a not-yet-invoked commit after closing.
 * A commit already invoked is never canceled or detached: its actual terminal
 * receipt still settles its update before disposal resolves. A hung port keeps
 * disposal pending; this owner does not invent a timeout or abort guarantee.
 */
export function createCodexUsagePreferencesStore(options) {
  const fields = exactFields(options, optionKeys, 'INVALID_OPTIONS');
  const { readDocument, commitDocument, maxPendingUpdates, maxDocumentBytes } = fields;
  if (typeof readDocument !== 'function' || typeof commitDocument !== 'function'
      || !Number.isSafeInteger(maxPendingUpdates) || maxPendingUpdates <= 0
      || maxPendingUpdates > 128
      || !Number.isSafeInteger(maxDocumentBytes) || maxDocumentBytes <= 0
      || maxDocumentBytes > 65_536) {
    fail('INVALID_OPTIONS');
  }
  const queue = [];
  let active = null;
  let scheduled = false;
  let closed = false;
  let pendingUpdates = 0;
  let disposal;
  let resolveDisposal;

  function settle(item, value, rejected) {
    if (item.isUpdate) pendingUpdates -= 1;
    if (rejected) item.reject(value);
    else item.resolve(value);
  }

  function schedule() {
    if (scheduled || active || closed || queue.length === 0) return;
    scheduled = true;
    queueMicrotask(() => {
      scheduled = false;
      if (closed || active || queue.length === 0) return;
      const item = queue.shift();
      active = item;
      function finish(value, rejected) {
        active = null;
        settle(item, value, rejected);
        if (closed) resolveDisposal();
        else schedule();
      }
      try {
        Promise.resolve(item.operation()).then(
          (value) => finish(value, false),
          (error) => finish(error, true),
        );
      } catch (error) {
        finish(error, true);
      }
    });
  }

  function serialize(operation, isUpdate = false) {
    if (closed) return Promise.reject(new CodexUsagePreferencesError('CLOSED'));
    if (queue.length + Number(active !== null) >= 128
        || (isUpdate && pendingUpdates >= maxPendingUpdates)) {
      return Promise.reject(new CodexUsagePreferencesError('QUEUE_FULL'));
    }
    if (isUpdate) pendingUpdates += 1;
    const result = new Promise((resolve, reject) => {
      queue.push({ operation, isUpdate, resolve, reject });
    });
    schedule();
    return result;
  }

  async function readAuthoritative() {
    let raw;
    try {
      raw = await readDocument();
    } catch {
      fail('READ_FAILED');
    }
    try {
      return decodeDocument(raw, maxDocumentBytes);
    } catch (error) {
      // WeakSet membership cannot invoke an untrusted thrown value's traps.
      if (safeErrors.has(error)) throw error;
      fail('INVALID_DOCUMENT');
    }
  }

  const owner = {
    load() {
      return serialize(readAuthoritative);
    },
    update(expectedRevision, nextPrefs) {
      if (closed) return Promise.reject(new CodexUsagePreferencesError('CLOSED'));
      let preferences;
      let document;
      try {
        if (!isRevision(expectedRevision)) fail('INVALID_REVISION');
        preferences = validateCodexUsagePreferences(nextPrefs);
        if (expectedRevision === Number.MAX_SAFE_INTEGER) fail('REVISION_OVERFLOW');
        document = documentOf(expectedRevision + 1, preferences);
        checkBytes(JSON.stringify(document), maxDocumentBytes);
      } catch (error) {
        return Promise.reject(error);
      }
      return serialize(async () => {
        const current = await readAuthoritative();
        if (closed) fail('CLOSED');
        if (current.revision !== expectedRevision) fail('CONFLICT');
        // An external throw, unknown receipt or indeterminate status cannot
        // establish whether storage changed. Never turn it into a read default.
        let outcome;
        try {
          outcome = await commitDocument(Object.freeze({ expectedRevision, document }));
        } catch {
          fail('INDETERMINATE');
        }
        const receipt = exactFields(outcome, ['status'], 'INDETERMINATE');
        if (receipt.status === 'conflict') fail('CONFLICT');
        if (receipt.status !== 'committed') fail('INDETERMINATE');
        return snapshotOf(document, true);
      }, true);
    },
    dispose() {
      if (closed) return disposal;
      closed = true;
      disposal = new Promise((resolve) => { resolveDisposal = resolve; });
      for (const item of queue.splice(0)) {
        settle(item, new CodexUsagePreferencesError('CLOSED'), true);
      }
      if (!active) resolveDisposal();
      return disposal;
    },
    toJSON() {
      return '[CodexUsagePreferencesStore]';
    },
    [inspect]() {
      return '[CodexUsagePreferencesStore]';
    },
  };
  return Object.freeze(owner);
}
