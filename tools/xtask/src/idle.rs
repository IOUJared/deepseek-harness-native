//! Sample owned native-app idle resources over a longer window than the package smoke.
//!
//! Port of `scripts/measure-native-idle.py`. Scope and limits, deliberately narrow:
//!
//! - One owned release binary plus the isolated alpha Host it starts; fresh private
//!   XDG/HOME/workspace and a new data container per run. No credentials, no prompts.
//! - CPU is identity-checked: if the observed pid set changes inside the warm window,
//!   or any tick counter decreases, the run reports `unknown` instead of a number.
//! - `cpuPercentOneCore` is the endpoint delta over the warm span, matching the
//!   package qualification. It is not a profiler, GC attribution, presented-frame
//!   latency, input latency or a toolkit ranking.
//! - RSS/PSS come from `smaps_rollup` for the whole owned tree and are medians over
//!   the warm window, so they exclude most startup allocation; peak RSS is separate.
use crate::proc;
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const MAX_SAMPLES: usize = 4096;
const DEFAULT_RUNTIME: &str = "../deepseek-harness-linux/apps/cli";

#[derive(Clone, Debug)]
pub struct IdleSample {
    pub at: f64,
    pub pids: usize,
    pub rss_kib: u64,
    pub pss_kib: u64,
    /// pid to CPU ticks for the whole owned tree.
    pub ticks: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WarmWindow {
    pub identities_stable: bool,
    pub seconds: Option<f64>,
    pub cpu_percent_one_core: Option<f64>,
    pub sample_count: usize,
    pub rss_kib_median: Option<f64>,
    pub pss_kib_median: Option<f64>,
}

fn unknown() -> WarmWindow {
    WarmWindow {
        identities_stable: false,
        seconds: None,
        cpu_percent_one_core: None,
        sample_count: 0,
        rss_kib_median: None,
        pss_kib_median: None,
    }
}

/// Identity-checked endpoint CPU plus median RSS/PSS over the warm span.
pub fn warm_window(samples: &[IdleSample], warm_start_s: f64) -> WarmWindow {
    let Some(first) = samples.first() else {
        return unknown();
    };
    let warm: Vec<&IdleSample> = samples
        .iter()
        .filter(|sample| sample.at - first.at >= warm_start_s)
        .collect();
    if warm.len() < 2 || warm.len() > MAX_SAMPLES {
        return unknown();
    }
    let identities: Vec<&String> = warm[0].ticks.keys().collect();
    if identities.is_empty()
        || warm.iter().any(|sample| {
            sample.ticks.len() != identities.len()
                || !identities.iter().all(|key| sample.ticks.contains_key(*key))
        })
    {
        return unknown();
    }
    for pair in warm.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if b.at <= a.at || identities.iter().any(|key| b.ticks[*key] < a.ticks[*key]) {
            return unknown();
        }
    }
    let elapsed = warm.last().unwrap().at - warm[0].at;
    let delta: u64 = identities
        .iter()
        .map(|key| warm.last().unwrap().ticks[*key] - warm[0].ticks[*key])
        .sum();
    let rss: Vec<f64> = warm.iter().map(|sample| sample.rss_kib as f64).collect();
    let pss: Vec<f64> = warm.iter().map(|sample| sample.pss_kib as f64).collect();
    WarmWindow {
        identities_stable: true,
        seconds: Some(elapsed),
        sample_count: warm.len(),
        cpu_percent_one_core: Some((delta as f64 / proc::clock_ticks()) / elapsed * 100.0),
        rss_kib_median: proc::median(&rss),
        pss_kib_median: proc::median(&pss),
    }
}

/// Current pid to ppid for every readable process.
pub fn ppid_map(proc_root: &Path) -> BTreeMap<i32, i32> {
    let mut map = BTreeMap::new();
    let Ok(entries) = fs::read_dir(proc_root) else {
        return map;
    };
    for entry in entries.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<i32>() else {
            continue;
        };
        let Ok(text) = fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        if let Some(parent) = proc::ppid(&text) {
            map.insert(pid, parent);
        }
    }
    map
}

