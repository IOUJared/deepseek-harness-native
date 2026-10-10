/** PUBLIC fixtures in exclusively owned temporary directories; real pinned flock. */
import nodeTest from 'node:test';
import assert from 'node:assert/strict';
import { promises as fs, fstat } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname, resolve } from 'node:path';
import { inspect, promisify } from 'node:util';
import { spawn, execFile } from 'node:child_process';
import { once } from 'node:events';
import { openCodexUsageStorage, CodexUsageStorageError } from './codex-usage-storage.mjs';
import { defaultCodexUsagePreferences } from './codex-usage-preferences.mjs';
import { decideCodexResetRetry } from './codex-usage-policy.mjs';
// Missing companion artifacts skip real-flock checks; broken artifacts fail.
async function artifactExists(url) {
  try { await fs.lstat(url); return true; }
  catch (error) { if (error.code === 'ENOENT') return false; throw error; }
}
const companionManifest = new URL('../../../deepseek-harness-linux/apps/cli/package.json', import.meta.url);
const manifestPresent = await artifactExists(companionManifest);
if (manifestPresent) {
  const manifest = JSON.parse(await fs.readFile(companionManifest, 'utf8'));
  assert.equal(manifest.name, '@deepseek-ai/dsh');
  assert.equal(manifest.version, '0.2.1-alpha.1', 'Companion CLI must be the exact supported alpha');
}
const storageURL = new URL('./codex-usage-storage.mjs', import.meta.url).href;
const flockURL = new URL('../../../deepseek-harness-linux/native/system/packages/entry/lib/flock.js', import.meta.url).href;
const flockPresent = await artifactExists(new URL(flockURL));
assert.ok(!flockPresent || manifestPresent, 'Present flock artifact requires its companion CLI manifest');
const artifactSkip = flockPresent ? false : 'Requires built same-version alpha flock artifact; real-flock integration unrun';
if (artifactSkip && process.env.DSH_REQUIRE_ALPHA_TEST_ARTIFACTS === '1') throw new Error(artifactSkip);
const { tryLockExclusive } = flockPresent ? await import(flockURL) : {};
if (flockPresent) assert.equal(typeof tryLockExclusive, 'function', 'Invalid flock artifact export');
const test = (name, body) => nodeTest(name, { skip: artifactSkip }, body);
const prefs = changes => ({ ...defaultCodexUsagePreferences(), ...changes });
const doc = (revision = 1, changes) => JSON.stringify({ version: 1, revision, preferences: prefs(changes) });
const code = expected => error => {
  assert.equal(error.code, expected);
  assert.equal(Object.hasOwn(error, 'cause'), false);
  return true;
};
const deferred = () => {
  let resolve;
  const promise = new Promise(done => { resolve = done; });
  return { promise, resolve };
};

async function fixture(t) {
  const base = await fs.realpath(await fs.mkdtemp(join(tmpdir(), 'PUBLIC-usage-storage-')));
  await fs.chmod(base, 0o700);
  const original = await fs.lstat(base);
  const directory = join(base, 'usage');
  const options = { directory, lockExclusive: tryLockExclusive, maxPendingUpdates: 4, maxDocumentBytes: 4096 };
  const owners = [];
  t.after(async () => {
    t.mock.restoreAll();
    for (const owner of owners) await owner.dispose().catch(error => {
      assert.equal(error.code, 'CLOSE_FAILED');
    });
    assert.equal(resolve(base), base);
    assert.equal(await fs.realpath(base), base);
    const actual = await fs.lstat(base);
    assert.equal(actual.ino, original.ino);
    assert.equal(actual.dev, original.dev);
    await fs.rm(base, { recursive: true });
  });
  return { base, directory, options, path: join(directory, 'usage.v1.json'),
    lock: join(directory, 'owner.lock'),
    async open(overrides = {}) {
      const owner = await openCodexUsageStorage({ ...options, ...overrides });
      owners.push(owner);
      return owner;
    },
    async seed(bytes, mode = 0o600) {
      await fs.mkdir(directory, { mode: 0o700 });
      await fs.writeFile(join(directory, 'usage.v1.json'), bytes, { mode });
      await fs.chmod(join(directory, 'usage.v1.json'), mode);
    },
  };
}

