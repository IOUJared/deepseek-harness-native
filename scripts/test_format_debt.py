"""Formatting-debt ratchet guards; no Cargo build, GUI or provider startup."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("format_debt", Path(__file__).with_name("format-debt.py"))
FORMAT_DEBT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(FORMAT_DEBT)


class ParsingTests(unittest.TestCase):
    def test_counts_hunks_per_file_and_normalizes_parent_segments(self):
        text = "\n".join((
            "Diff in /repo/app/examples/../src/ui.rs:12:",
            "Diff in /repo/app/examples/../src/ui.rs:40:",
            "Diff in /repo/core/src/lib.rs:7:",
            "not a diff header",
        ))
        counts = FORMAT_DEBT.parse_diff_paths(text, Path("/repo"))
        self.assertEqual(counts, {"app/src/ui.rs": 2, "core/src/lib.rs": 1})

    def test_no_headers_means_no_debt(self):
        self.assertEqual(FORMAT_DEBT.parse_diff_paths("", Path("/repo")), {})


class RatchetTests(unittest.TestCase):
    def test_clean_file_set_passes_repeatedly(self):
        current = {"app/src/ui.rs": 5}
        self.assertEqual(FORMAT_DEBT.evaluate(current, ["app/src/ui.rs"]), ([], []))

    def test_new_unformatted_file_is_an_offender(self):
        current = {"app/src/ui.rs": 5, "app/src/new.rs": 1}
        new, resolved = FORMAT_DEBT.evaluate(current, ["app/src/ui.rs"])
        self.assertEqual(new, ["app/src/new.rs"])
        self.assertEqual(resolved, [])

    def test_removing_debt_is_reported_not_failed(self):
        current = {}
        new, resolved = FORMAT_DEBT.evaluate(current, ["app/src/ui.rs"])
        self.assertEqual(new, [])
        self.assertEqual(resolved, ["app/src/ui.rs"])

    def test_write_then_load_round_trip_is_sorted(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "baseline.json"
            FORMAT_DEBT.write_baseline({"b.rs": 1, "a.rs": 2}, path)
            self.assertEqual(FORMAT_DEBT.load_baseline(path), ["a.rs", "b.rs"])

    def test_missing_baseline_is_empty_not_an_error(self):
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(FORMAT_DEBT.load_baseline(Path(directory) / "absent.json"), [])


if __name__ == "__main__":
    unittest.main()
