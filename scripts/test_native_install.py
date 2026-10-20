"""Installer guards: real staged install into a temporary DESTDIR; no user state touched."""
from pathlib import Path
import os
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
INSTALL = ROOT / "packaging/install.sh"
DESKTOP = ROOT / "packaging/ai.deepseek.harness.native.app.desktop"


class InstallerTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="native-install-test-"))
        self.binary = self.tmp / "dsh-native-app"
        self.binary.write_text("#!/bin/sh\nexit 0\n")
        self.binary.chmod(0o755)
        self.stage = self.tmp / "stage"

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def run_install(self, *args, env=None):
        return subprocess.run(["bash", str(INSTALL), *args], capture_output=True, text=True, env=env)

    def install_env(self):
        env = dict(os.environ)
        env["XDG_CONFIG_HOME"] = str(self.tmp / "config")
        return env

    def write_binary(self, name, marker):
        path = self.tmp / name
        path.write_text(f"#!/bin/sh\necho {marker}\n")
        path.chmod(0o755)
        return path

    def manifest(self, prefix="/usr"):
        return self.staged(f"{prefix}/share/dsh-native/install-manifest")

    def manifest_value(self, key, prefix="/usr"):
        for line in self.manifest(prefix).read_text().splitlines():
            if line.startswith(f"{key}="):
                return line.split("=", 1)[1]
        return None

    def staged(self, relative):
        return self.stage / relative.lstrip("/")

    def test_staged_install_writes_executable_launcher_entry_and_icon(self):
        result = self.run_install("--binary", str(self.binary), "--prefix", "/usr", "--destdir", str(self.stage))
        self.assertEqual(result.returncode, 0, result.stderr)
        for relative, mode in (("usr/bin/dsh-native-app", 0o755), ("usr/bin/dsh-native", 0o755),
                               ("usr/share/applications/ai.deepseek.harness.native.app.desktop", 0o644),
                               ("usr/share/icons/hicolor/scalable/apps/ai.deepseek.harness.native.app.svg", 0o644)):
            path = self.staged(relative)
            self.assertTrue(path.is_file(), relative)
            self.assertEqual(path.stat().st_mode & 0o777, mode, relative)
        syntax = subprocess.run(["bash", "-n", str(self.staged("usr/bin/dsh-native"))], capture_output=True, text=True)
        self.assertEqual(syntax.returncode, 0, syntax.stderr)
        self.assertIn("Exec=dsh-native", self.staged("usr/share/applications/ai.deepseek.harness.native.app.desktop").read_text())

    def test_desktop_entry_validates_when_the_tool_is_installed(self):
        validator = shutil.which("desktop-file-validate")
        if validator is None:
            self.skipTest("desktop-file-validate is not installed")
        result = subprocess.run([validator, str(DESKTOP)], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_install_refuses_missing_or_symlinked_binary(self):
        missing = self.run_install("--binary", str(self.tmp / "absent"), "--prefix", "/usr", "--destdir", str(self.stage))
        self.assertNotEqual(missing.returncode, 0)
        link = self.tmp / "link-app"
        link.symlink_to(self.binary)
        linked = self.run_install("--binary", str(link), "--prefix", "/usr", "--destdir", str(self.stage))
        self.assertNotEqual(linked.returncode, 0)
        not_executable = self.tmp / "plain"
        not_executable.write_text("data")
        refused = self.run_install("--binary", str(not_executable), "--prefix", "/usr", "--destdir", str(self.stage))
        self.assertNotEqual(refused.returncode, 0)

    def test_existing_file_needs_force(self):
        target = self.staged("usr/share/applications/ai.deepseek.harness.native.app.desktop")
        target.parent.mkdir(parents=True)
        target.write_text("[Desktop Entry]\nName=Hand written\n")
        refused = self.run_install("--binary", str(self.binary), "--prefix", "/usr", "--destdir", str(self.stage))
        self.assertNotEqual(refused.returncode, 0)
        self.assertIn("refusing to replace existing", refused.stderr)
        self.assertIn("Hand written", target.read_text())
        forced = self.run_install("--binary", str(self.binary), "--prefix", "/usr", "--destdir", str(self.stage),
                                  "--force")
        self.assertEqual(forced.returncode, 0, forced.stderr)
        self.assertIn("Exec=dsh-native", target.read_text())

    def test_help_prints_only_the_comment_header(self):
        result = self.run_install("--help")
        self.assertEqual(result.returncode, 0)
        self.assertIn("--uninstall", result.stdout)
        self.assertIn("--force", result.stdout)
        self.assertNotIn("set -euo pipefail", result.stdout)

    def test_refuses_root_prefix(self):
        result = self.run_install("--binary", str(self.binary), "--prefix", "/", "--destdir", str(self.stage))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unsafe prefix", result.stderr)

    def test_uninstall_removes_installed_files_and_keeps_configuration(self):
        self.run_install("--binary", str(self.binary), "--prefix", "/usr", "--destdir", str(self.stage))
        removed = self.run_install("--uninstall", "--prefix", "/usr", "--destdir", str(self.stage))
        self.assertEqual(removed.returncode, 0, removed.stderr)
        for relative in ("usr/bin/dsh-native-app", "usr/bin/dsh-native",
                         "usr/share/applications/ai.deepseek.harness.native.app.desktop",
                         "usr/share/icons/hicolor/scalable/apps/ai.deepseek.harness.native.app.svg"):
            self.assertFalse(self.staged(relative).exists(), relative)
        again = self.run_install("--uninstall", "--prefix", "/usr", "--destdir", str(self.stage))
        self.assertEqual(again.returncode, 0, again.stderr)
        self.assertIn("absent", again.stdout)

    def test_uninstall_leaves_a_foreign_symlink_alone(self):
        target = self.staged("usr/bin/dsh-native")
        target.parent.mkdir(parents=True)
        victim = self.tmp / "victim"
        victim.write_text("keep")
        target.symlink_to(victim)
        result = self.run_install("--uninstall", "--prefix", "/usr", "--destdir", str(self.stage))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(target.is_symlink())
        self.assertTrue(victim.exists())


    def test_upgrade_replaces_the_binary_keeps_files_and_records_the_new_version(self):
        first = self.write_binary("app-r1", "v1")
        second = self.write_binary("app-r2", "v2")
        staged = ("--prefix", "/usr", "--destdir", str(self.stage))
        installed = self.run_install("--binary", str(first), "--version", "2026.10.10-r1", *staged)
        self.assertEqual(installed.returncode, 0, installed.stderr)
        self.assertEqual(self.manifest_value("version"), "2026.10.10-r1")
        self.assertEqual(self.manifest_value("prefix"), str(self.staged("usr")))
        self.assertRegex(self.manifest_value("binary_sha256"), r"^[0-9a-f]{64}$")
        upgraded = self.run_install("--upgrade", "--binary", str(second), "--version", "2026.10.10-r2", *staged)
        self.assertEqual(upgraded.returncode, 0, upgraded.stderr)
        self.assertIn("upgrading 2026.10.10-r1 -> 2026.10.10-r2", upgraded.stdout)
        self.assertIn("echo v2", self.staged("usr/bin/dsh-native-app").read_text())
        self.assertEqual(self.manifest_value("version"), "2026.10.10-r2")
        self.assertEqual(int(self.staged("usr/bin/dsh-native-app").stat().st_mode) & 0o777, 0o755)
        # No staging leftovers after an atomic replace.
        self.assertEqual(list(self.staged("usr/bin").glob(".*new*")), [])

    def test_upgrade_refuses_a_downgrade_without_force_and_leaves_the_install_alone(self):
        first = self.write_binary("app-r1", "v1")
        second = self.write_binary("app-r2", "v2")
        staged = ("--prefix", "/usr", "--destdir", str(self.stage))
        self.run_install("--binary", str(second), "--version", "2026.10.10-r2", *staged)
        refused = self.run_install("--upgrade", "--binary", str(first), "--version", "2026.10.10-r1", *staged)
        self.assertNotEqual(refused.returncode, 0)
        self.assertIn("refusing to downgrade 2026.10.10-r2 to 2026.10.10-r1", refused.stderr)
        self.assertIn("echo v2", self.staged("usr/bin/dsh-native-app").read_text())
        self.assertEqual(self.manifest_value("version"), "2026.10.10-r2")
        forced = self.run_install("--upgrade", "--force", "--binary", str(first), "--version", "2026.10.10-r1", *staged)
        self.assertEqual(forced.returncode, 0, forced.stderr)
        self.assertIn("echo v1", self.staged("usr/bin/dsh-native-app").read_text())

    def test_an_unorderable_version_is_recorded_without_a_downgrade_claim(self):
        first = self.write_binary("app-r1", "v1")
        second = self.write_binary("app-r2", "v2")
        staged = ("--prefix", "/usr", "--destdir", str(self.stage))
        self.run_install("--binary", str(second), "--version", "PUBLIC-not-a-calendar-version", *staged)
        allowed = self.run_install("--upgrade", "--binary", str(first), "--version", "2026.10.10-r1", *staged)
        self.assertEqual(allowed.returncode, 0, allowed.stderr)
        self.assertEqual(self.manifest_value("version"), "2026.10.10-r1")

    def test_upgrade_without_an_installation_is_refused(self):
        first = self.write_binary("app-r1", "v1")
        result = self.run_install("--upgrade", "--binary", str(first), "--prefix", "/usr",
                                  "--destdir", str(self.stage))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("--upgrade needs an existing installation", result.stderr)
        self.assertFalse(self.staged("usr/bin/dsh-native-app").exists())

    def test_plain_install_over_an_installation_points_at_upgrade(self):
        first = self.write_binary("app-r1", "v1")
        staged = ("--prefix", "/usr", "--destdir", str(self.stage))
        self.run_install("--binary", str(first), *staged)
        again = self.run_install("--binary", str(first), *staged)
        self.assertNotEqual(again.returncode, 0)
        self.assertIn("pass --upgrade, or --force to overwrite", again.stderr)

    def test_dry_run_reports_the_plan_and_writes_nothing(self):
        first = self.write_binary("app-r1", "v1")
        staged = ("--prefix", "/usr", "--destdir", str(self.stage))
        plan = self.run_install("--dry-run", "--binary", str(first), "--version", "2026.10.10-r1", *staged)
        self.assertEqual(plan.returncode, 0, plan.stderr)
        self.assertIn("would write", plan.stdout)
        self.assertIn("dry run: nothing was written", plan.stdout)
        self.assertFalse(self.stage.exists())
        # A dry run over an existing installation changes neither file nor manifest.
        self.run_install("--binary", str(first), "--version", "2026.10.10-r1", *staged)
        before = self.staged("usr/bin/dsh-native-app").read_text()
        second = self.write_binary("app-r2", "v2")
        preview = self.run_install("--dry-run", "--upgrade", "--binary", str(second),
                                   "--version", "2026.10.10-r2", *staged)
        self.assertEqual(preview.returncode, 0, preview.stderr)
        self.assertEqual(self.staged("usr/bin/dsh-native-app").read_text(), before)
        self.assertEqual(self.manifest_value("version"), "2026.10.10-r1")

    def test_upgrade_keeps_a_user_configuration_and_never_rewrites_it(self):
        first = self.write_binary("app-r1", "v1")
        second = self.write_binary("app-r2", "v2")
        prefix = self.tmp / "prefix"
        env = self.install_env()
        installed = self.run_install("--binary", str(first), "--version", "2026.10.10-r1",
                                     "--prefix", str(prefix), env=env)
        self.assertEqual(installed.returncode, 0, installed.stderr)
        config = self.tmp / "config/dsh-native/config.env"
        self.assertTrue(config.is_file())
        self.assertEqual(config.stat().st_mode & 0o777, 0o600)
        config.write_text("DSH_NATIVE_RUNTIME=/PUBLIC/edited-by-the-operator\n")
        upgraded = self.run_install("--upgrade", "--binary", str(second), "--version", "2026.10.10-r2",
                                    "--prefix", str(prefix), env=env)
        self.assertEqual(upgraded.returncode, 0, upgraded.stderr)
        self.assertIn("kept existing", upgraded.stdout)
        self.assertEqual(config.read_text(), "DSH_NATIVE_RUNTIME=/PUBLIC/edited-by-the-operator\n")

    def test_uninstall_removes_the_manifest_and_its_directory(self):
        first = self.write_binary("app-r1", "v1")
        staged = ("--prefix", "/usr", "--destdir", str(self.stage))
        self.run_install("--binary", str(first), "--version", "2026.10.10-r1", *staged)
        removed = self.run_install("--uninstall", *staged)
        self.assertEqual(removed.returncode, 0, removed.stderr)
        self.assertIn("removed", removed.stdout)
        self.assertFalse(self.manifest().exists())
        self.assertFalse(self.staged("usr/share/dsh-native").exists())

    def test_a_dry_run_version_is_recorded_as_version(self):
        first = self.write_binary("app-r1", "v1")
        staged = ("--prefix", "/usr", "--destdir", str(self.stage))
        self.run_install("--binary", str(first), "--version", "2026.10.10-r2", *staged)
        for key in ("version", "binary_sha256", "installed_at", "prefix"):
            self.assertRegex(self.manifest().read_text(), rf"(?m)^{key}=")
        self.assertNotIn(str(first), self.manifest().read_text())


class PackagingFileTests(unittest.TestCase):
    def test_pkgbuild_is_self_contained_and_pins_the_released_archive(self):
        text = (ROOT / "packaging/PKGBUILD").read_text()
        self.assertIn("sha256sums=('29bf25543e14787c16153e0ba204aa0262980db67f645df6aafa8c09a03ac7d1')", text)
        self.assertIn("pkgver=2026.10.20.r1", text)
        self.assertIn("ai.deepseek.harness.native.app.desktop", text)
        self.assertIn("ai.deepseek.harness.native.app.svg", text)
        self.assertIn("0.2.1-alpha.1", text)
        # The package must not depend on files outside the downloaded archive.
        self.assertNotIn("$srcdir/../packaging", text)

    def test_pkgbuild_auxiliary_files_match_the_installer_copies(self):
        pkgbuild = (ROOT / "packaging/PKGBUILD").read_text()
        desktop = (ROOT / "packaging/ai.deepseek.harness.native.app.desktop").read_text()
        self.assertIn(desktop.strip(), pkgbuild)
        wrapper = (ROOT / "packaging/dsh-native-launch.sh").read_text()
        for variable in ("DSH_NATIVE_RUNTIME", "DSH_NATIVE_HOME", "DSH_NATIVE_WORKSPACE"):
            self.assertIn(variable, wrapper)
            self.assertIn(variable, pkgbuild)

    def test_launcher_wrapper_requires_explicit_configuration(self):
        text = (ROOT / "packaging/dsh-native-launch.sh").read_text()
        for variable in ("DSH_NATIVE_RUNTIME", "DSH_NATIVE_HOME", "DSH_NATIVE_WORKSPACE"):
            self.assertIn(variable, text)
        self.assertIn("0.2.1-alpha.1", text)


if __name__ == "__main__":
    unittest.main()
