/**
 * Private Host-owned Codex 0.160.1 account client. Construction is inert; only
 * start({ explicitUserAction: true }) spawns. No ambient credentials, browser,
 * logging, token persistence, generic RPC entry point, or user DTO projection.
 * Protocol: the pinned CLI's generate-json-schema output (v1 initialize, v2
 * account methods, ChatgptAuthTokensRefreshParams/Response). Official lifecycle:
 * https://github.com/openai/codex/blob/rust-v0.160.1/codex-rs/app-server/src/request_processors/initialize_processor.rs
 * https://github.com/openai/codex/blob/rust-v0.160.1/codex-rs/login/src/auth/default_client.rs
 * Experimental external-auth documentation: https://learn.chatgpt.com/docs/app-server.md
 */
import { spawn } from 'node:child_process';
import { access, lstat, realpath } from 'node:fs/promises';
import { constants } from 'node:fs';
import { isAbsolute, normalize } from 'node:path';

const PINNED_VERSION = '0.160.1';
const NOTIFICATIONS = new Set(['account/updated', 'account/login/completed', 'account/rateLimits/updated']);
const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const keys = (value, allowed) => object(value) && Object.keys(value).every(key => allowed.includes(key));
const text = value => typeof value === 'string' && value.length > 0 && !/[\x00-\x1f\x7f]/.test(value);
const own = (value, key) => Object.hasOwn(value, key);
const validPath = value => text(value) && Buffer.byteLength(value) <= 4096 && isAbsolute(value)
  && normalize(value) === value && !value.split('/').includes('..');

/** Sanitized failure: mutation abandonment is indeterminate, never an undo. */
export class CodexAppServerError extends Error {
  /** @param {string} code Fixed local error code. @param {boolean} indeterminate Mutation may have occurred. */
  constructor(code, indeterminate = false) {
    super(`Codex app-server: ${code}`);
    this.name = 'CodexAppServerError';
    this.code = code;
    this.indeterminate = indeterminate;
  }
}
const error = (code, mutation = false) => new CodexAppServerError(code, mutation);

function validateOptions(options) {
  const allowed = ['absoluteCliPath', 'codexHome', 'nativeCwd', 'expectedCliVersion', 'credentialStorage',
    'enableExperimentalChatgptTokens', 'clientInfo',
    'maxFrameBytes', 'maxPendingRequests', 'maxPendingBytes', 'maxServerRequests', 'maxServerRequestIds',
    'maxRequestIdBytes', 'startupTimeoutMs', 'requestTimeoutMs', 'writeTimeoutMs', 'serverRequestTimeoutMs',
    'stopGraceMs', 'stopTermMs', 'stopKillMs', 'onNotification', 'onAuthTokensRefresh'];
  if (!keys(options, allowed) || !['absoluteCliPath', 'codexHome', 'nativeCwd'].every(key => validPath(options[key]))
    || options.expectedCliVersion !== PINNED_VERSION || !['ephemeral', 'private-file'].includes(options.credentialStorage)
    || typeof options.enableExperimentalChatgptTokens !== 'boolean'
    || typeof options.onNotification !== 'function'
    || (options.onAuthTokensRefresh !== undefined && typeof options.onAuthTokensRefresh !== 'function')
    || !keys(options.clientInfo, ['name', 'title', 'version'])
    || !/^[A-Za-z0-9_-]{1,128}$/.test(options.clientInfo.name)
    || !text(options.clientInfo.version) || options.clientInfo.version.length > 128
    || (options.clientInfo.title != null && (!text(options.clientInfo.title) || options.clientInfo.title.length > 128))) {
    throw error('INVALID_OPTIONS');
  }
  const numbers = allowed.filter(key => key.startsWith('max') || key.endsWith('Ms'));
  if (numbers.some(key => !Number.isSafeInteger(options[key]) || options[key] < 1)
    || options.maxFrameBytes > 16 * 1024 * 1024 || options.maxFrameBytes < 512
    || options.maxPendingBytes > 256 * 1024 * 1024 || options.maxPendingBytes < 2 * (options.maxFrameBytes + 1)
    || options.maxPendingRequests > 4096 || options.maxServerRequests > 4096
    || options.maxServerRequestIds > 65536 || options.maxRequestIdBytes > 4096
    || options.maxServerRequests > options.maxServerRequestIds
    || (options.enableExperimentalChatgptTokens && options.serverRequestTimeoutMs >= 10000)
    || numbers.filter(key => key.endsWith('Ms')).some(key => options[key] > 2147483647)) throw error('INVALID_OPTIONS');
  return Object.freeze({ ...options, clientInfo: Object.freeze({ ...options.clientInfo }) });
}

