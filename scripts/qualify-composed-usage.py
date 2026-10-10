#!/usr/bin/env python3
"""Own native App/worker/full-alpha PUBLIC usage, runtime or local account lifecycle.

Usage remains the default. Runtime uses a route-only PUBLIC seed, eight pins and
inheritance reset; its document verifier requires existing PyYAML. Accounts uses
create-new owned synthetic PUBLIC stored records, never real grants/sign-in, and
qualifies local selection/logout/Maintenance/restart. No OAuth, token refresh,
browser, route activation or provider request is invoked by the reviewed commands.

Reuses existing composed Settings pidfd identity/containment supervision. Never
reads installed homes or credentials; does not start auth, catalog or providers.
The native fixture explicitly reserves an UNSENT startup catalog flight. This is
not normal startup-catalog, physical input or live quota qualification.
"""
import argparse
import asyncio
import fcntl
import hashlib
import importlib.util
import json
import os
import signal
from pathlib import Path
import subprocess
import time

BASE = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('composed_settings', BASE / 'scripts/qualify-composed-settings.py')
SUP = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SUP)
NATIVE = SUP.NATIVE


def private_json(path, data):
    SUP.private_json(path, data)


def document(root):
    path = root / 'harness/codex-usage/usage.v1.json'
    meta = path.lstat()
    assert path.is_file() and not path.is_symlink() and meta.st_mode & 0o777 == 0o600 and meta.st_size < 65536
    value = json.loads(path.read_bytes())
    assert value == {'version': 1, 'revision': 5, 'preferences': {
        'composerDisplay': 'off', 'usageAlert': 'off', 'customRemainingPercent': 17.5,
        'retryAfterReset': False, 'experimentalQuota': False}}
    return {'exactPublicOffDocument': True, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
            'bytes': meta.st_size, 'mode': oct(meta.st_mode & 0o777)}


def lock_released(root):
    path = root / 'harness/codex-usage/owner.lock'
    assert path.is_file() and not path.is_symlink()
    fd = os.open(path, os.O_RDWR | os.O_NOFOLLOW)
    try:
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        fcntl.flock(fd, fcntl.LOCK_UN)
    finally:
        os.close(fd)
    return True


def runtime_document(root, pinned):
    # Conditional verifier prerequisite; no dependency or codec is added to the app.
    import yaml
    path = root / 'harness/profiles/desktop/cordis.patch.yml'
    meta = path.lstat()
    assert path.is_file() and not path.is_symlink() and meta.st_mode & 0o777 == 0o600 and meta.st_size < 65536
    raw = path.read_bytes()
    rows = yaml.safe_load(raw)
    assert isinstance(rows, list) and len(rows) == 1
    row = rows[0]
    assert row['id'] == 'llm-pi-ai' and row['name'] == '@deepseek-ai/dsh-llm-pi-ai'
    config = row['config']
    assert config['nativeCodexAccounts'] is True
    assert config['nativeCodexAccountVault'] == str(root / 'harness/codex-accounts')
    assert set(config['providers']) == {'openai-codex'}
    route = config['providers']['openai-codex']
    assert route['displayName'] == 'PUBLIC preserved route label'
    pins = {'transport': 'websocket-cached', 'cacheRetention': 'long', 'timeoutMs': 0,
            'websocketConnectTimeoutMs': 0, 'streamIdleTimeoutMs': 12.5,
            'requestImagePixelBudget': 524288, 'requestImageMaxBytes': 524288, 'maxRequestImageBytes': 8388608}
    assert {key: route[key] for key in pins if key in route} == (pins if pinned else {})
    other = json.loads(json.dumps(rows))
    for key in pins:
        other[0]['config']['providers']['openai-codex'].pop(key, None)
    return {'exactEightPublicPins': pinned, 'allEightPinsRemoved': not pinned, 'labelAndNativeFlagsPreserved': True,
            'unrelatedFieldsSha256': hashlib.sha256(json.dumps(other, sort_keys=True).encode()).hexdigest(),
            'sha256': hashlib.sha256(raw).hexdigest(), 'bytes': meta.st_size, 'mode': oct(meta.st_mode & 0o777),
            'yamlReaderVersion': yaml.__version__}


def account_document(root, selected):
    # Read ONLY this owned synthetic PUBLIC document, never installed/real vaults.
    path = root / 'harness/codex-accounts/accounts.v1.json'
    meta = path.lstat()
    assert path.is_file() and not path.is_symlink() and meta.st_mode & 0o777 == 0o600 and meta.st_size < 65536
    raw = path.read_bytes()
    doc = json.loads(raw)
    ids = ['account-11111111-1111-4111-8111-111111111111', 'account-22222222-2222-4222-8222-222222222222']
    accounts = [{'opaqueId': key, 'label': 'PUBLIC ' + label, 'email': None, 'plan': None} for key, label in zip(ids, ['first', 'second'])]
    assert doc['version'] == 1 and doc['manifest'] == {'accounts': accounts if selected else [], 'selected': ids[1] if selected else None, 'epoch': 2 if selected else 4}
    records = [{'key': 'llm-pi-ai-codex-accounts/' + key, 'record': {'kind': 'grant', 'payload': {'type': 'oauth', 'access': f'PUBLIC-access-{n}', 'refresh': f'PUBLIC-refresh-{n}', 'expires': 4102444800000, 'accountId': f'PUBLIC-issuer-{n}', 'extra': {'untouched': ['PUBLIC-extra', None]}}}} for n, key in enumerate(ids)]
    assert doc['records'] == (records if selected else [])
    return {'version': 1, 'epoch': doc['manifest']['epoch'], 'publicAccountCount': len(doc['manifest']['accounts']),
            'selectedPublicSecond': selected, 'publicStoredPayloadsUnchanged': selected, 'allLocalRecordsDeleted': not selected,
            'sha256': hashlib.sha256(raw).hexdigest(), 'bytes': meta.st_size, 'mode': oct(meta.st_mode & 0o777)}


