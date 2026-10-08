//! PUBLIC standalone native widget-operation probe. No App, worker, Host or model.
//! Operations run against rebuilt Iced layouts; no physical-input/App-coupling claim.
#[path = "../src/scroll_operation.rs"]
mod scroll_operation;

use iced::advanced::widget::{
    Id, Operation,
    operation::{Outcome, Scrollable},
};
use iced::widget::{Column, column, container, scrollable, text};
use iced::{
    Color, Element, Font, Length, Rectangle, Size, Subscription, Task, Theme, Vector, window,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Component, Path, PathBuf},
    time::Duration,
};

const BASE: Color = Color::from_rgb8(0x19, 0x17, 0x24);
const SURFACE: Color = Color::from_rgb8(0x1f, 0x1d, 0x2e);
const TEXT: Color = Color::from_rgb8(0xe0, 0xde, 0xf4);
const MUTED: Color = Color::from_rgb8(0x90, 0x8c, 0xaa);
const FOAM: Color = Color::from_rgb8(0x9c, 0xcf, 0xd8);
const LOVE: Color = Color::from_rgb8(0xeb, 0x6f, 0x92);
const FONT: Font = Font::with_name("DejaVu Sans");
const ROW: f32 = 48.0;

#[derive(Clone)]
struct Options {
    report: PathBuf,
    screenshot: Option<PathBuf>,
}
fn new_output(value: String) -> Result<PathBuf, &'static str> {
    let path = PathBuf::from(value);
    if !path.is_absolute()
        || path.components().any(|c| matches!(c, Component::ParentDir))
        || path
            .to_str()
            .is_none_or(|s| s.bytes().any(|b| b < 32 || b == 127))
    {
        return Err("output-must-be-absolute-without-traversal");
    }
    let name = path.file_name().ok_or("output-name-required")?;
    let parent = path
        .parent()
        .and_then(|p| p.canonicalize().ok())
        .filter(|p| p.is_dir())
        .ok_or("output-parent-required")?;
    let metadata = fs::metadata(&parent).map_err(|_| "output-parent-unavailable")?;
    let uid = fs::metadata("/proc/self")
        .map_err(|_| "own-uid-unavailable")?
        .uid();
    if metadata.uid() != uid || metadata.permissions().mode() & 0o7777 != 0o700 {
        return Err("output-parent-must-be-owned-private-0700");
    }
    let path = parent.join(name);
    match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(path),
        _ => Err("output-must-be-new"),
    }
}
fn options() -> Result<Options, &'static str> {
    let mut args = std::env::args().skip(1);
    let mut values = BTreeMap::new();
    while let Some(key) = args.next() {
        if !["--report", "--screenshot"].contains(&key.as_str()) {
            return Err("unknown-argument");
        }
        let value = args.next().ok_or("argument-value-required")?;
        if values.insert(key, value).is_some() {
            return Err("duplicate-argument");
        }
    }
    let report = new_output(values.remove("--report").ok_or("report-required")?)?;
    let screenshot = values.remove("--screenshot").map(new_output).transpose()?;
    if screenshot.as_ref() == Some(&report) {
        return Err("distinct-output-files-required");
    }
    Ok(Options { report, screenshot })
}
fn theme(_: &Probe) -> Theme {
    Theme::custom(
        "Rose Pine",
        iced::theme::Palette {
            background: BASE,
            text: TEXT,
            primary: FOAM,
            success: FOAM,
            warning: Color::from_rgb8(0xf6, 0xc1, 0x77),
            danger: LOVE,
        },
    )
}

#[derive(Debug, Clone, Copy)]
struct Observation {
    actual_y: f32,
    viewport_height: f32,
    content_height: f32,
    viewport_width: f32,
}
struct Observe {
    id: Id,
    found: Option<Observation>,
}
impl Operation<Observation> for Observe {
    fn traverse(&mut self, f: &mut dyn FnMut(&mut dyn Operation<Observation>)) {
        f(self);
    }
    fn scrollable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        content: Rectangle,
        translation: Vector,
        _: &mut dyn Scrollable,
    ) {
        if id == Some(&self.id) && self.found.is_none() {
            // Default top anchoring: translation is positive effective offset, rounded by Iced.
            self.found = Some(Observation {
                actual_y: translation.y,
                viewport_height: bounds.height,
                content_height: content.height,
                viewport_width: bounds.width,
            });
        }
    }
    fn finish(&self) -> Outcome<Observation> {
        self.found.map_or(Outcome::None, Outcome::Some)
    }
}
fn observe(id: String, phase: u8) -> Task<Message> {
    iced::advanced::widget::operate(Observe {
        id: Id::from(id),
        found: None,
    })
    .map(move |o| Message::Observed(phase, o))
}

