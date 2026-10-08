#!/usr/bin/python3
"""Own a relocated development launcher→native→isolated Host keyless lifecycle."""
import argparse
import asyncio
import importlib.util
import json
import os
from pathlib import Path
import signal
import statistics
import subprocess
import sys
import time

BASE = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('package_owner_registry', BASE / 'scripts/qualify-composed-registry.py')
REG = importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(REG)
SPEC = importlib.util.spec_from_file_location('package_launcher', BASE / 'scripts/development-launcher.py')
L = importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(L)
APP_ID = 'ai.deepseek.harness.native.app'


def valid_app(app, pid, scale):
    if not isinstance(app, dict): return False
    return (app.get('status') == 'passed' and app.get('pid') == pid
            and app.get('applicationId') == APP_ID and app.get('applicationScale') == scale
            and app.get('windowBackend') == 'wayland' and app.get('modelPrompts') == 0
            and app.get('modelCatalogRequested') is False
            and app.get('coreReady') is True and app.get('workspaceOpened') is True
            and app.get('sessionCreated') is True and app.get('workspaceCount') == 1
            and app.get('sessionCount') == 1 and app.get('snapshotFoldValid') is True
            and type(app.get('snapshotCursor')) is int and app['snapshotCursor'] >= 0
            and app.get('screenshotRequested') is True and app.get('screenshotSaved') is True
            and app.get('stopError') is False and app.get('deadlineReached') is True
            and app.get('stop') == dict(exited=True, graceful=True, containmentUnknown=False,
                                      observedDescendantsRemaining=0, exitCode=0))


def capture_valid(evidence, app):
    data = REG.private_regular(evidence / 'own-window.png', 16 * 1024 * 1024)
    if data is None: return False
    dimensions = REG.png_dimensions(data)
    return dimensions is not None and isinstance(app, dict) and app.get('screenshotPhysicalSize') == list(dimensions)


