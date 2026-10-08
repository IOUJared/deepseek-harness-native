mod model;
mod snapshot;
mod ui;

use model::{Message, ROW_HEIGHT, SESSIONS, TOKEN, WORKSPACES};
use serde_json::{Value, json};
use slint::{ComponentHandle, Model, ModelRc, Timer, TimerMode, VecModel};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};
use ui::{BenchWindow, ChatRow};

const APP_ID: &str = "org.deepseek.NativeBenchmark.Slint";
const GPU_READY: &str = "first_frame_render_submission_complete_before_swap_not_presented";
const SOFTWARE_READY: &str = "event_loop_after_show_proxy_not_render_or_presentation";
const PHASE_DEADLINES_MS: [u64; 4] = [6_000, 12_000, 18_000, 22_000];
const GEOMETRY_OBSERVATION_MS: u64 = 750;

fn shared_logical_size() -> slint::LogicalSize {
    slint::LogicalSize::new(1200.0, 800.0)
}

fn geometry_matches_workload(
    logical: [f32; 2],
    physical: [u32; 2],
    scale: f32,
    requested_scale: Option<f32>,
) -> bool {
    if !scale.is_finite() || scale <= 0.0 {
        return false;
    }
    let expected = shared_logical_size().to_physical(scale);
    let tolerance = 0.5 / scale + 0.001;
    (logical[0] - 1200.0).abs() <= tolerance
        && (logical[1] - 800.0).abs() <= tolerance
        && physical == [expected.width, expected.height]
        && requested_scale.is_none_or(|requested| (scale - requested).abs() < 0.001)
}

fn layout_matches_workload(layout: [[f32; 4]; 4]) -> bool {
    let expected = [
        [0.0, 0.0, 260.0, 800.0],
        [260.0, 0.0, 940.0, 48.0],
        [260.0, 48.0, 940.0, 648.0],
        [260.0, 696.0, 940.0, 104.0],
    ];
    layout
        .into_iter()
        .flatten()
        .zip(expected.into_iter().flatten())
        .all(|(actual, expected)| (actual - expected).abs() < 0.01)
}

#[derive(Debug, PartialEq)]
struct Options {
    bench: bool,
    scale: Option<f32>,
    renderer: Option<String>,
    snapshot: Option<std::path::PathBuf>,
    help: bool,
}

impl Options {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut result = Self {
            bench: false,
            scale: None,
            renderer: None,
            snapshot: None,
            help: false,
        };
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--bench" => result.bench = true,
                "--help" | "-h" => result.help = true,
                "--scale" => {
                    let scale: f32 = args
                        .next()
                        .ok_or("--scale needs a number")?
                        .parse()
                        .map_err(|_| "invalid --scale number")?;
                    if !scale.is_finite() || !(0.5..=4.0).contains(&scale) {
                        return Err("--scale must be finite and between 0.5 and 4".into());
                    }
                    result.scale = Some(scale);
                }
                "--snapshot" => {
                    result.snapshot = Some(
                        args.next()
                            .ok_or("--snapshot needs a new absolute .png path inside this package")?
                            .into(),
                    );
                }
                "--renderer" => {
                    let value = args.next().ok_or("--renderer needs software or femtovg")?;
                    if value != "software" && value != "femtovg" {
                        return Err("--renderer accepts software or femtovg only".into());
                    }
                    result.renderer = Some(value);
                }
                _ => return Err(format!("unknown argument: {arg}")),
            }
        }
        Ok(result)
    }

    fn renderer(&self) -> Result<String, String> {
        if let Some(renderer) = &self.renderer {
            return Ok(renderer.clone());
        }
        match std::env::var("SLINT_BACKEND").as_deref() {
            Ok("winit-software" | "software") => Ok("software".into()),
            Ok("winit-femtovg" | "femtovg" | "winit") | Err(_) => Ok("femtovg".into()),
            Ok(other) => Err(format!(
                "unsupported SLINT_BACKEND={other}; use winit-software or winit-femtovg"
            )),
        }
    }
}

