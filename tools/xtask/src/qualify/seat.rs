//! Reviewed opt-in PRIVATE headless Gamescope seat: the exact PUBLIC result contract.
//!
//! Port of the validator half of `scripts/qualify-private-native-seat.py`. The
//! seat spawn itself needs a headless Gamescope run and is not ported.
use super::upload::sha256_hex;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub const TEXT: &str = "public native keyboard check";
const MAX_EXECUTABLE: u64 = 128 * 1024 * 1024;

fn zero_stop() -> Value {
    serde_json::json!({"exited": true, "graceful": true, "containmentUnknown": false,
                       "observedDescendantsRemaining": 0, "exitCode": 0})
}

pub fn natural(value: &Value, minimum: u64, maximum: u64) -> bool {
    value
        .as_u64()
        .is_some_and(|number| (minimum..=maximum).contains(&number))
}

/// An explicit canonical executable whose bytes match its recorded digest.
pub fn executable(value: &Path, digest: &str) -> Result<Vec<u8>, String> {
    use std::os::unix::fs::PermissionsExt;
    let raw = value.to_string_lossy();
    if !value.is_absolute()
        || raw.as_ref() != value.canonicalize().unwrap_or_default().to_string_lossy()
        || digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("explicit-canonical-executable-and-sha256-required".into());
    }
    let before = std::fs::symlink_metadata(value).map_err(|_| "regular-executable-required")?;
    if !before.is_file() || before.permissions().mode() & 0o111 == 0 {
        return Err("regular-executable-required".into());
    }
    let data = crate::launcher::private_read(value, MAX_EXECUTABLE + 1)
        .map_err(|_| "executable-changed")?;
    if data.len() as u64 > MAX_EXECUTABLE || sha256_hex(&data) != digest {
        return Err("executable-sha256-mismatch".into());
    }
    Ok(data)
}

/// Copy the verified bytes into a private 0700 file.
pub fn copy_executable(path: &Path, data: &[u8]) -> Result<PathBuf, String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o700)
        .open(path)
        .map_err(|error| error.to_string())?;
    file.write_all(data).map_err(|error| error.to_string())?;
    Ok(path.to_path_buf())
}

/// The seat's ready report, before any keyboard input.
pub fn valid_ready(value: &Value) -> bool {
    let Some(display) = value["waylandDisplay"].as_str() else {
        return false;
    };
    let (Some(width), Some(height)) = (
        value["windowWidth"].as_u64(),
        value["windowHeight"].as_u64(),
    ) else {
        return false;
    };
    value["version"].as_i64() == Some(1)
        && value["publicOnly"].as_bool() == Some(true)
        && value["phase"].as_str() == Some("keyboard-ready")
        && value["nativeWayland"].as_bool() == Some(true)
        && value["composerEmpty"].as_bool() == Some(true)
        && value["promptCommands"].as_i64() == Some(0)
        && natural(&value["pid"], 1, (1 << 31) - 1)
        && display.strip_prefix("gamescope-").is_some_and(|tail| {
            !tail.is_empty() && tail.len() <= 3 && tail.bytes().all(|byte| byte.is_ascii_digit())
        })
        && (200..=4096).contains(&width)
        && (200..=4096).contains(&height)
        && natural(&value["pointerFocusX"], 1, width - 1)
        && natural(&value["pointerFocusY"], 1, height - 1)
}

/// The seat's completed report: the exact PUBLIC text typed in the real editor.
pub fn valid_result(value: &Value, pid: u64) -> bool {
    if value["version"].as_i64() != Some(1)
        || value["publicOnly"].as_bool() != Some(true)
        || value["phase"].as_str() != Some("complete")
        || value["pid"].as_u64() != Some(pid)
        || value["actualProductionEditor"].as_bool() != Some(true)
        || value["actualWaylandKeyboardEvents"].as_bool() != Some(true)
        || value["body"].as_str() != Some(TEXT)
        || value["keyPressed"].as_u64() != Some(TEXT.len() as u64)
        || value["promptCommands"].as_i64() != Some(0)
        || value["manualSendAllowed"].as_bool() != Some(false)
    {
        return false;
    }
    value["stop"] == zero_stop()
}

/// The private Wayland socket beside the isolated runtime.
pub fn private_socket_path(runtime: &Path, display: &str) -> Result<PathBuf, String> {
    let Some(tail) = display.strip_prefix("gamescope-") else {
        return Err("private-socket-name-refused".into());
    };
    if tail.is_empty() || tail.len() > 3 || !tail.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("private-socket-name-refused".into());
    }
    let path = runtime.join(format!("{display}-ei"));
    if !path.is_absolute() || path.as_os_str().len() >= 108 {
        return Err("private-socket-path-too-long".into());
    }
    Ok(path)
}