#[derive(Debug, Clone)]
enum Message {
    Opened(window::Id),
    Native(&'static str, f32, Size),
    Applied(u8, f32, f32),
    Observed(u8, Observation),
    StaleApplied,
    Capture(window::Id),
    Screenshot(window::Screenshot),
    Saved(bool, [u32; 2]),
    Deadline,
    Close,
    Closed,
}
struct Probe {
    options: Options,
    window: Option<window::Id>,
    revision: u64,
    viewport: Size,
    rows: usize,
    counts: [u8; 3],
    observations: [bool; 3],
    stale_callbacks: u32,
    applied: Option<(u8, f32, f32)>,
    report: Value,
    finished: bool,
}
impl Probe {
    fn boot(options: Options) -> (Self, Task<Message>) {
        (
            Self {
                options,
                window: None,
                revision: 0,
                viewport: Size::new(660.0, 320.0),
                rows: 40,
                counts: [0; 3],
                observations: [false; 3],
                stale_callbacks: 0,
                applied: None,
                finished: false,
                report: json!({"scope":"PUBLIC standalone native scroll operation after rebuilt layout; not production App coupling or physical input", "publicSource":true,"zeroBusinessEffects":true,"effectsDispatched":0,"workerStarted":false,"hostStarted":false,"pid":std::process::id(),"rendererRequested":"wgpu","applicationScale":1.0,"font":"DejaVu Sans","theme":"RosePine","steps":[],"capture":{"desktopCapture":false,"source":"own-Iced-renderer-buffer"},"physicalInputQualified":false,"appCouplingQualified":false}),
            },
            Task::perform(
                async { tokio::time::sleep(Duration::from_secs(15)).await },
                |_| Message::Deadline,
            ),
        )
    }
    fn id(&self) -> String {
        format!("probe-{}", self.revision)
    }
    fn restore(&self, phase: u8, target: f32) -> Task<Message> {
        scroll_operation::restore(self.id(), target, move |actual, height| {
            Message::Applied(phase, actual, height)
        })
    }
    fn finish(&mut self, passed: bool) -> Task<Message> {
        self.finished = true;
        self.report["callbackCounts"] = json!(self.counts);
        self.report["staleCallbackCount"] = json!(self.stale_callbacks);
        self.report["observationsPassed"] = json!(self.observations);
        self.report["status"] = json!(if passed { "passed" } else { "failed" });
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
            eprintln!("PUBLIC probe report could not be created");
        }
        self.window.map_or_else(iced::exit, window::close)
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        if self.finished {
            return if matches!(message, Message::Closed) {
                iced::exit()
            } else {
                Task::none()
            };
        }
        match message {
            Message::Opened(id) if self.window.is_none() => {
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
                        window::size(id).map(move |size| Message::Native(backend, scale, size))
                    })
                });
            }
            Message::Native(backend, scale, size) => {
                self.report["windowBackend"] = json!(backend);
                self.report["nativeScaleReportedByIced"] = json!(scale);
                self.report["windowLogicalSize"] = json!([size.width, size.height]);
                if backend != "wayland" || !scale.is_finite() || scale <= 0.0 {
                    return self.finish(false);
                }
                return self.restore(0, 320.0);
            }
            Message::Applied(phase, actual, height) if phase < 3 => {
                let index = phase as usize;
                self.counts[index] = self.counts[index].saturating_add(1);
                if phase as u64 != self.revision
                    || self.counts[index] != 1
                    || !actual.is_finite()
                    || !height.is_finite()
                {
                    return self.finish(false);
                }
                self.applied = Some((phase, actual, height));
                return observe(self.id(), phase);
            }
            Message::StaleApplied => {
                self.stale_callbacks += 1;
                return self.finish(false);
            }
            Message::Observed(phase, observation) if phase < 3 => {
                let (expected, view, width) = match phase {
                    0 => (320.0, 320.0, 660.0),
                    1 => (396.0, 180.0, 420.0),
                    _ => (0.0, 260.0, 420.0),
                };
                let intrinsic_height = self.rows as f32 * ROW;
                // Iced expands fitting content to the viewport's minimum height.
                // Operations see those actual bounds, not the intrinsic row-height sum.
                let content = intrinsic_height.max(view);
                let near = |a: f32, b: f32| a.is_finite() && (a - b).abs() <= 0.5;
                let applied = self
                    .applied
                    .is_some_and(|(p, y, h)| p == phase && near(y, expected) && near(h, view));
                let passed = phase as u64 == self.revision
                    && applied
                    && near(observation.actual_y, expected)
                    && near(observation.viewport_height, view)
                    && near(observation.content_height, content)
                    && near(observation.viewport_width, width);
                self.report["steps"].as_array_mut().unwrap().push(json!({"phase":phase,"revision":self.revision,"requestedY":if phase==0{320.0}else if phase==1{100000.0}else{120.0},"callbackActualY":self.applied.map(|(_,y,_)|y),"observedRoundedY":observation.actual_y,"actualViewportHeight":observation.viewport_height,"actualContentHeight":observation.content_height,"intrinsicRowHeightSum":intrinsic_height,"expectedOperatedContentHeight":content,"actualViewportWidth":observation.viewport_width,"passed":passed}));
                self.observations[phase as usize] = passed;
                if !passed {
                    return self.finish(false);
                }
                if phase == 0 {
                    let stale_id = self.id();
                    self.revision = 1;
                    self.viewport = Size::new(420.0, 180.0);
                    self.rows = 12;
                    // Both operations are collected before the newly rebuilt revision-1 view.
                    return Task::batch([
                        scroll_operation::restore(stale_id, 0.0, |_, _| Message::StaleApplied),
                        self.restore(1, 100000.0),
                    ]);
                }
                if phase == 1 {
                    let stale_id = self.id();
                    self.revision = 2;
                    self.viewport.height = 260.0;
                    self.rows = 3;
                    // Fits-content case still produces custom operation callback; no on_scroll ACK.
                    return Task::batch([
                        scroll_operation::restore(stale_id, 80.0, |_, _| Message::StaleApplied),
                        self.restore(2, 120.0),
                    ]);
                }
                let passed = self.counts == [1, 1, 1]
                    && self.stale_callbacks == 0
                    && self.observations.iter().all(|v| *v);
                if let (true, Some(_), Some(id)) = (passed, &self.options.screenshot, self.window) {
                    // Paint-only grace period; layout/scroll operations above were never deferred.
                    return Task::perform(
                        async { tokio::time::sleep(Duration::from_millis(250)).await },
                        move |_| Message::Capture(id),
                    );
                }
                return self.finish(passed);
            }
            Message::Capture(id) => return window::screenshot(id).map(Message::Screenshot),
            Message::Screenshot(screenshot) => {
                let Some(path) = self.options.screenshot.clone() else {
                    return self.finish(false);
                };
                let size = [screenshot.size.width, screenshot.size.height];
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            save_screenshot(&path, &screenshot).is_ok()
                        })
                        .await
                        .unwrap_or(false)
                    },
                    move |saved| Message::Saved(saved, size),
                );
            }
            Message::Saved(saved, size) => {
                self.report["capture"]["saved"] = json!(saved);
                self.report["capture"]["physicalBufferSize"] = json!(size);
                return self.finish(saved && self.counts == [1, 1, 1] && self.stale_callbacks == 0);
            }
            Message::Deadline | Message::Close => return self.finish(false),
            Message::Closed => return iced::exit(),
            _ => {}
        }
        Task::none()
    }
    fn view(&self) -> Element<'_, Message> {
        let mut rows = Column::new().spacing(0);
        for index in 0..self.rows {
            rows = rows.push(
                container(
                    text(format!("PUBLIC fixed-height row {} · no Host", index + 1))
                        .font(FONT)
                        .color(TEXT),
                )
                .height(ROW)
                .width(Length::Fill)
                .padding(10)
                .style(|_| container::Style {
                    background: Some(SURFACE.into()),
                    ..Default::default()
                }),
            );
        }
        container(
            column![
                text("PUBLIC standalone scroll-operation probe · zero Host")
                    .font(FONT)
                    .color(LOVE)
                    .size(14),
                text(format!(
                    "Revision {} · {} raw rows · actual native widget geometry",
                    self.revision, self.rows
                ))
                .font(FONT)
                .color(MUTED)
                .size(12),
                scrollable(rows)
                    .id(self.id())
                    .width(self.viewport.width)
                    .height(self.viewport.height),
                text("Scripted operations only · not physical input or production App coupling")
                    .font(FONT)
                    .color(MUTED)
                    .size(12)
            ]
            .spacing(12),
        )
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
    fn subscription(&self) -> Subscription<Message> {
        window::events().filter_map(|(id, event)| match event {
            window::Event::Opened { .. } => Some(Message::Opened(id)),
            window::Event::CloseRequested => Some(Message::Close),
            window::Event::Closed => Some(Message::Closed),
            _ => None,
        })
    }
}
fn save_screenshot(path: &Path, screenshot: &window::Screenshot) -> std::io::Result<()> {
    let size = screenshot.size;
    let bytes = (size.width as usize)
        .checked_mul(size.height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| std::io::Error::other("invalid-public-buffer-size"))?;
    if bytes == 0 || bytes > 64 * 1024 * 1024 || bytes != screenshot.rgba.len() {
        return Err(std::io::Error::other("invalid-public-buffer-length"));
    }
    let file = fs::OpenOptions::new()
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
fn main() -> iced::Result {
    // Select renderer before Iced/runtime threads. No inherited model credentials are read.
    unsafe {
        std::env::set_var("ICED_BACKEND", "wgpu");
    }
    let options = options().unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2)
    });
    let report = options.report.clone();
    let result = iced::application(
        move || Probe::boot(options.clone()),
        Probe::update,
        Probe::view,
    )
    .title("Harness native scroll operation · PUBLIC zero-Host probe")
    .window(window::Settings {
        size: Size::new(760.0, 560.0),
        resizable: false,
        platform_specific: window::settings::PlatformSpecific {
            application_id: "ai.deepseek.harness.native.public-scroll-operation".into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .exit_on_close_request(false)
    .default_font(FONT)
    .theme(theme)
    .subscription(Probe::subscription)
    .run();
    let passed = fs::read(report)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .is_some_and(|v| v["status"] == "passed" && v["zeroBusinessEffects"] == true);
    if result.is_ok() && !passed {
        eprintln!("PUBLIC scroll-operation probe failed");
        std::process::exit(1);
    }
    result
}
