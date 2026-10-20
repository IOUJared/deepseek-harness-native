//! Measure only owned benchmark process trees; no compositor or settings changes.
//!
//! Port of `benchmarks/run.py`. The protocol is one JSON object per line prefixed
//! with `DSH_BENCH:`; the runner samples the child's own process tree, qualifies
//! settled geometry against the compositor's independent view, and retains raw
//! events and samples. Nothing here changes the live compositor.
use crate::proc;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

pub const PREFIX: &str = "DSH_BENCH:";

pub use crate::compositor::Facts;

#[derive(Clone, Debug)]
pub struct Limits {
    /// Hard bound on one run; a stuck child is signalled, never waited on forever.
    pub run: Duration,
    /// Bound for collecting the output pipes after exit.
    pub drain: Duration,
    pub sample_interval: Duration,
    /// Compositor probe; disabled by tests, which have no owned window.
    pub probe: bool,
    pub proc_root: PathBuf,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            run: Duration::from_secs(40),
            drain: Duration::from_secs(3),
            sample_interval: Duration::from_millis(500),
            probe: true,
            proc_root: PathBuf::from("/proc"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RunSpec<'a> {
    pub binary: &'a Path,
    pub output: &'a Path,
    pub label: &'a str,
    pub scale: f64,
    pub renderer_env: &'a [(String, String)],
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    pub at: f64,
    #[serde(rename = "system_loadavg")]
    pub loadavg: [f64; 3],
    pub process_count: usize,
    pub rss_kib: u64,
    pub pss_kib: Option<u64>,
    pub drm_vram_kib: Option<u64>,
    pub phase: String,
    pub elapsed_ms: f64,
    pub cpu_percent_one_core: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub label: String,
    pub binary: String,
    pub binary_bytes: u64,
    pub frontend_prototype_only: bool,
    pub scale_requested: f64,
    pub wayland_display_present: bool,
    pub display_removed: bool,
    pub native_wayland_verified: bool,
    pub native_window_facts: Option<Facts>,
    pub geometry_qualified: bool,
    pub compositor_fractional_scaling_verified: bool,
    pub ready: Option<Value>,
    pub duration_seconds: f64,
    pub exit_code: i32,
    pub timeout: bool,
    pub errors: Vec<String>,
    pub events: Vec<Value>,
    pub samples: Vec<Sample>,
}

struct BenchState {
    events: Vec<Value>,
    errors: Vec<String>,
    phase: String,
    facts: Option<Facts>,
}

fn numbers(value: Option<&Value>) -> Option<Vec<f64>> {
    value?.as_array()?.iter().map(Value::as_f64).collect()
}

/// Require settled geometry and independent native mapping before ranking runs.
pub fn geometry_errors(events: &[Value], facts: Option<&Facts>, scale: f64) -> Vec<String> {
    let Some(geometry) = events
        .iter()
        .find(|event| event.get("event").and_then(Value::as_str) == Some("settled_geometry"))
    else {
        return vec!["no settled geometry observation".to_string()];
    };
    let mut errors = Vec::new();
    let logical = numbers(geometry.get("logical_size"));
    let matched = logical.as_ref().is_some_and(|logical| {
        logical.len() == 2
            && logical
                .iter()
                .zip([1200.0, 800.0])
                .all(|(value, expected)| value.is_finite() && (value - expected).abs() <= 1.0)
    });
    if !matched {
        errors.push("settled UI geometry is not the matched 1200x800 workload".to_string());
    }
    let effective = geometry.get("effective_scale").and_then(Value::as_f64);
    if !effective.is_some_and(|effective| (effective - scale).abs() < 0.01) {
        errors.push(
            "settled scale differs from requested application scale on native scale 1".to_string(),
        );
    }
    let mapped = facts
        .and_then(|facts| facts.get("mapped"))
        .and_then(Value::as_bool);
    let xwayland = facts
        .and_then(|facts| facts.get("xwayland"))
        .and_then(Value::as_bool);
    if !matches!((mapped, xwayland), (Some(true), Some(false))) {
        errors.push(
            "own test window not independently verified as mapped native Wayland".to_string(),
        );
    } else {
        let facts = facts.expect("checked above");
        if let Some(app_id) = geometry.get("app_id").and_then(Value::as_str)
            && facts.get("class").and_then(Value::as_str) != Some(app_id)
        {
            errors.push("own test window app_id differs from compositor class".to_string());
        }
        let expected = [(1200.0 * scale).round(), (800.0 * scale).round()];
        let size = numbers(facts.get("size"));
        if size.as_deref() != Some(expected.as_slice()) {
            errors.push(
                "own compositor geometry differs from matched UI at native scale 1".to_string(),
            );
        }
        let physical = numbers(
            geometry
                .get("physical_size")
                .or_else(|| geometry.get("physical_size_at_observation")),
        );
        if physical.as_deref() != Some(expected.as_slice()) {
            errors.push(
                "settled physical viewport differs from matched UI at native scale 1".to_string(),
            );
        }
    }
    errors
}

fn phase_after(event: &Value, current: &mut String) {
    match event.get("event").and_then(Value::as_str) {
        Some("idle_start") => {
            *current = if event.get("phase").and_then(Value::as_str) == Some("final_idle") {
                "final_idle".to_string()
            } else {
                "initial_idle".to_string()
            };
        }
        Some("scroll_start") => *current = "scroll".to_string(),
        Some("stream_start") => *current = "stream".to_string(),
        Some("stream_end") => *current = "final_idle".to_string(),
        _ => {}
    }
}

fn exit_code(status: std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    status
        .code()
        .or_else(|| status.signal().map(|signal| -signal))
        .unwrap_or(0)
}

fn kill_group(child: &mut Child, signal: i32) {
    // Safe: a signal to a process group this run created and has not yet reaped.
    unsafe {
        libc::kill(-(child.id() as i32), signal);
    }
}

/// Run one benchmark target and write `result.json` beneath a fresh run directory.
pub fn run_one(spec: RunSpec<'_>, limits: &Limits) -> io::Result<Report> {
    let run_dir = spec
        .output
        .join(format!("{}-scale-{}", spec.label, spec.scale));
    fs::create_dir(&run_dir)?;
    let mut env: BTreeMap<OsString, OsString> = std::env::vars_os()
        .filter(|(name, _)| {
            let name = name.to_string_lossy();
            !name.starts_with("DEEPSEEK_")
                && !["API_KEY", "AUTH_TOKEN", "ACCESS_TOKEN", "SECRET"]
                    .iter()
                    .any(|part| name.contains(part))
                && name != "DISPLAY"
        })
        .collect();
    for (key, suffix) in [
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_CACHE_HOME", "cache"),
        ("XDG_DATA_HOME", "data"),
    ] {
        let target = run_dir.join(suffix);
        fs::create_dir(&target)?;
        env.insert(key.into(), target.into_os_string());
    }
    for (key, value) in spec.renderer_env {
        env.insert(key.into(), value.into());
    }
    let wayland_present = env.contains_key(std::ffi::OsStr::new("WAYLAND_DISPLAY"));

    let started = proc::monotonic();
    let mut child = Command::new(spec.binary)
        .args(["--bench", "--scale", &format!("{}", spec.scale)])
        .current_dir(&run_dir)
        .env_clear()
        .envs(&env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()?;
    let Some(stat) = proc::stat(&limits.proc_root, child.id() as i32) else {
        let _ = child.wait();
        return Err(io::Error::other(format!(
            "{}: process exited before its identity could be recorded",
            spec.label
        )));
    };
    let root_start = stat.start_ticks;

    let state = Arc::new(Mutex::new(BenchState {
        events: Vec::new(),
        errors: Vec::new(),
        phase: "startup".to_string(),
        facts: None,
    }));

    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let (events_tx, events_rx) = mpsc::channel::<()>();
    let reader_state = Arc::clone(&state);
    let pid = child.id() as i32;
    let run_started = started;
    let probe = limits.probe;
    let _events_thread = std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        while reader.read_line(&mut line).unwrap_or(0) > 0 {
            let decoded = line.trim_end();
            if let Some(body) = decoded.strip_prefix(PREFIX) {
                match serde_json::from_str::<Value>(body) {
                    Ok(mut value) if value.is_object() && value.get("event").is_some() => {
                        value["runner_elapsed_ms"] =
                            json!((proc::monotonic() - run_started) * 1000.0);
                        let mut state = reader_state.lock().unwrap();
                        let kind = value["event"].as_str().unwrap_or_default().to_string();
                        phase_after(&value, &mut state.phase);
                        state.events.push(value);
                        drop(state);
                        if probe && matches!(kind.as_str(), "ready" | "settled_geometry") {
                            let facts = crate::compositor::window_facts(pid);
                            reader_state.lock().unwrap().facts = facts;
                        }
                    }
                    Ok(_) => reader_state
                        .lock()
                        .unwrap()
                        .errors
                        .push("invalid benchmark event".to_string()),
                    Err(_) => reader_state
                        .lock()
                        .unwrap()
                        .errors
                        .push("invalid benchmark JSON".to_string()),
                }
            }
            line.clear();
        }
        let _ = events_tx.send(());
    });