async function validateLaunch(options) {
  try {
    const executable = await realpath(options.absoluteCliPath);
    if (!(await lstat(executable)).isFile()) throw error('INVALID_EXECUTABLE');
    await access(executable, constants.X_OK);
    for (const path of [options.codexHome, options.nativeCwd]) {
      const stat = await lstat(path);
      if (!stat.isDirectory() || stat.isSymbolicLink() || await realpath(path) !== path
        || (stat.mode & 0o077) !== 0 || (typeof process.getuid === 'function' && stat.uid !== process.getuid())) {
        throw error('INVALID_PRIVATE_DIRECTORY');
      }
    }
    return executable;
  } catch (cause) {
    if (cause instanceof CodexAppServerError) throw cause;
    throw error('INVALID_LAUNCH_PATH');
  }
}

function loginParams(value, enableExperimentalChatgptTokens) {
  if (!object(value)) throw error('INVALID_PARAMS');
  switch (value.type) {
    case 'apiKey':
      if (!keys(value, ['type', 'apiKey']) || !text(value.apiKey)) throw error('INVALID_PARAMS');
      return { type: 'apiKey', apiKey: value.apiKey };
    case 'chatgpt': {
      if (!keys(value, ['type', 'appBrand', 'codexStreamlinedLogin', 'useHostedLoginSuccessPage'])
        || (value.appBrand != null && !['codex', 'chatgpt'].includes(value.appBrand))
        || ['codexStreamlinedLogin', 'useHostedLoginSuccessPage'].some(key => own(value, key) && typeof value[key] !== 'boolean')) throw error('INVALID_PARAMS');
      return { ...value };
    }
    case 'chatgptDeviceCode':
      if (!keys(value, ['type'])) throw error('INVALID_PARAMS');
      return { type: value.type };
    case 'chatgptAuthTokens':
      if (!enableExperimentalChatgptTokens) throw error('EXPERIMENTAL_AUTH_DISABLED');
      if (!keys(value, ['type', 'accessToken', 'chatgptAccountId', 'chatgptPlanType'])
        || !text(value.accessToken) || !text(value.chatgptAccountId)
        || (own(value, 'chatgptPlanType') && value.chatgptPlanType !== null
          && (typeof value.chatgptPlanType !== 'string' || /[\x00-\x1f\x7f]/.test(value.chatgptPlanType)))) throw error('INVALID_PARAMS');
      return { type: value.type, accessToken: value.accessToken, chatgptAccountId: value.chatgptAccountId,
        ...(own(value, 'chatgptPlanType') ? { chatgptPlanType: value.chatgptPlanType } : {}) };
    default: throw error('UNSUPPORTED_LOGIN_TYPE');
  }
}

/**
 * Create an inert, single-use private process owner. All deployment bounds are
 * required positive integers, with hard security ceilings validated up front.
 * Directories must already exist, canonical, current-user-owned and mode 0700
 * (or stricter). HOME/XDG paths are confined to codexHome. Required
 * credentialStorage is 'ephemeral' (CLI memory only) or 'private-file' (caller
 * explicitly permits CLI-managed CODEX_HOME/auth.json persistence). Fixed CLI
 * overrides disallow OS-global primary-account keyring/auto storage. This
 * module never reads, writes or exports that file; caller owns provisioning.
 * Startup deadline includes validation, handshake and initialized write.
 * Required enableExperimentalChatgptTokens:boolean is the private caller's
 * explicit user-Beta-consent decision, not an inferred account preference.
 * False negotiates experimentalApi:false and denies token login/refresh. True
 * negotiates experimentalApi:true but admits only the exact external-token
 * login and refresh fields; all other experimental business methods stay absent.
 * This is documented Beta, pinned to 0.160.1, not a stable public guarantee:
 * the generated schema still labels this variant INTERNAL/UNSTABLE while current
 * official docs intend it for host-owned auth. Callback timeout must be <10s,
 * before the server's external-refresh deadline; owner must discard late work.
 *
 * onNotification({method, params}) is synchronous and PRIVATE: it receives only
 * known account notifications, unprojected. Its throws/Promise rejections are
 * contained. onAuthTokensRefresh({requestId, params, signal}) may be asynchronous;
 * it must honor abort to release its own work. Only that exact server method is
 * dispatched. Callback completion after timeout/stop is discarded, not replayed.
 *
 * Account methods require readiness; they never start implicitly. Request IDs
 * are monotonic, connection-local and never reused; duplicate/abandoned replies
 * cannot settle another request. No retry occurs, including mutations. Owner
 * must reconcile uncertain login/logout/refresh outcomes before retrying.
 * @param {object} options Explicit paths, pinned version, clientInfo, bounds and private callbacks.
 * @returns {object} Fixed account methods, explicit start/stop and sanitized lifecycle counters.
 */
