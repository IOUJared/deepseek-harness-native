mod model;

use iced::widget::{Column, Space, button, column, container, row, scrollable, text, text_input};
use iced::{Color, Element, Font, Length, Size, Subscription, Task, Theme, window};
use model::*;
use raw_window_handle::RawDisplayHandle;
use serde_json::{Value, json};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const BASE: Color = Color::from_rgb8(0x19, 0x17, 0x24);
const SURFACE: Color = Color::from_rgb8(0x1f, 0x1d, 0x2e);
const OVERLAY: Color = Color::from_rgb8(0x26, 0x23, 0x3a);
const TEXT: Color = Color::from_rgb8(0xe0, 0xde, 0xf4);
const MUTED: Color = Color::from_rgb8(0x6e, 0x6a, 0x86);
const SUBTLE: Color = Color::from_rgb8(0x90, 0x8c, 0xaa);
const ACCENT: Color = Color::from_rgb8(0xc4, 0xa7, 0xe7);
const BORDER: Color = Color::from_rgb8(0x40, 0x3d, 0x52);
const FONT: Font = Font::with_name("DejaVu Sans");
#[cfg(feature = "gpu")]
const RENDERER: &str = "wgpu";
#[cfg(not(feature = "gpu"))]
const RENDERER: &str = "tiny-skia";
#[cfg(feature = "gpu")]
const APP_ID: &str = "org.deepseek.harness.bench.iced.wgpu";
#[cfg(not(feature = "gpu"))]
const APP_ID: &str = "org.deepseek.harness.bench.iced.tinyskia";

#[derive(Debug, Clone)]
struct Options {
    bench: bool,
    scale: f32,
    screenshot: Option<PathBuf>,
}
impl Options {
    fn parse() -> Result<Option<Self>, String> {
        let mut options = Self {
            bench: false,
            scale: 1.0,
            screenshot: None,
        };
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--bench" => options.bench = true,
                "--scale" => {
                    options.scale = args
                        .next()
                        .ok_or("--scale requires a number")?
                        .parse()
                        .map_err(|_| "invalid scale")?;
                    if !options.scale.is_finite() || !(0.5..=3.0).contains(&options.scale) {
                        return Err("scale must be finite and between 0.5 and 3".into());
                    }
                }
                "--screenshot" => {
                    let path = PathBuf::from(
                        args.next()
                            .ok_or("--screenshot requires an explicit PNG path")?,
                    );
                    let parent = path
                        .parent()
                        .filter(|p| !p.as_os_str().is_empty())
                        .ok_or("screenshot must have an explicit directory")?;
                    let parent = parent.canonicalize().map_err(|e| e.to_string())?;
                    if !parent.starts_with(env!("CARGO_MANIFEST_DIR")) {
                        return Err(
                            "screenshot output must stay inside this Iced package directory".into(),
                        );
                    }
                    if path.extension().and_then(|s| s.to_str()) != Some("png") {
                        return Err("screenshot output must end in .png".into());
                    }
                    options.screenshot =
                        Some(parent.join(path.file_name().ok_or("missing screenshot filename")?));
                }
                "--help" | "-h" => {
                    println!(
                        "dsh-iced-bench ({RENDERER})\n  --bench             6s idle / 6s scroll / 6s stream / 4s idle then exit\n  --scale NUMBER      application UI scale multiplier (default 1; e.g. 1.25)\n  --screenshot PATH   own internally rendered fake content; new PNG inside package\n  Without --bench, stay open as an interactive fake-session prototype."
                    );
                    return Ok(None);
                }
                _ => return Err(format!("unknown argument: {arg}")),
            }
        }
        Ok(Some(options))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Waiting,
    Idle,
    Scroll,
    Stream,
    FinalIdle,
}
#[derive(Debug, Clone)]
enum Message {
    Window(window::Id, window::Event),
    FirstRedraw(window::Id),
    WindowInfo {
        backend: &'static str,
        native_scale: f32,
        size: Size,
        settled: bool,
    },
    QuerySettledGeometry(window::Id),
    BeginScroll,
    EndScroll,
    EndStream,
    Finish,
    ScrollTick,
    StreamTick(u64),
    Scrolled(scrollable::Viewport),
    Composer(String),
    SelectSession(usize),
    SendSynthetic,
    StopSynthetic,
    Settings,
    Screenshot(window::Screenshot),
}