/// The short runtime manifest written beside the scratch tree.
pub fn valid_runtime_manifest(value: &Value, output: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let keys: Vec<&str> = value
        .as_object()
        .map(|object| object.keys().map(String::as_str).collect())
        .unwrap_or_default();
    if keys.len() != 6
        || ![
            "version",
            "publicOnly",
            "runtimePath",
            "runtimeStamp",
            "scratchPath",
            "evidenceRoot",
        ]
        .iter()
        .all(|key| keys.contains(key))
        || value["version"].as_i64() != Some(1)
        || value["publicOnly"].as_bool() != Some(true)
    {
        return false;
    }
    let (Some(scratch), Some(runtime), Some(evidence)) = (
        value["scratchPath"].as_str(),
        value["runtimePath"].as_str(),
        value["evidenceRoot"].as_str(),
    ) else {
        return false;
    };
    let (scratch, runtime, evidence) =
        (Path::new(scratch), Path::new(runtime), Path::new(evidence));
    let Ok(output) = output.canonicalize() else {
        return false;
    };
    if [scratch, runtime, evidence].iter().any(|path| {
        !path.is_absolute()
            || path
                .canonicalize()
                .map(|resolved| resolved != **path)
                .unwrap_or(true)
    }) || scratch.parent() != Some(Path::new("/tmp"))
        || !scratch
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("PUBLIC-native-seat-"))
        || runtime != scratch.join("runtime")
        || evidence != output
    {
        return false;
    }
    for path in [scratch, runtime, evidence] {
        let Ok(info) = std::fs::symlink_metadata(path) else {
            return false;
        };
        if !info.is_dir()
            || info.uid() != unsafe { libc::geteuid() }
            || info.mode() & 0o777 != 0o700
        {
            return false;
        }
    }
    let Ok(info) = std::fs::symlink_metadata(runtime) else {
        return false;
    };
    if value["runtimeStamp"]
        != serde_json::json!({"device": info.dev(), "inode": info.ino(), "uid": info.uid()})
    {
        return false;
    }
    private_socket_path(runtime, "gamescope-127").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn a_seat_ready_report_needs_its_private_display_and_pointer_inside_the_window() {
        let ready = serde_json::json!({
            "version": 1, "publicOnly": true, "phase": "keyboard-ready",
            "nativeWayland": true, "composerEmpty": true, "promptCommands": 0,
            "pid": 900, "waylandDisplay": "gamescope-42",
            "windowWidth": 608, "windowHeight": 448,
            "pointerFocusX": 300, "pointerFocusY": 200,
        });
        assert!(valid_ready(&ready));
        // A foreign display name, a pointer outside the window and a focused composer fail.
        let mut display = ready.clone();
        display["waylandDisplay"] = serde_json::json!("wayland-1");
        assert!(!valid_ready(&display));
        let mut outside = ready.clone();
        outside["pointerFocusX"] = serde_json::json!(608);
        assert!(!valid_ready(&outside));
        let mut typed = ready;
        typed["composerEmpty"] = serde_json::json!(false);
        assert!(!valid_ready(&typed));
    }

    #[test]
    fn a_seat_result_needs_the_exact_text_and_owned_stop() {
        let result = serde_json::json!({
            "version": 1, "publicOnly": true, "phase": "complete", "pid": 901,
            "actualProductionEditor": true, "actualWaylandKeyboardEvents": true,
            "body": TEXT, "keyPressed": TEXT.len(), "promptCommands": 0,
            "manualSendAllowed": false, "stop": zero_stop(),
        });
        assert!(valid_result(&result, 901));
        // A different pid, a manual send and one keystroke short are refused.
        assert!(!valid_result(&result, 900));
        let mut sent = result.clone();
        sent["manualSendAllowed"] = serde_json::json!(true);
        assert!(!valid_result(&sent, 901));
        let mut short = result.clone();
        short["keyPressed"] = serde_json::json!(TEXT.len() - 1);
        assert!(!valid_result(&short, 901));
    }

    #[test]
    fn the_short_runtime_is_a_private_scratch_with_a_matching_stamp() {
        let base =
            crate::tempdir::make_temp_dir(Path::new("/tmp"), "PUBLIC-native-seat-test-").unwrap();
        let scratch = base.canonicalize().unwrap();
        let runtime = scratch.join("runtime");
        fs::create_dir(&runtime).unwrap();
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
        let output =
            crate::tempdir::make_temp_dir(&std::env::temp_dir(), "seat-evidence-").unwrap();
        let info = std::fs::symlink_metadata(&runtime).unwrap();
        let manifest = serde_json::json!({
            "version": 1, "publicOnly": true,
            "runtimePath": runtime.to_string_lossy(),
            "runtimeStamp": {"device": std::os::unix::fs::MetadataExt::dev(&info),
                              "inode": std::os::unix::fs::MetadataExt::ino(&info),
                              "uid": std::os::unix::fs::MetadataExt::uid(&info)},
            "scratchPath": scratch.to_string_lossy(),
            "evidenceRoot": output.canonicalize().unwrap().to_string_lossy(),
        });
        assert!(valid_runtime_manifest(&manifest, &output));
        // A wrong stamp, a runtime outside the scratch and an over-long socket are refused.
        let mut stamp = manifest.clone();
        stamp["runtimeStamp"]["inode"] = serde_json::json!(1);
        assert!(!valid_runtime_manifest(&stamp, &output));
        let mut foreign = manifest.clone();
        foreign["runtimePath"] = serde_json::json!("/tmp/elsewhere/runtime");
        assert!(!valid_runtime_manifest(&foreign, &output));
        assert_eq!(
            private_socket_path(Path::new("/tmp/very-long"), &"gamescope-1".repeat(20))
                .unwrap_err(),
            "private-socket-name-refused"
        );
        fs::remove_dir_all(base).unwrap();
        fs::remove_dir_all(output).unwrap();
    }
}