async function handlePrototype(f) {
  const path = join(f.base, 'PUBLIC-prototype');
  const handle = await fs.open(path, 'wx', 0o600);
  const prototype = Object.getPrototypeOf(handle);
  await handle.close();
  return prototype;
}

// An owned Node child receives explicit module URLs, empty env and IPC only.
async function childOwner(t, f) {
  const source = `
    import { openCodexUsageStorage } from ${JSON.stringify(storageURL)};
    import { tryLockExclusive } from ${JSON.stringify(flockURL)};
    let owner;
    process.on('message', async message => {
      if (message === 'open') {
        try {
          owner = await openCodexUsageStorage({directory:${JSON.stringify(f.directory)},
            lockExclusive:tryLockExclusive,maxPendingUpdates:4,maxDocumentBytes:4096});
          process.send({status:'owned'});
        } catch(error) { process.send({status:error.code}); }
      } else if (message === 'crash') process.exit(17);
      else if (message === 'exit') { if(owner) await owner.dispose(); process.exit(0); }
    });
    process.send({status:'ready'});
  `;
  const child = spawn(process.execPath, ['--input-type=module', '-e', source], {
    env: {}, stdio: ['ignore', 'ignore', 'pipe', 'ipc'],
  });
  let stderr = '';
  child.stderr.on('data', bytes => { stderr += bytes; });
  const exit = once(child, 'exit');
  t.after(async () => {
    if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL');
    await exit;
    assert.equal(stderr, '');
  });
  assert.deepEqual((await once(child, 'message'))[0], { status: 'ready' });
  return { child, exit, async ask(message) {
    const reply = once(child, 'message');
    child.send(message);
    return (await reply)[0];
  } };
}

test('real filesystem write/read, restart revision restore and consent-false policy conformance', async t => {
  const f = await fixture(t);
  let owner = await f.open();
  const initial = await owner.load();
  assert.equal(initial.persisted, false);
  assert.equal(initial.revision, 0);
  const changed = prefs({ composerDisplay: 'percent', retryAfterReset: true, experimentalQuota: true });
  assert.equal((await owner.update(0, changed)).revision, 1);
  assert.equal((await fs.lstat(f.path)).mode & 0o777, 0o600);
  assert.equal((await fs.lstat(f.directory)).mode & 0o777, 0o700);
  assert.equal((await fs.lstat(f.lock)).mode & 0o777, 0o600);
  const lockInode = (await fs.lstat(f.lock)).ino;
  await owner.dispose();
  owner = await f.open();
  const restored = await owner.load();
  assert.deepEqual(restored.preferences, changed);
  assert.equal(restored.revision, 1);
  assert.equal(restored.persisted, true);
  assert.equal((await fs.lstat(f.lock)).ino, lockInode);
  assert.equal((await owner.update(1, changed)).revision, 2);
  const context = { accountOpaqueId: 'PUBLIC', accountGeneration: 1, nowUnixSeconds: 100, modelAlias: null };
  const snapshot = { ...context, observedAtUnixSeconds: 100, ordinaryUsageAllowed: false, buckets: [] };
  const spec = { maxSnapshotAgeSeconds: 60, maxHistorySamples: 8, minRateIntervalSeconds: 1,
    maxRateIntervalSeconds: 60, maxRetryWaitSeconds: 60, maxRetryAttempts: 2, earlyWarningSeconds: 30 };
  const intent = { requestId: 'PUBLIC-r', currentRequestId: 'PUBLIC-r', accountOpaqueId: 'PUBLIC',
    accountGeneration: 1, modelAlias: null, failureKind: 'quotaDenied', failedBeforeOutput: true,
    cancelled: false, attempts: 0, resetAtUnixSeconds: 100, requestDeadlineUnixSeconds: 130 };
  assert.equal(decideCodexResetRetry(snapshot, context, restored.preferences, spec, intent).reason, 'usageDisallowed');
  assert.equal(inspect(owner), '[CodexUsageStorage]');
  assert.equal(JSON.stringify(owner), '"[CodexUsageStorage]"');
  assert.deepEqual((await fs.readdir(f.directory)).sort(), ['owner.lock', 'usage.v1.json']);
});

