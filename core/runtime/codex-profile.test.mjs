/** Private profile admission only; real ConfigEditor/Loader composition is qualified separately. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { chmod, mkdir, mkdtemp, readFile, realpath, rm, symlink, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { initializeManagedCodex } from './codex-profile.mjs';

async function fixture(t) {
  const home = await realpath(await mkdtemp(join(tmpdir(), 'dsh-public-codex-profile-')));
  t.after(() => rm(home, { recursive: true, force: true }));
  const dir = join(home, 'profiles', 'desktop');
  await mkdir(dir, { recursive: true, mode: 0o700 });
  const patchPath = join(dir, 'cordis.patch.yml');
  await writeFile(patchPath, 'PUBLIC-preserved-profile\n', { mode: 0o600 });
  const profile = { name: 'desktop', home, dir, patchPath };
  let reads = 0;
  let edits = 0;
  let ready = 0;
  const entry = { options: { id: 'llm-pi-ai', name: '@deepseek-ai/dsh-llm-pi-ai', config: {
    providers: { 'PUBLIC-provider': { displayName: 'PUBLIC preserved' } }, unknownOwnedSetting: 'PUBLIC retained',
  } }, fiber: { async await() { ready++; } } };
  const editor = { documentPath: patchPath, entries() { reads++; return [entry]; },
    async edit(target, change) {
      assert.equal(target, entry);
      edits++;
      entry.options.config = change(structuredClone(entry.options.config));
    } };
  const ctx = { get(name) { return { profileContext: profile, configEditor: editor, codexAccounts: {} }[name]; } };
  return { home, profile, entry, editor, ctx, counts: () => ({ reads, edits, ready }) };
}

test('uses official editor, preserves all current fields and avoids repeat writes', async t => {
  const f = await fixture(t);
  await initializeManagedCodex(f.ctx, f.home);
  assert.deepEqual(f.entry.options.config, { providers: { 'PUBLIC-provider': { displayName: 'PUBLIC preserved' } },
    unknownOwnedSetting: 'PUBLIC retained', nativeCodexAccounts: true, nativeCodexAccountVault: join(f.home, 'codex-accounts') });
  assert.deepEqual(f.counts(), { reads: 2, edits: 1, ready: 1 });
  await initializeManagedCodex(f.ctx, f.home);
  assert.deepEqual(f.counts(), { reads: 4, edits: 1, ready: 2 });
  assert.equal(await readFile(f.profile.patchPath, 'utf8'), 'PUBLIC-preserved-profile\n');
});

test('refuses wrong profile, detached editor and missing root account service', async t => {
  const f = await fixture(t);
  f.profile.name = 'web';
  await assert.rejects(initializeManagedCodex(f.ctx, f.home));
  assert.equal(f.counts().edits, 0);
  f.profile.name = 'desktop';
  f.editor.documentPath = join(f.home, 'outside.patch.yml');
  await assert.rejects(initializeManagedCodex(f.ctx, f.home));
  assert.equal(f.counts().edits, 0);
  f.editor.documentPath = f.profile.patchPath;
  const ctx = { get(name) { return name === 'codexAccounts' ? undefined : f.ctx.get(name); } };
  await assert.rejects(initializeManagedCodex(ctx, f.home));
});

test('refuses aliases and insecure home permissions before config writes', async t => {
  const f = await fixture(t);
  const alias = join(f.home, 'alias');
  await symlink(f.home, alias);
  await assert.rejects(initializeManagedCodex(f.ctx, alias));
  await chmod(f.home, 0o755);
  await assert.rejects(initializeManagedCodex(f.ctx, f.home));
  assert.equal(f.counts().edits, 0);
});

test('propagates official edit refusal without fallback to legacy account mode', async t => {
  const f = await fixture(t);
  f.editor.edit = async () => { throw new Error('PUBLIC shadowed configuration'); };
  await assert.rejects(initializeManagedCodex(f.ctx, f.home), /PUBLIC shadowed configuration/);
  assert.equal(f.counts().ready, 0);
});