struct App {
    entry: Instant,
    options: Options,
    session: FakeSession,
    composer: String,
    phase: Phase,
    ready: Option<Instant>,
    phase_start: Instant,
    window: Option<window::Id>,
    redraw_seen: bool,
    first_redraw_ms: Option<u128>,
    info: Option<(&'static str, f32)>,
    size: Size,
    viewport: f32,
    offset: f32,
    direction: f32,
    scroll_operations: u64,
    scroll_callbacks: u64,
    peak_row_widgets: usize,
    stream_start_count: u64,
    settings_open: bool,
}

fn panel(background: Color) -> container::Style {
    container::Style {
        background: Some(background.into()),
        text_color: Some(TEXT),
        ..Default::default()
    }
}
fn labeled<'a>(
    label: impl iced::widget::text::IntoFragment<'a>,
    color: Color,
) -> iced::widget::Text<'a> {
    text(label)
        .size(14)
        .line_height(iced::Pixels(18.0))
        .color(color)
        .wrapping(iced::widget::text::Wrapping::None)
}
fn action<'a>(
    label: impl iced::widget::text::IntoFragment<'a>,
    message: Message,
) -> iced::widget::Button<'a, Message> {
    button(labeled(label, TEXT))
        .padding([6, 10])
        .on_press(message)
        .style(|_, status| button::Style {
            background: Some(
                if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                    OVERLAY
                } else {
                    SURFACE
                }
                .into(),
            ),
            text_color: TEXT,
            border: iced::Border {
                color: BORDER,
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        })
}

impl App {
    fn boot(entry: Instant, options: Options) -> (Self, Task<Message>) {
        let app = Self {
            entry,
            options,
            session: FakeSession::new(),
            composer: String::new(),
            phase: Phase::Waiting,
            ready: None,
            phase_start: entry,
            window: None,
            redraw_seen: false,
            first_redraw_ms: None,
            info: None,
            size: Size::new(1200.0, 800.0),
            viewport: 648.0,
            offset: 0.0,
            direction: 1.0,
            scroll_operations: 0,
            scroll_callbacks: 0,
            peak_row_widgets: 12,
            stream_start_count: 0,
            settings_open: false,
        };
        app.emit("initialized", json!({"renderer_variant": RENDERER, "rows": app.session.rows.len(), "font_requested": "DejaVu Sans", "font_size_logical_px": 14, "font_fallback": "not introspected; verify with installed font evidence", "prototype_only": true}));
        (app, Task::none())
    }

    fn emit(&self, event: &str, extra: Value) {
        let mut payload = json!({ "framework": "iced", "event": event, "elapsed_ms": self.entry.elapsed().as_millis() });
        payload
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        println!("DSH_BENCH:{payload}");
        let _ = std::io::stdout().flush();
    }

    fn query_window_info(id: window::Id, settled: bool) -> Task<Message> {
        let handle = window::run(id, |window| {
            match window.display_handle().map(|handle| handle.as_raw()) {
                Ok(RawDisplayHandle::Wayland(_)) => "wayland",
                Ok(RawDisplayHandle::Xlib(_) | RawDisplayHandle::Xcb(_)) => "x11",
                _ => "other_or_unavailable",
            }
        });
        handle.then(move |backend| {
            window::scale_factor(id).then(move |native_scale| {
                window::size(id).map(move |size| Message::WindowInfo {
                    backend,
                    native_scale,
                    size,
                    settled,
                })
            })
        })
    }

