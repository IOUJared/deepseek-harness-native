//! Relocated development launcher → native → isolated Host keyless lifecycle.
//!
//! Report and capture contract of `scripts/qualify-development-package.py`.
use super::registry::{self, MAX_PNG_BYTES, png_dimensions, private_regular};
use crate::compositor;
use serde_json::Value;
use std::path::Path;

pub const APP_ID: &str = "ai.deepseek.harness.native.app";

/// The exact owned stop both the launcher and the native side must report.
fn stopped(app: &Value) -> bool {
    let stop = &app["stop"];
    stop["exited"].as_bool() == Some(true)
        && stop["graceful"].as_bool() == Some(true)
        && stop["containmentUnknown"].as_bool() == Some(false)
        && stop["observedDescendantsRemaining"].as_i64() == Some(0)
        && stop["exitCode"].as_i64() == Some(0)
}

pub fn valid_app(app: &Value, pid: u64, scale: f64) -> bool {
    app["status"].as_str() == Some("passed")
        && app["pid"].as_u64() == Some(pid)
        && app["applicationId"].as_str() == Some(APP_ID)
        && app["applicationScale"].as_f64() == Some(scale)
        && app["windowBackend"].as_str() == Some("wayland")
        && app["modelPrompts"].as_i64() == Some(0)
        && app["modelCatalogRequested"].as_bool() == Some(false)
        && [
            "coreReady",
            "workspaceOpened",
            "sessionCreated",
            "snapshotFoldValid",
            "screenshotRequested",
            "screenshotSaved",
            "deadlineReached",
        ]
        .iter()
        .all(|key| app[*key].as_bool() == Some(true))
        && app["workspaceCount"].as_i64() == Some(1)
        && app["sessionCount"].as_i64() == Some(1)
        && app["snapshotCursor"]
            .as_i64()
            .is_some_and(|cursor| cursor >= 0)
        && app["stopError"].as_bool() == Some(false)
        && stopped(app)
}

/// The own-window capture must be a private PNG matching the reported size.
pub fn capture_valid(evidence: &Path, app: &Value) -> bool {
    let Some(data) = private_regular(&evidence.join("own-window.png"), 16 * 1024 * 1024) else {
        return false;
    };
    let Some((width, height)) = png_dimensions(&data) else {
        return false;
    };
    app["screenshotPhysicalSize"] == serde_json::json!([width, height])
        && data.len() as u64 <= MAX_PNG_BYTES
}

use super::composed::{self, Key, Owned, cleanup_owned, identity, observe_descendants, pin_root};
use crate::launcher;
use crate::proc::{self, TreeSample};
use std::collections::BTreeSet;
use std::os::unix::process::CommandExt;

const MAX_RESOURCE_SAMPLES: usize = 256;

/// Endpoint CPU is meaningful only for a stable observed identity set/window.
pub fn warm_cpu_window(samples: &[TreeSample]) -> serde_json::Value {
    let unknown = serde_json::json!({
        "sampleCount": samples.len(), "identitiesStable": false, "seconds": null,
        "cpuPercentOneCore": null, "oneTickPercentOneCore": null,
    });
    if !(2..=MAX_RESOURCE_SAMPLES).contains(&samples.len()) {
        return unknown;
    }
    let identities = &samples[0].ticks;
    if identities.is_empty()
        || samples
            .iter()
            .any(|sample| sample.ticks.keys().ne(identities.keys()))
    {
        return unknown;
    }
    for pair in samples.windows(2) {
        let (previous, current) = (&pair[0], &pair[1]);
        if current.at <= previous.at
            || identities
                .keys()
                .any(|key| current.ticks.get(key) < previous.ticks.get(key))
        {
            return unknown;
        }
    }
    let elapsed = samples[samples.len() - 1].at - samples[0].at;
    if elapsed <= 0.0 {
        return unknown;
    }
    serde_json::json!({
        "sampleCount": samples.len(),
        "identitiesStable": true,
        "seconds": elapsed,
        "cpuPercentOneCore": proc::cpu_percent(&samples[0], &samples[samples.len() - 1], proc::clock_ticks()),
        "oneTickPercentOneCore": 100.0 / proc::clock_ticks() / elapsed,
    })
}

