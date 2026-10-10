# Native Codex credential storage

## Ownership

Managed accounts use a separate private vault, not the legacy credentials record. The [backend vault](<../../deepseek-harness-linux/packages/llm/llm-pi-ai/src/codex-account-vault.ts>) takes a lifetime kernel `flock` on a stable lock inode in a canonical directory. A live owner is never expired or replaced. Directory/file permissions are 0700/0600; aliases, symlinks, foreign ownership and hard-linked files are refused. POSIX support is required; missing native bindings fail closed.

The vault holds the public account manifest and opaque SDK OAuth grants in one versioned JSON document. Commits write an exclusively created private sibling, fsync it, rename it and fsync the directory. Metadata publishes only afterward. Logout removes the forgotten account's grant in the same commit; it neither selects a replacement nor revokes credentials at OpenAI. Concurrent uncommitted Add staging is retained until its own outcome. Restart deletes only unlisted crash staging and strictly named private temporary files; it never imports the legacy grant.

An uncertain commit or replaced lock aborts the ownership signal and blocks further admission until a fresh owner reopens the vault. Account/SDK/refresh work drains before the descriptor closes. Corrupt or unknown-version state is refused, not reset. Storage limits are 128 committed accounts, 256 grant records and 16 MiB; native account replies have a separate complete-frame limit. Power-loss guarantees also depend on the filesystem honoring fsync.

## Native activation

The [initializer](<../core/runtime/codex-profile.mjs>) enables managed mode through the official profile ConfigEditor before native Ready. It edits only the validated isolated desktop profile, preserves existing configuration and leaves model routes editable. It does not use a final provider overlay, migrate legacy credentials, launch OAuth, choose an account or send a model request. Existing legacy grants remain unadopted and require explicit sign-in to create managed accounts.

Installed Harness profiles and the currently running Web GUI are separate. These source changes do not update older executables or archives. A native profile with an unsupported Codex endpoint or explicit credential override is refused rather than silently rewritten.

## Verification

[Vault regressions](<../../deepseek-harness-linux/packages/llm/llm-pi-ai/tests/codex-account-vault.spec.ts>) exercise real flock, independent-process exclusion/death, persistent restart, atomic logout, concurrent staging, refresh serialization, ownership loss, corrupt/insecure files and private crash-temp cleanup. [Initializer tests](<../core/runtime/codex-profile.test.mjs>) exercise admission and preserve-current-field behavior; fake editor tests alone do not establish real profile integration. [The composed driver](<../scripts/qualify-managed-codex.mjs>) uses the built full profile and PUBLIC synthetic stored grants; its result must be collected before claiming a pass. No real sign-in, entitlement, paid generation or issuer-revocation evidence is implied.
