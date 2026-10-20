//! Relocated development launcher → native → isolated Host keyless lifecycle.
//!
//! Report and capture contract of `scripts/qualify-development-package.py`.
use super::registry::{MAX_PNG_BYTES, png_dimensions, private_regular};
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
