//! Real native App save interruption at production Store checkpoints; not kernel/power-loss evidence.
//! Test observers exist only with public-layout-fixture. All agent work and manual business input are refused.
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
#[path = "../src/fork.rs"]
mod fork;
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
    pub(super) use draft_store::{SaveCheckpoint, SaveObserver};
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
        a.selected.as_ref() == Some(id) && a.follow_ready && !a.roster_pending && a.ready
    }
    pub(super) fn contains(a: &App, id: &SessionId) -> bool {
        a.sessions.contains_key(id)
    }
    pub(super) fn body(a: &App) -> String {
        a.editor.text()
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
    pub(super) fn confirmation(a: &App) -> bool {
        matches!(a.close, Close::Confirm(_))
    }
    pub(super) fn failed_close(a: &App) -> bool {
        matches!(a.close, Close::Failed(_))
    }
    pub(super) fn discarded(a: &App) -> bool {
        a.draft_discard_on_exit
    }
    pub(super) fn reviewed(a: &App) -> bool {
        a.recovered_draft_needs_review() && !a.send_allowed()
    }
    pub(super) fn counters(a: &App) -> (u64, u64) {
        (a.prompt_serial, a.smoke_evidence.model_prompts)
    }
    pub(super) fn install(a: &App, observer: SaveObserver) -> bool {
        if a.draft_state.busy() {
            return false;
        }
        a.draft_store
            .as_ref()
            .and_then(|store| store.lock().ok())
            .is_some_and(|mut store| {
                store.set_save_observer(Some(observer));
                true
            })
    }
    pub(super) fn durable(a: &App, id: &SessionId, body: &str) -> bool {
        if a.draft_state.busy() {
            return false;
        }
        a.draft_snapshot().get(id).map(String::as_str) == Some(body)
            && a.draft_store
                .as_ref()
                .and_then(|store| store.lock().ok())
                .and_then(|mut store| store.load().ok())
                .is_some_and(|drafts| {
                    drafts.len() == 1 && drafts.get(id).map(String::as_str) == Some(body)
                })
    }
}
use dsh_native_transport::dto::*;
use iced::widget::{column, container, text_editor};
use iced::{Element, Subscription, Task, window};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};
const OLD: &str = "PUBLIC committed draft\nΚαλημέρα · 日本語 · 🦀";
const EDIT: &str = "\nPUBLIC interrupted replacement · مرحبا";
const NEW: &str =
    "PUBLIC committed draft\nΚαλημέρα · 日本語 · 🦀\nPUBLIC interrupted replacement · مرحبا";
