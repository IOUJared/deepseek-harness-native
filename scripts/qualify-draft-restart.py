#!/usr/bin/env python3
"""Two owned native App processes, joined save/shutdown and cold UNSENT recovery; no Send, bounded App intent/PUBLIC records, network unqualified."""
import argparse
import asyncio
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import stat
import subprocess
import time

BASE = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('draft_owned_lifecycle', BASE / 'scripts/qualify-composed-settings.py')
OWNED = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(OWNED)
LATEST = 'PUBLIC unsent draft\nΚαλημέρα · 日本語 · 🦀\nPUBLIC newer edit while joined save outcome is held\nمرحبا · café'
STOP = {'exited': True, 'graceful': True, 'containmentUnknown': False, 'observedDescendantsRemaining': 0, 'exitCode': 0}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def private_bytes(path, limit=65536):
    before = path.lstat()
    if (not stat.S_ISREG(before.st_mode) or stat.S_IMODE(before.st_mode) != 0o600
            or before.st_uid != os.getuid() or before.st_nlink != 1 or not 0 < before.st_size <= limit):
        raise ValueError('private-regular-artifact-required')
    with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC), 'rb') as stream:
        opened = os.fstat(stream.fileno())
        if (opened.st_dev, opened.st_ino, opened.st_mode, opened.st_uid, opened.st_nlink, opened.st_size) != (
                before.st_dev, before.st_ino, before.st_mode, before.st_uid, before.st_nlink, before.st_size):
            raise ValueError('private-artifact-changed')
        data = stream.read(limit + 1)
    if len(data) != before.st_size:
        raise ValueError('private-artifact-size-changed')
    return data


def unique_object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise ValueError('duplicate-private-json-key')
        value[key] = item
    return value


def private_json(path):
    value = json.loads(private_bytes(path), object_pairs_hook=unique_object)
    if type(value) is not dict:
        raise ValueError('private-object-required')
    return value


def sources():
    paths = {Path(__file__), Path(OWNED.__file__), Path(OWNED.NATIVE.__file__), Path(OWNED.METRICS.__file__),
             BASE / 'app/examples/draft_composed_smoke.rs', BASE / 'scripts/test_qualify_draft_restart.py'}
    for directory, extension in [('app/src', '*.rs'), ('core/src', '*.rs'), ('transport/src', '*.rs'), ('core/runtime', '*.mjs')]:
        paths.update((BASE / directory).rglob(extension))
    for directory in ['app', 'core', 'transport']:
        paths.update(path for name in ['Cargo.toml', 'Cargo.lock'] if (path := BASE / directory / name).is_file())
    runtime = BASE.parent / 'deepseek-harness-linux/apps/cli'
    paths.add(runtime / 'package.json')
    paths.update((runtime / 'lib').rglob('*.js'))
    return {str(path): digest(path) for path in sorted(paths)}


def natural(value, maximum=(1 << 64) - 1, minimum=0):
    return type(value) is int and minimum <= value <= maximum


def session_id(value):
    return (type(value) is str and 0 < len(value.encode('utf-8', errors='replace')) <= 128
            and not any(char.isspace() or ord(char) < 32 or ord(char) == 127 or 0xD800 <= ord(char) <= 0xDFFF for char in value))


