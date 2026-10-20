//! Owned PUBLIC/native-keyless control-layout captures, run serially.
//!
//! Port of `app/qualify_control_layout.py`. Never a desktop capture or model
//! work: the exact observed executables are frozen, sources are pinned as
//! observations (not a full compiler-closure claim), and the real-keyless run
//! uses the explicitly isolated same-version Alpha with zero credentials.
use super::private_json;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const MODES: [&str; 32] = [
    "wide-welcome",
    "narrow-welcome",
    "tiny-welcome",
    "menu",
    "navigation",
    "navigation-tiny",
    "collapsed-wide",
    "conversation",
    "long-model",
    "tiny-long-model",
    "warnings",
    "settings-general",
    "settings-codex",
    "information",
    "information-expanded",
    "tool-activity",
    "tool-activity-narrow",
    "tool-activity-tiny",
    "tool-activity-details",
    "chat-density",
    "assistant-details-source",
    "assistant-details-formatted",
    "assistant-details-narrow",
    "assistant-details-fallback",
    "file-path-review",
    "file-path-tiny",
    "file-ready",
    "file-ready-two",
    "file-unknown",
    "inbox-canceled",
    "title-controls",
    "title-controls-expanded",
];

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
        ("ICED_BACKEND", "wgpu".to_string()),
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
        (
            "HOME",
            output.join("user-home").to_string_lossy().into_owned(),
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
        app.join("examples/layout_smoke.rs"),
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

/// A public capture passes when the fixture reports itself passed, dispatched no
/// effects and is public source.
pub fn public_passed(exit_code: i32, report: &Value) -> bool {
    exit_code == 0
        && report["status"].as_str() == Some("passed")
        && report["effectsDispatched"].as_u64() == Some(0)
        && report["publicSource"].as_bool() == Some(true)
}

/// The real-keyless run passes only with a graceful, contained, native stop.
pub fn real_keyless_passed(exit_code: i32, report: &Value) -> bool {
    exit_code == 0
        && report["status"].as_str() == Some("passed")
        && report["modelPrompts"].as_u64() == Some(0)
        && report["modelCatalogRequested"].as_bool() == Some(false)
        && report["windowBackend"].as_str() == Some("wayland")
        && report["snapshotFoldValid"].as_bool() == Some(true)
        && report["stop"]["graceful"].as_bool() == Some(true)
        && report["stop"]["containmentUnknown"].as_bool() == Some(false)
}

fn run_logged(command: &mut Command, folder: &Path, timeout: Duration) -> io::Result<(i32, Value)> {
    let log = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(folder.join("private-run.log"))?;
    let mut child = command
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
    let code = crate::exit_code(child.wait()?);
    let report: Value = fs::read_to_string(folder.join("result.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null);
    Ok((code, report))
}

/// `xtask qualify control` — 32 public layout captures plus one real-keyless run.
pub fn run(app: &Path) -> i32 {
    let evidence = app.join("evidence");
    if let Err(error) = private_dirs(&evidence) {
        eprintln!("cannot create app/evidence: {error}");
        return 1;
    }
    let output = match crate::tempdir::make_temp_dir(&evidence, "control-layout-") {
        Ok(output) => output,
        Err(error) => {
            eprintln!("cannot create evidence root: {error}");
            return 1;
        }
    };
    println!("EVIDENCE={}", output.display());
    for name in ["user-home", "config", "data", "cache"] {
        let _ = private_dirs(&output.join(name));
    }
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
        let _ = fs::set_permissions(artifacts.join(name), fs::Permissions::from_mode(0o700));
    }
    let before = pins(app);
    let mut results = Vec::new();
    for mode in MODES {
        let folder = output.join(mode);
        if let Err(error) = private_dirs(&folder) {
            eprintln!("cannot create {mode}: {error}");
            return 1;
        }
        let mut command = Command::new(artifacts.join("layout_smoke"));
        command
            .arg("--mode")
            .arg(mode)
            .arg("--screenshot")
            .arg(folder.join("own-window.png"))
            .arg("--report")
            .arg(folder.join("result.json"))
            .current_dir(app)
            .env_clear()
            .envs(environment(&output));
        let (code, report) = match run_logged(&mut command, &folder, Duration::from_secs(20)) {
            Ok(outcome) => outcome,
            Err(error) => {
                eprintln!("{mode}: {error}");
                return 1;
            }
        };
        let passed = public_passed(code, &report);
        println!("{mode}: exit={code}, passed={passed}");
        results.push(json!({
            "mode": mode,
            "exitCode": code,
            "passed": passed,
            "imageSha256": crate::sha256_file(&folder.join("own-window.png")).ok(),
        }));
    }

    // Actual production binary against an explicitly isolated same-version Alpha.
    let folder = output.join("real-keyless");
    if let Err(error) = private_dirs(&folder) {
        eprintln!("cannot create real-keyless: {error}");
        return 1;
    }
    for name in ["native-home", "user-home", "workspace"] {
        let _ = private_dirs(&folder.join(name));
    }
    let runtime = app
        .parent()
        .and_then(Path::parent)
        .map(|parent| parent.join("deepseek-harness-linux/apps/cli"))
        .unwrap_or_else(|| PathBuf::from("deepseek-harness-linux/apps/cli"));
    let mut command = Command::new(artifacts.join("dsh-native-app"));
    command
        .arg("--runtime")
        .arg(&runtime)
        .arg("--expected-version")
        .arg("0.2.1-alpha.1")
        .arg("--home")
        .arg(folder.join("native-home"))
        .arg("--user-home")
        .arg(folder.join("user-home"))
        .arg("--cwd")
        .arg(folder.join("workspace"))
        .arg("--scale")
        .arg("1.0")
        .arg("--smoke-new-session")
        .arg("--exit-after-seconds")
        .arg("8")
        .arg("--smoke-report")
        .arg(folder.join("result.json"))
        .arg("--smoke-screenshot")
        .arg(folder.join("own-window.png"))
        .current_dir(app)
        .env_clear()
        .envs(environment(&output));
    let (code, report) = match run_logged(&mut command, &folder, Duration::from_secs(30)) {
        Ok(outcome) => outcome,
        Err(error) => {
            eprintln!("real-keyless: {error}");
            return 1;
        }
    };
    let real_pass = real_keyless_passed(code, &report);
    println!("real-keyless: exit={code}, passed={real_pass}");

    let after = pins(app);
    let manifest = json!({
        "scope": "native-control-layout-and-Harness-style-conversation",
        "defaultReleaseSha256": crate::sha256_file(&artifacts.join("dsh-native-app")).ok(),
        "defaultReleaseBytes": fs::metadata(artifacts.join("dsh-native-app")).map(|meta| meta.len()).unwrap_or_default(),
        "fixtureReleaseSha256": crate::sha256_file(&artifacts.join("layout_smoke")).ok(),
        "sourceSha256": before,
        "sourceUnchangedDuringCaptures": before == after,
        "fullCompiledSourceClosureQualified": false,
        "defaultReleaseArtifact": "compiled-snapshot/dsh-native-app",
        "fixtureReleaseArtifact": "compiled-snapshot/layout_smoke",
        "publicCases": results,
        "realKeyless": {
            "exitCode": code,
            "passed": real_pass,
            "imageSha256": crate::sha256_file(&folder.join("own-window.png")).ok(),
        },
        "parentPaintInspectionComplete": false,
        "physicalInputQualified": false,
        "realAccountQualified": false,
        "modelGenerationQualified": false,
        "compositorFractionalScalingQualified": false,
        "fullParityQualified": false,
    });
    if let Err(error) = private_json(&output.join("qualification.json"), &manifest) {
        eprintln!("cannot write the qualification: {error}");
        return 1;
    }
    println!(
        "QUALIFICATION={}",
        output.join("qualification.json").display()
    );
    let public_ok = manifest["publicCases"].as_array().is_some_and(|cases| {
        cases
            .iter()
            .all(|case| case["passed"].as_bool() == Some(true))
    });
    if public_ok && real_pass { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn public_report() -> Value {
        json!({"status": "passed", "effectsDispatched": 0, "publicSource": true})
    }

    fn keyless_report() -> Value {
        json!({
            "status": "passed", "modelPrompts": 0, "modelCatalogRequested": false,
            "windowBackend": "wayland", "snapshotFoldValid": true,
            "stop": {"graceful": true, "containmentUnknown": false},
        })
    }

    #[test]
    fn a_public_capture_must_dispatch_nothing_and_be_public_source() {
        assert!(public_passed(0, &public_report()));
        assert!(!public_passed(1, &public_report()));
        let mut dispatched = public_report();
        dispatched["effectsDispatched"] = json!(1);
        assert!(!public_passed(0, &dispatched));
        let mut private = public_report();
        private["publicSource"] = json!(false);
        assert!(!public_passed(0, &private));
    }

    #[test]
    fn a_real_keyless_run_must_stop_gracefully_without_a_catalog_request() {
        assert!(real_keyless_passed(0, &keyless_report()));
        let mut uncontained = keyless_report();
        uncontained["stop"]["containmentUnknown"] = json!(true);
        assert!(!real_keyless_passed(0, &uncontained));
        let mut catalogued = keyless_report();
        catalogued["modelCatalogRequested"] = json!(true);
        assert!(!real_keyless_passed(0, &catalogued));
        let mut no_fold = keyless_report();
        no_fold["snapshotFoldValid"] = json!(false);
        assert!(!real_keyless_passed(0, &no_fold));
        let mut forced = keyless_report();
        forced["stop"]["graceful"] = json!(false);
        assert!(!real_keyless_passed(0, &forced));
    }
}
