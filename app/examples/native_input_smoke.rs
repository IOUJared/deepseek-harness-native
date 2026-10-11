//! Production App keyboard input on a parent-owned private synthetic Wayland seat only.
//! No manufactured Editor action, Send, physical keyboard/IME or desktop clipboard qualification.
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
#[path = "../src/rename.rs"]
mod rename;
#[path = "../src/settings.rs"]
mod settings;
#[path = "../src/smoke.rs"]
mod smoke;
#[path = "../src/worker.rs"]
mod worker;
mod ui {
    include!("../src/ui.rs");
    pub(super) use draft_recovery::Outcome as DraftOutcome;
    pub(super) fn startup(a: &App) -> bool {
        a.ready
            && a.root_ready
            && a.management.registry.ready()
            && !a.roster_pending
            && !a.catalog_read.pending()
            && a.draft_state.ready()
            && a.draft_state.failure().is_none()
    }
    pub(super) fn selected(a: &App, id: &SessionId) -> bool {
        a.selected.as_ref() == Some(id)
            && a.follow_ready
            && !a.roster_pending
            && a.ready
            && a.composer_visible()
    }
    pub(super) fn body(a: &App) -> String {
        a.editor.text()
    }
    pub(super) fn size(a: &App) -> iced::Size {
        a.layout_size()
    }
    pub(super) fn own_window(a: &App) -> Option<window::Id> {
        a.own_window
    }
    pub(super) fn busy(a: &App) -> bool {
        a.draft_state.busy()
    }
    pub(super) fn clean(a: &App) -> bool {
        a.draft_state.can_exit()
    }
    pub(super) fn failed(a: &App) -> bool {
        a.draft_state.failure().is_some()
    }
    pub(super) fn stopped(a: &App) -> bool {
        a.stopped
    }
    pub(super) fn failed_close(a: &App) -> bool {
        matches!(a.close, Close::Failed(_))
    }
    pub(super) fn confirm_close(a: &App) -> bool {
        matches!(a.close, Close::Confirm(_))
    }
    pub(super) fn stopping(a: &App) -> bool {
        matches!(a.close, Close::Stopping)
    }
    pub(super) fn counters(a: &App) -> (u64, u64) {
        (a.prompt_serial, a.smoke_evidence.model_prompts)
    }
    pub(super) fn pending_ticket_matches(a: &App, outcome: &DraftOutcome) -> bool {
        let ticket = match outcome {
            DraftOutcome::Loaded { ticket, .. } | DraftOutcome::Saved { ticket, .. } => ticket,
        };
        a.draft_state.pending() == Some(*ticket)
    }
    pub(super) fn durable(a: &App, id: &SessionId, body: &str) -> bool {
        // Never acquire the Store mutex while its blocking operation is in flight.
        !a.draft_state.busy()
            && a.draft_store
                .as_ref()
                .and_then(|store| store.try_lock().ok())
                .and_then(|mut store| store.load().ok())
                .is_some_and(|drafts| {
                    drafts.len() == 1 && drafts.get(id).map(String::as_str) == Some(body)
                })
    }
}
use dsh_native_transport::dto::*;
use iced::widget::text_editor;
use iced::{Element, Subscription, Task, event, keyboard, mouse, window};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::Duration,
};
const EXPECTED: &str = "public native keyboard check";
fn private_display(display: &str) -> bool {
    display.strip_prefix("gamescope-").is_some_and(|digits| {
        (1..=3).contains(&digits.len()) && digits.bytes().all(|byte| byte.is_ascii_digit())
    })
}
fn input_ready(
    size: iced::Size,
    scale: f32,
    native: bool,
    display: &str,
    (serial, prompts): (u64, u64),
) -> Result<Value, &'static str> {
    if serial != 0 || prompts != 0 {
        return Err("unexpected-model-or-prompt-work-before-ready");
    }
    if !native || scale != 1.0 {
        return Err("native-wayland-scale-one-required");
    }
    if !private_display(display) {
        return Err("private-gamescope-display-required");
    }
    if !size.width.is_finite()
        || !size.height.is_finite()
        || !(800.0..=4096.0).contains(&size.width)
        || !(620.0..=4096.0).contains(&size.height)
    {
        return Err("bounded-native-window-required");
    }
    // Bounds above make every floor/cast finite and representable. Coordinates are only
    // a click heuristic in actual logical geometry, not evidence that focus was acquired.
    let width = size.width.floor() as u32;
    let height = size.height.floor() as u32;
    let x = (size.width * 0.65).floor() as u32;
    let y = (size.height - 135.0).floor() as u32;
    if !(1..width).contains(&x) || !(1..height).contains(&y) {
        return Err("bounded-native-pointer-required");
    }
    Ok(
        json!({"version":1,"publicOnly":true,"phase":"keyboard-ready","pid":std::process::id(),
        "nativeWayland":native,"waylandDisplay":display,"composerEmpty":true,"promptCommands":prompts,
        "expectedText":EXPECTED,"windowWidth":width,"windowHeight":height,
        "pointerFocusX":x,"pointerFocusY":y,"coordinateHeuristic":true}),
    )
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Startup,
    Selecting,
    Typing,
    Stopping,
}
#[derive(Clone, Debug)]
enum Message {
    Ui(ui::Message),
    Tick,
    Deadline,
    Raw {
        event: iced::Event,
        status: event::Status,
        window: window::Id,
    },
}
fn stamp(path: &Path) -> Result<Value, &'static str> {
    let info = fs::symlink_metadata(path).map_err(|_| "private-directory-missing")?;
    if !info.is_dir()
        || info.file_type().is_symlink()
        || info.mode() & 0o7777 != 0o700
        || info.uid()
            != fs::metadata("/proc/self")
                .map_err(|_| "private-directory-uid")?
                .uid()
        || path
            .canonicalize()
            .map_err(|_| "private-directory-resolve")?
            != path
    {
        return Err("private-canonical-directory-required");
    }
    Ok(json!({"device":info.dev(),"inode":info.ino(),"uid":info.uid()}))
}
fn arguments(
    args: impl IntoIterator<Item = String>,
) -> Result<(config::Options, PathBuf), &'static str> {
    let mut args = args.into_iter();
    let mut forwarded = Vec::new();
    let mut root = None;
    while let Some(arg) = args.next() {
        if arg == "--input-evidence-root" {
            if root.is_some() {
                return Err("duplicate-input-evidence-root");
            }
            root = Some(PathBuf::from(
                args.next().ok_or("missing-input-evidence-root")?,
            ));
        } else {
            forwarded.push(arg);
        }
    }
    if !forwarded.iter().any(|arg| arg == "--user-home") {
        return Err("explicit-private-user-home-required");
    }
    let options = config::parse(forwarded)
        .ok()
        .flatten()
        .ok_or("explicit-native-options-required")?;
    Ok((
        options,
        root.ok_or("explicit-input-evidence-root-required")?,
    ))
}
fn guard(options: &config::Options, root: &Path) -> Result<(Value, String), &'static str> {
    if options.smoke.is_some()
        || options.backend.expected_version != "0.2.1-alpha.1"
        || options.scale != 1.0
    {
        return Err("ordinary-exact-alpha-scale-one-required");
    }
    let allowed = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("evidence")
        .canonicalize()
        .map_err(|_| "evidence-root-missing")?;
    if root.parent() != Some(allowed.as_path())
        || !root
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("PUBLIC-native-seat-"))
    {
        return Err("explicit-PUBLIC-native-seat-root-required");
    }
    let admitted = stamp(root)?;
    for (path, name) in [
        (&options.backend.native_home, "harness"),
        (&options.backend.user_home, "user"),
        (&options.backend.working_directory, "workspace"),
    ] {
        if path.parent() != Some(root) || path.file_name().is_none_or(|value| value != name) {
            return Err("isolated-sibling-layout-required");
        }
        stamp(path)?;
        if fs::read_dir(path)
            .map_err(|_| "private-directory-read")?
            .next()
            .is_some()
        {
            return Err("fresh-empty-private-homes-required");
        }
    }
    if std::env::var_os("HOME").map(PathBuf::from).as_ref() != Some(&options.backend.user_home)
        || std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some_and(|value| !value.is_empty())
    {
        return Err("private-home-without-desktop-dbus-required");
    }
    let display =
        std::env::var("WAYLAND_DISPLAY").map_err(|_| "private-wayland-display-required")?;
    if !private_display(&display) {
        return Err("private-gamescope-display-required");
    }
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .ok_or("private-runtime-directory-required")?;
    let runtime_stamp = stamp(&runtime)?;
    let scratch = runtime.parent().ok_or("private-scratch-root-required")?;
    if scratch.parent() != Some(Path::new("/tmp"))
        || !scratch
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("PUBLIC-native-seat-"))
        || runtime.file_name().is_none_or(|name| name != "runtime")
    {
        return Err("short-private-scratch-layout-required");
    }
    stamp(scratch)?;
    // The short external runtime is admitted only by this exact private parent-owned manifest.
    // Typed struct visitors reject duplicate/unknown keys and bool/float integer substitutes.
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RuntimeStamp {
        device: u64,
        inode: u64,
        uid: u32,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct RuntimeManifest {
        version: u8,
        public_only: bool,
        runtime_path: PathBuf,
        runtime_stamp: RuntimeStamp,
        scratch_path: PathBuf,
        evidence_root: PathBuf,
    }
    let manifest_path = root.join("runtime-manifest.json");
    let before = fs::symlink_metadata(&manifest_path)
        .map_err(|_| "explicit-private-runtime-manifest-required")?;
    let uid = fs::metadata("/proc/self")
        .map_err(|_| "runtime-manifest-uid")?
        .uid();
    if !before.is_file()
        || before.mode() & 0o7777 != 0o600
        || before.uid() != uid
        || before.nlink() != 1
        || before.len() == 0
        || before.len() > 16_384
    {
        return Err("private-runtime-manifest-regular-required");
    }
    let mut file = fs::File::from(
        rustix::fs::open(
            &manifest_path,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|_| "private-runtime-manifest-open")?,
    );
    let identity = |info: &fs::Metadata| {
        (
            info.dev(),
            info.ino(),
            info.uid(),
            info.gid(),
            info.mode(),
            info.nlink(),
            info.len(),
            info.mtime(),
            info.mtime_nsec(),
            info.ctime(),
            info.ctime_nsec(),
        )
    };
    let opened = file
        .metadata()
        .map_err(|_| "private-runtime-manifest-stat")?;
    if identity(&before) != identity(&opened) {
        return Err("private-runtime-manifest-changed");
    }
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut std::io::Read::take(&mut file, 16_385), &mut bytes)
        .map_err(|_| "private-runtime-manifest-read")?;
    let after = file
        .metadata()
        .map_err(|_| "private-runtime-manifest-stat")?;
    let named =
        fs::symlink_metadata(&manifest_path).map_err(|_| "private-runtime-manifest-changed")?;
    if bytes.len() as u64 != before.len()
        || identity(&opened) != identity(&after)
        || identity(&after) != identity(&named)
    {
        return Err("private-runtime-manifest-changed");
    }
    let manifest: RuntimeManifest =
        serde_json::from_slice(&bytes).map_err(|_| "private-runtime-manifest-invalid")?;
    // Serde struct visitors also accept positional arrays. Require the exact object document,
    // after typed parsing has already rejected duplicate keys and malformed field types.
    let actual: Value =
        serde_json::from_slice(&bytes).map_err(|_| "private-runtime-manifest-invalid")?;
    let expected = json!({"version":1,"publicOnly":true,"runtimePath":runtime,
        "runtimeStamp":runtime_stamp,"scratchPath":scratch,"evidenceRoot":root});
    if actual != expected
        || manifest.version != 1
        || !manifest.public_only
        || manifest.runtime_path != runtime
        || manifest.scratch_path != scratch
        || manifest.evidence_root != root
        || json!({"device":manifest.runtime_stamp.device,"inode":manifest.runtime_stamp.inode,"uid":manifest.runtime_stamp.uid})
            != runtime_stamp
        || stamp(&runtime)? != runtime_stamp
        || stamp(root)? != admitted
    {
        return Err("private-runtime-manifest-mismatch");
    }
    let socket_path = runtime.join(&display);
    if socket_path.to_str().is_none_or(|path| path.len() > 107) {
        return Err("bounded-unix-wayland-socket-required");
    }
    let socket =
        fs::symlink_metadata(&socket_path).map_err(|_| "private-wayland-socket-required")?;
    if !socket.file_type().is_socket() || socket.uid() != uid {
        return Err("owned-private-wayland-socket-required");
    }
    for name in ["input-ready.json", "input-result.json"] {
        if root.join(name).symlink_metadata().is_ok() {
            return Err("fresh-input-artifacts-required");
        }
    }
    Ok((admitted, display))
}
struct Fixture {
    app: ui::App,
    options: config::Options,
    output: PathBuf,
    root_stamp: Value,
    display: String,
    phase: Phase,
    subject: Option<SessionId>,
    native: bool,
    failed: bool,
    completed: bool,
    report: Value,
}
impl Fixture {
    fn boot(
        options: config::Options,
        output: PathBuf,
        root_stamp: Value,
        display: String,
        handle: worker::Handle,
        feed: worker::Feed,
    ) -> (Self, Task<Message>) {
        let (app, task) = ui::App::boot(options.clone(), handle, feed);
        let mut report = json!({"version":1,"status":"running","phase":"initializing","pid":std::process::id(),"publicOnly":true,
            "syntheticOwnSeat":true,"syntheticIsolatedSeat":true,"physicalInputQualified":false,"physicalIME":false,
            "desktopClipboard":false,"networkTraceQualified":false,"sourceClosureComplete":false,"manualSendAllowed":false,
            "actualProductionEditor":false,"actualWaylandKeyboardEvents":false,"observedInput":false,"body":"",
            "expectedText":EXPECTED,"waylandDisplay":display,
            "modelCatalogSucceeded":false,"draftRecoveryLoaded":false,
            "completePUBLICSnapshotObserved":false,"inspectionObserved":false,
            "coordinateHeuristic":true,"latestSnapshotDurable":false,"draftIoJoined":false,"draftsCleanAtExit":false});
        for key in [
            "promptCommands",
            "promptSerial",
            "unexpectedWork",
            "keyPressed",
            "keyReleased",
            "capturedKeyPressed",
            "modifierEvents",
            "modifierBitsObserved",
            "inputMethodEvents",
            "pointerPressed",
            "widgetEditorMessages",
            "widgetEditorEdits",
            "modelCatalogReads",
            "savedOutcomes",
            "createdSessions",
        ] {
            report[key] = json!(0_u64);
        }
        (
            Self {
                app,
                options,
                output,
                root_stamp,
                display,
                phase: Phase::Startup,
                subject: None,
                native: false,
                failed: false,
                completed: false,
                report,
            },
            Task::batch([
                task.map(Message::Ui),
                Task::perform(
                    async {
                        tokio::time::sleep(Duration::from_secs(65)).await;
                    },
                    |_| Message::Deadline,
                ),
            ]),
        )
    }
    fn dispatch(&mut self, message: ui::Message) -> Task<Message> {
        self.app.update(message).map(Message::Ui)
    }
    fn increment(&mut self, key: &str) {
        self.report[key] = json!(self.report[key].as_u64().unwrap_or(0).saturating_add(1));
    }
    fn write(&self, name: &str, value: &Value) -> Result<(), &'static str> {
        if stamp(&self.output)? != self.root_stamp {
            return Err("private-root-changed");
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(self.output.join(name))
            .map_err(|_| "private-artifact-create")?;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| "private-artifact-mode")?;
        file.write_all(value.to_string().as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|_| "private-artifact-write")
    }
    fn close(&mut self) -> Task<Message> {
        self.phase = Phase::Stopping;
        let task = self.dispatch(ui::Message::Close);
        let confirm = if ui::confirm_close(&self.app) {
            self.dispatch(ui::Message::ConfirmClose)
        } else {
            Task::none()
        };
        Task::batch([task, confirm, self.finish()])
    }
    fn fail(&mut self, code: &'static str) -> Task<Message> {
        if self.completed {
            return Task::none();
        }
        self.failed = true;
        if self.report.get("errorCode").is_none() {
            self.report["errorCode"] = json!(code);
        }
        if self.phase == Phase::Stopping {
            self.finish()
        } else {
            self.close()
        }
    }
    fn tick(&mut self) -> Task<Message> {
        if ui::failed(&self.app) && !self.failed {
            return self.fail("actual-draft-IO-failed");
        }
        match self.phase {
            Phase::Startup
                if self.native
                    && ui::startup(&self.app)
                    && self.report["modelCatalogReads"] == 1 =>
            {
                self.phase = Phase::Selecting;
                self.dispatch(ui::Message::NewSession) // The only driver-created business intent: one blank session.
            }
            Phase::Selecting
                if self
                    .subject
                    .as_ref()
                    .is_some_and(|id| ui::selected(&self.app, id))
                    && self.report["completePUBLICSnapshotObserved"] == true =>
            {
                if !ui::body(&self.app).is_empty() {
                    return self.fail("new-session-composer-not-empty");
                }
                let counters = ui::counters(&self.app);
                self.report["promptSerial"] = json!(counters.0);
                self.report["promptCommands"] = json!(counters.1);
                let ready = match input_ready(
                    ui::size(&self.app),
                    self.options.scale,
                    self.native,
                    &self.display,
                    counters,
                ) {
                    Ok(ready) => ready,
                    Err(code) => return self.fail(code),
                };
                if self.write("input-ready.json", &ready).is_err() {
                    return self.fail("input-ready-write-failed");
                }
                self.phase = Phase::Typing;
                Task::none()
            }
            Phase::Typing => {
                let body = ui::body(&self.app);
                if !EXPECTED.starts_with(&body) {
                    return self.fail("unexpected-native-editor-body");
                }
                if body == EXPECTED
                    && self.report["keyPressed"].as_u64().unwrap_or(0) > 0
                    && self.report["widgetEditorEdits"].as_u64().unwrap_or(0) > 0
                {
                    self.report["observedInput"] = json!(true);
                    self.report["actualProductionEditor"] = json!(true);
                    self.report["actualWaylandKeyboardEvents"] = json!(self.native);
                    self.close()
                } else {
                    Task::none()
                }
            }
            Phase::Stopping => self.finish(),
            _ => Task::none(),
        }
    }
    fn finish(&mut self) -> Task<Message> {
        if self.completed || !ui::stopped(&self.app) || ui::busy(&self.app) {
            return Task::none();
        }
        if !ui::clean(&self.app) && !ui::failed_close(&self.app) {
            return Task::none();
        }
        let (serial, prompts) = ui::counters(&self.app);
        let body = ui::body(&self.app);
        let durable = self
            .subject
            .as_ref()
            .is_some_and(|id| ui::durable(&self.app, id, EXPECTED));
        let passed = !self.failed
            && self.native
            && ui::clean(&self.app)
            && !ui::failed_close(&self.app)
            && self.report["observedInput"] == true
            && self.report["actualProductionEditor"] == true
            && self.report["actualWaylandKeyboardEvents"] == true
            && body == EXPECTED
            && durable
            && self.report["modelCatalogReads"] == 1
            && self.report["modelCatalogSucceeded"] == true
            && self.report["createdSessions"] == 1
            && self.report["unexpectedWork"] == 0
            && self.report["draftRecoveryLoaded"] == true
            && self.report["completePUBLICSnapshotObserved"] == true
            && self.report["inspectionObserved"] == true
            && self.report["gracefulStopped"] == true
            && serial == 0
            && prompts == 0;
        self.report["phase"] = json!("complete");
        self.report["status"] = json!(if passed { "passed" } else { "failed" });
        self.report["body"] = json!(body);
        self.report["promptSerial"] = json!(serial);
        self.report["promptCommands"] = json!(prompts);
        self.report["latestSnapshotDurable"] = json!(durable);
        self.report["draftIoJoined"] = json!(true);
        self.report["draftsCleanAtExit"] = json!(ui::clean(&self.app));
        if self.write("input-result.json", &self.report).is_err() {
            self.report["status"] = json!("failed");
            self.report["errorCode"] = json!("input-result-write-failed");
        }
        self.completed = true;
        println!("{}", self.report); // Before returning any original production exit Task.
        if ui::failed_close(&self.app) {
            self.dispatch(ui::Message::ExitAnyway)
        } else {
            Task::none()
        }
    }
    fn raw(
        &mut self,
        event: iced::Event,
        status: event::Status,
        window: window::Id,
    ) -> Task<Message> {
        if ui::own_window(&self.app) != Some(window) {
            return Task::none();
        }
        let modifiers = match &event {
            iced::Event::Keyboard(keyboard::Event::KeyPressed { modifiers, .. }) => {
                self.increment("keyPressed");
                if status == event::Status::Captured {
                    self.increment("capturedKeyPressed");
                }
                Some(*modifiers)
            }
            iced::Event::Keyboard(keyboard::Event::KeyReleased { modifiers, .. }) => {
                self.increment("keyReleased");
                Some(*modifiers)
            }
            iced::Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                self.increment("modifierEvents");
                Some(*modifiers)
            }
            iced::Event::InputMethod(_) => {
                self.increment("inputMethodEvents");
                None
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed(_)) => {
                self.increment("pointerPressed");
                None
            }
            _ => None,
        };
        if let Some(modifiers) = modifiers {
            self.report["modifierBitsObserved"] = json!(
                self.report["modifierBitsObserved"].as_u64().unwrap_or(0)
                    | u64::from(modifiers.bits())
            );
        }
        Task::none()
    }
    fn worker_event(&mut self, event: worker::Event) -> Task<Message> {
        if matches!(
            &event,
            worker::Event::Prompt { .. }
                | worker::Event::PromptNotSent { .. }
                | worker::Event::Selected {
                    frame: SessionFollowFrame::AssistantStream { .. },
                    ..
                }
        ) {
            self.increment("unexpectedWork");
            return self.fail("unexpected-model-or-prompt-work");
        }
        if let worker::Event::Selected {
            frame: SessionFollowFrame::Event { event },
            ..
        } = &event
            && forbidden_kind(&event.event_type)
        {
            self.increment("unexpectedWork");
            return self.fail("unexpected-live-agent-event");
        }
        if let worker::Event::Selected {
            frame:
                SessionFollowFrame::Snapshot {
                    records,
                    has_more,
                    assistant_stream,
                    ..
                },
            ..
        } = &event
        {
            if *has_more
                || records.len() > 128
                || !idle_baseline(assistant_stream.as_ref())
                || records
                    .iter()
                    .any(|HistoryRecord::Event { event }| forbidden_kind(&event.event_type))
            {
                self.increment("unexpectedWork");
                return self.fail("unexpected-recorded-agent-work");
            }
            self.report["completePUBLICSnapshotObserved"] = json!(true);
        }
        if let worker::Event::Created(Ok(value)) = &event {
            if self.phase != Phase::Selecting || self.subject.is_some() {
                return self.fail("unexpected-created-session");
            }
            self.subject = Some(value.session_id.clone());
            self.increment("createdSessions");
            self.report["sessionId"] = json!(value.session_id.as_str());
        }
        if let worker::Event::Catalog { result, .. } = &event {
            self.increment("modelCatalogReads");
            self.report["modelCatalogSucceeded"] = json!(result.is_ok());
            if result.is_err() || self.report["modelCatalogReads"] != 1 {
                return self.fail("keyless-catalog-not-qualified");
            }
        }
        if let worker::Event::Inspection(result) = &event {
            self.report["inspectionObserved"] = json!(true);
            self.report["inspectionKnownIdle"] =
                json!(result.as_ref().is_ok_and(|value| value.is_known_idle()));
            let task = self.dispatch(ui::Message::Worker(event));
            let confirm = if ui::confirm_close(&self.app) {
                self.dispatch(ui::Message::ConfirmClose)
            } else {
                Task::none()
            };
            return Task::batch([task, confirm]);
        }
        if let worker::Event::Stopped(result) = &event {
            self.report["gracefulStopped"] = json!(result.as_ref().is_ok_and(|stop| stop.exited
                && stop.graceful
                && !stop.containment_unknown
                && stop.observed_descendants_remaining == 0
                && stop.exit_code == Some(0)));
            if let Ok(stop) = result {
                self.report["stop"] = json!({"exited":stop.exited,"graceful":stop.graceful,"containmentUnknown":stop.containment_unknown,"observedDescendantsRemaining":stop.observed_descendants_remaining,"exitCode":stop.exit_code});
            }
            let task = self.dispatch(ui::Message::Worker(event));
            return Task::batch([task, self.finish()]);
        }
        if matches!(
            &event,
            worker::Event::Fault(_)
                | worker::Event::FollowError { .. }
                | worker::Event::Roster(Err(_))
                | worker::Event::Created(Err(_))
                | worker::Event::NoBackendStarted
                | worker::Event::ObservationsLost
        ) {
            let task = self.dispatch(ui::Message::Worker(event));
            return Task::batch([task, self.fail("actual-worker-failed")]);
        }
        self.dispatch(ui::Message::Worker(event))
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Ui(ui::Message::Worker(event)) => self.worker_event(event),
            Message::Ui(ui::Message::NativeOpened(id)) => {
                self.dispatch(ui::Message::NativeOpened(id))
            }
            Message::Ui(message @ ui::Message::NativeInfo { .. }) => {
                if let ui::Message::NativeInfo {
                    backend,
                    native_scale,
                    ..
                } = &message
                {
                    self.native = *backend == "wayland";
                    self.report["nativeWayland"] = json!(self.native);
                    self.report["nativeScale"] = json!(native_scale);
                    if !self.native {
                        return self.fail("native-wayland-required");
                    }
                    if *native_scale != 1.0 {
                        return self.fail("private-seat-scale-one-required");
                    }
                }
                self.dispatch(message)
            }
            Message::Ui(
                message @ (ui::Message::Resized(_)
                | ui::Message::WindowActive(_)
                | ui::Message::MotionFrame(_)
                | ui::Message::WorkingTick
                | ui::Message::ScrollApplied { .. }
                | ui::Message::Scrolled { .. }
                | ui::Message::DraftFlush),
            ) => self.dispatch(message),
            Message::Ui(ui::Message::DraftOutcome(outcome)) => {
                if !ui::pending_ticket_matches(&self.app, &outcome) {
                    return self.fail("unexpected-draft-outcome-ticket");
                }
                let failed = match &outcome {
                    ui::DraftOutcome::Loaded { result, .. } => {
                        self.report["draftRecoveryLoaded"] = json!(result.is_ok());
                        result.is_err()
                    }
                    ui::DraftOutcome::Saved { result, .. } => {
                        self.increment("savedOutcomes");
                        result.is_err()
                    }
                };
                let task = self.dispatch(ui::Message::DraftOutcome(outcome));
                let finish = if failed {
                    self.fail("actual-draft-IO-failed")
                } else {
                    self.finish()
                };
                Task::batch([task, finish])
            }
            Message::Ui(ui::Message::Editor(action)) if self.phase == Phase::Typing => {
                if matches!(
                    &action,
                    text_editor::Action::Edit(text_editor::Edit::Paste(_))
                ) {
                    return self.fail("clipboard-paste-outside-keyboard-scope");
                }
                self.increment("widgetEditorMessages");
                if action.is_edit() {
                    self.increment("widgetEditorEdits");
                }
                self.dispatch(ui::Message::Editor(action)) // Only messages emitted by the unchanged production widget.
            }
            Message::Ui(ui::Message::Send | ui::Message::KeyboardSend) => {
                self.fail("send-denied-by-input-fixture")
            }
            Message::Ui(ui::Message::Close) => self.fail("manual-close-before-complete"),
            Message::Ui(ui::Message::Editor(_)) => Task::none(),
            Message::Ui(_) => self.fail("manual-business-action-outside-input-scope"),
            Message::Raw {
                event,
                status,
                window,
            } => self.raw(event, status, window),
            Message::Tick => self.tick(),
            Message::Deadline => self.fail("owned-input-fixture-deadline"),
        }
    }
    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            self.app.subscription().map(Message::Ui),
            event::listen_with(observed_event),
            iced::time::every(Duration::from_millis(60)).map(|_| Message::Tick),
        ])
    }
    fn view(&self) -> Element<'_, Message> {
        self.app.view().map(Message::Ui)
    }
}
fn observed_event(
    event: iced::Event,
    status: event::Status,
    window: window::Id,
) -> Option<Message> {
    match event {
        iced::Event::Keyboard(_) | iced::Event::InputMethod(_) | iced::Event::Mouse(_) => {
            Some(Message::Raw {
                event,
                status,
                window,
            })
        }
        _ => None,
    }
}
fn forbidden_kind(kind: &str) -> bool {
    kind.len() > 128
        || kind.contains("llm")
        || [
            "step/start",
            "step/end",
            "request/header",
            "tool/call",
            "tool/result",
            "assistant/message",
            "user/message",
        ]
        .contains(&kind)
}
fn idle_baseline(baseline: Option<&AssistantBaseline>) -> bool {
    baseline.is_none_or(|value| value.active_attempt.is_none())
}
fn main() -> iced::Result {
    let (options, output) = arguments(std::env::args().skip(1)).unwrap_or_else(|code| {
        eprintln!("{code}");
        std::process::exit(2)
    });
    let (root_stamp, display) = guard(&options, &output).unwrap_or_else(|code| {
        eprintln!("{code}");
        std::process::exit(2)
    });
    unsafe {
        std::env::set_var("ICED_BACKEND", "wgpu");
    }
    let (handle, feed, owner) =
        worker::start(options.backend.clone(), false).unwrap_or_else(|_| {
            eprintln!("owned-input-worker-unavailable");
            std::process::exit(1)
        });
    let result = iced::application(
        move || {
            Fixture::boot(
                options.clone(),
                output.clone(),
                root_stamp.clone(),
                display.clone(),
                handle.clone(),
                feed.clone(),
            )
        },
        Fixture::update,
        Fixture::view,
    )
    .title("Harness PUBLIC private native input fixture")
    .window(window::Settings {
        size: iced::Size::new(1024.0, 768.0),
        resizable: false,
        platform_specific: window::settings::PlatformSpecific {
            application_id: "ai.deepseek.harness.native.input-fixture".into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .exit_on_close_request(false)
    .default_font(ui::FONT)
    .theme(|_: &Fixture| ui::native_theme())
    .scale_factor(|fixture: &Fixture| fixture.options.scale)
    .subscription(Fixture::subscription)
    .run();
    drop(owner); // Retain and join the production Core/worker owner outside Iced's entire lifespan.
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn input_ready_exact_integer_protocol_and_fractional_geometry() {
        let ready = input_ready(
            iced::Size::new(1024.75, 768.75),
            1.0,
            true,
            "gamescope-0",
            (0, 0),
        )
        .unwrap();
        let parsed: Value = serde_json::from_str(&ready.to_string()).unwrap();
        assert_eq!(
            parsed,
            json!({"version":1,"publicOnly":true,"phase":"keyboard-ready",
            "pid":std::process::id(),"nativeWayland":true,"waylandDisplay":"gamescope-0",
            "composerEmpty":true,"promptCommands":0,"expectedText":EXPECTED,"windowWidth":1024,
            "windowHeight":768,"pointerFocusX":666,"pointerFocusY":633,"coordinateHeuristic":true})
        );
        for key in [
            "version",
            "pid",
            "promptCommands",
            "windowWidth",
            "windowHeight",
            "pointerFocusX",
            "pointerFocusY",
        ] {
            assert!(parsed[key].is_u64(), "{key} must be a JSON integer");
        }
    }
    #[test]
    fn input_ready_geometry_bounds_remain_inside_integer_dimensions() {
        for (width, height, x, y) in [(800.0, 620.0, 520, 485), (4096.0, 4096.0, 2662, 3961)] {
            let ready = input_ready(
                iced::Size::new(width, height),
                1.0,
                true,
                "gamescope-127",
                (0, 0),
            )
            .unwrap();
            assert_eq!(ready["windowWidth"], json!(width.floor() as u32));
            assert_eq!(ready["windowHeight"], json!(height.floor() as u32));
            assert_eq!(ready["pointerFocusX"], json!(x));
            assert_eq!(ready["pointerFocusY"], json!(y));
        }
    }
    #[test]
    fn input_ready_refuses_nonfinite_outside_geometry_and_nonunit_scale() {
        for width in [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -1.0,
            799.99,
            4096.01,
        ] {
            assert_eq!(
                input_ready(
                    iced::Size::new(width, 768.0),
                    1.0,
                    true,
                    "gamescope-0",
                    (0, 0)
                ),
                Err("bounded-native-window-required")
            );
        }
        for height in [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -1.0,
            619.99,
            4096.01,
        ] {
            assert_eq!(
                input_ready(
                    iced::Size::new(1024.0, height),
                    1.0,
                    true,
                    "gamescope-0",
                    (0, 0)
                ),
                Err("bounded-native-window-required")
            );
        }
        for scale in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, 0.5, 2.0] {
            assert_eq!(
                input_ready(
                    iced::Size::new(1024.0, 768.0),
                    scale,
                    true,
                    "gamescope-0",
                    (0, 0)
                ),
                Err("native-wayland-scale-one-required")
            );
        }
        assert_eq!(
            input_ready(
                iced::Size::new(1024.0, 768.0),
                1.0,
                false,
                "gamescope-0",
                (0, 0)
            ),
            Err("native-wayland-scale-one-required")
        );
    }
    #[test]
    fn input_ready_requires_private_display_and_zero_sampled_counters() {
        let size = iced::Size::new(1024.0, 768.0);
        for display in [
            "wayland-0",
            "gamescope-",
            "gamescope-1000",
            "gamescope-1/",
            "gamescope-１",
            "gamescope-1\n",
        ] {
            assert_eq!(
                input_ready(size, 1.0, true, display, (0, 0)),
                Err("private-gamescope-display-required")
            );
        }
        for counters in [(1, 0), (0, 1), (1, 1), (u64::MAX, u64::MAX)] {
            assert_eq!(
                input_ready(size, 1.0, true, "gamescope-0", counters),
                Err("unexpected-model-or-prompt-work-before-ready")
            );
        }
    }
    #[test]
    fn input_scope_has_one_fixed_lowercase_ascii_body_and_no_active_baseline() {
        assert_eq!(EXPECTED, "public native keyboard check");
        assert!(
            EXPECTED
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b' ')
        );
        assert!(idle_baseline(None));
        assert!(idle_baseline(Some(&AssistantBaseline {
            revision: 0,
            active_attempt: None
        })));
        assert!(!idle_baseline(Some(&AssistantBaseline {
            revision: 1,
            active_attempt: Some(AssistantAttempt {
                attempt_id: "PUBLIC".into(),
                started_after_seq: 0,
                turn: 1,
                step: 1,
                next_index: 0,
                stream: vec![]
            })
        })));
        assert!(forbidden_kind("session/title-llm-request"));
        assert!(forbidden_kind("tool/call"));
        assert!(!forbidden_kind("permission/preset"));
    }
    #[test]
    fn missing_or_duplicate_evidence_root_and_ambient_user_home_refuse() {
        assert!(matches!(
            arguments(Vec::<String>::new()),
            Err("explicit-private-user-home-required")
        ));
        assert!(matches!(
            arguments(["--input-evidence-root".into()]),
            Err("missing-input-evidence-root")
        ));
        assert!(matches!(
            arguments([
                "--input-evidence-root".into(),
                "/PUBLIC".into(),
                "--input-evidence-root".into(),
                "/PUBLIC-other".into()
            ]),
            Err("duplicate-input-evidence-root")
        ));
    }
    #[test]
    fn native_observer_includes_widget_captured_events_without_redraw_feedback() {
        let window = window::Id::unique();
        for status in [event::Status::Captured, event::Status::Ignored] {
            assert!(matches!(
                observed_event(
                    iced::Event::Keyboard(keyboard::Event::ModifiersChanged(
                        keyboard::Modifiers::CTRL
                    )),
                    status,
                    window
                ),
                Some(Message::Raw { .. })
            ));
            assert!(matches!(
                observed_event(
                    iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                    status,
                    window
                ),
                Some(Message::Raw { .. })
            ));
            assert!(
                observed_event(iced::Event::Window(window::Event::Focused), status, window)
                    .is_none()
            );
        }
    }
}
