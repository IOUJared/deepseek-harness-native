/** Private POSIX preferences persistence; advisory flock excludes cooperating writers only. */
import { constants, promises as fs } from 'node:fs';
import { createHash, randomUUID } from 'node:crypto';
import { dirname, isAbsolute, join, resolve } from 'node:path';
import { createCodexUsagePreferencesStore } from './codex-usage-preferences.mjs';

const inspect = Symbol.for('nodejs.util.inspect.custom');
const messages = Object.freeze({
  INVALID_OPTIONS: 'Invalid Codex usage storage options.',
  UNSUPPORTED_PLATFORM: 'Codex usage storage requires Linux or Darwin.',
  OPEN_FAILED: 'Codex usage storage could not be opened.',
  LOCK_BUSY: 'Codex usage storage is already owned.',
  LOCK_FAILED: 'Codex usage storage lock could not be acquired.',
  OWNERSHIP_LOST: 'Codex usage storage ownership was lost.',
  IO_FAILED: 'Codex usage storage I/O failed.',
  CLOSE_FAILED: 'Codex usage storage descriptor cleanup failed.',
});
const safeErrors = new WeakSet();
const receipt = status => Object.freeze({ status });
const flags = constants.O_NOFOLLOW | constants.O_NONBLOCK;
const identity = (a, b) => a.dev === b.dev && a.ino === b.ino
  && a.uid === b.uid && a.mode === b.mode && (a.isDirectory() || a.nlink === b.nlink);
const stamp = (a, b) => identity(a, b) && a.size === b.size
  && a.mtimeNs === b.mtimeNs && a.ctimeNs === b.ctimeNs;

/** Fixed diagnostics with no filesystem path, external error or cause. */
export class CodexUsageStorageError extends Error {
  /** @param {string} code Fixed storage error code. */
  constructor(code) {
    const safe = typeof code === 'string' && Object.hasOwn(messages, code) ? code : 'IO_FAILED';
    super(messages[safe]);
    this.name = 'CodexUsageStorageError';
    this.code = safe;
    safeErrors.add(this);
    Object.freeze(this);
  }
  toJSON() { return { name: this.name, code: this.code }; }
  [inspect]() { return `${this.name} [${this.code}]`; }
}
const fail = code => { throw new CodexUsageStorageError(code); };

function optionsOf(value) {
  try {
    if (!value || ![Object.prototype, null].includes(Object.getPrototypeOf(value))) fail('INVALID_OPTIONS');
    const fields = Object.getOwnPropertyDescriptors(value);
    const keys = ['directory', 'lockExclusive', 'maxPendingUpdates', 'maxDocumentBytes'];
    if (Reflect.ownKeys(fields).length !== keys.length) fail('INVALID_OPTIONS');
    const options = {};
    for (const key of keys) {
      if (!fields[key]?.enumerable || !Object.hasOwn(fields[key], 'value')) fail('INVALID_OPTIONS');
      options[key] = fields[key].value;
    }
    const { directory, lockExclusive, maxPendingUpdates, maxDocumentBytes } = options;
    if (typeof directory !== 'string' || !isAbsolute(directory) || resolve(directory) !== directory
        || dirname(directory) === directory || /[\x00-\x1f\x7f]/u.test(directory)
        || typeof lockExclusive !== 'function'
        || !Number.isSafeInteger(maxPendingUpdates) || maxPendingUpdates < 1 || maxPendingUpdates > 128
        || !Number.isSafeInteger(maxDocumentBytes) || maxDocumentBytes < 1 || maxDocumentBytes > 65_536) {
      fail('INVALID_OPTIONS');
    }
    return options;
  } catch { fail('INVALID_OPTIONS'); }
}

function privateStat(stat, directory, uid) {
  return stat.uid === BigInt(uid) && (stat.mode & 0o077n) === 0n
    && (directory ? stat.isDirectory() && (stat.mode & 0o700n) === 0o700n
      : stat.isFile() && stat.nlink === 1n && (stat.mode & 0o600n) === 0o600n);
}

