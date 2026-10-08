//! PUBLIC zero-Host screenshot fixture embedding the actual production App view.
//! No worker::start, Node, network, account lookup, provider/catalog, prompt or ACK.
//! Feature opt-in only. The parent serializes builds/runs and inspects actual paint.
#![allow(dead_code)]
#[path = "../src/codex.rs"]
mod codex;
#[path = "../src/config.rs"]
mod config;
#[path = "../src/design.rs"]
mod design;
#[path = "../src/export_file.rs"]
mod export_file;
#[path = "../src/exporter.rs"]
mod exporter;
#[path = "../src/interactions.rs"]
mod interactions;
#[path = "../src/known.rs"]
mod known;
#[path = "../src/management.rs"]
mod management;
#[path = "../src/plugins.rs"]
mod plugins;
#[path = "../src/reducer.rs"]
mod reducer;
#[path = "../src/settings.rs"]
mod settings;
#[path = "../src/smoke.rs"]
mod smoke;
#[path = "../src/ui.rs"]
mod ui;
#[path = "../src/worker.rs"]
mod worker;

use iced::advanced::widget::{Id, Operation, operation::Outcome};
use iced::widget::{column, container};
use iced::{Element, Rectangle, Size, Subscription, Task, window};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Component, Path, PathBuf},
    time::Duration,
};

const BANNER_HEIGHT: f32 = 32.0;
const KNOWN_IDS: &[&str] = &[
    "fixture-frame",
    "native-settings-entry",
    "native-stop-control",
    "native-send-control",
    "native-model-controls",
    "native-reasoning-control",
    "native-header",
    "native-file-open",
    "native-file-draft",
    "native-detail-formatted",
    "native-detail-source",
    "native-detail-back",
    "native-detail-copy-source",
];

#[derive(Clone)]
struct Options {
    mode: String,
    size: Size,
    scale: f32,
    screenshot: PathBuf,
    report: PathBuf,
}
fn new_output(value: String) -> Result<PathBuf, &'static str> {
    let path = PathBuf::from(value);
    if !path.is_absolute()
        || path.components().any(|p| matches!(p, Component::ParentDir))
        || path
            .to_str()
            .is_none_or(|s| s.bytes().any(|b| b < 32 || b == 127))
    {
        return Err("output-must-be-absolute-without-traversal");
    }
    let name = path.file_name().ok_or("output-filename-required")?;
    let parent = path
        .parent()
        .and_then(|p| p.canonicalize().ok())
        .filter(|p| p.is_dir())
        .ok_or("output-parent-must-already-exist")?;
    let metadata = fs::metadata(&parent).map_err(|_| "output-parent-unavailable")?;
    let own_uid = fs::metadata("/proc/self")
        .map_err(|_| "process-ownership-unavailable")?
        .uid();
    if metadata.uid() != own_uid || metadata.permissions().mode() & 0o7777 != 0o700 {
        return Err("output-parent-must-be-owned-private-0700-directory");
    }
    let path = parent.join(name);
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(path),
        _ => Err("output-must-be-new"),
    }
}
fn options() -> Result<Options, &'static str> {
    let mut args = std::env::args().skip(1);
    let mut values = BTreeMap::new();
    while let Some(key) = args.next() {
        if ![
            "--mode",
            "--width",
            "--height",
            "--scale",
            "--screenshot",
            "--report",
        ]
        .contains(&key.as_str())
        {
            return Err("unknown-argument");
        }
        let value = args.next().ok_or("missing-argument-value")?;
        if values.insert(key, value).is_some() {
            return Err("duplicate-argument");
        }
    }
    let mode = values
        .remove("--mode")
        .unwrap_or_else(|| "wide-welcome".into());
    let (mut size, mut scale) =
        ui::layout_fixture::preset_size(&mode).ok_or("unknown-public-layout-mode")?;
    for (key, target) in [
        ("--width", &mut size.width),
        ("--height", &mut size.height),
        ("--scale", &mut scale),
    ] {
        if let Some(value) = values.remove(key) {
            *target = value.parse().map_err(|_| "invalid-number")?;
        }
    }
    if !size.width.is_finite()
        || !size.height.is_finite()
        || !scale.is_finite()
        || !(280.0..=4096.0).contains(&size.width)
        || !(240.0..=4096.0).contains(&size.height)
        || !(0.75..=2.0).contains(&scale)
    {
        return Err("invalid-frame-or-application-scale");
    }
    let screenshot = new_output(values.remove("--screenshot").ok_or("screenshot-required")?)?;
    let report = new_output(values.remove("--report").ok_or("report-required")?)?;
    if screenshot == report {
        return Err("distinct-new-output-files-required");
    }
    Ok(Options {
        mode,
        size,
        scale,
        screenshot,
        report,
    })
}
fn public_options(scale: f32) -> config::Options {
    // These explicit fake private paths are inert metadata. Never invoke startup/parse or
    // consult ambient HOME, installed runtimes, profiles, credentials or actual workspaces.
    config::Options {
        backend: dsh_native_core::RustBackendOptions {
            runtime: "/PUBLIC-layout-fixture/NEVER-START/runtime".into(),
            expected_version: "0.2.1-alpha.1".into(),
            native_home: "/PUBLIC-layout-fixture/NEVER-START/native-home".into(),
            user_home: "/PUBLIC-layout-fixture/NEVER-START/user-home".into(),
            working_directory: "/PUBLIC-layout-fixture/workspace".into(),
            absolute_node_path: None,
            startup_timeout: Duration::from_secs(1),
            stop_policy: Default::default(),
        },
        scale,
        smoke: None,
    }
}

