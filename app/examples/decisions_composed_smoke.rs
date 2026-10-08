//! Fixed PUBLIC real App -> worker -> Gateway -> registered root/turn -> interaction services.
//! Physical business messages are dropped; no model step, tool operation or account sign-in is requested.
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
#[path = "../src/worker.rs"]
mod worker;
mod ui {
    include!("../src/ui.rs");
    pub(super) fn fixture_ready(app: &App) -> bool {
        app.ready && app.root_ready && !app.roster_pending && !app.stopped
    }
    pub(super) fn fixture_count(app: &App) -> usize {
        app.decisions.len()
    }
}
use dsh_native_transport::dto::{ApprovalOutcome, RemoteEventFrame, WaterfallKind};
use iced::widget::{column, container};
use iced::{Element, Subscription, Task, window};
use serde_json::{Value, json};
use std::{
    fs,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::Duration,
};
const NOTE: &str = "PUBLIC_NATIVE_DECISION_FIXTURE_NOTE";
#[derive(Clone)]
struct Options {
    output: PathBuf,
    app: config::Options,
    minimum_viewport: bool,
}
fn private_empty(path: &Path, boundary: &Path) -> Result<(), &'static str> {
    let meta = fs::symlink_metadata(path).map_err(|_| "private-directory-required")?;
    let canonical = path
        .canonicalize()
        .map_err(|_| "private-directory-required")?;
    if !path.is_absolute()
        || !meta.is_dir()
        || meta.file_type().is_symlink()
        || meta.permissions().mode() & 0o7777 != 0o700
        || meta.uid()
            != fs::metadata("/proc/self")
                .map_err(|_| "ownership-unavailable")?
                .uid()
        || !canonical.starts_with(boundary)
        || fs::read_dir(path)
            .map_err(|_| "directory-unavailable")?
            .next()
            .is_some()
    {
        return Err("new-empty-private-directory-required");
    }
    Ok(())
}
fn private_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}
fn options() -> Result<Options, &'static str> {
    let mut args = std::env::args().skip(1);
    let mut output = None;
    let mut scale = None;
    let mut viewport = None;
    while let Some(key) = args.next() {
        let value = args.next().ok_or("missing-value")?;
        match key.as_str() {
            "--output" if output.is_none() => output = Some(PathBuf::from(value)),
            "--scale" if scale.is_none() => {
                scale = Some(value.parse::<f32>().map_err(|_| "invalid-scale")?)
            }
            "--viewport" if viewport.is_none() => {
                viewport = Some(match value.as_str() {
                    "full" => false,
                    "minimum" => true,
                    _ => return Err("invalid-viewport"),
                })
            }
            _ => return Err("invalid-or-duplicate-argument"),
        }
    }
    let output = output.ok_or("output-required")?;
    let scale = scale.unwrap_or(1.0);
    if ![1.0, 1.25].contains(&scale) {
        return Err("invalid-scale");
    }
    let boundary = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("evidence")
        .canonicalize()
        .map_err(|_| "boundary-unavailable")?;
    let meta = fs::symlink_metadata(&output).map_err(|_| "output-unavailable")?;
    let canonical = output.canonicalize().map_err(|_| "output-unavailable")?;
    if !output.is_absolute()
        || !meta.is_dir()
        || meta.file_type().is_symlink()
        || meta.permissions().mode() & 0o7777 != 0o700
        || meta.uid()
            != fs::metadata("/proc/self")
                .map_err(|_| "ownership-unavailable")?
                .uid()
        || !canonical.starts_with(&boundary)
    {
        return Err("new-private-evidence-output-required");
    }
    let names = [
        "harness",
        "workspace",
        "user",
        "config",
        "cache",
        "data",
        "state",
    ];
    for entry in fs::read_dir(&output).map_err(|_| "output-unavailable")? {
        if !names
            .iter()
            .any(|name| entry.as_ref().is_ok_and(|entry| entry.file_name() == *name))
        {
            return Err("unexpected-output-content");
        }
    }
    for name in names {
        private_empty(&output.join(name), &canonical)?;
    }
    let runtime = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../deepseek-harness-linux/apps/cli")
        .canonicalize()
        .map_err(|_| "explicit-owned-alpha-fork-required")?;
    let app = config::parse(vec![
        "--runtime".into(),
        runtime.to_string_lossy().into_owned(),
        "--expected-version".into(),
        "0.2.1-alpha.1".into(),
        "--home".into(),
        canonical.join("harness").to_string_lossy().into_owned(),
        "--cwd".into(),
        canonical.join("workspace").to_string_lossy().into_owned(),
        "--user-home".into(),
        canonical.join("user").to_string_lossy().into_owned(),
        "--scale".into(),
        scale.to_string(),
    ])
    .map_err(|_| "fixture-options-invalid")?
    .ok_or("fixture-options-required")?;
    Ok(Options {
        output: canonical,
        app,
        minimum_viewport: viewport.unwrap_or(false),
    })
}
fn stage_profile(options: &Options) -> Result<(), &'static str> {
    let fixture = options.output.join("fixture");
    let profile = options.output.join("harness/profiles/desktop");
    for path in [
        &fixture,
        &fixture.join("control"),
        &options.output.join("harness/profiles"),
        &profile,
    ] {
        fs::create_dir(path).map_err(|_| "fixture-directory-create")?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|_| "fixture-directory-private")?;
    }
    let module = fixture.join("decision-host.mjs");
    private_write(&module, include_bytes!("support/decision_host_fixture.mjs"))
        .map_err(|_| "fixture-module-create")?;
    let plugin = json!(module).to_string();
    let runtime = json!(options.app.backend.runtime).to_string();
    let workspace = json!(options.app.backend.working_directory).to_string();
    let patch = format!(
        "- id: approval\n  config:\n    policy: ask\n- insert:\n    - id: native-decision-host-fixture\n      name: {plugin}\n      config:\n        runtime: {runtime}\n        workspace: {workspace}\n"
    );
    private_write(&profile.join("cordis.patch.yml"), patch.as_bytes())
        .map_err(|_| "fixture-profile-create")
}
fn private_json(path: &Path) -> Option<Value> {
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.is_file()
        || meta.file_type().is_symlink()
        || meta.permissions().mode() & 0o7777 != 0o600
        || meta.len() > 16384
    {
        return None;
    }
    let mut bytes = Vec::new();
    fs::OpenOptions::new()
        .read(true)
        .custom_flags(0o400000)
        .open(path)
        .ok()?
        .take(16385)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 16384 {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}
#[derive(Clone)]
enum Message {
    Ui(ui::Message),
    Tick,
    Deadline,
    Capture(window::Id),
    Screenshot(window::Screenshot),
    Saved(bool),
}
impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeDecisionFixture(redacted)")
    }
}
struct Fixture {
    options: Options,
    app: ui::App,
    phase: u8,
    active: Option<interactions::Key>,
    attempt: Option<u64>,
    ack: bool,
    cancel: bool,
    window: Option<window::Id>,
    native: bool,
    roster_succeeded: bool,
    stopping: bool,
    failed: bool,
    completed: bool,
    report: Value,
}
impl Fixture {
    fn boot(options: Options, handle: worker::Handle, feed: worker::Feed) -> (Self, Task<Message>) {
        let (app, task) = ui::App::boot(options.app.clone(), handle, feed);
        let deadline = Task::perform(
            async {
                tokio::time::sleep(Duration::from_secs(25)).await;
            },
            |_| Message::Deadline,
        );
        let report = json!({"scope":"scripted actual native UI/worker/Gateway/live root/turn/interaction services","pid":std::process::id(),"publicFixtureOnly":true,"scriptedUiMessages":true,"nativeKeyboardPointerInputQualified":false,"realCredentialsUsed":false,"modelCatalogRequested":false,"browserSignInRequested":false,"manualBusinessMessagesForwarded":false,"applicationScale":options.app.scale,"logicalViewport":if options.minimum_viewport {Some([608,448])} else {None},"nativeAcks":0,"hostSettlements":0,"timedUiEnabled":false,"mountedDefaultDesktopServices":true,"outboundPacketTracePerformed":false});
        (
            Self {
                options,
                app,
                phase: 0,
                active: None,
                attempt: None,
                ack: false,
                cancel: false,
                window: None,
                native: false,
                roster_succeeded: false,
                stopping: false,
                failed: false,
                completed: false,
                report,
            },
            Task::batch([task.map(Message::Ui), deadline]),
        )
    }
    fn dispatch(&mut self, action: interactions::Action) -> Task<Message> {
        self.app
            .update(ui::Message::Interaction(action))
            .map(Message::Ui)
    }
    fn mark(&self, name: &str) -> bool {
        private_write(
            &self.options.output.join("fixture/control").join(name),
            b"PUBLIC\n",
        )
        .is_ok()
    }
    fn stop(&mut self, error: Option<&'static str>) -> Task<Message> {
        if let Some(error) = error {
            self.failed = true;
            self.report["errorCode"] = json!(error);
        }
        if self.stopping {
            return Task::none();
        }
        self.stopping = true;
        self.app.update(ui::Message::ConfirmClose).map(Message::Ui)
    }
    fn submit(&mut self) -> Task<Message> {
        let Some(key) = self.active.clone() else {
            return self.stop(Some("fixture-active-key-missing"));
        };
        let task = match self.phase {
            1 => self.dispatch(interactions::Action::Approval(
                key.clone(),
                ApprovalOutcome::AllowedOnce,
            )),
            2 => {
                let mut tasks = Vec::new();
                for option in [0, 1] {
                    tasks.push(self.dispatch(interactions::Action::Toggle {
                        key: key.clone(),
                        question: 0,
                        option,
                    }));
                }
                tasks.push(self.dispatch(interactions::Action::Custom {
                    key: key.clone(),
                    question: 1,
                    text: NOTE.into(),
                }));
                tasks.push(self.dispatch(interactions::Action::Skip {
                    key: key.clone(),
                    question: 2,
                }));
                tasks.push(self.dispatch(interactions::Action::Submit(key.clone())));
                Task::batch(tasks)
            }
            3 => self.dispatch(interactions::Action::CancelQuestion(key.clone())),
            _ => return self.stop(Some("unexpected-submit-phase")),
        };
        // This closed script emits exactly one explicit submission per phase; no physical actions enter.
        self.attempt = Some(u64::from(self.phase));
        if ui::fixture_count(&self.app) != 1 {
            return self.stop(Some("ui-live-delivery-missing"));
        }
        task
    }
    fn tick(&mut self) -> Task<Message> {
        if self.stopping {
            return Task::none();
        }
        let control = self.options.output.join("fixture/control");
        if control.join("failed.json").exists() {
            return self.stop(Some("fixed-host-fixture-failure"));
        }
        if self.phase == 0
            && self.roster_succeeded
            && ui::fixture_ready(&self.app)
            && private_json(&control.join("ready.json"))
                .is_some_and(|value| value["rootRegistered"] == true)
        {
            self.report["realWorkerAndRootReady"] = json!(true);
            self.phase = 1;
            if !self.mark("go-1") {
                return self.stop(Some("marker-create-failed"));
            }
        }
        if (1..=4).contains(&self.phase) {
            if let Some(value) = private_json(&control.join(format!("phase-{}.json", self.phase))) {
                if value["phase"] != json!(self.phase) || value["settled"] != true {
                    return self.stop(Some("host-settlement-mismatch"));
                }
                if self.ack || (self.phase == 4 && self.cancel) {
                    if self
                        .active
                        .as_ref()
                        .is_none_or(|_| ui::fixture_count(&self.app) != 0)
                    {
                        return self.stop(Some("ui-delivery-not-retired"));
                    }
                    self.report["hostSettlements"] = json!(self.phase);
                    self.active = None;
                    self.attempt = None;
                    self.ack = false;
                    self.cancel = false;
                    self.phase += 1;
                    if self.phase <= 4 && !self.mark(&format!("go-{}", self.phase)) {
                        return self.stop(Some("marker-create-failed"));
                    }
                }
            }
        }
        if self.phase == 5 {
            if let Some(value) = private_json(&control.join("complete.json")) {
                if value["disposed"] != true
                    || value["turnClosed"] != true
                    || value["approvalAuditPair"] != true
                    || value["zeroAdmittedModelSteps"] != true
                {
                    return self.stop(Some("host-turn-or-model-step-evidence-invalid"));
                }
                self.report["host"] = value;
                self.report["liveHostDecisionQualified"] = json!(true);
                return self.stop(None);
            }
        }
        Task::none()
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Ui(ui::Message::NativeOpened(id)) => {
                self.window = Some(id);
                self.app
                    .update(ui::Message::NativeOpened(id))
                    .map(Message::Ui)
            }
            Message::Ui(message @ ui::Message::NativeInfo { .. }) => {
                if let ui::Message::NativeInfo { backend, .. } = &message {
                    self.native = *backend == "wayland";
                    self.report["windowBackend"] = json!(backend);
                }
                self.app.update(message).map(Message::Ui)
            }
            Message::Ui(ui::Message::Close) => self.stop(Some("manual-close-before-completion")),
            Message::Ui(ui::Message::Worker(event)) => {
                let task = self
                    .app
                    .update(ui::Message::Worker(event.clone()))
                    .map(Message::Ui);
                match event {
                    worker::Event::Roster(Ok(_)) => {
                        self.roster_succeeded = true;
                        self.report["actualRosterSucceeded"] = json!(true);
                        task
                    }
                    worker::Event::Roster(Err(_)) => self.stop(Some("actual-roster-failed")),
                    worker::Event::Root {
                        frame:
                            RemoteEventFrame::Waterfall {
                                agent_id, event, ..
                            },
                        decision_key: Some(key),
                    } if !self.stopping => {
                        if self.active.is_some()
                            || !matches!(
                                (self.phase, event),
                                (1, WaterfallKind::Approval) | (2..=4, WaterfallKind::Question)
                            )
                        {
                            return self.stop(Some("unexpected-live-delivery"));
                        }
                        let host =
                            private_json(&self.options.output.join("fixture/control/ready.json"));
                        if host
                            .as_ref()
                            .is_none_or(|value| value["actualAgentId"] != agent_id.as_str())
                        {
                            return self.stop(Some("actual-root-agent-correlation-failed"));
                        }
                        self.report["actualRootAgentCorrelated"] = json!(true);
                        self.active = Some(key.clone());
                        let open = self.dispatch(interactions::Action::Open(key));
                        if self.phase <= 2 {
                            let Some(id) = self.window else {
                                return self.stop(Some("own-window-unavailable"));
                            };
                            let capture = Task::perform(
                                async {
                                    tokio::time::sleep(Duration::from_millis(500)).await;
                                },
                                move |_| Message::Capture(id),
                            );
                            Task::batch([task, open, capture])
                        } else if self.phase == 3 {
                            let submit = self.submit();
                            Task::batch([task, open, submit])
                        } else {
                            if !self.mark("abort-4") {
                                return self.stop(Some("abort-marker-create-failed"));
                            }
                            Task::batch([task, open])
                        }
                    }
                    worker::Event::DecisionResult { submission, result }
                        if self.active.as_ref() == Some(&submission.key)
                            && self.attempt == Some(submission.attempt) =>
                    {
                        if result.is_err() {
                            return self.stop(Some("native-ack-error-indeterminate"));
                        }
                        self.ack = true;
                        self.report["nativeAcks"] = json!(self.phase);
                        task
                    }
                    worker::Event::Root {
                        frame: RemoteEventFrame::Cancel { event_id },
                        ..
                    } if self.phase == 4
                        && self
                            .active
                            .as_ref()
                            .is_some_and(|key| key.event_id == event_id) =>
                    {
                        self.cancel = true;
                        self.report["hostAbortCancelObserved"] = json!(true);
                        task
                    }
                    worker::Event::Fault(_) => self.stop(Some("fixed-worker-failure")),
                    worker::Event::NoBackendStarted => {
                        self.failed = true;
                        self.finish(false);
                        iced::exit()
                    }
                    worker::Event::Stopped(result) => {
                        let clean=result.is_ok_and(|stop|{self.report["stop"]=json!({"exited":stop.exited,"exitCode":stop.exit_code,"graceful":stop.graceful,"containmentUnknown":stop.containment_unknown,"observedDescendantsRemaining":stop.observed_descendants_remaining});stop.exited&&stop.exit_code==Some(0)&&stop.graceful&&!stop.containment_unknown&&stop.observed_descendants_remaining==0});
                        self.finish(
                            clean
                                && !self.failed
                                && self.native
                                && self.report["nativeAcks"] == 3
                                && self.report["hostSettlements"] == 4
                                && self.report["liveHostDecisionQualified"] == true,
                        );
                        Task::batch([task, iced::exit()])
                    }
                    _ => task,
                }
            }
            Message::Capture(id)
                if !self.stopping && self.window == Some(id) && self.phase <= 2 =>
            {
                window::screenshot(id).map(Message::Screenshot)
            }
            Message::Screenshot(screenshot) if !self.stopping && self.phase <= 2 => {
                let path = self
                    .options
                    .output
                    .join(format!("decision-{}.png", self.phase));
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            smoke::save_screenshot(&path, &screenshot).is_ok()
                        })
                        .await
                        .unwrap_or(false)
                    },
                    Message::Saved,
                )
            }
            Message::Saved(true) if !self.stopping && self.phase <= 2 => self.submit(),
            Message::Saved(false) => self.stop(Some("own-renderer-capture-failed")),
            Message::Tick => self.tick(),
            Message::Deadline => self.stop(Some("composed-fixture-deadline")),
            _ => Task::none(), // Never forward physical/business/edit/model/credential messages.
        }
    }
    fn finish(&mut self, passed: bool) {
        if self.completed {
            return;
        }
        self.completed = true;
        self.report["status"] = json!(if passed { "passed" } else { "failed" });
        let _ = private_write(
            &self.options.output.join("app.json"),
            serde_json::to_vec_pretty(&self.report).unwrap().as_slice(),
        );
    }
    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            self.app.subscription().map(Message::Ui),
            iced::time::every(Duration::from_millis(100)).map(|_| Message::Tick),
        ])
    }
    fn view(&self) -> Element<'_, Message> {
        let view = container(self.app.view().map(Message::Ui));
        let view = if self.options.minimum_viewport {
            view.width(iced::Length::Fixed(608.0))
                .height(iced::Length::Fixed(448.0))
        } else {
            view.width(iced::Length::Fill).height(iced::Length::Fill)
        };
        let frame = container(view).center_x(iced::Length::Fill);
        let frame = if self.options.minimum_viewport {
            frame
        } else {
            frame.height(iced::Length::Fill)
        };
        container(column![ui::label("PUBLIC LIVE DECISION FIXTURE · manual business actions blocked · no model step or tool execution",ui::DANGER),frame].spacing(12).width(iced::Length::Fill).height(iced::Length::Fill)).padding(16).height(iced::Length::Fill).into()
    }
}
#[cfg(test)]
mod fixture_tests {
    use super::*;
    fn fixture() -> (Fixture, tokio::sync::mpsc::Receiver<worker::Command>) {
        let (handle, feed, commands, _events, _closing) = worker::test_channels();
        let app = config::Options {
            backend: dsh_native_core::RustBackendOptions {
                runtime: PathBuf::from("/NONSTARTED-FIXTURE/apps/cli"),
                expected_version: "0.2.1-alpha.1".into(),
                native_home: PathBuf::from("/NONWRITTEN-FIXTURE/harness"),
                working_directory: PathBuf::from("/tmp"),
                user_home: PathBuf::from("/NONWRITTEN-FIXTURE/user"),
                absolute_node_path: None,
                startup_timeout: Duration::from_secs(15),
                stop_policy: Default::default(),
            },
            scale: 1.0,
            smoke: None,
        };
        (
            Fixture::boot(
                Options {
                    output: PathBuf::from("/NONWRITTEN-FIXTURE"),
                    app,
                    minimum_viewport: false,
                },
                handle,
                feed,
            )
            .0,
            commands,
        )
    }
    fn key(serial: u64) -> interactions::Key {
        interactions::Key {
            epoch: 1,
            event_id: dsh_native_transport::dto::RemoteEventId::new("PUBLIC-event").unwrap(),
            serial,
        }
    }
    #[test]
    fn physical_business_actions_never_enter_actual_command_path() {
        let (mut fixture, mut commands) = fixture();
        for message in [
            ui::Message::Send,
            ui::Message::LoadModels,
            ui::Message::Interaction(interactions::Action::Approval(
                key(1),
                ApprovalOutcome::AllowedOnce,
            )),
            ui::Message::NewSession,
        ] {
            let _ = fixture.update(Message::Ui(message));
        }
        assert!(commands.try_recv().is_err());
        assert_eq!(fixture.phase, 0);
    }
    #[test]
    fn wrong_key_or_attempt_receipt_does_not_advance_the_script() {
        let (mut fixture, mut commands) = fixture();
        fixture.phase = 1;
        fixture.active = Some(key(1));
        fixture.attempt = Some(1);
        for (serial, attempt) in [(2, 1), (1, 2)] {
            let _ = fixture.update(Message::Ui(ui::Message::Worker(
                worker::Event::DecisionResult {
                    submission: interactions::Submission {
                        key: key(serial),
                        attempt,
                        reply: interactions::Reply::Approval(ApprovalOutcome::AllowedOnce),
                    },
                    result: Ok(()),
                },
            )));
        }
        assert!(!fixture.ack);
        assert_eq!(fixture.report["nativeAcks"], 0);
        assert!(commands.try_recv().is_err());
    }
    #[test]
    fn failed_roster_never_authorizes_fixture_marker_start() {
        let (mut fixture, _commands) = fixture();
        let _ = fixture.update(Message::Ui(ui::Message::Worker(worker::Event::Roster(
            Err("PUBLIC-failure".into()),
        ))));
        assert!(!fixture.roster_succeeded);
        assert_eq!(fixture.phase, 0);
        assert!(fixture.failed);
    }
}
fn main() -> iced::Result {
    let options = options().unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2)
    });
    stage_profile(&options).unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2)
    });
    let report = options.output.join("app.json");
    unsafe {
        std::env::set_var("ICED_BACKEND", "wgpu");
    }
    let (handle, feed, owner) =
        worker::start(options.app.backend.clone(), false).unwrap_or_else(|_| {
            eprintln!("Owned fixture worker unavailable");
            std::process::exit(1)
        });
    let result = iced::application(
        move || Fixture::boot(options.clone(), handle.clone(), feed.clone()),
        Fixture::update,
        Fixture::view,
    )
    .title("Harness native live decisions · PUBLIC fixture")
    .window(window::Settings {
        size: iced::Size::new(1200.0, 800.0),
        platform_specific: window::settings::PlatformSpecific {
            application_id: "ai.deepseek.harness.native.decisions-composed-fixture".into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .exit_on_close_request(false)
    .default_font(ui::FONT)
    .theme(|_: &Fixture| ui::native_theme())
    .scale_factor(|f: &Fixture| f.options.app.scale)
    .subscription(Fixture::subscription)
    .run();
    drop(owner);
    if !private_json(&report).is_some_and(|value| value["status"] == "passed") {
        eprintln!("Composed decision fixture failed or report unavailable");
        std::process::exit(1)
    }
    result
}
