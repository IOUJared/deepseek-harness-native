//! Measure the PUBLIC actual-App controlled-frame history workload, and compare
//! two retained runs without rerunning either.
//!
//! Port of `app/measure_native_history.py` and `app/compare_native_history.py`.
//! Scope is unchanged: one owned fresh Rust process with private XDG directories;
//! /proc CPU/RSS belongs to this process only, and the child census is
//! observational rather than an adversarial ownership proof. Diagnostic
//! model/observer/I/O overhead is retained, not subtracted.
use crate::proc;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const SAMPLE_INTERVAL: Duration = Duration::from_millis(50);
const CASE_BOUND: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    pub monotonic_ns: u128,
    pub unix_timestamp_ns: u128,
    pub sample_duration_ns: u128,
    pub pid: i32,
    pub start_ticks: u64,
    pub cpu_ticks: u64,
    pub rss_kib: f64,
    pub pss_kib: Option<u64>,
    pub direct_children_observed: Vec<i32>,
}

/// Nearest-rank percentile: `sorted[ceil(p*n)-1]`, never interpolated.
pub fn percentile(values: &[f64], p: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut ordered = values.to_vec();
    ordered.sort_by(|a, b| a.total_cmp(b));
    let index = ((p * ordered.len() as f64).ceil() as usize).saturating_sub(1);
    Some(ordered[index.min(ordered.len() - 1)])
}

/// True median (`statistics.median`): the average of the two middle values.
pub fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut ordered = values.to_vec();
    ordered.sort_by(|a, b| a.total_cmp(b));
    let middle = ordered.len() / 2;
    Some(if ordered.len().is_multiple_of(2) {
        (ordered[middle - 1] + ordered[middle]) / 2.0
    } else {
        ordered[middle]
    })
}

fn page_size() -> f64 {
    // Safe: a constant sysconf selector with no pointer arguments.
    let size = unsafe { libc::sysconf(libc::_SC_PAGE_SIZE) };
    if size > 0 { size as f64 } else { 4096.0 }
}

fn now_ns() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn monotonic_ns() -> u128 {
    static BASE: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    BASE.get_or_init(Instant::now).elapsed().as_nanos()
}

/// One observation of the owned process: identity, CPU, memory, direct children.
pub fn proc_sample(proc_root: &Path, pid: i32, start: Option<u64>) -> Option<Sample> {
    let before = monotonic_ns();
    let text = fs::read_to_string(proc_root.join(pid.to_string()).join("stat")).ok()?;
    let close = text.rfind(')')?;
    let fields: Vec<&str> = text.get(close + 2..)?.split_whitespace().collect();
    let identity: u64 = fields.get(19)?.parse().ok()?;
    if start.is_some_and(|start| start != identity) {
        return None;
    }
    if matches!(*fields.first()?, "Z" | "X") {
        return None;
    }
    let cpu_ticks = fields.get(11)?.parse::<u64>().ok()? + fields.get(12)?.parse::<u64>().ok()?;
    let rss_pages: f64 = fields.get(21)?.parse().ok()?;
    let mut children = Vec::new();
    if let Ok(tasks) = fs::read_dir(proc_root.join(pid.to_string()).join("task")) {
        for task in tasks.flatten() {
            if let Ok(list) = fs::read_to_string(task.path().join("children")) {
                children.extend(
                    list.split_whitespace()
                        .filter_map(|value| value.parse::<i32>().ok()),
                );
            }
        }
    }
    children.sort_unstable();
    let pss = fs::read_to_string(proc_root.join(pid.to_string()).join("smaps_rollup"))
        .ok()
        .and_then(|rollup| proc::smaps_rollup(&rollup))
        .map(|(_, pss)| pss);
    Some(Sample {
        monotonic_ns: before,
        unix_timestamp_ns: now_ns(),
        sample_duration_ns: monotonic_ns() - before,
        pid,
        start_ticks: identity,
        cpu_ticks,
        rss_kib: rss_pages * page_size() / 1024.0,
        pss_kib: pss,
        direct_children_observed: children,
    })
}

/// Parse the phase log, keeping only well-formed JSON lines.
pub fn markers(path: &Path) -> Vec<Value> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect()
}

/// The idle hold's bracketing timestamps; names are fixed by the owned probe.
pub fn idle_interval(records: &[Value]) -> (Option<u128>, Option<u128>) {
    let mut start = None;
    let mut end = None;
    for record in records {
        let tag = record.get("stage").and_then(Value::as_str);
        let at = record
            .get("unixTimestampNs")
            .and_then(Value::as_u64)
            .map(u128::from);
        match tag {
            Some("idle-begin" | "idle-start" | "idle-hold-start" | "idleStart") => start = at,
            Some("idle-end" | "idle-hold-end" | "idleEnd") => end = at,
            _ => {}
        }
    }
    (start, end)
}

