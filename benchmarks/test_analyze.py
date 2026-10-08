"""Headless regression checks for phase classification and unknown metrics."""
import unittest
from analyze import summarize


class Phases(unittest.TestCase):
    def report(self):
        return {"label": "fixture", "scale_requested": 1, "binary_bytes": 1000,
                "native_wayland_verified": True, "ready": {}, "errors": [],
                "events": [{"event": name, "runner_elapsed_ms": at, **extra} for name, at, extra in (
                    ("idle_start", 0, {"phase": "initial_idle"}), ("scroll_start", 6000, {}),
                    ("stream_start", 12000, {}), ("stream_end", 18000, {}),
                    ("idle_start", 18001, {"phase": "final_idle"}))],
                "samples": [{"phase": "initial_idle", "elapsed_ms": at, "process_count": 1,
                             "rss_kib": 1024, "pss_kib": None, "cpu_percent_one_core": cpu,
                             "drm_vram_kib": None} for at, cpu in ((2000, 2), (7000, 10), (13000, 4), (19000, 0))]}

    def test_second_idle_event_does_not_overwrite_first_or_hide_final_idle(self):
        phases = summarize(self.report())["phases"]
        self.assertEqual([phases[p]["sample_count"] for p in phases], [1, 1, 1, 1])
        self.assertEqual(phases["initial_idle"]["cpu_percent_one_core_mean"], 2)
        self.assertEqual(phases["final_idle"]["cpu_percent_one_core_mean"], 0)

    def test_unavailable_pss_and_vram_are_not_zero(self):
        phase = summarize(self.report())["phases"]["scroll"]
        self.assertIsNone(phase["pss_mib_median"])
        self.assertIsNone(phase["drm_vram_mib_median"])

    def test_phase_boundary_crossing_samples_are_excluded(self):
        report = self.report()
        report["samples"][1]["elapsed_ms"] = 6100
        self.assertEqual(summarize(report)["phases"]["scroll"]["sample_count"], 0)


if __name__ == "__main__":
    unittest.main()