def account_lock_released(root):
    path = root / 'harness/codex-accounts/owner.lock'
    fd = os.open(path, os.O_RDWR | os.O_NOFOLLOW | os.O_CLOEXEC)
    try:
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        fcntl.flock(fd, fcntl.LOCK_UN)
    finally:
        os.close(fd)
    return True


async def phase(binary, root, runtime, name, scope='usage'):
    output = root / name
    output.mkdir(mode=0o700)
    command = [str(binary), '--output', str(output), '--state-root', str(root),
               '--runtime', str(runtime), '--phase', name, '--scope', scope]
    # Preserve exact same owned HOME/XDG state between independent backend processes.
    env = NATIVE.environment(root)
    # CLI deliberately requires a NEW EMPTY phase output; its log stays outside it.
    log = os.fdopen(os.open(root / (name + '-stderr.log'), os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), 'wb')
    interrupted = asyncio.Event()
    loop = asyncio.get_running_loop()
    handlers = []
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            loop.add_signal_handler(sig, interrupted.set)
            handlers.append(sig)
        child = subprocess.Popen(command, env=env, stdout=log, stderr=log, start_new_session=True)
    except BaseException:
        for sig in handlers:
            loop.remove_signal_handler(sig)
        log.close()
        raise
    pidfds = {}
    observed = set()
    root_pinned = False
    observation_failures = 0
    forced = False
    facts = None
    error = None
    try:
        identity, fd = SUP.pin_root(child)
        root_pinned = True
        pidfds[SUP.identity(identity)] = fd
        observed.add(SUP.identity(identity))
        deadline = time.monotonic() + SUP.QUALIFICATION_SECONDS
        while child.poll() is None:
            observation_failures += SUP.observe_descendants(child, identity, pidfds, observed)
            if facts is None:
                candidate = await SUP.METRICS.native_window(child.pid)
                if candidate and candidate.get('mapped'):
                    facts = candidate
            if interrupted.is_set() or time.monotonic() >= deadline:
                forced = True
                break
            await asyncio.sleep(.05)
    except (OSError, RuntimeError, ValueError):
        error = 'owned-observation-failed'
        forced = True
    finally:
        task = asyncio.create_task(SUP.cleanup_owned(child, pidfds, root_pinned))
        try:
            cleanup = await asyncio.shield(task)
        except asyncio.CancelledError:
            cleanup = await task
            error = 'qualification-cancelled'
            forced = True
        finally:
            for fd in pidfds.values():
                os.close(fd)
            for sig in handlers:
                loop.remove_signal_handler(sig)
            log.close()
    forced |= cleanup['rootWasRunningAtCleanup']
    app = json.loads((output / 'app.json').read_bytes()) if (output / 'app.json').is_file() else {'status': 'missing'}
    passed = (child.returncode == 0 and not forced and error is None and observation_failures == 0
              and cleanup['observedCleanupComplete'] and not cleanup['remainingBeforeExternalCleanup']
              and app.get('status') == 'passed' and facts
              and facts.get('class') == 'ai.deepseek.harness.native.usage-composed-fixture'
              and facts.get('xwayland') is False and (output / 'own-window.png').is_file())
    data = {'status': 'passed' if passed else 'failed', 'command': command, 'exitCode': child.returncode,
            'forced': forced, 'errorCode': error, 'ownedWindow': facts, 'observedOwnedProcessCount': len(observed),
            'ownershipObservationFailures': observation_failures, 'cleanup': cleanup, 'app': app,
            'parentPaintInspected': False, 'nativeKeyboardPointerInputQualified': False}
    if passed:
        if scope == 'usage':
            data['durableDocument'] = document(root)
            data['kernelUsageLockReleased'] = lock_released(root)
        elif scope == 'runtime':
            assert app.get('configurationScope') == 'runtime' and (output / 'own-images.png').is_file()
            data['durableRuntimePatch'] = runtime_document(root, name == 'initial')
        else:
            assert app.get('configurationScope') == 'accounts' and (output / 'own-maintenance.png').is_file()
            assert app.get('maintenanceStatusMatched') and app.get('maintenanceAccountActionsRejected') and app.get('accountScreenshotSaved')
            assert app.get('accountOrAuthorizationCommands') == {'initial': 8, 'restored': 6, 'inherited': 2}[name]
            consent = {'initial': ['own-review-cancel-select.png', 'own-review-select-first.png', 'own-review-select-second.png'],
                       'restored': ['own-review-local-logout-second.png', 'own-review-all.png'], 'inherited': []}[name]
            assert app.get('accountConsentCaptures') == consent
            for filename in consent:
                image = output / filename
                assert image.is_file() and not image.is_symlink() and image.stat().st_mode & 0o777 == 0o600
            if name == 'initial':
                assert app.get('localReviewCancelAndNoReviewConfirmNotAdmitted') and app.get('localReviewCancelEpochAndRosterReadbackMatched')
                assert app.get('localReviewCancelReadback') == app['accountLoaded']
                assert app.get('selectedSwitchActionsRejectedByActualApp') and app.get('selectedSwitchEpochAndSelectionUnchangedAfterExplicitReadback')
                assert app.get('selectedSwitchRejection') == {'epoch': 1, 'selected': 'account-11111111-1111-4111-8111-111111111111', 'ticketUnchangedAfterEachAction': True}
                readback = app.get('selectedSwitchReadback')
                assert readback == {**app['accountFinal'], 'epoch': 1, 'selected': 'account-11111111-1111-4111-8111-111111111111'}
            assert app.get('publicStoredFixturesOnly') and not app.get('oauthOrBrowserOrEnableModelsAdmitted')
            data['durableAccountDocument'] = account_document(root, name == 'initial')
            data['kernelAccountVaultLockReleased'] = account_lock_released(root)
    private_json(output / 'result.json', data)
    print(json.dumps({'phase': name, 'status': data['status'], 'exitCode': child.returncode}), flush=True)
    return data


