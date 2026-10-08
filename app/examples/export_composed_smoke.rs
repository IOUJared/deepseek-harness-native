//! Fixed PUBLIC actual-App -> worker -> isolated-alpha export qualification.
//! Manual widget/business messages are dropped. No prompt, catalog, credentials or plugin writes.
//! Own-renderer captures and scripted UI messages are not physical keyboard/pointer proof.
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
    // Lexical fixture-only metadata. No production accessor or bootstrap/credential exposure.
    pub(super) struct FixtureMetadata {
        pub ready: bool,
        pub root_ready: bool,
        pub roster_ready: bool,
        pub registry_ready: bool,
        pub follow_ready: bool,
        pub allowed: bool,
        pub selected: Option<SessionId>,
        pub sessions: Vec<SessionId>,
        pub members: Vec<SessionId>,
        pub view: Option<crate::management::View>,
        pub management_ticket: Option<crate::management::Ticket>,
        pub export_ticket: Option<crate::exporter::Ticket>,
        pub review: Option<crate::exporter::Review>,
        pub pending: Option<crate::exporter::Receipt>,
        pub notice: crate::exporter::Notice,
        pub path_empty: bool,
        pub close_confirmation: bool,
    }
    pub(super) fn fixture_metadata(app: &App) -> FixtureMetadata {
        let context = app.export_context();
        FixtureMetadata {
            ready: app.ready,
            root_ready: app.root_ready,
            roster_ready: !app.roster_pending,
            registry_ready: app.management.registry.ready(),
            follow_ready: app.follow_ready,
            allowed: context.allowed,
            selected: app.selected.clone(),
            sessions: app.sessions.keys().cloned().collect(),
            members: app
                .workspace
                .as_ref()
                .and_then(|id| app.workspaces.iter().find(|w| &w.workspace_id == id))
                .map(|w| w.session_ids.clone())
                .unwrap_or_default(),
            view: context.view,
            management_ticket: app.management.ticket(),
            export_ticket: app.exporter.ticket(),
            review: app.exporter.review().cloned(),
            pending: app.exporter.pending().cloned(),
            notice: app.exporter.notice(),
            path_empty: app.exporter.path().is_empty(),
            close_confirmation: matches!(app.close, Close::Confirm(_)),
        }
    }
}
use dsh_native_transport::dto::SessionId;
use iced::widget::{column, container};
use iced::{Element, Length, Subscription, Task, window};
use serde_json::{Value, json};
use std::{
    fs,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
const MAX_PUBLIC_ARCHIVE: usize = 4 * 1024 * 1024;
const ARCHIVE_NAME: &str = "PUBLIC-session-export.zip";
#[derive(Clone)]
struct Options {
    output: PathBuf,
    app: config::Options,
}
fn private_empty(path: &Path, boundary: &Path) -> Result<(), &'static str> {
    let meta = fs::symlink_metadata(path).map_err(|_| "private-directory-required")?;
    let canonical = path
        .canonicalize()
        .map_err(|_| "private-directory-required")?;
    if !path.is_absolute()
        || !meta.is_dir()
        || meta.file_type().is_symlink()
        || meta.permissions().mode() & 0o777 != 0o700
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
fn options() -> Result<Options, &'static str> {
    let mut args = std::env::args().skip(1);
    let mut output = None;
    let mut scale = None;
    while let Some(key) = args.next() {
        let value = args.next().ok_or("missing-value")?;
        match key.as_str() {
            "--output" if output.is_none() => output = Some(PathBuf::from(value)),
            "--scale" if scale.is_none() => {
                scale = Some(value.parse::<f32>().map_err(|_| "invalid-scale")?)
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
        || meta.permissions().mode() & 0o777 != 0o700
        || !canonical.starts_with(&boundary)
        || canonical == boundary
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
        let entry = entry.map_err(|_| "output-unavailable")?;
        if !names.iter().any(|name| entry.file_name() == *name) {
            return Err("unexpected-output-content");
        }
    }
    for name in names {
        private_empty(&canonical.join(name), &canonical)?;
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
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shot {
    Review,
    Saved,
}
impl Shot {
    fn filename(self) -> &'static str {
        match self {
            Self::Review => "export-review.png",
            Self::Saved => "export-saved.png",
        }
    }
    fn key(self) -> &'static str {
        match self {
            Self::Review => "exportReviewCapture",
            Self::Saved => "exportSavedCapture",
        }
    }
}
#[derive(Clone)]
struct FileSnapshot {
    bytes: Arc<[u8]>,
    identity: [u64; 4],
    times: [i64; 4],
}
impl FileSnapshot {
    fn unchanged(&self, other: &Self) -> bool {
        self.identity == other.identity && self.times == other.times && self.bytes == other.bytes
    }
}
// PUBLIC fixture bytes only. No decompression, JSONL parsing, hashing, or byte serialization.
fn read_public_archive(path: &Path) -> Option<FileSnapshot> {
    use rustix::fs::{Mode, OFlags, open};
    let fd = open(
        path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .ok()?;
    let file = fs::File::from(fd);
    let before = file.metadata().ok()?;
    if !before.is_file()
        || before.len() == 0
        || before.len() > MAX_PUBLIC_ARCHIVE as u64
        || before.permissions().mode() & 0o7777 != 0o600
    {
        return None;
    }
    let identity = [
        before.dev(),
        before.ino(),
        before.len(),
        before.permissions().mode() as u64,
    ];
    let times = [
        before.mtime(),
        before.mtime_nsec(),
        before.ctime(),
        before.ctime_nsec(),
    ];
    let mut bytes = Vec::new();
    (&file)
        .take(MAX_PUBLIC_ARCHIVE as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    let after = file.metadata().ok()?;
    if bytes.len() as u64 != before.len()
        || bytes.len() > MAX_PUBLIC_ARCHIVE
        || [
            after.dev(),
            after.ino(),
            after.len(),
            after.permissions().mode() as u64,
        ] != identity
        || [
            after.mtime(),
            after.mtime_nsec(),
            after.ctime(),
            after.ctime_nsec(),
        ] != times
    {
        return None;
    }
    Some(FileSnapshot {
        bytes: bytes.into(),
        identity,
        times,
    })
}
#[derive(Clone)]
enum Message {
    Ui(ui::Message),
    Capture {
        shot: Shot,
        id: window::Id,
    },
    Screenshot {
        shot: Shot,
        image: window::Screenshot,
    },
    Captured {
        shot: Shot,
        success: bool,
        size: [u32; 2],
    },
    Verified {
        index: usize,
        receipt: exporter::Receipt,
        snapshot: Option<FileSnapshot>,
    },
    Deadline,
}
impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ExportComposedMessage(redacted)")
    }
}
#[derive(Clone)]
enum Stage {
    Ready,
    Workspace,
    Creating,
    Created,
    PaintReview(exporter::Review),
    Export {
        index: usize,
        receipt: exporter::Receipt,
    },
    Verifying {
        index: usize,
        receipt: exporter::Receipt,
        bytes: Option<usize>,
    },
    PaintSaved,
    Stopping,
}
struct Fixture {
    options: Options,
    app: ui::App,
    stage: Stage,
    session: Option<SessionId>,
    first: Option<FileSnapshot>,
    receipts: Vec<exporter::Receipt>,
    own_window: Option<window::Id>,
    native_wayland: bool,
    report: Value,
    failed: bool,
    completed: bool,
}
impl Fixture {
    fn boot(options: Options, handle: worker::Handle, feed: worker::Feed) -> (Self, Task<Message>) {
        let (app, task) = ui::App::boot(options.app.clone(), handle, feed);
        let deadline = Task::perform(
            async {
                tokio::time::sleep(Duration::from_secs(35)).await;
            },
            |_| Message::Deadline,
        );
        let report = json!({
            "scope":"scripted actual native App/worker/Core isolated-alpha root export and exclusive-new-file persistence",
            "pid":std::process::id(), "publicFixtureOnly":true, "scriptedUiMessages":true,
            "nativeKeyboardPointerInputQualified":false, "realCredentialsUsed":false,
            "modelPrompts":0, "modelCatalogRequested":false, "browserSignInRequested":false,
            "apiKeySaveRequested":false, "pluginWritesRequested":false, "settingsWritesRequested":false,
            "manualBusinessMessagesForwarded":false, "sessionDeletionRequested":false,
            "stopActivityRequested":false, "includeDescendants":false, "rootAttachmentsRemainIncluded":true,
            "hostLiveFlushMayWrite":true, "hostMemoryBoundClaimed":false,
            "archiveReadBoundBytes":MAX_PUBLIC_ARCHIVE, "payloadDecompressionValidatedInApp":false,
            "parentIndependentZipCrcJsonlValidationRequired":true,
            "applicationScale":options.app.scale, "parentPaintInspectionRequired":true,
            "zipArtifact":ARCHIVE_NAME, "archiveArtifact":ARCHIVE_NAME, "stages":[]
        });
        (
            Self {
                options,
                app,
                stage: Stage::Ready,
                session: None,
                first: None,
                receipts: vec![],
                own_window: None,
                native_wayland: false,
                report,
                failed: false,
                completed: false,
            },
            Task::batch([task.map(Message::Ui), deadline]),
        )
    }
    fn dispatch(&mut self, message: ui::Message) -> Task<Message> {
        self.app.update(message).map(Message::Ui)
    }
    fn action(&mut self, action: exporter::Action) -> Task<Message> {
        self.dispatch(ui::Message::Export(action))
    }
    fn stop(&mut self, error: Option<&'static str>) -> Task<Message> {
        if let Some(error) = error {
            self.failed = true;
            if self.report.get("errorCode").is_none() {
                self.report["errorCode"] = json!(error);
            }
        }
        if matches!(self.stage, Stage::Stopping) {
            return Task::none();
        }
        self.stage = Stage::Stopping;
        self.report["actualCloseUiDispatched"] = json!(true);
        let close = self.dispatch(ui::Message::Close);
        let confirm = if ui::fixture_metadata(&self.app).close_confirmation {
            self.dispatch(ui::Message::ConfirmClose)
        } else {
            Task::none()
        };
        Task::batch([close, confirm])
    }
    fn capture(&self, shot: Shot) -> Task<Message> {
        let Some(id) = self.own_window else {
            return Task::none();
        };
        Task::perform(
            async {
                tokio::time::sleep(Duration::from_millis(1000)).await;
            },
            move |_| Message::Capture { shot, id },
        )
    }
    fn review_step(&mut self, index: usize) -> Task<Message> {
        let meta = ui::fixture_metadata(&self.app);
        if !meta.allowed
            || !meta.follow_ready
            || meta.pending.is_some()
            || meta.selected != self.session
        {
            return self.stop(Some("actual-export-view-unavailable"));
        }
        let mut tasks = vec![];
        if let Some(ticket) = meta.export_ticket {
            tasks.push(self.action(exporter::Action::Close(ticket)));
        }
        let Some(view) = ui::fixture_metadata(&self.app).view else {
            return self.stop(Some("actual-management-view-unavailable"));
        };
        tasks.push(self.dispatch(ui::Message::Management(management::Action::Open(view))));
        let Some(ticket) = ui::fixture_metadata(&self.app).management_ticket else {
            return self.stop(Some("actual-management-open-refused"));
        };
        tasks.push(self.dispatch(ui::Message::ExportSession(ticket)));
        let Some(ticket) = ui::fixture_metadata(&self.app).export_ticket else {
            return self.stop(Some("actual-export-entry-refused"));
        };
        let path = self.options.output.join(ARCHIVE_NAME);
        let Some(path) = path.to_str() else {
            return self.stop(Some("public-destination-not-utf8"));
        };
        tasks.push(self.action(exporter::Action::Edit {
            ticket: ticket.clone(),
            path: path.to_owned(),
        }));
        tasks.push(self.action(exporter::Action::Review(ticket)));
        let Some(review) = ui::fixture_metadata(&self.app).review else {
            return self.stop(Some("actual-export-review-refused"));
        };
        if self.session.as_ref() != Some(&review.ticket.target) {
            return self.stop(Some("actual-export-review-target-invalid"));
        }
        if index == 0 {
            self.stage = Stage::PaintReview(review);
            tasks.push(self.capture(Shot::Review));
        } else {
            tasks.push(self.confirm_step(index, review));
        }
        Task::batch(tasks)
    }
    fn confirm_step(&mut self, index: usize, review: exporter::Review) -> Task<Message> {
        let meta = ui::fixture_metadata(&self.app);
        if !meta.allowed || meta.review.as_ref() != Some(&review) || meta.pending.is_some() {
            return self.stop(Some("review-changed-before-confirm"));
        }
        let task = self.action(exporter::Action::Confirm(review.clone()));
        let meta = ui::fixture_metadata(&self.app);
        let Some(receipt) = meta.pending else {
            return self.stop(Some("actual-export-confirm-not-admitted"));
        };
        if receipt.ticket != review.ticket
            || receipt.editor != review.editor
            || !meta.path_empty
            || meta.review.is_some()
            || meta.notice != exporter::Notice::Pending
        {
            return self.stop(Some("actual-export-admission-metadata-invalid"));
        }
        if self.receipts.last().is_some_and(|previous| {
            receipt.attempt <= previous.attempt
                || receipt.editor <= previous.editor
                || receipt.ticket.serial <= previous.ticket.serial
                || receipt.ticket.epoch != previous.ticket.epoch
                || receipt.ticket.generation != previous.ticket.generation
                || receipt.ticket.target != previous.ticket.target
        }) {
            return self.stop(Some("second-export-receipt-not-distinct"));
        }
        self.receipts.push(receipt.clone());
        self.stage = Stage::Export { index, receipt };
        task
    }
    fn observe_export(
        &mut self,
        receipt: &exporter::Receipt,
        outcome: exporter::Outcome,
    ) -> Task<Message> {
        let Stage::Export {
            index,
            receipt: expected,
        } = self.stage.clone()
        else {
            return Task::none();
        };
        if receipt != &expected {
            return Task::none();
        }
        let bytes = match (index, outcome) {
            (0, exporter::Outcome::Saved { bytes })
                if (1..=MAX_PUBLIC_ARCHIVE).contains(&bytes) =>
            {
                Some(bytes)
            }
            (1, exporter::Outcome::NotCreated) => None,
            _ => return self.stop(Some("matching-export-outcome-unexpected")),
        };
        let meta = ui::fixture_metadata(&self.app);
        if meta.pending.is_some()
            || meta.notice != exporter::Notice::from(outcome)
            || !meta.path_empty
            || meta.review.is_some()
        {
            return self.stop(Some("matching-export-receipt-not-retired"));
        }
        if index == 0 {
            self.report["matchingSavedReceiptObserved"] = json!(true);
            self.report["rootOnlyExportConfirmed"] = json!(true);
        } else {
            self.report["noOverwriteReceiptObserved"] = json!(true);
        }
        self.report["stages"].as_array_mut().unwrap().push(json!({
            "name":if index==0 {"save-new-public-archive"} else {"same-path-no-overwrite"},
            "receipt":{"epoch":receipt.ticket.epoch,"generation":receipt.ticket.generation,"serial":receipt.ticket.serial,"editor":receipt.editor,"attempt":receipt.attempt},
            "matchingExportedReceipt":true,"controllerPendingRetired":true,"enteredPathCleared":true,
            "saved":index==0,"notCreated":index==1,"savedBytes":bytes
        }));
        self.stage = Stage::Verifying {
            index,
            receipt: expected.clone(),
            bytes,
        };
        let path = self.options.output.join(ARCHIVE_NAME);
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || read_public_archive(&path))
                    .await
                    .ok()
                    .flatten()
            },
            move |snapshot| Message::Verified {
                index,
                receipt: expected,
                snapshot,
            },
        )
    }
    fn advance(&mut self) -> Task<Message> {
        let meta = ui::fixture_metadata(&self.app);
        match self.stage.clone() {
            Stage::Ready
                if meta.ready
                    && meta.root_ready
                    && meta.roster_ready
                    && meta.registry_ready
                    && self.own_window.is_some() =>
            {
                if !meta.sessions.is_empty() {
                    return self.stop(Some("fresh-empty-session-baseline-required"));
                }
                self.report["realWorkerReady"] = json!(true);
                self.report["freshEmptyBaselineObserved"] = json!(true);
                self.stage = Stage::Workspace;
                self.dispatch(ui::Message::OpenWorkspace)
            }
            Stage::Created
                if self.session.is_some()
                    && meta.sessions.len() == 1
                    && meta.members == meta.sessions
                    && meta.selected == self.session
                    && meta.follow_ready
                    && meta.allowed
                    && meta
                        .view
                        .as_ref()
                        .is_some_and(|view| view.epoch > 0 && view.generation > 0) =>
            {
                self.report["oneOrdinaryPublicSessionCreated"] = json!(true);
                self.report["selectedFollowBaselineObserved"] = json!(true);
                self.report["selectedBaselineObserved"] = json!(true);
                self.report["ownLookupSessionPublicSourceValidated"] = json!(true);
                self.report["ownedWorkspaceMembershipObserved"] = json!(true);
                self.report["targetCount"] = json!(1);
                self.review_step(0)
            }
            _ => Task::none(),
        }
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Ui(ui::Message::NativeOpened(id)) => {
                self.own_window = Some(id);
                let task = self.dispatch(ui::Message::NativeOpened(id));
                let next = self.advance();
                Task::batch([task, next])
            }
            Message::Ui(message @ ui::Message::NativeInfo { .. }) => {
                if let ui::Message::NativeInfo { backend, .. } = &message {
                    self.native_wayland = *backend == "wayland";
                    self.report["windowBackend"] = json!(backend);
                }
                self.dispatch(message)
            }
            Message::Ui(
                message @ (ui::Message::Resized(_)
                | ui::Message::WindowActive(_)
                | ui::Message::MotionFrame(_)),
            ) => self.dispatch(message),
            Message::Ui(ui::Message::Close) => {
                self.stop(Some("manual-close-before-fixture-complete"))
            }
            Message::Ui(ui::Message::Worker(event)) => {
                let task = self.dispatch(ui::Message::Worker(event.clone()));
                match &event {
                    worker::Event::WorkspaceOpened(Ok(value))
                        if matches!(self.stage, Stage::Workspace) =>
                    {
                        if !value.created
                            || value.workspace.path
                                != self.options.app.backend.working_directory.to_string_lossy()
                        {
                            return self.stop(Some("fresh-owned-workspace-required"));
                        }
                        self.report["ownedWorkspaceCreated"] = json!(true);
                        self.stage = Stage::Creating;
                        let create = self.dispatch(ui::Message::NewSession);
                        return Task::batch([task, create]);
                    }
                    worker::Event::WorkspaceOpened(Err(_)) => {
                        return self.stop(Some("workspace-create-failed"));
                    }
                    worker::Event::Created(Ok(value)) => {
                        if !matches!(self.stage, Stage::Creating) || self.session.is_some() {
                            return self.stop(Some("unexpected-session-create-ack"));
                        }
                        self.session = Some(value.session_id.clone());
                        self.stage = Stage::Created;
                    }
                    worker::Event::Created(Err(_)) => {
                        return self.stop(Some("session-create-failed"));
                    }
                    worker::Event::Exported { receipt, outcome } => {
                        let next = self.observe_export(receipt, *outcome);
                        return Task::batch([task, next]);
                    }
                    worker::Event::Inspection(_) if matches!(self.stage, Stage::Stopping) => {
                        self.report["actualCloseInspectionObserved"] = json!(true);
                        if ui::fixture_metadata(&self.app).close_confirmation {
                            let confirm = self.dispatch(ui::Message::ConfirmClose);
                            return Task::batch([task, confirm]);
                        }
                    }
                    worker::Event::Catalog(_)
                    | worker::Event::Model { .. }
                    | worker::Event::Prompt { .. }
                    | worker::Event::Cancel { .. }
                    | worker::Event::KeySaved { .. }
                    | worker::Event::PluginSaved { .. } => {
                        return self.stop(Some("forbidden-business-result-observed"));
                    }
                    worker::Event::Fault(_) => return self.stop(Some("fixed-worker-failure")),
                    worker::Event::NoBackendStarted => {
                        self.failed = true;
                        self.finish(false);
                        return iced::exit();
                    }
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
                        self.report["gracefulStopped"] = json!(clean);
                        let passed = clean
                            && !self.failed
                            && self.native_wayland
                            && self.report["samePathBytesAndMetadataUnchanged"] == true
                            && self.report["stages"]
                                .as_array()
                                .is_some_and(|stages| stages.len() == 2)
                            && self.report[Shot::Review.key()]["saved"] == true
                            && self.report[Shot::Saved.key()]["saved"] == true;
                        self.finish(passed);
                        return Task::batch([task, iced::exit()]);
                    }
                    _ => {}
                }
                let next = self.advance();
                Task::batch([task, next])
            }
            Message::Capture { shot, id }
                if self.own_window == Some(id) && self.capture_stage(shot) =>
            {
                window::screenshot(id).map(move |image| Message::Screenshot { shot, image })
            }
            Message::Screenshot { shot, image } if self.capture_stage(shot) => {
                let path = self.options.output.join(shot.filename());
                let size = [image.size.width, image.size.height];
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            smoke::save_screenshot(&path, &image).is_ok()
                        })
                        .await
                        .unwrap_or(false)
                    },
                    move |success| Message::Captured {
                        shot,
                        success,
                        size,
                    },
                )
            }
            Message::Captured {
                shot,
                success,
                size,
            } if self.capture_stage(shot) => {
                self.report[shot.key()] = json!({"saved":success,"physicalSize":size,"source":"own actual-App Iced renderer","manualInputProof":false,"artifact":shot.filename()});
                if !success {
                    return self.stop(Some("own-renderer-capture-failed"));
                }
                match (shot, self.stage.clone()) {
                    (Shot::Review, Stage::PaintReview(review)) => self.confirm_step(0, review),
                    (Shot::Saved, Stage::PaintSaved) => self.review_step(1),
                    _ => Task::none(),
                }
            }
            Message::Verified {
                index,
                receipt,
                snapshot,
            } => {
                let Stage::Verifying {
                    index: expected_index,
                    receipt: expected,
                    bytes,
                } = self.stage.clone()
                else {
                    return Task::none();
                };
                if index != expected_index || receipt != expected {
                    return Task::none();
                }
                let Some(snapshot) = snapshot else {
                    return self.stop(Some("bounded-public-archive-read-failed"));
                };
                if index == 0 {
                    if bytes != Some(snapshot.bytes.len()) {
                        return self.stop(Some("saved-byte-count-disagrees"));
                    }
                    self.report["privateArchiveFileVerified"] = json!({"mode0600":true,"bytes":snapshot.bytes.len(),"regularFile":true,"withinFixtureReadBound":true});
                    self.report["privateZipVerified"] = json!(true);
                    self.first = Some(snapshot);
                    self.stage = Stage::PaintSaved;
                    self.capture(Shot::Saved)
                } else {
                    if self
                        .first
                        .as_ref()
                        .is_none_or(|first| !first.unchanged(&snapshot))
                    {
                        return self.stop(Some("same-path-file-changed"));
                    }
                    self.report["samePathBytesAndMetadataUnchanged"] = json!(true);
                    self.report["exactBytesUnchanged"] = json!(true);
                    self.report["twoExactReceiptsObserved"] = json!(true);
                    self.stop(None)
                }
            }
            Message::Deadline if !self.completed => {
                self.stop(Some("export-composed-fixture-deadline"))
            }
            _ => Task::none(), // Every manual widget/business message is dropped, including export edits/confirms.
        }
    }
    fn capture_stage(&self, shot: Shot) -> bool {
        matches!(
            (shot, &self.stage),
            (Shot::Review, Stage::PaintReview(_)) | (Shot::Saved, Stage::PaintSaved)
        )
    }
    fn finish(&mut self, passed: bool) {
        if self.completed {
            return;
        }
        self.completed = true;
        self.report["status"] = json!(if passed { "passed" } else { "failed" });
        if let Ok(file) = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(self.options.output.join("app.json"))
        {
            let _ = serde_json::to_writer_pretty(&file, &self.report);
            let _ = file.sync_all();
        }
    }
    fn view(&self) -> Element<'_, Message> {
        container(column![ui::label("PUBLIC EXPORT COMPOSED FIXTURE · scripted actions only · manual actions blocked · no model/key/plugin writes", ui::DANGER), self.app.view().map(Message::Ui)].spacing(12))
            .padding(16).width(Length::Fill).height(Length::Fill).into()
    }
    fn subscription(&self) -> Subscription<Message> {
        self.app.subscription().map(Message::Ui)
    }
}
fn main() -> iced::Result {
    let options = options().unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2)
    });
    let report = options.output.join("app.json");
    // SAFETY: select renderer before Iced/Tokio/owned threads exist.
    unsafe {
        std::env::set_var("ICED_BACKEND", "wgpu");
    }
    let (handle, feed, owner) =
        worker::start(options.app.backend.clone(), false).unwrap_or_else(|_| {
            eprintln!("Owned export fixture worker unavailable");
            std::process::exit(1)
        });
    let result = iced::application(
        move || Fixture::boot(options.clone(), handle.clone(), feed.clone()),
        Fixture::update,
        Fixture::view,
    )
    .title("Harness native composed export · PUBLIC fixture")
    .window(window::Settings {
        size: iced::Size::new(1200.0, 800.0),
        min_size: Some(iced::Size::new(900.0, 600.0)),
        platform_specific: window::settings::PlatformSpecific {
            application_id: "ai.deepseek.harness.native.export-composed-fixture".into(),
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
    .scale_factor(|f: &Fixture| f.options.app.scale)
    .subscription(Fixture::subscription)
    .run();
    drop(owner);
    let passed = fs::File::open(report)
        .ok()
        .and_then(|file| {
            let mut bytes = Vec::new();
            file.take(65_537)
                .read_to_end(&mut bytes)
                .ok()
                .map(|_| bytes)
        })
        .filter(|bytes| bytes.len() <= 65_536)
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .is_some_and(|value| value["status"] == "passed");
    if !passed {
        eprintln!("Composed public export fixture failed or report unavailable");
        std::process::exit(1);
    }
    result
}

#[cfg(test)]
mod composed_tests {
    use super::*;
    fn fixture() -> (Fixture, tokio::sync::mpsc::Receiver<worker::Command>) {
        let (handle, feed, commands, _events, _closing) = worker::test_channels();
        let options = Options {
            output: PathBuf::from("/NONWRITTEN-EXPORT-FIXTURE"),
            app: config::Options {
                backend: dsh_native_core::RustBackendOptions {
                    runtime: PathBuf::from("/EXPLICIT-NONSTARTED-FIXTURE/apps/cli"),
                    expected_version: "0.2.1-alpha.1".into(),
                    native_home: PathBuf::from("/EXPLICIT-NONSTARTED-FIXTURE/harness"),
                    working_directory: PathBuf::from("/tmp"),
                    user_home: PathBuf::from("/EXPLICIT-NONSTARTED-FIXTURE/user"),
                    absolute_node_path: None,
                    startup_timeout: Duration::from_secs(15),
                    stop_policy: Default::default(),
                },
                scale: 1.0,
                smoke: None,
            },
        };
        (Fixture::boot(options, handle, feed).0, commands)
    }
    fn receipt() -> exporter::Receipt {
        exporter::Receipt {
            ticket: management::Ticket {
                epoch: 1,
                generation: 3,
                serial: 5,
                target: SessionId::new("PUBLIC-session").unwrap(),
            },
            editor: 7,
            attempt: 11,
        }
    }
    #[test]
    fn every_manual_export_and_business_message_is_dropped() {
        let (mut fixture, mut commands) = fixture();
        let receipt = receipt();
        for message in [
            ui::Message::Send,
            ui::Message::Stop,
            ui::Message::NewSession,
            ui::Message::LoadModels,
            ui::Message::WorkspacePath("/PUBLIC_MANUAL_ACTION".into()),
            ui::Message::OpenWorkspace,
            ui::Message::Settings(settings::Action::Open),
            ui::Message::Management(management::Action::Open(management::View {
                epoch: 1,
                generation: 3,
                target: receipt.ticket.target.clone(),
            })),
            ui::Message::ExportSession(receipt.ticket.clone()),
            ui::Message::Export(exporter::Action::Open(management::View {
                epoch: 1,
                generation: 3,
                target: receipt.ticket.target.clone(),
            })),
            ui::Message::Export(exporter::Action::Edit {
                ticket: receipt.ticket.clone(),
                path: "/PUBLIC/manual.zip".into(),
            }),
            ui::Message::Export(exporter::Action::Review(receipt.ticket.clone())),
            ui::Message::Export(exporter::Action::Confirm(exporter::Review {
                ticket: receipt.ticket,
                editor: receipt.editor,
            })),
        ] {
            let _ = fixture.update(Message::Ui(message));
        }
        assert!(commands.try_recv().is_err());
        let meta = ui::fixture_metadata(&fixture.app);
        assert!(
            meta.export_ticket.is_none()
                && meta.management_ticket.is_none()
                && meta.pending.is_none()
        );
        assert!(meta.path_empty);
    }
    #[test]
    fn mismatched_export_receipt_never_advances_or_reports_a_save() {
        let (mut fixture, _) = fixture();
        let expected = receipt();
        fixture.stage = Stage::Export {
            index: 0,
            receipt: expected.clone(),
        };
        for field in 0..6 {
            let mut stale = expected.clone();
            match field {
                0 => stale.ticket.epoch += 1,
                1 => stale.ticket.generation += 1,
                2 => stale.ticket.serial += 1,
                3 => stale.editor += 1,
                4 => stale.attempt += 1,
                _ => stale.ticket.target = SessionId::new("PUBLIC-other").unwrap(),
            }
            let _ = fixture.observe_export(&stale, exporter::Outcome::Saved { bytes: 123 });
            assert!(matches!(&fixture.stage, Stage::Export { receipt,.. } if receipt==&expected));
            assert!(fixture.report["stages"].as_array().unwrap().is_empty());
            assert!(!fixture.failed);
        }
    }
    #[test]
    fn snapshots_compare_exact_public_bytes_and_inode_times_not_length_only() {
        let before = FileSnapshot {
            bytes: Arc::from(&b"PUBLIC"[..]),
            identity: [1, 2, 6, 0o100600],
            times: [3, 4, 5, 6],
        };
        assert!(before.unchanged(&before.clone()));
        let mut wrong = before.clone();
        wrong.bytes = Arc::from(&b"PUBLIQ"[..]);
        assert!(!before.unchanged(&wrong));
        let mut wrong = before.clone();
        wrong.identity[1] += 1;
        assert!(!before.unchanged(&wrong));
        let mut wrong = before.clone();
        wrong.times[0] += 1;
        assert!(!before.unchanged(&wrong));
    }
    #[test]
    fn capture_requests_only_match_the_actual_fixed_stage() {
        let (mut fixture, _) = fixture();
        assert!(!fixture.capture_stage(Shot::Review) && !fixture.capture_stage(Shot::Saved));
        let receipt = receipt();
        fixture.stage = Stage::PaintReview(exporter::Review {
            ticket: receipt.ticket,
            editor: receipt.editor,
        });
        assert!(fixture.capture_stage(Shot::Review) && !fixture.capture_stage(Shot::Saved));
        fixture.stage = Stage::PaintSaved;
        assert!(fixture.capture_stage(Shot::Saved) && !fixture.capture_stage(Shot::Review));
    }
}