/// Stopwatch begins at the first line of main; readiness stores its actual observed timestamp.
/// It never infers a rendered frame from request_redraw(), show(), or a phase tick.
struct StopwatchReady {
    entered: Instant,
    ready: Cell<Option<Instant>>,
}

impl StopwatchReady {
    fn new(entered: Instant) -> Self {
        Self {
            entered,
            ready: Cell::new(None),
        }
    }
    fn elapsed_ms(&self, timestamp: Instant) -> f64 {
        timestamp.duration_since(self.entered).as_secs_f64() * 1_000.0
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Waiting,
    InitialIdle,
    Scroll,
    Stream,
    FinalIdle,
    Finished,
}

struct Data {
    messages: Vec<Message>,
    range: std::ops::Range<usize>,
}

#[derive(Default)]
struct Metrics {
    scroll_updates: u64,
    stream_updates: u64,
    synthetic_stream_stopped: bool,
    submissions: u64,
    phase_submissions: u64,
    last_submission: Option<Instant>,
    intervals_ms: Vec<f64>,
}

struct Runtime {
    ui: slint::Weak<BenchWindow>,
    clock: StopwatchReady,
    renderer: String,
    bench: bool,
    requested_scale: Option<f32>,
    snapshot_path: Option<std::path::PathBuf>,
    snapshot_failed: Cell<bool>,
    snapshot_timer: Timer,
    notifier_supported: Cell<bool>,
    graphics_api: RefCell<Option<String>>,
    phase: Cell<Phase>,
    phase_started: Cell<Option<Instant>>,
    data: RefCell<Data>,
    visible: Rc<VecModel<ChatRow>>,
    metrics: RefCell<Metrics>,
    forward: Cell<bool>,
    canceled: Cell<bool>,
    tick_timer: Timer,
    phase_timer: Timer,
    ready_timer: Timer,
    geometry_timer: Timer,
}

fn chat_row(index: usize, message: &Message) -> ChatRow {
    ChatRow {
        index: index as i32,
        role: message.role.as_str().into(),
        body: message.body.as_str().into(),
        footer: message.footer.as_str().into(),
    }
}

impl Runtime {
    fn event_at(&self, event: &str, timestamp: Instant, extra: Value) {
        let mut record = json!({"framework": "slint", "event": event, "elapsed_ms": self.clock.elapsed_ms(timestamp)});
        if let (Some(record), Some(extra)) = (record.as_object_mut(), extra.as_object()) {
            record.extend(extra.clone());
        }
        println!("DSH_BENCH:{record}");
    }

    fn event(&self, event: &str, extra: Value) {
        self.event_at(event, Instant::now(), extra);
    }

    fn refresh_visible(&self) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        let rows = {
            let mut data = self.data.borrow_mut();
            let range = model::visible_range(
                data.messages.len(),
                ui.get_scroll_offset(),
                ui.get_viewport_height(),
            );
            if data.range == range && self.visible.row_count() == range.len() {
                return;
            }
            let rows: Vec<_> = range
                .clone()
                .map(|i| chat_row(i, &data.messages[i]))
                .collect();
            data.range = range;
            rows
        };
        // Only a bounded visible slice is a Slint model; the entire 1,000-message data stays in Rust.
        self.visible.set_vec(rows);
    }