#[derive(Clone, Default)]
struct LayoutObservation {
    rects: BTreeMap<&'static str, Vec<Rectangle>>,
}
impl Operation<LayoutObservation> for LayoutObservation {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<LayoutObservation>)) {
        operate(self);
    }
    fn container(&mut self, id: Option<&Id>, bounds: Rectangle) {
        if let Some(name) = KNOWN_IDS
            .iter()
            .copied()
            .find(|name| id.is_some_and(|id| id == &Id::from(*name)))
        {
            self.rects.entry(name).or_default().push(bounds);
        }
    }
    // No text, text-input contents, custom state or arbitrary widget IDs are observed.
    fn finish(&self) -> Outcome<LayoutObservation> {
        Outcome::Some(self.clone())
    }
}
fn rect_value(r: &Rectangle) -> Value {
    json!({"x":r.x,"y":r.y,"width":r.width,"height":r.height})
}
fn valid_rect(r: &Rectangle) -> bool {
    [r.x, r.y, r.width, r.height].iter().all(|v| v.is_finite()) && r.width > 0.0 && r.height > 0.0
}
fn inside(inner: &Rectangle, outer: &Rectangle) -> bool {
    const EPS: f32 = 0.5;
    valid_rect(inner)
        && valid_rect(outer)
        && inner.x >= outer.x - EPS
        && inner.y >= outer.y - EPS
        && inner.x + inner.width <= outer.x + outer.width + EPS
        && inner.y + inner.height <= outer.y + outer.height + EPS
}
impl LayoutObservation {
    fn count(&self, name: &str) -> usize {
        self.rects.get(name).map_or(0, Vec::len)
    }
    fn report(&self, requested: Size, viewport: Option<Size>, metadata: &Value) -> Value {
        let frame = self
            .rects
            .get("fixture-frame")
            .filter(|rs| rs.len() == 1)
            .and_then(|rs| rs.first());
        let composer = metadata["composerExpected"].as_bool().unwrap_or(false);
        let frame_matches = frame.is_some_and(|r| {
            valid_rect(r)
                && (r.width - requested.width).abs() <= 0.5
                && (r.height - requested.height).abs() <= 0.5
        });
        let frame_in_viewport = frame
            .zip(viewport)
            .is_some_and(|(r, size)| inside(r, &Rectangle::with_size(size)));
        let controls_in_frame = frame.is_some_and(|frame| {
            self.rects
                .iter()
                .filter(|(name, _)| **name != "fixture-frame")
                .all(|(_, rects)| rects.iter().all(|r| inside(r, frame)))
        });
        let counts = self.count("native-settings-entry")
            == usize::from(metadata["settingsEntryExpected"] == true)
            && self.count("native-stop-control") == usize::from(metadata["stopExpected"] == true)
            && self.count("native-send-control") == usize::from(composer)
            && self.count("native-model-controls")
                == usize::from(metadata["modelControlsExpected"] == true)
            && self.count("native-reasoning-control")
                == usize::from(metadata["reasoningControlExpected"] == true)
            && self.count("native-header") == usize::from(metadata["headerExpected"] == true)
            && self.count("native-detail-formatted")
                == usize::from(
                    metadata["detailVisible"] == true
                        && metadata["detailFormattedAvailable"] == true,
                )
            && self.count("native-detail-source")
                == usize::from(
                    metadata["detailVisible"] == true
                        && metadata["detailFormattedAvailable"] == true,
                )
            && self.count("native-detail-back") == usize::from(metadata["detailVisible"] == true)
            && self.count("native-detail-copy-source")
                == usize::from(metadata["detailVisible"] == true);
        let states_match = metadata["composerStateMatchesPreset"] == true
            && metadata["reasoningStateMatchesPreset"] == true
            && metadata["validPublicSnapshotApplied"] == true
            && metadata["registryReady"] == true
            && metadata["settingsStateMatchesPreset"] == true
            && metadata["informationStateMatchesPreset"] == true;
        let frame_origin_matches =
            frame.is_some_and(|r| r.x.abs() <= 0.5 && (r.y - BANNER_HEIGHT).abs() <= 0.5);
        let rects: BTreeMap<_, _> = KNOWN_IDS
            .iter()
            .map(|&name| {
                (
                    name,
                    self.rects
                        .get(name)
                        .map(|rs| rs.iter().map(rect_value).collect::<Vec<_>>())
                        .unwrap_or_default(),
                )
            })
            .collect();
        json!({
            "source":"actual-Iced-runtime-widget-operation-after-layout; known container IDs only",
            "coordinateSpace":"Iced application-logical widget layout bounds, not pixel/paint bounds",
            "knownContainerRects":rects,
            "measuredAppFrameBounds":frame.map(rect_value),
            "settingsCount":self.count("native-settings-entry"),
            "settingsRects":self.rects.get("native-settings-entry").map(|rs| rs.iter().map(rect_value).collect::<Vec<_>>()).unwrap_or_default(),
            "stopCount":self.count("native-stop-control"), "sendCount":self.count("native-send-control"),
            "frameMatchesRequestedSize":frame_matches, "frameWithinMeasuredWindowViewport":frame_in_viewport,
            "knownRenderedControlsWithinFrame":controls_in_frame, "expectedControlCountsMatch":counts,
            "presetStatesMatch":states_match, "frameOriginMatchesOutsideBanner":frame_origin_matches,
            "passed":frame_matches && frame_in_viewport && controls_in_frame && counts && states_match && frame_origin_matches,
            "allTextCollected":false, "paintVisibilityQualified":false,
            "scope":"known fixture-tagged container layout only; parent inspects actual screenshot paint"
        })
    }
}

