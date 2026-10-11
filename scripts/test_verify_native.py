"""Verification integration guards; no GUI, provider, toolbox or profile startup."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("native_verification", Path(__file__).with_name("verify-native.py"))
VERIFY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFY)


class VerificationTests(unittest.TestCase):
    def test_plan_executes_existing_examples_not_gui_main(self):
        checks = VERIFY.commands("examples")
        self.assertEqual([name for name, _ in checks], ["layout_smoke", "scroll_app_smoke"])
        for _, command in checks:
            self.assertEqual(command[:2], ["cargo", "test"])
            self.assertIn("--locked", command)
            self.assertIn("--offline", command)
            self.assertIn("--example", command)
            self.assertNotIn("run", command)

    def test_clippy_does_not_hide_or_deny_all_baseline_warnings(self):
        checks = VERIFY.commands("clippy")
        self.assertEqual(len(checks), 3)
        for _, command in checks:
            self.assertIn("clippy::todo", command)
            self.assertIn("clippy::unimplemented", command)
            self.assertIn("unsafe_op_in_unsafe_fn", command)
            self.assertNotIn("warnings", command)
            self.assertNotIn("-A", command)

    def test_all_keeps_every_lint_and_test_owner(self):
        self.assertEqual(VERIFY.commands("all"), VERIFY.commands("lint") + VERIFY.commands("test"))
        tests = [name for name, _ in VERIFY.commands("test")]
        for name in ("core", "core-transport", "transport", "app", "app-public", "wayland-drops", "layout_smoke", "scroll_app_smoke", "bridge"):
            self.assertIn(name, tests)

    def test_wayland_drop_owner_is_executed_locally_and_in_ci(self):
        command = dict(VERIFY.commands("test"))["wayland-drops"]
        self.assertIn("--locked", command)
        self.assertIn("--offline", command)
        self.assertEqual(command[command.index("-p") + 1], "winit")
        self.assertIn("data_device", command)
        workflow = (VERIFY.ROOT / ".github/workflows/native.yml").read_text()
        self.assertIn("-p winit --lib data_device -- --test-threads=2", workflow)

    def test_online_only_removes_offline_flag(self):
        for suite in ("clippy", "examples", "test"):
            self.assertEqual([(n, [a for a in c if a != "--offline"]) for n, c in VERIFY.commands(suite)],
                             VERIFY.commands(suite, offline=False))

    def test_invalid_suite_and_bounds_reject(self):
        with self.assertRaises(ValueError):
            VERIFY.commands("live-model")
        for argv in (["--timeout", "0"], ["--timeout", "3601"], ["--cargo-home", "relative-cache"]):
            result = subprocess.run([sys.executable, "-B", str(VERIFY.ROOT / "scripts/verify-native.py"), *argv, "--plan"], capture_output=True)
            self.assertEqual(result.returncode, 2)

    def test_plan_creates_no_report_or_execution(self):
        result = subprocess.run([sys.executable, "-B", str(VERIFY.ROOT / "scripts/verify-native.py"), "--suite", "examples", "--plan"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0)
        self.assertEqual(len(json.loads(result.stdout)["checks"]), 2)

    def test_failed_attempt_retains_exit_and_private_log(self):
        with tempfile.TemporaryDirectory() as directory:
            result = VERIFY.run_check("failure", [sys.executable, "-c", "print('counterexample');raise SystemExit(7)"], Path(directory), os.environ.copy(), 5)
            self.assertEqual(result["status"], "failed")
            self.assertEqual(result["exitCode"], 7)
            log = Path(directory) / "failure.log"
            self.assertIn("counterexample", log.read_text())
            self.assertEqual(log.stat().st_mode & 0o777, 0o600)

    def test_timeout_is_not_reported_as_a_successful_exit(self):
        with tempfile.TemporaryDirectory() as directory:
            result = VERIFY.run_check("timeout", [sys.executable, "-c", "import threading;threading.Event().wait()"], Path(directory), os.environ.copy(), 0.05)
            self.assertEqual(result["status"], "timed-out")
            self.assertTrue(result["timedOut"])
            self.assertEqual(result["signal"], 9)
            self.assertEqual(result["exitCode"], -9)

    def test_missing_command_is_unavailable_not_skipped(self):
        with tempfile.TemporaryDirectory() as directory:
            result = VERIFY.run_check("missing", ["/definitely-absent-impeccable-command"], Path(directory), os.environ.copy(), 1)
            self.assertEqual(result["status"], "unavailable")
            self.assertIsNone(result["exitCode"])
            self.assertFalse(result["timedOut"])

    def test_upstream_bytes_match_pinned_manifest_and_license(self):
        manifest = json.loads((VERIFY.SKILL / "UPSTREAM.json").read_text())
        self.assertEqual(manifest["commit"], "1e85e9ad30d157f9627bbd1ab50107f9938bf3f6")
        self.assertEqual(manifest["license"], "MIT")
        self.assertEqual(len(manifest["files"]), 7)
        for entry in manifest["files"]:
            data = (VERIFY.SKILL / entry["path"]).read_bytes()
            self.assertEqual(hashlib.sha256(data).hexdigest(), entry["sha256"], entry["path"])
            self.assertEqual(hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest(), entry["gitBlob"])
        self.assertIn("MIT License", (VERIFY.SKILL / "LICENSE").read_text())

    def test_skill_entry_is_loadable_without_pruning(self):
        entry = (VERIFY.SKILL / "SKILL.md").read_text()
        self.assertTrue(entry.startswith("---\nname: impeccable-rust\n"))
        self.assertLess(len(entry), 8192)
        self.assertNotIn("disable-model-invocation: true", entry)
        for path in ("references/upstream.md", "performance.md", "scripts/impeccable", "toolbox/tools.txt", "toolbox/Dockerfile"):
            self.assertTrue((VERIFY.SKILL / path).is_file())

    def test_a_stale_release_artifact_fails_the_guard_and_a_rebuilt_one_passes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            source = root / "app/src/ui.rs"
            source.parent.mkdir(parents=True)
            source.write_text("PUBLIC first-party source\n")
            artifact = root / "app/target/release/dsh-native-app"
            artifact.parent.mkdir(parents=True)
            artifact.write_text("PUBLIC binary\n")
            logs = root / "logs"
            logs.mkdir()
            os.utime(artifact, (1_000_000, 1_000_000))
            os.utime(source, (2_000_000, 2_000_000))
            with patch.object(VERIFY, "ROOT", root):
                built, stale = VERIFY.freshness_findings()
                self.assertEqual(built, ["app/target/release/dsh-native-app"])
                self.assertEqual(stale, ["app/target/release/dsh-native-app"])
                result = VERIFY.run_freshness_check(logs)
                self.assertEqual(result["status"], "failed")
                self.assertEqual(result["exitCode"], 1)
                text = Path(result["log"]).read_text()
                self.assertIn("older than their first-party sources", text)
                self.assertIn("cargo build --release", text)
                self.assertEqual(Path(result["log"]).stat().st_mode & 0o777, 0o600)
                # A rebuilt artifact of equal or newer age passes.
                os.utime(artifact, (2_000_000, 2_000_000))
                self.assertEqual(VERIFY.freshness_findings(), (built, []))

    def test_an_absent_release_artifact_is_not_a_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            source = root / "app/src/ui.rs"
            source.parent.mkdir(parents=True)
            source.write_text("PUBLIC first-party source\n")
            logs = root / "logs"
            logs.mkdir()
            with patch.object(VERIFY, "ROOT", root):
                self.assertEqual(VERIFY.freshness_findings(), ([], []))
                result = VERIFY.run_freshness_check(logs)
                self.assertEqual(result["status"], "passed")
                self.assertEqual(result["presentArtifacts"], [])

    def test_freshness_guard_is_planned_for_the_reporting_suites_only(self):
        for suite in VERIFY.FRESHNESS_SUITES:
            result = subprocess.run([sys.executable, "-B", str(VERIFY.ROOT / "scripts/verify-native.py"),
                                     "--suite", suite, "--plan"], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0)
            checks = json.loads(result.stdout)["checks"]
            self.assertEqual(checks[0]["name"], "binary-freshness")
            self.assertEqual(checks[0]["command"], list(VERIFY.FRESHNESS_ARGV))
        result = subprocess.run([sys.executable, "-B", str(VERIFY.ROOT / "scripts/verify-native.py"),
                                 "--suite", "examples", "--plan"], capture_output=True, text=True)
        names = [check["name"] for check in json.loads(result.stdout)["checks"]]
        self.assertNotIn("binary-freshness", names)

    def test_ci_runs_examples_and_selected_lints_via_same_runner(self):
        workflow = (VERIFY.ROOT / ".github/workflows/native.yml").read_text()
        self.assertIn("scripts/verify-native.py --suite examples --online", workflow)
        self.assertIn("scripts/verify-native.py --suite clippy --online", workflow)
        self.assertIn("rustup component add clippy", workflow)


if __name__ == "__main__":
    unittest.main()