/// `{pid: ticks}` for the root and its descendants, bounded.
pub fn read_tree(
    proc_root: &Path,
    root: i32,
    parents: &BTreeMap<i32, i32>,
    limit: usize,
) -> BTreeMap<i32, u64> {
    let mut tree: BTreeMap<i32, u64> = BTreeMap::new();
    let mut pending = vec![root];
    while let Some(pid) = pending.pop() {
        if tree.len() >= limit || tree.contains_key(&pid) {
            continue;
        }
        let Ok(text) = fs::read_to_string(proc_root.join(pid.to_string()).join("stat")) else {
            continue;
        };
        let Some((utime, stime)) = proc::stat_ticks(&text) else {
            continue;
        };
        tree.insert(pid, utime + stime);
        pending.extend(
            parents
                .iter()
                .filter(|(child, parent)| **parent == pid && !tree.contains_key(child))
                .map(|(child, _)| *child),
        );
    }
    tree
}

/// One sample of the owned tree, or `None` when any process's memory is unreadable.
pub fn sample(proc_root: &Path, root: i32) -> Option<IdleSample> {
    let parents = ppid_map(proc_root);
    let ticks = read_tree(proc_root, root, &parents, 64);
    let mut rss = 0;
    let mut pss = 0;
    for pid in ticks.keys() {
        let text = fs::read_to_string(proc_root.join(pid.to_string()).join("smaps_rollup")).ok()?;
        let (process_rss, process_pss) = proc::smaps_rollup(&text)?;
        rss += process_rss;
        pss += process_pss;
    }
    Some(IdleSample {
        at: proc::monotonic(),
        pids: ticks.len(),
        rss_kib: rss,
        pss_kib: pss,
        ticks: ticks
            .into_iter()
            .map(|(pid, value)| (pid.to_string(), value))
            .collect(),
    })
}

fn local_hhmmss() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as libc::time_t;
    let mut broken_down: libc::tm = unsafe { std::mem::zeroed() };
    // Safe: both pointers are valid for the call and no thread shares the buffer.
    unsafe {
        libc::localtime_r(&seconds, &mut broken_down);
    }
    format!(
        "{:02}{:02}{:02}",
        broken_down.tm_hour, broken_down.tm_min, broken_down.tm_sec
    )
}

fn private_dir(path: &Path) -> io::Result<()> {
    DirBuilder::new().recursive(true).mode(0o700).create(path)
}

fn write_private(path: &Path, contents: &str) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(contents.as_bytes())
}

struct RunSpec<'a> {
    binary: &'a Path,
    runtime: &'a Path,
    seconds: u32,
    scale: f64,
    warm_start_s: f64,
    output: &'a Path,
}