#[derive(Clone)]
enum Message {
    App(ui::Message),
    Opened(window::Id),
    Native {
        backend: &'static str,
        scale: f32,
        size: Size,
    },
    Observe(window::Id),
    Observed(window::Id, Size, LayoutObservation),
    Screenshot(window::Screenshot),
    Saved(bool, [u32; 2]),
    Deadline,
    Close,
    Closed,
}
impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PublicLayoutFixtureMessage(redacted)")
    }
}
struct Fixture {
    options: Options,
    app: ui::App,
    commands: tokio::sync::mpsc::Receiver<worker::Command>,
    report: Value,
    window: Option<window::Id>,
    viewport: Option<Size>,
    capture_started: bool,
    finished: bool,
    captured_count: usize,
    ignored_app_messages: usize,
}
impl Fixture {
    fn boot(options: Options) -> (Self, Task<Message>) {
        let (app, commands) =
            ui::layout_fixture::build(public_options(options.scale), &options.mode, options.size);
        let mut report = ui::layout_fixture::metadata(&app, &options.mode);
        report["pid"] = json!(std::process::id());
        report["rendererRequested"] = json!("wgpu");
        report["font"] = json!("DejaVu Sans");
        report["theme"] = json!("RosePine");
        report["capturedCommandKinds"] = json!([]);
        report["publicFixtureBannerOutsideAppFrame"] = json!(true);
        report["fixtureBannerLogicalHeight"] = json!(BANNER_HEIGHT);
        report["privateOwnedOutputParentsValidated"] = json!(true);
        report["capture"] = json!({"source":"own-Iced-window-renderer-buffer", "publicSource":true,
            "screenshotPath":"[REDACTED explicit new output path]", "reportPath":"[REDACTED explicit new output path]",
            "desktopCapture":false, "oneShot":true});
        let mut fixture = Self {
            options,
            app,
            commands,
            report,
            window: None,
            viewport: None,
            capture_started: false,
            finished: false,
            captured_count: 0,
            ignored_app_messages: 0,
        };
        fixture.inspect_commands();
        (
            fixture,
            Task::perform(
                async { tokio::time::sleep(Duration::from_secs(8)).await },
                |_| Message::Deadline,
            ),
        )
    }
    fn inspect_commands(&mut self) {
        // Exhaustively inspect only command variants; never serialize content or execute effects.
        while let Ok(command) = self.commands.try_recv() {
            let kind = match &command {
                worker::Command::StageFile(_) => "StageFile",
                worker::Command::DiscardFile(_) => "DiscardFile",
                worker::Command::PromptFile { .. } => "PromptFile",
                worker::Command::Codex(_) => "Codex",
                worker::Command::Export(_) => "Export",
                worker::Command::PluginsRead(_) => "PluginsRead",
                worker::Command::PluginSave { .. } => "PluginSave",
                worker::Command::Manage(_) => "Manage",
                worker::Command::KeyMetadata(_) => "KeyMetadata",
                worker::Command::SaveKey { .. } => "SaveKey",
                worker::Command::Decision(_) => "Decision",
                worker::Command::Select { .. } => "Select",
                worker::Command::Create { .. } => "Create",
                worker::Command::OpenWorkspace(_) => "OpenWorkspace",
                worker::Command::Catalog => "Catalog",
                worker::Command::SelectModel { .. } => "SelectModel",
                worker::Command::Prompt { .. } => "Prompt",
                worker::Command::Cancel { .. } => "Cancel",
                worker::Command::Page { .. } => "Page",
                worker::Command::Inspect => "Inspect",
                worker::Command::Shutdown => "Shutdown",
            };
            if self.report["capturedCommandKinds"].is_null() {
                self.report["capturedCommandKinds"] = json!([]);
            }
            self.report["capturedCommandKinds"]
                .as_array_mut()
                .unwrap()
                .push(json!(kind));
            self.captured_count += 1;
            drop(command);
        }
        self.report["capturedCommandCount"] = json!(self.captured_count);
        self.report["commandsInspectedAndDroppedNeverExecuted"] = json!(true);
        self.report["effectsDispatched"] = json!(0);
        self.report["workerStarted"] = json!(false);
    }
    fn finish(&mut self, passed: bool) -> Task<Message> {
        self.inspect_commands();
        self.finished = true;
        self.report["status"] = json!(if passed && self.captured_count == 0 {
            "passed"
        } else {
            "failed"
        });
        self.report["fixturePassed"] = json!(passed && self.captured_count == 0);
        self.report["ignoredAppWidgetMessages"] = json!(self.ignored_app_messages);
        self.report["explicitWindowCloseRequested"] = json!(self.window.is_some());
        let saved = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&self.options.report)
            .and_then(|file| {
                serde_json::to_writer_pretty(&file, &self.report).map_err(std::io::Error::other)?;
                file.sync_all()
            })
            .is_ok();
        if !saved {
            eprintln!("PUBLIC layout report could not be created");
        }
        match self.window {
            Some(id) => window::close(id),
            None => iced::exit(),
        }
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::App(message) => {
                // Actual App view, intentionally no physical-input/business dispatch path.
                drop(message);
                self.ignored_app_messages += 1;
                self.inspect_commands();
            }
            Message::Opened(id) if self.window.is_none() && !self.finished => {
                self.window = Some(id);
                return window::run(id, |w| match w.display_handle().map(|h| h.as_raw()) {
                    Ok(raw_window_handle::RawDisplayHandle::Wayland(_)) => "wayland",
                    Ok(
                        raw_window_handle::RawDisplayHandle::Xlib(_)
                        | raw_window_handle::RawDisplayHandle::Xcb(_),
                    ) => "x11",
                    _ => "other-or-unavailable",
                })
                .then(move |backend| {
                    window::scale_factor(id).then(move |scale| {
                        window::size(id).map(move |size| Message::Native {
                            backend,
                            scale,
                            size,
                        })
                    })
                });
            }
            Message::Native {
                backend,
                scale,
                size,
            } if !self.finished => {
                self.report["windowBackend"] = json!(backend);
                self.report["nativeScaleReportedByIced"] = json!(scale);
                self.report["measuredWindowLogicalViewport"] = json!([size.width, size.height]);
                self.viewport = Some(size);
                // Do NOT feed outer window geometry into App: its requested inner frame is fixed.
                if let Some(id) = self.window {
                    return Task::perform(
                        async { tokio::time::sleep(Duration::from_millis(1000)).await },
                        move |_| Message::Observe(id),
                    );
                }
            }
            Message::Observe(id) if !self.capture_started && !self.finished => {
                self.capture_started = true;
                return window::size(id).then(move |size| {
                    iced::advanced::widget::operate(LayoutObservation::default())
                        .map(move |o| Message::Observed(id, size, o))
                });
            }
            Message::Observed(id, size, observation) if !self.finished => {
                self.viewport = Some(size);
                self.report["measuredWindowLogicalViewport"] = json!([size.width, size.height]);
                self.report["layoutObservation"] =
                    observation.report(self.options.size, self.viewport, &self.report);
                return window::screenshot(id).map(Message::Screenshot);
            }
            Message::Screenshot(screenshot) if !self.finished => {
                let path = self.options.screenshot.clone();
                let size = [screenshot.size.width, screenshot.size.height];
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            smoke::save_screenshot(&path, &screenshot).is_ok()
                        })
                        .await
                        .unwrap_or(false)
                    },
                    move |saved| Message::Saved(saved, size),
                );
            }
            Message::Saved(saved, size) if !self.finished => {
                self.report["capture"]["saved"] = json!(saved);
                self.report["capture"]["physicalBufferSize"] = json!(size);
                let passed = saved
                    && self.report["windowBackend"] == "wayland"
                    && self.report["layoutObservation"]["passed"] == true;
                return self.finish(passed);
            }
            Message::Deadline | Message::Close if !self.finished => {
                self.report["deadlineOrManualCloseBeforeEvidence"] = json!(true);
                return self.finish(false);
            }
            Message::Closed => return iced::exit(),
            _ => {}
        }
        Task::none()
    }
    fn view(&self) -> Element<'_, Message> {
        // The PUBLIC banner is outside the fixed App frame; it never consumes App space.
        // The clipped frame constrains the *actual* production main shell, not a pixel mock.
        let banner = container(
            ui::label(
                "PUBLIC LOCAL FIXTURE · zero Host · synthetic metadata",
                ui::DANGER,
            )
            .size(11),
        )
        .padding([0, 12])
        .width(self.options.size.width)
        .center_y(BANNER_HEIGHT)
        .clip(true)
        .style(|_| ui::panel(ui::SURFACE));
        let frame = container(self.app.view().map(Message::App))
            .id("fixture-frame")
            .width(self.options.size.width)
            .height(self.options.size.height)
            .clip(true)
            .style(|_| ui::panel(ui::BASE));
        column![banner, frame]
            .width(self.options.size.width)
            .height(self.options.size.height + BANNER_HEIGHT)
            .into()
    }
    fn subscription(&self) -> Subscription<Message> {
        // Deliberately omit App.subscription(): no worker stream, keyboard or animation loop.
        window::events().filter_map(|(id, event)| match event {
            window::Event::Opened { .. } => Some(Message::Opened(id)),
            window::Event::CloseRequested => Some(Message::Close),
            window::Event::Closed => Some(Message::Closed),
            _ => None,
        })
    }
}
fn report_passed(path: &Path) -> bool {
    fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .is_some_and(|v| {
            v["status"] == "passed" && v["publicSource"] == true && v["effectsDispatched"] == 0
        })
}
fn main() -> iced::Result {
    // Before any Iced/Tokio/owner threads, matching the production renderer selection.
    unsafe {
        std::env::set_var("ICED_BACKEND", "wgpu");
    }
    let options = options().unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2)
    });
    let report = options.report.clone();
    let window_size = Size::new(
        options.size.width * options.scale,
        (options.size.height + BANNER_HEIGHT) * options.scale,
    );
    let result = iced::application(
        move || Fixture::boot(options.clone()),
        Fixture::update,
        Fixture::view,
    )
    .title("Harness actual App layout · PUBLIC LOCAL FIXTURE · zero Host")
    .window(window::Settings {
        size: window_size,
        resizable: false,
        platform_specific: window::settings::PlatformSpecific {
            application_id: "ai.deepseek.harness.native.public-layout-fixture".into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .exit_on_close_request(false)
    .settings(iced::Settings {
        default_text_size: iced::Pixels(14.0),
        ..Default::default()
    })
    .default_font(ui::FONT)
    .theme(|_: &Fixture| ui::native_theme())
    .scale_factor(|f: &Fixture| f.options.scale)
    .subscription(Fixture::subscription)
    .run();
    if result.is_ok() && !report_passed(&report) {
        eprintln!("PUBLIC actual-App layout fixture failed; inspect its report and paint");
        std::process::exit(1);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn observer_collects_only_known_ids_and_no_text() {
        let mut observation = LayoutObservation::default();
        observation.container(
            Some(&Id::from("private-unlisted-id")),
            Rectangle::with_size(Size::new(10.0, 10.0)),
        );
        observation.text(
            None,
            Rectangle::with_size(Size::new(10.0, 10.0)),
            "unobserved text",
        );
        assert!(observation.rects.is_empty());
        observation.container(
            Some(&Id::from("fixture-frame")),
            Rectangle::with_size(Size::new(760.0, 560.0)),
        );
        assert_eq!(observation.count("fixture-frame"), 1);
    }
    #[test]
    fn rectangle_check_rejects_off_frame_or_nonfinite_bounds() {
        let frame = Rectangle::with_size(Size::new(760.0, 560.0));
        assert!(inside(
            &Rectangle {
                x: 10.0,
                y: 10.0,
                width: 60.0,
                height: 30.0
            },
            &frame
        ));
        assert!(!inside(
            &Rectangle {
                x: 740.0,
                y: 10.0,
                width: 60.0,
                height: 30.0
            },
            &frame
        ));
        assert!(!inside(
            &Rectangle {
                x: f32::NAN,
                y: 10.0,
                width: 60.0,
                height: 30.0
            },
            &frame
        ));
    }
}
