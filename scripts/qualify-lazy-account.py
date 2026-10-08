#!/usr/bin/env python3
"""Own one fresh alpha public-profile Node process; qualify native observer opt-in only."""
import argparse
import asyncio
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import time

BASE = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('owned_settings', BASE / 'scripts/qualify-composed-settings.py')
OWNED = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(OWNED)


def valid_account(value):
    try:
        counts, events, stages = value['counts'], value['events'], value['snapshots']
        for group in [counts, events, *stages.values()]:
            if not isinstance(group, dict) or any(type(number) is not int or number < 0 for number in group.values()):
                return False
        return (value['status'] == 'passed' and value['phase'] == 'complete'
                and value['fatal'] is False and value['processWideNetworkTrace'] is False
                and value['providerHotReplacementQualified'] is False
                and all(value[name] is False for name in ['scriptedSignIn', 'scriptedModelOrCatalog', 'scriptedCredentialSave'])
                and stages['ready'] == {'watch': 0, 'state': 0, 'session': 0, 'live': 0, 'maxLive': 0, 'ended': 0, 'accountEvents': 0, 'platformEvents': 0}
                and stages['metadata']['state'] >= 2 and stages['metadata']['watch'] == 0
                and stages['metadata']['session'] == 0 and stages['metadata']['platformEvents'] == 0
                and stages['subscribed']['watch'] == 1 and stages['subscribed']['session'] == 1
                and stages['subscribed']['live'] == 1 and stages['subscribed']['accountEvents'] == 1
                and stages['unsubscribed']['live'] == 0 and stages['unsubscribed']['ended'] == 1
                and counts['watch'] == counts['session'] == counts['ended'] == 2
                and counts['live'] == 0 and counts['maxLive'] == 1
                and events['nonnullPlatform'] == events['observationErrors'] == 0
                and stages['stopped']['watch'] == counts['watch'] and stages['stopped']['live'] == 0)
    except (KeyError, TypeError):
        return False


async def run(options, *, script=None, report_name='account.json', report_key='account',
              evidence_env='DSH_NATIVE_ACCOUNT_EVIDENCE', validator=valid_account, scope=None,
              command_factory=None, capture_stdout=False):
    """Private shared ownership runner; CLI defaults remain the account qualification."""
    output = OWNED.NATIVE.prepare(options.output)
    script = script or BASE / 'scripts/qualify-lazy-account.mjs'
    source_paths = [script, BASE / 'core/runtime/fork-supervisor.mjs', BASE / 'core/runtime/supervisor.mjs', BASE / 'core/runtime/codex.mjs']
    digests = {str(path.relative_to(BASE)): hashlib.sha256(path.read_bytes()).hexdigest() for path in source_paths}
    env = OWNED.NATIVE.environment(output)
    env.update(DSH_TAURI_RUNTIME=str((BASE.parent / 'deepseek-harness-linux/apps/cli').resolve()),
               DSH_TAURI_EXPECTED_VERSION='0.2.1-alpha.1', DSH_TAURI_HOME=str(output / 'harness'))
    env[evidence_env] = str(output)
    command = command_factory(output) if command_factory else ['/usr/bin/node', str(script)]
    stdout = os.open(output / report_name, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600) if capture_stdout else subprocess.DEVNULL
    try:
        child = subprocess.Popen(command, env=env, cwd=output / 'workspace', stdin=subprocess.DEVNULL,
                                 stdout=stdout, stderr=subprocess.DEVNULL, start_new_session=True)
    finally:
        if capture_stdout:
            os.close(stdout)
    pidfds, observed = {}, set()
    pinned = False
    pin_failures = 0
    timed_out = False
    cancelled = False
    cleanup = None
    loop = asyncio.get_running_loop()
    def cancel():
        nonlocal cancelled
        cancelled = True
    for sig in [signal.SIGINT, signal.SIGTERM]:
        loop.add_signal_handler(sig, cancel)
    try:
        root, fd = OWNED.pin_root(child)
        pidfds[OWNED.identity(root)] = fd
        observed.add(OWNED.identity(root))
        pinned = True
        deadline = time.monotonic() + 35
        while child.returncode is None:
            pin_failures += OWNED.observe_descendants(child, root, pidfds, observed)
            if OWNED.poll_child(child):
                pin_failures += 1
                break
            if cancelled or time.monotonic() >= deadline:
                timed_out = not cancelled
                break
            if child.returncode is None:
                await asyncio.sleep(.02)
        cleanup = await OWNED.cleanup_owned(child, pidfds, pinned)
        account_path = output / report_name
        try:
            if account_path.is_symlink() or account_path.stat().st_mode & 0o7777 != 0o600 or account_path.stat().st_size > 65536:
                raise ValueError('invalid-private-report')
            account = json.loads(account_path.read_text())
        except (OSError, ValueError):
            account = {'status': 'invalid-or-missing-private-report'}
        unchanged = all(hashlib.sha256((BASE / name).read_bytes()).hexdigest() == digest for name, digest in digests.items())
        valid = (child.returncode == 0 and pinned and not timed_out and not cancelled and pin_failures == 0
                 and not cleanup['remainingBeforeExternalCleanup'] and not cleanup['rootWasRunningAtCleanup']
                 and cleanup['observedCleanupComplete'] and unchanged and validator(account))
        result = {'status': 'passed' if valid else 'failed', 'scope': scope or 'owned actual public alpha Loader/native supervisor observation; not Rust worker/GUI or global credential/network trace',
                  'sourceSha256': digests, 'sourceUnchanged': unchanged, 'exitCode': child.returncode,
                  'timedOut': timed_out, 'cancelled': cancelled, 'identityPinFailures': pin_failures,
                  'observedProcessCount': len(observed), 'cleanup': cleanup, report_key: account}
        OWNED.private_json(output / 'result.json', result)
        print(json.dumps({'status': result['status'], 'report': str(output / 'result.json')}))
        return 0 if valid else 1
    finally:
        if cleanup is None:
            await OWNED.cleanup_owned(child, pidfds, pinned)
        for fd in pidfds.values():
            os.close(fd)
        for sig in [signal.SIGINT, signal.SIGTERM]:
            loop.remove_signal_handler(sig)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True)
    raise SystemExit(asyncio.run(run(parser.parse_args())))
