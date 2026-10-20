//! Qualify an explicit native `--node` using a private interpreter copy.
//!
//! Port of `scripts/qualify-explicit-node.py`. The node identity is a point
//! observation of an already-pinned pid, not an exec-time binding.
use super::composed::{self, Key, Owned, cleanup_owned, identity, observe_descendants, pin_root};
use super::devpackage::{APP_ID, capture_valid, valid_app};
use super::registry;
use crate::compositor;
use crate::launcher;
use std::os::unix::process::CommandExt;
use std::path::Path;

/// The caller's pinned identity must still be that pid, its `exe` must be this
/// exact path, and the executable inode must match.
pub fn node_image_matches(pid: i32, key: Key, node: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let Some(before) = crate::proc::stat(Path::new("/proc"), pid) else {
        return false;
    };
    if identity(&before) != key {
        return false;
    }
    let image = Path::new("/proc").join(pid.to_string()).join("exe");
    let Ok(link) = std::fs::read_link(&image) else {
        return false;
    };
    if link != node {
        return false;
    }
    let (Ok(actual), Ok(expected)) = (std::fs::metadata(&image), node.metadata()) else {
        return false;
    };
    if !expected.is_file() || (actual.dev(), actual.ino()) != (expected.dev(), expected.ino()) {
        return false;
    }
    crate::proc::stat(Path::new("/proc"), pid)
        .map(|after| identity(&after) == key)
        .unwrap_or(false)
}