async def run(options):
    binary = Path(options.binary).resolve(strict=True)
    runtime = Path(options.runtime).resolve(strict=True)
    assert binary.is_file() and os.access(binary, os.X_OK)
    root = NATIVE.prepare(options.output)
    before_hash = hashlib.sha256(binary.read_bytes()).hexdigest()
    results = []
    scope = options.scope
    names = ['initial', 'restored'] if scope == 'usage' else ['initial', 'restored', 'inherited']
    for name in names:
        result = await phase(binary, root, runtime, name, scope)
        results.append(result)
        if result['status'] != 'passed':
            break
    passed = len(results) == len(names) and all(r['status'] == 'passed' for r in results)
    stable = hashlib.sha256(binary.read_bytes()).hexdigest() == before_hash
    if scope == 'usage':
        durable = passed and results[0]['durableDocument'] == results[1]['durableDocument']
    elif scope == 'accounts':
        durable = passed and (results[1]['durableAccountDocument'] == results[2]['durableAccountDocument']
            and results[0]['app']['accountFinal'] == results[1]['app']['accountLoaded']
            and results[1]['app']['accountFinal'] == results[2]['app']['accountLoaded']
            and results[1]['app']['selectedLogoutHasNoReplacement'] and results[2]['app']['emptySignoutAllRejectedByActualApp'])
    else:
        fields = ['version', 'configured', 'values', 'baseValues', 'overrides']
        projected = lambda value: {key: value[key] for key in fields}
        durable = passed and (len({r['durableRuntimePatch']['unrelatedFieldsSha256'] for r in results}) == 1
            and results[1]['durableRuntimePatch']['sha256'] == results[2]['durableRuntimePatch']['sha256']
            and projected(results[0]['app']['runtimeAcknowledged']) == projected(results[1]['app']['runtimeLoaded'])
            and projected(results[1]['app']['runtimeAcknowledged']) == projected(results[2]['app']['runtimeLoaded']))
    data = {'status': 'passed' if passed and stable and durable else 'failed',
            'scope': __doc__, 'configurationScope': scope, 'binarySha256': before_hash, 'binaryUnchanged': stable, 'results': results,
            'persistedConfigurationVerifiedAcrossIndependentProcesses': durable,
            'persistedDocumentUnchangedAcrossIndependentProcesses': durable if scope == 'usage' else None,
            'runtimePinnedAndInheritedRestartsMatched': durable if scope == 'runtime' else None,
            'installedRuntimeProfilesAccountsOrDesktopChanged': False, 'actualProviderRequests': 0,
            'zeroRequestsEvidence': 'reviewed commands/source scope; NOT network instrumentation',
            'accountCounterScope': 'admitted native UI effects, NOT underlying private commands; each account/status refresh issues two local reads',
            'realQuotaOrNativeAlertsOrResetRetryQualified': False, 'parentPaintInspectionRequired': True}
    private_json(root / 'qualification.json', data)
    print(json.dumps({'status': data['status'], 'report': str(root / 'qualification.json')}), flush=True)
    return 0 if data['status'] == 'passed' else 1


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True)
    parser.add_argument('--runtime', required=True)
    parser.add_argument('--output', required=True)
    parser.add_argument('--scope', choices=['usage', 'runtime', 'accounts'], default='usage')
    raise SystemExit(asyncio.run(run(parser.parse_args())))
