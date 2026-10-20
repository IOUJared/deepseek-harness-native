//! Own PUBLIC production-App/native-widget probe; no Host or business executor.
//!
//! Port of `app/qualify_scroll_app.py`. Run after a serial offline/locked feature
//! example build; the exact observed binary is frozen. Source pins are
//! observations, not a reproducible complete compiler/runtime closure.
use super::private_json;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn private_dirs(path: &Path) -> io::Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
}

fn environment(output: &Path) -> BTreeMap<std::ffi::OsString, std::ffi::OsString> {
    let mut env: BTreeMap<std::ffi::OsString, std::ffi::OsString> = std::env::vars_os()
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
        ("HOME", output.join("home").to_string_lossy().into_owned()),
        (
            "XDG_CONFIG_HOME",
            output.join("config").to_string_lossy().into_owned(),
        ),
        (
            "XDG_DATA_HOME",
            output.join("data").to_string_lossy().into_owned(),
        ),
        (
            "XDG_CACHE_HOME",
            output.join("cache").to_string_lossy().into_owned(),
        ),
    ] {
        env.insert(name.into(), value.into());
    }
    env
}

fn pins(app: &Path) -> BTreeMap<String, String> {
    let mut sources: Vec<PathBuf> = vec![
        app.join("Cargo.toml"),
        app.join("Cargo.lock"),
        app.join("examples/scroll_app_smoke.rs"),
        crate::root().join("tools/xtask/src/qualify/scroll.rs"),
    ];
    if let Ok(entries) = fs::read_dir(app.join("src")) {
        let mut names: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
        names.sort();
        sources.extend(names);
    }
    let mut pinned = BTreeMap::new();
    for path in sources {
        let relative = path
            .strip_prefix(app)
            .map(|relative| relative.to_string_lossy().into_owned())
            .unwrap_or_else(|_| path.to_string_lossy().into_owned());
        if let Ok(digest) = crate::sha256_file(&path) {
            pinned.insert(relative, digest);
        }
    }
    pinned
}

/// The pass decision, separated so it can be reasoned about without a run.
pub fn scroll_passed(exit_code: i32, report: &Value) -> bool {
    let callbacks: Option<Vec<u64>> = report["callbackCounts"]
        .as_array()
        .map(|values| values.iter().filter_map(Value::as_u64).collect());
    exit_code == 0
        && report["status"].as_str() == Some("passed")
        && report["windowBackend"].as_str() == Some("wayland")
        && callbacks.as_deref() == Some(&[1, 1, 1, 0, 1])
        && report["commandCount"].as_u64() == Some(0)
        && report["effectsDispatched"].as_u64() == Some(0)
        && report["zeroBusinessEffects"].as_bool() == Some(true)
        && report["hostStarted"].as_bool() == Some(false)
        && report["workerStarted"].as_bool() == Some(false)
}

/// `xtask qualify scroll` — freeze the production-App widget probe and qualify it.
pub fn run(app: &Path) -> i32 {
    let evidence = app.join("evidence");
    if let Err(error) = private_dirs(&evidence) {
        eprintln!("cannot create app/evidence: {error}");
        return 1;
    }
    let output = match crate::tempdir::make_temp_dir(&evidence, "scroll-app-") {
        Ok(output) => output,
        Err(error) => {
            eprintln!("cannot create evidence root: {error}");
            return 1;
        }
    };
    println!("EVIDENCE_ROOT={}", output.display());
    let probe = output.join("scroll-app-probe");
    if let Err(error) = fs::copy(app.join("target/release/examples/scroll_app_smoke"), &probe) {
        eprintln!("cannot freeze the scroll probe: {error}");
        return 1;
    }
    let _ = fs::set_permissions(&probe, fs::Permissions::from_mode(0o700));
    let before = pins(app);
    let observation = json!({
        "scope": "PUBLIC actual production App/native widget integration, no Host/business executor",
        "probeSha256": crate::sha256_file(&probe).ok(),
        "probeBytes": fs::metadata(&probe).map(|meta| meta.len()).unwrap_or_default(),
        "sourceSha256": before,
        "fullCompiledSourceClosureQualified": false,
    });
    if let Err(error) = private_json(&output.join("build-observation.json"), &observation) {
        eprintln!("cannot write the build observation: {error}");
        return 1;
    }
    for name in ["home", "config", "data", "cache"] {
        let _ = private_dirs(&output.join(name));
    }
    let log = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(output.join("private-run.log"))
    {
        Ok(log) => log,
        Err(error) => {
            eprintln!("cannot create the run log: {error}");
            return 1;
        }
    };
    let mut child = match Command::new(&probe)
        .arg("--report")
        .arg(output.join("result.json"))
        .arg("--screenshot")
        .arg(output.join("own-window.png"))
        .current_dir(app)
        .env_clear()
        .envs(environment(&output))
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone().expect("log clone")))
        .stderr(Stdio::from(log))
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            eprintln!("cannot start the scroll probe: {error}");
            return 1;
        }
    };
    let deadline = Instant::now() + Duration::from_secs(30);
    while child.try_wait().ok().flatten().is_none() {
        if Instant::now() >= deadline {
            let _ = child.kill();
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let status = match child.wait() {
        Ok(status) => status,
        Err(error) => {
            eprintln!("cannot reap the scroll probe: {error}");
            return 1;
        }
    };
    let code = crate::exit_code(status);
    let report: Value = fs::read_to_string(output.join("result.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null);
    let passed = scroll_passed(code, &report);
    let after = pins(app);
    let summary = json!({
        "scope": "PUBLIC actual production App view/update/receive/restore + native Scrolled feedback and independently observed raw slot bounds",
        "passed": passed,
        "exitCode": code,
        "probeArtifact": "scroll-app-probe",
        "probeSha256": crate::sha256_file(&probe).ok(),
        "probeBytes": fs::metadata(&probe).map(|meta| meta.len()).unwrap_or_default(),
        "sourceSha256": before,
        "sourceUnchangedDuringRun": before == after,
        "ownRendererPngSha256": crate::sha256_file(&output.join("own-window.png")).ok(),
        "parentPaintInspectionComplete": false,
        "publicProductionAppWidgetCouplingQualified": passed,
        "fullCompiledSourceClosureQualified": false,
        "physicalInputQualified": false,
        "liveHostQualified": false,
        "liveGeneratedTextQualified": false,
        "integratedPerformanceQualified": false,
        "fullParityQualified": false,
    });
    if let Err(error) = private_json(&output.join("qualification.json"), &summary) {
        eprintln!("cannot write the qualification: {error}");
        return 1;
    }
    println!(
        "QUALIFICATION={}",
        output.join("qualification.json").display()
    );
    println!("[exit code: {code}]");
    if passed { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(callbacks: Value) -> Value {
        json!({
            "status": "passed", "windowBackend": "wayland", "callbackCounts": callbacks,
            "commandCount": 0, "effectsDispatched": 0, "zeroBusinessEffects": true,
            "hostStarted": false, "workerStarted": false,
        })
    }

    #[test]
    fn a_qualified_run_needs_the_exact_callback_sequence_and_no_business_effects() {
        assert!(scroll_passed(0, &report(json!([1, 1, 1, 0, 1]))));
        assert!(!scroll_passed(0, &report(json!([1, 1, 1, 1, 1]))));
        assert!(!scroll_passed(1, &report(json!([1, 1, 1, 0, 1]))));
        let mut changed = report(json!([1, 1, 1, 0, 1]));
        changed["hostStarted"] = json!(true);
        assert!(
            !scroll_passed(0, &changed),
            "a started Host is not a qualification"
        );
    }
}
