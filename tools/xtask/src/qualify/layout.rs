//! Own-renderer PUBLIC assistant display Details captures.
//!
//! Port of `app/qualify_formatted_details.py`. Uses the actual App view and a
//! scripted mode update; no Host, business executor or desktop capture.
use super::private_json;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const MODES: [&str; 4] = [
    "assistant-details-source",
    "assistant-details-formatted",
    "assistant-details-narrow",
    "assistant-details-fallback",
];

fn private_dirs(path: &Path) -> io::Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
}

fn pins(root: &Path) -> BTreeMap<String, String> {
    let mut sources: Vec<PathBuf> = vec![
        root.join("Cargo.toml"),
        root.join("Cargo.lock"),
        root.join("examples/layout_smoke.rs"),
        crate::root().join("tools/xtask/src/qualify/layout.rs"),
    ];
    if let Ok(entries) = fs::read_dir(root.join("src")) {
        let mut names: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
        names.sort();
        sources.extend(names);
    }
    let mut pinned = BTreeMap::new();
    for path in sources {
        let relative = path
            .strip_prefix(root)
            .map(|relative| relative.to_string_lossy().into_owned())
            .unwrap_or_else(|_| path.to_string_lossy().into_owned());
        if let Ok(digest) = crate::sha256_file(&path) {
            pinned.insert(relative, digest);
        }
    }
    pinned
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

/// One mode's capture: exit code, verdict, source size and own-renderer digest.
pub fn capture(
    root: &Path,
    output: &Path,
    probe: &Path,
    mode: &str,
    timeout: Duration,
) -> io::Result<Value> {
    let folder = output.join(mode);
    private_dirs(&folder)?;
    let log = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(folder.join("private-run.log"))?;
    let mut child = Command::new(probe)
        .arg("--mode")
        .arg(mode)
        .arg("--screenshot")
        .arg(folder.join("own-window.png"))
        .arg("--report")
        .arg(folder.join("native.json"))
        .current_dir(root)
        .env_clear()
        .envs(environment(output))
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone()?))
        .stderr(Stdio::from(log))
        .spawn()?;
    let deadline = Instant::now() + timeout;
    while child.try_wait()?.is_none() {
        if Instant::now() >= deadline {
            let _ = child.kill();
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let status = child.wait()?;
    let code = crate::exit_code(status);
    let report: Value = fs::read_to_string(folder.join("native.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null);
    let expected_mode = if matches!(
        mode,
        "assistant-details-formatted" | "assistant-details-narrow"
    ) {
        "formatted"
    } else {
        "source"
    };
    let passed = code == 0
        && report["status"].as_str() == Some("passed")
        && report["effectsDispatched"].as_u64() == Some(0)
        && report["capturedCommandCount"].as_u64() == Some(0)
        && report["detailVisible"].as_bool() == Some(true)
        && report["detailMode"].as_str() == Some(expected_mode)
        && report["detailFormattedAvailable"].as_bool()
            == Some(mode != "assistant-details-fallback")
        && report["windowBackend"].as_str() == Some("wayland");
    let image = folder.join("own-window.png");
    Ok(json!({
        "mode": mode,
        "exitCode": code,
        "passed": passed,
        "sourceBytes": report.get("detailSourceBytes").cloned().unwrap_or(Value::Null),
        "imageSha256": crate::sha256_file(&image).ok(),
    }))
}

/// `xtask qualify details` — four scripted detail captures under one evidence root.
pub fn run(app: &Path) -> i32 {
    let evidence = app.join("evidence");
    if let Err(error) = private_dirs(&evidence) {
        eprintln!("cannot create app/evidence: {error}");
        return 1;
    }
    let output = match crate::tempdir::make_temp_dir(&evidence, "formatted-details-paint-") {
        Ok(output) => output,
        Err(error) => {
            eprintln!("cannot create evidence root: {error}");
            return 1;
        }
    };
    println!("EVIDENCE_ROOT={}", output.display());
    let probe_source = app.join("target/release/examples/layout_smoke");
    let probe = output.join("layout-probe");
    if let Err(error) = fs::copy(&probe_source, &probe) {
        eprintln!("cannot freeze the layout probe: {error}");
        return 1;
    }
    let _ = fs::set_permissions(&probe, fs::Permissions::from_mode(0o700));
    for name in ["home", "config", "data", "cache"] {
        let _ = private_dirs(&output.join(name));
    }
    let before = pins(app);
    let mut cases = Vec::new();
    for mode in MODES {
        match capture(app, &output, &probe, mode, Duration::from_secs(20)) {
            Ok(case) => {
                println!(
                    "{mode}: passed={} exit={}",
                    case["passed"], case["exitCode"]
                );
                cases.push(case);
            }
            Err(error) => {
                eprintln!("{mode}: {error}");
                return 1;
            }
        }
    }
    let after = pins(app);
    let passed = cases
        .iter()
        .all(|case| case["passed"].as_bool() == Some(true));
    let summary = json!({
        "scope": "PUBLIC bounded native basic-formatted frozen assistant display Details; actual App view and scripted mode update; exact native control bounds and own-renderer capture",
        "passed": passed,
        "cases": cases,
        "probeSha256": crate::sha256_file(&probe).ok(),
        "probeBytes": fs::metadata(&probe).map(|meta| meta.len()).unwrap_or_default(),
        "sourceSha256": before,
        "sourceUnchangedDuringCaptures": before == after,
        "parentPaintInspectionComplete": false,
        "hostStarted": false,
        "businessExecutorStarted": false,
        "clipboardExecutorStarted": false,
        "desktopCapture": false,
        "physicalInputQualified": false,
        "compositorFractionalScalingQualified": false,
        "fullMarkdownQualified": false,
        "fullCompiledSourceClosureQualified": false,
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
    if passed { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tempdir::make_temp_dir;

    fn fake_probe(base: &Path) -> PathBuf {
        let probe = base.join("layout-probe");
        fs::write(
            &probe,
            "#!/bin/sh\n\
             while [ $# -gt 0 ]; do\n\
               case \"$1\" in\n\
                 --mode) mode=\"$2\"; shift 2;;\n\
                 --report) report=\"$2\"; shift 2;;\n\
                 --screenshot) shot=\"$2\"; shift 2;;\n\
                 *) shift;;\n\
               esac\n\
             done\n\
             case \"$mode\" in\n\
               *-formatted|*-narrow) detail=formatted;;\n\
               *) detail=source;;\n\
             esac\n\
             printf '{\"status\":\"passed\",\"effectsDispatched\":0,\"capturedCommandCount\":0,\"detailVisible\":true,\"detailMode\":\"%s\",\"detailFormattedAvailable\":true,\"windowBackend\":\"wayland\",\"detailSourceBytes\":42}\n' \"$detail\" > \"$report\"\n\
             printf 'PUBLIC png\\n' > \"$shot\"\n",
        )
        .unwrap();
        fs::set_permissions(&probe, fs::Permissions::from_mode(0o755)).unwrap();
        probe
    }

    #[test]
    fn a_capture_reads_the_mode_verdict_and_digest() {
        let base = make_temp_dir(&std::env::temp_dir(), "details-").unwrap();
        let probe = fake_probe(&base);
        let output = base.join("out");
        fs::create_dir(&output).unwrap();
        let case = capture(
            &base,
            &output,
            &probe,
            "assistant-details-formatted",
            Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(case["passed"].as_bool(), Some(true), "{case}");
        assert_eq!(case["sourceBytes"].as_u64(), Some(42));
        assert_eq!(case["imageSha256"].as_str().map(str::len), Some(64));
        fs::remove_dir_all(base).unwrap();
    }
}

const SUBMENU_MODES: [&str; 9] = [
    "model-popup",
    "model-submenu",
    "effort-submenu",
    "speed-submenu",
    "output-submenu",
    "model-submenu-tiny",
    "effort-submenu-tiny",
    "model-submenu-empty",
    "model-popup-long",
];

/// The submenu verdict, separated so it can be reasoned about without a run.
pub fn submenu_passed(exit_code: i32, report: &Value) -> bool {
    exit_code == 0
        && report["status"].as_str() == Some("passed")
        && report["effectsDispatched"].as_u64() == Some(0)
        && report["publicSource"].as_bool() == Some(true)
        && report["fixture"]["modelCatalogRequested"].as_bool() != Some(true)
}

/// `xtask qualify submenus` — the nine click-open model/effort/speed/output panes.
pub fn submenus(app: &Path) -> i32 {
    let evidence = app.join("evidence");
    if let Err(error) = private_dirs(&evidence) {
        eprintln!("cannot create app/evidence: {error}");
        return 1;
    }
    let output = match crate::tempdir::make_temp_dir(&evidence, "model-submenu-paint-") {
        Ok(output) => output,
        Err(error) => {
            eprintln!("cannot create evidence root: {error}");
            return 1;
        }
    };
    println!("EVIDENCE={}", output.display());
    let artifacts = output.join("compiled-snapshot");
    if let Err(error) = private_dirs(&artifacts) {
        eprintln!("cannot create the artifact snapshot: {error}");
        return 1;
    }
    for (source, name) in [
        ("target/release/examples/layout_smoke", "layout_smoke"),
        ("target/release/dsh-native-app", "dsh-native-app"),
    ] {
        if let Err(error) = fs::copy(app.join(source), artifacts.join(name)) {
            eprintln!("cannot freeze {source}: {error}");
            return 1;
        }
    }
    for name in ["home", "config", "data", "cache"] {
        let _ = private_dirs(&output.join(name));
    }
    let before = pins(app);
    let mut results = Vec::new();
    for mode in SUBMENU_MODES {
        let folder = output.join(mode);
        if let Err(error) = private_dirs(&folder) {
            eprintln!("cannot create {mode}: {error}");
            return 1;
        }
        let Ok(log) = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(folder.join("run.log"))
        else {
            eprintln!("cannot create the log for {mode}");
            return 1;
        };
        let Ok(mut child) = Command::new(artifacts.join("layout_smoke"))
            .arg("--mode")
            .arg(mode)
            .arg("--screenshot")
            .arg(folder.join("own-window.png"))
            .arg("--report")
            .arg(folder.join("result.json"))
            .current_dir(app)
            .env_clear()
            .envs(environment(&output))
            .stdin(Stdio::null())
            .stdout(Stdio::from(log.try_clone().expect("log clone")))
            .stderr(Stdio::from(log))
            .spawn()
        else {
            eprintln!("cannot start {mode}");
            return 1;
        };
        let deadline = Instant::now() + std::time::Duration::from_secs(25);
        while child.try_wait().ok().flatten().is_none() {
            if Instant::now() >= deadline {
                let _ = child.kill();
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let code = child.wait().map(crate::exit_code).unwrap_or(-1);
        let report: Value = fs::read_to_string(folder.join("result.json"))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or(Value::Null);
        let passed = submenu_passed(code, &report);
        println!("{mode}: exit={code}, passed={passed}");
        results.push(json!({
            "mode": mode,
            "exitCode": code,
            "passed": passed,
            "imageSha256": crate::sha256_file(&folder.join("own-window.png")).ok(),
        }));
    }
    let after = pins(app);
    let default_release = artifacts.join("dsh-native-app");
    let summary = json!({
        "scope": "PUBLIC native click-open option panes",
        "defaultRelease": "compiled-snapshot/dsh-native-app",
        "defaultReleaseSha256": crate::sha256_file(&default_release).ok(),
        "defaultReleaseBytes": fs::metadata(&default_release).map(|meta| meta.len()).unwrap_or_default(),
        "fixtureReleaseSha256": crate::sha256_file(&artifacts.join("layout_smoke")).ok(),
        "sourceSha256": before,
        "sourceUnchangedDuringCaptures": before == after,
        "cases": results,
        "parentPaintInspectionComplete": false,
        "hostStarted": false,
        "modelCatalogRequested": false,
        "modelPrompts": 0,
        "physicalInputQualified": false,
        "fullCompiledSourceClosureQualified": false,
    });
    if let Err(error) = private_json(&output.join("qualification.json"), &summary) {
        eprintln!("cannot write the qualification: {error}");
        return 1;
    }
    let passed = summary["cases"].as_array().is_some_and(|cases| {
        cases
            .iter()
            .all(|case| case["passed"].as_bool() == Some(true))
    });
    if passed { 0 } else { 1 }
}

#[cfg(test)]
mod submenu_tests {
    use super::*;

    fn report() -> Value {
        json!({"status": "passed", "effectsDispatched": 0, "publicSource": true,
               "fixture": {"modelCatalogRequested": false}})
    }

    #[test]
    fn a_submenu_pane_must_dispatch_nothing_and_request_no_catalog() {
        assert!(submenu_passed(0, &report()));
        assert!(!submenu_passed(1, &report()));
        let mut catalogued = report();
        catalogued["fixture"]["modelCatalogRequested"] = json!(true);
        assert!(
            !submenu_passed(0, &catalogued),
            "a catalog request is not a public fixture"
        );
        let mut dispatched = report();
        dispatched["effectsDispatched"] = json!(1);
        assert!(!submenu_passed(0, &dispatched));
        let mut private = report();
        private["publicSource"] = json!(false);
        assert!(!submenu_passed(0, &private));
    }
}