    fn scroll_to(&self, offset: f32) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        let count = self.data.borrow().messages.len();
        ui.set_scroll_offset(model::clamp_offset(offset, count, ui.get_viewport_height()));
        self.refresh_visible();
    }

    fn ready(self: &Rc<Self>, measurement: &str, timestamp: Instant) {
        if self.canceled.get() || self.clock.ready.replace(Some(timestamp)).is_some() {
            return;
        }
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        let size = ui.window().size();
        self.event_at("ready", timestamp, json!({
            "measurement": measurement,
            "renderer": self.renderer,
            "graphics_api_observed": self.graphics_api.borrow().clone(),
            "logical_size": [ui.get_logical_width(), ui.get_logical_height()],
            "physical_size_at_observation": [size.width, size.height],
            "effective_scale": ui.window().scale_factor(),
            "requested_application_scale": self.requested_scale,
            "scale_control": if self.requested_scale.is_some() { "child_process_SLINT_SCALE_FACTOR_not_compositor_scale" } else { "framework_display_scale" },
            "notifier_supported": self.notifier_supported.get(),
            "backend_requested": "winit-wayland-only",
            "backend_verification": "runner_must_match_pid_app_id_and_xwayland_false",
            "app_id": APP_ID,
            "pid": std::process::id(),
            "font_family": "DejaVu Sans",
            "font_px": 14,
            "font_fallback_verification": "runner_fc_match_required",
            "rows": self.data.borrow().messages.len(),
            "row_height": ROW_HEIGHT,
            "virtualization": "rust_visible_slice_two_rows_overscan",
            "compositor_presentation_measured": false,
            "snapshot_requested": self.snapshot_path.is_some(),
            "benchmark_comparable": self.snapshot_path.is_none(),
        }));
        self.phase.set(Phase::InitialIdle);
        self.phase_started.set(Some(timestamp));
        self.event_at("idle_start", timestamp, json!({"phase": "initial", "scheduled_duration_ms": 6_000, "repeating_timer_active": false}));
        if self.bench {
            self.schedule_deadline(0);
        }
        let weak = Rc::downgrade(self);
        self.geometry_timer.start(
            TimerMode::SingleShot,
            Duration::from_millis(GEOMETRY_OBSERVATION_MS),
            move || {
                if let Some(runtime) = weak.upgrade() {
                    runtime.observe_settled_geometry();
                }
            },
        );
        if self.snapshot_path.is_some() {
            let weak = Rc::downgrade(self);
            self.snapshot_timer.start(
                TimerMode::SingleShot,
                Duration::from_millis(250),
                move || {
                    if let Some(runtime) = weak.upgrade() {
                        runtime.save_snapshot();
                    }
                },
            );
        }
    }

    /// Read-only bounded geometry observation, not a rendering or mapping callback.
    fn observe_settled_geometry(&self) {
        if self.canceled.get() {
            return;
        }
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        let physical = ui.window().size();
        let scale = ui.window().scale_factor();
        let logical = [ui.get_logical_width(), ui.get_logical_height()];
        let expected_physical = shared_logical_size().to_physical(scale);
        let window_logical = physical.to_logical(scale);
        let rect = |r: ui::RectGeometry| [r.x, r.y, r.width, r.height];
        let layout = [
            rect(ui.get_sidebar_geometry()),
            rect(ui.get_header_geometry()),
            rect(ui.get_transcript_geometry()),
            rect(ui.get_composer_geometry()),
        ];
        let layout_matches = layout_matches_workload(layout);
        self.event("settled_geometry", json!({
            "measurement": "one_shot_750ms_after_ready_window_geometry_observation_not_frame_or_presentation",
            "renderer": self.renderer,
            "logical_size": logical,
            "window_api_logical_size": [window_logical.width, window_logical.height],
            "physical_size": [physical.width, physical.height],
            "physical_size_at_observation": [physical.width, physical.height],
            "effective_scale": scale,
            "requested_application_scale": self.requested_scale,
            "requested_logical_size": [1200, 800],
            "expected_physical_size": [expected_physical.width, expected_physical.height],
            "layout_geometry_logical_xywh": {"sidebar": layout[0], "header": layout[1], "transcript": layout[2], "composer": layout[3]},
            "layout_matches_shared_workload": layout_matches,
            "geometry_matches_shared_workload": layout_matches && geometry_matches_workload(logical, [physical.width, physical.height], scale, self.requested_scale),
            "ready_elapsed_ms": self.clock.ready.get().map(|ready| ready.elapsed().as_secs_f64() * 1_000.0),
            "scheduled_observation_ms": GEOMETRY_OBSERVATION_MS,
            "app_id": APP_ID, "pid": std::process::id(),
            "backend_verification": "runner_must_match_pid_app_id_and_xwayland_false",
            "compositor_presentation_measured": false,
            "observer_requests_redraw": false,
            "repeating_timer_active": false,
        }));
    }

    fn save_snapshot(&self) {
        if self.canceled.get() {
            return;
        }
        let Some(path) = &self.snapshot_path else {
            return;
        };
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        let captured = ui.window().take_snapshot();
        let result = captured
            .as_ref()
            .map_err(|error| error.to_string())
            .and_then(|pixels| {
                snapshot::write_rgba(path, pixels).map_err(|error| error.to_string())
            });
        match result {
            Ok(()) => {
                let pixels = captured.as_ref().expect("snapshot validated before saving");
                self.event("snapshot_saved", json!({"path": path, "physical_size": [pixels.width(), pixels.height()], "measurement": "internal_Window_take_snapshot_synthetic_content_only", "benchmark_comparable": false}));
            }
            Err(error) => {
                eprintln!("Internal synthetic snapshot failed: {error}");
                self.snapshot_failed.set(true);
                self.event("snapshot_error", json!({"error": error, "measurement": "internal_Window_take_snapshot_failed_no_desktop_capture"}));
                self.finish("snapshot_failed");
            }
        }
    }

    fn on_submission(self: &Rc<Self>) {
        let now = Instant::now();
        {
            let mut metrics = self.metrics.borrow_mut();
            metrics.submissions += 1;
            if matches!(self.phase.get(), Phase::Scroll | Phase::Stream) {
                metrics.phase_submissions += 1;
                if let Some(last) = metrics.last_submission {
                    metrics
                        .intervals_ms
                        .push(now.duration_since(last).as_secs_f64() * 1_000.0);
                }
                metrics.last_submission = Some(now);
            }
        }
        if self.clock.ready.get().is_none() {
            self.ready(GPU_READY, now);
        }
        // No redraw request here. Rendering observation never drives rendering.
    }

    fn schedule_deadline(self: &Rc<Self>, index: usize) {
        let ready = self
            .clock
            .ready
            .get()
            .expect("phase scheduled only after ready");
        let deadline = ready + Duration::from_millis(PHASE_DEADLINES_MS[index]);
        let weak = Rc::downgrade(self);
        self.phase_timer.start(
            TimerMode::SingleShot,
            deadline.saturating_duration_since(Instant::now()),
            move || {
                if let Some(runtime) = weak.upgrade() {
                    runtime.transition(index);
                }
            },
        );
    }

    fn reset_phase_metrics(&self) {
        let mut metrics = self.metrics.borrow_mut();
        metrics.phase_submissions = 0;
        metrics.last_submission = None;
        metrics.intervals_ms.clear();
    }

    fn start_ticks(self: &Rc<Self>, duration: Duration) {
        let weak = Rc::downgrade(self);
        self.tick_timer
            .start(TimerMode::Repeated, duration, move || {
                if let Some(runtime) = weak.upgrade() {
                    runtime.tick();
                }
            });
    }

    fn tick(&self) {
        if self.canceled.get() {
            return;
        }
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        match self.phase.get() {
            Phase::Scroll => {
                let rows = self.data.borrow().messages.len();
                let mut forward = self.forward.get();
                let next = model::advance_scroll(
                    ui.get_scroll_offset(),
                    &mut forward,
                    model::max_offset(rows, ui.get_viewport_height()),
                );
                self.forward.set(forward);
                self.scroll_to(next);
                self.metrics.borrow_mut().scroll_updates += 1;
            }
            Phase::Stream => {
                if self.metrics.borrow().synthetic_stream_stopped {
                    return;
                }
                let updated = {
                    let mut data = self.data.borrow_mut();
                    let Some(index) = model::last_visible(
                        data.messages.len(),
                        ui.get_scroll_offset(),
                        ui.get_viewport_height(),
                    ) else {
                        return;
                    };
                    data.messages[index].body.push_str(TOKEN);
                    (
                        index,
                        data.range.clone(),
                        chat_row(index, &data.messages[index]),
                    )
                };
                if updated.1.contains(&updated.0) {
                    self.visible
                        .set_row_data(updated.0 - updated.1.start, updated.2);
                }
                self.metrics.borrow_mut().stream_updates += 1;
            }
            _ => self.tick_timer.stop(),
        }
    }

    fn phase_summary(&self) -> Value {
        let metrics = self.metrics.borrow();
        let mut intervals = metrics.intervals_ms.clone();
        intervals.sort_by(f64::total_cmp);
        let summary = if intervals.is_empty() {
            Value::Null
        } else {
            let percentile =
                |q: f64| intervals[((intervals.len() - 1) as f64 * q).round() as usize];
            json!({"samples": intervals.len(), "min": intervals[0], "mean": intervals.iter().sum::<f64>() / intervals.len() as f64, "p50": percentile(0.5), "p95": percentile(0.95), "max": intervals[intervals.len() - 1]})
        };
        json!({
            "phase_elapsed_ms": self.phase_started.get().map(|start| start.elapsed().as_secs_f64() * 1_000.0),
            "scroll_updates": metrics.scroll_updates,
            "stream_updates": metrics.stream_updates,
            "synthetic_stream_stopped": metrics.synthetic_stream_stopped,
            "submission_count": if self.notifier_supported.get() { Some(metrics.phase_submissions) } else { None },
            "submission_interval_ms": summary,
            "instrumentation": if self.notifier_supported.get() { GPU_READY } else { "unsupported_software_rendering_notifier_no_frame_counts" },
            "compositor_presentation_measured": false,
        })
    }

    fn transition(self: &Rc<Self>, index: usize) {
        if self.canceled.get() {
            return;
        }
        self.tick_timer.stop();
        match index {
            0 => {
                let idle_elapsed =
                    self.phase_started.get().unwrap().elapsed().as_secs_f64() * 1_000.0;
                self.phase.set(Phase::Scroll);
                self.phase_started.set(Some(Instant::now()));
                self.reset_phase_metrics();
                self.event("scroll_start", json!({"initial_idle_elapsed_ms": idle_elapsed, "scheduled_duration_ms": 6_000, "tick_ms": 16, "step_logical_px": 72}));
                self.start_ticks(Duration::from_millis(16));
                self.schedule_deadline(1);
            }
            1 => {
                self.event("scroll_end", self.phase_summary());
                self.phase.set(Phase::Stream);
                self.phase_started.set(Some(Instant::now()));
                self.reset_phase_metrics();
                self.event("stream_start", json!({"scheduled_duration_ms": 6_000, "tick_ms": 50, "fragment": TOKEN, "target": "last_visible_message_not_overscan"}));
                self.start_ticks(Duration::from_millis(50));
                self.schedule_deadline(2);
            }
            2 => {
                self.event("stream_end", self.phase_summary());
                self.phase.set(Phase::FinalIdle);
                self.phase_started.set(Some(Instant::now()));
                // stream_end begins final idle; idle_start is emitted once at ready.
                self.schedule_deadline(3);
            }
            3 => self.finish("completed"),
            _ => unreachable!(),
        }
    }

    fn stop_synthetic(&self) {
        if self.phase.get() == Phase::Stream {
            self.metrics.borrow_mut().synthetic_stream_stopped = true;
            self.tick_timer.stop();
        }
    }

    fn send(&self) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        let text = ui.get_draft();
        let count = {
            let mut data = self.data.borrow_mut();
            if !model::append_user(&mut data.messages, &text) {
                return;
            }
            data.messages.len()
        };
        ui.set_total_rows(count as i32);
        ui.set_draft("".into());
        self.scroll_to(model::max_offset(count, ui.get_viewport_height()));
    }

    fn finish(&self, reason: &str) {
        if self.canceled.replace(true) {
            return;
        }
        self.tick_timer.stop();
        self.phase_timer.stop();
        self.ready_timer.stop();
        self.snapshot_timer.stop();
        self.geometry_timer.stop();
        let ready_elapsed = self
            .clock
            .ready
            .get()
            .map(|ready| ready.elapsed().as_secs_f64() * 1_000.0);
        let final_idle = if self.phase.get() == Phase::FinalIdle {
            self.phase_started
                .get()
                .map(|start| start.elapsed().as_secs_f64() * 1_000.0)
        } else {
            None
        };
        self.phase.set(Phase::Finished);
        let metrics = self.metrics.borrow();
        self.event("finished", json!({
            "reason": reason, "benchmark": self.bench, "ready_elapsed_ms": ready_elapsed,
            "scheduled_total_ms": if self.bench { Some(22_000) } else { None },
            "final_idle_elapsed_ms": final_idle,
            "scroll_updates": metrics.scroll_updates, "stream_updates": metrics.stream_updates,
            "total_observed_submissions": if self.notifier_supported.get() { Some(metrics.submissions) } else { None },
            "instrumentation": if self.notifier_supported.get() { GPU_READY } else { "unsupported_software_rendering_notifier_no_frame_counts" },
            "repeating_timer_active": false, "saved_state": false,
        }));
        if let Err(error) = slint::quit_event_loop() {
            eprintln!("quit_event_loop failed: {error}");
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let entered = Instant::now();
    let options = Options::parse(std::env::args().skip(1))?;
    if options.help {
        println!(
            "Standalone synthetic Slint toolkit comparison (not the complete Harness app).\nUsage: dsh-slint-benchmark [--bench] [--scale NUMBER] [--renderer software|femtovg] [--snapshot ABSOLUTE_NEW.png]\n--snapshot captures only internal synthetic window content; path must be inside this Slint package.\nSLINT_BACKEND=winit-software or winit-femtovg selects the default variant.\n--scale uses process-local SLINT_SCALE_FACTOR, not compositor fractional scaling.\n--bench exits 22 seconds after the honestly labeled ready observation."
        );
        return Ok(());
    }
    if let Some(path) = &options.snapshot {
        snapshot::validate(path)?;
    }
    // SAFETY: this is before BackendSelector, Slint, or any thread is initialized. Only this
    // standalone child process's environment changes; no settings or parent environment change.
    if let Some(scale) = options.scale {
        unsafe {
            std::env::set_var("SLINT_SCALE_FACTOR", scale.to_string());
        }
    }
    let renderer = options.renderer()?;
    let selector = slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name(renderer.clone());
    if renderer == "femtovg" {
        selector.require_opengl().select()?;
    } else {
        selector.select()?;
    }
    slint::set_xdg_app_id(APP_ID)?;
    let ui = BenchWindow::new()?;
    ui.set_workspaces(ModelRc::new(VecModel::from(
        WORKSPACES.map(slint::SharedString::from).to_vec(),
    )));
    ui.set_sessions(ModelRc::new(VecModel::from(
        SESSIONS.map(slint::SharedString::from).to_vec(),
    )));
    ui.set_benchmark_mode(options.bench);
    let visible = Rc::new(VecModel::default());
    ui.set_visible_rows(visible.clone().into());
    let runtime = Rc::new(Runtime {
        ui: ui.as_weak(),
        clock: StopwatchReady::new(entered),
        renderer,
        bench: options.bench,
        requested_scale: options.scale,
        snapshot_path: options.snapshot,
        snapshot_failed: Cell::new(false),
        snapshot_timer: Timer::default(),
        notifier_supported: Cell::new(false),
        graphics_api: RefCell::new(None),
        phase: Cell::new(Phase::Waiting),
        phase_started: Cell::new(None),
        data: RefCell::new(Data {
            messages: model::initial_messages(),
            range: 0..0,
        }),
        visible,
        metrics: RefCell::new(Metrics::default()),
        forward: Cell::new(true),
        canceled: Cell::new(false),
        tick_timer: Timer::default(),
        phase_timer: Timer::default(),
        ready_timer: Timer::default(),
        geometry_timer: Timer::default(),
    });
    runtime.refresh_visible();
    let weak = Rc::downgrade(&runtime);
    ui.on_viewport_changed(move || {
        if let Some(runtime) = weak.upgrade() {
            runtime.refresh_visible();
        }
    });
    let weak = Rc::downgrade(&runtime);
    ui.on_scroll_request(move |delta| {
        if let Some(runtime) = weak.upgrade() {
            if let Some(ui) = runtime.ui.upgrade() {
                runtime.scroll_to(ui.get_scroll_offset() + delta);
            }
        }
    });
    let weak_ui = ui.as_weak();
    ui.on_select_workspace(move |index| {
        if let Some(ui) = weak_ui.upgrade() {
            if (0..3).contains(&index) {
                ui.set_selected_workspace(index);
            }
        }
    });
    let weak_ui = ui.as_weak();
    ui.on_select_session(move |index| {
        if let Some(ui) = weak_ui.upgrade() {
            if (0..12).contains(&index) {
                ui.set_selected_session(index);
            }
        }
    });
    // Settings only explains the isolated prototype; it never reads or writes user settings.
    ui.on_settings(|| {
        eprintln!("Synthetic toolkit comparison: no persistent settings or real Harness state.")
    });
    let weak = Rc::downgrade(&runtime);
    ui.on_send(move || {
        if let Some(runtime) = weak.upgrade() {
            runtime.send();
        }
    });
    let weak = Rc::downgrade(&runtime);
    ui.on_stop(move || {
        if let Some(runtime) = weak.upgrade() {
            runtime.stop_synthetic();
        }
    });
    let weak = Rc::downgrade(&runtime);
    ui.window().on_close_requested(move || {
        if let Some(runtime) = weak.upgrade() {
            runtime.finish("manual_close");
        }
        slint::CloseRequestResponse::HideWindow
    });
    let weak = Rc::downgrade(&runtime);
    match ui
        .window()
        .set_rendering_notifier(move |state, graphics_api| {
            if matches!(state, slint::RenderingState::AfterRendering) {
                if let Some(runtime) = weak.upgrade() {
                    if runtime.graphics_api.borrow().is_none() {
                        *runtime.graphics_api.borrow_mut() = Some(format!("{graphics_api:?}"));
                    }
                    runtime.on_submission();
                }
            }
        }) {
        Ok(()) if runtime.renderer == "femtovg" => runtime.notifier_supported.set(true),
        Ok(()) => return Err("software requested but GPU notifier is supported; refusing mislabeled renderer variant".into()),
        Err(slint::SetRenderingNotifierError::Unsupported) if runtime.renderer == "software" => {}
        Err(error) => return Err(error.into()),
    }
    runtime.event("initialized", json!({
        "renderer": runtime.renderer, "rows": 1_000, "visible_model_rows": runtime.visible.row_count(),
        "backend": "winit-wayland-only", "app_id": APP_ID, "pid": std::process::id(),
        "benchmark": runtime.bench, "notifier_supported": runtime.notifier_supported.get(),
    }));
    // Set an explicit *logical* size before native window creation. In Slint 1.18.1,
    // preferred_size() otherwise overwrites the SLINT_SCALE_FACTOR-adjusted attributes
    // later in ensure_window(). This supported public call marks has_explicit_size and
    // preserves 1200×800 logical -> 1500×1000 physical at application scale 1.25.
    // The DSL fixes constraints only in bench mode; interactive mode remains resizable.
    ui.window().set_size(shared_logical_size());
    ui.show()?;
    if !runtime.notifier_supported.get() {
        let weak = Rc::downgrade(&runtime);
        // Zero-duration single-shot event-loop turn after show is explicitly only a startup proxy.
        // Software lacks RenderingState notifications: no invented first-frame or frame counts.
        runtime
            .ready_timer
            .start(TimerMode::SingleShot, Duration::ZERO, move || {
                if let Some(runtime) = weak.upgrade() {
                    runtime.ready(SOFTWARE_READY, Instant::now());
                }
            });
    }
    slint::run_event_loop()?;
    runtime.tick_timer.stop();
    runtime.phase_timer.stop();
    runtime.ready_timer.stop();
    runtime.snapshot_timer.stop();
    runtime.geometry_timer.stop();
    ui.hide()?;
    if runtime.snapshot_failed.get() {
        return Err("internal synthetic snapshot failed; no fallback capture performed".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(args: &[&str]) -> Result<Options, String> {
        Options::parse(args.iter().map(|s| s.to_string()))
    }
    #[test]
    fn cli_validates_without_starting_a_gui() {
        assert_eq!(
            parse(&["--bench", "--scale", "1.25", "--renderer", "software"]).unwrap(),
            Options {
                bench: true,
                scale: Some(1.25),
                renderer: Some("software".into()),
                snapshot: None,
                help: false
            }
        );
        for args in [
            vec!["--unknown"],
            vec!["--scale"],
            vec!["--scale", "NaN"],
            vec!["--scale", "0"],
            vec!["--renderer", "skia"],
        ] {
            assert!(parse(&args).is_err());
        }
    }
    #[test]
    fn deadlines_use_absolute_ready_time_and_no_idle_ticks() {
        assert_eq!(PHASE_DEADLINES_MS, [6_000, 12_000, 18_000, 22_000]);
        assert_eq!(PHASE_DEADLINES_MS[3], 22_000);
        assert!(GPU_READY.contains("not_presented"));
        assert!(SOFTWARE_READY.contains("proxy_not_render_or_presentation"));
    }
    #[test]
    fn layout_validation_rejects_centered_sidebar_and_header_regressions() {
        let good = [
            [0.0, 0.0, 260.0, 800.0],
            [260.0, 0.0, 940.0, 48.0],
            [260.0, 48.0, 940.0, 648.0],
            [260.0, 696.0, 940.0, 104.0],
        ];
        assert!(layout_matches_workload(good));
        let mut centered_sidebar = good;
        centered_sidebar[0][0] = 470.0;
        assert!(!layout_matches_workload(centered_sidebar));
        let mut centered_header = good;
        centered_header[1][1] = 376.0;
        assert!(!layout_matches_workload(centered_header));
        let mut wrong_viewport = good;
        wrong_viewport[2][3] = 488.0;
        assert!(!layout_matches_workload(wrong_viewport));
    }
    #[test]
    fn explicit_logical_request_has_expected_physical_size_at_each_scale() {
        assert_eq!(
            shared_logical_size().to_physical(1.0),
            slint::PhysicalSize::new(1200, 800)
        );
        assert_eq!(
            shared_logical_size().to_physical(1.25),
            slint::PhysicalSize::new(1500, 1000)
        );
        assert_eq!(
            shared_logical_size().to_physical(2.0),
            slint::PhysicalSize::new(2400, 1600)
        );
        assert_eq!(GEOMETRY_OBSERVATION_MS, 750);
    }
    #[test]
    fn geometry_validation_detects_initial_scale_and_viewport_bug() {
        assert!(geometry_matches_workload(
            [1200.0, 800.0],
            [1200, 800],
            1.0,
            Some(1.0)
        ));
        assert!(geometry_matches_workload(
            [1200.0, 800.0],
            [1500, 1000],
            1.25,
            Some(1.25)
        ));
        assert!(!geometry_matches_workload(
            [960.0, 640.0],
            [1200, 800],
            1.25,
            Some(1.25)
        ));
        assert!(!geometry_matches_workload(
            [1200.0, 800.0],
            [1200, 800],
            1.0,
            Some(1.25)
        ));
        assert!(!geometry_matches_workload(
            [1200.0, 800.0],
            [1200, 800],
            1.25,
            Some(1.25)
        ));
        assert!(!geometry_matches_workload(
            [1200.0, 800.0],
            [1500, 1000],
            f32::NAN,
            None
        ));
        assert!(!geometry_matches_workload(
            [1200.0, 800.0],
            [0, 0],
            0.0,
            None
        ));
    }
    #[test]
    fn virtual_row_mapping_preserves_index_and_full_content() {
        let data = model::initial_messages();
        for index in [0, 1, 5, 999] {
            let row = chat_row(index, &data[index]);
            assert_eq!(row.index, index as i32);
            assert_eq!(row.role.as_str(), data[index].role);
            assert_eq!(row.body.as_str(), data[index].body);
            assert_eq!(row.footer.as_str(), data[index].footer);
        }
    }
}
