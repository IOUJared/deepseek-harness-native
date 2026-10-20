//! Summarize completed prototype runs without conflating different startup proxies.
//!
//! Port of `benchmarks/analyze.py`. Phase ownership is derived from event
//! timestamps so older runner files that mislabeled Iced's second `idle_start`
//! are still classified honestly, and an unavailable metric stays `null` rather
//! than becoming zero.
use crate::proc::median;
use serde_json::{Map, Value, json};
use std::path::Path;

fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

/// Phase that owned an elapsed time, from the event timeline.
fn observed_phase(timeline: &[(f64, &'static str)], at: f64) -> &'static str {
    timeline
        .iter()
        .rfind(|(start, _)| *start <= at)
        .map_or("startup", |(_, phase)| *phase)
}

/// One row for `summary.json` / the markdown table.
pub fn summarize(report: &Value) -> Value {
    let events = report["events"].as_array().cloned().unwrap_or_default();
    let mut starts: Map<String, Value> = Map::new();
    for event in &events {
        let name = event["event"].as_str().unwrap_or_default();
        let phase = match name {
            "scroll_start" => Some("scroll"),
            "stream_start" => Some("stream"),
            "stream_end" => Some("final_idle"),
            "idle_start" => Some(if event["phase"].as_str() == Some("final_idle") {
                "final_idle"
            } else {
                "initial_idle"
            }),
            _ => None,
        };
        if let (Some(phase), Some(at)) = (phase, event["runner_elapsed_ms"].as_f64()) {
            // `setdefault`: the first observation of a phase owns its start.
            starts.entry(phase.to_string()).or_insert(json!(at));
        }
    }
    let mut timeline: Vec<(f64, &'static str)> = starts
        .iter()
        .filter_map(|(phase, at)| {
            let phase: &'static str = match phase.as_str() {
                "initial_idle" => "initial_idle",
                "scroll" => "scroll",
                "stream" => "stream",
                "final_idle" => "final_idle",
                _ => return None,
            };
            Some((at.as_f64()?, phase))
        })
        .collect();
    timeline.sort_by(|a, b| a.0.total_cmp(&b.0));

    let samples = report["samples"].as_array().cloned().unwrap_or_default();
    let mut phases = Map::new();
    for phase in ["initial_idle", "scroll", "stream", "final_idle"] {
        // The initial idle needs a longer settle guard: Iced's second idle_start
        // can arrive before startup allocations have stopped.
        let guard_ms = if phase == "initial_idle" {
            1500.0
        } else {
            750.0
        };
        let start = starts.get(phase).and_then(Value::as_f64);
        let owned: Vec<&Value> = samples
            .iter()
            .filter(|sample| {
                let elapsed = sample["elapsed_ms"].as_f64().unwrap_or_default();
                observed_phase(&timeline, elapsed) == phase
                    && start.is_some_and(|start| elapsed >= start + guard_ms)
                    && sample["process_count"].as_u64().unwrap_or_default() > 0
            })
            .collect();
        let median_of = |field: &str, factor: f64| {
            let values: Vec<f64> = owned
                .iter()
                .filter_map(|sample| sample[field].as_f64().map(|value| value * factor))
                .collect();
            median(&values).map(round3)
        };
        let cpu: Vec<f64> = owned
            .iter()
            .filter_map(|sample| sample["cpu_percent_one_core"].as_f64())
            .collect();
        let mean = if cpu.is_empty() {
            None
        } else {
            Some(round3(cpu.iter().sum::<f64>() / cpu.len() as f64))
        };
        phases.insert(
            phase.to_string(),
            json!({
                "sample_count": owned.len(),
                "rss_mib_median": median_of("rss_kib", 1.0 / 1024.0),
                "pss_mib_median": median_of("pss_kib", 1.0 / 1024.0),
                "cpu_percent_one_core_median": median_of("cpu_percent_one_core", 1.0),
                "cpu_percent_one_core_mean": mean,
                "drm_vram_mib_median": median_of("drm_vram_kib", 1.0 / 1024.0),
            }),
        );
    }

    let ready = report.get("ready").cloned().unwrap_or(Value::Null);
    let startup_proxy = if ready.is_object() {
        ready
            .get("runner_elapsed_ms")
            .cloned()
            .unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    let settled = events
        .iter()
        .find(|event| event.get("event").and_then(Value::as_str) == Some("settled_geometry"))
        .cloned()
        .unwrap_or(Value::Null);
    let workload: Vec<Value> = events
        .iter()
        .filter(|event| {
            matches!(
                event.get("event").and_then(Value::as_str),
                Some("scroll_end" | "stream_end" | "finished")
            )
        })
        .cloned()
        .collect();
    json!({
        "label": report["label"].clone(),
        "scale_requested": report["scale_requested"].clone(),
        "frontend_prototype_only": true,
        "binary_mib": round3(report["binary_bytes"].as_f64().unwrap_or_default() / 1024.0 / 1024.0),
        "native_wayland_verified": report["native_wayland_verified"].clone(),
        "native_window_facts": report.get("native_window_facts").cloned().unwrap_or(Value::Null),
        "geometry_qualified": report.get("geometry_qualified").cloned().unwrap_or(json!(false)),
        "settled_geometry": settled,
        "ready": ready,
        "startup_proxy_ms": startup_proxy,
        "errors": report["errors"].clone(),
        "phases": phases,
        "workload_events": workload,
    })
}

/// The human table; its wording is part of the evidence, so it is reproduced.
pub fn markdown(rows: &[Value]) -> String {
    let mut lines = vec![
        "# Qualified native prototype measurements".to_string(),
        String::new(),
        "Frontend prototypes only: no Harness/Node Host process. CPU is percent of one logical core; memory values are phase medians in MiB. Startup columns are different proxies, not presented frames or a fair first-frame ranking.".to_string(),
        String::new(),
        "| Variant | UI scale | Idle RSS | Idle PSS | Idle CPU mean | Scroll CPU mean | Stream CPU mean | Final idle CPU mean | Idle DRM VRAM | Startup proxy ms | Qualified |".to_string(),
        "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|".to_string(),
    ];
    let format = |value: Option<f64>| {
        value.map_or_else(|| "unknown".to_string(), |value| format!("{value:.2}"))
    };
    for row in rows {
        let phase = |name: &str| &row["phases"][name];
        let idle = phase("initial_idle");
        let qualified = row["geometry_qualified"].as_bool() == Some(true)
            && row["native_wayland_verified"].as_bool() == Some(true)
            && row["errors"].as_array().is_none_or(Vec::is_empty);
        let values = [
            row["label"].as_str().unwrap_or_default().to_string(),
            format(row["scale_requested"].as_f64()),
            format(idle["rss_mib_median"].as_f64()),
            format(idle["pss_mib_median"].as_f64()),
            format(idle["cpu_percent_one_core_mean"].as_f64()),
            format(phase("scroll")["cpu_percent_one_core_mean"].as_f64()),
            format(phase("stream")["cpu_percent_one_core_mean"].as_f64()),
            format(phase("final_idle")["cpu_percent_one_core_mean"].as_f64()),
            format(idle["drm_vram_mib_median"].as_f64()),
            format(row["startup_proxy_ms"].as_f64()),
            if qualified { "yes" } else { "no" }.to_string(),
        ];
        lines.push(format!("| {} |", values.join(" | ")));
    }
    lines.push(String::new());
    lines.push("All qualifying runs independently match their owned PID/app_id, mapped=true, xwayland=false and expected compositor geometry. Application scaling 1.25 is not compositor fractional-scaling qualification. No hardware-completion, presentation-latency or input-latency measurements were taken.".to_string());
    lines.push(String::new());
    lines.push("Startup definitions: Iced first-redraw event plus own-window handle query; Slint GPU render-command submission callback before swap; Slint software event-loop-after-show proxy. Raw events and samples remain in sibling results.json; geometry and phase summaries are in summary.json.".to_string());
    lines.join("\n")
}

/// `xtask bench analyze RESULTS [--markdown]`.
pub fn run(arguments: &[String]) -> i32 {
    let mut results: Option<&str> = None;
    let mut markdown_output = false;
    for argument in arguments {
        match argument.as_str() {
            "--markdown" => markdown_output = true,
            other if other.starts_with('-') => {
                return rejected(&format!("unexpected argument: {other}"));
            }
            other => {
                if results.is_some() {
                    return rejected("exactly one results file is expected");
                }
                results = Some(other);
            }
        }
    }
    let Some(results) = results else {
        return rejected("a results.json path is required");
    };
    let text = match std::fs::read_to_string(Path::new(results)) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("cannot read {results}: {error}");
            return 1;
        }
    };
    let Ok(reports) = serde_json::from_str::<Vec<Value>>(&text) else {
        eprintln!("{results} is not a JSON array of benchmark reports");
        return 1;
    };
    let rows: Vec<Value> = reports.iter().map(summarize).collect();
    if markdown_output {
        println!("{}", markdown(&rows));
        return 0;
    }
    match serde_json::to_string_pretty(&rows) {
        Ok(rendered) => {
            println!("{rendered}");
            0
        }
        Err(error) => {
            eprintln!("cannot render the summary: {error}");
            1
        }
    }
}

fn rejected(message: &str) -> i32 {
    eprintln!("usage: xtask bench analyze RESULTS.json [--markdown]\n{message}");
    2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> Value {
        json!({
            "label": "fixture",
            "scale_requested": 1,
            "binary_bytes": 1000,
            "native_wayland_verified": true,
            "ready": {},
            "errors": [],
            "events": [
                {"event": "idle_start", "runner_elapsed_ms": 0.0, "phase": "initial_idle"},
                {"event": "scroll_start", "runner_elapsed_ms": 6000.0},
                {"event": "stream_start", "runner_elapsed_ms": 12000.0},
                {"event": "stream_end", "runner_elapsed_ms": 18000.0},
                {"event": "idle_start", "runner_elapsed_ms": 18001.0, "phase": "final_idle"}
            ],
            "samples": [
                {"phase": "initial_idle", "elapsed_ms": 2000.0, "process_count": 1,
                 "rss_kib": 1024, "pss_kib": null, "cpu_percent_one_core": 2.0, "drm_vram_kib": null},
                {"phase": "initial_idle", "elapsed_ms": 7000.0, "process_count": 1,
                 "rss_kib": 1024, "pss_kib": null, "cpu_percent_one_core": 10.0, "drm_vram_kib": null},
                {"phase": "initial_idle", "elapsed_ms": 13000.0, "process_count": 1,
                 "rss_kib": 1024, "pss_kib": null, "cpu_percent_one_core": 4.0, "drm_vram_kib": null},
                {"phase": "initial_idle", "elapsed_ms": 19000.0, "process_count": 1,
                 "rss_kib": 1024, "pss_kib": null, "cpu_percent_one_core": 0.0, "drm_vram_kib": null}
            ]
        })
    }

    #[test]
    fn a_second_idle_event_neither_overwrites_the_first_nor_hides_the_final_idle() {
        let row = summarize(&report());
        let count = |phase: &str| row["phases"][phase]["sample_count"].as_u64().unwrap();
        assert_eq!(
            [
                count("initial_idle"),
                count("scroll"),
                count("stream"),
                count("final_idle")
            ],
            [1, 1, 1, 1]
        );
        assert_eq!(
            row["phases"]["initial_idle"]["cpu_percent_one_core_mean"].as_f64(),
            Some(2.0)
        );
        assert_eq!(
            row["phases"]["final_idle"]["cpu_percent_one_core_mean"].as_f64(),
            Some(0.0)
        );
    }

    #[test]
    fn unavailable_pss_and_vram_stay_null_instead_of_zero() {
        let phase = summarize(&report())["phases"]["scroll"].clone();
        assert_eq!(phase["pss_mib_median"], Value::Null);
        assert_eq!(phase["drm_vram_mib_median"], Value::Null);
    }

    #[test]
    fn a_sample_crossing_a_phase_boundary_is_excluded() {
        let mut report = report();
        report["samples"][1]["elapsed_ms"] = json!(6100.0);
        let row = summarize(&report);
        assert_eq!(row["phases"]["scroll"]["sample_count"].as_u64(), Some(0));
    }

    #[test]
    fn an_unqualified_run_says_no_in_the_table_and_a_qualified_one_says_yes() {
        let mut row = summarize(&report());
        assert!(markdown(&[row.clone()]).contains("| fixture | 1.00 |"));
        assert!(markdown(&[row.clone()]).contains("| no |"));
        row["geometry_qualified"] = json!(true);
        assert!(markdown(&[row]).contains("| yes |"));
    }

    #[test]
    fn the_table_names_the_startup_proxies_it_does_not_collect() {
        let text = markdown(&[summarize(&report())]);
        assert!(text.contains("different proxies, not presented frames"));
        assert!(text.contains("not compositor fractional-scaling qualification"));
    }
}