    fn maybe_ready(&mut self) -> Task<Message> {
        if self.ready.is_some() || !self.redraw_seen || self.info.is_none() {
            return Task::none();
        }
        let now = Instant::now();
        self.ready = Some(now);
        self.phase = Phase::Idle;
        self.phase_start = now;
        let (backend, native_scale) = self.info.unwrap();
        self.emit("ready", json!({
            "measurement": "first_redraw_event_proxy_plus_window_handle_query_not_presented_frame",
            "first_redraw_event_elapsed_ms": self.first_redraw_ms,
            "renderer": RENDERER, "renderer_selection": "ICED_BACKEND_single_candidate_no_cross_renderer_fallback",
            "application_id": APP_ID,
            "window_backend": backend, "backend_verification": "raw_display_handle_of_own_window",
            "logical_size": [self.size.width, self.size.height],
            "native_window_scale": native_scale, "application_scale": self.options.scale,
            "effective_scale": native_scale * self.options.scale,
            "compositor_fractional_scale_tested": false,
            "row_widgets": visible_range(self.session.rows.len(), self.offset, self.viewport).len(),
        }));
        self.emit(
            "idle_start",
            json!({"phase": "initial_idle", "duration_ms": 6000, "repeating_timer_active": false}),
        );
        let mut tasks = Vec::new();
        if let Some(id) = self.window {
            tasks.push(Task::perform(
                async move {
                    tokio::time::sleep_until(tokio::time::Instant::from_std(
                        now + Duration::from_millis(750),
                    ))
                    .await;
                    id
                },
                Message::QuerySettledGeometry,
            ));
        }
        if self.options.bench {
            for (seconds, message) in [
                (6, Message::BeginScroll),
                (12, Message::EndScroll),
                (18, Message::EndStream),
                (22, Message::Finish),
            ] {
                tasks.push(Task::perform(
                    async move {
                        tokio::time::sleep_until(tokio::time::Instant::from_std(
                            now + Duration::from_secs(seconds),
                        ))
                        .await;
                        message
                    },
                    |message| message,
                ));
            }
        }
        if self.options.screenshot.is_some() {
            if let Some(id) = self.window {
                tasks.push(window::screenshot(id).map(Message::Screenshot));
            }
        }
        Task::batch(tasks)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Window(id, event) => match event {
                window::Event::Opened { .. } => {
                    self.window = Some(id);
                }
                window::Event::Resized(size) => {
                    self.size = size;
                    self.viewport = (size.height - HEADER_HEIGHT - COMPOSER_HEIGHT).max(1.0);
                    self.offset = self
                        .offset
                        .min(max_offset(self.session.rows.len(), self.viewport));
                }
                window::Event::CloseRequested | window::Event::Closed => {
                    self.session.stop_synthetic();
                    return iced::exit();
                }
                _ => {}
            },
            Message::FirstRedraw(id) => {
                if self.redraw_seen {
                    return Task::none();
                }
                self.redraw_seen = true; // immediately removes raw redraw listener: no feedback loop
                self.first_redraw_ms = Some(self.entry.elapsed().as_millis());
                self.window = Some(id);
                return Self::query_window_info(id, false);
            }
            Message::QuerySettledGeometry(id) => return Self::query_window_info(id, true),
            Message::WindowInfo {
                backend,
                native_scale,
                size,
                settled,
            } => {
                self.info = Some((backend, native_scale));
                self.size = size;
                self.viewport = (size.height - HEADER_HEIGHT - COMPOSER_HEIGHT).max(1.0);
                if settled {
                    self.emit("settled_geometry", json!({
                        "measurement": "settled_window_geometry_not_presentation",
                        "ready_elapsed_ms": self.ready.map(|ready| ready.elapsed().as_millis()),
                        "renderer": RENDERER,
                        "renderer_selection": "ICED_BACKEND_single_candidate_no_cross_renderer_fallback",
                        "app_id": APP_ID,
                        "window_backend": backend,
                        "backend_verification": "raw_display_handle_of_own_window",
                        "logical_size": [size.width, size.height],
                        "physical_size": [
                            (size.width * native_scale * self.options.scale).round() as u32,
                            (size.height * native_scale * self.options.scale).round() as u32,
                        ],
                        "physical_size_measurement": "derived_from_queried_iced_viewport_and_effective_scale_not_os_geometry_query",
                        "native_window_scale": native_scale,
                        "application_scale": self.options.scale,
                        "effective_scale": native_scale * self.options.scale,
                        "compositor_fractional_scale_tested": false,
                        "row_widgets": visible_range(self.session.rows.len(), self.offset, self.viewport).len(),
                    }));
                } else {
                    return self.maybe_ready();
                }
            }
            Message::BeginScroll if self.phase == Phase::Idle => {
                self.phase = Phase::Scroll;
                let idle_ms = self.phase_start.elapsed().as_millis();
                self.phase_start = Instant::now();
                self.emit("scroll_start", json!({"initial_idle_elapsed_ms": idle_ms, "tick_ms": 16, "step_logical_px": ROW_HEIGHT, "offset_logical_px": self.offset}));
            }
            Message::EndScroll if self.phase == Phase::Scroll => {
                self.emit("scroll_end", json!({"phase_elapsed_ms": self.phase_start.elapsed().as_millis(), "scroll_operations": self.scroll_operations, "native_scroll_callbacks": self.scroll_callbacks, "offset_logical_px": self.offset, "peak_row_widgets": self.peak_row_widgets, "counts_are_not_presented_frames": true}));
                self.phase = Phase::Stream;
                self.phase_start = Instant::now();
                self.stream_start_count = self.session.stream_updates;
                let target = last_visible(self.session.rows.len(), self.offset, self.viewport);
                self.session.start_synthetic(target);
                self.emit(
                    "stream_start",
                    json!({"tick_ms": 50, "target_row": target, "fragment": " token"}),
                );
            }
            Message::EndStream if self.phase == Phase::Stream => {
                self.session.stop_synthetic();
                self.emit("stream_end", json!({"phase_elapsed_ms": self.phase_start.elapsed().as_millis(), "stream_updates": self.session.stream_updates - self.stream_start_count, "target_row": self.session.stream_target}));
                self.phase = Phase::FinalIdle;
                self.phase_start = Instant::now();
                self.emit("idle_start", json!({"phase": "final_idle", "duration_ms": 4000, "repeating_timer_active": false}));
            }
            Message::Finish => {
                self.session.stop_synthetic();
                self.emit("finished", json!({"final_idle_elapsed_ms": self.phase_start.elapsed().as_millis(), "ready_to_finished_ms": self.ready.map(|r| r.elapsed().as_millis()), "scroll_operations": self.scroll_operations, "native_scroll_callbacks": self.scroll_callbacks, "stream_updates": self.session.stream_updates, "peak_row_widgets": self.peak_row_widgets, "retained_rows": self.session.rows.len()}));
                return iced::exit();
            }
            Message::ScrollTick if self.phase == Phase::Scroll => {
                self.offset = bounce(
                    self.offset,
                    &mut self.direction,
                    max_offset(self.session.rows.len(), self.viewport),
                );
                self.scroll_operations += 1;
                self.track_widgets();
                return self.scroll_to();
            }
            Message::StreamTick(generation) => {
                self.session.tick(generation);
            }
            Message::Scrolled(viewport) => {
                self.offset = viewport.absolute_offset().y;
                self.viewport = viewport.bounds().height;
                self.scroll_callbacks += 1;
                self.track_widgets();
            }
            Message::Composer(value) => self.composer = value,
            Message::SelectSession(index) => {
                self.session.select(index);
                self.settings_open = false;
            }
            Message::SendSynthetic => {
                if !self.composer.trim().is_empty() {
                    self.session
                        .send_synthetic(std::mem::take(&mut self.composer));
                    self.offset = max_offset(self.session.rows.len(), self.viewport);
                    self.track_widgets();
                    return self.scroll_to();
                }
            }
            Message::StopSynthetic => self.session.stop_synthetic(),
            Message::Settings => self.settings_open = !self.settings_open,
            Message::Screenshot(screenshot) => {
                if let Some(path) = self.options.screenshot.take() {
                    let result = save_screenshot(&path, &screenshot);
                    self.emit("screenshot", json!({"path": path, "measurement": "internal_own_window_render_not_desktop_capture", "success": result.is_ok(), "error": result.err().map(|e| e.to_string())}));
                }
            }
            _ => {}
        }
        Task::none()
    }

    fn track_widgets(&mut self) {
        self.peak_row_widgets = self
            .peak_row_widgets
            .max(visible_range(self.session.rows.len(), self.offset, self.viewport).len());
    }
    fn scroll_to(&self) -> Task<Message> {
        iced::widget::operation::scroll_to(
            "transcript",
            scrollable::AbsoluteOffset {
                x: 0.0,
                y: self.offset,
            },
        )
    }

    fn subscription(&self) -> Subscription<Message> {
        let events = iced::event::listen_with(|event, _, id| match event {
            iced::Event::Window(
                event @ (window::Event::Opened { .. }
                | window::Event::Resized(_)
                | window::Event::CloseRequested
                | window::Event::Closed),
            ) => Some(Message::Window(id, event)),
            _ => None,
        });
        let mut subscriptions = vec![events];
        if !self.redraw_seen {
            subscriptions.push(iced::event::listen_raw(|event, _, id| match event {
                iced::Event::Window(window::Event::RedrawRequested(_)) => {
                    Some(Message::FirstRedraw(id))
                }
                _ => None,
            }));
        }
        if self.phase == Phase::Scroll {
            subscriptions
                .push(iced::time::every(Duration::from_millis(16)).map(|_| Message::ScrollTick));
        }
        if self.session.streaming {
            subscriptions.push(
                iced::time::every(Duration::from_millis(50))
                    .with(self.session.generation)
                    .map(|(generation, _)| Message::StreamTick(generation)),
            );
        }
        Subscription::batch(subscriptions)
    }

    fn view(&self) -> Element<'_, Message> {
        let mut sidebar = column![
            labeled("DeepSeek Harness", ACCENT),
            labeled("Native UI prototype", MUTED),
            Space::new().height(12),
            labeled("Workspaces", SUBTLE)
        ]
        .spacing(5);
        for (i, name) in ["Workspace Alpha", "Workspace Beta", "Workspace Gamma"]
            .iter()
            .enumerate()
        {
            sidebar =
                sidebar.push(action(*name, Message::SelectSession(i * 4)).width(Length::Fill));
        }
        sidebar = sidebar
            .push(Space::new().height(10))
            .push(labeled("Sessions", SUBTLE));
        for i in 0..12 {
            let marker = if self.session.selected == i {
                "> "
            } else {
                "  "
            };
            sidebar = sidebar.push(
                action(
                    format!("{marker}Fake session {:02}", i + 1),
                    Message::SelectSession(i),
                )
                .width(Length::Fill),
            );
        }
        sidebar = sidebar
            .push(Space::new().height(Length::Fill))
            .push(action("Settings", Message::Settings).width(Length::Fill));
        let sidebar = container(sidebar)
            .padding(16)
            .width(SIDEBAR_WIDTH)
            .height(Length::Fill)
            .style(|_| panel(SURFACE));
        let heading = if self.settings_open {
            "Settings · backend integration forthcoming".into()
        } else {
            format!(
                "Fake session {:02} · synthetic content only",
                self.session.selected + 1
            )
        };
        let header = container(
            row![
                labeled(heading, TEXT),
                Space::new().width(Length::Fill),
                labeled(RENDERER, SUBTLE)
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([0, 16])
        .height(HEADER_HEIGHT)
        .width(Length::Fill)
        .center_y(HEADER_HEIGHT)
        .style(|_| panel(SURFACE));

        let range = visible_range(self.session.rows.len(), self.offset, self.viewport);
        let mut transcript: Column<'_, Message> = Column::new()
            .spacing(0)
            .width(Length::Fill)
            .push(Space::new().height(range.start as f32 * ROW_HEIGHT));
        for message in &self.session.rows[range.clone()] {
            let contents = column![
                labeled(message.role, ACCENT),
                labeled(message.body.as_str(), TEXT),
                labeled(message.footer, MUTED)
            ]
            .spacing(1);
            transcript = transcript.push(
                container(contents)
                    .padding([7, 14])
                    .height(ROW_HEIGHT)
                    .width(Length::Fill)
                    .clip(true)
                    .style(|_| container::Style {
                        border: iced::Border {
                            color: BORDER,
                            width: 1.0,
                            radius: 0.0.into(),
                        },
                        ..panel(BASE)
                    }),
            );
        }
        transcript = transcript
            .push(Space::new().height((self.session.rows.len() - range.end) as f32 * ROW_HEIGHT));
        let transcript = scrollable(transcript)
            .id("transcript")
            .on_scroll(Message::Scrolled)
            .width(Length::Fill)
            .height(Length::Fill);
        let input = text_input("Write a fake message…", &self.composer)
            .size(14)
            .padding(10)
            .on_input(Message::Composer)
            .on_submit(Message::SendSynthetic)
            .style(|_, _| text_input::Style {
                background: OVERLAY.into(),
                border: iced::Border {
                    color: BORDER,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                icon: SUBTLE,
                placeholder: SUBTLE,
                value: TEXT,
                selection: ACCENT,
            });
        let composer = container(
            column![
                row![
                    input,
                    action("Send", Message::SendSynthetic),
                    action("Stop", Message::StopSynthetic)
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
                labeled(
                    if self.session.streaming {
                        "Synthetic streaming · Stop cancels fake tokens only"
                    } else {
                        "Prototype wiring only · no models, accounts, or network"
                    },
                    MUTED
                ),
            ]
            .spacing(8),
        )
        .padding(16)
        .height(COMPOSER_HEIGHT)
        .width(Length::Fill)
        .style(|_| panel(SURFACE));
        container(row![
            sidebar,
            column![header, transcript, composer]
                .spacing(0)
                .width(Length::Fill)
                .height(Length::Fill)
        ])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| panel(BASE))
        .into()
    }
}

fn save_screenshot(
    path: &PathBuf,
    screenshot: &window::Screenshot,
) -> Result<(), Box<dyn std::error::Error>> {
    // create_new also refuses an existing file or symlink; never replace user data.
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let mut encoder = png::Encoder::new(
        BufWriter::new(file),
        screenshot.size.width,
        screenshot.size.height,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&screenshot.rgba)?;
    Ok(())
}

fn main() -> iced::Result {
    let entry = Instant::now();
    let options = match Options::parse() {
        Ok(Some(options)) => options,
        Ok(None) => return Ok(()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    // SAFETY: the first operation touching renderer selection, before Iced/Tokio
    // create any threads. GPU's only candidate is wgpu; tiny-skia rejects it.
    unsafe {
        std::env::set_var("ICED_BACKEND", RENDERER);
    }
    // iced_winit multiplies initial size by the application scale itself.
    // Wayland's non-resizable setting then pins its min/max to that scaled size.
    let window_size = Size::new(1200.0, 800.0);
    let bench = options.bench;
    iced::application(
        move || App::boot(entry, options.clone()),
        App::update,
        App::view,
    )
    .title(|_: &App| format!("DeepSeek Harness · Iced {RENDERER} prototype"))
    .window(window::Settings {
        size: window_size,
        min_size: bench.then_some(window_size),
        max_size: bench.then_some(window_size),
        resizable: !bench,
        #[cfg(target_os = "linux")]
        platform_specific: window::settings::PlatformSpecific {
            application_id: APP_ID.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .exit_on_close_request(false)
    .settings(iced::Settings {
        default_text_size: iced::Pixels(14.0),
        ..Default::default()
    })
    .default_font(FONT)
    .theme(|_: &App| {
        Theme::custom(
            "Rose Pine",
            iced::theme::Palette {
                background: BASE,
                text: TEXT,
                primary: ACCENT,
                success: Color::from_rgb8(0x9c, 0xcf, 0xd8),
                warning: Color::from_rgb8(0xf6, 0xc1, 0x77),
                danger: Color::from_rgb8(0xeb, 0x6f, 0x92),
            },
        )
    })
    .scale_factor(|app: &App| app.options.scale)
    .subscription(App::subscription)
    .run()
}
