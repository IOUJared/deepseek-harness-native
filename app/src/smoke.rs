//! Opt-in evidence for one real, empty isolated Host; never includes user data.
use dsh_native_core::StopResult;
use serde_json::json;
use std::{io::Write, os::unix::fs::OpenOptionsExt, path::Path};
#[derive(Default)]
pub struct Evidence {
    pub core_ready: bool,
    pub workspace_opened: bool,
    pub session_created: bool,
    pub snapshot_cursor: Option<i64>,
    pub snapshot_valid: bool,
    pub screenshot_requested: bool,
    pub screenshot_scheduled: bool,
    pub screenshot_started: bool,
    pub screenshot_saved: Option<bool>,
    pub screenshot_size: Option<[u32; 2]>,
    pub record_count: usize,
    pub record_kinds: Vec<String>,
    pub backend: Option<&'static str>,
    pub logical_size: Option<[f32; 2]>,
    pub native_scale: Option<f32>,
    pub deadline_reached: bool,
    pub model_prompts: u64,
    pub catalog_requested: bool,
}
impl Evidence {
    pub fn write(
        &self,
        path: &Path,
        sessions: usize,
        workspaces: usize,
        application_scale: f32,
        result: &Result<StopResult, String>,
    ) -> std::io::Result<()> {
        let clean = result.as_ref().is_ok_and(|s| {
            s.exited
                && s.graceful
                && !s.containment_unknown
                && s.observed_descendants_remaining == 0
        });
        let passed = self.core_ready
            && self.workspace_opened
            && self.session_created
            && self.snapshot_cursor.is_some()
            && self.snapshot_valid
            && (!self.screenshot_requested || self.screenshot_saved == Some(true))
            && self.backend == Some("wayland")
            && clean
            && self.model_prompts == 0
            && !self.catalog_requested;
        let stop=result.as_ref().ok().map(|s|json!({"exited":s.exited,"graceful":s.graceful,"containmentUnknown":s.containment_unknown,"observedDescendantsRemaining":s.observed_descendants_remaining,"exitCode":s.exit_code}));
        let value = json!({"scope":"real-native-iced-owned-keyless-host","status":if passed{"passed"}else{"failed"},"pid":std::process::id(),"applicationId":"ai.deepseek.harness.native.app","coreReady":self.core_ready,"workspaceOpened":self.workspace_opened,"sessionCreated":self.session_created,"workspaceCount":workspaces,"sessionCount":sessions,"snapshotCursor":self.snapshot_cursor,"snapshotFoldValid":self.snapshot_valid,"screenshotRequested":self.screenshot_requested,"screenshotSaved":self.screenshot_saved,"screenshotPhysicalSize":self.screenshot_size,"screenshotScope":"one internal own-window Iced render only; never desktop capture","recordCount":self.record_count,"recordKinds":self.record_kinds,"modelPrompts":self.model_prompts,"modelCatalogRequested":self.catalog_requested,"rendererPolicy":"wgpu single requested backend; no cross-renderer fallback","syntheticContent":false,"windowBackend":self.backend,"backendEvidence":"raw_display_handle_of_own_iced_window","logicalSize":self.logical_size,"nativeScale":self.native_scale,"applicationScale":application_scale,"fontRequested":"DejaVu Sans","fontSizeLogicalPx":14,"theme":"standard dark Rose Pine","paintVerified":false,"deadlineReached":self.deadline_reached,"stop":stop,"stopError":result.is_err()});
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(&serde_json::to_vec_pretty(&value)?)?;
        file.write_all(b"\n")?;
        file.sync_all()
    }
}
pub fn kind(value: &str) -> String {
    if crate::known::LOG_EVENTS.contains(&value)
        || [
            "assistant/message",
            "assistant/attempt",
            "user/message",
            "system/message",
            "developer/message",
            "tool/call",
            "tool/result",
            "turn/start",
            "turn/end",
            "step/start",
            "step/end",
            "request/header",
            "request/context",
            "session/end-seed",
            "image/offload",
        ]
        .contains(&value)
    {
        value.into()
    } else {
        "unsupported-event-type".into()
    }
}

/// Writes only an Iced-owned renderer buffer. No desktop API or external helper.
pub fn save_screenshot(path: &Path, screenshot: &iced::window::Screenshot) -> std::io::Result<()> {
    let size = screenshot.size;
    let bytes = (size.width as usize)
        .checked_mul(size.height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| std::io::Error::other("Screenshot dimensions overflow"))?;
    if bytes == 0 || bytes > 64 * 1024 * 1024 || bytes != screenshot.rgba.len() {
        return Err(std::io::Error::other(
            "Screenshot RGBA limit or dimensions invalid",
        ));
    }
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    let mut encoder = png::Encoder::new(&file, size.width, size.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&screenshot.rgba)?;
    writer.finish()?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn own_rgba_encoder_creates_private_png_without_overwriting() {
        use std::os::unix::fs::PermissionsExt;
        let directory =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-artifacts");
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join(format!(
            "unit-only-rgba-{}-{}.png",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let screenshot = iced::window::Screenshot::new(
            vec![25, 23, 36, 255, 31, 29, 46, 255],
            iced::Size::new(2, 1),
            1.0,
        );
        save_screenshot(&path, &screenshot).unwrap();
        let original = std::fs::read(&path).unwrap();
        assert_eq!(&original[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(save_screenshot(&path, &screenshot).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
    #[test]
    fn invalid_renderer_dimensions_fail_before_creating_file() {
        let screenshot = iced::window::Screenshot::new(vec![0; 4], iced::Size::new(2, 1), 1.0);
        assert!(
            save_screenshot(
                Path::new("/unused-output-must-never-be-opened.png"),
                &screenshot
            )
            .is_err()
        );
    }
}