    let (stderr_tx, stderr_rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let mut stream = stderr;
        let mut buffer: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 4096];
        while let Ok(read) = stream.read(&mut chunk) {
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..read]);
            if buffer.len() > 32 * 1024 {
                let excess = buffer.len() - 32 * 1024;
                buffer.drain(..excess);
            }
        }
        let _ = stderr_tx.send(String::from_utf8_lossy(&buffer).into_owned());
    });

    let hz = proc::clock_ticks();
    let mut samples: Vec<Sample> = Vec::new();
    let mut previous: Option<proc::TreeSample> = None;
    let mut timeout = false;
    let deadline = started + limits.run.as_secs_f64();
    let mut status = None;
    while status.is_none() {
        if let Some(finished) = child.try_wait()? {
            status = Some(finished);
            break;
        }
        if proc::monotonic() >= deadline {
            timeout = true;
            state.lock().unwrap().errors.push(format!(
                "bounded benchmark exceeded {} seconds",
                limits.run.as_secs()
            ));
            if proc::group_is_owned(&limits.proc_root, pid, root_start) {
                kill_group(&mut child, libc::SIGTERM);
            }
            let hard = Instant::now() + Duration::from_secs(5);
            loop {
                if let Some(finished) = child.try_wait()? {
                    status = Some(finished);
                    break;
                }
                if Instant::now() >= hard {
                    if proc::group_is_owned(&limits.proc_root, pid, root_start) {
                        kill_group(&mut child, libc::SIGKILL);
                    }
                    status = Some(child.wait()?);
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            break;
        }
        let sample = proc::sample_tree(&limits.proc_root, pid, root_start);
        let cpu = previous
            .as_ref()
            .and_then(|previous| proc::cpu_percent(previous, &sample, hz));
        let phase = state.lock().unwrap().phase.clone();
        samples.push(Sample {
            at: sample.at,
            loadavg: sample.loadavg,
            process_count: sample.process_count,
            rss_kib: sample.rss_kib,
            pss_kib: sample.pss_kib,
            drm_vram_kib: sample.drm_vram_kib,
            phase,
            elapsed_ms: (sample.at - started) * 1000.0,
            cpu_percent_one_core: cpu,
        });
        previous = Some(sample);
        std::thread::sleep(limits.sample_interval);
    }
    let status = status.expect("the loop returns a status");

    // A child can retain the pipes after the root exits, so collection is bounded
    // rather than treating EOF as the definition of "finished".
    let stdout_done = events_rx.recv_timeout(limits.drain).is_ok();
    let (stderr_text, stderr_done) = match stderr_rx.recv_timeout(limits.drain) {
        Ok(text) => (text, true),
        Err(_) => (
            "Output collection exceeded its deadline; see errors.".to_string(),
            false,
        ),
    };
    if !stdout_done || !stderr_done {
        state
            .lock()
            .unwrap()
            .errors
            .push("benchmark descendant retained an output pipe".to_string());
    }

    let mut log = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(run_dir.join("stderr.log"))?;
    log.write_all(stderr_text.as_bytes())?;

    let mut state = state.lock().unwrap();
    let ready = state
        .events
        .iter()
        .find(|event| event["event"] == "ready")
        .cloned();
    let finished = state
        .events
        .iter()
        .any(|event| event["event"] == "finished");
    let mut errors = std::mem::take(&mut state.errors);
    if ready.is_none() {
        errors.push("no ready event".to_string());
    }
    if !finished {
        errors.push("no finished event".to_string());
    }
    let code = exit_code(status);
    if code != 0 {
        errors.push(format!("exit status {code}"));
    }
    let qualification = geometry_errors(&state.events, state.facts.as_ref(), spec.scale);
    errors.extend(qualification.iter().cloned());
    let native_verified = state.facts.as_ref().is_some_and(|facts| {
        facts.get("xwayland").and_then(Value::as_bool) == Some(false)
            && facts.get("mapped").and_then(Value::as_bool) == Some(true)
    });
    let report = Report {
        label: spec.label.to_string(),
        binary: spec.binary.to_string_lossy().into_owned(),
        binary_bytes: fs::metadata(spec.binary)?.len(),
        frontend_prototype_only: true,
        scale_requested: spec.scale,
        wayland_display_present: wayland_present,
        display_removed: true,
        native_wayland_verified: native_verified,
        native_window_facts: state.facts.clone(),
        geometry_qualified: qualification.is_empty(),
        compositor_fractional_scaling_verified: false,
        ready,
        duration_seconds: proc::monotonic() - started,
        exit_code: code,
        timeout,
        errors,
        events: std::mem::take(&mut state.events),
        samples,
    };
    drop(state);
    let rendered = serde_json::to_string_pretty(&report).map_err(io::Error::other)?;
    fs::write(run_dir.join("result.json"), rendered + "\n")?;
    println!(
        "{}",
        json!({"label": &report.label, "errors": &report.errors, "result": run_dir.join("result.json").to_string_lossy()})
    );
    Ok(report)
}

/// One `label=/path` argument: a fresh, absolute, executable target.
fn parse_app(spec: &str) -> Result<(String, PathBuf), String> {
    let (label, path) = spec
        .split_once('=')
        .ok_or("--app must be LABEL=/absolute/path")?;
    if label.is_empty()
        || !label
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err("app label must be a simple identifier".to_string());
    }
    let binary =
        fs::canonicalize(path).map_err(|error| format!("not an executable: {path}: {error}"))?;
    let metadata = fs::metadata(&binary).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
        return Err(format!("not an executable: {}", binary.display()));
    }
    Ok((label.to_string(), binary))
}

