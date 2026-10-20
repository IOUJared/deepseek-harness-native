//! Owned synthetic Slint scaling qualification.
//!
//! Port of `benchmarks/slint/qualify-scale.py`. It never captures the desktop or
//! changes a setting: each renderer runs one bounded synthetic window, and the
//! proof comes from the compositor's own view of that pid. Qualification only —
//! this is not a timing baseline.
use crate::compositor;
use crate::proc;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, BufRead, BufReader};
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const PREFIX: &str = "DSH_BENCH:";
const APPLICATION_SCALE: f64 = 1.25;
const EXPECTED_CLASS: &str = "org.deepseek.NativeBenchmark.Slint";

fn numbers_eq(value: &Value, expected: &[f64]) -> bool {
    value.as_array().is_some_and(|values| {
        values.len() == expected.len()
            && values
                .iter()
                .zip(expected)
                .all(|(value, expected)| value.as_f64() == Some(*expected))
    })
}

/// The settled geometry must describe the shared 1200x800 workload at 1.25.
pub fn geometry_qualified(events: &[Value]) -> bool {
    events
        .iter()
        .find(|event| event["event"].as_str() == Some("settled_geometry"))
        .is_some_and(|geometry| {
            geometry["geometry_matches_shared_workload"].as_bool() == Some(true)
                && numbers_eq(&geometry["logical_size"], &[1200.0, 800.0])
                && numbers_eq(&geometry["physical_size"], &[1500.0, 1000.0])
        })
}

/// The compositor must independently see a mapped native Slint window.
pub fn proof_qualified(facts: Option<&compositor::Facts>) -> bool {
    facts.is_some_and(|facts| {
        facts["mapped"].as_bool() == Some(true)
            && facts["xwayland"].as_bool() == Some(false)
            && facts["class"].as_str() == Some(EXPECTED_CLASS)
            && numbers_eq(&facts["size"], &[1500.0, 1000.0])
    })
}

pub fn qualified(
    timed_out: bool,
    exit_code: i32,
    events: &[Value],
    facts: Option<&compositor::Facts>,
) -> bool {
    exit_code == 0
        && !timed_out
        && geometry_qualified(events)
        && proof_qualified(facts)
        && events
            .iter()
            .any(|event| event["event"].as_str() == Some("snapshot_saved"))
}

fn environment(output: &Path, renderer: &str) -> BTreeMap<std::ffi::OsString, std::ffi::OsString> {
    let mut env: BTreeMap<std::ffi::OsString, std::ffi::OsString> = std::env::vars_os()
        .filter(|(name, _)| {
            let name = name.to_string_lossy();
            !name.starts_with("DEEPSEEK_")
                && !["API_KEY", "AUTH_TOKEN", "ACCESS_TOKEN", "SECRET"]
                    .iter()
                    .any(|part| name.contains(part))
                && name != "DISPLAY"
        })
        .collect();
    for (name, value) in [
        ("HOME", output.join("home")),
        ("XDG_CONFIG_HOME", output.join("config")),
        ("XDG_CACHE_HOME", output.join("cache")),
        ("XDG_DATA_HOME", output.join("data")),
        ("XDG_STATE_HOME", output.join("state")),
    ] {
        env.insert(name.into(), value.into_os_string());
    }
    env.insert("SLINT_BACKEND".into(), format!("winit-{renderer}").into());
    env
}

struct Options {
    probe: bool,
    bound: Duration,
}