fn run_once(spec: &RunSpec<'_>) -> io::Result<Value> {
    let run = spec.output.join(format!("run-{}", local_hhmmss()));
    for name in [
        "home",
        "user",
        "workspace",
        "xdg/config",
        "xdg/data",
        "xdg/cache",
    ] {
        private_dir(&run.join(name))?;
    }
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
    for (key, value) in [
        ("PATH", "/usr/bin:/bin".to_string()),
        ("HOME", run.join("home").to_string_lossy().into_owned()),
        ("ICED_BACKEND", "wgpu".to_string()),
        (
            "XDG_CONFIG_HOME",
            run.join("xdg/config").to_string_lossy().into_owned(),
        ),
        (
            "XDG_DATA_HOME",
            run.join("xdg/data").to_string_lossy().into_owned(),
        ),
        (
            "XDG_CACHE_HOME",
            run.join("xdg/cache").to_string_lossy().into_owned(),
        ),
    ] {
        env.insert(key.into(), value.into());
    }
    let report = run.join("smoke.json");
    let screenshot = run.join("own-window.png");
    let log = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(run.join("run.log"))?;
    let mut child = Command::new(spec.binary)
        .arg("--runtime")
        .arg(spec.runtime)
        .arg("--expected-version")
        .arg("0.2.1-alpha.1")
        .arg("--home")
        .arg(run.join("home"))
        .arg("--user-home")
        .arg(run.join("user"))
        .arg("--cwd")
        .arg(run.join("workspace"))
        .arg("--scale")
        .arg(format!("{}", spec.scale))
        .arg("--smoke-new-session")
        .arg("--exit-after-seconds")
        .arg(spec.seconds.to_string())
        .arg("--smoke-report")
        .arg(&report)
        .arg("--smoke-screenshot")
        .arg(&screenshot)
        .env_clear()
        .envs(&env)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone()?))
        .stderr(Stdio::from(log.try_clone()?))
        .process_group(0)
        .spawn()?;
    let mut samples = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(u64::from(spec.seconds) + 45);
    while child.try_wait()?.is_none() && Instant::now() < deadline && samples.len() < MAX_SAMPLES {
        if let Some(current) = sample(Path::new("/proc"), child.id() as i32) {
            samples.push(current);
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    if child.try_wait()?.is_none() {
        // Safe: this run created the group and has not reaped the child.
        unsafe {
            libc::kill(-(child.id() as i32), libc::SIGKILL);
        }
    }
    let status = child.wait()?;
    drop(log);
    let smoke: Value = fs::read_to_string(&report)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null);
    let mut smoke_subset = Map::new();
    for key in [
        "status",
        "windowBackend",
        "coreReady",
        "paintVerified",
        "modelPrompts",
        "modelCatalogRequested",
        "deadlineReached",
        "stop",
    ] {
        smoke_subset.insert(
            key.to_string(),
            smoke.get(key).cloned().unwrap_or(Value::Null),
        );
    }
    let peak = samples.iter().map(|sample| sample.rss_kib).max();
    let first = samples.first().map(|sample| sample.at).unwrap_or_default();
    let result = json!({
        "secondsRequested": spec.seconds,
        "exitCode": crate::exit_code(status),
        "sampleCount": samples.len(),
        "peakRssKib": peak,
        "warm": warm_window(&samples, spec.warm_start_s),
        "samples": samples.iter().map(|sample| json!({
            "at": ((sample.at - first) * 1000.0).round() / 1000.0,
            "pids": sample.pids,
            "rssKib": sample.rss_kib,
            "pssKib": sample.pss_kib,
        })).collect::<Vec<Value>>(),
        "smoke": smoke_subset,
    });
    let rendered = serde_json::to_string_pretty(&result).map_err(io::Error::other)?;
    write_private(&run.join("result.json"), &(rendered + "\n"))?;
    Ok(result)
}

fn usage() -> &'static str {
    "usage: xtask idle [--binary PATH] [--runtime PATH] [--seconds 8-600] [--repeats 1-10] \
     [--scale S] [--warm-start SECONDS] --output NEW_DIR"
}

fn rejected(message: &str) -> i32 {
    eprintln!("{}\n{message}", usage());
    2
}

