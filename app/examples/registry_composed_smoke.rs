//! Fixed PUBLIC actual-App -> worker -> isolated-alpha registry qualification.
//! Manual widget/business messages are dropped. No model, credentials, forced stop or deletion.
//! Scripted integration and own-renderer captures are NOT genuine keyboard/pointer proof.
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
// Test-only lexical metadata adapter. No production getter, secrets, status bodies or shortcuts.
mod ui {
    include!("../src/ui.rs");
    pub(super) struct FixtureMetadata {
        pub ready: bool,
        pub root_ready: bool,
        pub roster_ready: bool,
        pub registry_ready: bool,
        pub follow_ready: bool,
        pub allowed: bool,
        pub selected: Option<SessionId>,
        pub workspace: Option<WorkspaceId>,
        pub workspace_members: Vec<SessionId>,
        pub sessions: Vec<SessionId>,
        pub navigation: Vec<SessionId>,
        pub archived_view: bool,
        pub target_archived: bool,
        pub pins: Vec<SessionId>,
        pub view: Option<crate::management::View>,
        pub ticket: Option<crate::management::Ticket>,
        pub review: Option<crate::management::Submission>,
        pub pending: bool,
        pub notice: crate::management::Notice,
    }
    pub(super) fn fixture_metadata(app: &App, target: Option<&SessionId>) -> FixtureMetadata {
        let context = app.management_context();
        FixtureMetadata {
            ready: app.ready,
            root_ready: app.root_ready,
            roster_ready: !app.roster_pending,
            registry_ready: app.management.registry.ready(),
            follow_ready: app.follow_ready,
            allowed: context.allowed,
            selected: app.selected.clone(),
            workspace: app.workspace.clone(),
            workspace_members: app
                .workspace
                .as_ref()
                .and_then(|id| app.workspaces.iter().find(|w| &w.workspace_id == id))
                .map(|w| w.session_ids.clone())
                .unwrap_or_default(),
            sessions: app.sessions.keys().cloned().collect(),
            navigation: app
                .navigation_sessions()
                .into_iter()
                .map(|s| s.session_id.clone())
                .collect(),
            archived_view: app.show_archived,
            target_archived: target.is_some_and(|id| app.management.registry.archived(id)),
            pins: app.management.registry.ordered_pins().to_vec(),
            view: context.view,
            ticket: app.management.ticket(),
            review: app.management.review().cloned(),
            pending: app.management.pending(),
            notice: app.management.notice(),
        }
    }
}
use dsh_native_transport::dto::{SessionId, WorkspaceFollowFrame};
use iced::widget::{column, container};
use iced::{Element, Length, Subscription, Task, window};
use serde_json::{Value, json};
use std::{
    fs,
    io::Read,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::Duration,
};
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
        if !names.iter().any(|n| entry.file_name() == *n) {
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
    let values = vec![
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
    ];
    let app = config::parse(values)
        .map_err(|_| "fixture-options-invalid")?
        .ok_or("fixture-options-required")?;
    Ok(Options {
        output: canonical,
        app,
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shot {
    Archive,
    Restore,
}
impl Shot {
    fn filename(self) -> &'static str {
        match self {
            Self::Archive => "archive-review.png",
            Self::Restore => "restore-review.png",
        }
    }
    fn key(self) -> &'static str {
        match self {
            Self::Archive => "archiveReviewCapture",
            Self::Restore => "archivedRestoreReviewCapture",
        }
    }
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
    Saved {
        shot: Shot,
        success: bool,
        size: [u32; 2],
    },
    Deadline,
}
impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RegistryComposedMessage(redacted)")
    }
}
#[derive(Clone, Copy)]
struct Step {
    label: &'static str,
    operation: management::Operation,
    target: usize,
    pins: &'static [usize],
    archived: bool,
    pin_frame: bool,
    archive_frame: bool,
    shot: Option<Shot>,
}
const STEPS: [Step; 6] = [
    Step {
        label: "pin-first",
        operation: management::Operation::Pin,
        target: 0,
        pins: &[0],
        archived: false,
        pin_frame: true,
        archive_frame: false,
        shot: None,
    },
    Step {
        label: "pin-second",
        operation: management::Operation::Pin,
        target: 1,
        pins: &[1, 0],
        archived: false,
        pin_frame: true,
        archive_frame: false,
        shot: None,
    },
    Step {
        label: "unpin-first",
        operation: management::Operation::Unpin,
        target: 0,
        pins: &[1],
        archived: false,
        pin_frame: true,
        archive_frame: false,
        shot: None,
    },
    Step {
        label: "repin-first",
        operation: management::Operation::Pin,
        target: 0,
        pins: &[0, 1],
        archived: false,
        pin_frame: true,
        archive_frame: false,
        shot: None,
    },
    Step {
        label: "archive-first",
        operation: management::Operation::Archive,
        target: 0,
        pins: &[1],
        archived: true,
        pin_frame: true,
        archive_frame: true,
        shot: Some(Shot::Archive),
    },
    Step {
        label: "restore-first",
        operation: management::Operation::Restore,
        target: 0,
        pins: &[1],
        archived: false,
        pin_frame: false,
        archive_frame: true,
        shot: Some(Shot::Restore),
    },
];
#[derive(Clone)]
struct Pending {
    index: usize,
    submission: management::Submission,
    ack: bool,
    pin_seen: bool,
    archive_seen: bool,
}
#[derive(Clone)]
enum Stage {
    Ready,
    Workspace,
    Creating(usize),
    Created(usize),
    Selecting(usize),
    Paint {
        index: usize,
        submission: management::Submission,
        shot: Shot,
    },
    Mutation(Pending),
    Stopping,
}
struct Fixture {
    options: Options,
    app: ui::App,
    stage: Stage,
    sessions: Vec<SessionId>,
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
        let report = json!({"scope":"scripted actual native App/worker/Core isolated-alpha reversible registry mutations","pid":std::process::id(),"publicFixtureOnly":true,"scriptedUiMessages":true,"nativeKeyboardPointerInputQualified":false,"liveHostDecisionQualified":false,"realCredentialsUsed":false,"modelPrompts":0,"modelCatalogRequested":false,"browserSignInRequested":false,"apiKeySaveRequested":false,"stopActivityRequested":false,"sessionDeletionRequested":false,"forkOrRenameRequested":false,"manualBusinessMessagesForwarded":false,"rpcSetsInstalled":false,"filesRetainedNotDeletionRollback":true,"applicationScale":options.app.scale,"parentPaintInspectionRequired":true,"stages":[]});
        (
            Self {
                options,
                app,
                stage: Stage::Ready,
                sessions: vec![],
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
    fn action(&mut self, action: management::Action) -> Task<Message> {
        self.dispatch(ui::Message::Management(action))
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
        self.dispatch(ui::Message::ConfirmClose)
    }
    fn expected_pins(&self, index: usize) -> Vec<SessionId> {
        STEPS[index]
            .pins
            .iter()
            .map(|i| self.sessions[*i].clone())
            .collect()
    }
    fn start_step(&mut self, index: usize) -> Task<Message> {
        let mut tasks = vec![];
        let meta = ui::fixture_metadata(&self.app, None);
        if let Some(ticket) = meta.ticket {
            tasks.push(self.action(management::Action::Close(ticket)));
        }
        if index == 5 {
            if meta.archived_view {
                return self.stop(Some("unexpected-archived-navigation-state"));
            }
            tasks.push(self.dispatch(ui::Message::ArchivedView));
            let meta = ui::fixture_metadata(&self.app, Some(&self.sessions[0]));
            if !meta.archived_view || meta.navigation != vec![self.sessions[0].clone()] {
                return self.stop(Some("actual-archived-navigation-not-first-session"));
            }
            self.report["actualArchivedViewEntered"] = json!(true);
        }
        let target = self.sessions[STEPS[index].target].clone();
        self.stage = Stage::Selecting(index);
        tasks.push(self.dispatch(ui::Message::Select(target)));
        Task::batch(tasks)
    }
    fn review_step(&mut self, index: usize) -> Task<Message> {
        let target = self.sessions[STEPS[index].target].clone();
        let meta = ui::fixture_metadata(&self.app, Some(&target));
        let Some(view) = meta.view else {
            return self.stop(Some("actual-management-view-unavailable"));
        };
        let open = self.action(management::Action::Open(view));
        let Some(ticket) = ui::fixture_metadata(&self.app, Some(&target)).ticket else {
            return self.stop(Some("actual-management-open-refused"));
        };
        let review = self.action(management::Action::Review {
            ticket,
            operation: STEPS[index].operation,
        });
        let Some(submission) = ui::fixture_metadata(&self.app, Some(&target)).review else {
            return self.stop(Some("actual-management-review-refused"));
        };
        if submission.operation != STEPS[index].operation || submission.ticket.target != target {
            return self.stop(Some("actual-review-identity-invalid"));
        }
        if let Some(shot) = STEPS[index].shot {
            let Some(id) = self.own_window else {
                return self.stop(Some("own-window-unavailable"));
            };
            self.stage = Stage::Paint {
                index,
                submission,
                shot,
            };
            let capture = Task::perform(
                async {
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                },
                move |_| Message::Capture { shot, id },
            );
            Task::batch([open, review, capture])
        } else {
            let confirm = self.confirm_step(index, submission);
            Task::batch([open, review, confirm])
        }
    }
    fn confirm_step(&mut self, index: usize, submission: management::Submission) -> Task<Message> {
        let meta = ui::fixture_metadata(&self.app, Some(&submission.ticket.target));
        if meta.review.as_ref() != Some(&submission) || !meta.allowed {
            return self.stop(Some("review-changed-before-confirm"));
        }
        self.stage = Stage::Mutation(Pending {
            index,
            submission: submission.clone(),
            ack: false,
            pin_seen: false,
            archive_seen: false,
        });
        let task = self.action(management::Action::Confirm(submission.ticket));
        if !ui::fixture_metadata(&self.app, None).pending {
            return self.stop(Some("actual-management-submit-not-admitted"));
        }
        task
    }
    fn observe_mutation(&mut self, event: &worker::Event) -> Option<&'static str> {
        let Stage::Mutation(pending) = self.stage.clone() else {
            return None;
        };
        let expected_pins = self.expected_pins(pending.index);
        let expected_archives = if STEPS[pending.index].archived {
            vec![self.sessions[0].clone()]
        } else {
            vec![]
        };
        let Stage::Mutation(active) = &mut self.stage else {
            return None;
        };
        match event {
            worker::Event::Managed { submission, result } if submission == &active.submission => {
                if result.is_err() {
                    return Some("matching-mutation-error-outcome-may-be-indeterminate");
                }
                active.ack = true;
            }
            worker::Event::Workspace(WorkspaceFollowFrame::Pinned { pinned_session_ids })
                if *pinned_session_ids == expected_pins =>
            {
                active.pin_seen = true
            }
            worker::Event::Workspace(WorkspaceFollowFrame::Archived {
                archived_session_ids,
            }) if *archived_session_ids == expected_archives => active.archive_seen = true,
            _ => {}
        }
        None
    }
    fn advance(&mut self) -> Task<Message> {
        let meta = ui::fixture_metadata(&self.app, self.sessions.first());
        match self.stage.clone() {
            Stage::Ready
                if meta.ready
                    && meta.root_ready
                    && meta.roster_ready
                    && meta.registry_ready
                    && self.own_window.is_some() =>
            {
                if !meta.sessions.is_empty() || !meta.pins.is_empty() {
                    return self.stop(Some("fresh-empty-registry-required"));
                }
                self.report["realWorkerReady"] = json!(true);
                self.report["freshEmptyBaselineObserved"] = json!(true);
                self.stage = Stage::Workspace;
                self.dispatch(ui::Message::OpenWorkspace)
            }
            Stage::Created(index)
                if self.sessions.len() == index + 1
                    && meta.sessions.contains(&self.sessions[index])
                    && meta.workspace_members.contains(&self.sessions[index])
                    && meta.selected.as_ref() == Some(&self.sessions[index])
                    && meta.follow_ready =>
            {
                if index == 0 {
                    self.stage = Stage::Creating(1);
                    self.dispatch(ui::Message::NewSession)
                } else {
                    if meta.sessions.len() != 2 || meta.workspace_members.len() != 2 {
                        return self.stop(Some("exactly-two-owned-public-sessions-required"));
                    }
                    self.report["twoDistinctPublicSessionsCreated"] = json!(true);
                    self.report["ownedWorkspaceFeedMembershipObserved"] = json!(true);
                    self.report["publicSessionIds"] = json!(self.sessions);
                    self.start_step(0)
                }
            }
            Stage::Selecting(index)
                if meta.allowed
                    && meta.follow_ready
                    && meta.selected.as_ref() == Some(&self.sessions[STEPS[index].target]) =>
            {
                self.review_step(index)
            }
            Stage::Mutation(pending) => {
                let step = STEPS[pending.index];
                if !pending.ack
                    || (step.pin_frame && !pending.pin_seen)
                    || (step.archive_frame && !pending.archive_seen)
                {
                    return Task::none();
                }
                if !meta.registry_ready
                    || meta.pending
                    || meta.notice != management::Notice::Confirmed
                    || meta.pins != self.expected_pins(pending.index)
                    || meta.target_archived != step.archived
                {
                    return self.stop(Some("ack-and-authoritative-registry-state-disagree"));
                }
                let expected_navigation = if pending.index == 4 {
                    vec![self.sessions[1].clone()]
                } else if pending.index == 5 {
                    vec![]
                } else if pending.index == 0 {
                    vec![self.sessions[0].clone(), self.sessions[1].clone()]
                } else {
                    self.expected_pins(pending.index)
                        .into_iter()
                        .chain((pending.index == 2).then(|| self.sessions[0].clone()))
                        .collect()
                };
                if meta.navigation != expected_navigation {
                    return self.stop(Some("actual-navigation-order-or-filter-invalid"));
                }
                if !self
                    .sessions
                    .iter()
                    .all(|id| meta.sessions.contains(id) && meta.workspace_members.contains(id))
                {
                    return self.stop(Some("archive-or-restore-lost-session-accounting"));
                }
                self.report["stages"].as_array_mut().unwrap().push(json!({"name":step.label,"operation":step.operation.label(),"targetSlot":step.target+1,"ticket":{"epoch":pending.submission.ticket.epoch,"generation":pending.submission.ticket.generation,"serial":pending.submission.ticket.serial},"matchingSubmissionAcknowledged":true,"pinIncrementObserved":pending.pin_seen,"archiveIncrementObserved":pending.archive_seen,"requiredFeedIncrementsObserved":true,"authoritativeRegistryVerified":true,"pinOrderSlots":step.pins.iter().map(|slot|slot+1).collect::<Vec<_>>(),"actualNavigationVerified":true,"bothSessionsAndAccountingRetained":true}));
                if pending.index < 5 {
                    self.start_step(pending.index + 1)
                } else {
                    let Some(ticket) = meta.ticket else {
                        return self.stop(Some("restore-panel-ticket-unavailable"));
                    };
                    let close = self.action(management::Action::Close(ticket));
                    let recent = self.dispatch(ui::Message::ArchivedView);
                    let meta = ui::fixture_metadata(&self.app, Some(&self.sessions[0]));
                    if meta.archived_view
                        || meta.target_archived
                        || meta.pins != vec![self.sessions[1].clone()]
                        || meta.navigation
                            != vec![self.sessions[1].clone(), self.sessions[0].clone()]
                    {
                        return self.stop(Some("restored-recent-navigation-or-no-repin-invalid"));
                    }
                    self.report["restoredRecentViewVerified"] = json!(true);
                    self.report["restoreDidNotRepin"] = json!(true);
                    self.report["archiveAndUnpinCoupledFeedVerified"] = json!(true);
                    self.report["allMatchingAcksAndIndependentFeedsVerified"] = json!(true);
                    let stop = self.stop(None);
                    Task::batch([close, recent, stop])
                }
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
                        self.stage = Stage::Creating(0);
                        let create = self.dispatch(ui::Message::NewSession);
                        return Task::batch([task, create]);
                    }
                    worker::Event::WorkspaceOpened(Err(_)) => {
                        return self.stop(Some("workspace-create-failed"));
                    }
                    worker::Event::Created(Ok(value)) => {
                        let Stage::Creating(index) = self.stage else {
                            return self.stop(Some("unexpected-create-ack"));
                        };
                        if self.sessions.contains(&value.session_id) || self.sessions.len() != index
                        {
                            return self.stop(Some("two-distinct-session-acks-required"));
                        }
                        self.sessions.push(value.session_id.clone());
                        self.stage = Stage::Created(index);
                    }
                    worker::Event::Created(Err(_)) => {
                        return self.stop(Some("session-create-failed"));
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
                        let passed = clean
                            && !self.failed
                            && self.native_wayland
                            && self.report["allMatchingAcksAndIndependentFeedsVerified"] == true
                            && self.report["stages"]
                                .as_array()
                                .is_some_and(|stages| stages.len() == 6)
                            && self.report[Shot::Archive.key()]["saved"] == true
                            && self.report[Shot::Restore.key()]["saved"] == true;
                        self.finish(passed);
                        return Task::batch([task, iced::exit()]);
                    }
                    _ => {}
                }
                if let Some(error) = self.observe_mutation(&event) {
                    return self.stop(Some(error));
                }
                let next = self.advance();
                Task::batch([task, next])
            }
            Message::Capture { shot, id }
                if matches!(&self.stage,Stage::Paint{shot:expected,..} if *expected==shot)
                    && self.own_window == Some(id) =>
            {
                window::screenshot(id).map(move |image| Message::Screenshot { shot, image })
            }
            Message::Screenshot { shot, image } if matches!(&self.stage,Stage::Paint{shot:expected,..} if *expected==shot) =>
            {
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
                    move |success| Message::Saved {
                        shot,
                        success,
                        size,
                    },
                )
            }
            Message::Saved {
                shot,
                success,
                size,
            } => {
                let Stage::Paint {
                    index,
                    submission,
                    shot: expected,
                } = self.stage.clone()
                else {
                    return Task::none();
                };
                if shot != expected {
                    return Task::none();
                }
                self.report[shot.key()] = json!({"saved":success,"physicalSize":size,"source":"own actual-App Iced renderer","manualInputProof":false});
                if !success {
                    return self.stop(Some("own-renderer-capture-failed"));
                }
                self.confirm_step(index, submission)
            }
            Message::Deadline if !self.completed => {
                self.stop(Some("registry-composed-fixture-deadline"))
            }
            _ => Task::none(), // All manual widget/business messages, including keys/model/actions, are dropped.
        }
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
        container(column![ui::label("PUBLIC REGISTRY COMPOSED FIXTURE · scripted actions only · manual actions blocked · no model/key/forced-stop/deletion",ui::DANGER),self.app.view().map(Message::Ui)].spacing(12)).padding(16).width(Length::Fill).height(Length::Fill).into()
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
    // SAFETY: renderer environment is selected before Iced/Tokio/owned threads start.
    unsafe {
        std::env::set_var("ICED_BACKEND", "wgpu");
    }
    let (handle, feed, owner) =
        worker::start(options.app.backend.clone(), false).unwrap_or_else(|_| {
            eprintln!("Owned registry fixture worker unavailable");
            std::process::exit(1)
        });
    let result = iced::application(
        move || Fixture::boot(options.clone(), handle.clone(), feed.clone()),
        Fixture::update,
        Fixture::view,
    )
    .title("Harness native composed registry · PUBLIC fixture")
    .window(window::Settings {
        size: iced::Size::new(1200.0, 800.0),
        min_size: Some(iced::Size::new(900.0, 600.0)),
        platform_specific: window::settings::PlatformSpecific {
            application_id: "ai.deepseek.harness.native.registry-composed-fixture".into(),
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
        eprintln!("Composed public registry fixture failed or report unavailable");
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
            output: PathBuf::from("/NONWRITTEN-REGISTRY-FIXTURE"),
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
    fn ids() -> Vec<SessionId> {
        vec![
            SessionId::new("session-PUBLIC-FIRST").unwrap(),
            SessionId::new("session-PUBLIC-SECOND").unwrap(),
        ]
    }
    fn pending(index: usize) -> Pending {
        Pending {
            index,
            submission: management::Submission {
                ticket: management::Ticket {
                    epoch: 1,
                    generation: 3,
                    serial: 5,
                    target: ids()[STEPS[index].target].clone(),
                },
                operation: STEPS[index].operation,
            },
            ack: false,
            pin_seen: false,
            archive_seen: false,
        }
    }
    #[test]
    fn all_manual_business_and_widget_messages_are_dropped() {
        let (mut fixture, mut commands) = fixture();
        for message in [
            ui::Message::Send,
            ui::Message::Stop,
            ui::Message::NewSession,
            ui::Message::LoadModels,
            ui::Message::ArchivedView,
            ui::Message::WorkspacePath("/PUBLIC_MANUAL_ACTION".into()),
            ui::Message::Settings(settings::Action::Open),
            ui::Message::Management(management::Action::Open(management::View {
                epoch: 1,
                generation: 3,
                target: ids()[0].clone(),
            })),
        ] {
            let _ = fixture.update(Message::Ui(message));
        }
        assert!(commands.try_recv().is_err());
        assert!(!ui::fixture_metadata(&fixture.app, None).archived_view);
        assert!(ui::fixture_metadata(&fixture.app, None).ticket.is_none());
    }
    #[test]
    fn mismatched_ack_cannot_advance_fixed_script() {
        let (mut fixture, _) = fixture();
        fixture.sessions = ids();
        let expected = pending(0);
        fixture.stage = Stage::Mutation(expected.clone());
        let mut stale = expected.submission.clone();
        stale.ticket.serial += 1;
        assert!(
            fixture
                .observe_mutation(&worker::Event::Managed {
                    submission: stale,
                    result: Ok(())
                })
                .is_none()
        );
        assert!(matches!(&fixture.stage,Stage::Mutation(current) if !current.ack));
        assert!(
            fixture
                .observe_mutation(&worker::Event::Managed {
                    submission: expected.submission,
                    result: Ok(())
                })
                .is_none()
        );
        let _ = fixture.advance();
        assert!(
            matches!(&fixture.stage,Stage::Mutation(current) if current.ack&&!current.pin_seen)
        );
        assert!(fixture.report["stages"].as_array().unwrap().is_empty());
    }
    #[test]
    fn archive_requires_both_independent_archive_and_unpin_frames() {
        let (mut fixture, _) = fixture();
        fixture.sessions = ids();
        fixture.stage = Stage::Mutation(pending(4));
        let archive = worker::Event::Workspace(WorkspaceFollowFrame::Archived {
            archived_session_ids: vec![fixture.sessions[0].clone()],
        });
        assert!(fixture.observe_mutation(&archive).is_none());
        assert!(
            matches!(&fixture.stage,Stage::Mutation(current) if current.archive_seen&&!current.pin_seen)
        );
        let wrong_order = worker::Event::Workspace(WorkspaceFollowFrame::Pinned {
            pinned_session_ids: fixture.sessions.clone(),
        });
        assert!(fixture.observe_mutation(&wrong_order).is_none());
        assert!(matches!(&fixture.stage,Stage::Mutation(current) if !current.pin_seen));
        let pins = worker::Event::Workspace(WorkspaceFollowFrame::Pinned {
            pinned_session_ids: vec![fixture.sessions[1].clone()],
        });
        assert!(fixture.observe_mutation(&pins).is_none());
        assert!(
            matches!(&fixture.stage,Stage::Mutation(current) if current.archive_seen&&current.pin_seen&&!current.ack)
        );
    }
    #[test]
    fn host_pin_order_is_checked_not_membership_only() {
        let (mut fixture, _) = fixture();
        fixture.sessions = ids();
        fixture.stage = Stage::Mutation(pending(1));
        let wrong = worker::Event::Workspace(WorkspaceFollowFrame::Pinned {
            pinned_session_ids: fixture.sessions.clone(),
        });
        fixture.observe_mutation(&wrong);
        assert!(matches!(&fixture.stage,Stage::Mutation(current) if !current.pin_seen));
        let right = worker::Event::Workspace(WorkspaceFollowFrame::Pinned {
            pinned_session_ids: vec![fixture.sessions[1].clone(), fixture.sessions[0].clone()],
        });
        fixture.observe_mutation(&right);
        assert!(matches!(&fixture.stage,Stage::Mutation(current) if current.pin_seen));
    }
    #[test]
    fn stage_plan_has_no_forced_activity_stop_and_restore_never_repins() {
        assert_eq!(STEPS.len(), 6);
        assert_eq!(STEPS[4].operation, management::Operation::Archive);
        assert!(STEPS[4].archive_frame && STEPS[4].pin_frame);
        assert_eq!(STEPS[5].operation, management::Operation::Restore);
        assert_eq!(STEPS[5].pins, &[1]);
        assert!(!STEPS[5].pin_frame && STEPS[5].archive_frame);
        assert!(STEPS[4].shot == Some(Shot::Archive) && STEPS[5].shot == Some(Shot::Restore));
    }
}