/// `xtask qualify explicit-node --binary PATH --output NEW_DIR [--scale]`.
pub fn run(root: &Path, binary_arg: &Path, requested: &Path, scale: f64) -> i32 {
    let started = std::time::Instant::now();
    let Ok(binary) = binary_arg.canonicalize() else {
        eprintln!("Explicit-Node qualification refused or failed.");
        return 1;
    };
    use std::os::unix::fs::PermissionsExt;
    let executable = std::fs::metadata(&binary)
        .map(|info| info.is_file() && info.permissions().mode() & 0o111 != 0)
        .unwrap_or(false);
    if !executable {
        eprintln!("Explicit-Node qualification refused or failed.");
        return 1;
    }
    let Ok(before_binary) = crate::sha256_file(&binary) else {
        eprintln!("Explicit-Node qualification refused or failed.");
        return 1;
    };
    let output = match super::prepare(root, requested) {
        Ok(output) => output,
        Err(message) => {
            eprintln!("Explicit-Node qualification refused or failed: {message}");
            return 1;
        }
    };
    let mut environment = super::environment(&output);
    environment.insert("PATH".into(), "/usr/bin:/bin".into());
    let node = output.join("node private copy · 世界");
    let system_node = Path::new("/usr/bin/node");
    let copy = std::fs::copy(system_node, &node);
    if copy.is_err() {
        eprintln!("Explicit-Node qualification refused or failed.");
        return 1;
    }
    let _ = std::fs::set_permissions(&node, std::fs::Permissions::from_mode(0o700));
    let (Ok(node_sha), Ok(system_sha)) =
        (crate::sha256_file(&node), crate::sha256_file(system_node))
    else {
        eprintln!("Explicit-Node qualification refused or failed.");
        return 1;
    };
    if node_sha != system_sha {
        eprintln!("Private Node copy mismatch");
        return 1;
    }
    let inherited: std::collections::BTreeMap<String, String> = std::env::vars().collect();
    let Ok(probe_environment) = launcher::environment(&output, &inherited, false) else {
        eprintln!("Explicit-Node qualification refused or failed.");
        return 1;
    };
    let Ok(version) = launcher::probe_node(&probe_environment, Some(&node)) else {
        eprintln!("Explicit-Node qualification refused or failed.");
        return 1;
    };
    let Some(runtime) = root
        .parent()
        .map(|parent| parent.join("deepseek-harness-linux/apps/cli"))
        .and_then(|path| path.canonicalize().ok())
    else {
        eprintln!("Explicit-Node qualification refused or failed.");
        return 1;
    };
    let spawned = std::process::Command::new(&binary)
        .arg("--runtime")
        .arg(&runtime)
        .arg("--expected-version")
        .arg("0.2.1-alpha.1")
        .arg("--home")
        .arg(output.join("harness"))
        .arg("--cwd")
        .arg(output.join("workspace"))
        .arg("--user-home")
        .arg(output.join("user"))
        .arg("--node")
        .arg(&node)
        .arg("--scale")
        .arg(format!("{scale}"))
        .arg("--smoke-new-session")
        .arg("--exit-after-seconds")
        .arg("8")
        .arg("--smoke-report")
        .arg(output.join("app.json"))
        .arg("--smoke-screenshot")
        .arg(output.join("own-window.png"))
        .env_clear()
        .envs(&environment)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .process_group(0)
        .spawn();
    let Ok(child) = spawned else {
        eprintln!("Explicit-Node qualification refused or failed.");
        return 1;
    };
    let pid = child.id();
    let mut owned = Owned {
        child,
        status: None,
    };
    let owner = std::process::id() as i32;
    let mut pidfds: std::collections::BTreeMap<Key, i32> = std::collections::BTreeMap::new();
    let mut observed: std::collections::BTreeSet<Key> = std::collections::BTreeSet::new();
    let mut node_identities: std::collections::BTreeSet<Key> = std::collections::BTreeSet::new();
    let mut root_pinned = false;
    let mut forced = false;
    let mut facts: Option<serde_json::Value> = None;
    let mut failure: Option<&'static str> = None;
    let mut observation_failures = 0u64;
    let mut root: Option<crate::proc::Stat> = None;
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
        let deadline = started + std::time::Duration::from_secs(45);
        loop {
            if owned.poll().ok().flatten().is_some() {
                break;
            }
            observation_failures +=
                observe_descendants(&mut owned, pinned, &mut pidfds, &mut observed, owner);
            for key in pidfds.keys() {
                if *key != identity(pinned) && node_image_matches(key.0, *key, &node) {
                    node_identities.insert(*key);
                }
            }
            if composed::INTERRUPTED.load(std::sync::atomic::Ordering::SeqCst)
                || std::time::Instant::now() >= deadline
            {
                forced = true;
                break;
            }
            if facts.is_none()
                && let Some(candidate) = compositor::window_facts(pid as i32)
                && candidate["mapped"].as_bool() == Some(true)
            {
                facts = Some(serde_json::Value::Object(candidate));
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
    let app = registry::read_app(&output);
    let valid = valid_app(&app, pid as u64, scale);
    let frame = capture_valid(&output, &app);
    let unchanged = crate::sha256_file(&binary).is_ok_and(|hash| hash == before_binary)
        && crate::sha256_file(&node).is_ok_and(|hash| hash == node_sha);
    let passed = valid
        && frame
        && unchanged
        && !node_identities.is_empty()
        && owned.code() == 0
        && !forced
        && failure.is_none()
        && observation_failures == 0
        && cleanup.observed_cleanup_complete
        && cleanup.remaining_before_external_cleanup.is_empty()
        && facts.as_ref().is_some_and(|facts| {
            facts["pid"].as_i64() == Some(pid as i64)
                && facts["class"].as_str() == Some(APP_ID)
                && facts["xwayland"].as_bool() == Some(false)
        });
    let report = serde_json::json!({
        "status": if passed { "passed" } else { "failed" },
        "scope": "Qualify explicit native --node using a private interpreter copy and fresh keyless Host.",
        "nativeSha256": before_binary,
        "privateNodeSha256": node_sha,
        "privateNodeVersionObserved": version,
        "privateNodeCopyMatchesSystemBytes": true,
        "explicitNodeArgContainedSpacesAndUnicode": true,
        "ownedNodeExecutablePathAndInodeMatched": !node_identities.is_empty(),
        "observedMatchingNodeIdentityCount": node_identities.len(),
        "nodeExecutablePathRedacted": true,
        "binariesUnchanged": unchanged,
        "exitCode": owned.code(),
        "forced": forced,
        "failureCode": failure,
        "ownedWindow": facts,
        "app": app,
        "appEvidenceVerified": valid,
        "completeOwnRendererCaptureVerified": frame,
        "ownershipObservationFailures": observation_failures,
        "observedOwnedProcessCount": observed.len(),
        "cleanup": &cleanup,
        "elapsedWallSeconds": started.elapsed().as_secs_f64(),
        "paintInspected": false,
        "containmentScope": "observed pidfd-pinned identities; executable path/inode checks are point observations, not exec-time binding",
        "nodeVersionPolicyAddedToProduct": false,
        "nodeRuntimeDependenciesBundled": false,
        "archiveRebuilt": false,
        "realCredentialsUsed": false,
        "actualModelCalls": 0,
        "physicalNativeInputQualified": false,
        "clipboardTasksExecuted": false,
        "installedRuntimeDataOrDesktopSettingsChanged": false,
        "fullDesktopParity": false,
    });
    if let Err(error) = super::private_json(&output.join("result.json"), &report) {
        eprintln!("cannot write the explicit-node report: {error}");
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
    use crate::proc;

    #[test]
    fn only_a_matching_identity_path_and_inode_admits_a_point_observation() {
        let pid = std::process::id() as i32;
        let stat = proc::stat(Path::new("/proc"), pid).expect("our own process");
        let key = identity(&stat);
        let executable = Path::new("/proc/self/exe")
            .canonicalize()
            .expect("our executable");
        assert!(node_image_matches(pid, key, &executable));
        // The same executable at another path is not the selected path.
        assert!(!node_image_matches(pid, key, Path::new("/usr/bin/node")));
        // A stale identity for the same pid is refused before the executable is read.
        let stale = (key.0, key.1 + 1);
        assert!(!node_image_matches(pid, stale, &executable));
        // A missing process is refused rather than throwing.
        assert!(!node_image_matches(2_000_000_000, key, &executable));
    }
}