test('deleting an observed document fails closed and never resets or recreates revision zero', async t => {
  const f = await fixture(t);
  await f.seed(doc(7));
  const owner = await f.open();
  assert.equal((await owner.load()).revision, 7);
  assert.equal(resolve(f.path), f.path);
  await fs.unlink(f.path);
  await assert.rejects(owner.load(), code('READ_FAILED'));
  await assert.rejects(owner.update(0, prefs()), code('READ_FAILED'));
  await assert.rejects(owner.update(7, prefs()), code('READ_FAILED'));
  assert.deepEqual(await fs.readdir(f.directory), ['owner.lock']);
});

test('separate opens contend on actual flock; release preserves the stable lock file', async t => {
  const f = await fixture(t);
  const owner = await f.open();
  const lock = await fs.open(f.lock, 'r+');
  try { await assert.rejects(tryLockExclusive(lock.fd), error => ['EAGAIN', 'EWOULDBLOCK'].includes(error.code)); }
  finally { await lock.close(); }
  await assert.rejects(f.open(), code('LOCK_BUSY'));
  await owner.dispose();
  assert.equal((await (await f.open()).load()).persisted, false);
});

test('cross-process contention and crash recovery await the exact owned child exit', async t => {
  const f = await fixture(t);
  const owner = await f.open();
  const child = await childOwner(t, f);
  assert.deepEqual(await child.ask('open'), { status: 'LOCK_BUSY' });
  await owner.dispose();
  assert.deepEqual(await child.ask('open'), { status: 'owned' });
  await assert.rejects(f.open(), code('LOCK_BUSY'));
  child.child.send('crash');
  assert.deepEqual(await child.exit, [17, null]);
  assert.equal((await (await f.open()).load()).revision, 0);
});

test('options, canonical private existing parent and nonrecursive creation fail loud', async t => {
  const f = await fixture(t);
  for (const changes of [{ directory: 'relative' }, { directory: f.directory + '/.' },
    { lockExclusive: null }, { maxDocumentBytes: 65537 }, { maxPendingUpdates: 0 },
    { extra: true }, { directory: '/' }]) {
    await assert.rejects(openCodexUsageStorage({ ...f.options, ...changes }), code('INVALID_OPTIONS'));
  }
  await assert.rejects(f.open({ directory: join(f.base, 'missing', 'usage') }), code('OPEN_FAILED'));
  assert.deepEqual(await fs.readdir(f.base), []);
  await fs.chmod(f.base, 0o755);
  await assert.rejects(f.open(), code('OPEN_FAILED'));
  await fs.chmod(f.base, 0o700);
  const link = join(f.base, 'PUBLIC-parent-link');
  await fs.symlink(f.base, link);
  await assert.rejects(f.open({ directory: join(link, 'usage') }), code('OPEN_FAILED'));
});

test('corrupt, unsupported, invalid UTF8, multibyte oversize and nonprivate documents deny load', async t => {
  for (const bytes of ['{', 'null', doc().replace('"version":1', '"version":2'),
    doc().replace('"revision":1', '"revision":-1'), doc().replace('"off"', '"秘密"'),
    Buffer.from([0xc3, 0x28]), ' '.repeat(4097), '秘密'.repeat(700)]) {
    const f = await fixture(t);
    await f.seed(bytes);
    const owner = await f.open({ maxDocumentBytes: 2048 });
    await assert.rejects(owner.load(), code('READ_FAILED'));
  }
  const f = await fixture(t);
  await f.seed(doc(), 0o644);
  await assert.rejects((await f.open()).load(), code('READ_FAILED'));
});

test('symlink, hardlink, FIFO and nonprivate root or lock are rejected without reading targets', async t => {
  for (const kind of ['symlink', 'hardlink', 'fifo']) {
    const f = await fixture(t);
    const owner = await f.open();
    const target = join(f.base, 'PUBLIC-target');
    await fs.writeFile(target, doc(), { mode: 0o600 });
    if (kind === 'symlink') await fs.symlink(target, f.path);
    else if (kind === 'hardlink') await fs.link(target, f.path);
    else await promisify(execFile)('/usr/bin/mkfifo', ['-m', '600', f.path], { env: {} });
    await assert.rejects(owner.load(), code('READ_FAILED'));
    assert.equal(await fs.readFile(target, 'utf8'), doc());
  }
  for (const kind of ['symlink', 'hardlink', 'fifo']) {
    const f = await fixture(t);
    await fs.mkdir(f.directory, { mode: 0o700 });
    const target = join(f.base, 'PUBLIC-lock-target');
    await fs.writeFile(target, '', { mode: 0o600 });
    if (kind === 'symlink') await fs.symlink(target, f.lock);
    else if (kind === 'hardlink') await fs.link(target, f.lock);
    else await promisify(execFile)('/usr/bin/mkfifo', ['-m', '600', f.lock], { env: {} });
    await assert.rejects(f.open(), code('OPEN_FAILED'));
  }
  for (const target of ['root', 'lock']) {
    const f = await fixture(t);
    const owner = await f.open();
    await owner.dispose();
    await fs.chmod(target === 'root' ? f.directory : f.lock, 0o755);
    await assert.rejects(f.open(), code('OPEN_FAILED'));
  }
});

