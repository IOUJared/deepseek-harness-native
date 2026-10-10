/** Native-owned account mode uses the editable isolated profile, never a shadowing owner overlay. */
import { lstat, realpath } from 'node:fs/promises';
import { isAbsolute, join, normalize } from 'node:path';

const fail = () => new Error('Native Codex profile initialization unavailable');

/** Activate managed storage only in the already validated private desktop profile.
 * @param {object} ctx Same-version supported profile's root Context.
 * @param {string} home Canonical isolated Harness home owned by Core.
 * @returns {Promise<void>} After official ConfigEditor reconciliation and account service readiness.
 */
export async function initializeManagedCodex(ctx, home) {
  if (typeof home !== 'string' || !isAbsolute(home) || normalize(home) !== home
      || /[\x00-\x1f\x7f]/.test(home) || await realpath(home) !== home) throw fail();
  const info = await lstat(home);
  if (!info.isDirectory() || (info.mode & 0o077) !== 0 || info.uid !== process.getuid()) throw fail();
  const profile = ctx.get('profileContext');
  const editor = ctx.get('configEditor');
  const expected = join(home, 'profiles', 'desktop');
  if (!profile || profile.name !== 'desktop' || profile.home !== home || profile.dir !== expected
      || profile.patchPath !== join(expected, 'cordis.patch.yml') || !editor
      || editor.documentPath !== profile.patchPath || await realpath(expected) !== expected) throw fail();
  const patch = await lstat(profile.patchPath);
  if (!patch.isFile() || patch.isSymbolicLink()) throw fail();
  const rows = editor.entries().filter(entry => entry.options.id === 'llm-pi-ai'
    && entry.options.name === '@deepseek-ai/dsh-llm-pi-ai');
  if (rows.length !== 1 || !rows[0].fiber) throw fail();
  const vault = join(home, 'codex-accounts');
  const current = rows[0].options.config ?? {};
  if (current.nativeCodexAccounts !== true || current.nativeCodexAccountVault !== vault) {
    await editor.edit(rows[0], raw => ({ ...raw, nativeCodexAccounts: true, nativeCodexAccountVault: vault }));
  }
  const next = editor.entries().filter(entry => entry.options.id === 'llm-pi-ai'
    && entry.options.name === '@deepseek-ai/dsh-llm-pi-ai');
  if (next.length !== 1 || !next[0].fiber) throw fail();
  await next[0].fiber.await();
  if (!ctx.get('codexAccounts')) throw fail();
}
