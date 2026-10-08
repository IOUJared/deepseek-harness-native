#!/usr/bin/env python3
"""Summarize completed prototype runs without conflating different startup proxies."""
import argparse
import json
from pathlib import Path
import statistics


def summarize(report):
    events = report["events"]
    starts = {}
    for event in events:
        name = event["event"]
        phase = {"scroll_start": "scroll", "stream_start": "stream", "stream_end": "final_idle"}.get(name)
        if name == "idle_start":
            phase = "final_idle" if event.get("phase") == "final_idle" else "initial_idle"
        if phase is not None:
            starts.setdefault(phase, event["runner_elapsed_ms"])
    timeline = sorted((at, phase) for phase, at in starts.items())
    def observed_phase(at):
        preceding = [phase for start, phase in timeline if start <= at]
        return preceding[-1] if preceding else "startup"
    phases = {}
    for phase in ("initial_idle", "scroll", "stream", "final_idle"):
        # Derive phase from event timestamps, including older runner files that
        # mislabeled Iced's second idle_start. Never overwrite the first idle start.
        guard_ms = 1500 if phase == "initial_idle" else 750
        samples = [sample for sample in report["samples"]
                   if observed_phase(sample["elapsed_ms"]) == phase and sample["elapsed_ms"] >= starts.get(phase, float("inf")) + guard_ms
                   and sample["process_count"] > 0]
        def median(field, factor=1):
            values = [sample[field] * factor for sample in samples if sample.get(field) is not None]
            return None if not values else round(statistics.median(values), 3)
        cpu = [sample["cpu_percent_one_core"] for sample in samples if sample.get("cpu_percent_one_core") is not None]
        phases[phase] = {"sample_count": len(samples), "rss_mib_median": median("rss_kib", 1 / 1024),
            "pss_mib_median": median("pss_kib", 1 / 1024), "cpu_percent_one_core_median": median("cpu_percent_one_core"),
            "cpu_percent_one_core_mean": round(statistics.mean(cpu), 3) if cpu else None,
            "drm_vram_mib_median": median("drm_vram_kib", 1 / 1024)}
    ready = report.get("ready") or {}
    return {"label": report["label"], "scale_requested": report["scale_requested"], "frontend_prototype_only": True,
            "binary_mib": round(report["binary_bytes"] / 1024**2, 3), "native_wayland_verified": report["native_wayland_verified"],
            "native_window_facts": report.get("native_window_facts"), "geometry_qualified": report.get("geometry_qualified", False),
            "settled_geometry": next((event for event in events if event.get("event") == "settled_geometry"), None), "ready": ready,
            "startup_proxy_ms": ready.get("runner_elapsed_ms"), "errors": report["errors"], "phases": phases,
            "workload_events": [event for event in events if event["event"] in ("scroll_end", "stream_end", "finished")]}


def markdown(rows):
    lines = ["# Qualified native prototype measurements", "",
             "Frontend prototypes only: no Harness/Node Host process. CPU is percent of one logical core; memory values are phase medians in MiB. Startup columns are different proxies, not presented frames or a fair first-frame ranking.", "",
             "| Variant | UI scale | Idle RSS | Idle PSS | Idle CPU mean | Scroll CPU mean | Stream CPU mean | Final idle CPU mean | Idle DRM VRAM | Startup proxy ms | Qualified |",
             "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|"]
    def fmt(value):
        return "unknown" if value is None else f"{value:.2f}"
    for row in rows:
        phases = row["phases"]
        idle = phases["initial_idle"]
        values = [row["label"], fmt(row["scale_requested"]), fmt(idle["rss_mib_median"]),
                  fmt(idle["pss_mib_median"]), fmt(idle["cpu_percent_one_core_mean"]),
                  fmt(phases["scroll"]["cpu_percent_one_core_mean"]), fmt(phases["stream"]["cpu_percent_one_core_mean"]),
                  fmt(phases["final_idle"]["cpu_percent_one_core_mean"]), fmt(idle["drm_vram_mib_median"]),
                  fmt(row["startup_proxy_ms"]), "yes" if row["geometry_qualified"] and row["native_wayland_verified"] and not row["errors"] else "no"]
        lines.append("| " + " | ".join(values) + " |")
    lines += ["", "All qualifying runs independently match their owned PID/app_id, mapped=true, xwayland=false and expected compositor geometry. Application scaling 1.25 is not compositor fractional-scaling qualification. No hardware-completion, presentation-latency or input-latency measurements were taken.", "",
              "Startup definitions: Iced first-redraw event plus own-window handle query; Slint GPU render-command submission callback before swap; Slint software event-loop-after-show proxy. Raw events and samples remain in sibling results.json; geometry and phase summaries are in summary.json."]
    return "\n".join(lines)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("results", type=Path)
    parser.add_argument("--markdown", action="store_true")
    arguments = parser.parse_args()
    reports = json.loads(arguments.results.read_text())
    rows = [summarize(report) for report in reports]
    print(markdown(rows) if arguments.markdown else json.dumps(rows, indent=2))