async function closeAll(handles) {
  let failed = false;
  for (const handle of handles) {
    try { await handle.close(); }
    catch { failed = true; }
  }
  if (failed) fail('CLOSE_FAILED');
}

/**
 * Open an explicit canonical private usage directory (never a grant vault).
 * Its canonical, current-UID, private parent must already exist. Only the final
 * directory may be created, nonrecursively. owner.lock is stable and never deleted.
 * Required trusted lockExclusive(fd) makes one kernel flock attempt; its promise
 * must settle before this owner closes fd. No retries, deadlines or fallback.
 * load/update use the canonical preferences validator/serial queue. Persistence
 * uses bounded reads, content+inode CAS, exclusive 0600 temporary files, file fsync,
 * atomic rename and directory fsync. Invalid files fail loud; ENOENT is null only
 * before any valid document observation or rename attempt in this owner's lifetime.
 * A fresh owner cannot distinguish manual unlink from first use without history.
 * Failures during commit are indeterminate, never replayed; load reconciles and
 * fsyncs the actual observed valid file and root before acknowledging persistence.
 * Ownership loss or an I/O descriptor close rejection permanently poisons this
 * owner. A rejected close stays uncertain even if the fd actually closed;
 * I/O-close uncertainty makes disposal reject CLOSE_FAILED after all pinned close
 * attempts reach terminal.
 * Path checks are not adversarial containment. Descriptors are never retried.
 * dispose closes admission immediately, waits for actual I/O, then closes all fds
 * with the kernel-lock descriptor last, even when directory close attempts fail;
 * repeated calls return the same promise and close failures remain visible.
 * @param {{directory:string,lockExclusive:(fd:number)=>Promise<void>,maxPendingUpdates:number,maxDocumentBytes:number}} options Explicit limits and kernel primitive.
 * @returns {Promise<Readonly<{load:Function,update:Function,dispose:Function}>>} Preferences owner with safe inspection.
 */
