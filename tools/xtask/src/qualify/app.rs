//! Qualify only the owned real native app: keyless startup/lifecycle.
//!
//! Port of `scripts/qualify-native-app.py`. No desktop setting, model call or
//! screenshot is taken unless explicitly asked for. Samples belong to the owned
//! process tree only; the compositor view is probed until the window is mapped.
use super::{environment, pidfd_open, pidfd_readable, pidfd_send, prepare, private_json};
use crate::compositor;
use crate::proc;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const VERSION: &str = "0.2.1-alpha.1";
const APP_CLASS: &str = "ai.deepseek.harness.native.app";

static CANCELLED: AtomicBool = AtomicBool::new(false);

extern "C" fn cancel(_: libc::c_int) {
    CANCELLED.store(true, Ordering::SeqCst);
}

#[derive(Clone, Copy, Debug)]
pub struct AppOptions {
    pub scale: f64,
    pub seconds: u32,
    pub screenshot: bool,
    /// Compositor probe; tests without an owned window disable it.
    pub probe: bool,
}

impl Default for AppOptions {
    fn default() -> Self {
        Self {
            scale: 1.0,
            seconds: 10,
            screenshot: false,
            probe: true,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Thread {
    ticks: u64,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cpu_percent_one_core: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppSample {
    at: f64,
    #[serde(rename = "system_loadavg")]
    loadavg: [f64; 3],
    process_count: usize,
    rss_kib: u64,
    pss_kib: Option<u64>,
    drm_vram_kib: Option<u64>,
    ticks: BTreeMap<String, u64>,
    threads: BTreeMap<String, Thread>,
    elapsed_ms: f64,
    cpu_percent_one_core: Option<f64>,
}

/// Every thread of every owned process, keyed `pid:tid:startTicks`.
fn owned_threads(proc_root: &Path, root_pid: i32, root_start: u64) -> BTreeMap<String, Thread> {
    let mut threads = BTreeMap::new();
    for process in proc::process_tree(proc_root, root_pid, root_start) {
        let Ok(entries) = fs::read_dir(proc_root.join(process.pid.to_string()).join("task")) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(tid) = entry.file_name().to_string_lossy().parse::<i32>() else {
                continue;
            };
            let Ok(text) = fs::read_to_string(entry.path().join("stat")) else {
                continue;
            };
            let Some((utime, stime)) = proc::stat_ticks(&text) else {
                continue;
            };
            let started = proc::start_ticks(&text).unwrap_or_default();
            let name: String = fs::read_to_string(entry.path().join("comm"))
                .unwrap_or_default()
                .trim()
                .chars()
                .take(64)
                .collect();
            threads.insert(
                format!("{}:{tid}:{started}", process.pid),
                Thread {
                    ticks: utime + stime,
                    name,
                    cpu_percent_one_core: None,
                },
            );
        }
    }
    threads
}

/// The pass decision, separated so it can be reasoned about without a window.
pub fn app_passed(
    exit_code: i32,
    forced: bool,
    retained: &[Value],
    app: &Value,
    facts: Option<&compositor::Facts>,
) -> bool {
    exit_code == 0
        && !forced
        && retained.is_empty()
        && app.get("status").and_then(Value::as_str) == Some("passed")
        && facts.is_some_and(|facts| {
            facts.get("xwayland").and_then(Value::as_bool) == Some(false)
                && facts.get("mapped").and_then(Value::as_bool) == Some(true)
                && facts.get("class").and_then(Value::as_str) == Some(APP_CLASS)
        })
}

/// Late bounded keyless-idle samples; never a matched long-chat benchmark.
fn keyless_idle(samples: &[AppSample], seconds: u32) -> Value {
    let idle: Vec<&AppSample> = samples
        .iter()
        .filter(|sample| {
            sample.elapsed_ms >= 3000.0 && sample.elapsed_ms <= f64::from(seconds - 1) * 1000.0
        })
        .collect();
    let median_of = |values: Vec<f64>| crate::history::median(&values);
    json!({
        "sampleCount": idle.len(),
        "rssKiBMedian": median_of(idle.iter().map(|sample| sample.rss_kib as f64).collect()),
        "pssKiBMedian": median_of(idle.iter().filter_map(|sample| sample.pss_kib.map(|value| value as f64)).collect()),
        "drmVramKiBMedian": median_of(idle.iter().filter_map(|sample| sample.drm_vram_kib.map(|value| value as f64)).collect()),
        "cpuPercentOneCoreMedian": median_of(idle.iter().filter_map(|sample| sample.cpu_percent_one_core).collect()),
    })
}

struct Drive {
    samples: Vec<AppSample>,
    facts: Option<compositor::Facts>,
    mapped_ms: Option<f64>,
    forced: bool,
    interrupted: bool,
    exit_code: i32,
    retained: Vec<Value>,
}

fn drive(
    child: &mut Child,
    root_start: u64,
    output: &Path,
    options: &AppOptions,
    pidfds: &mut BTreeMap<(i32, u64, i32), i32>,
) -> io::Result<Drive> {
    let root_pid = child.id() as i32;
    let started = proc::monotonic();
    let mut samples: Vec<AppSample> = Vec::new();
    let mut previous: Option<proc::TreeSample> = None;
    let mut previous_threads: BTreeMap<String, Thread> = BTreeMap::new();
    let mut facts: Option<compositor::Facts> = None;
    let mut mapped_ms: Option<f64> = None;
    let mut forced = false;
    let mut interrupted = false;
    let deadline = started + f64::from(options.seconds) + 30.0;

    while child.try_wait()?.is_none() {
        let tree = proc::sample_tree(Path::new("/proc"), root_pid, root_start);
        let elapsed_ms = (tree.at - started) * 1000.0;
        let mut threads = owned_threads(Path::new("/proc"), root_pid, root_start);
        let elapsed = previous.as_ref().map(|previous| tree.at - previous.at);
        if let Some(elapsed) = elapsed
            && elapsed > 0.0
        {
            for (key, thread) in threads.iter_mut() {
                if let Some(old) = previous_threads.get(key) {
                    thread.cpu_percent_one_core = Some(
                        (thread.ticks.saturating_sub(old.ticks)) as f64
                            / proc::clock_ticks()
                            / elapsed
                            * 100.0,
                    );
                }
            }
        }
        let cpu = previous
            .as_ref()
            .and_then(|previous| proc::cpu_percent(previous, &tree, proc::clock_ticks()));
        samples.push(AppSample {
            at: tree.at,
            loadavg: tree.loadavg,
            process_count: tree.process_count,
            rss_kib: tree.rss_kib,
            pss_kib: tree.pss_kib,
            drm_vram_kib: tree.drm_vram_kib,
            ticks: tree.ticks.clone(),
            threads: threads.clone(),
            elapsed_ms,
            cpu_percent_one_core: cpu,
        });
        previous = Some(tree);
        previous_threads = threads;

        for stat in proc::process_tree(Path::new("/proc"), root_pid, root_start) {
            let identity = (stat.pid, stat.start_ticks, stat.pgrp);
            if !pidfds.contains_key(&identity)
                && let Some(fd) = pidfd_open(stat.pid)
            {
                let current = proc::stat(Path::new("/proc"), stat.pid);
                if current.map(|current| (current.pid, current.start_ticks, current.pgrp))
                    == Some(identity)
                {
                    pidfds.insert(identity, fd);
                } else {
                    // Safe: a descriptor opened for this identity and not yet shared.
                    unsafe {
                        libc::close(fd);
                    }
                }
            }
        }

        if facts.is_none()
            && options.probe
            && let Some(candidate) = compositor::window_facts(root_pid)
            && candidate.get("mapped").and_then(Value::as_bool) == Some(true)
        {
            facts = Some(candidate);
            mapped_ms = Some((proc::monotonic() - started) * 1000.0);
        }

        if CANCELLED.load(Ordering::SeqCst) || proc::monotonic() >= deadline {
            forced = true;
            interrupted = CANCELLED.load(Ordering::SeqCst);
            // Signal only identity-checked descriptors captured from this tree.
            for fd in pidfds.values() {
                pidfd_send(*fd, libc::SIGTERM);
            }
            let hard = Instant::now() + Duration::from_secs(3);
            while child.try_wait()?.is_none() && Instant::now() < hard {
                std::thread::sleep(Duration::from_millis(20));
            }
            if child.try_wait()?.is_none() {
                for fd in pidfds.values() {
                    pidfd_send(*fd, libc::SIGKILL);
                }
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    let status = child.wait()?;
    let mut retained = Vec::new();
    for identity in pidfds.keys() {
        let fd = pidfds[identity];
        if !pidfd_readable(fd) {
            retained.push(json!({
                "pid": identity.0,
                "startTicks": identity.1,
                "group": identity.2,
            }));
        }
    }
    let _ = output;
    Ok(Drive {
        samples,
        facts,
        mapped_ms,
        forced,
        interrupted,
        exit_code: crate::exit_code(status),
        retained,
    })
}

fn read_app(output: &Path) -> Value {
    let path = output.join("app.json");
    match fs::read(&path) {
        Ok(data) if data.len() <= 65536 => serde_json::from_slice(&data)
            .unwrap_or_else(|_| json!({"status": "missing-or-invalid-report"})),
        Ok(_) => json!({"status": "oversize-report"}),
        Err(_) => json!({"status": "missing-or-invalid-report"}),
    }
}

/// Run the owned app and return its qualification report.
pub fn measure(root: &Path, output: &Path, options: &AppOptions) -> Result<Value, String> {
    let output = prepare(root, output)?;
    let binary = root
        .join("app/target/release/dsh-native-app")
        .canonicalize()
        .map_err(|_| "release app binary is not ready".to_string())?;
    if !binary.is_file() {
        return Err("release app binary is not ready".into());
    }
    let runtime = root
        .parent()
        .map(|parent| parent.join("deepseek-harness-linux/apps/cli"))
        .unwrap_or_else(|| PathBuf::from("deepseek-harness-linux/apps/cli"));
    let binary_sha256 = crate::sha256_file(&binary).map_err(|error| error.to_string())?;
    let mut command = Command::new(&binary);
    command
        .arg("--runtime")
        .arg(&runtime)
        .arg("--expected-version")
        .arg(VERSION)
        .arg("--home")
        .arg(output.join("harness"))
        .arg("--cwd")
        .arg(output.join("workspace"))
        .arg("--user-home")
        .arg(output.join("user"))
        .arg("--scale")
        .arg(format!("{}", options.scale))
        .arg("--smoke-new-session")
        .arg("--exit-after-seconds")
        .arg(options.seconds.to_string())
        .arg("--smoke-report")
        .arg(output.join("app.json"))
        .arg("--smoke-evidence-root")
        .arg(&output)
        .env_clear()
        .envs(environment(&output))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    if options.screenshot {
        command
            .arg("--smoke-screenshot")
            .arg(output.join("own-window.png"));
    }
    let mut child = command.spawn().map_err(|error| error.to_string())?;
    let Some(root_stat) = proc::stat(Path::new("/proc"), child.id() as i32) else {
        let _ = child.wait();
        return Err("app exited before process identity was acquired".into());
    };
    // SIGINT/SIGTERM ask the loop to signal the identity-checked tree before exiting.
    // Safe: constant signals and a handler that only stores an atomic flag.
    let previous_int =
        unsafe { libc::signal(libc::SIGINT, cancel as *const () as libc::sighandler_t) };
    let previous_term =
        unsafe { libc::signal(libc::SIGTERM, cancel as *const () as libc::sighandler_t) };
    CANCELLED.store(false, Ordering::SeqCst);
    let mut pidfds: BTreeMap<(i32, u64, i32), i32> = BTreeMap::new();
    let outcome = drive(
        &mut child,
        root_stat.start_ticks,
        &output,
        options,
        &mut pidfds,
    );
    // Exceptions and cancellation must not abandon the new-session UI or its Host.
    if child.try_wait().ok().flatten().is_none() {
        for fd in pidfds.values() {
            pidfd_send(*fd, libc::SIGTERM);
        }
        let hard = Instant::now() + Duration::from_secs(3);
        while child.try_wait().ok().flatten().is_none() && Instant::now() < hard {
            std::thread::sleep(Duration::from_millis(20));
        }
        if child.try_wait().ok().flatten().is_none() {
            for fd in pidfds.values() {
                pidfd_send(*fd, libc::SIGKILL);
            }
            let _ = child.wait();
        }
    }
    for fd in pidfds.values() {
        // Safe: each descriptor was opened for one identity this run and closed once.
        unsafe {
            libc::close(*fd);
        }
    }
    // Safe: restore the handlers this run installed.
    unsafe {
        libc::signal(libc::SIGINT, previous_int);
        libc::signal(libc::SIGTERM, previous_term);
    }
    let drive = outcome.map_err(|error| error.to_string())?;
    let app = read_app(&output);
    let valid = app_passed(
        drive.exit_code,
        drive.forced,
        &drive.retained,
        &app,
        drive.facts.as_ref(),
    );
    let idle = keyless_idle(&drive.samples, options.seconds);
    let report = json!({
        "status": if valid { "passed" } else { "failed" },
        "scope": "owned native app keyless startup/lifecycle, not full parity or matched chat performance",
        "binarySha256": binary_sha256,
        "scale": options.scale,
        "exitCode": drive.exit_code,
        "forced": drive.forced,
        "interrupted": drive.interrupted,
        "ownedWindow": drive.facts,
        "compositorMappedProxyMs": drive.mapped_ms,
        "presentedFrameTiming": false,
        "remainingObservedProcesses": drive.retained,
        "app": app,
        "samples": drive.samples,
        "keylessIdle": idle,
    });
    private_json(&output.join("result.json"), &report).map_err(|error| error.to_string())?;
    println!(
        "{}",
        json!({"status": report["status"], "report": output.join("result.json").to_string_lossy(), "keylessIdle": report["keylessIdle"]})
    );
    Ok(report)
}

/// `xtask qualify app --output NEW_DIR [--scale] [--seconds] [--screenshot]`.
pub fn run(root: &Path, output: &Path, options: &AppOptions) -> i32 {
    match measure(root, output, options) {
        Ok(report) => {
            if report["status"].as_str() == Some("passed") {
                0
            } else {
                1
            }
        }
        Err(message) => {
            eprintln!("app qualification failed: {message}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tempdir::make_temp_dir;
    use serde_json::json;
    use std::os::unix::fs::PermissionsExt;

    fn sample(elapsed_ms: f64, rss: u64, pss: Option<u64>, cpu: Option<f64>) -> AppSample {
        AppSample {
            at: elapsed_ms / 1000.0,
            loadavg: [0.0; 3],
            process_count: 1,
            rss_kib: rss,
            pss_kib: pss,
            drm_vram_kib: None,
            ticks: BTreeMap::new(),
            threads: BTreeMap::new(),
            elapsed_ms,
            cpu_percent_one_core: cpu,
        }
    }

    #[test]
    fn an_app_decision_requires_a_clean_owned_run_and_a_matching_window() {
        let app = json!({"status": "passed"});
        let mut facts = compositor::Facts::new();
        facts.insert("class".into(), json!(APP_CLASS));
        facts.insert("xwayland".into(), json!(false));
        facts.insert("mapped".into(), json!(true));
        assert!(app_passed(0, false, &[], &app, Some(&facts)));
        assert!(
            !app_passed(1, false, &[], &app, Some(&facts)),
            "a failing exit is not a pass"
        );
        assert!(
            !app_passed(0, true, &[], &app, Some(&facts)),
            "a forced run is not a pass"
        );
        assert!(
            !app_passed(0, false, &[json!({"pid": 7})], &app, Some(&facts)),
            "a retained process is not a pass"
        );
        assert!(
            !app_passed(0, false, &[], &app, None),
            "an unproven window is not a pass"
        );
    }

    #[test]
    fn keyless_idle_uses_only_the_late_bounded_window() {
        let samples = [
            sample(1000.0, 9000, Some(8000), Some(50.0)),
            sample(4000.0, 1000, Some(900), Some(2.0)),
            sample(5000.0, 3000, Some(2700), Some(4.0)),
            sample(9000.0, 2000, Some(1800), Some(1.0)),
        ];
        // seconds=10 → the window ends at 9000 ms inclusive of 3s..9s.
        let idle = keyless_idle(&samples, 10);
        assert_eq!(idle["sampleCount"].as_u64(), Some(3));
        assert_eq!(idle["rssKiBMedian"].as_f64(), Some(2000.0));
        assert_eq!(idle["pssKiBMedian"].as_f64(), Some(1800.0));
        assert_eq!(idle["cpuPercentOneCoreMedian"].as_f64(), Some(2.0));
        // A shorter run keeps only what its bound allows.
        assert_eq!(keyless_idle(&samples, 5)["sampleCount"].as_u64(), Some(1));
    }

    #[test]
    fn owned_threads_reads_each_task_and_names_it() {
        let base = make_temp_dir(&std::env::temp_dir(), "app-threads-").unwrap();
        let root = base.join("proc");
        for (path, text) in [
            (
                "100/stat",
                "100 (owned) S 1 100 100 0 -1 0 0 0 0 0 4 2 0 0 0 0 0 0 20 0 1 0",
            ),
            (
                "100/task/100/stat",
                "100 (main) S 1 100 100 0 -1 0 0 0 0 0 4 2 0 0 0 0 0 0 20 0 1 0",
            ),
            ("100/task/100/comm", "main\n"),
            (
                "100/task/101/stat",
                "101 (worker) S 1 100 100 0 -1 0 0 0 0 0 1 1 0 0 0 0 0 0 21 0 1 0",
            ),
            ("100/task/101/comm", "worker\n"),
        ] {
            let target = root.join(path);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, text).unwrap();
        }
        let threads = owned_threads(&root, 100, 20);
        assert_eq!(threads.len(), 2);
        assert_eq!(threads["100:100:20"].name, "main");
        assert_eq!(threads["100:100:20"].ticks, 6);
        assert_eq!(threads["100:101:21"].name, "worker");
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn a_stand_in_app_exercises_the_whole_run_and_keeps_its_report_private() {
        let base = make_temp_dir(&std::env::temp_dir(), "app-run-").unwrap();
        fs::create_dir_all(base.join("app/evidence")).unwrap();
        let binary = base.join("app/target/release/dsh-native-app");
        fs::create_dir_all(binary.parent().unwrap()).unwrap();
        fs::write(
            &binary,
            "#!/bin/sh\n\
             while [ $# -gt 0 ]; do\n\
               case \"$1\" in\n\
                 --smoke-report) report=\"$2\"; shift 2;;\n\
                 *) shift;;\n\
               esac\n\
             done\n\
             sleep 0.1\n\
             printf '{\"status\":\"passed\",\"windowBackend\":\"wayland\"}\\n' > \"$report\"\n",
        )
        .unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
        let output = base.join("app/evidence/run");
        let report = measure(
            &base,
            &output,
            &AppOptions {
                scale: 1.0,
                seconds: 8,
                screenshot: false,
                probe: false,
            },
        )
        .unwrap();
        assert_eq!(report["exitCode"].as_i64(), Some(0));
        assert_eq!(report["app"]["status"].as_str(), Some("passed"));
        assert_eq!(
            report["status"].as_str(),
            Some("failed"),
            "no window was proven"
        );
        assert!(
            report["remainingObservedProcesses"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        let path = output.join("result.json");
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let retained: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(retained["binarySha256"].as_str().map(str::len), Some(64));
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn a_missing_app_binary_is_refused_before_any_run() {
        let base = make_temp_dir(&std::env::temp_dir(), "app-missing-").unwrap();
        fs::create_dir_all(base.join("app/evidence")).unwrap();
        let error = measure(
            &base,
            &base.join("app/evidence/run"),
            &AppOptions {
                probe: false,
                seconds: 8,
                ..AppOptions::default()
            },
        )
        .unwrap_err();
        assert!(error.contains("release app binary"), "{error}");
        fs::remove_dir_all(base).unwrap();
    }
}