test('external revision and same-revision content changes conflict nondestructively', async t => {
  const f = await fixture(t);
  const owner = await f.open();
  await owner.update(0, prefs());
  await fs.writeFile(f.path, doc(8), { mode: 0o600 });
  await assert.rejects(owner.update(1, prefs()), code('CONFLICT'));
  const prototype = await handlePrototype(f);
  const original = prototype.write;
  t.mock.method(prototype, 'write', async function (...args) {
    const result = await original.apply(this, args);
    await fs.writeFile(f.path, doc(8, { composerDisplay: 'runway' }), { mode: 0o600 });
    return result;
  });
  await assert.rejects(owner.update(8, prefs()), code('CONFLICT'));
  assert.equal(await fs.readFile(f.path, 'utf8'), doc(8, { composerDisplay: 'runway' }));
  assert.deepEqual((await fs.readdir(f.directory)).sort(), ['owner.lock', 'usage.v1.json']);
});

test('lost root, parent or lock inode and permissions poison subsequent operations', async t => {
  for (const kind of ['root', 'parent', 'lock', 'mode']) {
    const f = await fixture(t);
    const parent = join(f.base, 'PUBLIC-parent');
    if (kind === 'parent') await fs.mkdir(parent, { mode: 0o700 });
    const owner = await f.open(kind === 'parent' ? { directory: join(parent, 'usage') } : {});
    const path = kind === 'root' ? f.directory : kind === 'lock' ? f.lock : parent;
    if (kind === 'mode') await fs.chmod(f.directory, 0o755);
    else {
      const moved = join(f.base, `PUBLIC-old-${kind}`);
      assert.equal(resolve(path), path);
      assert.equal(resolve(moved), moved);
      await fs.rename(path, moved);
      if (kind !== 'lock') await fs.mkdir(path, { mode: 0o700 });
      else await fs.writeFile(path, '', { mode: 0o600 });
    }
    await assert.rejects(owner.load(), code('READ_FAILED'));
    if (kind === 'mode') await fs.chmod(f.directory, 0o700);
    await assert.rejects(owner.load(), code('READ_FAILED'));
  }
});

test('acquisition fd remains open until the real kernel acquisition promise wrapper settles', async t => {
  const f = await fixture(t);
  const acquired = deferred();
  const release = deferred();
  let descriptor;
  const opening = f.open({ lockExclusive: async fd => {
    descriptor = fd;
    await tryLockExclusive(fd);
    acquired.resolve();
    await release.promise;
  } });
  await acquired.promise;
  await fs.chmod(f.directory, 0o755);
  assert.ok((await promisify(fstat)(descriptor)).isFile());
  release.resolve();
  await assert.rejects(opening, code('OWNERSHIP_LOST'));
  await assert.rejects(promisify(fstat)(descriptor), code('EBADF'));
  await fs.chmod(f.directory, 0o700);
  await f.open();
});

test('dispose rejects admission immediately but retains kernel lease through terminal write I/O', async t => {
  const f = await fixture(t);
  const owner = await f.open();
  const prototype = await handlePrototype(f);
  const original = prototype.write;
  const entered = deferred();
  const release = deferred();
  t.mock.method(prototype, 'write', async function (...args) {
    entered.resolve();
    await release.promise;
    return original.apply(this, args);
  });
  const active = owner.update(0, prefs());
  await entered.promise;
  const queued = owner.load();
  const disposal = owner.dispose();
  assert.equal(owner.dispose(), disposal);
  await assert.rejects(queued, code('CLOSED'));
  await assert.rejects(owner.load(), code('CLOSED'));
  await assert.rejects(f.open(), code('LOCK_BUSY'));
  release.resolve();
  assert.equal((await active).revision, 1);
  await disposal;
  await f.open();
});