async def run(options):
    started = time.monotonic()
    package = Path(options.package).resolve(strict=True)
    manifest, binary = L.package(package)
    before = L.binary_digest(binary)
    output = REG.NATIVE.prepare(options.output)
    evidence = output / 'evidence'; evidence.mkdir(mode=0o700)
    data = output / 'native-container'
    environment = REG.NATIVE.environment(output)
    environment['PATH'] = '/usr/bin:/bin'
    node_value = getattr(options, 'node', None)
    node = L.selected_node(node_value)
    node_version = L.probe_node(environment) if node_value is None else L.probe_node(environment, node=node)
    node_match = None
    if node_value is not None:
        # Load only after this module is initialized: the existing direct qualifier
        # imports our shared App/capture validators, but its run is not executed.
        spec = importlib.util.spec_from_file_location('package_explicit_node_observer', BASE / 'scripts/qualify-explicit-node.py')
        observer = importlib.util.module_from_spec(spec); spec.loader.exec_module(observer)
        node_match = observer.node_image_matches
    runtime = (BASE.parent / 'deepseek-harness-linux/apps/cli').resolve(strict=True)
    command = [str(package / 'launch.py'), '--runtime', str(runtime), '--workspace', str(output / 'workspace'),
               '--user-home', str(output / 'user'), '--data-home', str(data), '--qualify-evidence', str(evidence),
               '--scale', str(options.scale)]
    if node_value is not None:
        command += ['--node', str(node)]
    loop = asyncio.get_running_loop(); interrupted = asyncio.Event(); handlers = []
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            loop.add_signal_handler(sig, interrupted.set); handlers.append(sig)
        child = subprocess.Popen(command, env=environment, stdout=subprocess.DEVNULL,
                                 stderr=subprocess.DEVNULL, start_new_session=True)
    except BaseException:
        for sig in handlers: loop.remove_signal_handler(sig)
        raise
    pidfds, observed, samples = {}, set(), []
    facts = None; failure = None; mapped_ms = None; root_pinned = forced = False
    observation_failures = 0
    node_identities = set()
    try:
        try: root, fd = REG.pin_root(child)
        except (OSError, RuntimeError): failure, forced = 'root-pin-failed', True
        else:
            root_pinned = True; pidfds[REG.identity(root)] = fd; observed.add(REG.identity(root))
            deadline = time.monotonic() + 45
            while child.poll() is None:
                observation_failures += REG.observe_descendants(child, root, pidfds, observed)
                if node_match is not None:
                    for key in pidfds:
                        if key != REG.identity(root) and node_match(key, node):
                            node_identities.add(key)
                remaining = deadline - time.monotonic()
                if interrupted.is_set() or remaining <= 0: forced = True; break
                sample = REG.NATIVE.METRICS.sample_tree(child.pid, root['start_ticks'])
                sample['elapsedMs'] = (sample['at'] - started) * 1000
                if samples: sample['cpuPercentOneCore'] = REG.NATIVE.METRICS.cpu_percent(samples[-1], sample)
                samples.append(sample)
                if facts is None:
                    candidate = await REG.native_window(child.pid, environment, min(REG.WINDOW_PROBE_SECONDS, remaining))
                    if candidate and candidate.get('mapped'):
                        facts = candidate; mapped_ms = (time.monotonic() - started) * 1000
                await asyncio.sleep(min(.2, max(0, deadline - time.monotonic())))
    except (OSError, RuntimeError, ValueError): failure, forced = 'owned-observation-failed', True
    finally:
        task = asyncio.create_task(REG.cleanup_owned(child, pidfds, root_pinned))
        try: cleanup = await asyncio.shield(task)
        except asyncio.CancelledError:
            cleanup = await task; failure, forced = 'qualification-cancelled', True
        finally:
            for fd in pidfds.values(): os.close(fd)
            for sig in handlers: loop.remove_signal_handler(sig)
    forced = forced or cleanup['rootWasRunningAtCleanup']
    app = REG.read_app(evidence)
    valid = valid_app(app, child.pid, options.scale)
    frame = capture_valid(evidence, app)
    unchanged = L.binary_digest(binary) == before
    marker_valid = False; lock_released = False
    try:
        marker = L.marker_for(manifest)
        marker_valid = L.inspect_state(data, marker) == 'owned'
        fd = L.acquire_state(data, marker); os.close(fd); lock_released = True
    except (OSError, L.Refused, ValueError): pass
    passed = (valid and frame and unchanged and marker_valid and lock_released and child.returncode == 0
              and (node_value is None or bool(node_identities))
              and failure is None and not forced and observation_failures == 0
              and cleanup['observedCleanupComplete'] and not cleanup['remainingBeforeExternalCleanup']
              and facts and facts.get('class') == APP_ID and facts.get('xwayland') is False)
    warm = [s for s in samples if 3000 <= s['elapsedMs'] <= 6500 and s['process_count'] > 0]
    def median(key):
        values = [s[key] for s in warm if s.get(key) is not None]
        return statistics.median(values) if values else None
    report = dict(status='passed' if passed else 'failed', scope=__doc__, nativeSha256=before,
                  packageRuntimeMode='external-explicit', runtimeBundled=False,
                  nodeVersionObserved=node_version, nodeSelectionMode='system-default' if node_value is None else 'explicit-absolute',
                  explicitNodeExecutablePathAndInodeMatched=bool(node_identities) if node_value is not None else None,
                  launcherExecPidMatchesNativeWindow=bool(facts and facts.get('pid') == child.pid),
                  binaryUnchanged=unchanged, failureCode=failure, forced=forced, exitCode=child.returncode,
                  ownedWindow=facts, observedOwnedProcessCount=len(observed), ownershipObservationFailures=observation_failures,
                  cleanup=cleanup, app=app, appEvidenceVerified=valid, completeOwnRendererCaptureVerified=frame,
                  nativeContainerAndHostPrivateMarkerVerified=marker_valid, inheritedLockReleasedAfterOwnedStop=lock_released,
                  firstOwnWindowMappedUpperBoundMs=mapped_ms, fullTreeWarmMedianRssKiB=median('rss_kib'),
                  fullTreeWarmMedianPssKiB=median('pss_kib'), fullTreeWarmMedianCpuPercentOneCore=median('cpuPercentOneCore'),
                  warmSamples=len(warm), performanceScope='single fresh blank-profile eight-second integrated smoke; sampling overhead, not representative long-chat benchmark',
                  elapsedWallSeconds=time.monotonic() - started, hardTotalWallClockBoundClaimed=False,
                  containmentScope='observed pidfd-pinned identities only', primaryExecutableEntrypointQualified=bool(passed),
                  physicalNativeInputQualified=False, actualModelCalls=0, realCredentialsUsed=False,
                  installedRuntimeDataOrDesktopSettingsChanged=False, paintInspected=False,
                  standaloneInstallerOrRuntimeRelocationQualified=False, fullDesktopParity=False)
    if node_value is None:
        report.update(systemNodeQualified=node_version.removeprefix('v'), systemNodeVersionObserved=node_version)
    REG.private_json(output / 'result.json', report)
    print(json.dumps({'status': report['status'], 'report': str(output / 'result.json')}))
    return 0 if passed else 1


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument('--package', required=True); parser.add_argument('--output', required=True)
    parser.add_argument('--node', help='Explicit absolute Node executable to pass through extracted launcher')
    parser.add_argument('--scale', choices=[1.0, 1.25], type=float, default=1.0)
    try: sys.exit(asyncio.run(run(parser.parse_args())))
    except (OSError, RuntimeError, ValueError, L.Refused, subprocess.SubprocessError):
        print('development-package-qualification-failed', file=sys.stderr); sys.exit(1)