/// `xtask idle --output NEW_DIR`.
pub fn run(arguments: &[String]) -> i32 {
    let root = crate::root();
    let mut binary = root.join("app/target/release/dsh-native-app");
    let mut runtime = root
        .parent()
        .map(|parent| parent.join(DEFAULT_RUNTIME))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_RUNTIME));
    let mut seconds: u32 = 60;
    let mut repeats: u32 = 3;
    let mut scale = 1.0f64;
    let mut warm_start = 3.0f64;
    let mut output: Option<PathBuf> = None;
    let mut arguments = arguments.iter();
    while let Some(argument) = arguments.next() {
        let mut value = |name: &str| {
            arguments
                .next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        let parsed: Result<(), String> = (|| match argument.as_str() {
            "--binary" => {
                binary = PathBuf::from(value("--binary")?);
                Ok(())
            }
            "--runtime" => {
                runtime = PathBuf::from(value("--runtime")?);
                Ok(())
            }
            "--seconds" => {
                seconds = value("--seconds")?.parse().map_err(|_| "not a number")?;
                Ok(())
            }
            "--repeats" => {
                repeats = value("--repeats")?.parse().map_err(|_| "not a number")?;
                Ok(())
            }
            "--scale" => {
                scale = value("--scale")?.parse().map_err(|_| "not a number")?;
                Ok(())
            }
            "--warm-start" => {
                warm_start = value("--warm-start")?.parse().map_err(|_| "not a number")?;
                Ok(())
            }
            "--output" => {
                output = Some(PathBuf::from(value("--output")?));
                Ok(())
            }
            other => Err(format!("unexpected argument: {other}")),
        })();
        if let Err(message) = parsed {
            return rejected(&message);
        }
    }
    if !(8..=600).contains(&seconds) || !(1..=10).contains(&repeats) {
        return rejected("--seconds must be 8-600 and --repeats 1-10");
    }
    let Some(output) = output else {
        return rejected("--output is required");
    };
    let binary = match fs::canonicalize(&binary) {
        Ok(path) => path,
        Err(error) => return rejected(&format!("--binary: {error}")),
    };
    let runtime = match fs::canonicalize(&runtime) {
        Ok(path) => path,
        Err(error) => return rejected(&format!("--runtime: {error}")),
    };
    if output.exists() || output.is_symlink() {
        return rejected("--output must be a new directory inside an existing parent");
    }
    // Non-recursive, matching `mkdir(mode=0o700)`: a missing parent is an error.
    if let Err(error) = DirBuilder::new().mode(0o700).create(&output) {
        eprintln!("cannot create {}: {error}", output.display());
        return 1;
    }
    let mut runs = Vec::new();
    for _ in 0..repeats {
        match run_once(&RunSpec {
            binary: &binary,
            runtime: &runtime,
            seconds,
            scale,
            warm_start_s: warm_start,
            output: &output,
        }) {
            Ok(result) => runs.push(result),
            Err(error) => {
                eprintln!("idle run failed: {error}");
                return 1;
            }
        }
    }
    let warm: Vec<&Value> = runs
        .iter()
        .map(|run| &run["warm"])
        .filter(|warm| warm["identitiesStable"].as_bool() == Some(true))
        .collect();
    let cpu: Vec<f64> = warm
        .iter()
        .filter_map(|warm| warm["cpuPercentOneCore"].as_f64())
        .collect();
    let rss: Vec<f64> = warm
        .iter()
        .filter_map(|warm| warm["rssKibMedian"].as_f64())
        .collect();
    let pss: Vec<f64> = warm
        .iter()
        .filter_map(|warm| warm["pssKibMedian"].as_f64())
        .collect();
    let peak = runs
        .iter()
        .filter_map(|run| run["peakRssKib"].as_u64())
        .filter(|value| *value > 0)
        .max();
    let measured = warm.len() == runs.len();
    let summary = json!({
        "status": if measured { "measured" } else { "partial" },
        "binarySha256": crate::sha256_file(&binary).ok(),
        "binaryBytes": fs::metadata(&binary).map(|meta| meta.len()).unwrap_or_default(),
        "secondsRequested": seconds,
        "repeats": repeats,
        "scale": scale,
        "warmStartSeconds": warm_start,
        "cpuPercentOneCoreMedian": proc::median(&cpu),
        "cpuPercentOneCoreRange": [
            cpu.iter().copied().reduce(f64::min),
            cpu.iter().copied().reduce(f64::max),
        ],
        "rssKibMedian": proc::median(&rss),
        "pssKibMedian": proc::median(&pss),
        "peakRssKibMax": peak,
        "scope": "Owned-tree sampled idle resources over one owned blank session; not presented-frame, input, VRAM, provider or full-dependency-closure measurement",
        "runs": runs,
    });
    let rendered = match serde_json::to_string_pretty(&summary) {
        Ok(rendered) => rendered,
        Err(error) => {
            eprintln!("cannot render the summary: {error}");
            return 1;
        }
    };
    if let Err(error) = write_private(&output.join("summary.json"), &(rendered + "\n")) {
        eprintln!("cannot write summary.json: {error}");
        return 1;
    }
    let printed = json!({
        "status": summary["status"].clone(),
        "secondsRequested": summary["secondsRequested"].clone(),
        "repeats": summary["repeats"].clone(),
        "cpuPercentOneCoreMedian": summary["cpuPercentOneCoreMedian"].clone(),
        "cpuPercentOneCoreRange": summary["cpuPercentOneCoreRange"].clone(),
        "rssKibMedian": summary["rssKibMedian"].clone(),
        "pssKibMedian": summary["pssKibMedian"].clone(),
        "peakRssKibMax": summary["peakRssKibMax"].clone(),
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&printed).unwrap_or_default()
    );
    if summary["status"] == "measured" {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tempdir::make_temp_dir;

    fn sampled(at: f64, ticks: &[(i32, u64)], rss: u64, pss: u64) -> IdleSample {
        IdleSample {
            at,
            pids: ticks.len(),
            rss_kib: rss,
            pss_kib: pss,
            ticks: ticks
                .iter()
                .map(|(pid, value)| (pid.to_string(), *value))
                .collect(),
        }
    }

    #[test]
    fn endpoint_cpu_is_a_percentage_of_one_core() {
        let span = 2.0;
        let hz = proc::clock_ticks();
        let samples = [
            sampled(0.0, &[(1, 0)], 0, 0),
            sampled(1.0, &[(1, 0)], 0, 0),
            sampled(3.0, &[(1, 0)], 0, 0),
            sampled(3.0 + span, &[(1, (hz * span) as u64)], 0, 0),
        ];
        let warm = warm_window(&samples, 3.0);
        assert!(warm.identities_stable);
        assert!((warm.seconds.unwrap() - span).abs() < 1e-9);
        assert!((warm.cpu_percent_one_core.unwrap() - 100.0).abs() < 1e-6);
        assert_eq!(warm.sample_count, 2);
    }

    #[test]
    fn identity_churn_reports_unknown_not_zero() {
        let samples = [
            sampled(0.0, &[(1, 0)], 0, 0),
            sampled(4.0, &[(1, 0)], 0, 0),
            sampled(5.0, &[(1, 0), (2, 0)], 0, 0),
        ];
        let warm = warm_window(&samples, 3.0);
        assert!(!warm.identities_stable);
        assert_eq!(warm.cpu_percent_one_core, None);
    }

    #[test]
    fn a_decreasing_counter_reports_unknown() {
        let samples = [
            sampled(0.0, &[(1, 0)], 0, 0),
            sampled(3.0, &[(1, 500)], 0, 0),
            sampled(4.0, &[(1, 400)], 0, 0),
        ];
        assert!(!warm_window(&samples, 3.0).identities_stable);
    }

    #[test]
    fn too_few_warm_samples_report_unknown() {
        assert!(!warm_window(&[sampled(0.0, &[(1, 0)], 0, 0)], 3.0).identities_stable);
        assert!(!warm_window(&[], 3.0).identities_stable);
    }

    #[test]
    fn the_medians_cover_the_warm_span_only() {
        let samples = [
            sampled(0.0, &[(1, 0)], 9000, 9000),
            sampled(1.0, &[(1, 10)], 9000, 9000),
            sampled(3.0, &[(1, 0)], 100, 90),
            sampled(4.0, &[(1, 0)], 300, 270),
        ];
        let warm = warm_window(&samples, 3.0);
        assert_eq!(warm.rss_kib_median, Some(200.0));
        assert_eq!(warm.pss_kib_median, Some(180.0));
    }

    #[test]
    fn a_tree_sample_sums_memory_for_the_whole_owned_tree() {
        let root = make_temp_dir(&std::env::temp_dir(), "idle-").unwrap();
        for (path, text) in [
            (
                "100/stat",
                "100 (owned) S 1 100 100 0 -1 0 0 0 0 0 4 2 0 0 20 0 1 0".to_string(),
            ),
            (
                "100/smaps_rollup",
                "Rss: 1000 kB\nPss: 800 kB\n".to_string(),
            ),
            (
                "200/stat",
                "200 (child) S 100 100 100 0 -1 0 0 0 0 0 1 1 0 0 21 0 1 0".to_string(),
            ),
            ("200/smaps_rollup", "Rss: 500 kB\nPss: 300 kB\n".to_string()),
        ] {
            let target = root.join(path);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, text).unwrap();
        }
        let measured = sample(&root, 100).expect("a complete tree samples");
        assert_eq!(measured.pids, 2);
        assert_eq!(measured.rss_kib, 1500);
        assert_eq!(measured.pss_kib, 1100);
        assert_eq!(measured.ticks["100"], 6);
        // A process whose memory cannot be read makes the total unknown, not zero.
        fs::remove_file(root.join("200/smaps_rollup")).unwrap();
        assert!(sample(&root, 100).is_none());
        fs::remove_dir_all(root).unwrap();
    }
}