/// Retained CPU/RSS accounting for a sample window. An unavailable observation is
/// `null`, never zero.
pub fn resources(samples: &[Sample], interval: Option<(u128, u128)>) -> Value {
    let selected: Vec<&Sample> = match interval {
        Some((start, end)) => samples
            .iter()
            .filter(|sample| sample.unix_timestamp_ns >= start && sample.unix_timestamp_ns <= end)
            .collect(),
        None => samples.iter().collect(),
    };
    let ticks = proc::clock_ticks();
    let mut cpus = Vec::new();
    for pair in selected.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let dt = (b.monotonic_ns as f64 - a.monotonic_ns as f64) / 1e9;
        if dt > 0.0 && b.cpu_ticks >= a.cpu_ticks {
            cpus.push((b.cpu_ticks - a.cpu_ticks) as f64 / ticks / dt * 100.0);
        }
    }
    let elapsed = match (selected.first(), selected.last()) {
        (Some(first), Some(last)) => (last.monotonic_ns - first.monotonic_ns) as f64 / 1e9,
        _ => 0.0,
    };
    let cpu_percent = match (selected.first(), selected.last()) {
        (Some(first), Some(last)) if elapsed > 0.0 => {
            Some((last.cpu_ticks - first.cpu_ticks) as f64 / ticks / elapsed * 100.0)
        }
        _ => None,
    };
    let rss: Vec<f64> = selected
        .iter()
        .map(|sample| sample.rss_kib / 1024.0)
        .collect();
    let pss: Vec<f64> = selected
        .iter()
        .filter_map(|sample| sample.pss_kib.map(|pss| pss as f64 / 1024.0))
        .collect();
    json!({
        "samples": selected.len(),
        "observedSeconds": elapsed,
        "oneCoreCpuPercent": cpu_percent,
        "oneCoreCpuP95Percent": percentile(&cpus, 0.95),
        "peakRssMiB": rss.iter().copied().reduce(f64::max),
        "medianRssMiB": percentile(&rss, 0.5),
        "peakPssMiB": pss.iter().copied().reduce(f64::max),
        "medianPssMiB": percentile(&pss, 0.5),
        "maximumSampleReadMs": selected.iter().map(|sample| sample.sample_duration_ns as f64 / 1e6).reduce(f64::max),
        "ownedDirectChildrenAbsentInSamples": selected.iter().all(|sample| sample.direct_children_observed.is_empty()),
    })
}

fn private_dirs(path: &Path) -> io::Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
}

fn private_json(path: &Path, value: &Value) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(
        (serde_json::to_string_pretty(value).map_err(io::Error::other)? + "\n").as_bytes(),
    )
}

fn sha256(path: &Path) -> io::Result<String> {
    crate::sha256_file(path)
}

struct CaseSpec<'a> {
    binary: &'a Path,
    output: &'a Path,
    app: &'a Path,
    rows: u32,
    repeat: u32,
    cycles: u32,
    idle_ms: u32,
}

