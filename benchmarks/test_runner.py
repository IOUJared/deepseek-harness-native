"""Headless checks for PID ownership and honest process measurement."""
import os
import unittest
from unittest.mock import patch
import run as benchmark


class Measurements(unittest.TestCase):
    def test_current_process_identity_has_stable_kernel_start(self):
        value = benchmark.proc_stat(os.getpid())
        self.assertIsNotNone(value)
        self.assertEqual(value["pid"], os.getpid())
        self.assertGreater(value["start_ticks"], 0)

    def test_process_tree_rejects_reused_root_pid(self):
        with patch.object(benchmark, "proc_stat", return_value={"start_ticks": 20}):
            self.assertEqual(benchmark.process_tree(100, 10), [])

    def test_cpu_reports_percent_of_one_core(self):
        previous = {"at": 10, "ticks": {"100:1": 100}}
        current = {"at": 12, "ticks": {"100:1": 100 + benchmark.HZ}}
        self.assertEqual(benchmark.cpu_percent(previous, current), 50)

    def test_disappeared_process_does_not_claim_zero_cpu(self):
        previous = {"at": 10, "ticks": {"100:1": 100, "200:2": 50}}
        current = {"at": 12, "ticks": {"100:1": 100}}
        self.assertIsNone(benchmark.cpu_percent(previous, current))

    def test_new_descendant_does_not_charge_cpu_before_its_first_sample(self):
        previous = {"at": 10, "ticks": {"100:1": 100}}
        current = {"at": 12, "ticks": {"100:1": 100, "200:2": 10000}}
        self.assertEqual(benchmark.cpu_percent(previous, current), 0)

    def test_group_signal_requires_root_pid_start_and_own_group(self):
        with patch.object(benchmark, "proc_stat", return_value={"start_ticks": 10, "pgrp": 100}):
            self.assertTrue(benchmark.group_is_owned(100, 10))
            self.assertFalse(benchmark.group_is_owned(100, 11))
        with patch.object(benchmark, "proc_stat", return_value={"start_ticks": 10, "pgrp": 50}):
            self.assertFalse(benchmark.group_is_owned(100, 10))
        with patch.object(benchmark, "proc_stat", return_value=None):
            self.assertFalse(benchmark.group_is_owned(100, 10))

    def test_current_process_tree_has_finite_memory_without_other_processes(self):
        identity = benchmark.proc_stat(os.getpid())
        value = benchmark.sample_tree(os.getpid(), identity["start_ticks"])
        self.assertGreaterEqual(value["process_count"], 1)
        self.assertGreater(value["rss_kib"], 0)
        self.assertEqual(len(value["system_loadavg"]), 3)
        self.assertTrue(all(value >= 0 for value in value["system_loadavg"]))
        self.assertIn(f"{os.getpid()}:{identity['start_ticks']}", value["ticks"])


class Geometry(unittest.TestCase):
    def valid(self):
        return ([{"event": "settled_geometry", "logical_size": [1200, 800],
                  "physical_size": [1500, 1000], "effective_scale": 1.25, "app_id": "owned"}],
                {"mapped": True, "xwayland": False, "class": "owned", "size": [1500, 1000]})

    def test_matched_scaled_ui_requires_independent_native_mapping(self):
        events, facts = self.valid()
        self.assertEqual(benchmark.geometry_errors(events, facts, 1.25), [])

    def test_double_scaled_and_shrunken_ui_are_rejected(self):
        for size in ([1500, 1000], [960, 640], [float("nan"), 800]):
            events, facts = self.valid()
            events[0]["logical_size"] = size
            self.assertTrue(benchmark.geometry_errors(events, facts, 1.25))

    def test_wrong_compositor_pixels_are_rejected_even_with_correct_metadata(self):
        events, facts = self.valid()
        facts["size"] = [1875, 1250]
        self.assertTrue(benchmark.geometry_errors(events, facts, 1.25))

    def test_missing_unmapped_xwayland_or_wrong_app_id_are_rejected(self):
        events, facts = self.valid()
        self.assertTrue(benchmark.geometry_errors(events, None, 1.25))
        for key, value in (("mapped", False), ("xwayland", True), ("class", "not-owned")):
            changed = {**facts, key: value}
            self.assertTrue(benchmark.geometry_errors(events, changed, 1.25))

    def test_early_startup_proxy_is_not_settled_geometry(self):
        _, facts = self.valid()
        self.assertTrue(benchmark.geometry_errors([{"event": "ready"}], facts, 1.25))


if __name__ == "__main__":
    unittest.main()
