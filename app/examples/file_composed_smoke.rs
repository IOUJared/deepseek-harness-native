//! Scripted production App/Worker/Core/Alpha file flow in an own native window; no model step.
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
    pub(super) fn startup_ready(a: &App) -> bool {
        a.ready && a.root_ready && !a.roster_pending && a.management.registry.ready() && !a.stopped
    }
    pub(super) fn selected_ready(a: &App, target: &SessionId) -> bool {
        a.selected.as_ref() == Some(target)
            && !a.roster_pending
            && !a.stopped
            && a.file_context().allowed
    }
    pub(super) fn review_messages(a: &App, path: String) -> Option<(Message, Message)> {
        let (stamp, _) = a.files.editor()?;
        Some((Message::FilePath(stamp, path), Message::FileUpload(stamp)))
    }
    pub(super) fn upload_message(a: &App) -> Option<Message> {
        a.files
            .editor()
            .map(|(stamp, _)| Message::FileUpload(stamp))
    }
    pub(super) fn review_open(a: &App) -> bool {
        a.files.editor_open() && !a.files.pending() && !a.files.prompt_pending()
    }
    pub(super) fn file_ticket(a: &App) -> Option<worker::attachments::Ticket> {
        a.files.ticket().cloned()
    }
    pub(super) fn ready_ticket(a: &App) -> Option<worker::attachments::Ticket> {
        a.files
            .has_file(&a.file_context())
            .then(|| a.files.ready().unwrap().0.clone())
    }
    pub(super) fn send_ready(a: &App) -> bool {
        a.send_allowed() && a.files.has_file(&a.file_context())
    }
    pub(super) fn request(
        a: &App,
        ticket: &worker::attachments::Ticket,
    ) -> Option<SessionRequestId> {
        a.pending
            .get(&ticket.generation)
            .filter(|p| p.session == ticket.target)
            .and_then(|p| p.file_request_id.clone())
    }
    pub(super) fn composer_text(a: &App) -> String {
        a.editor.text()
    }
    pub(super) fn counters(a: &App) -> (u64, u64, bool) {
        (
            a.prompt_serial,
            a.smoke_evidence.model_prompts,
            a.smoke_evidence.catalog_requested,
        )
    }
    pub(super) fn projection(a: &App) -> (bool, usize, usize, usize) {
        (
            a.records_expanded,
            a.transcript.rows().len(),
            a.transcript.display_rows(a.records_expanded).count(),
            a.transcript
                .rows()
                .iter()
                .filter(|r| r.role == "agent/inbox/spliced" && !a.transcript.inbox_canceled(r.key))
                .count(),
        )
    }
    pub(super) fn cleared(a: &App) -> bool {
        a.files.ticket().is_none()
            && !a.files.pending()
            && !a.files.prompt_pending()
            && a.pending.is_empty()
            && a.editor.text().is_empty()
    }
}
use dsh_native_transport::dto::*;
use iced::widget::{column, container, text_editor};
use iced::{Element, Subscription, Task, window};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::Duration,
};
use worker::attachments::{Outcome, PromptOutcome, Ticket};
const TEXT: &str = "PUBLIC native file prompt; reject before any model step.";
const CASES: [(&str, u64); 3] = [
    ("PUBLIC owned file.bin", 5003),
    ("PUBLIC empty.bin", 0),
    ("PUBLIC ceiling.bin", 4 * 1024 * 1024),
];
fn private_write(path: &Path, bytes: &[u8]) -> Result<(), &'static str> {
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| "fixture-create")?;
    f.write_all(bytes)
        .and_then(|_| f.sync_all())
        .map_err(|_| "fixture-write")
}
fn private_json(path: &Path) -> Option<Value> {
    let before = fs::symlink_metadata(path).ok()?;
    let uid = fs::metadata("/proc/self").ok()?.uid();
    if !before.is_file()
        || before.file_type().is_symlink()
        || before.permissions().mode() & 0o7777 != 0o600
        || before.uid() != uid
        || before.len() > 16384
    {
        return None;
    }
    let f = fs::OpenOptions::new()
        .read(true)
        .custom_flags(0o400000 | 0o4000 | 0o2000000)
        .open(path)
        .ok()?;
    let opened = f.metadata().ok()?;
    if !opened.is_file()
        || (opened.dev(), opened.ino()) != (before.dev(), before.ino())
        || opened.len() > 16384
        || opened.uid() != uid
        || opened.permissions().mode() & 0o7777 != 0o600
    {
        return None;
    }
    let mut bytes = Vec::new();
    f.take(16385).read_to_end(&mut bytes).ok()?;
    if bytes.len() > 16384 {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}
fn stage(options: &config::Options) -> Result<PathBuf, &'static str> {
    if options.smoke.is_some() {
        return Err("smoke-options-not-supported");
    }
    let backend = &options.backend;
    let home = backend
        .native_home
        .canonicalize()
        .map_err(|_| "fixture-home")?;
    let output = home.parent().ok_or("fixture-output")?.to_owned();
    let uid = fs::metadata("/proc/self").map_err(|_| "fixture-uid")?.uid();
    for (path, name, empty) in [
        (&backend.native_home, "harness", true),
        (&backend.working_directory, "workspace", false),
        (&backend.user_home, "user", true),
    ] {
        let info = fs::symlink_metadata(path).map_err(|_| "fixture-directory")?;
        if !info.is_dir()
            || info.file_type().is_symlink()
            || info.uid() != uid
            || info.permissions().mode() & 0o7777 != 0o700
            || path.canonicalize().map_err(|_| "fixture-directory")? != *path
            || path.parent() != Some(output.as_path())
            || path.file_name().is_none_or(|v| v != name)
        {
            return Err("isolated-private-sibling-layout-required");
        }
        if empty
            && fs::read_dir(path)
                .map_err(|_| "fixture-directory")?
                .next()
                .is_some()
        {
            return Err("new-empty-private-home-required");
        }
    }
    let fixture = output.join("fixture");
    let profile = home.join("profiles/desktop");
    for p in [
        &fixture,
        &fixture.join("control"),
        &home.join("profiles"),
        &profile,
    ] {
        fs::create_dir(p).map_err(|_| "fixture-directory-create")?;
        fs::set_permissions(p, fs::Permissions::from_mode(0o700))
            .map_err(|_| "fixture-directory-mode")?;
    }
    let module = fixture.join("file-prompt-host.mjs");
    private_write(
        &module,
        include_bytes!("support/file_prompt_host_fixture.mjs"),
    )?;
    let plugin = json!(module).to_string();
    let runtime = json!(backend.runtime).to_string();
    let workspace = json!(backend.working_directory).to_string();
    private_write(&profile.join("cordis.patch.yml"), format!("- insert:\n    - id: native-file-prompt-host-fixture\n      name: {plugin}\n      config:\n        runtime: {runtime}\n        workspace: {workspace}\n        appRequests: true\n").as_bytes())?;
    Ok(output)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Startup,
    Selecting,
    Review(usize),
    Upload(usize),
    Probe(usize, u8),
    Sent(usize),
    HostComplete,
    Stopping,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Capture {
    Review,
    Ready(usize),
    Records,
}
impl Capture {
    fn name(self) -> &'static str {
        match self {
            Self::Review => "file-review.png",
            Self::Ready(0) => "file-ready-1.png",
            Self::Ready(1) => "file-ready-2.png",
            Self::Ready(_) => "unsupported.png",
            Self::Records => "file-records.png",
        }
    }
}
#[derive(Clone, Debug)]
enum Message {
    Ui(ui::Message),
    Tick,
    Deadline,
    Capture(Capture),
    Screenshot(Capture, window::Screenshot),
}
struct Fixture {
    options: config::Options,
    output: PathBuf,
    app: ui::App,
    handle: worker::Handle,
    phase: Phase,
    subject: Option<SessionId>,
    ticket: Option<Ticket>,
    request: Option<SessionRequestId>,
    old_ticket: Option<Ticket>,
    old_ack: Option<worker::Event>,
    ack: bool,
    window: Option<window::Id>,
    native: bool,
    failed: bool,
    completed: bool,
    capture: Option<Capture>,
    capture_cancel: tokio::sync::watch::Sender<bool>,
    report: Value,
}
impl Fixture {
    fn boot(
        options: config::Options,
        output: PathBuf,
        handle: worker::Handle,
        feed: worker::Feed,
    ) -> (Self, Task<Message>) {
        let (app, task) = ui::App::boot(options.clone(), handle.clone(), feed);
        let (capture_cancel, _) = tokio::sync::watch::channel(false);
        let deadline = Task::perform(
            async {
                tokio::time::sleep(Duration::from_secs(26)).await;
            },
            |_| Message::Deadline,
        );
        let report = json!({"status":"running","version":"0.2.1-alpha.1","pid":std::process::id(),"scope":"scripted actual native App/worker/Core/full Alpha Host file intake and prompt admission; no model step or history retirement","applicationScale":options.scale,"actualAppExercised":true,"nativeWorkerExercised":true,"scriptedUiMessages":true,"publicFixtureOnly":true,"modelCatalogRequested":false,"browserSignInRequested":false,"realCredentialsUsed":false,"processWideNetworkTrace":false,"nativeKeyboardPointerInputQualified":false,"historyDrivenReceiptRetirementQualified":false,"manualBusinessMessagesForwarded":0,"unexpectedWork":0,"explicitModelPrompts":0,"explicitPrompts":0,"actualAppPromptCommands":0,"actualFileStages":0,"nativeAcks":0,"hostSettlements":0,"currentTicketProbeRefusals":0,"staleUploadRefusals":0,"duplicateSendRefusals":0,"staleRemoveRefusals":0,"staleAckRefusals":0,"recordsDisclosureRoundTrips":0,"quietProjectionObserved":false,"retainedRecordsRevealed":false,"files":CASES.map(|(name,bytes)|json!({"name":name,"bytes":bytes})),"requests":[],"tickets":[],"phases":[],"captures":[]});
        (
            Self {
                options,
                output,
                app,
                handle,
                phase: Phase::Startup,
                subject: None,
                ticket: None,
                request: None,
                old_ticket: None,
                old_ack: None,
                ack: false,
                window: None,
                native: false,
                failed: false,
                completed: false,
                capture: None,
                capture_cancel,
                report,
            },
            Task::batch([task.map(Message::Ui), deadline]),
        )
    }
    fn dispatch(&mut self, message: ui::Message) -> Task<Message> {
        self.app.update(message).map(Message::Ui)
    }
    fn increment(&mut self, key: &str) {
        self.report[key] = json!(self.report[key].as_u64().unwrap() + 1);
    }
    fn stop(&mut self, error: Option<&'static str>) -> Task<Message> {
        if let Some(e) = error {
            self.failed = true;
            self.report["errorCode"] = json!(e);
        }
        if self.phase == Phase::Stopping {
            return Task::none();
        }
        self.capture_cancel.send_replace(true);
        self.phase = Phase::Stopping;
        self.dispatch(ui::Message::ConfirmClose)
    }
    fn start_review(&mut self, index: usize) -> Task<Message> {
        if !ui::selected_ready(&self.app, self.subject.as_ref().unwrap()) || !ui::cleared(&self.app)
        {
            return self.stop(Some("file-context-not-current-or-empty"));
        }
        let mut tasks = vec![self.dispatch(ui::Message::FileOpen)];
        let path = self
            .options
            .backend
            .working_directory
            .join(CASES[index].0)
            .to_string_lossy()
            .into_owned();
        let Some((edit, stale_upload)) = ui::review_messages(&self.app, path) else {
            return self.stop(Some("file-review-not-open"));
        };
        tasks.push(self.dispatch(edit));
        tasks.push(self.dispatch(stale_upload));
        if !ui::review_open(&self.app) || ui::file_ticket(&self.app).is_some() {
            return self.stop(Some("stale-upload-consent-not-refused"));
        }
        self.increment("staleUploadRefusals");
        self.phase = Phase::Review(index);
        if index == 0 {
            tasks.push(self.schedule_capture(Capture::Review));
        } else {
            tasks.push(self.upload(index));
        }
        Task::batch(tasks)
    }
    fn schedule_capture(&mut self, capture: Capture) -> Task<Message> {
        if self.capture.is_some() || self.window.is_none() {
            return self.stop(Some("capture-owner-not-available"));
        }
        self.capture = Some(capture);
        let id = self.window.unwrap();
        let mut closing = self.capture_cancel.subscribe();
        // No intermediate App message: rebuilding editor layout would invalidate
        // its old renderer Weak before Iced screenshots the previous primitive list.
        Task::future(async move {
            if *closing.borrow() {
                return false;
            }
            tokio::select! {
                biased;
                _ = closing.changed() => false,
                _ = tokio::time::sleep(Duration::from_millis(350)) => !*closing.borrow(),
            }
        })
        .then(move |ready| {
            if ready {
                window::screenshot(id).map(move |s| Message::Screenshot(capture, s))
            } else {
                Task::none()
            }
        })
    }
    fn upload(&mut self, index: usize) -> Task<Message> {
        let Some(message) = ui::upload_message(&self.app) else {
            return self.stop(Some("current-upload-consent-unavailable"));
        };
        let task = self.dispatch(message);
        let Some(ticket) = ui::file_ticket(&self.app) else {
            return self.stop(Some("production-upload-ticket-missing"));
        };
        if ticket.target != *self.subject.as_ref().unwrap() || ticket.serial != (index + 1) as u64 {
            return self.stop(Some("production-upload-ticket-scope"));
        }
        self.report["tickets"].as_array_mut().unwrap().push(json!({"epoch":ticket.epoch,"generation":ticket.generation,"serial":ticket.serial,"target":ticket.target.as_str()}));
        self.ticket = Some(ticket);
        self.phase = Phase::Upload(index);
        task
    }
    fn ready(&mut self, index: usize) -> Task<Message> {
        let Some(ticket) = self.ticket.clone() else {
            return self.stop(Some("ready-owner-missing"));
        };
        if ui::ready_ticket(&self.app).as_ref() != Some(&ticket) || !ui::send_ready(&self.app) {
            return self.stop(Some("actual-upload-not-ready-in-App"));
        }
        let mut tasks = Vec::new();
        if let Some(old) = self.old_ticket.clone() {
            tasks.push(self.dispatch(ui::Message::FileRemove(old)));
            if ui::ready_ticket(&self.app).as_ref() != Some(&ticket) {
                return self.stop(Some("old-remove-cleared-current-file"));
            }
            self.increment("staleRemoveRefusals");
        }
        if let Some(old) = self.old_ack.clone() {
            let before = ui::composer_text(&self.app);
            tasks.push(self.dispatch(ui::Message::Worker(old)));
            if ui::ready_ticket(&self.app).as_ref() != Some(&ticket)
                || ui::composer_text(&self.app) != before
            {
                return self.stop(Some("real-old-ack-cleared-current-draft"));
            }
            self.increment("staleAckRefusals");
        }
        if index != 1 {
            tasks.push(self.dispatch(ui::Message::Editor(text_editor::Action::Edit(
                text_editor::Edit::Paste(std::sync::Arc::new(TEXT.into())),
            ))));
        }
        if ui::composer_text(&self.app) != if index == 1 { "" } else { TEXT } {
            return self.stop(Some("actual-editor-content-mismatch"));
        }
        if index < 2 {
            tasks.push(self.schedule_capture(Capture::Ready(index)));
        } else {
            tasks.push(self.probe(index, 0));
        }
        Task::batch(tasks)
    }
    fn probe(&mut self, index: usize, kind: u8) -> Task<Message> {
        let ticket = self.ticket.clone().unwrap();
        let request = SessionPromptRequest {
            request_id: SessionRequestId::new(format!(
                "PUBLIC_native_rejected_{}_{}",
                index + 1,
                kind
            ))
            .unwrap(),
            session_id: ticket.target.clone(),
            mode: if kind == 0 {
                PromptMode::Queue
            } else {
                PromptMode::Steer
            },
            content: if kind == 0 {
                vec![PromptContentPart::File {
                    receipt_id: FileUploadReceiptId::new("PUBLIC_FORGED_FILE_RECEIPT").unwrap(),
                }]
            } else {
                vec![PromptContentPart::Text { text: TEXT.into() }]
            },
            client_time_zone: Some("UTC".into()),
        };
        if self
            .handle
            .send(worker::Command::PromptFile { ticket, request })
            .is_err()
        {
            return self.stop(Some("probe-queue-not-admitted"));
        }
        self.phase = Phase::Probe(index, kind);
        Task::none()
    }
    fn send(&mut self, index: usize) -> Task<Message> {
        let ticket = self.ticket.clone().unwrap();
        if !ui::send_ready(&self.app) {
            return self.stop(Some("explicit-file-Send-not-available"));
        }
        let task = self.dispatch(ui::Message::Send);
        let Some(request) = ui::request(&self.app, &ticket) else {
            return self.stop(Some("production-file-prompt-owner-missing"));
        };
        let before = ui::counters(&self.app);
        let again = self.dispatch(ui::Message::Send);
        if ui::request(&self.app, &ticket).as_ref() != Some(&request)
            || ui::counters(&self.app) != before
            || ui::send_ready(&self.app)
        {
            return self.stop(Some("duplicate-UI-Send-not-refused"));
        }
        self.increment("duplicateSendRefusals");
        self.increment("explicitPrompts");
        self.report["actualAppPromptCommands"] = json!(before.1);
        self.report["requests"]
            .as_array_mut()
            .unwrap()
            .push(json!(request.as_str()));
        self.request = Some(request);
        self.ack = false;
        self.phase = Phase::Sent(index);
        Task::batch([task, again])
    }
    fn tick(&mut self) -> Task<Message> {
        if self.phase == Phase::Stopping {
            return Task::none();
        }
        let control = self.output.join("fixture/control");
        if fs::symlink_metadata(control.join("failed.json")).is_ok() {
            return self.stop(Some("fixed-Host-fixture-failed"));
        }
        match self.phase {
            Phase::Startup if self.native && ui::startup_ready(&self.app) => {
                let Some(ready) = private_json(&control.join("ready.json")) else {
                    return Task::none();
                };
                let Some(target) = ready["actualAgentId"]
                    .as_str()
                    .and_then(|v| SessionId::new(v).ok())
                else {
                    return self.stop(Some("Host-root-ready-ID-invalid"));
                };
                if ready["rootRegistered"] != true
                    || ready["ordinary"] != true
                    || ready["headerVersion"] != 4
                {
                    return self.stop(Some("Host-root-ready-scope-invalid"));
                }
                self.report["actualRootAgentCorrelated"] = json!(true);
                self.subject = Some(target.clone());
                self.phase = Phase::Selecting;
                self.dispatch(ui::Message::Select(target))
            }
            Phase::Selecting
                if ui::selected_ready(&self.app, self.subject.as_ref().unwrap())
                    && self.report["ordinaryHeaderValidated"] == true =>
            {
                self.start_review(0)
            }
            Phase::Sent(index) if self.ack => {
                let Some(phase) = private_json(&control.join(format!("phase-{}.json", index + 1)))
                else {
                    return Task::none();
                };
                if phase["phase"] != json!(index + 1)
                    || phase["requestId"] != self.request.as_ref().unwrap().as_str()
                    || phase["durableInboxFileMatched"] != true
                    || phase["claimedFileMatched"] != true
                    || phase["blockedTurn"] != true
                    || phase["zeroAdmittedModelSteps"] != true
                {
                    return self.stop(Some("actual-Host-and-App-prompt-mismatch"));
                }
                self.report["phases"].as_array_mut().unwrap().push(phase);
                self.increment("hostSettlements");
                self.old_ticket = self.ticket.take();
                self.request = None;
                if index < 2 {
                    self.start_review(index + 1)
                } else {
                    if private_write(&control.join("go-close"), b"PUBLIC\n").is_err() {
                        return self.stop(Some("Host-close-marker-failed"));
                    }
                    self.phase = Phase::HostComplete;
                    Task::none()
                }
            }
            Phase::HostComplete => {
                let Some(host) = private_json(&control.join("complete.json")) else {
                    return Task::none();
                };
                if host["status"] != "passed"
                    || host["rootDisposed"] != true
                    || host["turnClosed"] != true
                    || host["zeroAdmittedModelSteps"] != true
                {
                    return self.stop(Some("Host-complete-not-qualified"));
                }
                self.report["host"] = host;
                self.stop(None)
            }
            _ => Task::none(),
        }
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Ui(ui::Message::NativeOpened(id)) => {
                self.window = Some(id);
                self.dispatch(ui::Message::NativeOpened(id))
            }
            Message::Ui(message @ ui::Message::NativeInfo { .. }) => {
                if let ui::Message::NativeInfo { backend, .. } = &message {
                    self.native = *backend == "wayland";
                    self.report["windowBackend"] = json!(backend);
                }
                self.dispatch(message)
            }
            Message::Ui(
                message @ (ui::Message::Resized(..)
                | ui::Message::WindowActive(..)
                | ui::Message::MotionFrame(..)
                | ui::Message::ScrollApplied { .. }),
            ) => self.dispatch(message),
            Message::Ui(ui::Message::Close) => self.stop(Some("manual-close-before-complete")),
            Message::Ui(ui::Message::Worker(event)) => {
                let task = self.dispatch(ui::Message::Worker(event.clone()));
                if let worker::Event::Selected {
                    frame: SessionFollowFrame::Event { event },
                    ..
                } = &event
                {
                    if [
                        "step/start",
                        "step/end",
                        "request/header",
                        "tool/call",
                        "tool/result",
                        "assistant/message",
                        "user/message",
                    ]
                    .contains(&event.event_type.as_str())
                    {
                        self.increment("unexpectedWork");
                        return self.stop(Some("unexpected-owned-model-work"));
                    }
                }
                match &event {
                    worker::Event::Roster(Ok(_)) => {
                        self.report["actualRosterSucceeded"] = json!(true);
                        task
                    }
                    worker::Event::Selected {
                        frame: SessionFollowFrame::Snapshot { header, .. },
                        ..
                    } => {
                        if self.subject.as_ref() == Some(&header.id)
                            && header.version == 4
                            && header.origin.is_none()
                        {
                            self.report["ordinaryHeaderValidated"] = json!(true);
                        }
                        task
                    }
                    worker::Event::FileStaged { ticket, outcome }
                        if self.phase != Phase::Stopping =>
                    {
                        let Phase::Upload(index) = self.phase else {
                            return self.stop(Some("unexpected-upload-result"));
                        };
                        if self.ticket.as_ref() != Some(ticket)
                            || outcome
                                != &Outcome::Staged(worker::attachments::Metadata {
                                    name: CASES[index].0.into(),
                                    bytes: CASES[index].1,
                                })
                        {
                            return self.stop(Some("actual-staging-scope-or-metadata-mismatch"));
                        }
                        self.increment("actualFileStages");
                        Task::batch([task, self.ready(index)])
                    }
                    worker::Event::FilePrompt {
                        ticket,
                        request_id,
                        outcome,
                    } if self.phase != Phase::Stopping => match self.phase {
                        Phase::Probe(index, kind) => {
                            if self.ticket.as_ref() != Some(ticket)
                                || request_id.as_str()
                                    != format!("PUBLIC_native_rejected_{}_{}", index + 1, kind)
                                || *outcome != PromptOutcome::NotSent
                                || ui::ready_ticket(&self.app).as_ref() != Some(ticket)
                                || !ui::send_ready(&self.app)
                                || ui::composer_text(&self.app)
                                    != if index == 1 { "" } else { TEXT }
                            {
                                return self.stop(Some("current-ticket-probe-consumed-authority"));
                            }
                            self.increment("currentTicketProbeRefusals");
                            Task::batch([
                                task,
                                if kind == 0 {
                                    self.probe(index, 1)
                                } else {
                                    self.send(index)
                                },
                            ])
                        }
                        Phase::Sent(_)
                            if self.ticket.as_ref() == Some(ticket)
                                && self.request.as_ref() == Some(request_id)
                                && *outcome == PromptOutcome::Accepted
                                && !self.ack =>
                        {
                            if !ui::cleared(&self.app) {
                                return self
                                    .stop(Some("matching-actual-ACK-did-not-clear-current-draft"));
                            }
                            self.old_ack = Some(event.clone());
                            self.ack = true;
                            self.increment("nativeAcks");
                            let before = ui::counters(&self.app);
                            let again = self.dispatch(ui::Message::Send);
                            if ui::counters(&self.app) != before || !ui::cleared(&self.app) {
                                return self.stop(Some("empty-after-ACK-Send-not-refused"));
                            }
                            self.increment("duplicateSendRefusals");
                            Task::batch([task, again])
                        }
                        _ => self.stop(Some("unexpected-or-indeterminate-file-prompt-result")),
                    },
                    worker::Event::Stopped(result) => {
                        let clean = match result {
                            Ok(stop) => {
                                self.report["stop"] = json!({"exited":stop.exited,"exitCode":stop.exit_code,"graceful":stop.graceful,"containmentUnknown":stop.containment_unknown,"observedDescendantsRemaining":stop.observed_descendants_remaining});
                                stop.exited
                                    && stop.exit_code == Some(0)
                                    && stop.graceful
                                    && !stop.containment_unknown
                                    && stop.observed_descendants_remaining == 0
                            }
                            Err(_) => false,
                        };
                        self.finish(
                            clean
                                && !self.failed
                                && self.native
                                && self.report["nativeAcks"] == 3
                                && self.report["hostSettlements"] == 3
                                && self.report["host"]["status"] == "passed",
                        );
                        Task::batch([task, iced::exit()])
                    }
                    worker::Event::Fault(_)
                    | worker::Event::FollowError { .. }
                    | worker::Event::Roster(Err(_)) => {
                        self.stop(Some("real-worker-or-follow-failed"))
                    }
                    worker::Event::NoBackendStarted => {
                        self.finish(false);
                        iced::exit()
                    }
                    _ => task,
                }
            }
            Message::Screenshot(capture, screenshot)
                if self.capture == Some(capture) && self.phase != Phase::Stopping =>
            {
                // Synchronous owned finite renderer write: no detached file I/O on close/deadline.
                if smoke::save_screenshot(&self.output.join(capture.name()), &screenshot).is_err() {
                    return self.stop(Some("own-render-buffer-save-failed"));
                }
                self.report["captures"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!(capture.name()));
                self.capture = None;
                match (capture, self.phase) {
                    (Capture::Review, Phase::Review(0)) => self.upload(0),
                    (Capture::Ready(1), Phase::Upload(1)) => {
                        let (expanded, raw, displayed, splices) = ui::projection(&self.app);
                        if expanded || raw < 5 || displayed != 0 || splices < 2 {
                            return self.stop(Some("actual-routine-inbox-projection-not-quiet"));
                        }
                        self.report["quietProjectionObserved"] = json!(true);
                        let task = self.dispatch(ui::Message::ToggleRecords);
                        let (expanded, visible_raw, displayed, _) = ui::projection(&self.app);
                        if !expanded || raw != visible_raw || raw != displayed {
                            return self.stop(Some("actual-records-disclosure-not-retained"));
                        }
                        self.report["retainedRecordsRevealed"] = json!(true);
                        Task::batch([task, self.schedule_capture(Capture::Records)])
                    }
                    (Capture::Records, Phase::Upload(1)) => {
                        let task = self.dispatch(ui::Message::ToggleRecords);
                        if ui::projection(&self.app).0
                            || ui::projection(&self.app).2 != 0
                            || !ui::send_ready(&self.app)
                        {
                            return self.stop(Some("actual-records-disclosure-close-not-quiet"));
                        }
                        self.increment("recordsDisclosureRoundTrips");
                        Task::batch([task, self.probe(1, 0)])
                    }
                    (Capture::Ready(index), Phase::Upload(current)) if index == current => {
                        self.probe(index, 0)
                    }
                    _ => self.stop(Some("capture-phase-owner-changed")),
                }
            }
            Message::Tick => self.tick(),
            Message::Deadline => self.stop(Some("actual-App-file-fixture-deadline")),
            _ => Task::none(), // Physical business/editor/account/model messages are never forwarded.
        }
    }
    fn finish(&mut self, passed: bool) {
        if self.completed {
            return;
        }
        self.completed = true;
        self.report["status"] = json!(if passed { "passed" } else { "failed" });
        self.report["modelCatalogRequested"] = json!(ui::counters(&self.app).2);
        println!("{}", self.report);
    }
    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            self.app.subscription().map(Message::Ui),
            if self.capture.is_none() {
                iced::time::every(Duration::from_millis(80)).map(|_| Message::Tick)
            } else {
                Subscription::none()
            },
        ])
    }
    fn view(&self) -> Element<'_, Message> {
        container(
            column![
                ui::label(
                    "PUBLIC LIVE FILE FLOW · scripted controls · model steps blocked",
                    ui::DANGER
                ),
                self.app.view().map(Message::Ui)
            ]
            .spacing(8),
        )
        .padding(12)
        .height(iced::Length::Fill)
        .into()
    }
}
#[cfg(test)]
#[path = "support/file_composed_fixture_tests.rs"]
mod fixture_tests;
fn main() -> iced::Result {
    let options = config::parse(std::env::args().skip(1))
        .ok()
        .flatten()
        .unwrap_or_else(|| {
            eprintln!("Explicit native fixture options required");
            std::process::exit(2)
        });
    let output = stage(&options).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    });
    unsafe {
        std::env::set_var("ICED_BACKEND", "wgpu");
    }
    let (handle, feed, owner) =
        worker::start(options.backend.clone(), false).unwrap_or_else(|_| {
            eprintln!("Owned native fixture worker unavailable");
            std::process::exit(1)
        });
    let result = iced::application(
        move || {
            Fixture::boot(
                options.clone(),
                output.clone(),
                handle.clone(),
                feed.clone(),
            )
        },
        Fixture::update,
        Fixture::view,
    )
    .title("Harness native file flow · PUBLIC live fixture")
    .window(window::Settings {
        size: iced::Size::new(1000.0, 720.0),
        platform_specific: window::settings::PlatformSpecific {
            application_id: "ai.deepseek.harness.native.file-composed-fixture".into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .exit_on_close_request(false)
    .default_font(ui::FONT)
    .theme(|_: &Fixture| ui::native_theme())
    .scale_factor(|f: &Fixture| f.options.scale)
    .subscription(Fixture::subscription)
    .run();
    drop(owner);
    result
}