/// `xtask bench run --app LABEL=PATH ... --scale S --output NEW_DIR`.
pub fn run(arguments: &[String]) -> i32 {
    let mut apps: Vec<(String, PathBuf)> = Vec::new();
    let mut scales: Vec<f64> = Vec::new();
    let mut output: Option<PathBuf> = None;
    let mut arguments = arguments.iter();
    while let Some(argument) = arguments.next() {
        let mut value = |name: &str| {
            arguments
                .next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match argument.as_str() {
            "--app" => match value("--app").and_then(|value| parse_app(&value)) {
                Ok(app) => apps.push(app),
                Err(message) => return rejected(&message),
            },
            "--scale" => match value("--scale").and_then(|value| {
                let scale: f64 = value
                    .parse()
                    .map_err(|_| format!("scale is not a number: {value}"))?;
                if !(0.5..=3.0).contains(&scale) {
                    return Err("scale must be between 0.5 and 3".to_string());
                }
                Ok(scale)
            }) {
                Ok(scale) => scales.push(scale),
                Err(message) => return rejected(&message),
            },
            "--output" => match value("--output") {
                Ok(value) => output = Some(PathBuf::from(value)),
                Err(message) => return rejected(&message),
            },
            other => return rejected(&format!("unexpected argument: {other}")),
        }
    }
    if apps.is_empty() {
        return rejected("--app is required");
    }
    let Some(output) = output else {
        return rejected("--output is required");
    };
    if output.exists() || output.is_symlink() {
        return rejected("--output must be a new directory; refusing to replace it");
    }
    if let Err(error) = fs::create_dir_all(&output) {
        eprintln!("cannot create {}: {error}", output.display());
        return 1;
    }
    if scales.is_empty() {
        scales.push(1.0);
    }
    let mut reports = Vec::new();
    for (label, binary) in &apps {
        let renderer_env: Vec<(String, String)> = if label.starts_with("slint-software") {
            vec![("SLINT_BACKEND".into(), "winit-software".into())]
        } else if label.starts_with("slint-gpu") {
            vec![("SLINT_BACKEND".into(), "winit-femtovg".into())]
        } else {
            Vec::new()
        };
        for scale in &scales {
            match run_one(
                RunSpec {
                    binary,
                    output: &output,
                    label,
                    scale: *scale,
                    renderer_env: &renderer_env,
                },
                &Limits::default(),
            ) {
                Ok(report) => reports.push(report),
                Err(error) => {
                    eprintln!("{label} at scale {scale}: {error}");
                    return 1;
                }
            }
        }
    }
    let rendered = match serde_json::to_string_pretty(&reports) {
        Ok(rendered) => rendered,
        Err(error) => {
            eprintln!("cannot render results: {error}");
            return 1;
        }
    };
    if let Err(error) = fs::write(output.join("results.json"), rendered + "\n") {
        eprintln!("cannot write results.json: {error}");
        return 1;
    }
    if reports.iter().any(|report| !report.errors.is_empty()) {
        1
    } else {
        0
    }
}

fn rejected(message: &str) -> i32 {
    eprintln!(
        "usage: xtask bench run --app LABEL=PATH [--app ...] [--scale 0.5-3] \
         --output NEW_DIR\n{message}"
    );
    2
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tempdir::make_temp_dir;

    fn facts(mapped: bool, xwayland: bool, class: &str, size: [u64; 2]) -> Facts {
        let mut facts = Facts::new();
        facts.insert("pid".into(), json!(4242));
        facts.insert("class".into(), json!(class));
        facts.insert("xwayland".into(), json!(xwayland));
        facts.insert("mapped".into(), json!(mapped));
        facts.insert("size".into(), json!(size));
        facts
    }

    fn settled(logical: Value, physical: Value) -> Vec<Value> {
        vec![json!({
            "event": "settled_geometry",
            "logical_size": logical,
            "physical_size": physical,
            "effective_scale": 1.25,
            "app_id": "owned",
        })]
    }

    #[test]
    fn a_matched_scaled_ui_requires_independent_native_mapping() {
        let events = settled(json!([1200, 800]), json!([1500, 1000]));
        let facts = facts(true, false, "owned", [1500, 1000]);
        assert!(geometry_errors(&events, Some(&facts), 1.25).is_empty());
    }

    #[test]
    fn a_double_scaled_or_shrunken_ui_is_rejected() {
        for logical in [
            json!([1500, 1000]),
            json!([960, 640]),
            // JSON cannot carry NaN, so a non-finite measurement arrives as null.
            json!([null, 800]),
        ] {
            let events = settled(logical, json!([1500, 1000]));
            let facts = facts(true, false, "owned", [1500, 1000]);
            assert!(
                !geometry_errors(&events, Some(&facts), 1.25).is_empty(),
                "accepted a wrong logical size"
            );
        }
    }

    #[test]
    fn wrong_compositor_pixels_are_rejected_even_with_correct_metadata() {
        let events = settled(json!([1200, 800]), json!([1500, 1000]));
        let facts = facts(true, false, "owned", [1875, 1250]);
        assert!(!geometry_errors(&events, Some(&facts), 1.25).is_empty());
    }

    #[test]
    fn a_missing_unmapped_xwayland_or_foreign_app_id_is_rejected() {
        let events = settled(json!([1200, 800]), json!([1500, 1000]));
        assert!(!geometry_errors(&events, None, 1.25).is_empty());
        for changed in [
            facts(false, false, "owned", [1500, 1000]),
            facts(true, true, "owned", [1500, 1000]),
            facts(true, false, "not-owned", [1500, 1000]),
        ] {
            assert!(!geometry_errors(&events, Some(&changed), 1.25).is_empty());
        }
    }

    #[test]
    fn an_early_startup_proxy_is_not_settled_geometry() {
        let events = vec![json!({"event": "ready"})];
        let facts = facts(true, false, "owned", [1500, 1000]);
        assert_eq!(
            geometry_errors(&events, Some(&facts), 1.25),
            ["no settled geometry observation"]
        );
    }

    fn fake_bench(directory: &Path, script: &str) -> PathBuf {
        let path = directory.join("fake-bench");
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn limits() -> Limits {
        Limits {
            run: Duration::from_secs(5),
            drain: Duration::from_secs(1),
            sample_interval: Duration::from_millis(10),
            probe: false,
            proc_root: PathBuf::from("/proc"),
        }
    }

    fn complete_script() -> &'static str {
        "#!/bin/sh\n\
         printf 'DSH_BENCH:{\"event\":\"ready\"}\\n'\n\
         sleep 0.05\n\
         printf 'DSH_BENCH:{\"event\":\"idle_start\",\"phase\":\"initial_idle\"}\\n'\n\
         sleep 0.05\n\
         printf 'DSH_BENCH:{\"event\":\"settled_geometry\",\"logical_size\":[1200,800],\"physical_size\":[1200,800],\"effective_scale\":1.0,\"app_id\":\"owned\"}\\n'\n\
         sleep 0.05\n\
         printf 'DSH_BENCH:{\"event\":\"stream_end\"}\\n'\n\
         sleep 0.05\n\
         printf 'DSH_BENCH:{\"event\":\"finished\"}\\n'\n\
         exit 0\n"
    }

    #[test]
    fn a_complete_run_retains_events_samples_and_a_private_log() {
        let base = make_temp_dir(&std::env::temp_dir(), "bench-").unwrap();
        let output = base.join("out");
        fs::create_dir(&output).unwrap();
        let binary = fake_bench(&base, complete_script());
        let report = run_one(
            RunSpec {
                binary: &binary,
                output: &output,
                label: "fixture",
                scale: 1.0,
                renderer_env: &[],
            },
            &limits(),
        )
        .unwrap();
        assert_eq!(report.events.len(), 5);
        assert!(report.ready.is_some());
        assert_eq!(report.exit_code, 0);
        // Without a compositor probe the window is not independently verified.
        assert!(!report.geometry_qualified);
        assert_eq!(
            report.errors,
            ["own test window not independently verified as mapped native Wayland"]
        );
        assert!(
            !report.samples.is_empty(),
            "the tree is sampled while it runs"
        );
        assert!(
            report
                .samples
                .iter()
                .any(|sample| sample.phase == "final_idle")
        );
        let run_dir = output.join("fixture-scale-1");
        let retained: Value =
            serde_json::from_str(&fs::read_to_string(run_dir.join("result.json")).unwrap())
                .unwrap();
        assert_eq!(retained["label"], "fixture");
        let log = fs::metadata(run_dir.join("stderr.log")).unwrap();
        assert_eq!(log.permissions().mode() & 0o777, 0o600);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn an_invalid_event_is_recorded_rather_than_dropped() {
        let base = make_temp_dir(&std::env::temp_dir(), "bench-invalid-").unwrap();
        let output = base.join("out");
        fs::create_dir(&output).unwrap();
        let binary = fake_bench(
            &base,
            "#!/bin/sh\nprintf 'DSH_BENCH:not-json\\n'\nprintf 'DSH_BENCH:[1,2]\\n'\n\
             printf 'DSH_BENCH:{\"event\":\"ready\"}\\n'\nprintf 'DSH_BENCH:{\"event\":\"finished\"}\\n'\n",
        );
        let report = run_one(
            RunSpec {
                binary: &binary,
                output: &output,
                label: "invalid",
                scale: 1.0,
                renderer_env: &[],
            },
            &limits(),
        )
        .unwrap();
        assert_eq!(report.errors[0], "invalid benchmark JSON");
        assert_eq!(report.errors[1], "invalid benchmark event");
        assert_eq!(report.events.len(), 2);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn a_run_without_ready_or_finished_fails_qualification() {
        let base = make_temp_dir(&std::env::temp_dir(), "bench-incomplete-").unwrap();
        let output = base.join("out");
        fs::create_dir(&output).unwrap();
        let binary = fake_bench(
            &base,
            "#!/bin/sh\nprintf 'DSH_BENCH:{\"event\":\"ready\"}\\n'\n",
        );
        let report = run_one(
            RunSpec {
                binary: &binary,
                output: &output,
                label: "incomplete",
                scale: 1.0,
                renderer_env: &[],
            },
            &limits(),
        )
        .unwrap();
        assert!(
            report
                .errors
                .iter()
                .any(|error| error == "no finished event")
        );
        assert!(!report.errors.iter().any(|error| error == "no ready event"));
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn a_bounded_run_kills_an_overrunning_child() {
        let base = make_temp_dir(&std::env::temp_dir(), "bench-timeout-").unwrap();
        let output = base.join("out");
        fs::create_dir(&output).unwrap();
        let binary = fake_bench(
            &base,
            "#!/bin/sh\nprintf 'DSH_BENCH:{\"event\":\"ready\"}\\n'\nsleep 30\n",
        );
        let mut bounded = limits();
        bounded.run = Duration::from_millis(200);
        let report = run_one(
            RunSpec {
                binary: &binary,
                output: &output,
                label: "slow",
                scale: 1.0,
                renderer_env: &[],
            },
            &bounded,
        )
        .unwrap();
        assert!(report.timeout);
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.starts_with("bounded benchmark exceeded"))
        );
        fs::remove_dir_all(base).unwrap();
    }
}