def valid(app, mode, pid):
    """Validate exact native report types; counters/records are not a global network trace."""
    common = ['publicOnly', 'actualAppExercised', 'actualDraftTasks', 'wholeProfileUnchanged', 'joinedOutcomeNotBlockedSyscall',
              'draftRecoveryLoaded', 'draftIoJoined', 'draftsCleanAtExit', 'latestSnapshotDurable', 'modelCatalogSucceeded',
              'inspectionObserved', 'completePUBLICSnapshotObserved', 'gracefulStopped']
    false_flags = ['backendMetricsAvailable', 'networkTraceQualified', 'physicalInputQualified', 'crashDurabilityQualified',
                   'driverPromptResumeModelSigninAllowed']
    if type(app) is not dict or mode not in ('write', 'recover') or not natural(pid, (1 << 31) - 1, 1):
        return False
    stop, kinds = app.get('stop'), app.get('observedPUBLICRecordKinds')
    if not (app.get('status') == 'passed' and app.get('mode') == mode and natural(app.get('pid'), (1 << 31) - 1, 1)
            and app['pid'] == pid and session_id(app.get('sessionId')) and 'errorCode' not in app
            and app.get('version') == '0.2.1-alpha.1' and app.get('windowBackend') == 'wayland'
            and all(app.get(key) is True for key in common) and all(app.get(key) is False for key in false_flags)
            and all(natural(app.get(key)) and app[key] == 0 for key in ['actualAppPromptCommands', 'promptSerial', 'unexpectedWork', 'manualBusinessMessagesForwarded'])
            and natural(app.get('modelCatalogReads')) and app['modelCatalogReads'] == 1
            and type(stop) is dict and set(stop) == set(STOP)
            and all(type(stop[key]) is type(value) and stop[key] == value for key, value in STOP.items())
            and type(kinds) is list and kinds and all(type(kind) is str for kind in kinds)
            and all('llm' not in kind and kind not in ['step/start', 'step/end', 'request/header', 'tool/call', 'tool/result', 'assistant/message', 'user/message'] for kind in kinds)):
        return False
    if mode == 'write':
        return (all(app.get(key) is True for key in ['stoppedWhileBusy', 'exitAnywayBlockedWhileBusy', 'heldOutcomeReleased'])
                and natural(app.get('savedOutcomes')) and app['savedOutcomes'] == 2
                and natural(app.get('heldRevision'), minimum=1) and natural(app.get('latestSavedRevision'), minimum=1)
                and app['latestSavedRevision'] > app['heldRevision'])
    return (app.get('recoveredNeedsReview') is True and app.get('coldExplicitSelectOnly') is True
            and app.get('coldRosterContainedSubject') is True and app.get('sendAllowed') is False
            and natural(app.get('savedOutcomes')) and app['savedOutcomes'] == 0)


def valid_restart(first, second, manifest, document):
    """Correlate two validated App reports with only the explicitly owned PUBLIC document."""
    return (type(first) is dict and type(second) is dict and type(manifest) is dict and type(document) is dict
            and valid(first, 'write', first.get('pid')) and valid(second, 'recover', second.get('pid'))
            and first['pid'] != second['pid'] and first['sessionId'] == second['sessionId']
            and natural(manifest.get('version')) and manifest['version'] == 1 and manifest.get('publicOnly') is True
            and natural(manifest.get('firstPid'), (1 << 31) - 1, 1) and manifest['firstPid'] == first['pid']
            and manifest.get('sessionId') == first['sessionId'] and set(document) == {'version', 'drafts'}
            and natural(document.get('version')) and document['version'] == 1 and type(document.get('drafts')) is dict
            and document['drafts'] == {first['sessionId']: LATEST})


async def phase(output, binary, runtime, mode, scale, interrupted, deadline):
    args = [str(binary), '--fixture-mode', mode, '--runtime', str(runtime), '--expected-version', '0.2.1-alpha.1',
            '--home', str(output / 'harness'), '--user-home', str(output / 'user'), '--cwd', str(output / 'workspace'), '--scale', str(scale)]
    env = OWNED.NATIVE.environment(output)
    env['PATH'] = '/usr/bin:/bin'
    descriptors = [os.open(output / f'{mode}-{name}.log', os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600) for name in ['stdout', 'stderr']]
    try:
        child = subprocess.Popen(args, env=env, cwd=output / 'workspace', stdin=subprocess.DEVNULL,
                                 stdout=descriptors[0], stderr=descriptors[1], start_new_session=True)
    finally:
        for fd in descriptors:
            os.close(fd)
    pidfds, observed, pinned, cleanup = {}, set(), False, None
    report = {'status': 'failed', 'mode': mode, 'pid': child.pid, 'identityPinFailures': 0, 'timedOut': False}
    try:
        root, fd = OWNED.pin_root(child)
        pidfds[OWNED.identity(root)] = fd
        observed.add(OWNED.identity(root))
        pinned = True
        while child.returncode is None:
            report['identityPinFailures'] += OWNED.observe_descendants(child, root, pidfds, observed)
            if OWNED.poll_child(child):
                raise ValueError('owned-child-poll-failed')
            if interrupted.is_set() or time.monotonic() >= deadline:
                report['timedOut'] = not interrupted.is_set()
                break
            if child.returncode is None:
                await asyncio.sleep(.02)
        cleanup = await OWNED.cleanup_owned(child, pidfds, pinned)
        report.update(exitCode=child.returncode, cleanup=cleanup, observedIdentities=[list(key) for key in sorted(observed)])
        app = private_json(output / f'{mode}-app.json')
        report['app'] = app
        if (child.returncode == 0 and not interrupted.is_set() and not report['timedOut'] and report['identityPinFailures'] == 0
                and cleanup['observedCleanupComplete'] and not cleanup['rootWasRunningAtCleanup']
                and not cleanup['remainingBeforeExternalCleanup'] and valid(app, mode, child.pid)):
            report['status'] = 'passed'
    except (OSError, ValueError, RuntimeError, KeyError, TypeError) as error:
        report['errorType'] = type(error).__name__
    finally:
        if cleanup is None:
            cleanup = await OWNED.cleanup_owned(child, pidfds, pinned)
            report.update(exitCode=child.returncode, cleanup=cleanup)
        for fd in pidfds.values():
            os.close(fd)
        OWNED.private_json(output / f'{mode}-owned.json', report)
    return report