test('write, file fsync and rename faults yield indeterminate and exact own-temp cleanup', async t => {
  for (const kind of ['write', 'sync', 'rename']) {
    const f = await fixture(t);
    const owner = await f.open();
    const prototype = await handlePrototype(f);
    const host = kind === 'rename' ? fs : prototype;
    const method = host[kind];
    const mock = t.mock.method(host, kind, async function (...args) {
      if (kind !== 'sync' || (await this.stat()).isFile()) throw new Error('PUBLIC fault');
      return method.apply(this, args);
    });
    await assert.rejects(owner.update(0, prefs()), code('INDETERMINATE'));
    mock.mock.restore();
    if (kind === 'rename') await assert.rejects(owner.load(), code('READ_FAILED'));
    else assert.equal((await owner.load()).persisted, false);
    assert.deepEqual(await fs.readdir(f.directory), ['owner.lock']);
  }
});

test('partial writes complete; zero-progress and cleanup faults stay indeterminate without sweeping', async t => {
  const f = await fixture(t);
  const owner = await f.open();
  const unowned = join(f.directory, '.usage-PUBLIC-unowned.tmp');
  await fs.writeFile(unowned, 'PUBLIC unowned', { mode: 0o600 });
  const prototype = await handlePrototype(f);
  const write = prototype.write;
  let calls = 0;
  const partial = t.mock.method(prototype, 'write', async function (buffer, offset, length, position) {
    calls += 1;
    return write.call(this, buffer, offset, Math.min(7, length), position);
  });
  assert.equal((await owner.update(0, prefs())).revision, 1);
  assert.ok(calls > 1);
  partial.mock.restore();
  const zero = t.mock.method(prototype, 'write', async () => ({ bytesWritten: 0 }));
  const unlink = t.mock.method(fs, 'unlink', async () => { throw new Error('PUBLIC cleanup'); });
  await assert.rejects(owner.update(1, prefs()), code('INDETERMINATE'));
  zero.mock.restore();
  unlink.mock.restore();
  assert.equal((await owner.load()).revision, 1);
  assert.equal(await fs.readFile(unowned, 'utf8'), 'PUBLIC unowned');
  assert.equal((await fs.readdir(f.directory)).length, 4); // lock, prefs, unowned and exact failed temp
  const short = t.mock.method(prototype, 'write', async function (buffer, offset, length, position) {
    await write.call(this, buffer, offset, 1, position);
    return { bytesWritten: length }; // A short real write falsely acknowledged by the fault.
  });
  await assert.rejects(owner.update(1, prefs()), code('INDETERMINATE'));
  short.mock.restore();
  assert.equal((await owner.load()).revision, 1);
});

test('cleanup refuses a replaced temporary inode and preserves the replacement', async t => {
  const f = await fixture(t);
  const owner = await f.open();
  const rename = fs.rename;
  let replacement;
  const mock = t.mock.method(fs, 'rename', async (source, destination) => {
    assert.equal(resolve(source), source);
    assert.equal(dirname(source), f.directory);
    assert.equal(destination, f.path);
    const moved = join(f.base, 'PUBLIC-original-temp');
    assert.equal(resolve(moved), moved);
    await rename(source, moved);
    await fs.writeFile(source, 'PUBLIC replacement', { mode: 0o600 });
    replacement = source;
    throw new Error('PUBLIC replaced temp');
  });
  await assert.rejects(owner.update(0, prefs()), code('INDETERMINATE'));
  mock.mock.restore();
  assert.equal(await fs.readFile(replacement, 'utf8'), 'PUBLIC replacement');
  await assert.rejects(owner.load(), code('READ_FAILED'));
});

test('post-rename root fsync uncertainty is reconciled by fsync of the actual loaded file and root', async t => {
  const f = await fixture(t);
  const owner = await f.open();
  const prototype = await handlePrototype(f);
  const sync = prototype.sync;
  const rename = fs.rename;
  let renamed = false;
  const renameMock = t.mock.method(fs, 'rename', async (...args) => {
    await rename(...args);
    renamed = true;
  });
  const syncMock = t.mock.method(prototype, 'sync', async function () {
    if (renamed && (await this.stat()).isDirectory()) throw new Error('PUBLIC root fsync');
    return sync.call(this);
  });
  await assert.rejects(owner.update(0, prefs()), code('INDETERMINATE'));
  syncMock.mock.restore();
  renameMock.mock.restore();
  const observed = [];
  t.mock.method(prototype, 'sync', async function () {
    observed.push((await this.stat()).isFile() ? 'file' : 'root');
    return sync.call(this);
  });
  assert.equal((await owner.load()).revision, 1);
  assert.deepEqual(observed, ['file', 'root']);
});