export async function openCodexUsageStorage(options) {
  const { directory, lockExclusive, maxPendingUpdates, maxDocumentBytes } = optionsOf(options);
  if (!['linux', 'darwin'].includes(process.platform)) fail('UNSUPPORTED_PLATFORM');
  const uid = process.getuid();
  const parentPath = dirname(directory);
  const documentPath = join(directory, 'usage.v1.json');
  const lockPath = join(directory, 'owner.lock');
  const pins = [];
  let poisoned = false;
  let ioCloseFailed = false;
  let captured;
  let everObservedDocument = false;
  let renameAttempted = false;
  const cleanupHandles = () => [...pins.filter(item => item.path !== lockPath).reverse(),
    ...pins.filter(item => item.path === lockPath)].map(item => item.handle);

  async function pin(path, isDirectory) {
    const before = await fs.lstat(path, { bigint: true });
    if (!privateStat(before, isDirectory, uid) || await fs.realpath(path) !== path) fail('OPEN_FAILED');
    const handle = await fs.open(path, flags | (isDirectory
      ? constants.O_RDONLY | constants.O_DIRECTORY : constants.O_RDWR));
    const item = { path, handle, stat: before, isDirectory };
    pins.push(item);
    if (!identity(before, await handle.stat({ bigint: true }))) fail('OPEN_FAILED');
    return item;
  }

  async function guard() {
    if (poisoned) fail('OWNERSHIP_LOST');
    try {
      for (const item of pins) {
        const live = await fs.lstat(item.path, { bigint: true });
        const actual = await item.handle.stat({ bigint: true });
        if (!privateStat(live, item.isDirectory, uid) || !identity(item.stat, live)
            || !identity(item.stat, actual) || await fs.realpath(item.path) !== item.path) {
          fail('OWNERSHIP_LOST');
        }
      }
    } catch {
      poisoned = true;
      fail('OWNERSHIP_LOST');
    }
  }

  async function phase(action) {
    await guard();
    const result = await action();
    await guard();
    return result;
  }

  async function closeIO(handle) {
    let failed = false;
    try { await guard(); }
    catch { failed = true; }
    try { await handle.close(); }
    catch { failed = true; ioCloseFailed = true; poisoned = true; }
    try { await guard(); }
    catch { failed = true; }
    if (failed) fail('IO_FAILED');
  }

  let root;
  try {
    const parent = await pin(parentPath, true);
    await guard();
    try { await fs.mkdir(directory, { mode: 0o700 }); }
    catch (error) { if (error.code !== 'EEXIST') throw error; }
    await guard();
    root = await pin(directory, true);
    await phase(() => parent.handle.sync());
    await phase(() => root.handle.sync());
    let newLock;
    try {
      await guard();
      newLock = await fs.open(lockPath, flags | constants.O_RDWR
        | constants.O_CREAT | constants.O_EXCL, 0o600);
    } catch (error) { if (error.code !== 'EEXIST') throw error; }
    if (newLock) {
      // Track immediately, including setup failures before the stable pin exists.
      const item = { path: lockPath, handle: newLock, stat: null, isDirectory: false };
      pins.push(item);
      item.stat = await newLock.stat({ bigint: true });
    } else await pin(lockPath, false);
    await guard();
    const lock = pins.at(-1);
    try { await lockExclusive(lock.handle.fd); }
    catch (error) {
      fail(error?.code === 'EAGAIN' || error?.code === 'EWOULDBLOCK' ? 'LOCK_BUSY' : 'LOCK_FAILED');
    }
    await phase(() => lock.handle.sync());
    await phase(() => root.handle.sync());
  } catch (error) {
    await closeAll(cleanupHandles());
    if (safeErrors.has(error)) throw error;
    fail('OPEN_FAILED');
  }

  // Reuse the exact canonical decoder instead of maintaining a second schema.
  async function validate(bytes) {
    const validator = createCodexUsagePreferencesStore({
      readDocument: () => bytes, commitDocument: () => receipt('indeterminate'),
      maxPendingUpdates, maxDocumentBytes,
    });
    try { await validator.load(); }
    finally { await validator.dispose(); }
  }

  async function readDisk() {
    await guard();
    let handle;
    try { handle = await fs.open(documentPath, flags | constants.O_RDONLY); }
    catch (error) {
      await guard();
      if (error.code !== 'ENOENT') throw error;
      // Recheck absence; a newly appeared file is not an unwritten snapshot.
      try { await fs.lstat(documentPath); }
      catch (missing) {
        if (missing.code !== 'ENOENT') throw missing;
        if (everObservedDocument || renameAttempted) fail('IO_FAILED');
        await phase(() => root.handle.sync());
        return { bytes: null, stat: null, hash: null };
      }
      fail('IO_FAILED');
    }
    try {
      await guard();
      const before = await phase(() => handle.stat({ bigint: true }));
      if (!privateStat(before, false, uid) || before.size > BigInt(maxDocumentBytes)) fail('IO_FAILED');
      const bytes = Buffer.alloc(Number(before.size));
      let offset = 0;
      while (offset < bytes.length) {
        const { bytesRead } = await phase(() => handle.read(bytes, offset, bytes.length - offset, offset));
        if (bytesRead === 0) fail('IO_FAILED');
        offset += bytesRead;
      }
      const extra = await phase(() => handle.read(Buffer.alloc(1), 0, 1, offset));
      const sameFile = async () => {
        const actual = await handle.stat({ bigint: true });
        const named = await fs.lstat(documentPath, { bigint: true });
        if (!stamp(before, actual) || !stamp(before, named)) fail('IO_FAILED');
      };
      if (extra.bytesRead !== 0) fail('IO_FAILED');
      await phase(sameFile);
      await validate(bytes);
      everObservedDocument = true;
      await phase(() => handle.sync());
      await phase(() => root.handle.sync());
      await phase(sameFile);
      return { bytes, stat: before, hash: createHash('sha256').update(bytes).digest('hex') };
    } finally { await closeIO(handle); }
  }

  const matches = (a, b) => a && a.hash === b.hash
    && (a.stat === null ? b.stat === null : b.stat !== null && stamp(a.stat, b.stat));

  async function commitDocument({ document }) {
    const expected = captured;
    let temp;
    let tempStat;
    let tempPath;
    let renamed = false;
    let status = 'indeterminate';
    try {
      if (!matches(expected, await readDisk())) return receipt('conflict');
      tempPath = join(directory, `.usage-${randomUUID()}.tmp`);
      // Exact canonical same-directory path only; no caller-supplied temp names.
      if (resolve(tempPath) !== tempPath || dirname(tempPath) !== directory) fail('IO_FAILED');
      await guard();
      temp = await fs.open(tempPath, flags | constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL, 0o600);
      tempStat = await phase(() => temp.stat({ bigint: true }));
      if (!privateStat(tempStat, false, uid)) fail('IO_FAILED');
      const bytes = Buffer.from(JSON.stringify(document));
      let offset = 0;
      while (offset < bytes.length) {
        const { bytesWritten } = await phase(() => temp.write(bytes, offset, bytes.length - offset, offset));
        if (bytesWritten === 0) fail('IO_FAILED');
        offset += bytesWritten;
      }
      await phase(() => temp.sync());
      const written = await phase(() => temp.stat({ bigint: true }));
      if (written.size !== BigInt(bytes.byteLength)) fail('IO_FAILED');
      await phase(async () => {
        const live = await fs.lstat(tempPath, { bigint: true });
        if (!privateStat(live, false, uid) || !identity(tempStat, live)
            || !stamp(written, live)) fail('IO_FAILED');
      });
      if (!matches(expected, await readDisk())) status = 'conflict';
      else {
        await guard();
        renameAttempted = true;
        await fs.rename(tempPath, documentPath);
        renamed = true;
        await guard();
        const live = await fs.lstat(documentPath, { bigint: true });
        if (!privateStat(live, false, uid) || !identity(tempStat, live)
            || live.size !== written.size || live.mtimeNs !== written.mtimeNs) fail('IO_FAILED');
        await phase(() => root.handle.sync());
        await phase(async () => {
          if (!stamp(live, await fs.lstat(documentPath, { bigint: true }))
              || !stamp(live, await temp.stat({ bigint: true }))) fail('IO_FAILED');
        });
        status = 'committed';
      }
    } catch { status = 'indeterminate'; }
    finally {
      if (temp) {
        try { await closeIO(temp); }
        catch { status = 'indeterminate'; }
      }
      if (tempPath && tempStat && !renamed) {
        try {
          await guard();
          const live = await fs.lstat(tempPath, { bigint: true });
          if (resolve(tempPath) !== tempPath || dirname(tempPath) !== directory
              || await fs.realpath(tempPath) !== tempPath || !identity(tempStat, live)) fail('IO_FAILED');
          await phase(() => fs.unlink(tempPath));
          await phase(() => root.handle.sync());
        } catch { status = 'indeterminate'; }
      }
    }
    return receipt(status);
  }

  const engine = createCodexUsagePreferencesStore({
    async readDocument() { captured = await readDisk(); return captured.bytes; },
    commitDocument, maxPendingUpdates, maxDocumentBytes,
  });
  let disposal;
  return Object.freeze({
    load: () => engine.load(),
    update: (revision, preferences) => engine.update(revision, preferences),
    dispose() {
      if (!disposal) {
        const drained = engine.dispose();
        disposal = drained.then(async () => {
          await closeAll(cleanupHandles());
          if (ioCloseFailed) fail('CLOSE_FAILED');
        });
      }
      return disposal;
    },
    toJSON: () => '[CodexUsageStorage]',
    [inspect]: () => '[CodexUsageStorage]',
  });
}