const EXTRA: &str = "\nPUBLIC newest text while actual save is unfinished · café";
const LATEST: &str = "PUBLIC committed draft\nΚαλημέρα · 日本語 · 🦀\nPUBLIC interrupted replacement · مرحبا\nPUBLIC newest text while actual save is unfinished · café";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Hold,
    Crash,
    Recover,
}
impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Hold => "hold",
            Self::Crash => "crash",
            Self::Recover => "recover",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Point {
    BeforeWrite,
    AfterWrite,
    BeforeTempSync,
    AfterTempSync,
    BeforeRename,
    AfterRename,
    BeforeDirSync,
    AfterDirSync,
    BeforeAck,
}
impl Point {
    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "before-write" => Self::BeforeWrite,
            "after-write" => Self::AfterWrite,
            "before-temp-sync" => Self::BeforeTempSync,
            "after-temp-sync" => Self::AfterTempSync,
            "before-rename" => Self::BeforeRename,
            "after-rename" => Self::AfterRename,
            "before-dir-sync" => Self::BeforeDirSync,
            "after-dir-sync" => Self::AfterDirSync,
            "before-ack" => Self::BeforeAck,
            _ => return None,
        })
    }
    fn name(self) -> &'static str {
        match self {
            Self::BeforeWrite => "before-write",
            Self::AfterWrite => "after-write",
            Self::BeforeTempSync => "before-temp-sync",
            Self::AfterTempSync => "after-temp-sync",
            Self::BeforeRename => "before-rename",
            Self::AfterRename => "after-rename",
            Self::BeforeDirSync => "before-dir-sync",
            Self::AfterDirSync => "after-dir-sync",
            Self::BeforeAck => "before-ack",
        }
    }
    fn step(self) -> ui::SaveCheckpoint {
        match self {
            Self::BeforeWrite => ui::SaveCheckpoint::BeforeTempWrite,
            Self::AfterWrite => ui::SaveCheckpoint::AfterTempWrite,
            Self::BeforeTempSync => ui::SaveCheckpoint::BeforeTempSync,
            Self::AfterTempSync => ui::SaveCheckpoint::AfterTempSync,
            Self::BeforeRename => ui::SaveCheckpoint::BeforeRename,
            Self::AfterRename => ui::SaveCheckpoint::AfterRename,
            Self::BeforeDirSync => ui::SaveCheckpoint::BeforeDirectorySync,
            Self::AfterDirSync => ui::SaveCheckpoint::AfterDirectorySync,
            Self::BeforeAck => ui::SaveCheckpoint::BeforeAcknowledgement,
        }
    }
    fn expected(self) -> &'static str {
        match self {
            Self::BeforeWrite
            | Self::AfterWrite
            | Self::BeforeTempSync
            | Self::AfterTempSync
            | Self::BeforeRename => OLD,
            _ => NEW,
        }
    }
}
fn write_json(path: &Path, value: &Value) -> std::io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(value.to_string().as_bytes())?;
    file.sync_all()
}
fn read_json(path: &Path) -> Result<Value, &'static str> {
    use std::io::Read;
    let before = fs::symlink_metadata(path).map_err(|_| "missing-artifact")?;
    let uid = fs::metadata("/proc/self")
        .map_err(|_| "uid-unavailable")?
        .uid();
    if !before.is_file()
        || before.file_type().is_symlink()
        || before.mode() & 0o7777 != 0o600
        || before.uid() != uid
        || before.nlink() != 1
        || before.len() > 16384
    {
        return Err("invalid-private-artifact");
    }
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(0o400000 | 0o4000 | 0o2000000)
        .open(path)
        .map_err(|_| "artifact-open")?;
    let opened = file.metadata().map_err(|_| "artifact-stat")?;
    if (
        before.dev(),
        before.ino(),
        before.mode(),
        before.uid(),
        before.nlink(),
        before.len(),
    ) != (
        opened.dev(),
        opened.ino(),
        opened.mode(),
        opened.uid(),
        opened.nlink(),
        opened.len(),
    ) {
        return Err("artifact-changed");
    }
    let mut bytes = vec![];
    (&mut file)
        .take(16385)
        .read_to_end(&mut bytes)
        .map_err(|_| "artifact-read")?;
    if bytes.len() as u64 != before.len() {
        return Err("artifact-size-changed");
    }
    serde_json::from_slice(&bytes).map_err(|_| "artifact-json")
}
fn stamp(path: &Path) -> Result<Value, &'static str> {
    let m = fs::symlink_metadata(path).map_err(|_| "directory-missing")?;
    if !m.is_dir()
        || m.file_type().is_symlink()
        || m.mode() & 0o7777 != 0o700
        || m.uid() != fs::metadata("/proc/self").map_err(|_| "uid")?.uid()
        || path.canonicalize().map_err(|_| "canonical")? != path
    {
        return Err("private-canonical-directory-required");
    }
    Ok(json!({"device":m.dev(),"inode":m.ino(),"uid":m.uid()}))
}
fn guard(
    o: &config::Options,
    mode: Mode,
    point: Point,
) -> Result<(PathBuf, Option<SessionId>), &'static str> {
    if o.smoke.is_some() || o.backend.expected_version != "0.2.1-alpha.1" {
        return Err("ordinary-alpha-required");
    }
    let root = o
        .backend
        .native_home
        .parent()
        .ok_or("root-required")?
        .to_owned();
    let allowed = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("evidence")
        .canonicalize()
        .map_err(|_| "evidence-root")?;
    if !root.starts_with(&allowed)
        || root == allowed
        || !root.file_name().is_some_and(|n| {
            n.to_string_lossy()
                .starts_with("PUBLIC-draft-interruption-")
        })
    {
        return Err("PUBLIC-root-required");
    }
    let mut dirs = serde_json::Map::new();
    for (p, n) in [
        (&o.backend.native_home, "harness"),
        (&o.backend.user_home, "user"),
        (&o.backend.working_directory, "workspace"),
    ] {
        if p.parent() != Some(root.as_path()) || p.file_name().is_none_or(|v| v != n) {
            return Err("isolated-sibling-required");
        }
        dirs.insert(n.into(), stamp(p)?);
        if mode != Mode::Recover
            && fs::read_dir(p)
                .map_err(|_| "directory-read")?
                .next()
                .is_some()
        {
            return Err("fresh-empty-required");
        }
    }
    let manifest = json!({"version":1,"publicOnly":true,"root":stamp(&root)?,"directories":dirs,"runtime":o.backend.runtime,"checkpoint":point.name()});
    if mode != Mode::Recover {
        write_json(&root.join("root-manifest.json"), &manifest)
            .map_err(|_| "root-manifest-write")?;
        return Ok((root, None));
    }
    if read_json(&root.join("root-manifest.json"))? != manifest {
        return Err("root-manifest-mismatch");
    }
    let session = read_json(&root.join("session-manifest.json"))?;
    let subject = session["sessionId"]
        .as_str()
        .and_then(|s| SessionId::new(s).ok())
        .ok_or("session-id")?;
    let pid = session["firstPid"]
        .as_u64()
        .filter(|p| *p > 0 && *p != std::process::id() as u64)
        .ok_or("first-pid")?;
    if session
        != json!({"version":1,"publicOnly":true,"sessionId":subject.as_str(),"firstPid":pid,"checkpoint":point.name()})
    {
        return Err("session-manifest-mismatch");
    }
    let terminated = read_json(&root.join("termination.json"))?;
    let observed = read_json(&root.join("crash-observed.json"))?;
    if terminated["pid"] != pid
        || terminated["nativeReaped"] != true
        || terminated["ownedCleanupComplete"] != true
        || terminated["signal"] != 9
        || observed["pid"] != pid
        || observed["checkpoint"] != point.name()
        || observed["sessionId"] != subject.as_str()
        || observed["saveUnfinished"] != true
        || observed["crashGateLatchedUntilOwnedKill"] != true
        || observed["promptSerial"] != 0
        || observed["actualAppPromptCommands"] != 0
    {
        return Err("owned-crash-receipt-required");
    }
    Ok((root, Some(subject)))
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GateWaitPolicy {
    ReleaseWithin(Duration),
    UntilOwnedKill,
}
impl GateWaitPolicy {
    fn for_mode(mode: Mode) -> Self {
        match mode {
            Mode::Hold | Mode::Recover => Self::ReleaseWithin(Duration::from_secs(25)),
            Mode::Crash => Self::UntilOwnedKill,
        }
    }
}
struct Gate {
    policy: GateWaitPolicy,
    reached: Arc<AtomicBool>,
    released: Arc<AtomicBool>,
    sender: Option<mpsc::SyncSender<()>>,
}
impl Gate {
    fn make(point: Point, mode: Mode) -> (Self, ui::SaveObserver) {
        let policy = GateWaitPolicy::for_mode(mode);
        let (tx, rx) = mpsc::sync_channel(1);
        let receiver = Mutex::new(rx);
        let reached = Arc::new(AtomicBool::new(false));
        let released = Arc::new(AtomicBool::new(false));
        let (r, s) = (reached.clone(), released.clone());
        let observer = Arc::new(move |step| {
            if step == point.step() && !r.swap(true, Ordering::SeqCst) {
                match policy {
                    GateWaitPolicy::UntilOwnedKill => loop {
                        // Crash requires the parent's finite 70s deadline and owned pidfd cleanup.
                        // Never return on timeout, release, Drop, GUI failure or spurious wakeup:
                        // an emitted receipt must describe an unfinished save until process kill.
                        std::thread::park();
                    },
                    GateWaitPolicy::ReleaseWithin(timeout) => {
                        receiver
                            .lock()
                            .map_err(|_| std::io::Error::other("PUBLIC gate poisoned"))?
                            .recv_timeout(timeout)
                            .map_err(|_| std::io::Error::other("PUBLIC gate timeout"))?;
                        s.store(true, Ordering::SeqCst);
                    }
                }
            }
            Ok(())
        });
        (
            Self {
                policy,
                reached,
                released,
                sender: match policy {
                    GateWaitPolicy::ReleaseWithin(_) => Some(tx),
                    GateWaitPolicy::UntilOwnedKill => None,
                },
            },
            observer,
        )
    }
    fn latched_until_owned_kill(&self) -> bool {
        self.policy == GateWaitPolicy::UntilOwnedKill
            && self.reached.load(Ordering::SeqCst)
            && !self.released.load(Ordering::SeqCst)
    }
    fn release(&mut self) {
        if let Some(sender) = self.sender.take() {
            let _ = sender.try_send(());
        }
    }
}
impl Drop for Gate {
    fn drop(&mut self) {
        self.release();
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Startup,
    Selecting,
    Seed,
    Waiting,
    Blocked,
    Stopping,
}
#[derive(Clone, Debug)]
enum Message {
    Ui(ui::Message),
    Tick,
    Deadline,
}
struct Fixture {
    app: ui::App,
    output: PathBuf,
    mode: Mode,
    point: Point,
    phase: Phase,
    subject: Option<SessionId>,
    gate: Option<Gate>,
    native: bool,
    failed: bool,
    completed: bool,
    observed: bool,
    heartbeat: u64,
    report: Value,
}
impl Fixture {
    fn boot(
        o: config::Options,
        output: PathBuf,
        mode: Mode,
        point: Point,
        subject: Option<SessionId>,
        handle: worker::Handle,
        feed: worker::Feed,
    ) -> (Self, Task<Message>) {
        let (app, task) = ui::App::boot(o, handle, feed);
        let report = json!({"status":"running","version":"0.2.1-alpha.1","mode":mode.name(),"checkpoint":point.name(),"pid":std::process::id(),"sessionId":subject.as_ref().map(SessionId::as_str),"publicOnly":true,"actualAppPromptCommands":0,"promptSerial":0,"catalogReads":0,"catalogSucceeded":false,"savedOutcomes":0,"unexpectedWork":0,"manualBusinessMessagesForwarded":0,"kernelBlockedFsyncQualified":false,"powerLossQualified":false,"physicalInputQualified":false,"networkTraceQualified":false});
        (
            Self {
                app,
                output,
                mode,
                point,
                phase: Phase::Startup,
                subject,
                gate: None,
                native: false,
                failed: false,
                completed: false,
                observed: false,
                heartbeat: 0,
                report,
            },
            Task::batch([
                task.map(Message::Ui),
                Task::perform(
                    async {
                        tokio::time::sleep(Duration::from_secs(60)).await;
                    },
                    |_| Message::Deadline,
                ),
            ]),
        )
    }
    fn dispatch(&mut self, m: ui::Message) -> Task<Message> {
        self.app.update(m).map(Message::Ui)
    }
    fn count(&mut self, key: &str) {
        self.report[key] = json!(self.report[key].as_u64().unwrap_or(0) + 1);
    }
    fn release(&mut self) {
        if let Some(g) = &mut self.gate {
            g.release();
        }
    }
    fn fail(&mut self, code: &'static str) -> Task<Message> {
        self.failed = true;
        if self.report.get("errorCode").is_none() {
            self.report["errorCode"] = json!(code);
        }
        self.release();
        self.phase = Phase::Stopping;
        let stop = if ui::stopped(&self.app) {
            self.dispatch(ui::Message::Close)
        } else {
            self.dispatch(ui::Message::ConfirmClose)
        };
        let done = self.finish();
        Task::batch([stop, done])
    }
    fn paste(&mut self, s: &str) -> Task<Message> {
        self.dispatch(ui::Message::Editor(text_editor::Action::Edit(
            text_editor::Edit::Paste(Arc::new(s.into())),
        )))
    }
    fn expected(&self) -> &'static str {
        if self.mode == Mode::Hold {
            LATEST
        } else {
            self.point.expected()
        }
    }
    fn tick(&mut self) -> Task<Message> {
        if ui::failed(&self.app) && !self.failed {
            return self.fail("draft-save-failed");
        }
        match self.phase {
            Phase::Startup if self.native && ui::startup(&self.app) => {
                self.phase = Phase::Selecting;
                if self.mode == Mode::Recover {
                    let id = self.subject.clone().unwrap();
                    if !ui::contains(&self.app, &id) {
                        return self.fail("cold-roster-missing");
                    }
                    self.dispatch(ui::Message::Select(id))
                } else {
                    self.dispatch(ui::Message::NewSession)
                }
            }
            Phase::Selecting
                if self
                    .subject
                    .as_ref()
                    .is_some_and(|id| ui::selected(&self.app, id)) =>
            {
                if self.mode == Mode::Recover {
                    if ui::body(&self.app) != self.expected() || !ui::reviewed(&self.app) {
                        return self.fail("crash-recovery-body-or-review-mismatch");
                    }
                    self.report["recoveredNeedsReview"] = json!(true);
                    self.phase = Phase::Stopping;
                    self.dispatch(ui::Message::Close)
                } else {
                    if !ui::body(&self.app).is_empty() {
                        return self.fail("fresh-editor-not-empty");
                    }
                    let edit = self.paste(OLD);
                    self.phase = Phase::Seed;
                    let save = self.dispatch(ui::Message::DraftFlush);
                    Task::batch([edit, save])
                }
            }
            Phase::Seed if ui::clean(&self.app) && self.report["savedOutcomes"] == 1 => {
                if !ui::durable(&self.app, self.subject.as_ref().unwrap(), OLD) {
                    return self.fail("old-seed-not-durable");
                }
                let (gate, observer) = Gate::make(self.point, self.mode);
                if !ui::install(&self.app, observer) {
                    return self.fail("observer-not-installed");
                }
                self.gate = Some(gate);
                let edit = self.paste(EDIT);
                if ui::body(&self.app) != NEW {
                    return self.fail("replacement-editor-mismatch");
                }
                self.phase = Phase::Waiting;
                let save = self.dispatch(ui::Message::DraftFlush);
                Task::batch([edit, save])
            }
            Phase::Waiting
                if self
                    .gate
                    .as_ref()
                    .is_some_and(|g| g.reached.load(Ordering::SeqCst)) =>
            {
                if !ui::busy(&self.app)
                    || self.report["savedOutcomes"] != 1
                    || self.gate.as_ref().unwrap().released.load(Ordering::SeqCst)
                {
                    return self.fail("unfinished-save-not-owned");
                }
                self.report["saveUnfinished"] = json!(true);
                self.phase = Phase::Blocked;
                if self.mode == Mode::Hold {
                    let edit = self.paste(EXTRA);
                    let close = self.dispatch(ui::Message::Close);
                    Task::batch([edit, close])
                } else {
                    Task::none()
                }
            }
            Phase::Blocked => {
                self.heartbeat += 1;
                if self.mode == Mode::Crash && !self.observed {
                    // App intent counters, including queue refusal; not a network/provider trace.
                    let (serial, prompts) = ui::counters(&self.app);
                    self.report["promptSerial"] = json!(serial);
                    self.report["actualAppPromptCommands"] = json!(prompts);
                    if serial != 0 || prompts != 0 {
                        return self.fail("unexpected-app-prompt-intent");
                    }
                    let latched = self
                        .gate
                        .as_ref()
                        .is_some_and(Gate::latched_until_owned_kill);
                    if !latched || !ui::busy(&self.app) || self.report["savedOutcomes"] != 1 {
                        return self.fail("crash-save-not-latched");
                    }
                    let mut observed = self.report.clone();
                    observed["status"] = json!("checkpoint-observed");
                    observed["sessionId"] = json!(self.subject.as_ref().unwrap().as_str());
                    observed["saveUnfinished"] = json!(ui::busy(&self.app));
                    observed["crashGateLatchedUntilOwnedKill"] = json!(latched);
                    if write_json(&self.output.join("crash-observed.json"), &observed).is_err() {
                        return self.fail("crash-observation-write");
                    }
                    self.observed = true;
                }
                if self.mode == Mode::Hold && ui::stopped(&self.app) && self.heartbeat >= 3 {
                    if !ui::busy(&self.app)
                        || self.report["savedOutcomes"] != 1
                        || self.gate.as_ref().unwrap().released.load(Ordering::SeqCst)
                    {
                        return self.fail("save-finished-before-stop");
                    }
                    let refused = self.dispatch(ui::Message::ExitAnyway);
                    if refused.units() != 0 || !ui::busy(&self.app) || ui::discarded(&self.app) {
                        return self.fail("exit-anyway-bypassed-unfinished-save");
                    }
                    self.report["stoppedWhileSaveUnfinished"] = json!(true);
                    self.report["exitAnywayRefused"] = json!(true);
                    self.report["guiHeartbeatsWhileBlocked"] = json!(self.heartbeat);
                    self.release();
                    self.phase = Phase::Stopping;
                }
                self.finish()
            }
            _ => self.finish(),
        }
    }
    fn finish(&mut self) -> Task<Message> {
        if self.completed
            || self.mode == Mode::Crash && !self.failed
            || self.phase != Phase::Stopping
            || !ui::stopped(&self.app)
            || ui::busy(&self.app)
        {
            return Task::none();
        }
        let clean = ui::clean(&self.app);
        if !clean && !ui::failed_close(&self.app) {
            return Task::none();
        }
        let (serial, prompts) = ui::counters(&self.app);
        self.report["promptSerial"] = json!(serial);
        self.report["actualAppPromptCommands"] = json!(prompts);
        self.report["draftIoJoined"] = json!(true);
        self.report["draftsCleanAtExit"] = json!(clean);
        let durable = self
            .subject
            .as_ref()
            .is_some_and(|id| ui::durable(&self.app, id, self.expected()));
        self.report["expectedSnapshotDurable"] = json!(durable);
        let saved = self.report["savedOutcomes"].as_u64();
        let counts = if self.mode == Mode::Hold {
            saved == Some(3)
        } else {
            saved == Some(0)
        };
        let ok = !self.failed
            && clean
            && durable
            && counts
            && serial == 0
            && prompts == 0
            && self.native
            && self.report["catalogReads"] == 1
            && self.report["catalogSucceeded"] == true
            && self.report["gracefulStopped"] == true;
        self.report["status"] = json!(if ok { "passed" } else { "failed" });
        self.report["sessionId"] = json!(self.subject.as_ref().map(SessionId::as_str));
        if write_json(
            &self.output.join(format!("{}-app.json", self.mode.name())),
            &self.report,
        )
        .is_err()
        {
            self.failed = true;
        }
        println!("{}", self.report);
        self.completed = true;
        if ui::failed_close(&self.app) {
            self.dispatch(ui::Message::ExitAnyway)
        } else {
            Task::none()
        }
    }
    fn update(&mut self, m: Message) -> Task<Message> {
        match m {
            Message::Tick => self.tick(),
            Message::Deadline => self.fail("fixture-deadline"),
            Message::Ui(ui::Message::DraftOutcome(outcome)) => {
                if matches!(&outcome, ui::DraftOutcome::Saved { .. }) {
                    self.count("savedOutcomes");
                }
                let task = self.dispatch(ui::Message::DraftOutcome(outcome));
                let finish = self.finish();
                Task::batch([task, finish])
            }
            Message::Ui(ui::Message::Worker(event)) => {
                if let worker::Event::Selected { frame, .. } = &event {
                    let active = match frame {
                        SessionFollowFrame::AssistantStream { .. } => true,
                        SessionFollowFrame::Event { event } => work_kind(&event.event_type),
                        SessionFollowFrame::Snapshot {
                            records,
                            has_more,
                            assistant_stream,
                            ..
                        } => {
                            let kinds: Vec<_> = records
                                .iter()
                                .map(|HistoryRecord::Event { event }| event.event_type.as_str())
                                .collect();
                            self.report["recordKinds"] = json!(kinds);
                            *has_more
                                || assistant_stream
                                    .as_ref()
                                    .is_some_and(|b| b.active_attempt.is_some())
                                || kinds.iter().any(|s| work_kind(s))
                        }
                    };
                    if active {
                        self.count("unexpectedWork");
                        return self.fail("recorded-agent-work");
                    }
                }
                if let worker::Event::Catalog { result, .. } = &event {
                    self.count("catalogReads");
                    self.report["catalogSucceeded"] = json!(result.is_ok());
                    if result.is_err() {
                        return self.fail("catalog-failed");
                    }
                }
                if let worker::Event::Created(Ok(summary)) = &event {
                    if self.mode == Mode::Recover
                        || self.phase != Phase::Selecting
                        || self.subject.is_some()
                    {
                        return self.fail("unexpected-created-session");
                    }
                    self.subject = Some(summary.session_id.clone());
                    self.report["sessionId"] = json!(summary.session_id.as_str());
                    if write_json(&self.output.join("session-manifest.json"),&json!({"version":1,"publicOnly":true,"sessionId":summary.session_id.as_str(),"firstPid":std::process::id(),"checkpoint":self.point.name()})).is_err(){return self.fail("session-manifest-write");}
                }
                if let worker::Event::Inspection(result) = &event {
                    self.report["inspectionObserved"] = json!(result.is_ok());
                }
                if let worker::Event::Stopped(result) = &event {
                    self.report["gracefulStopped"] = json!(result.as_ref().is_ok_and(|s| s.exited
                        && s.exit_code == Some(0)
                        && s.graceful
                        && !s.containment_unknown
                        && s.observed_descendants_remaining == 0));
                    if let Ok(stop) = result {
                        self.report["stop"] = json!({"exited":stop.exited,"graceful":stop.graceful,"containmentUnknown":stop.containment_unknown,"observedDescendantsRemaining":stop.observed_descendants_remaining,"exitCode":stop.exit_code});
                    }
                }
                let task = self.dispatch(ui::Message::Worker(event));
                let confirm = if ui::confirmation(&self.app) {
                    self.dispatch(ui::Message::ConfirmClose)
                } else {
                    Task::none()
                };
                let finish = self.finish();
                Task::batch([task, confirm, finish])
            }
            Message::Ui(m @ ui::Message::NativeInfo { .. }) => {
                if let ui::Message::NativeInfo { backend, .. } = &m {
                    self.native = *backend == "wayland";
                    self.report["windowBackend"] = json!(backend);
                }
                self.dispatch(m)
            }
            Message::Ui(
                m @ (ui::Message::NativeOpened(_)
                | ui::Message::Resized(_)
                | ui::Message::WindowActive(_)
                | ui::Message::MotionFrame(_)
                | ui::Message::WorkingTick
                | ui::Message::ScrollApplied { .. }
                | ui::Message::DraftFlush),
            ) => self.dispatch(m),
            Message::Ui(ui::Message::Close) => self.fail("manual-close"),
            Message::Ui(_) => Task::none(),
        }
    }
    fn view(&self) -> Element<'_, Message> {
        container(column![
            iced::widget::text(
                "PUBLIC native save interruption · NO Send · no physical input qualification"
            )
            .size(12),
            self.app.view().map(Message::Ui)
        ])
        .into()
    }
    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            self.app.subscription().map(Message::Ui),
            iced::time::every(Duration::from_millis(100)).map(|_| Message::Tick),
        ])
    }
}
fn work_kind(s: &str) -> bool {
    s.contains("llm")
        || [
            "step/start",
            "step/end",
            "request/header",
            "tool/call",
            "tool/result",
            "assistant/message",
            "user/message",
        ]
        .contains(&s)
}
fn main() -> iced::Result {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let (mut filtered, mut mode, mut point) = (vec![], None, None);
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--campaign-mode" || args[index] == "--checkpoint" {
            let key = &args[index];
            let value = args.get(index + 1).unwrap_or_else(|| {
                eprintln!("Missing campaign option");
                std::process::exit(2)
            });
            if key == "--campaign-mode" {
                mode = Some(match value.as_str() {
                    "hold" => Mode::Hold,
                    "crash" => Mode::Crash,
                    "recover" => Mode::Recover,
                    _ => {
                        eprintln!("Invalid campaign mode");
                        std::process::exit(2)
                    }
                })
            } else {
                point = Point::parse(value);
            }
            index += 2;
        } else {
            filtered.push(args[index].clone());
            index += 1;
        }
    }
    let (mode, point) = (
        mode.unwrap_or_else(|| {
            eprintln!("Explicit mode required");
            std::process::exit(2)
        }),
        point.unwrap_or_else(|| {
            eprintln!("Explicit checkpoint required");
            std::process::exit(2)
        }),
    );
    let options = config::parse(filtered)
        .unwrap_or_else(|_| {
            eprintln!("Invalid ordinary native options");
            std::process::exit(2)
        })
        .unwrap_or_else(|| std::process::exit(0));
    let (output, subject) = guard(&options, mode, point).unwrap_or_else(|s| {
        eprintln!("{s}");
        std::process::exit(2)
    });
    // No environment mutation after any Iced/Tokio/owner thread exists.
    unsafe {
        std::env::set_var("ICED_BACKEND", "wgpu");
    }
    let (handle, feed, owner) =
        worker::start(options.backend.clone(), false).unwrap_or_else(|_| {
            eprintln!("Owned worker unavailable");
            std::process::exit(1)
        });
    let scale = options.scale;
    let result = iced::application(
        move || {
            Fixture::boot(
                options.clone(),
                output.clone(),
                mode,
                point,
                subject.clone(),
                handle.clone(),
                feed.clone(),
            )
        },
        Fixture::update,
        Fixture::view,
    )
    .title("Harness PUBLIC draft interruption")
    .window(window::Settings {
        size: iced::Size::new(1000.0, 720.0),
        platform_specific: window::settings::PlatformSpecific {
            application_id: "ai.deepseek.harness.native.draft-interruption-fixture".into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .exit_on_close_request(false)
    .default_font(ui::FONT)
    .theme(|_: &Fixture| ui::native_theme())
    .scale_factor(move |_: &Fixture| scale)
    .subscription(Fixture::subscription)
    .run();
    drop(owner);
    result
}

#[cfg(test)]
mod interruption_gate_tests {
    use super::*;

    #[test]
    fn hold_is_bounded_but_crash_waits_for_owned_kill() {
        let bounded = GateWaitPolicy::ReleaseWithin(Duration::from_secs(25));
        assert_eq!(GateWaitPolicy::for_mode(Mode::Hold), bounded);
        assert_eq!(GateWaitPolicy::for_mode(Mode::Recover), bounded);
        assert_eq!(
            GateWaitPolicy::for_mode(Mode::Crash),
            GateWaitPolicy::UntilOwnedKill
        );
    }

    #[test]
    fn crash_has_no_release_sender_and_drop_never_marks_it_released() {
        // Inspect the policy/release path without invoking or spawning a permanent callback.
        let (mut gate, observer) = Gate::make(Point::BeforeWrite, Mode::Crash);
        assert!(gate.sender.is_none());
        assert!(!gate.latched_until_owned_kill());
        gate.reached.store(true, Ordering::SeqCst);
        gate.release();
        assert!(gate.latched_until_owned_kill());
        let released = gate.released.clone();
        drop(gate);
        assert!(!released.load(Ordering::SeqCst));
        drop(observer);
    }

    #[test]
    fn hold_release_before_callback_is_consumed_without_a_waiting_thread() {
        let (mut gate, observer) = Gate::make(Point::BeforeWrite, Mode::Hold);
        gate.release();
        assert!(observer(ui::SaveCheckpoint::BeforeTempWrite).is_ok());
        assert!(gate.reached.load(Ordering::SeqCst));
        assert!(gate.released.load(Ordering::SeqCst));
        assert!(!gate.latched_until_owned_kill());
    }
}