test('read and temp close uncertainty permanently stops reopening and remains visible on disposal', async t => {
  for (const kind of ['read', 'temp']) {
    const f = await fixture(t);
    if (kind === 'read') await f.seed(doc());
    const open = fs.open;
    let opens = 0;
    let failedCloses = 0;
    let pinCloses = 0;
    t.mock.method(fs, 'open', async (...args) => {
      opens += 1;
      const handle = await open(...args);
      const target = kind === 'read' ? args[0] === f.path
        : dirname(args[0]) === f.directory && args[0].includes('/.usage-');
      const close = handle.close.bind(handle);
      handle.close = async () => {
        await close();
        if (target) {
          failedCloses += 1;
          throw new Error('PUBLIC actual-close-then-reject');
        }
        pinCloses += 1;
      };
      return handle;
    });
    const owner = await f.open();
    if (kind === 'read') await assert.rejects(owner.load(), code('READ_FAILED'));
    else await assert.rejects(owner.update(0, prefs()), code('INDETERMINATE'));
    assert.equal(failedCloses, 1);
    const before = opens;
    await assert.rejects(owner.load(), code('READ_FAILED'));
    await assert.rejects(owner.update(kind === 'read' ? 1 : 0, prefs()), code('READ_FAILED'));
    assert.equal(opens, before);
    const disposal = owner.dispose();
    assert.equal(owner.dispose(), disposal);
    await assert.rejects(disposal, code('CLOSE_FAILED'));
    await assert.rejects(owner.dispose(), code('CLOSE_FAILED'));
    assert.equal(failedCloses, 1);
    assert.equal(pinCloses, 3);
    t.mock.restoreAll();
  }
});

test('premature EOF and in-place read changes fail loud; close errors remain visible', async t => {
  for (const kind of ['eof', 'change']) {
    const f = await fixture(t);
    await f.seed(doc());
    const owner = await f.open();
    const prototype = await handlePrototype(f);
    const read = prototype.read;
    const mock = t.mock.method(prototype, 'read', async function (...args) {
      if (kind === 'eof') return { bytesRead: 0, buffer: args[0] };
      const result = await read.apply(this, args);
      await fs.writeFile(f.path, doc(2), { mode: 0o600 });
      return result;
    });
    await assert.rejects(owner.load(), code('READ_FAILED'));
    mock.mock.restore();
  }
  const f = await fixture(t);
  const open = fs.open;
  let failClose = false;
  let gated = false;
  const entered = deferred();
  const release = deferred();
  const order = [];
  t.mock.method(fs, 'open', async (...args) => {
    const handle = await open(...args);
    const close = handle.close.bind(handle);
    handle.close = async () => {
      if (failClose) {
        const kind = (await handle.stat()).isDirectory() ? 'directory' : 'lock';
        order.push(kind);
        if (!gated && kind === 'directory') { gated = true; entered.resolve(); await release.promise; }
      }
      await close();
      if (failClose) throw new Error('PUBLIC close');
    };
    return handle;
  });
  const owner = await f.open();
  failClose = true;
  const disposal = owner.dispose();
  assert.equal(owner.dispose(), disposal);
  await entered.promise;
  const contender = await open(f.lock, 'r+');
  try { await assert.rejects(tryLockExclusive(contender.fd), error => ['EAGAIN', 'EWOULDBLOCK'].includes(error.code)); }
  finally { await contender.close(); }
  release.resolve();
  await assert.rejects(disposal, error => {
    assert.ok(error instanceof CodexUsageStorageError);
    assert.equal(inspect(error), 'CodexUsageStorageError [CLOSE_FAILED]');
    assert.deepEqual(error.toJSON(), { name: 'CodexUsageStorageError', code: 'CLOSE_FAILED' });
    return code('CLOSE_FAILED')(error);
  });
  assert.deepEqual(order, ['directory', 'directory', 'lock']);
});
