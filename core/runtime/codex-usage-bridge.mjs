/** Private preference commands only; never account, quota, notification or model I/O. */
import { validateCodexUsagePreferences } from './codex-usage-preferences.mjs';

const inspect = Symbol.for('nodejs.util.inspect.custom');
const messages = Object.freeze({
  CODEX_USAGE_CONFLICT: 'Codex usage preferences changed. Refresh before saving.',
  CODEX_USAGE_INDETERMINATE: 'Codex usage preference save is uncertain. Refresh without replaying it.',
  CODEX_USAGE_UNAVAILABLE: 'Codex usage preferences are unavailable.',
});
const maximumOperations = 128;
const fail = code => { throw new CodexUsageBridgeError(code); };

/** Fixed public diagnostics; external exceptions, paths and credential data are not retained. */
export class CodexUsageBridgeError extends Error {
  /** @param {string} code Fixed preference result code. */
  constructor(code) {
    const safe = typeof code === 'string' && Object.hasOwn(messages, code) ? code : 'CODEX_USAGE_UNAVAILABLE';
    super(messages[safe]);
    this.name = 'CodexUsageBridgeError';
    this.code = safe;
    Object.freeze(this);
  }
  toJSON() { return { name: this.name, code: this.code }; }
  [inspect]() { return `${this.name} [${this.code}]`; }
}

function safeCode(error) {
  try { return Object.getOwnPropertyDescriptor(error, 'code')?.value; }
  catch { return undefined; }
}

/**
 * Lazy storage owner for already parsed private commands. Opening is shared and
 * never retried implicitly. Stop closes admission immediately, prevents a waiting
 * initialization from starting a write, and awaits actual initialization, storage
 * disposal and admitted calls. Late results are suppressed by the supervisor.
 * @param {{openStorage:()=>Promise<{load:Function,update:Function,dispose:Function}>}} options Native-owned private filesystem factory.
 * @returns {{handle:Function,stop:()=>Promise<void>}} Closed preference command owner.
 */
export function createCodexUsageBridge({ openStorage }) {
  if (typeof openStorage !== 'function') fail('CODEX_USAGE_UNAVAILABLE');
  let closed = false;
  let opening;
  let storage;
  let stopping;
  const pending = new Set();
  const getStorage = () => {
    if (!opening) {
      opening = Promise.resolve().then(() => {
        if (closed) fail('CODEX_USAGE_UNAVAILABLE');
        return openStorage();
      }).then(owner => { storage = owner; return owner; });
    }
    return opening;
  };
  return Object.freeze({
    handle(command) {
      if (closed || pending.size >= maximumOperations) return Promise.reject(new CodexUsageBridgeError('CODEX_USAGE_UNAVAILABLE'));
      // Detached preference inputs remain stable while initialization is pending.
      let preferences;
      try {
        if (command.type === 'codex-usage-save') {
          if (!Number.isSafeInteger(command.expectedRevision) || command.expectedRevision < 0) fail('CODEX_USAGE_UNAVAILABLE');
          preferences = validateCodexUsagePreferences(command.preferences);
        } else if (command.type !== 'codex-usage-load') fail('CODEX_USAGE_UNAVAILABLE');
      } catch { return Promise.reject(new CodexUsageBridgeError('CODEX_USAGE_UNAVAILABLE')); }
      const type = command.type;
      const revision = command.expectedRevision;
      const task = (async () => {
        let invokedWrite = false;
        try {
          const owner = await getStorage();
          if (closed) fail('CODEX_USAGE_UNAVAILABLE');
          if (type === 'codex-usage-load') return await owner.load();
          invokedWrite = true;
          return await owner.update(revision, preferences);
        } catch (error) {
          const code = safeCode(error);
          if (code === 'CONFLICT') fail('CODEX_USAGE_CONFLICT');
          if (code === 'INDETERMINATE' || invokedWrite && !['READ_FAILED', 'CLOSED', 'QUEUE_FULL', 'INVALID_REVISION', 'INVALID_PREFERENCES', 'INVALID_OPTIONS',
            'INVALID_DOCUMENT', 'UNSUPPORTED_VERSION', 'DOCUMENT_TOO_LARGE', 'REVISION_OVERFLOW'].includes(code)) {
            fail('CODEX_USAGE_INDETERMINATE');
          }
          fail('CODEX_USAGE_UNAVAILABLE');
        }
      })();
      pending.add(task);
      void task.then(() => pending.delete(task), () => pending.delete(task));
      return task;
    },
    stop() {
      closed = true;
      if (!stopping) {
        // Dispose admission synchronously when the owner already exists.
        const started = storage?.dispose();
        stopping = (async () => {
          if (opening) await opening.catch(() => {}); // Failed opening acquired no usable owner.
          const disposal = started ?? storage?.dispose();
          await Promise.allSettled([...pending]);
          await disposal;
        })();
      }
      return stopping;
    },
    toJSON: () => '[CodexUsageBridge]',
    [inspect]: () => '[CodexUsageBridge]',
  });
}