export function createCodexAppServer(options) {
  const opts = validateOptions(options);
  const pending = new Map();
  const serverPending = new Map();
  const serverSeen = new Set();
  const writes = [];
  const decoder = new TextDecoder('utf-8', { fatal: true });
  let state = 'idle';
  let faultCode = null;
  let child = null;
  let startTask = null;
  let stopTask = null;
  let serial = 0;
  let reservedBytes = 0;
  let writeBytes = 0;
  let writing = null;
  let input = Buffer.alloc(0);
  let exitObserved = false;
  let closeObserved = false;
  let exitCode = null;
  let exitSignal = null;
  let resolveClose;
  const closed = new Promise(resolve => { resolveClose = resolve; });
  const active = () => state === 'starting' || state === 'ready';
  const byteRoom = amount => reservedBytes + writeBytes + amount <= opts.maxPendingBytes;

  function settle(id, failure, result) {
    const entry = pending.get(id);
    if (!entry) return;
    pending.delete(id);
    reservedBytes -= entry.bytes;
    clearTimeout(entry.timer);
    entry.signal?.removeEventListener('abort', entry.abort);
    if (failure) entry.reject(error(failure, entry.mutation));
    else entry.resolve(result);
  }

  function dispose(code) {
    for (const id of pending.keys()) settle(id, code);
    for (const entry of serverPending.values()) {
      clearTimeout(entry.timer);
      entry.controller.abort();
    }
    serverPending.clear();
    serverSeen.clear();
    // Client reservations were removed by settle; remaining ones belong to server callbacks.
    reservedBytes = 0;
    for (const entry of writes.splice(0)) {
      writeBytes -= entry.frame.length;
      entry.frame.fill(0);
      entry.reject(error(code));
    }
    if (writing) writing.finish(code);
    input.fill(0);
    input = Buffer.alloc(0);
  }

  function poison(code) {
    if (!active()) return;
    faultCode = code;
    state = 'poisoned';
    dispose(code);
    void stop();
  }

  function encode(value) {
    // Only owned flat protocol records reach this encoder. Preflight each leaf
    // before serializing the complete record, so huge owner inputs cannot create
    // an unbounded JSON string or UTF-8 buffer. A string's escaped temporary is
    // bounded by six times maxFrameBytes; the full frame remains maxFrameBytes.
    let budget = opts.maxFrameBytes;
    function measure(item, depth = 0) {
      if (depth > 4) throw error('INVALID_PARAMS');
      if (typeof item === 'string') {
        if (item.length > opts.maxFrameBytes || Buffer.byteLength(item, 'utf8') > opts.maxFrameBytes) throw error('FRAME_LIMIT');
        budget -= Buffer.byteLength(JSON.stringify(item), 'utf8');
      } else if (item === null || typeof item === 'boolean' || (typeof item === 'number' && Number.isSafeInteger(item))) {
        budget -= String(item).length;
      } else if (object(item)) {
        budget -= 2;
        let count = 0;
        for (const [key, child] of Object.entries(item)) {
          if (child === undefined) continue;
          if (count++) budget--;
          measure(key, depth + 1);
          budget--;
          measure(child, depth + 1);
        }
      } else throw error('INVALID_PARAMS');
      if (budget < 0) throw error('FRAME_LIMIT');
    }
    measure(value);
    let json;
    try { json = JSON.stringify(value); } catch (_cause) { throw error('INVALID_PARAMS'); }
    if (typeof json !== 'string' || Buffer.byteLength(json, 'utf8') > opts.maxFrameBytes) throw error('FRAME_LIMIT');
    return Buffer.from(`${json}\n`, 'utf8');
  }

  function pump() {
    if (writing || !writes.length || !active()) return;
    const entry = writes.shift();
    writing = entry;
    let finished = false;
    const timer = setTimeout(() => { entry.finish('WRITE_TIMEOUT'); poison('WRITE_TIMEOUT'); }, opts.writeTimeoutMs);
    entry.finish = code => {
      if (finished) return;
      finished = true;
      clearTimeout(timer);
      writeBytes -= entry.frame.length;
      // A write may still own the buffer until its callback: do not wipe in-flight data.
      if (!code) entry.frame.fill(0);
      writing = null;
      if (code) entry.reject(error(code)); else entry.resolve();
      if (!code) pump();
    };
    try {
      child.stdin.write(entry.frame, cause => {
        if (finished) { entry.frame.fill(0); return; }
        entry.finish(cause ? 'WRITE_FAULT' : null);
        if (cause) poison('WRITE_FAULT');
      });
    } catch (_cause) { entry.finish('WRITE_FAULT'); poison('WRITE_FAULT'); }
  }

  function writeFrame(frame) {
    if (!active()) { frame.fill(0); return Promise.reject(error('UNAVAILABLE')); }
    if (!byteRoom(frame.length) || writes.length + (writing ? 1 : 0) >= opts.maxPendingRequests + opts.maxServerRequests + 1) {
      frame.fill(0);
      return Promise.reject(error('PENDING_LIMIT'));
    }
    writeBytes += frame.length;
    return new Promise((resolve, reject) => { writes.push({ frame, resolve, reject }); pump(); });
  }

  function rpc(method, params, mutation, requestOptions = {}, startup = false) {
    if (!keys(requestOptions, ['signal']) || (requestOptions.signal !== undefined && !(requestOptions.signal instanceof AbortSignal))) return Promise.reject(error('INVALID_PARAMS'));
    const signal = requestOptions.signal;
    if (signal?.aborted) return Promise.reject(error('ABORTED'));
    if (!active() || (!startup && state !== 'ready')) return Promise.reject(error('NOT_READY'));
    if (pending.size >= opts.maxPendingRequests || serial >= Number.MAX_SAFE_INTEGER) return Promise.reject(error('PENDING_LIMIT'));
    const id = `native-${++serial}`;
    let frame;
    try { frame = encode({ id, method, params }); } catch (cause) { return Promise.reject(cause); }
    if (!byteRoom(2 * frame.length)) { frame.fill(0); return Promise.reject(error('PENDING_LIMIT')); }
    reservedBytes += frame.length;
    return new Promise((resolve, reject) => {
      const abort = () => settle(id, 'ABORTED');
      const timer = setTimeout(() => settle(id, 'REQUEST_TIMEOUT'), startup ? opts.startupTimeoutMs : opts.requestTimeoutMs);
      pending.set(id, { resolve, reject, timer, abort, signal, mutation, bytes: frame.length });
      signal?.addEventListener('abort', abort, { once: true });
      // An abandoned request may already have been sent. It is never retransmitted
      // and abort does not send account/login/cancel or claim to reverse a mutation.
      void writeFrame(frame).catch(cause => settle(id, cause.code));
    });
  }

  function serverReply(id, result, failure) {
    let frame;
    try {
      frame = encode(failure ? { id, error: { code: failure, message: 'Request unavailable' } } : { id, result });
    } catch (_cause) { poison('FRAME_LIMIT'); return; }
    void writeFrame(frame).catch(cause => poison(cause.code));
  }

  function handleServerRequest(packet, bytes) {
    const id = packet.id;
    const tag = `${typeof id}:${id}`;
    if (serverSeen.has(tag)) { poison('SERVER_REQUEST_REPLAY'); return; }
    if (serverSeen.size >= opts.maxServerRequestIds) { poison('SERVER_REQUEST_LIMIT'); return; }
    serverSeen.add(tag);
    if (packet.method !== 'account/chatgptAuthTokens/refresh') { serverReply(id, null, -32601); return; }
    // Never release owner credentials before identity, home and version have
    // passed initialize and initialized has been written.
    if (state !== 'ready') { serverReply(id, null, -32000); return; }
    if (!opts.enableExperimentalChatgptTokens) { serverReply(id, null, -32601); return; }
    if (!keys(packet.params, ['reason', 'previousAccountId']) || packet.params.reason !== 'unauthorized'
      || (packet.params.previousAccountId != null && !text(packet.params.previousAccountId))) { poison('INVALID_SERVER_REQUEST'); return; }
    if (!opts.onAuthTokensRefresh) { serverReply(id, null, -32601); return; }
    if (serverPending.size >= opts.maxServerRequests || !byteRoom(bytes)) { poison('SERVER_REQUEST_LIMIT'); return; }
    const controller = new AbortController();
    const finish = (result, failure) => {
      const entry = serverPending.get(tag);
      if (!entry) return;
      serverPending.delete(tag);
      reservedBytes -= bytes;
      clearTimeout(entry.timer);
      controller.abort();
      if (active() && !entry.abandoned) serverReply(id, result, failure);
    };
    const timer = setTimeout(() => {
      const entry = serverPending.get(tag);
      if (!entry) return;
      entry.abandoned = true;
      controller.abort();
      if (active()) serverReply(id, null, -32000);
      // An owner callback can ignore abort. Keep its slot and byte reservation
      // until settlement, rather than accumulate unbounded abandoned work.
    }, opts.serverRequestTimeoutMs);
    serverPending.set(tag, { controller, timer, abandoned: false });
    reservedBytes += bytes;
    Promise.resolve().then(() => {
      if (!active() || !serverPending.has(tag)) return undefined;
      return opts.onAuthTokensRefresh({ requestId: id, params: packet.params, signal: controller.signal });
    }).then(result => {
      if (!serverPending.has(tag)) return;
      if (!keys(result, ['accessToken', 'chatgptAccountId', 'chatgptPlanType']) || !text(result.accessToken)
        || !text(result.chatgptAccountId) || (result.chatgptPlanType != null && !text(result.chatgptPlanType))) finish(null, -32603);
      else finish({ accessToken: result.accessToken, chatgptAccountId: result.chatgptAccountId,
        ...(own(result, 'chatgptPlanType') ? { chatgptPlanType: result.chatgptPlanType } : {}) }, null);
    }, _cause => finish(null, -32603));
  }

  function dispatch(packet, bytes) {
    if (!keys(packet, ['jsonrpc', 'id', 'method', 'params', 'result', 'error'])
      || (own(packet, 'jsonrpc') && packet.jsonrpc !== '2.0')) { poison('INVALID_FRAME'); return; }
    if (own(packet, 'method')) {
      if (!text(packet.method) || own(packet, 'result') || own(packet, 'error')) { poison('INVALID_FRAME'); return; }
      if (own(packet, 'id')) {
        if (!(typeof packet.id === 'string' && text(packet.id) && Buffer.byteLength(packet.id) <= opts.maxRequestIdBytes)
          && !Number.isSafeInteger(packet.id)) { poison('INVALID_FRAME'); return; }
        handleServerRequest(packet, bytes);
      } else if (NOTIFICATIONS.has(packet.method)) {
        if (!object(packet.params)) { poison('INVALID_FRAME'); return; }
        // Notifications are snapshots, not authorization. The owner explicitly
        // rereads after readiness; startup snapshots are not dispatched.
        if (state !== 'ready') return;
        try {
          const outcome = opts.onNotification({ method: packet.method, params: packet.params });
          // Synchronous owner callbacks are the admission rule. Contain an accidental
          // rejection, but fail closed rather than accumulate unbounded callback work.
          if (outcome && typeof outcome.then === 'function') {
            Promise.resolve(outcome).catch(_cause => { /* No private callback diagnostics escape. */ });
            poison('ASYNC_NOTIFICATION_CALLBACK');
          }
        } catch (_cause) { /* One owner listener must not break protocol dispatch. */ }
      }
      return;
    }
    if (own(packet, 'params') || !own(packet, 'id') || own(packet, 'result') === own(packet, 'error')
      || typeof packet.id !== 'string' || !/^native-[1-9][0-9]*$/.test(packet.id)) { poison('INVALID_FRAME'); return; }
    const number = Number(packet.id.slice(7));
    if (!Number.isSafeInteger(number) || number > serial) { poison('UNKNOWN_REPLY'); return; }
    if (own(packet, 'error') && (!object(packet.error) || !Number.isSafeInteger(packet.error.code) || typeof packet.error.message !== 'string')) {
      poison('INVALID_FRAME'); return;
    }
    // Old, duplicate or abandoned IDs never consume a current request.
    if (!pending.has(packet.id)) return;
    settle(packet.id, own(packet, 'error') ? 'RPC_REJECTED' : null, packet.result);
  }

  function consume(chunk) {
    if (!active()) return;
    let offset = 0;
    while (offset < chunk.length && active()) {
      const newline = chunk.indexOf(10, offset);
      const end = newline < 0 ? chunk.length : newline;
      const size = input.length + end - offset;
      if (size > opts.maxFrameBytes) { poison('FRAME_LIMIT'); return; }
      const fragment = chunk.subarray(offset, end);
      // Copy rather than retain an entire potentially large child-output chunk.
      const frame = input.length ? Buffer.concat([input, fragment], size) : Buffer.from(fragment);
      input.fill(0);
      input = frame;
      if (newline < 0) return;
      let packet;
      try { packet = JSON.parse(decoder.decode(input)); }
      catch (_cause) { poison('INVALID_FRAME'); return; }
      const bytes = input.length;
      input.fill(0);
      input = Buffer.alloc(0);
      dispatch(packet, bytes);
      offset = newline + 1;
    }
  }

  function waitClose(milliseconds) {
    if (closeObserved || !child) return Promise.resolve(true);
    return new Promise(resolve => {
      const timer = setTimeout(() => resolve(false), milliseconds);
      closed.then(() => { clearTimeout(timer); resolve(true); });
    });
  }

  function killOwned(signal) {
    // Never signal a reaped numeric PID (or any group). The ChildProcess owns
    // the process handle; exit and close are separate observed events.
    if (!child || exitObserved || child.exitCode !== null || child.signalCode !== null || !Number.isSafeInteger(child.pid)) return false;
    try { return child.kill(signal); } catch (_cause) { return false; }
  }

  function stop() {
    if (stopTask) return stopTask;
    state = 'stopping';
    dispose(faultCode ?? 'STOPPED');
    stopTask = (async () => {
      let forced = false;
      let timedOut = false;
      if (child) {
        // End the owned input first, then close our stdio resources. Output is
        // never forwarded during teardown and cannot hold close hostage.
        try { child.stdin.end(); } catch (_cause) { /* Disposal still closes the owned resource. */ }
        child.stdin.destroy();
        child.stdout.destroy();
        if (!await waitClose(opts.stopGraceMs)) {
          timedOut = true;
          forced = killOwned('SIGTERM') || forced;
          if (!await waitClose(opts.stopTermMs)) {
            forced = killOwned('SIGKILL') || forced;
            await waitClose(opts.stopKillMs);
          }
        }
      }
      state = 'stopped';
      return Object.freeze({ exitObserved, closeObserved, exitCode, signal: exitSignal, forced, timedOut });
    })();
    return stopTask;
  }

  function start(action) {
    if (!keys(action, ['explicitUserAction', 'signal']) || action.explicitUserAction !== true
      || (action.signal !== undefined && !(action.signal instanceof AbortSignal))) return Promise.reject(error('EXPLICIT_ACTION_REQUIRED'));
    if (startTask && active()) return startTask;
    if (state !== 'idle') return Promise.reject(error('UNAVAILABLE'));
    if (action.signal?.aborted) return Promise.reject(error('ABORTED'));
    state = 'starting';
    let startupTimer;
    let startupAbort;
    const startupDeadline = new Promise((_resolve, reject) => {
      const abandon = code => { poison(code); reject(error(code)); };
      startupTimer = setTimeout(() => abandon('STARTUP_TIMEOUT'), opts.startupTimeoutMs);
      startupAbort = () => abandon('ABORTED');
      action.signal?.addEventListener('abort', startupAbort, { once: true });
    });
    const launching = (async () => {
      try {
        const executable = await validateLaunch(opts);
        if (state !== 'starting' || action.signal?.aborted) throw error(action.signal?.aborted ? 'ABORTED' : 'STOPPED');
        const store = opts.credentialStorage === 'ephemeral' ? 'ephemeral' : 'file';
        // Pinned core/config.schema.json AuthCredentialsStoreMode: no auto/keyring.
        child = spawn(executable, ['-c', `cli_auth_credentials_store="${store}"`, 'app-server', '--listen', 'stdio://'], {
          cwd: opts.nativeCwd, shell: false, detached: false, windowsHide: true,
          env: { PATH: '/usr/bin:/bin', HOME: opts.codexHome, CODEX_HOME: opts.codexHome,
            XDG_CONFIG_HOME: opts.codexHome, XDG_DATA_HOME: opts.codexHome, XDG_CACHE_HOME: opts.codexHome },
          stdio: ['pipe', 'pipe', 'ignore'],
        });
        child.once('error', () => poison('PROCESS_FAULT'));
        child.once('exit', (code, signal) => {
          exitObserved = true; exitCode = code; exitSignal = signal;
          if (active()) poison('PROCESS_EXIT');
        });
        child.once('close', (code, signal) => {
          closeObserved = true;
          if (!exitObserved) { exitCode = code; exitSignal = signal; }
          resolveClose();
          if (active()) poison('PROCESS_CLOSE');
        });
        child.stdin.on('error', () => poison('WRITE_FAULT'));
        child.stdout.on('error', () => poison('READ_FAULT'));
        child.stdout.on('data', consume);
        child.stdout.once('end', () => poison(input.length ? 'PARTIAL_EOF' : 'EOF'));
        const result = await rpc('initialize', { clientInfo: opts.clientInfo,
          capabilities: { experimentalApi: opts.enableExperimentalChatgptTokens, explicitGatewayOauth: true, requestAttestation: false } }, false,
        { signal: action.signal }, true);
        if (!object(result) || result.codexHome !== opts.codexHome || result.platformFamily !== 'unix'
          || result.platformOs !== 'linux' || typeof result.userAgent !== 'string'
          || !result.userAgent.startsWith(`${opts.clientInfo.name}/${opts.expectedCliVersion} (`)) throw error('INITIALIZE_MISMATCH');
        await writeFrame(encode({ method: 'initialized' }));
        if (state !== 'starting') throw error('UNAVAILABLE');
        state = 'ready';
        return result;
      } catch (cause) {
        const failure = cause instanceof CodexAppServerError ? cause : error('PROCESS_FAULT');
        poison(failure.code);
        await stop();
        throw failure;
      }
    })();
    startTask = Promise.race([launching, startupDeadline]).catch(async cause => {
      await stop();
      throw cause instanceof CodexAppServerError ? cause : error('PROCESS_FAULT');
    }).finally(() => {
      clearTimeout(startupTimer);
      action.signal?.removeEventListener('abort', startupAbort);
    });
    return startTask;
  }

  return Object.freeze({
    start,
    stop,
    /** Private metadata only; no process IDs, raw diagnostics, paths or credentials. */
    get lifecycle() { return Object.freeze({ state, faultCode, pendingRequests: pending.size,
      serverRequests: serverPending.size, pendingBytes: reservedBytes + writeBytes,
      queuedWrites: writes.length + (writing ? 1 : 0), exitObserved, closeObserved }); },
    /** @param {object} params Explicit refreshToken boolean. @param {object} requestOptions Optional AbortSignal. @returns {Promise<object>} Raw private account result. */
    readAccount(params, requestOptions) {
      if (!keys(params, ['refreshToken']) || typeof params.refreshToken !== 'boolean') return Promise.reject(error('INVALID_PARAMS'));
      return rpc('account/read', { refreshToken: params.refreshToken }, params.refreshToken, requestOptions);
    },
    /** @param {object} requestOptions Optional AbortSignal. @returns {Promise<object>} Raw private quotas; no experiment exposure opted in. */
    readRateLimits(requestOptions) {
      return rpc('account/rateLimits/read', { supportsLunaReserve: false, excludeResetCreditDetails: true }, false, requestOptions);
    },
    /** @param {object} params API-key/ChatGPT/device-code parameters, or explicitly enabled Beta chatgptAuthTokens fields. @param {object} requestOptions Optional AbortSignal. @returns {Promise<object>} Raw private login result; owner controls browser separately. */
    startLogin(params, requestOptions) {
      let value;
      try { value = loginParams(params, opts.enableExperimentalChatgptTokens); } catch (cause) { return Promise.reject(cause); }
      return rpc('account/login/start', value, true, requestOptions);
    },
    /** @param {string} loginId Server login ID, not an RPC ID. @param {object} requestOptions Optional AbortSignal. @returns {Promise<object>} Raw cancellation acknowledgment, not rollback. */
    cancelLogin(loginId, requestOptions) {
      if (!text(loginId)) return Promise.reject(error('INVALID_PARAMS'));
      return rpc('account/login/cancel', { loginId }, true, requestOptions);
    },
    /** @param {object} requestOptions Optional AbortSignal. @returns {Promise<object>} Raw logout acknowledgment; timeout remains indeterminate. */
    logout(requestOptions) { return rpc('account/logout', null, true, requestOptions); },
  });
}