async def run(options):
    os.umask(0o077)
    requested = Path(options.output).absolute()
    if not requested.name.startswith('PUBLIC-draft-restart-'):
        raise ValueError('output basename must start PUBLIC-draft-restart-')
    output = OWNED.NATIVE.prepare(requested).resolve()
    summary = {'status': 'failed', 'scope': __doc__, 'phases': [], 'backendIndependentZeroPromptsQualified': False,
               'networkTraceQualified': False, 'physicalInputQualified': False, 'crashDurabilityQualified': False,
               'sourceClosureComplete': False, 'artifactDeploymentQualified': False}
    interrupted = asyncio.Event()
    loop = asyncio.get_running_loop()
    for sig in [signal.SIGINT, signal.SIGTERM]:
        loop.add_signal_handler(sig, interrupted.set)
    try:
        source_before = sources()
        summary['sourceSha256Before'] = source_before
        original = Path(options.binary).resolve(strict=True)
        binary = output / 'draft_composed_smoke'
        with os.fdopen(os.open(binary, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o700), 'wb') as stream:
            stream.write(original.read_bytes())
            stream.flush()
            os.fsync(stream.fileno())
        summary['artifactSha256'] = digest(binary)
        if digest(original) != summary['artifactSha256']:
            raise ValueError('binary-changed-during-copy')
        runtime = (BASE.parent / 'deepseek-harness-linux/apps/cli').resolve(strict=True)
        deadline = time.monotonic() + 170
        for mode in ['write', 'recover']:
            report = await phase(output, binary, runtime, mode, options.scale, interrupted, min(deadline, time.monotonic() + 80))
            summary['phases'].append(report)
            if report['status'] != 'passed':
                raise ValueError('owned-phase-not-qualified')
        first, second = [report['app'] for report in summary['phases']]
        manifest = private_json(output / 'session-manifest.json')
        document = private_json(output / 'harness/native-drafts.v1.json')
        if not valid_restart(first, second, manifest, document):
            raise ValueError('cold-process-or-independent-durable-body-mismatch')
        source_after = sources()
        summary.update(fullProcessRestartObserved=True, independentlyReadPUBLICDraftMatched=True,
                       sourceSha256After=source_after, sourceUnchanged=source_after == source_before,
                       artifactUnchanged=digest(binary) == summary['artifactSha256'], interrupted=interrupted.is_set())
        if not summary['sourceUnchanged'] or not summary['artifactUnchanged']:
            raise ValueError('source-or-artifact-drift')
        summary['status'] = 'passed'
    except (OSError, ValueError, RuntimeError, KeyError, TypeError) as error:
        summary['errorType'] = type(error).__name__
    finally:
        try:
            summary['sourceSha256After'] = sources()
            summary['sourceUnchanged'] = summary['sourceSha256After'] == summary.get('sourceSha256Before')
            snapshot = output / 'draft_composed_smoke'
            if snapshot.is_file():
                summary['artifactSha256After'] = digest(snapshot)
                summary['artifactUnchanged'] = summary['artifactSha256After'] == summary.get('artifactSha256')
            if not summary['sourceUnchanged'] or summary.get('artifactUnchanged') is not True:
                summary['status'] = 'failed'
        except OSError:
            summary['status'] = 'failed'
            summary['sourceOrArtifactRecheckFailed'] = True
        for sig in [signal.SIGINT, signal.SIGTERM]:
            loop.remove_signal_handler(sig)
        OWNED.private_json(output / 'draft-restart-qualification.json', summary)
    print(json.dumps({'status': summary['status'], 'report': str(output / 'draft-restart-qualification.json')}))
    return 0 if summary['status'] == 'passed' else 1


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True)
    parser.add_argument('--binary', default=str(BASE / 'app/target/debug/examples/draft_composed_smoke'))
    parser.add_argument('--scale', type=float, choices=[1.0, 1.25], default=1.0)
    raise SystemExit(asyncio.run(run(parser.parse_args())))
