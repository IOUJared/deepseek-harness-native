import argparse
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import stat
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('dev_launcher', Path(__file__).with_name('development-launcher.py'))
L = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(L)


class LauncherTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='PUBLIC-native-launcher-')
        self.root = Path(self.temp.name).resolve()
        self.assertEqual(self.root.parent, Path(tempfile.gettempdir()).resolve())
        self.addCleanup(self.temp.cleanup)
        self.pkg = self.root / 'package with spaces'
        (self.pkg / 'bin').mkdir(parents=True, mode=0o700)
        binary = self.pkg / 'bin/dsh-native-app'
        binary.write_bytes(b'PUBLIC-NATIVE-TEST-NOT-A-RUNTIME')
        binary.chmod(0o700)
        self.manifest = dict(application=L.APP, runtimeVersion=L.VERSION, architecture='x86_64',
                             runtimeMode='external-explicit', minimumGlibc='2.44', nativeSha256=L.hashlib.sha256(binary.read_bytes()).hexdigest())
        (self.pkg / 'development-manifest.json').write_text(json.dumps(self.manifest))
        self.runtime = self.root / 'explicit alpha runtime'
        (self.runtime / 'lib').mkdir(parents=True)
        (self.runtime / 'package.json').write_text(json.dumps(dict(name='@deepseek-ai/dsh', version=L.VERSION)))
        (self.runtime / 'lib/bin.js').write_text('// PUBLIC fixture only\n')
        self.user = self.root / 'PUBLIC-user'
        self.workspace = self.root / 'PUBLIC-workspace'
        self.user.mkdir(mode=0o700); self.workspace.mkdir(mode=0o700)
        self.data = self.root / 'PUBLIC-new-native-data'
        self.env = dict(HOME=str(self.user), WAYLAND_DISPLAY='PUBLIC-wayland', XDG_RUNTIME_DIR='/run/user/PUBLIC',
                        API_KEY='PRIVATE-NEVER-FORWARD', NODE_OPTIONS='PRIVATE-NEVER-FORWARD',
                        LD_PRELOAD='PRIVATE-NEVER-FORWARD', DSH_LAUNCH_TOKEN='PRIVATE-NEVER-FORWARD')
        self.version = patch.object(L.subprocess, 'run', return_value=L.subprocess.CompletedProcess([], 0, b'v26.10.0\n', b''))
        self.version.start(); self.addCleanup(self.version.stop)
        self.libc = patch.object(L.os, 'confstr', return_value='glibc 2.44')
        self.libc.start(); self.addCleanup(self.libc.stop)

    def args(self, **changes):
        value = dict(workspace=str(self.workspace), runtime=str(self.runtime), user_home=str(self.user),
                     data_home=str(self.data), scale=1.0, check=True, qualify_evidence=None)
        value.update(changes)
        return argparse.Namespace(**value)

    def test_check_plans_exact_private_args_without_creating_data(self):
        argv, env, data, marker, status = L.plan(self.args(), self.pkg, self.env)
        self.assertEqual(status, 'new'); self.assertFalse(data.exists())
        self.assertEqual(argv[0], str(self.pkg / 'bin/dsh-native-app'))
        self.assertEqual(argv[argv.index('--runtime') + 1], str(self.runtime))
        self.assertEqual(argv[argv.index('--home') + 1], str(self.data / 'host'))
        self.assertNotIn('--smoke-new-session', argv)
        self.assertNotIn('PRIVATE-NEVER-FORWARD', json.dumps(env))
        self.assertEqual(env['PATH'], '/usr/bin:/bin')

    def test_node_version_parser_accepts_actual_standard_patch_versions(self):
        for raw in [b'v26.0.0',b'v26.10.0\n',b'v26.12.1\n',b'v26.9999.9999\n']:
            result=L.subprocess.CompletedProcess([],0,raw,b'')
            self.assertEqual(L.parse_node_version(result),raw.removesuffix(b'\n').decode('ascii'))

    def test_node_version_parser_refuses_malformed_failed_or_excess_output(self):
        malformed=[b'',b'v26.',b'v26.not-a-version',b'v26.1',b'v026.1.0',b'v25.1.0',b'v27.1.0',
                   b'v26.1.0\nPRIVATE-output',b'v26.1.0\n\n',b' v26.1.0\n',b'v26.1.0 \n',
                   b'v26.1.0\r\n',b'v26.1.0\x00',b'v26.1.0-custom',b'v26.1.0\xff',b'v26.12345.0',
                   b'x'*33,'v26.1.0\n',None]
        for raw in malformed:
            with self.subTest(raw=raw),self.assertRaises(L.Refused):L.parse_node_version(L.subprocess.CompletedProcess([],0,raw,b''))
        for code,err in [(1,b''),(-15,b''),(False,b''),(True,b''),(None,b''),(0,b'x'*1025),(0,None),(0,'private')]:
            with self.subTest(code=code,stderr=err),self.assertRaises(L.Refused):L.parse_node_version(L.subprocess.CompletedProcess([],code,b'v26.1.0\n',err))

    def test_plan_observations_use_actual_patch_and_exact_scrubbed_probe(self):
        for raw in [b'v26.10.0\n',b'v26.12.1\n']:
            observations={}
            with patch.object(L.subprocess,'run',return_value=L.subprocess.CompletedProcess([],0,raw,b'')) as probe:
                L.plan(self.args(),self.pkg,self.env,observations=observations)
            self.assertEqual(observations,dict(systemNodeVersionObserved=raw.removesuffix(b'\n').decode('ascii'),nodeVersionObserved=raw.removesuffix(b'\n').decode('ascii'),nodeMajorAccepted=26,nodeSelectionMode='system-default'))
            probe.assert_called_once_with(['/usr/bin/node','--version'],env=L.environment(self.user,self.env),cwd=self.pkg,capture_output=True,timeout=5,check=False)
            self.assertFalse(self.data.exists())

    def test_malformed_node_refuses_before_state_or_exec_and_redacts_output(self):
        tokens=['--workspace',str(self.workspace),'--runtime',str(self.runtime),'--user-home',str(self.user),'--data-home',str(self.data)]
        for failure in [L.subprocess.CompletedProcess([],0,b'v26.PRIVATE-output',b'PRIVATE-stderr'),L.subprocess.TimeoutExpired('PRIVATE-command',5,output=b'PRIVATE-output')]:
            output=io.StringIO()
            options={'side_effect':failure} if isinstance(failure,Exception) else {'return_value':failure}
            with patch.dict(os.environ,self.env,clear=True),patch.object(L.subprocess,'run',**options),patch.object(L.os,'execve') as execute,contextlib.redirect_stderr(output):
                self.assertEqual(L.main(tokens,self.pkg),2)
            self.assertNotIn('PRIVATE',output.getvalue());execute.assert_not_called();self.assertFalse(self.data.exists())

    def test_explicit_node_spaces_unicode_is_probed_and_forwarded_without_path_metadata(self):
        node = self.root / 'PUBLIC node copy · 世界'
        node.write_bytes(b'PUBLIC fake never executed'); node.chmod(0o700)
        observations = {}
        with patch.object(L.subprocess, 'run', return_value=L.subprocess.CompletedProcess([], 0, b'v26.10.0\n', b'')) as probe:
            argv, env, data, marker, status = L.plan(self.args(node=str(node)), self.pkg, self.env, observations=observations)
        probe.assert_called_once_with([str(node), '--version'], env=L.environment(self.user, self.env), cwd=self.pkg, capture_output=True, timeout=5, check=False)
        self.assertEqual(argv[argv.index('--node') + 1], str(node))
        self.assertEqual(observations, dict(nodeVersionObserved='v26.10.0', nodeMajorAccepted=26, nodeSelectionMode='explicit-absolute'))
        self.assertNotIn(str(node), json.dumps(observations)); self.assertNotIn('systemNodeVersionObserved', observations)
        self.assertEqual(env['PATH'], '/usr/bin:/bin'); self.assertFalse(data.exists())

    def test_node_default_is_fixed_and_explicit_symlink_resolves_to_same_selected_file(self):
        self.assertEqual(L.selected_node(), Path('/usr/bin/node'))
        argv, *_ = L.plan(self.args(), self.pkg, dict(self.env, PATH='/PUBLIC unsafe PATH', DSH_NODE='/PUBLIC ignored'))
        self.assertNotIn('--node', argv)
        node = self.root / 'PUBLIC selected node'; node.write_bytes(b'PUBLIC'); node.chmod(0o700)
        link = self.root / 'PUBLIC node alias'; link.symlink_to(node)
        argv, *_ = L.plan(self.args(node=str(link)), self.pkg, self.env)
        self.assertEqual(argv[argv.index('--node') + 1], str(node))

    def test_invalid_explicit_node_never_probes_or_falls_back(self):
        nonexec = self.root / 'PUBLIC nonexecuting node'; nonexec.write_bytes(b'PUBLIC'); nonexec.chmod(0o600)
        candidates = ['', 'PUBLIC relative', str(self.root/'..'/'PUBLIC traversal'), str(self.root/'PUBLIC missing'), str(self.root), str(nonexec), str(self.root/'PUBLIC\ncontrol')]
        for value in candidates:
            with patch.object(L.subprocess, 'run') as probe:
                with self.assertRaises((L.Refused, OSError)): L.plan(self.args(node=value), self.pkg, self.env)
                probe.assert_not_called(); self.assertFalse(self.data.exists())

    def test_clean_alias_to_control_or_nonutf8_target_refuses_before_probe(self):
        for index, name in enumerate(['PUBLIC newline\nnode', 'PUBLIC del\x7fnode', os.fsdecode(b'PUBLIC nonutf8\xffnode')]):
            target = self.root / name; target.write_bytes(b'PUBLIC'); target.chmod(0o700)
            alias = self.root / ('PUBLIC clean alias' + str(index)); alias.symlink_to(target)
            with patch.object(L.subprocess, 'run') as probe:
                with self.assertRaises((L.Refused, ValueError)): L.plan(self.args(node=str(alias)), self.pkg, self.env)
                probe.assert_not_called(); self.assertFalse(self.data.exists())

    def test_bad_explicit_version_refuses_before_state_exec_and_redacts_node_path(self):
        node = self.root / 'PRIVATE selected node'; node.write_bytes(b'PUBLIC'); node.chmod(0o700)
        tokens = ['--workspace', str(self.workspace), '--runtime', str(self.runtime), '--user-home', str(self.user), '--data-home', str(self.data), '--node', str(node)]
        output = io.StringIO()
        with patch.dict(os.environ, self.env, clear=True), patch.object(L.subprocess, 'run', return_value=L.subprocess.CompletedProcess([], 0, b'v25.1.0\n', b'PRIVATE output')), patch.object(L.os, 'execve') as execute, contextlib.redirect_stderr(output):
            self.assertEqual(L.main(tokens, self.pkg), 2)
        self.assertNotIn('PRIVATE', output.getvalue()); self.assertNotIn(str(node), output.getvalue())
        execute.assert_not_called(); self.assertFalse(self.data.exists())

    def test_duplicate_or_missing_node_option_refuses_before_probe(self):
        tokens = ['--workspace', str(self.workspace), '--runtime', str(self.runtime)]
        for suffix in [['--node'], ['--node', '/PUBLIC first', '--node', '/PUBLIC second']]:
            with patch.object(L.subprocess, 'run') as probe, contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit): L.main(tokens + suffix, self.pkg)
                probe.assert_not_called(); self.assertFalse(self.data.exists())

    def test_marker_private_host_and_nonblocking_lock_ownership(self):
        marker = L.marker_for(self.manifest)
        fd = L.acquire_state(self.data, marker)
        try:
            self.assertTrue(os.get_inheritable(fd))
            self.assertEqual(stat.S_IMODE(self.data.stat().st_mode), 0o700)
            self.assertEqual(stat.S_IMODE((self.data / 'host').stat().st_mode), 0o700)
            self.assertEqual(stat.S_IMODE((self.data / '.native-development.json').stat().st_mode), 0o600)
            self.assertEqual(L.inspect_state(self.data, marker), 'owned')
            with self.assertRaises(L.Refused): L.acquire_state(self.data, marker)
        finally: os.close(fd)
        fd = L.acquire_state(self.data, marker); os.close(fd)

    def test_existing_unknown_private_directory_is_not_adopted(self):
        self.data.mkdir(mode=0o700)
        (self.data / 'PUBLIC-preserve').write_bytes(b'PUBLIC-existing')
        with self.assertRaises((L.Refused, OSError)): L.plan(self.args(), self.pkg, self.env)
        self.assertEqual((self.data / 'PUBLIC-preserve').read_bytes(), b'PUBLIC-existing')
        self.assertFalse((self.data / '.native-development.json').exists())

    def test_symlink_data_and_host_are_rejected_not_followed(self):
        other = self.root / 'PUBLIC-unrelated'; other.mkdir(mode=0o700)
        self.data.symlink_to(other, target_is_directory=True)
        with self.assertRaises(L.Refused): L.plan(self.args(), self.pkg, self.env)
        self.assertEqual(list(other.iterdir()), [])
        self.assertEqual(self.data.parent.resolve(), self.root)
        self.assertEqual(Path(os.readlink(self.data)).resolve(), other.resolve())
        self.data.unlink()  # Delete only the verified owned link, never its target.
        self.data.mkdir(mode=0o700)
        path = self.data / '.native-development.json'; path.write_text(json.dumps(L.marker_for(self.manifest))); path.chmod(0o600)
        (self.data / 'host').symlink_to(other, target_is_directory=True)
        with self.assertRaises(L.Refused): L.plan(self.args(), self.pkg, self.env)
        with self.assertRaises((L.Refused, OSError)): L.acquire_state(self.data, L.marker_for(self.manifest))
        self.assertEqual(list(other.iterdir()), [])

    def test_changed_build_marker_refuses_migration(self):
        fd = L.acquire_state(self.data, L.marker_for(self.manifest)); os.close(fd)
        marker = L.marker_for(self.manifest); marker['nativeSha256'] = 'f' * 64
        with self.assertRaises(L.Refused): L.inspect_state(self.data, marker)

    def test_default_data_is_sha_qualified_not_installed_home(self):
        (self.user / '.local/share').mkdir(parents=True)
        argv, _, data, _, _ = L.plan(self.args(data_home=None), self.pkg, self.env)
        self.assertEqual(data, self.user / '.local/share' / ('dsh-native-dev-' + self.manifest['nativeSha256'][:12]))
        self.assertNotEqual(data, self.user / '.dsh')
        inherited = dict(self.env, DSH_HOME=str(data))
        with self.assertRaises(L.Refused): L.plan(self.args(data_home=None), self.pkg, inherited)

    def test_protected_paths_and_parent_traversal_refuse(self):
        for target in (self.user, self.user / '.dsh/new', self.pkg / 'data', self.runtime / 'data', Path('/'), self.root / 'a/../b'):
            with self.subTest(target=target):
                with self.assertRaises((L.Refused, OSError)): L.plan(self.args(data_home=str(target)), self.pkg, self.env)

    def test_malformed_json_objects_fail_closed(self):
        for value in ([], None, 7, 'PUBLIC'):
            (self.pkg / 'development-manifest.json').write_text(json.dumps(value))
            with self.assertRaises(L.Refused): L.package(self.pkg)
        (self.pkg / 'development-manifest.json').write_text(json.dumps(self.manifest))
        (self.runtime / 'package.json').write_text('[]')
        with self.assertRaises(L.Refused): L.plan(self.args(), self.pkg, self.env)

    def test_changed_binary_and_rc_runtime_refuse(self):
        (self.runtime / 'package.json').write_text(json.dumps(dict(name='@deepseek-ai/dsh', version='0.2.0-rc.2')))
        with self.assertRaises(L.Refused): L.plan(self.args(), self.pkg, self.env)
        (self.pkg / 'bin/dsh-native-app').write_bytes(b'PUBLIC-changed')
        with self.assertRaises(L.Refused): L.package(self.pkg)

    def test_linux_architecture_node_and_scale_gates(self):
        with patch.object(L.sys, 'platform', 'darwin'):
            with self.assertRaises(L.Refused): L.package(self.pkg)
        with patch.object(L.platform, 'machine', return_value='arm64'):
            with self.assertRaises(L.Refused): L.package(self.pkg)
        with patch.object(L.os, 'confstr', return_value='glibc 2.43'):
            with self.assertRaises(L.Refused): L.package(self.pkg)
        for scale in (float('nan'), float('inf'), 0.5, 3.0):
            with self.assertRaises(L.Refused): L.plan(self.args(scale=scale), self.pkg, self.env)
        with patch.object(L.subprocess, 'run', return_value=L.subprocess.CompletedProcess([], 0, b'v24.1.0\n', b'')):
            with self.assertRaises(L.Refused): L.plan(self.args(), self.pkg, self.env)

    def test_check_no_display_launch_requires_wayland(self):
        L.plan(self.args(), self.pkg, {'HOME': str(self.user)})
        with self.assertRaises(L.Refused): L.plan(self.args(check=False), self.pkg, {'HOME': str(self.user)})

    def test_no_abbreviated_duplicate_or_hidden_passthrough_options(self):
        for tokens in (['--workspace', str(self.workspace), '--worksp', str(self.workspace)],
                       ['--workspace', str(self.workspace), '--workspace', str(self.workspace)],
                       ['--workspace', str(self.workspace), '--runtime-override', str(self.runtime)]):
            with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit): L.main(tokens, self.pkg)

    def test_main_check_output_is_prerequisites_only_and_no_exec(self):
        tokens = ['--workspace', str(self.workspace), '--runtime', str(self.runtime), '--data-home', str(self.data), '--check']
        output = io.StringIO()
        with patch.dict(os.environ, self.env, clear=True), patch.object(L.os, 'execve') as execute, contextlib.redirect_stdout(output):
            self.assertEqual(L.main(tokens, self.pkg), 0)
        report = json.loads(output.getvalue())
        self.assertFalse(report['runtimeStartupValidated']); self.assertFalse(report['nativeDataCreated'])
        self.assertEqual(report['status'], 'prerequisites-valid'); self.assertFalse(self.data.exists())
        self.assertEqual(report['systemNodeVersionObserved'],'v26.10.0')
        self.assertEqual(report['nodeMajorAccepted'],26)
        execute.assert_not_called()

    def test_failed_exec_releases_lock_without_removing_private_data(self):
        tokens = ['--workspace', str(self.workspace), '--runtime', str(self.runtime),
                  '--user-home', str(self.user), '--data-home', str(self.data)]
        output = io.StringIO()
        with patch.dict(os.environ, self.env, clear=True), patch.object(L.os, 'execve', side_effect=OSError('PRIVATE-sentinel')), contextlib.redirect_stderr(output):
            self.assertEqual(L.main(tokens, self.pkg), 2)
        self.assertNotIn('PRIVATE-sentinel', output.getvalue())
        self.assertTrue((self.data / '.native-development.json').exists())
        fd = L.acquire_state(self.data, L.marker_for(self.manifest)); os.close(fd)

    def test_explicit_public_qualification_is_only_for_fresh_empty_private_dirs(self):
        evidence = self.root / 'PUBLIC-evidence'; evidence.mkdir(mode=0o700)
        argv, _, _, _, _ = L.plan(self.args(check=False, qualify_evidence=str(evidence)), self.pkg, self.env)
        self.assertIn('--smoke-evidence-root', argv); self.assertIn('--smoke-new-session', argv)
        self.assertFalse(self.data.exists()); self.assertEqual(list(evidence.iterdir()), [])
        with self.assertRaises(L.Refused): L.plan(self.args(qualify_evidence=str(evidence)), self.pkg, self.env)
        (self.workspace / 'PUBLIC-not-empty').write_bytes(b'PUBLIC')
        with self.assertRaises(L.Refused): L.plan(self.args(check=False, qualify_evidence=str(evidence)), self.pkg, self.env)


if __name__ == '__main__':
    unittest.main()
