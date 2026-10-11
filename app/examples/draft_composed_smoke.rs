//! Two owned native processes: real App draft Tasks, joined outcome hold, shutdown and cold recovery.
//! The hold occurs AFTER a real save has returned; no blocked-syscall/crash durability claim.
#![allow(dead_code)]
#[path = "../src/codex.rs"] mod codex;
#[path = "../src/config.rs"] mod config;
#[path = "../src/design.rs"] mod design;
#[path = "../src/export_file.rs"] mod export_file;
#[path = "../src/exporter.rs"] mod exporter;
#[path = "../src/fork.rs"] mod fork;
#[path = "../src/interactions.rs"] mod interactions;
#[path = "../src/known.rs"] mod known;
#[path = "../src/management.rs"] mod management;
#[path = "../src/plugins.rs"] mod plugins;
#[path = "../src/reducer.rs"] mod reducer;
#[path = "../src/rename.rs"] mod rename;
#[path = "../src/settings.rs"] mod settings;
#[path = "../src/smoke.rs"] mod smoke;
#[path = "../src/worker.rs"] mod worker;
mod ui {
    include!("../src/ui.rs");
    pub(super) use draft_recovery::Outcome as DraftOutcome;
    pub(super) fn startup(a: &App) -> bool {
        a.ready && a.root_ready && a.management.registry.ready() && !a.roster_pending
            && !a.catalog_read.pending() && a.draft_state.ready() && a.draft_state.failure().is_none()
    }
    pub(super) fn selected(a: &App, id: &SessionId) -> bool {
        a.selected.as_ref() == Some(id) && a.follow_ready && !a.roster_pending && a.ready
    }
    pub(super) fn roster_has(a: &App, id: &SessionId) -> bool { a.sessions.contains_key(id) }
    pub(super) fn busy(a: &App) -> bool { a.draft_state.busy() }
    pub(super) fn clean(a: &App) -> bool { a.draft_state.can_exit() }
    pub(super) fn failed(a: &App) -> bool { a.draft_state.failure().is_some() }
    pub(super) fn stopped(a: &App) -> bool { a.stopped }
    pub(super) fn failed_close(a: &App) -> bool { matches!(a.close, Close::Failed(_)) }
    pub(super) fn confirm_close(a: &App) -> bool { matches!(a.close, Close::Confirm(_)) }
    pub(super) fn stopping(a: &App) -> bool { matches!(a.close, Close::Stopping) }
    pub(super) fn loaded(a: &App) -> bool { a.draft_state.ready() && a.draft_state.failure().is_none() }
    pub(super) fn discard(a: &App) -> bool { a.draft_discard_on_exit }
    pub(super) fn pending_ticket_matches(a: &App, outcome: &DraftOutcome) -> bool {
        let ticket = match outcome { DraftOutcome::Loaded { ticket, .. } | DraftOutcome::Saved { ticket, .. } => ticket };
        a.draft_state.pending() == Some(*ticket)
    }
    pub(super) fn body(a: &App) -> String { a.editor.text() }
    pub(super) fn recovery_guard(a: &App) -> bool { a.recovered_draft_needs_review() && !a.send_allowed() }
    pub(super) fn send_allowed(a: &App) -> bool { a.send_allowed() }
    pub(super) fn counters(a: &App) -> (u64, u64) { (a.prompt_serial, a.smoke_evidence.model_prompts) }
    pub(super) fn durable(a: &App, id: &SessionId, expected: &str) -> bool {
        a.draft_snapshot().get(id).map(String::as_str) == Some(expected)
            && a.draft_store.as_ref().and_then(|store| store.lock().ok())
                .and_then(|mut store| store.load().ok())
                .is_some_and(|drafts| drafts.len() == 1 && drafts.get(id).map(String::as_str) == Some(expected))
    }
}
use dsh_native_transport::dto::*;
use iced::{Element, Subscription, Task, window};
use iced::widget::{column, container, text_editor};
use serde_json::{Value, json};
use std::{fs, io::{Read, Write}, os::unix::fs::{MetadataExt, OpenOptionsExt}, path::{Path, PathBuf}, time::Duration};
const FIRST: &str = "PUBLIC unsent draft\nΚαλημέρα · 日本語 · 🦀";
const MORE: &str = "\nPUBLIC newer edit while joined save outcome is held\nمرحبا · café";
const LATEST: &str = "PUBLIC unsent draft\nΚαλημέρα · 日本語 · 🦀\nPUBLIC newer edit while joined save outcome is held\nمرحبا · café";
#[derive(Clone, Copy, Debug, PartialEq, Eq)] enum Mode { Write, Recover }
#[derive(Clone, Copy, Debug, PartialEq, Eq)] enum Phase { Startup, Selecting, AwaitSave, HeldClosing, Recovering, Stopping }
#[derive(Clone, Debug)] enum Message { Ui(ui::Message), Tick, Deadline, Release }
fn private_write(path: &Path, value: &Value) -> Result<(), &'static str> {
    let mut file = fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path).map_err(|_| "private-artifact-create")?;
    file.write_all(value.to_string().as_bytes()).and_then(|_| file.sync_all()).map_err(|_| "private-artifact-write")
}
fn private_json(path: &Path) -> Result<Value, &'static str> {
    let before = fs::symlink_metadata(path).map_err(|_| "guard-missing")?;
    let uid = fs::metadata("/proc/self").map_err(|_| "guard-uid")?.uid();
    if !before.is_file() || before.file_type().is_symlink() || before.nlink() != 1 || before.uid() != uid
        || before.mode() & 0o7777 != 0o600 || before.len() > 16384 { return Err("guard-not-private-regular"); }
    let file = fs::OpenOptions::new().read(true).custom_flags(0o400000 | 0o4000 | 0o2000000).open(path).map_err(|_| "guard-open")?;
    let opened = file.metadata().map_err(|_| "guard-stat")?;
    if (opened.dev(), opened.ino(), opened.uid(), opened.mode(), opened.nlink(), opened.len())
        != (before.dev(), before.ino(), before.uid(), before.mode(), before.nlink(), before.len()) { return Err("guard-changed"); }
    let mut bytes = Vec::new();
    file.take(16385).read_to_end(&mut bytes).map_err(|_| "guard-read")?;
    if bytes.len() as u64 != before.len() { return Err("guard-size"); }
    serde_json::from_slice(&bytes).map_err(|_| "guard-invalid")
}
fn stamp(path: &Path) -> Result<Value, &'static str> {
    let info = fs::symlink_metadata(path).map_err(|_| "private-directory-missing")?;
    if !info.is_dir() || info.file_type().is_symlink() || info.mode() & 0o7777 != 0o700
        || info.uid() != fs::metadata("/proc/self").map_err(|_| "private-directory-uid")?.uid()
        || path.canonicalize().map_err(|_| "private-directory-resolve")? != path { return Err("private-canonical-directory-required"); }
    Ok(json!({"device":info.dev(),"inode":info.ino(),"uid":info.uid()}))
}
fn guard(options: &config::Options, mode: Mode) -> Result<(PathBuf, Option<SessionId>), &'static str> {
    if options.smoke.is_some() || options.backend.expected_version != "0.2.1-alpha.1" { return Err("ordinary-exact-alpha-required"); }
    let backend = &options.backend;
    let output = backend.native_home.parent().ok_or("private-root-required")?.to_owned();
    let allowed = Path::new(env!("CARGO_MANIFEST_DIR")).join("evidence").canonicalize().map_err(|_| "evidence-root-missing")?;
    if !output.starts_with(&allowed) || output == allowed
        || !output.file_name().is_some_and(|name| name.to_string_lossy().starts_with("PUBLIC-draft-restart-")) { return Err("explicit-PUBLIC-evidence-root-required"); }
    let root = stamp(&output)?;
    let mut directories = serde_json::Map::new();
    for (path, name) in [(&backend.native_home, "harness"), (&backend.user_home, "user"), (&backend.working_directory, "workspace")] {
        if path.parent() != Some(output.as_path()) || path.file_name().is_none_or(|value| value != name) { return Err("isolated-sibling-layout-required"); }
        directories.insert(name.into(), stamp(path)?);
        if mode == Mode::Write && fs::read_dir(path).map_err(|_| "private-directory-read")?.next().is_some() { return Err("fresh-empty-directories-required"); }
    }
    let manifest = json!({"version":1,"publicOnly":true,"root":root,"directories":directories,"runtime":backend.runtime,"sessionId":null,"firstPid":null});
    if mode == Mode::Write {
        private_write(&output.join("root-manifest.json"), &manifest)?;
        if output.join("session-manifest.json").exists() { return Err("fresh-root-required"); }
        return Ok((output, None));
    }
    if private_json(&output.join("root-manifest.json"))? != manifest { return Err("root-manifest-mismatch"); }
    let admitted = private_json(&output.join("session-manifest.json"))?;
    let mut expected = manifest;
    expected["sessionId"] = admitted["sessionId"].clone();
    expected["firstPid"] = admitted["firstPid"].clone();
    let subject = admitted["sessionId"].as_str().and_then(|id| SessionId::new(id).ok()).ok_or("PUBLIC-session-id-invalid")?;
    if admitted != expected || !admitted["firstPid"].as_u64().is_some_and(|pid| pid > 0 && pid != std::process::id() as u64) { return Err("first-process-manifest-invalid"); }
    let first_report = private_json(&output.join("write-app.json"))?;
    if first_report["status"] != "passed" || first_report["mode"] != "write"
        || first_report["pid"] != admitted["firstPid"] || first_report["sessionId"] != admitted["sessionId"] { return Err("successful-first-process-required"); }
    Ok((output, Some(subject)))
}
struct Fixture {
    app: ui::App, options: config::Options, output: PathBuf, mode: Mode, phase: Phase,
    subject: Option<SessionId>, held: Option<ui::DraftOutcome>, native: bool, failed: bool,
    completed: bool, report: Value,
}
impl Fixture {
    fn boot(options: config::Options, output: PathBuf, mode: Mode, subject: Option<SessionId>, handle: worker::Handle, feed: worker::Feed) -> (Self, Task<Message>) {
        let (app, task) = ui::App::boot(options.clone(), handle, feed);
        let report = json!({"status":"running","mode":if mode==Mode::Write{"write"}else{"recover"},"version":"0.2.1-alpha.1","pid":std::process::id(),"sessionId":subject.as_ref().map(SessionId::as_str),"publicOnly":true,"actualAppExercised":true,"actualDraftTasks":true,"wholeProfileUnchanged":true,"joinedOutcomeNotBlockedSyscall":true,"physicalInputQualified":false,"crashDurabilityQualified":false,"networkTraceQualified":false,"manualBusinessMessagesForwarded":0,"unexpectedWork":0,"modelCatalogReads":0,"modelCatalogSucceeded":false,"actualAppPromptCommands":0,"promptSerial":0,"driverPromptResumeModelSigninAllowed":false,"backendMetricsAvailable":false,"savedOutcomes":0,"draftRecoveryLoaded":false,"stoppedWhileBusy":false,"exitAnywayBlockedWhileBusy":false,"heldOutcomeReleased":false,"latestSnapshotDurable":false,"recoveredNeedsReview":false,"sendAllowed":false,"inspectionObserved":false,"confirmCloseDispatched":false,"draftIoJoined":false,"draftsCleanAtExit":false});
        (Self { app, options, output, mode, phase:Phase::Startup, subject, held:None, native:false, failed:false, completed:false, report },
            Task::batch([task.map(Message::Ui), Task::perform(async { tokio::time::sleep(Duration::from_secs(65)).await; }, |_| Message::Deadline)]))
    }
    fn dispatch(&mut self, message: ui::Message) -> Task<Message> { self.app.update(message).map(Message::Ui) }
    fn increment(&mut self, key: &str) { self.report[key] = json!(self.report[key].as_u64().unwrap_or(0) + 1); }
    fn fail(&mut self, code: &'static str) -> Task<Message> {
        self.failed = true;
        if self.report.get("errorCode").is_none() { self.report["errorCode"] = json!(code); }
        self.phase = Phase::Stopping;
        let release = self.held.take().map_or_else(Task::none, |outcome| self.dispatch(ui::Message::DraftOutcome(outcome)));
        let stop = if ui::stopped(&self.app) { self.dispatch(ui::Message::Close) } else { self.dispatch(ui::Message::ConfirmClose) };
        let finish = self.finish();
        Task::batch([release, stop, finish])
    }
    fn close(&mut self) -> Task<Message> { self.phase = Phase::Stopping; self.dispatch(ui::Message::Close) }
    fn paste(&mut self, text: &str) -> Task<Message> {
        self.dispatch(ui::Message::Editor(text_editor::Action::Edit(text_editor::Edit::Paste(std::sync::Arc::new(text.into())))))
    }
    fn tick(&mut self) -> Task<Message> {
        if ui::failed(&self.app) && !self.failed { return self.fail("actual-draft-IO-failed"); }
        match self.phase {
            Phase::Startup if self.native && ui::startup(&self.app) => {
                if self.mode == Mode::Write {
                    self.phase = Phase::Selecting;
                    self.dispatch(ui::Message::NewSession)
                } else {
                    let subject = self.subject.clone().unwrap();
                    if !ui::roster_has(&self.app, &subject) { return self.fail("cold-roster-missing-PUBLIC-session"); }
                    self.report["coldRosterContainedSubject"] = json!(true);
                    self.phase = Phase::Recovering;
                    self.dispatch(ui::Message::Select(subject))
                }
            }
            Phase::Selecting if self.subject.as_ref().is_some_and(|id| ui::selected(&self.app, id)) => {
                if !ui::body(&self.app).is_empty() { return self.fail("new-session-editor-not-empty"); }
                let paste = self.paste(FIRST);
                if ui::body(&self.app) != FIRST { return self.fail("actual-first-editor-mismatch"); }
                self.phase = Phase::AwaitSave;
                let save = self.dispatch(ui::Message::DraftFlush);
                if !ui::busy(&self.app) { return self.fail("actual-save-ticket-not-admitted"); }
                Task::batch([paste, save])
            }
            Phase::Recovering if self.subject.as_ref().is_some_and(|id| ui::selected(&self.app, id)) => {
                if ui::body(&self.app) != LATEST || !ui::recovery_guard(&self.app) { return self.fail("cold-recovery-body-or-review-guard-mismatch"); }
                self.report["recoveredNeedsReview"] = json!(true);
                self.report["sendAllowed"] = json!(ui::send_allowed(&self.app));
                self.report["coldExplicitSelectOnly"] = json!(true);
                self.close()
            }
            Phase::Stopping => self.finish(),
            _ => Task::none(),
        }
    }
    fn finish(&mut self) -> Task<Message> {
        if self.completed || !ui::stopped(&self.app) || ui::busy(&self.app) { return Task::none(); }
        if !ui::clean(&self.app) && !ui::failed_close(&self.app) { return Task::none(); }
        self.report["draftIoJoined"] = json!(true);
        self.report["draftsCleanAtExit"] = json!(ui::clean(&self.app));
        let (serial, prompts) = ui::counters(&self.app);
        self.report["promptSerial"] = json!(serial);
        self.report["actualAppPromptCommands"] = json!(prompts);
        let durable = self.subject.as_ref().is_some_and(|id| ui::durable(&self.app, id, LATEST));
        self.report["latestSnapshotDurable"] = json!(durable);
        let passed = !self.failed && self.native && ui::clean(&self.app) && !ui::failed_close(&self.app)
            && self.report["gracefulStopped"] == true && self.report["draftRecoveryLoaded"] == true
            && self.report["modelCatalogReads"] == 1 && self.report["modelCatalogSucceeded"] == true
            && serial == 0 && prompts == 0 && self.report["unexpectedWork"] == 0 && durable
            && self.report["completePUBLICSnapshotObserved"] == true && self.report["inspectionObserved"] == true
            && if self.mode == Mode::Write { self.report["savedOutcomes"] == 2 && self.report["stoppedWhileBusy"] == true
                && self.report["exitAnywayBlockedWhileBusy"] == true && self.report["heldOutcomeReleased"] == true }
                else { self.report["recoveredNeedsReview"] == true && self.report["coldExplicitSelectOnly"] == true };
        self.report["status"] = json!(if passed { "passed" } else { "failed" });
        self.completed = true;
        let name = if self.mode == Mode::Write { "write-app.json" } else { "recover-app.json" };
        if private_write(&self.output.join(name), &self.report).is_err() { self.report["status"] = json!("failed"); }
        println!("{}", self.report);
        if ui::failed_close(&self.app) { self.dispatch(ui::Message::ExitAnyway) } else { Task::none() }
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Ui(ui::Message::NativeOpened(id)) => self.dispatch(ui::Message::NativeOpened(id)),
            Message::Ui(message @ ui::Message::NativeInfo { .. }) => {
                if let ui::Message::NativeInfo { backend, .. } = &message { self.native = *backend == "wayland"; self.report["windowBackend"] = json!(backend); }
                self.dispatch(message)
            }
            Message::Ui(message @ (ui::Message::Resized(..) | ui::Message::WindowActive(..) | ui::Message::MotionFrame(..) | ui::Message::WorkingTick | ui::Message::ScrollApplied { .. })) => self.dispatch(message),
            Message::Ui(ui::Message::DraftOutcome(outcome)) => {
                if !ui::pending_ticket_matches(&self.app, &outcome) { return self.fail("unexpected-draft-outcome-ticket"); }
                if let ui::DraftOutcome::Saved { ticket, result } = &outcome {
                    self.increment("savedOutcomes");
                    if result.is_err() { let task = self.dispatch(ui::Message::DraftOutcome(outcome)); return Task::batch([task, self.fail("real-save-failed")]); }
                    if self.phase == Phase::AwaitSave && self.mode == Mode::Write {
                        self.report["heldRevision"] = json!(ticket.revision);
                        self.held = Some(outcome);
                        self.phase = Phase::HeldClosing;
                        let paste = self.paste(MORE);
                        if ui::body(&self.app) != LATEST || !ui::busy(&self.app) { return self.fail("newer-edit-not-owned-while-busy"); }
                        let close = self.dispatch(ui::Message::Close);
                        return Task::batch([paste, close]);
                    }
                    self.report["latestSavedRevision"] = json!(ticket.revision);
                }
                let loaded = matches!(&outcome, ui::DraftOutcome::Loaded { result: Ok(_), .. });
                let task = self.dispatch(ui::Message::DraftOutcome(outcome));
                if loaded && ui::loaded(&self.app) { self.report["draftRecoveryLoaded"] = json!(true); }
                let finish = self.finish(); // Write report BEFORE returning the production exit Task.
                Task::batch([task, finish])
            }
            Message::Ui(ui::Message::DraftFlush) => self.dispatch(ui::Message::DraftFlush),
            Message::Ui(ui::Message::Worker(event)) => {
                if matches!(&event,worker::Event::Selected { frame:SessionFollowFrame::AssistantStream { .. }, .. }) {self.increment("unexpectedWork");return self.fail("unexpected-live-assistant-stream");}
                if let worker::Event::Selected { frame:SessionFollowFrame::Event { event }, .. } = &event {
                    if event.event_type.contains("llm") || ["step/start","step/end","request/header","tool/call","tool/result","assistant/message","user/message"].contains(&event.event_type.as_str()) {
                        self.increment("unexpectedWork"); return self.fail("unexpected-real-session-agent-work");
                    }
                }
                if let worker::Event::Selected { frame:SessionFollowFrame::Snapshot { records, has_more, assistant_stream, .. }, .. } = &event {
                    let kinds: Vec<_> = records.iter().map(|HistoryRecord::Event { event }| event.event_type.as_str()).collect();
                    self.report["observedPUBLICRecordKinds"] = json!(kinds);
                    let idle_baseline=idle_assistant_baseline(assistant_stream.as_ref());
                    self.report["assistantBaselinePresent"] = json!(assistant_stream.is_some());
                    self.report["activeAssistantAttemptPresent"] = json!(!idle_baseline);
                    self.report["completePUBLICSnapshotObserved"] = json!(!has_more && idle_baseline);
                    if *has_more || !idle_baseline || kinds.iter().any(|kind| {
                        kind.contains("llm") || ["step/start","step/end","request/header","tool/call","tool/result","assistant/message","user/message"].contains(kind)
                    }) { self.increment("unexpectedWork"); return self.fail("unexpected-PUBLIC-recorded-model-work"); }
                }
                if let worker::Event::Created(Ok(value)) = &event {
                    if self.mode != Mode::Write || self.subject.is_some() { return self.fail("unexpected-session-created"); }
                    self.subject = Some(value.session_id.clone());
                    self.report["sessionId"] = json!(value.session_id.as_str());
                    let manifest = private_json(&self.output.join("root-manifest.json"));
                    let Ok(mut manifest) = manifest else { return self.fail("root-manifest-unavailable"); };
                    manifest["sessionId"] = json!(value.session_id.as_str()); manifest["firstPid"] = json!(std::process::id());
                    if private_write(&self.output.join("session-manifest.json"), &manifest).is_err() { return self.fail("session-manifest-write-failed"); }
                }
                if let worker::Event::Catalog { result, .. } = &event {
                    self.increment("modelCatalogReads"); self.report["modelCatalogSucceeded"] = json!(result.is_ok());
                    if result.is_err() || self.report["modelCatalogReads"] != 1 { let task = self.dispatch(ui::Message::Worker(event)); return Task::batch([task, self.fail("keyless-catalog-not-qualified")]); }
                }
                if matches!(&event, worker::Event::Fault(_) | worker::Event::FollowError { .. } | worker::Event::Roster(Err(_)) | worker::Event::Created(Err(_)) | worker::Event::NoBackendStarted) {
                    let task = self.dispatch(ui::Message::Worker(event)); return Task::batch([task, self.fail("real-worker-failed")]);
                }
                if let worker::Event::Inspection(result) = &event {
                    self.report["inspectionObserved"] = json!(true);
                    self.report["inspectionKnownIdle"] = json!(result.as_ref().is_ok_and(|value| value.is_known_idle()));
                    let task = self.dispatch(ui::Message::Worker(event));
                    self.report["inspectionAutoShutdown"] = json!(ui::stopping(&self.app));
                    if self.phase != Phase::HeldClosing { self.phase = Phase::Stopping; }
                    let confirm = if ui::confirm_close(&self.app) {
                        self.report["confirmCloseDispatched"] = json!(true);
                        self.dispatch(ui::Message::ConfirmClose)
                    } else { Task::none() };
                    return Task::batch([task, confirm]);
                }
                if let worker::Event::Stopped(result) = &event {
                    let clean = result.as_ref().is_ok_and(|stop| stop.exited && stop.graceful && !stop.containment_unknown && stop.observed_descendants_remaining == 0 && stop.exit_code == Some(0));
                    self.report["gracefulStopped"] = json!(clean);
                    if let Ok(stop) = result { self.report["stop"] = json!({"exited":stop.exited,"graceful":stop.graceful,"containmentUnknown":stop.containment_unknown,"observedDescendantsRemaining":stop.observed_descendants_remaining,"exitCode":stop.exit_code}); }
                    let task = self.dispatch(ui::Message::Worker(event));
                    if self.phase == Phase::HeldClosing {
                        self.report["stoppedWhileBusy"] = json!(ui::stopped(&self.app) && ui::busy(&self.app) && !ui::clean(&self.app) && task.units() == 0);
                        let refused = self.dispatch(ui::Message::ExitAnyway);
                        self.report["exitAnywayBlockedWhileBusy"] = json!(ui::busy(&self.app) && !ui::discard(&self.app) && refused.units() == 0);
                        self.phase = Phase::Stopping;
                        return Task::batch([task, refused, Task::done(Message::Release)]);
                    }
                    let finish = self.finish(); return Task::batch([task, finish]);
                }
                self.dispatch(ui::Message::Worker(event))
            }
            Message::Release => {
                let Some(outcome) = self.held.take() else { return self.fail("held-real-outcome-missing"); };
                self.report["heldOutcomeReleased"] = json!(true);
                let task = self.dispatch(ui::Message::DraftOutcome(outcome));
                if !ui::busy(&self.app) { return Task::batch([task, self.fail("latest-coalesced-save-not-admitted")]); }
                task
            }
            Message::Ui(ui::Message::Close) => self.fail("manual-close-before-complete"),
            Message::Tick => self.tick(),
            Message::Deadline => self.fail("owned-draft-fixture-deadline"),
            _ => Task::none(), // Never forward manual editor/model/consent/account/workspace actions.
        }
    }
    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([self.app.subscription().map(Message::Ui), iced::time::every(Duration::from_millis(60)).map(|_|Message::Tick)])
    }
    fn view(&self) -> Element<'_, Message> {
        container(column![ui::label("PUBLIC UNSENT DRAFT · actual joined save / process restart · no Send", ui::DANGER), self.app.view().map(Message::Ui)].spacing(8)).padding(12).height(iced::Length::Fill).into()
    }
}
fn idle_assistant_baseline(baseline:Option<&AssistantBaseline>)->bool {baseline.is_none_or(|baseline|baseline.active_attempt.is_none())}
#[cfg(test)] mod baseline_tests {
    use super::*;
    #[test] fn idle_opening_baseline_is_not_an_active_attempt(){
        assert!(idle_assistant_baseline(None));
        assert!(idle_assistant_baseline(Some(&AssistantBaseline{revision:0,active_attempt:None})));
        assert!(!idle_assistant_baseline(Some(&AssistantBaseline{revision:1,active_attempt:Some(AssistantAttempt{attempt_id:"PUBLIC".into(),started_after_seq:0,turn:1,step:1,next_index:0,stream:vec![]})})));
    }
}
fn main() -> iced::Result {
    let mut arguments = std::env::args().skip(1);
    let mode = match (arguments.next().as_deref(), arguments.next().as_deref()) {
        (Some("--fixture-mode"), Some("write")) => Mode::Write,
        (Some("--fixture-mode"), Some("recover")) => Mode::Recover,
        _ => { eprintln!("Explicit --fixture-mode write|recover required"); std::process::exit(2); }
    };
    let options = config::parse(arguments).ok().flatten().unwrap_or_else(|| { eprintln!("Explicit native fixture options required"); std::process::exit(2) });
    let (output, subject) = guard(&options, mode).unwrap_or_else(|error| { eprintln!("{error}"); std::process::exit(2) });
    unsafe { std::env::set_var("ICED_BACKEND", "wgpu"); }
    let (handle, feed, owner) = worker::start(options.backend.clone(), false).unwrap_or_else(|_| { eprintln!("Owned fixture worker unavailable"); std::process::exit(1) });
    let result = iced::application(move ||Fixture::boot(options.clone(), output.clone(), mode, subject.clone(), handle.clone(), feed.clone()), Fixture::update, Fixture::view)
        .title("Harness PUBLIC draft restart fixture")
        .window(window::Settings { size:iced::Size::new(1000.0,720.0), platform_specific:window::settings::PlatformSpecific { application_id:"ai.deepseek.harness.native.draft-composed-fixture".into(), ..Default::default() }, ..Default::default() })
        .exit_on_close_request(false).default_font(ui::FONT).theme(|_: &Fixture|ui::native_theme())
        .scale_factor(|fixture:&Fixture|fixture.options.scale).subscription(Fixture::subscription).run();
    drop(owner);
    result
}