fn qualify_one(binary: &Path, run: &Path, renderer: &str, options: &Options) -> io::Result<Value> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(run)?;
    for name in ["home", "config", "cache", "data", "state"] {
        fs::DirBuilder::new().mode(0o700).create(run.join(name))?;
    }
    let snapshot = run.join("synthetic-scale-1.25.png");
    let mut child = Command::new(binary)
        .arg("--bench")
        .arg("--scale")
        .arg("1.25")
        .arg("--snapshot")
        .arg(&snapshot)
        .current_dir(run)
        .env_clear()
        .envs(environment(run, renderer))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()?;
    let Some(identity) = proc::stat(Path::new("/proc"), child.id() as i32) else {
        let _ = child.wait();
        return Err(io::Error::other("Slint window exited before its identity"));
    };
    let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
    let facts = std::sync::Arc::new(std::sync::Mutex::new(None::<compositor::Facts>));
    let stdout = child.stdout.take().expect("piped stdout");
    let (events_tx, events_rx) = mpsc::channel::<()>();
    let pid = child.id() as i32;
    {
        let events = std::sync::Arc::clone(&events);
        let facts = std::sync::Arc::clone(&facts);
        let probe = options.probe;
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or(0) > 0 {
                if let Some(body) = line.trim_end().strip_prefix(PREFIX)
                    && let Ok(value) = serde_json::from_str::<Value>(body)
                {
                    if value["event"].as_str() == Some("settled_geometry") && probe {
                        *facts.lock().unwrap() = compositor::window_facts(pid);
                    }
                    events.lock().unwrap().push(value);
                }
                line.clear();
            }
            let _ = events_tx.send(());
        });
    }
    let (stderr_tx, stderr_rx) = mpsc::channel::<String>();
    if let Some(mut stream) = child.stderr.take() {
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = std::io::Read::read_to_string(&mut stream, &mut text);
            let _ = stderr_tx.send(text);
        });
    }
    let deadline = Instant::now() + options.bound;
    let mut timed_out = false;
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            if proc::group_is_owned(Path::new("/proc"), pid, identity.start_ticks) {
                // Safe: a signal to a process group this run created.
                unsafe {
                    libc::kill(-pid, libc::SIGTERM);
                }
            }
            let hard = Instant::now() + Duration::from_secs(5);
            while child.try_wait()?.is_none() && Instant::now() < hard {
                std::thread::sleep(Duration::from_millis(20));
            }
            if child.try_wait()?.is_none() {
                let _ = child.kill();
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let status = child.wait()?;
    let exit_code = crate::exit_code(status);
    let _ = events_rx.recv_timeout(Duration::from_secs(5));
    let stderr = stderr_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap_or_default();
    let events = std::mem::take(&mut *events.lock().unwrap());
    let facts = facts.lock().unwrap().clone();
    let result = json!({
        "renderer": renderer,
        "application_scale": APPLICATION_SCALE,
        "qualification_only_not_timing_baseline": true,
        "pid": pid,
        "exit_code": exit_code,
        "timed_out": timed_out,
        "native_window": facts,
        "snapshot": snapshot.to_string_lossy(),
        "events": events,
    });
    fs::write(
        run.join("result.json"),
        serde_json::to_string_pretty(&result)? + "\n",
    )?;
    fs::write(run.join("stderr.log"), stderr)?;
    Ok(json!({
        "renderer": renderer,
        "success": qualified(timed_out, exit_code, &events, facts.as_ref()),
        "result": run.join("result.json").to_string_lossy(),
        "geometry": events.iter().find(|event| event["event"].as_str() == Some("settled_geometry")).cloned().unwrap_or(Value::Null),
        "native_window": facts,
    }))
}

/// `xtask bench scale` — both renderers, sequentially, into a fresh directory.
pub fn run(root: &Path) -> i32 {
    let directory = root.join("benchmarks/slint");
    let binary = directory.join("target/release/dsh-slint-benchmark");
    if !binary.is_file() {
        eprintln!("slint qualification failed: the Slint benchmark is not built");
        return 1;
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let destination = directory.join(format!("qualification-scale-fix-{stamp}"));
    if let Err(error) = fs::DirBuilder::new().mode(0o700).create(&destination) {
        eprintln!("cannot create the qualification directory: {error}");
        return 1;
    }
    // Sequential qualification avoids one prototype occluding the other's mapping.
    let mut results = Vec::new();
    for renderer in ["software", "femtovg"] {
        match qualify_one(
            &binary,
            &destination.join(renderer),
            renderer,
            &Options {
                probe: true,
                bound: Duration::from_secs(35),
            },
        ) {
            Ok(result) => {
                println!("{result}");
                results.push(result);
            }
            Err(error) => {
                eprintln!("{renderer}: {error}");
                return 1;
            }
        }
    }
    let passed = results
        .iter()
        .all(|result| result["success"].as_bool() == Some(true));
    println!(
        "{}",
        json!({"qualification_directory": destination.to_string_lossy(), "all_passed": passed})
    );
    if passed { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn geometry() -> Value {
        json!({
            "event": "settled_geometry", "geometry_matches_shared_workload": true,
            "logical_size": [1200.0, 800.0], "physical_size": [1500.0, 1000.0],
        })
    }

    fn facts() -> compositor::Facts {
        let mut facts = compositor::Facts::new();
        facts.insert("mapped".into(), json!(true));
        facts.insert("xwayland".into(), json!(false));
        facts.insert("class".into(), json!(EXPECTED_CLASS));
        facts.insert("size".into(), json!([1500, 1000]));
        facts
    }

    #[test]
    fn a_qualified_run_needs_the_shared_geometry_a_native_proof_and_a_snapshot() {
        let events = vec![geometry(), json!({"event": "snapshot_saved"})];
        assert!(qualified(false, 0, &events, Some(&facts())));
        // A missing snapshot, a timeout, a foreign class or wrong pixels all fail.
        assert!(!qualified(false, 0, &[geometry()], Some(&facts())));
        assert!(!qualified(true, 0, &events, Some(&facts())));
        let mut foreign = facts();
        foreign.insert("class".into(), json!("another-class"));
        assert!(!qualified(false, 0, &events, Some(&foreign)));
        let mut shrunken = geometry();
        shrunken["logical_size"] = json!([960, 640]);
        assert!(!qualified(
            false,
            0,
            &[shrunken, json!({"event": "snapshot_saved"})],
            Some(&facts())
        ));
        // XWayland is not a native proof.
        let mut xwayland = facts();
        xwayland.insert("xwayland".into(), json!(true));
        assert!(!qualified(false, 0, &events, Some(&xwayland)));
    }

    #[test]
    fn a_missing_proof_is_not_a_qualified_run() {
        assert!(!proof_qualified(None));
        assert!(!qualified(
            false,
            0,
            &[geometry(), json!({"event": "snapshot_saved"})],
            None
        ));
    }
}