fn run_case(spec: &CaseSpec<'_>, proc_root: &Path) -> io::Result<Value> {
    let folder = spec
        .output
        .join(format!("rows-{}-run-{}", spec.rows, spec.repeat));
    private_dirs(&folder)?;
    let mut env: BTreeMap<OsString, OsString> = std::env::vars_os()
        .filter(|(name, _)| {
            matches!(
                name.to_str().unwrap_or_default(),
                "WAYLAND_DISPLAY"
                    | "XDG_RUNTIME_DIR"
                    | "DBUS_SESSION_BUS_ADDRESS"
                    | "LANG"
                    | "LC_ALL"
                    | "XDG_SESSION_TYPE"
                    | "XDG_CURRENT_DESKTOP"
                    | "XDG_SESSION_DESKTOP"
            )
        })
        .collect();
    for (name, value) in [
        ("PATH", "/usr/bin:/bin".to_string()),
        ("HOME", folder.join("home").to_string_lossy().into_owned()),
        (
            "XDG_CONFIG_HOME",
            folder.join("config").to_string_lossy().into_owned(),
        ),
        (
            "XDG_DATA_HOME",
            folder.join("data").to_string_lossy().into_owned(),
        ),
        (
            "XDG_CACHE_HOME",
            folder.join("cache").to_string_lossy().into_owned(),
        ),
    ] {
        env.insert(name.into(), value.into());
    }
    for name in ["home", "config", "data", "cache"] {
        private_dirs(&folder.join(name))?;
    }
    let native = folder.join("native.json");
    let phases = folder.join("phases.jsonl");
    let log = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(folder.join("private-run.log"))?;
    let started = monotonic_ns();
    let mut child = Command::new(spec.binary)
        .arg("--report")
        .arg(&native)
        .arg("--phase-log")
        .arg(&phases)
        .arg("--rows")
        .arg(spec.rows.to_string())
        .arg("--cycles")
        .arg(spec.cycles.to_string())
        .arg("--idle-ms")
        .arg(spec.idle_ms.to_string())
        .current_dir(spec.app)
        .env_clear()
        .envs(&env)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone()?))
        .stderr(Stdio::from(log.try_clone()?))
        .spawn()?;
    let mut samples = Vec::new();
    let mut missing = 0u64;
    let mut identity: Option<u64> = None;
    let mut timeout = false;
    let deadline = Instant::now() + CASE_BOUND;
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        match proc_sample(proc_root, child.id() as i32, identity) {
            Some(sample) => {
                identity = Some(sample.start_ticks);
                samples.push(sample);
            }
            None => missing += 1,
        }
        if Instant::now() >= deadline {
            timeout = true;
            let _ = child.kill();
            break;
        }
        std::thread::sleep(SAMPLE_INTERVAL);
    }
    let status = child.wait()?;
    let code = crate::exit_code(status);
    let wall = (monotonic_ns() - started) as f64 / 1e9;
    let report: Value = fs::read_to_string(&native)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null);
    let interval = idle_interval(&markers(&phases));
    let idle = match interval {
        (Some(start), Some(end)) => Some(resources(&samples, Some((start, end)))),
        _ => None,
    };
    let anchor_key = if spec.rows == 80 {
        112
    } else {
        100 + spec.rows / 2
    };
    let valid = code == 0
        && report.get("status").and_then(Value::as_str) == Some("passed")
        && report.get("commandCount").and_then(Value::as_u64) == Some(0)
        && report.get("windowBackend").and_then(Value::as_str) == Some("wayland")
        && report["finalModel"]["keysCount"].as_u64() == Some(u64::from(spec.rows) + 12)
        && report["finalModel"]["anchorKey"].as_u64() == Some(u64::from(anchor_key))
        && report["finalModel"]["anchorIntra"].as_u64() == Some(12)
        && idle.as_ref().and_then(|idle| idle["samples"].as_u64()) >= Some(10);
    let case = json!({
        "rowsInitial": spec.rows,
        "rowsFinal": spec.rows + 12,
        "repeat": spec.repeat,
        "cycles": spec.cycles,
        "exitCode": code,
        "timeout": timeout,
        "passed": valid,
        "wallSeconds": wall,
        "pid": child.id(),
        "startTicks": identity,
        "clockTicksPerSecond": proc::clock_ticks(),
        "samplingTargetSeconds": 0.05,
        "missingSamples": missing,
        "wholeInstrumentedProcess": resources(&samples, None),
        "idleHold": idle,
        "idleUnixIntervalNs": [interval.0, interval.1],
        "sampleScope": "single owned fresh Rust process, threads included in process ticks; no Host/worker; children census observational, RSS/PSS not GPU VRAM or complete backend memory",
        "startupScope": "fresh process/private profile+cache; global OS/font/GPU caches not cleared",
        "physicalInputQualified": false,
        "liveHostQualified": false,
        "nativeWindowResizeQualified": false,
        "fullAppPerformanceQualified": false,
        "nativeReport": "native.json",
        "phaseLog": "phases.jsonl",
    });
    private_json(
        &folder.join("samples.json"),
        &serde_json::to_value(&samples).unwrap_or_default(),
    )?;
    private_json(&folder.join("measurement.json"), &case)?;
    println!(
        "rows={} run={} passed={} wall={:.3}s idle={}",
        spec.rows,
        spec.repeat,
        valid,
        wall,
        idle.map(|idle| idle.to_string())
            .unwrap_or_else(|| "null".into())
    );
    Ok(case)
}

fn usage() -> &'static str {
    "usage: xtask history measure [--label L] [--rows N...] [--repeats 1-8] [--cycles 1-32] \
     [--idle-ms 1000-5000]\n       xtask history compare BEFORE AFTER OUTPUT.json"
}

fn rejected(message: &str) -> i32 {
    eprintln!("{}\n{message}", usage());
    2
}

