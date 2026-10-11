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

    def run_install(self, *args):
        return subprocess.run(["bash", str(INSTALL), *args], capture_output=True, text=True)

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


class PackagingFileTests(unittest.TestCase):
    def test_pkgbuild_is_self_contained_and_pins_the_released_archive(self):
        text = (ROOT / "packaging/PKGBUILD").read_text()
        self.assertIn("sha256sums=('a2f97dd899ae978d01f5fb0cbf3e6d04b83bca86e3bd6c7eeaea13029647b594')", text)
        self.assertIn("pkgver=2026.10.10.r2", text)
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
