"""Idle-sampler guards; no GUI, Host or Cargo startup."""
import importlib.util
import os
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location("measure_native_idle", Path(__file__).with_name("measure-native-idle.py"))
IDLE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(IDLE)
HZ = os.sysconf("SC_CLK_TCK")


def sample(at, ticks, rss=1000, pss=800):
    return {"at": at, "pids": len(ticks), "rssKib": rss, "pssKib": pss,
            "ticks": {str(pid): value for pid, value in ticks.items()}}


class ParseTests(unittest.TestCase):
    def test_stat_parses_ticks_when_comm_contains_spaces_and_parens(self):
        text = "42 (weird ) name) R 1 42 42 0 -1 4194304 10 0 0 0 50 25 0 0 20 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0"
        self.assertEqual(IDLE.parse_stat(text), (50, 25))

    def test_smaps_rollup_requires_both_totals(self):
        self.assertEqual(IDLE.parse_smaps_rollup("Rss: 100 kB\nPss: 80 kB\n"), (100, 80))
        self.assertIsNone(IDLE.parse_smaps_rollup("Rss: 100 kB\n"))
        self.assertIsNone(IDLE.parse_smaps_rollup(""))

    def test_median_handles_odd_even_and_empty(self):
        self.assertEqual(IDLE.median([3, 1, 2]), 2)
        self.assertEqual(IDLE.median([1, 2, 3, 4]), 2.5)
        self.assertIsNone(IDLE.median([]))


class WarmWindowTests(unittest.TestCase):
    def test_endpoint_cpu_is_percentage_of_one_core(self):
        span = 2.0
        samples = [sample(0.0, {1: 0}), sample(1.0, {1: 0}),
                   sample(3.0, {1: 0}), sample(3.0 + span, {1: int(HZ * span)})]
        warm = IDLE.warm_window(samples, 3.0)
        self.assertTrue(warm["identitiesStable"])
        self.assertAlmostEqual(warm["seconds"], span, places=6)
        self.assertAlmostEqual(warm["cpuPercentOneCore"], 100.0, places=6)
        self.assertEqual(warm["sampleCount"], 2)

    def test_identity_churn_reports_unknown_not_zero(self):
        samples = [sample(0.0, {1: 0}), sample(4.0, {1: 0}), sample(5.0, {1: 0, 2: 0})]
        self.assertFalse(IDLE.warm_window(samples, 3.0)["identitiesStable"])
        self.assertIsNone(IDLE.warm_window(samples, 3.0)["cpuPercentOneCore"])

    def test_decreasing_counter_reports_unknown(self):
        samples = [sample(0.0, {1: 0}), sample(3.0, {1: 500}), sample(4.0, {1: 400})]
        self.assertFalse(IDLE.warm_window(samples, 3.0)["identitiesStable"])

    def test_too_few_warm_samples_report_unknown(self):
        self.assertFalse(IDLE.warm_window([sample(0.0, {1: 0})], 3.0)["identitiesStable"])
        self.assertFalse(IDLE.warm_window([], 3.0)["identitiesStable"])

    def test_medians_cover_the_warm_span_only(self):
        samples = [sample(0.0, {1: 0}, rss=9000, pss=9000), sample(1.0, {1: 10}, rss=9000, pss=9000),
                   sample(3.0, {1: 0}, rss=100, pss=90), sample(4.0, {1: 0}, rss=300, pss=270)]
        warm = IDLE.warm_window(samples, 3.0)
        self.assertEqual(warm["rssKibMedian"], 200)
        self.assertEqual(warm["pssKibMedian"], 180)


if __name__ == "__main__":
    unittest.main()