fn median(samples: &[TreeSample], key: &str) -> Option<f64> {
    let values: Vec<f64> = samples
        .iter()
        .filter_map(|sample| match key {
            "rss_kib" => Some(sample.rss_kib as f64),
            "pss_kib" => sample.pss_kib.map(|value| value as f64),
            _ => None,
        })
        .collect();
    proc::median(&values)
}

fn node_identity_matches(pid: i32, key: Key, node: &Path) -> bool {
    super::explicit::node_image_matches(pid, key, node)
}

#[allow(dead_code)]
fn unused_legacy_matcher(pid: i32, node: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let Ok(executable) = std::fs::read_link(format!("/proc/{pid}/exe")) else {
        return false;
    };
    let Ok(target) = executable.canonicalize() else {
        return false;
    };
    let (Ok(opened), Ok(expected)) = (target.metadata(), node.metadata()) else {
        return false;
    };
    opened.dev() == expected.dev() && opened.ino() == expected.ino()
}

/// `xtask qualify development-package`.
pub fn run(
    root: &Path,
    package_dir: &Path,
    requested: &Path,
    node_value: Option<&Path>,
    scale: f64,
) -> i32 {
    let started = std::time::Instant::now();
    let output = match super::prepare(root, requested) {
        Ok(output) => output,
        Err(message) => {
            eprintln!("development package failed: {message}");
            return 1;
        }
    };
    let evidence = output.join("evidence");
    if std::fs::create_dir(&evidence).is_err() {
        eprintln!("development package failed: evidence directory");
        return 1;
    }
    let _ = std::fs::set_permissions(
        &evidence,
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    );
    let data = output.join("native-container");
    let Ok((manifest, binary)) = launcher::package(package_dir) else {
        eprintln!("development package failed: unsupported package");
        return 1;
    };
    let Ok(before) = launcher::binary_digest(&binary) else {
        eprintln!("development package failed: native binary");
        return 1;
    };
    let mut child_environment = super::environment(&output);
    child_environment.insert("PATH".into(), "/usr/bin:/bin".into());
    let Ok(node) = launcher::selected_node(
        node_value
            .map(|path| path.to_string_lossy().into_owned())
            .as_deref(),
    ) else {
        eprintln!("development package failed: node");
        return 1;
    };
    let inherited: std::collections::BTreeMap<String, String> = std::env::vars().collect();
    let Ok(probe_environment) = launcher::environment(&output, &inherited, false) else {
        eprintln!("development package failed: environment");
        return 1;
    };
    let Ok(node_version) = launcher::probe_node(&probe_environment, Some(&node)) else {
        eprintln!("development package failed: node version");
        return 1;
    };
    let Some(runtime) = root
        .parent()
        .map(|parent| parent.join("deepseek-harness-linux/apps/cli"))
        .and_then(|path| path.canonicalize().ok())
    else {
        eprintln!("development package failed: runtime");
        return 1;
    };
    let launcher_binary = package_dir.join("launch");
    let mut command = std::process::Command::new(&launcher_binary);
    command
        .arg("--runtime")
        .arg(&runtime)
        .arg("--workspace")
        .arg(output.join("workspace"))
        .arg("--user-home")
        .arg(output.join("user"))
        .arg("--data-home")
        .arg(&data)
        .arg("--qualify-evidence")
        .arg(&evidence)
        .arg("--scale")
        .arg(format!("{scale}"));
    if node_value.is_some() {
        command.arg("--node").arg(&node);
    }
    let spawned = command
        .env_clear()
        .envs(&child_environment)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .process_group(0)
        .spawn();
    let Ok(child) = spawned else {
        eprintln!("development package failed: cannot start the launcher");
        return 1;
    };
    let pid = child.id();
    let mut owned = Owned {
        child,
        status: None,
    };
    let owner = std::process::id() as i32;
    let mut pidfds: std::collections::BTreeMap<Key, i32> = std::collections::BTreeMap::new();
    let mut observed: BTreeSet<Key> = BTreeSet::new();
    let mut samples: Vec<(f64, TreeSample)> = Vec::new();
    let mut facts: Option<serde_json::Value> = None;
    let mut failure: Option<&'static str> = None;
    let mut mapped_ms: Option<f64> = None;
    let mut forced = false;
    let mut observation_failures = 0u64;
    let mut node_identities = 0usize;
    let mut root_pinned = false;
    let mut root: Option<proc::Stat> = None;
    match pin_root(&mut owned, owner) {
        Err(()) => {
            failure = Some("root-pin-failed");
            forced = true;
        }
        Ok((pinned, fd)) => {
            root_pinned = true;
            pidfds.insert(identity(&pinned), fd);
            observed.insert(identity(&pinned));
            root = Some(pinned);
        }
    }
    if let Some(pinned) = root.as_ref() {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(45);
        loop {
            if owned.poll().ok().flatten().is_some() {
                break;
            }
            observation_failures +=
                observe_descendants(&mut owned, pinned, &mut pidfds, &mut observed, owner);
            if node_value.is_some() {
                for key in pidfds.keys() {
                    if *key != identity(pinned) && node_identity_matches(key.0, *key, &node) {
                        node_identities += 1;
                    }
                }
            }
            if composed::INTERRUPTED.load(std::sync::atomic::Ordering::SeqCst)
                || std::time::Instant::now() >= deadline
            {
                forced = true;
                break;
            }
            if samples.len() >= MAX_RESOURCE_SAMPLES {
                failure = Some("resource-sample-bound-exceeded");
                forced = true;
                break;
            }
            let tree = proc::sample_tree(
                std::path::Path::new("/proc"),
                pid as i32,
                pinned.start_ticks,
            );
            samples.push((started.elapsed().as_secs_f64() * 1000.0, tree));
            if facts.is_none()
                && let Some(candidate) = compositor::window_facts(pid as i32)
                && candidate["mapped"].as_bool() == Some(true)
            {
                facts = Some(serde_json::Value::Object(candidate));
                mapped_ms = Some(started.elapsed().as_secs_f64() * 1000.0);
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }
    let cleanup = cleanup_owned(&mut owned, &pidfds, root_pinned);
    for fd in pidfds.values() {
        // Safe: each descriptor was opened for one identity this run and closed once.
        unsafe {
            libc::close(*fd);
        }
    }
    let forced = forced || cleanup.root_was_running_at_cleanup;
    let app = registry::read_app(&evidence);
    let valid = valid_app(&app, pid as u64, scale);
    let frame = capture_valid(&evidence, &app);
    let unchanged = launcher::binary_digest(&binary).as_deref() == Ok(before.as_str());
    let marker = launcher::marker_for(&manifest);
    let marker_valid = launcher::inspect_state(&data, &marker)
        .map(|state| state == launcher::State::Owned)
        .unwrap_or(false);
    let lock_released = launcher::acquire_state(&data, &marker).map(drop).is_ok();
    let passed = valid
        && frame
        && unchanged
        && marker_valid
        && lock_released
        && owned.code() == 0
        && (node_value.is_none() || node_identities > 0)
        && failure.is_none()
        && !forced
        && observation_failures == 0
        && cleanup.observed_cleanup_complete
        && cleanup.remaining_before_external_cleanup.is_empty()
        && facts.as_ref().is_some_and(|facts| {
            facts["class"].as_str() == Some(APP_ID) && facts["xwayland"].as_bool() == Some(false)
        });
    // The warm window is 3–6.5 s after start, measured when the sample was taken.
    let warm: Vec<TreeSample> = samples
        .iter()
        .filter(|(elapsed, sample)| (3000.0..=6500.0).contains(elapsed) && sample.process_count > 0)
        .map(|(_, sample)| sample.clone())
        .collect();
    let report = serde_json::json!({
        "status": if passed { "passed" } else { "failed" },
        "scope": "Own a relocated development launcher → native → isolated Host keyless lifecycle.",
        "nativeSha256": before,
        "packageRuntimeMode": "external-explicit",
        "runtimeBundled": false,
        "nodeVersionObserved": node_version,
        "nodeSelectionMode": if node_value.is_some() { "explicit-absolute" } else { "system-default" },
        "explicitNodeExecutablePathAndInodeMatched": if node_value.is_some() { serde_json::json!(node_identities > 0) } else { serde_json::Value::Null },
        "launcherExecPidMatchesNativeWindow": facts.as_ref().is_some_and(|facts| facts["pid"].as_i64() == Some(pid as i64)),
        "binaryUnchanged": unchanged,
        "failureCode": failure,
        "forced": forced,
        "exitCode": owned.code(),
        "ownedWindow": facts,
        "observedOwnedProcessCount": observed.len(),
        "ownershipObservationFailures": observation_failures,
        "cleanup": &cleanup,
        "app": app,
        "appEvidenceVerified": valid,
        "completeOwnRendererCaptureVerified": frame,
        "nativeContainerAndHostPrivateMarkerVerified": marker_valid,
        "inheritedLockReleasedAfterOwnedStop": lock_released,
        "firstOwnWindowMappedUpperBoundMs": mapped_ms,
        "fullTreeWarmMedianRssKiB": median(&warm, "rss_kib"),
        "fullTreeWarmMedianPssKiB": median(&warm, "pss_kib"),
        "warmSamples": warm.len(),
        "warmCpuWindow": warm_cpu_window(&warm),
        "resourceSampleBound": MAX_RESOURCE_SAMPLES,
        "performanceScope": "single fresh blank-profile eight-second integrated smoke; warm window 3–6.5s; sampled RSS/PSS, not representative long-chat benchmark",
        "elapsedWallSeconds": started.elapsed().as_secs_f64(),
        "hardTotalWallClockBoundClaimed": false,
        "containmentScope": "observed pidfd-pinned identities only",
        "primaryExecutableEntrypointQualified": passed,
        "physicalNativeInputQualified": false,
        "actualModelCalls": 0,
        "realCredentialsUsed": false,
        "installedRuntimeDataOrDesktopSettingsChanged": false,
        "paintInspected": false,
        "standaloneInstallerOrRuntimeRelocationQualified": false,
        "fullDesktopParity": false,
    });
    if let Err(error) = super::private_json(&output.join("result.json"), &report) {
        eprintln!("cannot write the development-package report: {error}");
        return 1;
    }
    println!(
        "{}",
        serde_json::json!({"status": report["status"], "report": output.join("result.json").to_string_lossy()})
    );
    if passed { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    fn app() -> Value {
        serde_json::json!({
            "status": "passed", "pid": 8100, "applicationId": APP_ID,
            "applicationScale": 1.0, "windowBackend": "wayland",
            "modelPrompts": 0, "modelCatalogRequested": false,
            "coreReady": true, "workspaceOpened": true, "sessionCreated": true,
            "workspaceCount": 1, "sessionCount": 1, "snapshotFoldValid": true,
            "snapshotCursor": 0, "screenshotRequested": true, "screenshotSaved": true,
            "screenshotPhysicalSize": [608, 448], "stopError": false, "deadlineReached": true,
            "stop": {"exited": true, "graceful": true, "containmentUnknown": false,
                     "observedDescendantsRemaining": 0, "exitCode": 0},
        })
    }

    fn png() -> Vec<u8> {
        fn chunk(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
            let mut out = Vec::new();
            out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            out.extend_from_slice(kind);
            out.extend_from_slice(payload);
            let mut crc = 0xffff_ffffu32;
            for byte in kind.iter().chain(payload) {
                crc ^= *byte as u32;
                for _ in 0..8 {
                    crc = if crc & 1 != 0 {
                        (crc >> 1) ^ 0xedb8_8320
                    } else {
                        crc >> 1
                    };
                }
            }
            out.extend_from_slice(&(!crc).to_be_bytes());
            out
        }
        let (width, height) = (608u32, 448u32);
        let stride = width as usize * 4 + 1;
        let rows = vec![0u8; stride * height as usize];
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&rows).unwrap();
        let idat = encoder.finish().unwrap();
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        out.extend(chunk(b"IHDR", &ihdr));
        out.extend(chunk(b"IDAT", &idat));
        out.extend(chunk(b"IEND", &[]));
        out
    }

    #[test]
    fn a_relocated_launch_needs_its_owned_stop_and_matching_capture() {
        let base = crate::tempdir::make_temp_dir(&std::env::temp_dir(), "devpackage-").unwrap();
        fs::write(base.join("own-window.png"), png()).unwrap();
        fs::set_permissions(
            base.join("own-window.png"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        assert!(valid_app(&app(), 8100, 1.0));
        assert!(capture_valid(&base, &app()));
        // A substituted pid, a missing core, a second workspace and a foreign app id fail.
        let mut moved = app();
        moved["pid"] = serde_json::json!(8101);
        assert!(!valid_app(&moved, 8100, 1.0));
        let mut core = app();
        core["coreReady"] = serde_json::json!(false);
        assert!(!valid_app(&core, 8100, 1.0));
        let mut workspaces = app();
        workspaces["workspaceCount"] = serde_json::json!(2);
        assert!(!valid_app(&workspaces, 8100, 1.0));
        let mut foreign = app();
        foreign["applicationId"] = serde_json::json!("ai.deepseek.harness.native.other");
        assert!(!valid_app(&foreign, 8100, 1.0));
        // A non-graceful stop or a negative cursor fails.
        let mut abrupt = app();
        abrupt["stop"]["graceful"] = serde_json::json!(false);
        assert!(!valid_app(&abrupt, 8100, 1.0));
        let mut cursor = app();
        cursor["snapshotCursor"] = serde_json::json!(-1);
        assert!(!valid_app(&cursor, 8100, 1.0));
        // The capture must match the reported size and stay private.
        let mut size = app();
        size["screenshotPhysicalSize"] = serde_json::json!([448, 608]);
        assert!(!capture_valid(&base, &size));
        fs::set_permissions(
            base.join("own-window.png"),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert!(!capture_valid(&base, &app()));
        fs::remove_file(base.join("own-window.png")).unwrap();
        assert!(!capture_valid(&base, &app()));
        fs::remove_dir_all(base).unwrap();
    }
}

#[cfg(test)]
mod window_tests {
    use super::*;

    fn tree(at: f64, ticks: u64, processes: usize) -> TreeSample {
        TreeSample {
            at,
            loadavg: [0.0; 3],
            process_count: processes,
            rss_kib: 1024,
            pss_kib: Some(900),
            drm_vram_kib: None,
            ticks: std::collections::BTreeMap::from([("42:7".to_string(), ticks)]),
        }
    }

    #[test]
    fn a_warm_window_needs_stable_identities_and_increasing_ticks() {
        let samples = vec![tree(0.0, 10, 2), tree(1.0, 30, 2), tree(2.0, 50, 2)];
        let window = warm_cpu_window(&samples);
        assert_eq!(window["identitiesStable"], true);
        assert_eq!(window["sampleCount"], 3);
        assert!(window["cpuPercentOneCore"].as_f64().unwrap() > 0.0);
        // A single sample, a vanished identity, a reused pid and a reordered clock are unknown.
        assert_eq!(warm_cpu_window(&samples[..1])["identitiesStable"], false);
        let mut vanished = samples.clone();
        vanished[1].ticks.clear();
        assert_eq!(warm_cpu_window(&vanished)["identitiesStable"], false);
        let mut reversed = samples.clone();
        reversed[2].at = 0.5;
        assert_eq!(warm_cpu_window(&reversed)["identitiesStable"], false);
        let mut decreased = samples.clone();
        decreased[2].ticks.insert("42:7".to_string(), 10);
        assert_eq!(warm_cpu_window(&decreased)["identitiesStable"], false);
    }
}