fn measure(arguments: &[String]) -> i32 {
    let app = crate::root().join("app");
    let mut label = "baseline".to_string();
    let mut rows: Vec<u32> = vec![80, 4084];
    let mut repeats: u32 = 3;
    let mut cycles: u32 = 4;
    let mut idle_ms: u32 = 2000;
    let mut arguments = arguments.iter();
    while let Some(argument) = arguments.next() {
        let mut value = |name: &str| {
            arguments
                .next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        let parsed: Result<(), String> = (|| match argument.as_str() {
            "--label" => {
                label = value("--label")?;
                Ok(())
            }
            "--rows" => {
                rows.clear();
                rows.push(value("--rows")?.parse().map_err(|_| "not a number")?);
                Ok(())
            }
            "--repeats" => {
                repeats = value("--repeats")?.parse().map_err(|_| "not a number")?;
                Ok(())
            }
            "--cycles" => {
                cycles = value("--cycles")?.parse().map_err(|_| "not a number")?;
                Ok(())
            }
            "--idle-ms" => {
                idle_ms = value("--idle-ms")?.parse().map_err(|_| "not a number")?;
                Ok(())
            }
            other => Err(format!("unexpected argument: {other}")),
        })();
        if let Err(message) = parsed {
            return rejected(&message);
        }
    }
    if !label.replace('-', "").chars().all(char::is_alphanumeric)
        || rows.iter().any(|rows| !(80..=4084).contains(rows))
        || !(1..=8).contains(&repeats)
        || !(1..=32).contains(&cycles)
        || !(1000..=5000).contains(&idle_ms)
    {
        return rejected("invalid bounded workload");
    }
    let evidence = app.join("evidence");
    if let Err(error) = private_dirs(&evidence) {
        eprintln!("cannot create app/evidence: {error}");
        return 1;
    }
    let output = match crate::tempdir::make_temp_dir(&evidence, &format!("native-history-{label}-"))
    {
        Ok(output) => output,
        Err(error) => {
            eprintln!("cannot create evidence root: {error}");
            return 1;
        }
    };
    println!("EVIDENCE_ROOT={}", output.display());
    let frozen = output.join("native-history-probe");
    let probe = app.join("target/release/examples/scroll_app_smoke");
    if let Err(error) = fs::copy(&probe, &frozen) {
        eprintln!("cannot freeze the probe ({}): {error}", probe.display());
        return 1;
    }
    let _ = fs::set_permissions(&frozen, fs::Permissions::from_mode(0o700));
    let mut sources: Vec<PathBuf> = vec![
        app.join("Cargo.toml"),
        app.join("Cargo.lock"),
        app.join("examples/scroll_app_smoke.rs"),
        crate::root().join("tools/xtask/Cargo.toml"),
        crate::root().join("tools/xtask/src/history.rs"),
    ];
    if let Ok(entries) = fs::read_dir(app.join("src")) {
        let mut names: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
            .collect();
        names.sort();
        sources.extend(names);
    }
    let mut before_pins = BTreeMap::new();
    for path in &sources {
        let relative = path
            .strip_prefix(&app)
            .map(|relative| relative.to_string_lossy().into_owned())
            .unwrap_or_else(|_| path.to_string_lossy().into_owned());
        match sha256(path) {
            Ok(digest) => {
                before_pins.insert(relative, digest);
            }
            Err(error) => {
                eprintln!("cannot hash {}: {error}", path.display());
                return 1;
            }
        }
    }
    let build = json!({
        "probeSha256": sha256(&frozen).unwrap_or_default(),
        "probeBytes": fs::metadata(&frozen).map(|meta| meta.len()).unwrap_or_default(),
        "sourceSha256": before_pins.clone(),
        "fullCompiledSourceClosureQualified": false,
    });
    if let Err(error) = private_json(&output.join("build-observation.json"), &build) {
        eprintln!("cannot write build observation: {error}");
        return 1;
    }
    let mut cases = Vec::new();
    for repeat in 1..=repeats {
        for rows in &rows {
            match run_case(
                &CaseSpec {
                    binary: &frozen,
                    output: &output,
                    app: &app,
                    rows: *rows,
                    repeat,
                    cycles,
                    idle_ms,
                },
                Path::new("/proc"),
            ) {
                Ok(case) => cases.push(case),
                Err(error) => {
                    eprintln!("history case failed: {error}");
                    return 1;
                }
            }
        }
    }
    let passed = cases
        .iter()
        .all(|case| case["passed"].as_bool() == Some(true));
    // A source change during the runs invalidates the probe pins, so re-hash them.
    let mut after_pins = BTreeMap::new();
    for path in &sources {
        let relative = path
            .strip_prefix(&app)
            .map(|relative| relative.to_string_lossy().into_owned())
            .unwrap_or_else(|_| path.to_string_lossy().into_owned());
        if let Ok(digest) = sha256(path) {
            after_pins.insert(relative, digest);
        }
    }
    let source_unchanged = before_pins == after_pins;
    let summary = json!({
        "scope": "PUBLIC actual native production-App controlled-frame history reflow and instrumented process resources; not wheel scrolling FPS, physical input or integrated Host performance",
        "label": label,
        "passed": passed,
        "cases": cases,
        "probeSha256": sha256(&frozen).unwrap_or_default(),
        "probeBytes": fs::metadata(&frozen).map(|meta| meta.len()).unwrap_or_default(),
        "sourceUnchangedDuringRuns": source_unchanged,
        "fullCompiledSourceClosureQualified": false,
        "fullAppPerformanceQualified": false,
        "physicalInputQualified": false,
        "liveHostQualified": false,
        "nativeWindowResizeQualified": false,
    });
    if let Err(error) = private_json(&output.join("qualification.json"), &summary) {
        eprintln!("cannot write qualification: {error}");
        return 1;
    }
    println!(
        "QUALIFICATION={}",
        output.join("qualification.json").display()
    );
    if passed { 0 } else { 1 }
}

/// Paths under `<root>/app` are reported relative to it, as the Python tool did.
fn relative_to_app(path: &Path) -> String {
    let app = crate::root().join("app");
    path.strip_prefix(&app)
        .map(|relative| relative.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

fn load(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

fn summarize(folder: &Path, rows: u64) -> Result<Value, String> {
    let qualification = load(&folder.join("qualification.json"))?;
    let cases: Vec<&Value> = qualification["cases"]
        .as_array()
        .ok_or("no cases")?
        .iter()
        .filter(|case| case["rowsInitial"].as_u64() == Some(rows))
        .collect();
    let mut samples: Vec<Vec<f64>> = Vec::new();
    let mut reports = Vec::new();
    for case in &cases {
        let repeat = case["repeat"].as_u64().ok_or("no repeat")?;
        let report = load(&folder.join(format!("rows-{rows}-run-{repeat}/native.json")))?;
        if !report["fixtureMetrics"].is_object() {
            return Err("native report lacks fixtureMetrics".into());
        }
        samples.push(
            report["steps"]
                .as_array()
                .ok_or("no steps")?
                .iter()
                .filter(|step| step["phase"].as_u64() == Some(1))
                .filter_map(|step| {
                    step["timing"]["operationToAppliedNs"]
                        .as_f64()
                        .map(|ns| ns / 1e6)
                })
                .collect(),
        );
        reports.push(report);
    }
    let warm: Vec<f64> = samples
        .iter()
        .flat_map(|run| run.iter().skip(1).copied())
        .collect();
    let first: Vec<f64> = samples
        .iter()
        .filter_map(|run| run.first().copied())
        .collect();
    Ok(json!({
        "freshProcesses": cases.len(),
        "reflowsPerProcess": samples.first().map(Vec::len),
        "allReflowIssueToReceiptMs": samples,
        "firstNarrowMedianMs": median(&first),
        "subsequentReflowSamples": warm.len(),
        "subsequentReflowMedianMs": median(&warm),
        "subsequentReflowMaximumMs": warm.iter().copied().reduce(f64::max),
        "nativeParagraphShapes": reports.iter().map(|report| report["fixtureMetrics"]["nativeParagraphShapes"].clone()).collect::<Vec<Value>>(),
        "shapeConstructorTotalMs": reports.iter().filter_map(|report| report["fixtureMetrics"]["nativeParagraphShapeNs"].as_f64().map(|ns| ns / 1e6)).collect::<Vec<f64>>(),
        "productionUpdateTotalMs": reports.iter().filter_map(|report| report["fixtureMetrics"]["productionUpdate"]["totalNs"].as_f64().map(|ns| ns / 1e6)).collect::<Vec<f64>>(),
        "productionViewTotalMs": reports.iter().filter_map(|report| report["fixtureMetrics"]["productionView"]["totalNs"].as_f64().map(|ns| ns / 1e6)).collect::<Vec<f64>>(),
        "idleRssMiB": cases.iter().map(|case| case["idleHold"]["medianRssMiB"].clone()).collect::<Vec<Value>>(),
        "idlePssMiB": cases.iter().map(|case| case["idleHold"]["medianPssMiB"].clone()).collect::<Vec<Value>>(),
        "idleCpuPercent": cases.iter().map(|case| case["idleHold"]["oneCoreCpuPercent"].clone()).collect::<Vec<Value>>(),
    }))
}

fn compare(before: &Path, after: &Path, output: &Path) -> Result<(), String> {
    // Resolve both folders first, as the Python tool did; the report names their
    // qualification files relative to `app/`.
    let before = &before.canonicalize().map_err(|error| error.to_string())?;
    let after = &after.canonicalize().map_err(|error| error.to_string())?;
    const FIELDS: [&str; 6] = [
        "nativeAnchorRowBounds",
        "nativeCurrent",
        "nativeAnchorRelativeTop",
        "actualApplicationFrameBounds",
        "currentWidgetMatches",
        "previousWidgetMatches",
    ];
    const MODEL: [&str; 9] = [
        "anchorKey",
        "anchorIntra",
        "anchorHeight",
        "offset",
        "viewport",
        "totalHeight",
        "keysCount",
        "followBottom",
        "active",
    ];
    let a = load(&before.join("qualification.json"))?;
    let b = load(&after.join("qualification.json"))?;
    if !(a["passed"].as_bool() == Some(true)
        && b["passed"].as_bool() == Some(true)
        && a["sourceUnchangedDuringRuns"].as_bool() == Some(true)
        && b["sourceUnchangedDuringRuns"].as_bool() == Some(true))
    {
        return Err("a qualification did not pass or its sources changed".into());
    }
    let cases_a = a["cases"].as_array().ok_or("no cases")?;
    let cases_b = b["cases"].as_array().ok_or("no cases")?;
    if cases_a.len() != cases_b.len() {
        return Err("case sets differ".into());
    }
    for case in cases_a {
        let rows = case["rowsInitial"].as_u64().ok_or("no rows")?;
        let repeat = case["repeat"].as_u64().ok_or("no repeat")?;
        let matching = cases_b
            .iter()
            .find(|other| {
                other["rowsInitial"].as_u64() == Some(rows)
                    && other["repeat"].as_u64() == Some(repeat)
            })
            .ok_or("case sets differ")?;
        if case["cycles"] != matching["cycles"] {
            return Err("cycle counts differ".into());
        }
        let x = load(&before.join(format!("rows-{rows}-run-{repeat}/native.json")))?;
        let y = load(&after.join(format!("rows-{rows}-run-{repeat}/native.json")))?;
        let xs = x["steps"].as_array().ok_or("no steps")?;
        let ys = y["steps"].as_array().ok_or("no steps")?;
        if xs.len() != ys.len() {
            return Err("step counts differ".into());
        }
        for (p, q) in xs.iter().zip(ys.iter()) {
            if p["passed"].as_bool() != Some(true) || q["passed"].as_bool() != Some(true) {
                return Err("a step did not pass".into());
            }
            for field in FIELDS {
                if p.get(field) != q.get(field) {
                    return Err(format!("{field} differs at rows {rows} run {repeat}"));
                }
            }
            for field in MODEL {
                if p["model"].get(field) != q["model"].get(field) {
                    return Err(format!("model.{field} differs at rows {rows} run {repeat}"));
                }
            }
        }
    }
    let mut rows: BTreeMap<String, Value> = BTreeMap::new();
    let mut row_values: Vec<u64> = cases_a
        .iter()
        .filter_map(|case| case["rowsInitial"].as_u64())
        .collect();
    row_values.sort_unstable();
    row_values.dedup();
    for rows_initial in row_values {
        let old = summarize(before, rows_initial)?;
        let new = summarize(after, rows_initial)?;
        let ratio = match (
            old["subsequentReflowMedianMs"].as_f64(),
            new["subsequentReflowMedianMs"].as_f64(),
        ) {
            (Some(old), Some(new)) if new != 0.0 => Some(old / new),
            _ => None,
        };
        rows.insert(
            rows_initial.to_string(),
            json!({"before": old, "after": new, "subsequentReflowObservedSpeedRatio": ratio}),
        );
    }
    let result = json!({
        "scope": "paired PUBLIC actual native App controlled two-width reflow; observed issue-to-widget-receipt wall time, NOT presented-frame/wheel-input latency; diagnostics are included, not subtracted",
        "exactNativeGeometryAndModelParity": true,
        "baselineQualification": relative_to_app(&before.join("qualification.json")),
        "optimizedQualification": relative_to_app(&after.join("qualification.json")),
        "baselineProbeSha256": a["probeSha256"].clone(),
        "optimizedProbeSha256": b["probeSha256"].clone(),
        "fullAppPerformanceQualified": false,
        "physicalInputQualified": false,
        "liveHostQualified": false,
        "nativeWindowResizeQualified": false,
        "rows": rows,
    });
    let mut writer = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|error| error.to_string())?;
    writer
        .write_all(
            (serde_json::to_string_pretty(&result).map_err(|error| error.to_string())? + "\n")
                .as_bytes(),
        )
        .map_err(|error| error.to_string())?;
    println!("Exact native geometry/model parity and paired raw timing summaries retained.");
    Ok(())
}

/// `xtask history measure|compare`.
pub fn run(arguments: &[String]) -> i32 {
    match arguments.first().map(String::as_str) {
        Some("measure") => measure(&arguments[1..]),
        Some("compare") => {
            let rest = &arguments[1..];
            if rest.len() != 3 {
                return rejected("compare needs BEFORE AFTER OUTPUT.json");
            }
            let before = PathBuf::from(&rest[0]);
            let after = PathBuf::from(&rest[1]);
            let output = PathBuf::from(&rest[2]);
            match compare(&before, &after, &output) {
                Ok(()) => 0,
                Err(message) => {
                    eprintln!("history comparison failed: {message}");
                    1
                }
            }
        }
        Some("help" | "--help" | "-h") | None => {
            println!("{}", usage());
            0
        }
        Some(other) => rejected(&format!("unknown history command: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tempdir::make_temp_dir;

    fn sample(monotonic_ns: u128, unix: u128, cpu: u64, rss: f64, pss: Option<u64>) -> Sample {
        Sample {
            monotonic_ns,
            unix_timestamp_ns: unix,
            sample_duration_ns: 1,
            pid: 1,
            start_ticks: 1,
            cpu_ticks: cpu,
            rss_kib: rss,
            pss_kib: pss,
            direct_children_observed: Vec::new(),
        }
    }

    #[test]
    fn nearest_rank_percentiles_are_not_interpolated() {
        assert_eq!(percentile(&[], 0.95), None);
        assert_eq!(percentile(&[3.0, 1.0, 2.0], 0.5), Some(2.0));
        assert_eq!(percentile(&[3.0, 1.0, 2.0], 0.95), Some(3.0));
        // The true median averages the middle pair; the percentile does not.
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), Some(2.5));
        assert_eq!(percentile(&[1.0, 2.0, 3.0, 4.0], 0.5), Some(2.0));
    }

    #[test]
    fn cpu_denominator_and_pss_ignore_an_unavailable_observation() {
        let ticks = proc::clock_ticks();
        let samples = [
            sample(0, 10, 20, 2048.0, None),
            sample(2_000_000_000, 30, 20 + ticks as u64, 3072.0, Some(1024)),
        ];
        let report = resources(&samples, None);
        assert_eq!(report["oneCoreCpuPercent"].as_f64(), Some(50.0));
        assert_eq!(report["peakRssMiB"].as_f64(), Some(3.0));
        assert_eq!(report["medianPssMiB"].as_f64(), Some(1.0));
        assert_eq!(report["observedSeconds"].as_f64(), Some(2.0));
        assert_eq!(
            report["ownedDirectChildrenAbsentInSamples"].as_bool(),
            Some(true)
        );
    }

    #[test]
    fn an_empty_observation_is_not_zero_cpu_evidence() {
        let report = resources(&[], None);
        assert_eq!(report["oneCoreCpuPercent"], Value::Null);
        assert_eq!(report["peakRssMiB"], Value::Null);
        assert_eq!(report["samples"].as_u64(), Some(0));
    }

    #[test]
    fn an_idle_marker_brackets_only_the_completed_hold() {
        let records = vec![
            json!({"stage": "native-observed", "unixTimestampNs": 5}),
            json!({"stage": "idle-begin", "unixTimestampNs": 10}),
            json!({"stage": "idle-end", "unixTimestampNs": 20}),
            json!({"stage": "paint-only", "unixTimestampNs": 25}),
        ];
        assert_eq!(idle_interval(&records), (Some(10), Some(20)));
        assert_eq!(idle_interval(&records[..1]), (None, None));
        let samples: Vec<Sample> = [5u128, 10, 15, 20, 25]
            .iter()
            .map(|value| Sample {
                direct_children_observed: if *value == 25 { vec![99] } else { Vec::new() },
                ..sample(*value * 1_000_000, *value, 0, 1024.0, Some(512))
            })
            .collect();
        let idle = resources(&samples, Some((10, 20)));
        assert_eq!(idle["samples"].as_u64(), Some(3));
        assert_eq!(
            idle["ownedDirectChildrenAbsentInSamples"].as_bool(),
            Some(true)
        );
        assert_eq!(
            resources(&samples, None)["ownedDirectChildrenAbsentInSamples"].as_bool(),
            Some(false)
        );
    }

    fn fake_probe(base: &Path) -> PathBuf {
        let path = base.join("fake-probe");
        fs::write(
            &path,
            "#!/bin/sh\n\
             while [ $# -gt 0 ]; do\n\
               case \"$1\" in\n\
                 --report) report=\"$2\"; shift 2;;\n\
                 --phase-log) phases=\"$2\"; shift 2;;\n\
                 --rows) rows=\"$2\"; shift 2;;\n\
                 *) shift;;\n\
               esac\n\
             done\n\
             printf '{\"stage\":\"idle-begin\",\"unixTimestampNs\":%s}\\n' \"$(date +%s%N)\" > \"$phases\"\n\
             sleep 0.8\n\
             printf '{\"stage\":\"idle-end\",\"unixTimestampNs\":%s}\\n' \"$(date +%s%N)\" >> \"$phases\"\n\
             printf '{\"status\":\"passed\",\"commandCount\":0,\"windowBackend\":\"wayland\",\"finalModel\":{\"keysCount\":%d,\"anchorKey\":112,\"anchorIntra\":12},\"steps\":[]}\\n' $((rows + 12)) > \"$report\"\n",
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[test]
    fn a_case_retains_samples_and_accepts_a_matching_probe_report() {
        let base = make_temp_dir(&std::env::temp_dir(), "history-").unwrap();
        let probe = fake_probe(&base);
        let case = run_case(
            &CaseSpec {
                binary: &probe,
                output: &base,
                app: &base,
                rows: 80,
                repeat: 1,
                cycles: 2,
                idle_ms: 1000,
            },
            Path::new("/proc"),
        )
        .unwrap();
        assert_eq!(case["passed"].as_bool(), Some(true), "{case}");
        assert!(base.join("rows-80-run-1/samples.json").is_file());
        let measurement = base.join("rows-80-run-1/measurement.json");
        assert_eq!(
            fs::metadata(&measurement).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(case["idleHold"]["samples"].as_u64().unwrap_or(0) >= 10);
        fs::remove_dir_all(base).unwrap();
    }

    fn compare_fixture(base: &Path, timings: [f64; 2], native_anchor: i64) -> PathBuf {
        let folder = base.join("run");
        fs::create_dir_all(folder.join("rows-80-run-1")).unwrap();
        let case = json!({
            "rowsInitial": 80, "repeat": 1, "cycles": 4, "passed": true,
            "idleHold": {"medianRssMiB": 1.0, "medianPssMiB": 1.0, "oneCoreCpuPercent": 1.0}
        });
        fs::write(
            folder.join("qualification.json"),
            json!({
                "passed": true, "sourceUnchangedDuringRuns": true, "probeSha256": "PUBLIC",
                "cases": [case]
            })
            .to_string(),
        )
        .unwrap();
        let steps: Vec<Value> = timings
            .iter()
            .map(|ms| {
                json!({
                    "phase": 1, "passed": true,
                    "timing": {"operationToAppliedNs": ms * 1e6},
                    "nativeCurrent": "same", "nativeAnchorRowBounds": [0, 1],
                    "nativeAnchorRelativeTop": 1.0, "actualApplicationFrameBounds": [0, 1],
                    "currentWidgetMatches": true, "previousWidgetMatches": true,
                    "model": {"anchorKey": native_anchor, "anchorIntra": 12, "anchorHeight": 1.0,
                              "offset": 0.0, "viewport": 100.0, "totalHeight": 200.0,
                              "keysCount": 92, "followBottom": false, "active": true}
                })
            })
            .collect();
        fs::write(
            folder.join("rows-80-run-1/native.json"),
            json!({"steps": steps, "fixtureMetrics": {
                "nativeParagraphShapes": 3, "nativeParagraphShapeNs": 1e6,
                "productionUpdate": {"totalNs": 2e6}, "productionView": {"totalNs": 3e6}
            }})
            .to_string(),
        )
        .unwrap();
        folder
    }

    #[test]
    fn a_comparison_requires_exact_geometry_and_model_parity_and_reports_the_ratio() {
        let base = make_temp_dir(&std::env::temp_dir(), "history-compare-").unwrap();
        let before = compare_fixture(&base.join("before"), [1.0, 3.0], 112);
        let after = compare_fixture(&base.join("after"), [1.0, 2.0], 112);
        let output = base.join("comparison.json");
        compare(&before, &after, &output).unwrap();
        let result: Value = serde_json::from_str(&fs::read_to_string(&output).unwrap()).unwrap();
        assert_eq!(
            result["rows"]["80"]["subsequentReflowObservedSpeedRatio"].as_f64(),
            Some(1.5)
        );
        assert_eq!(
            result["exactNativeGeometryAndModelParity"].as_bool(),
            Some(true)
        );
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn a_model_that_differs_between_runs_fails_the_comparison() {
        let base = make_temp_dir(&std::env::temp_dir(), "history-mismatch-").unwrap();
        let before = compare_fixture(&base.join("before"), [1.0, 3.0], 112);
        let after = compare_fixture(&base.join("after"), [1.0, 3.0], 111);
        assert!(
            compare(&before, &after, &base.join("out.json")).is_err(),
            "a changed model must not compare equal"
        );
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn a_folder_without_a_passing_qualification_is_refused() {
        let base = make_temp_dir(&std::env::temp_dir(), "history-refused-").unwrap();
        let before = compare_fixture(&base.join("before"), [1.0, 3.0], 112);
        let after = compare_fixture(&base.join("after"), [1.0, 3.0], 112);
        let broken = fs::read_to_string(after.join("qualification.json"))
            .unwrap()
            .replace("\"passed\":true", "\"passed\":false");
        fs::write(after.join("qualification.json"), broken).unwrap();
        assert!(compare(&before, &after, &base.join("out.json")).is_err());
        fs::remove_dir_all(base).unwrap();
    }
}
