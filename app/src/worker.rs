//! The worker owns Core, authenticated transport, subscriptions, and shutdown.
use dsh_native_core::{
    Backend, Command as CoreCommand, Inspection, PublicEvent, PublicReply, RustBackendOptions,
    StopResult,
};
use dsh_native_transport::{Limits, NativeStream, dto::*};
use std::{
    future::Future,
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use tokio::{
    sync::{mpsc, watch},
    task::{JoinHandle, JoinSet},
};

pub const TRANSPORT_EPOCH: u64 = 1;
const MAX_OPERATIONS: usize = 8;
#[path = "catalog_read.rs"]
pub(crate) mod catalog_read;
#[path = "codex_worker.rs"]
mod codex_worker;
#[cfg(test)]
#[path = "export_worker_tests.rs"]
mod export_worker_tests;
#[cfg(test)]
#[path = "mcp_worker_tests.rs"]
mod mcp_worker_tests;
#[cfg(test)]
#[path = "plugin_worker_tests.rs"]
mod plugin_worker_tests;
use codex_worker::{CodexCoordinator, codex_ticket, execute_codex};
#[path = "attachments.rs"]
pub(crate) mod attachments;
#[path = "cloudflare_worker.rs"]
mod cloudflare_worker;
#[path = "decision_worker.rs"]
mod decision_worker;
#[path = "file_intake.rs"]
pub(crate) mod file_intake;
#[path = "prompt_recovery.rs"]
pub(crate) mod prompt_recovery;
#[path = "prompt_worker.rs"]
mod prompt_worker;
use decision_worker::{Admission as DecisionAdmission, DecisionCoordinator};
pub enum Command {
    Account(crate::settings::account::Effect),
    Cloudflare(crate::settings::cloudflare::Effect),
    StageFile(attachments::Submission),
    DiscardFile(attachments::Ticket),
    PromptFile {
        tickets: Vec<attachments::Ticket>,
        request: SessionPromptRequest,
    },
    Codex(crate::codex::Effect),
    Export(crate::exporter::Submission),
    PluginsRead(crate::plugins::ReadTicket),
    /// Read the plugin manager's profile availability and installed bundles. Read-only.
    PluginManagerRead,
    /// Switch one bundle on or off after the page's explicit confirmation.
    PluginBundleEnabled {
        name: String,
        enabled: bool,
    },
    /// Remove one bundle after the page's explicit confirmation. Destructive.
    PluginRemove {
        name: String,
    },
    /// Read what one typed install spec names, before installing anything.
    PluginInspect {
        spec: String,
        /// The registry the read asks first; `None` is the Host's configured default.
        registry: Option<String>,
    },
    /// Read the registries an inspect or install would ask, in order.
    PluginRegistries,
    /// Read the live profile entries the manager lists (their enabled state and patch ids).
    PluginManagerPlugins,
    /// Switch one live plugin row on or off after the page's confirmation.
    PluginEntryEnabled {
        entry_id: String,
        enabled: bool,
    },
    /// Install one inspected spec after the page's confirmation.
    PluginInstall {
        spec: String,
        registry: Option<String>,
        request_id: String,
    },
    /// The active installation's result for one request id; a recovery read, not a retry.
    PluginWaitForInstall {
        request_id: String,
    },
    /// Ask the Host to stop one installation.
    PluginCancelInstall {
        request_id: String,
    },
    /// Re-read the session roster, whose summaries carry the projections a subagent catalog is built
    /// from. The reference refreshes a child's projection when its chevron is pressed
    /// (`SubagentHeaderLineage.tsx:554-561`); this is the same read, whole-roster.
    RosterRefresh,
    /// Read the Host's permission-preset catalog for the chip's menu.
    PermissionCatalogRead,
    /// Read the Host's agent-preset roster for the hero chip and the header label.
    AgentPresetsRead,
    /// Stage one advertised agent preset for a session that has not started its first turn.
    AgentPresetSelect {
        session: dsh_native_transport::dto::SessionId,
        preset: String,
    },
    /// Make one roster preset the default for sessions created later. The reply carries the
    /// settings view; the roster re-read is what confirms the change.
    AgentPresetDefault(String),
    /// Ask the Host to stop one background job.
    /// Open one job's live output stream; the panel sends this when a row expands.
    JobOutput {
        session: dsh_native_transport::dto::SessionId,
        job: String,
    },
    /// Mutate one pending queue item (edit, remove or steer).
    UpdateQueue(crate::queue_update::Submission),
    /// Read one turn's changed-file summary.
    ChangesSummary {
        session: dsh_native_transport::dto::SessionId,
        seq: u64,
    },
    /// List `@` file-reference candidates.
    FileReferences {
        session: dsh_native_transport::dto::SessionId,
        query: String,
    },
    /// Open one workspace path on the Host desktop.
    OpenWorkspacePath {
        path: String,
    },
    /// Read the Host's installed desktop applications for the workspace opener.
    OpenInAppApps,
    /// Launch one installed desktop app on one workspace directory.
    OpenInAppOpen {
        app: String,
        path: String,
    },
    /// Open the Host's native directory chooser.
    PickDirectory,
    /// List one directory level for the in-app browser.
    ListDirectory {
        path: Option<String>,
    },
    /// Write one rating, or remove it when the user clicks the active one.
    FeedbackPut {
        session: dsh_native_transport::dto::SessionId,
        message_id: String,
        rating: String,
        if_version: Option<String>,
        note: Option<String>,
        category: Option<String>,
    },
    /// Record one Session-level remark through `sessionFeedback/record`.
    SessionFeedbackRecord {
        session: dsh_native_transport::dto::SessionId,
        text: Option<String>,
        category: Option<String>,
    },
    /// Remove one message's feedback.
    FeedbackDelete {
        session: dsh_native_transport::dto::SessionId,
        message_id: String,
        version: String,
    },
    /// List `@` session-reference candidates.
    SessionReferences {
        session: dsh_native_transport::dto::SessionId,
        query: String,
    },
    /// Read one listed file's comparison.
    ChangesDiff {
        session: dsh_native_transport::dto::SessionId,
        seq: u64,
        index: usize,
    },
    /// Read the Host's whole schedule catalog.
    ScheduleCatalog,
    /// Delete one schedule inside its original session binding.
    ScheduleDelete {
        session: dsh_native_transport::dto::SessionId,
        id: String,
    },
    /// Read one delivery-history page for a schedule.
    ScheduleHistory {
        session: dsh_native_transport::dto::SessionId,
        id: String,
        limit: u64,
        before: Option<String>,
    },
    /// Compare-and-update one schedule's title and/or prompt.
    ScheduleUpdate {
        session: dsh_native_transport::dto::SessionId,
        expected: serde_json::Value,
        title: Option<String>,
        prompt: Option<String>,
        change: Option<serde_json::Value>,
    },
    /// One goal mutation for a session.
    Goal {
        session: dsh_native_transport::dto::SessionId,
        operation: crate::goal_ui::Operation,
    },
    /// Read one session's live goal activation. Activation is process-local and absent from
    /// the durable projection, so a session opened after its goal was disarmed has to ask.
    GoalActivationRead {
        session: dsh_native_transport::dto::SessionId,
    },
    /// Full-text search at one generation/serial fence.
    Search {
        generation: u64,
        serial: u64,
        query: String,
    },
    JobKill {
        session: dsh_native_transport::dto::SessionId,
        job: String,
    },
    /// Read the deployment's new-session permission default for the settings row.
    PermissionSettingsRead,
    /// Persist the new-session default preset at the descriptor's own revision.
    PermissionDefaultSet {
        name: String,
        expected_revision: u64,
    },
    PluginSave {
        ticket: crate::plugins::WriteTicket,
        /// Boxed alongside [`crate::plugins::Effect::Save`]: the namespace view
        /// carries every field of the namespace.
        namespace: Box<dsh_native_transport::plugin::SettingsNamespaceView>,
        field: dsh_native_transport::plugin::SettingsField,
        value: dsh_native_transport::plugin::SettingsScalar,
    },
    Manage(crate::management::Submission),
    Rename(crate::rename::Submission),
    /// Read the Host's advertised commands for one session. Read-only, and the
    /// composer asks for it lazily rather than on every selection.
    CommandsList(SessionId),
    /// Read the Session's skill catalog: the composer's second `/` source.
    SkillsList(SessionId),
    /// Invoke one Host command without a model turn. Paired with
    /// [`Event::CommandsExecuted`] by session, since no submission id exists.
    CommandsExecute {
        session_id: SessionId,
        line: String,
    },
    Fork(crate::fork::Submission),
    TerminalRead(crate::terminal::ReadTicket),
    TerminalFollow(crate::terminal::FollowTicket),
    /// Stop the live terminal follow this worker owns. No reply: the panel that asked for the
    /// stream has already closed and cleared its own copy.
    TerminalUnfollow,
    TerminalCreate(crate::terminal::CreateSubmission),
    TerminalClose(crate::terminal::CloseSubmission),
    TerminalRename(crate::terminal::RenameSubmission),
    TerminalResize(crate::terminal::ResizeSubmission),
    TerminalWrite(crate::terminal::WriteSubmission),
    /// Read the app-owned MCP patch file and the live Host MCP inventory.
    McpRead(crate::mcp::ReadTicket),
    /// Replace the app-owned MCP patch file. Shares the single management flight.
    McpWrite(crate::mcp::Submission),
    KeyMetadata(crate::settings::Ticket),
    SaveKey {
        ticket: crate::settings::Ticket,
        secret: dsh_native_core::SecretApiKey,
    },
    Decision(crate::interactions::Submission),
    Select {
        generation: u64,
        address: SessionAddress,
    },
    Create {
        workspace: Option<WorkspaceId>,
        cwd: Option<String>,
    },
    OpenWorkspace(String),
    Catalog(catalog_read::Ticket),
    SelectModel {
        generation: u64,
        request: SelectModelRequest,
    },
    Prompt {
        generation: u64,
        request: SessionPromptRequest,
    },
    CheckSubmission(crate::worker::prompt_recovery::CheckTicket),
    RejoinObservations {
        epoch: u64,
    },
    Cancel {
        generation: u64,
        session: SessionId,
    },
    Page {
        generation: u64,
        request: SessionPageRequest,
    },
    Inspect,
    Shutdown,
}
#[derive(Clone)]
pub enum Event {
    /// The Host's permission-preset catalog, read once when the chip's menu opens.
    PermissionCatalog(Result<dsh_native_core::PermissionCatalog, String>),
    /// Every declared agent preset, for the hero chip, the header label and the lineage.
    AgentPresets(Result<dsh_native_core::AgentPresetRoster, String>),
    /// The committed identity of an agent preset picked for a session that had not started.
    AgentPresetSelected(Result<String, String>),
    /// The outcome of making one preset the new-session default.
    AgentPresetDefault {
        preset: String,
        result: Result<(), String>,
    },
    /// A bounded status line, for an outcome that owns no state of its own.
    Notice(String),
    /// The receipt of a job kill: `requested` or `already-finished`.
    JobKilled(Result<dsh_native_transport::dto::JobKillOutcome, String>),
    QueueUpdated {
        submission: crate::queue_update::Submission,
        result: Result<(), crate::queue_update::Failure>,
    },
    Searched {
        generation: u64,
        serial: u64,
        result: Result<dsh_native_transport::dto::SessionSearchValue, String>,
    },
    GoalUpdated {
        session: dsh_native_transport::dto::SessionId,
        result: Result<(), String>,
    },
    ChangesLoaded {
        seq: u64,
        result: Result<dsh_native_transport::dto::ChangesSummary, String>,
    },
    ChangesDiffLoaded {
        seq: u64,
        index: usize,
        result: Result<dsh_native_transport::dto::ChangesDiff, String>,
    },
    FileReferencesLoaded {
        query: String,
        result: Result<Vec<dsh_native_transport::dto::FileReferenceCandidate>, String>,
    },
    SessionReferencesLoaded {
        query: String,
        result: Result<Vec<dsh_native_transport::dto::SessionReferenceCandidate>, String>,
    },
    WorkspacePathOpened {
        path: String,
        result: Result<bool, String>,
    },
    /// The installed-desktop catalog the workspace opener may offer.
    OpenInAppApps(Result<Vec<String>, String>),
    /// One launched application, acknowledged by the Host's `{ok:true}` receipt.
    OpenInAppLaunched {
        app: String,
        result: Result<(), String>,
    },
    DirectoryPicked(Result<Option<String>, String>),
    DirectoryListed(Result<dsh_native_transport::dto::DirectoryListing, String>),
    FeedbackWritten {
        session: dsh_native_transport::dto::SessionId,
        message_id: String,
        result: Result<dsh_native_transport::dto::MessageFeedbackPutOutcome, String>,
    },
    /// The answer to a session-level remark: recorded, no live session, or a named refusal.
    SessionFeedbackRecorded {
        session: dsh_native_transport::dto::SessionId,
        result: Result<dsh_native_transport::SessionFeedbackOutcome, String>,
    },
    FeedbackDeleted {
        session: dsh_native_transport::dto::SessionId,
        message_id: String,
        result: Result<bool, String>,
    },
    SchedulesLoaded(Result<Vec<dsh_native_transport::dto::ScheduleCatalogItem>, String>),
    ScheduleDeleted {
        session: dsh_native_transport::dto::SessionId,
        id: String,
        result: Result<dsh_native_transport::dto::ScheduleDeleteOutcome, String>,
    },
    ScheduleHistoryLoaded {
        session: dsh_native_transport::dto::SessionId,
        id: String,
        result: Result<dsh_native_transport::dto::ScheduleHistoryOutcome, String>,
    },
    ScheduleUpdated {
        session: dsh_native_transport::dto::SessionId,
        id: String,
        result: Result<dsh_native_transport::dto::ScheduleUpdateOutcome, String>,
    },
    /// The complete set of background jobs the session can see. Each list frame replaces
    /// the set wholesale (`api/job-controller/src/types.ts:19-27`).
    JobsRows(Vec<dsh_native_transport::dto::JobView>),
    /// One job's opening, output or status change (`:72-89`).
    /// A follow frame **with the job it belongs to**: an `output` frame carries no job id, and
    /// the stream is per job, so only the worker — which opened that stream — knows which row the
    /// bytes are for (`dto::JobFollowRequest { job_id }`).
    JobsFollow {
        job: String,
        frame: dsh_native_transport::dto::JobFollowFrame,
    },
    /// The deployment's new-session permission default, for the settings row.
    PermissionSettings(Result<dsh_native_core::PermissionSettingsState, String>),
    /// The acknowledgement of a new-session default write.
    PermissionDefaultSet(Result<(), String>),
    AccountCompleted {
        ticket: crate::settings::Ticket,
        outcome: crate::settings::account::Outcome,
    },
    CloudflareCompleted {
        ticket: crate::settings::Ticket,
        outcome: crate::settings::cloudflare::Outcome,
    },
    FileStaged {
        ticket: attachments::Ticket,
        outcome: attachments::Outcome,
    },
    FilePrompt {
        tickets: Vec<attachments::Ticket>,
        request_id: SessionRequestId,
        outcome: attachments::PromptOutcome,
    },
    CodexCompleted {
        ticket: crate::codex::Ticket,
        outcome: crate::codex::Outcome,
    },
    Exported {
        receipt: crate::exporter::Receipt,
        outcome: crate::exporter::Outcome,
    },
    /// The plugin manager's read: the Host's profile availability and, when manageable, its
    /// bundles. `Ok((false, []))` is the reference's own unavailable answer, not an empty profile.
    PluginManagerLoaded(Result<(bool, Vec<dsh_native_transport::plugin::PluginBundle>), String>),
    /// One bundle switch's settlement; the page re-reads the list from the Host after it.
    PluginBundleChanged {
        name: String,
        enabled: bool,
        result: Result<dsh_native_transport::plugin::PluginChange, String>,
    },
    /// One bundle removal's settlement; the page re-reads the list from the Host after it.
    PluginRemoved {
        name: String,
        result: Result<dsh_native_transport::plugin::PluginChange, String>,
    },
    /// One install-spec inspection's answer: accepted preview or a normal refusal.
    PluginInspected(Result<dsh_native_transport::plugin::PluginInspection, String>),
    /// The Host's registry order for the Add-plugin form.
    PluginRegistriesLoaded(Result<dsh_native_transport::plugin::PluginRegistries, String>),
    /// The live profile entries, for the rows each bundle declares.
    PluginManagerPluginsLoaded(Result<Vec<dsh_native_transport::plugin::PluginRow>, String>),
    /// One plugin-row switch's settlement; the page re-reads the rows from the Host after it.
    PluginEntryChanged {
        entry_id: String,
        enabled: bool,
        result: Result<dsh_native_transport::plugin::PluginChange, String>,
    },
    /// One installation's settlement; the page re-reads the list from the Host after it.
    PluginInstallSettled(Result<dsh_native_transport::plugin::PluginChange, String>),
    /// A recovery read's answer: the active result, or `None` when the Host no longer has it.
    PluginWaitForInstall(Result<Option<dsh_native_transport::plugin::PluginChange>, String>),
    /// The Host's answer to a cancellation request.
    PluginInstallCancelAsked(Result<dsh_native_transport::plugin::PluginCancellation, String>),
    PluginsLoaded {
        ticket: crate::plugins::ReadTicket,
        result: Result<crate::plugins::Snapshot, crate::plugins::ReadFailure>,
    },
    PluginSaved {
        ticket: crate::plugins::WriteTicket,
        result: crate::plugins::SaveOutcome,
    },
    Managed {
        submission: crate::management::Submission,
        result: Result<(), ()>,
    },
    Renamed {
        submission: crate::rename::Submission,
        result: Result<SessionRenameValue, ()>,
    },
    /// The session's advertised commands, or a failure that names its kind so the
    /// composer can say whether the method is missing, refused, or unreadable.
    CommandsListed {
        session_id: SessionId,
        result: Result<Vec<CommandDescriptor>, CommandListFailureOutcome>,
    },
    /// The session's skill catalog, or the failure that explains why it is absent.
    SkillsListed {
        session_id: SessionId,
        result: Result<Vec<dsh_native_transport::dto::SkillDescriptor>, ()>,
    },
    /// One command's Host-owned outcome. `Option` is meaningful: the Host may
    /// settle a command with no execution record, which is not a success. The
    /// line travels back so a reply for a command the app is no longer waiting
    /// on cannot be reported as the current one.
    CommandsExecuted {
        session_id: SessionId,
        line: String,
        result: Result<Option<CommandExecution>, ()>,
    },
    Forked {
        submission: crate::fork::Submission,
        result: Result<SessionForkValue, crate::fork::Failure>,
    },
    TerminalLoaded {
        ticket: crate::terminal::ReadTicket,
        result: Result<crate::terminal::Loaded, crate::terminal::ReadFailure>,
    },
    TerminalFrame {
        ticket: crate::terminal::FollowTicket,
        frame: dsh_native_transport::terminal::TerminalFrame,
    },
    TerminalFollowEnded {
        ticket: crate::terminal::FollowTicket,
        failure: Option<crate::terminal::ReadFailure>,
    },
    TerminalCreated {
        submission: crate::terminal::CreateSubmission,
        result:
            Result<dsh_native_transport::terminal::WebTerminalInfo, crate::terminal::CreateFailure>,
    },
    TerminalResized {
        submission: crate::terminal::ResizeSubmission,
        result: Result<(), crate::terminal::ResizeOutcome>,
    },
    TerminalRenamed {
        submission: crate::terminal::RenameSubmission,
        result: Result<(), crate::terminal::RenameOutcome>,
    },
    TerminalClosed {
        submission: crate::terminal::CloseSubmission,
        result: Result<(), crate::terminal::CloseFailure>,
    },
    TerminalWritten {
        submission: crate::terminal::WriteSubmission,
        result: Result<(), crate::terminal::WriteFailure>,
    },
    /// One answer to [`Command::McpRead`]: the app-owned document and the
    /// Host's live MCP inventory. There is no failure type: the local read and
    /// the inventory read each report their own reason.
    McpLoaded {
        ticket: crate::mcp::ReadTicket,
        loaded: crate::mcp::Loaded,
    },
    /// One answer to [`Command::McpWrite`]. `NotSent` is a refusal before any
    /// file operation; `Indeterminate` may already have replaced the file.
    McpWritten {
        submission: crate::mcp::Submission,
        outcome: crate::mcp::WriteOutcome,
    },
    KeyMetadata {
        ticket: crate::settings::Ticket,
        result: Result<dsh_native_core::OnboardingMetadata, ()>,
    },
    KeySaved {
        ticket: crate::settings::Ticket,
        result: crate::settings::SaveResult,
    },
    Ready(String),
    DecisionResult {
        submission: crate::interactions::Submission,
        result: Result<(), String>,
    },
    Core(PublicEvent),
    Roster(Result<SessionListValue, String>),
    Workspace(WorkspaceFollowFrame),
    /// One Host-computed projection push (`session/control`, `ControlFrame`).
    Control(dsh_native_transport::dto::ControlFrame),
    /// One session's live goal activation, or `None` when it has no active goal.
    GoalActivationRead {
        session: dsh_native_transport::dto::SessionId,
        result: Result<Option<dsh_native_transport::dto::GoalView>, String>,
    },
    Root {
        frame: RemoteEventFrame,
        decision_key: Option<crate::interactions::Key>,
    },
    Selected {
        generation: u64,
        frame: SessionFollowFrame,
    },
    Created(Result<SessionCreateValue, String>),
    WorkspaceOpened(Result<WorkspaceCreateValue, String>),
    Catalog {
        ticket: catalog_read::Ticket,
        result: Result<ModelCatalog, String>,
    },
    Model {
        generation: u64,
        result: Result<SelectModelValue, String>,
    },
    Prompt {
        generation: u64,
        request_id: SessionRequestId,
        result: Result<Accepted, String>,
    },
    PromptNotSent {
        ticket: crate::worker::prompt_recovery::Ticket,
    },
    SubmissionChecked {
        ticket: crate::worker::prompt_recovery::CheckTicket,
        value: Option<dsh_native_transport::prompt_admission::PromptAdmission>,
    },
    Cancel {
        generation: u64,
        result: Result<Accepted, String>,
    },
    Page {
        generation: u64,
        result: Result<SessionPage, String>,
    },
    FollowError {
        generation: u64,
        error: String,
    },
    /// A job panel stream failed; unlike a session-follow loss this must not gate sending.
    JobsError {
        generation: u64,
        error: String,
    },
    Inspection(Result<Inspection, String>),
    Fault(String),
    Stopped(Result<StopResult, String>),
    NoBackendStarted,
    ObservationsLost,
    ObservationsRejoined {
        epoch: u64,
        result: Result<(), String>,
    },
}
impl std::fmt::Debug for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeWorkerEvent(redacted)")
    }
}
#[derive(Clone)]
pub struct Handle {
    commands: mpsc::Sender<Command>,
    priority: mpsc::Sender<Command>,
    shutdown: watch::Sender<bool>,
}
impl std::fmt::Debug for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeWorkerHandle(redacted)")
    }
}
impl Handle {
    /// Returning the value that could not be sent is the [`mpsc::Sender::send`]
    /// idiom this mirrors, so the error type is [`Command`] itself (240 bytes).
    /// Boxing it in the error alone would not help: the value that fails is the
    /// caller's own command, which it may retry, keep or drop.
    #[allow(clippy::result_large_err)]
    pub fn send(&self, command: Command) -> Result<(), Command> {
        if matches!(command, Command::Shutdown) {
            self.shutdown.send_replace(true);
            return Ok(());
        }
        let lane = if matches!(
            &command,
            Command::Cloudflare(crate::settings::cloudflare::Effect::CancelLogin { .. })
        ) || prompt_worker::priority(&command)
        {
            &self.priority
        } else {
            &self.commands
        };
        lane.try_send(command).map_err(|e| e.into_inner())
    }
}
#[derive(Clone)]
pub struct Feed {
    receiver: Arc<Mutex<Option<mpsc::Receiver<Event>>>>,
}
impl Hash for Feed {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.receiver) as usize).hash(state);
    }
}
pub fn stream(
    feed: &Feed,
) -> std::pin::Pin<Box<dyn futures_util::Stream<Item = Event> + Send + 'static>> {
    let receiver = feed.receiver.lock().ok().and_then(|mut r| r.take());
    Box::pin(futures_util::stream::unfold(
        (receiver, false),
        |(mut receiver, terminal)| async move {
            if let Some(event) = match receiver.as_mut() {
                Some(receiver) => receiver.recv().await,
                None => None,
            } {
                let terminal = matches!(&event, Event::Stopped(_) | Event::NoBackendStarted);
                return Some((event, (receiver, terminal)));
            }
            if terminal {
                None
            } else {
                Some((
                    Event::Stopped(Err(
                        "Native worker channel closed unexpectedly; Core stop result unknown"
                            .into(),
                    )),
                    (None, true),
                ))
            }
        },
    ))
}
/// One monotonic lifecycle signal bypasses the bounded business queue in every phase.
/// Retained outside Iced: even an application error waits for its owner thread.
pub struct Owner {
    shutdown: watch::Sender<bool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.shutdown.send_replace(true);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
pub fn start(options: RustBackendOptions, smoke: bool) -> Result<(Handle, Feed, Owner), String> {
    let (commands, receiver) = mpsc::channel(32);
    let (priority, priority_receiver) = mpsc::channel(2);
    let (events, feed) = mpsc::channel(32);
    let (shutdown, closing) = watch::channel(false);
    let thread = thread::Builder::new()
        .name("native-backend-owner".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
            {
                Ok(r) => r,
                Err(_) => {
                    let _ =
                        events.try_send(Event::Fault("Native worker runtime unavailable".into()));
                    let _ = events.try_send(Event::NoBackendStarted);
                    return;
                }
            };
            runtime.block_on(run(
                options,
                receiver,
                priority_receiver,
                events,
                closing,
                smoke,
            ));
        })
        .map_err(|_| "Native worker thread unavailable".to_owned())?;
    Ok((
        Handle {
            commands,
            priority,
            shutdown: shutdown.clone(),
        },
        Feed {
            receiver: Arc::new(Mutex::new(Some(feed))),
        },
        Owner {
            shutdown,
            thread: Some(thread),
        },
    ))
}
fn publish(events: &mpsc::Sender<Event>, event: Event) -> bool {
    events.try_send(event).is_ok()
}
async fn operation<F: Future>(
    closing: &mut watch::Receiver<bool>,
    events: &mpsc::Sender<Event>,
    future: F,
) -> Option<F::Output> {
    if *closing.borrow() || events.is_closed() {
        return None;
    }
    tokio::select! {biased;_=closing.changed()=>None,_=events.closed()=>None,result=future=>Some(result)}
}
async fn finish_core(backend: Arc<Backend>, events: &mpsc::Sender<Event>, fault: Option<String>) {
    let result = backend.stop().map_err(|e| format!("Core stop: {e}"));
    drop(backend);
    if let Some(fault) = fault {
        let _ = events.send(Event::Fault(fault)).await;
    }
    let _ = events.send(Event::Stopped(result)).await;
}
fn forward_core(
    receiver: std::sync::mpsc::Receiver<PublicEvent>,
    sender: mpsc::Sender<PublicEvent>,
) {
    while let Ok(event) = receiver.recv() {
        let exited = matches!(&event, PublicEvent::Exited(_));
        if sender.blocking_send(event).is_err() || exited {
            break;
        }
    }
}
async fn next_root(
    root: &mut Option<NativeStream<RemoteEventFrame>>,
) -> Option<dsh_native_transport::Result<dsh_native_transport::OwnedRemoteEvent>> {
    match root {
        Some(root) => root.next_event().await,
        None => std::future::pending().await,
    }
}
async fn next_workspaces(
    workspaces: &mut Option<NativeStream<WorkspaceFollowFrame>>,
) -> Option<dsh_native_transport::Result<WorkspaceFollowFrame>> {
    match workspaces {
        Some(workspaces) => workspaces.next().await,
        None => std::future::pending().await,
    }
}
async fn next_control(
    control: &mut Option<NativeStream<dsh_native_transport::dto::ControlFrame>>,
) -> Option<dsh_native_transport::Result<dsh_native_transport::dto::ControlFrame>> {
    match control {
        Some(control) => control.next().await,
        None => std::future::pending().await,
    }
}
async fn drop_observations(
    root: &mut Option<NativeStream<RemoteEventFrame>>,
    workspaces: &mut Option<NativeStream<WorkspaceFollowFrame>>,
    control: &mut Option<NativeStream<dsh_native_transport::dto::ControlFrame>>,
    selected: &mut Option<NativeStream<SessionFollowFrame>>,
    terminal: &mut Option<NativeStream<dsh_native_transport::terminal::TerminalFrame>>,
    mux: &mut Option<dsh_native_transport::Mux>,
) {
    if let Some(mut stream) = root.take() {
        stream.cancel().await;
    }
    if let Some(mut stream) = workspaces.take() {
        stream.cancel().await;
    }
    if let Some(mut stream) = control.take() {
        stream.cancel().await;
    }
    if let Some(mut stream) = selected.take() {
        stream.cancel().await;
    }
    if let Some(mut stream) = terminal.take() {
        stream.cancel().await;
    }
    if let Some(mux) = mux.take() {
        mux.close().await;
    }
}
async fn next_selected(
    stream: &mut Option<NativeStream<SessionFollowFrame>>,
) -> Option<dsh_native_transport::Result<SessionFollowFrame>> {
    match stream {
        Some(stream) => stream.next().await,
        None => std::future::pending().await,
    }
}
async fn next_jobs(
    stream: &mut Option<NativeStream<dsh_native_transport::dto::JobListFrame>>,
) -> Option<dsh_native_transport::Result<dsh_native_transport::dto::JobListFrame>> {
    match stream {
        Some(stream) => stream.next().await,
        None => std::future::pending().await,
    }
}

async fn next_job_output(
    stream: &mut Option<NativeStream<dsh_native_transport::dto::JobFollowFrame>>,
) -> Option<dsh_native_transport::Result<dsh_native_transport::dto::JobFollowFrame>> {
    match stream {
        Some(stream) => stream.next().await,
        None => std::future::pending().await,
    }
}
async fn next_terminal(
    stream: &mut Option<NativeStream<dsh_native_transport::terminal::TerminalFrame>>,
) -> Option<dsh_native_transport::Result<dsh_native_transport::terminal::TerminalFrame>> {
    match stream {
        Some(stream) => stream.next().await,
        None => std::future::pending().await,
    }
}
/// One account effect against the Host. Nothing here reads a token, and the
/// destination stays in the backend: the browser effect only asks for the
/// attempt's one-use capability to be spent.
fn execute_account(
    backend: &Backend,
    effect: crate::settings::account::Effect,
) -> crate::settings::account::Outcome {
    use crate::settings::account::{Effect, Outcome};
    let timeout = Duration::from_secs(10);
    let result = match effect {
        Effect::Subscribe { .. } => {
            return match backend.request(CoreCommand::AccountSubscribe, timeout) {
                Ok(PublicReply::Account(metadata)) => Outcome::Observed(metadata),
                _ => Outcome::Unavailable,
            };
        }
        Effect::Start { .. } | Effect::SignOut { .. } => {
            let (locale, timezone_offset_seconds) = dsh_native_core::account_identity();
            let command = match effect {
                Effect::Start { .. } => CoreCommand::AccountStart {
                    locale,
                    timezone_offset_seconds,
                },
                _ => CoreCommand::AccountSignOut {
                    locale,
                    timezone_offset_seconds,
                },
            };
            backend.request(command, timeout)
        }
        Effect::Cancel { attempt_id, .. } => {
            backend.request(CoreCommand::AccountCancel { attempt_id }, timeout)
        }
        Effect::OpenBrowser {
            attempt_id, permit, ..
        } => {
            return match backend.open_account_browser(&attempt_id, &permit) {
                Ok(()) => Outcome::BrowserOpened,
                Err(_) => Outcome::Unknown,
            };
        }
    };
    match result {
        Ok(PublicReply::Account(metadata)) => Outcome::Observed(metadata),
        Ok(_) => Outcome::Unknown,
        Err(_) => Outcome::Unknown,
    }
}
fn execute_cloudflare(
    backend: &Backend,
    effect: crate::settings::cloudflare::Effect,
) -> crate::settings::cloudflare::Outcome {
    use crate::settings::cloudflare::{Effect, Outcome};
    let timeout = Duration::from_secs(10);
    let result = match effect {
        Effect::AuthMode { revision, mode, .. } => backend
            .request(
                CoreCommand::CloudflareAuthModeSave {
                    expected_revision: revision,
                    auth_mode: mode,
                },
                timeout,
            )
            .and_then(cloudflare_reply)
            .and_then(|snapshot| {
                if snapshot.auth_status.auth_mode == mode
                    && !snapshot.models_enabled
                    && !snapshot.browser_enabled
                    && (revision..=revision + 1).contains(&snapshot.revision)
                {
                    Ok(snapshot)
                } else {
                    Err(dsh_native_core::Error::Protocol)
                }
            }),
        Effect::Setup {
            revision, setup, ..
        } => {
            let expected = setup.clone();
            backend
                .request(
                    CoreCommand::CloudflareSetupSave {
                        expected_revision: revision,
                        setup,
                    },
                    timeout,
                )
                .and_then(cloudflare_reply)
                .and_then(|snapshot| {
                    if snapshot.oauth_setup == expected
                        && snapshot.auth_status.auth_mode
                            == dsh_native_core::CloudflareAuthMode::Oauth
                        && !snapshot.models_enabled
                        && !snapshot.browser_enabled
                        && (revision..=revision + 1).contains(&snapshot.revision)
                    {
                        Ok(snapshot)
                    } else {
                        Err(dsh_native_core::Error::Protocol)
                    }
                })
        }
        Effect::Start {
            revision, permit, ..
        } => {
            if !permit.load(std::sync::atomic::Ordering::Acquire) {
                return Outcome::NotSent;
            }
            backend.start_cloudflare(revision, &permit, timeout)
        }
        Effect::OpenBrowser {
            attempt, permit, ..
        } => {
            return match backend.open_cloudflare_browser(&attempt, &permit) {
                Ok(()) => Outcome::BrowserOpened,
                Err(_) => Outcome::Unknown,
            };
        }
        Effect::CancelLogin { attempt, .. } => {
            backend.revoke_cloudflare_browser();
            let Some(attempt_id) = attempt else {
                return Outcome::NotSent;
            };
            backend
                .request(CoreCommand::CloudflareOauthCancel { attempt_id }, timeout)
                .and_then(cloudflare_reply)
        }
        Effect::Disconnect {
            revision, revoke, ..
        } => backend
            .request(
                CoreCommand::CloudflareOauthDisconnect {
                    expected_revision: revision,
                    revoke,
                },
                timeout,
            )
            .and_then(cloudflare_reply),
        Effect::Read(_) => backend
            .request(CoreCommand::CloudflareRead, timeout)
            .and_then(|reply| match reply {
                PublicReply::Cloudflare(value) => Ok(value),
                _ => Err(dsh_native_core::Error::Protocol),
            }),
        Effect::Save {
            revision,
            account,
            models,
            browser,
            free,
            token,
            ..
        } => backend.save_cloudflare(
            revision,
            &account,
            dsh_native_core::CloudflareSaveChoice {
                models_enabled: models,
                browser_enabled: browser,
                free_plan_confirmed: free,
            },
            token,
            timeout,
        ),
        Effect::Disable { revision, .. } => backend
            .request(
                CoreCommand::CloudflareDisable {
                    expected_revision: revision,
                },
                timeout,
            )
            .and_then(|reply| match reply {
                PublicReply::Cloudflare(value)
                    if !value.models_enabled
                        && !value.browser_enabled
                        && (revision..=revision + 1).contains(&value.revision) =>
                {
                    Ok(value)
                }
                _ => Err(dsh_native_core::Error::Protocol),
            }),
    };
    match result {
        Ok(snapshot) => Outcome::Snapshot(snapshot),
        Err(
            dsh_native_core::Error::InvalidCommand
            | dsh_native_core::Error::Starting
            | dsh_native_core::Error::Stopping
            | dsh_native_core::Error::WriterBusy
            | dsh_native_core::Error::TooManyRequests
            | dsh_native_core::Error::CommandTooLarge,
        ) => Outcome::NotSent,
        Err(_) => Outcome::Unknown,
    }
}
fn cloudflare_reply(
    reply: PublicReply,
) -> Result<dsh_native_core::CloudflareSnapshot, dsh_native_core::Error> {
    match reply {
        PublicReply::Cloudflare(value) => Ok(value),
        _ => Err(dsh_native_core::Error::Protocol),
    }
}
fn rejected(command: Command) -> Event {
    let error = "Native concurrent operation limit reached; retry explicitly".to_owned();
    match command {
        // A read-only catalog costs nothing to refuse, and the menu keeps the design set.
        Command::RosterRefresh => Event::Roster(Err(error.clone())),
        Command::PermissionCatalogRead => Event::PermissionCatalog(Err(error)),
        Command::PermissionSettingsRead => Event::PermissionSettings(Err(error)),
        Command::AgentPresetsRead => Event::AgentPresets(Err(error)),
        Command::AgentPresetSelect { .. } => Event::AgentPresetSelected(Err(error)),
        Command::AgentPresetDefault(preset) => Event::AgentPresetDefault {
            preset,
            result: Err(error),
        },
        Command::JobKill { .. } => Event::JobKilled(Err(error)),
        Command::ChangesSummary { seq, .. } => Event::ChangesLoaded {
            seq,
            result: Err(error),
        },
        Command::FileReferences { query, .. } => Event::FileReferencesLoaded {
            query,
            result: Err(error),
        },
        Command::SessionReferences { query, .. } => Event::SessionReferencesLoaded {
            query,
            result: Err(error),
        },
        Command::PickDirectory => Event::DirectoryPicked(Err(error)),
        Command::ListDirectory { .. } => Event::DirectoryListed(Err(error)),
        Command::FeedbackPut {
            session,
            message_id,
            ..
        } => Event::FeedbackWritten {
            session,
            message_id,
            result: Err(error),
        },
        Command::SessionFeedbackRecord { session, .. } => Event::SessionFeedbackRecorded {
            session,
            result: Err(error),
        },
        Command::FeedbackDelete {
            session,
            message_id,
            ..
        } => Event::FeedbackDeleted {
            session,
            message_id,
            result: Err(error),
        },
        Command::OpenWorkspacePath { path } => Event::WorkspacePathOpened {
            path,
            result: Err(error),
        },
        Command::OpenInAppApps => Event::OpenInAppApps(Err(error)),
        Command::OpenInAppOpen { app, .. } => Event::OpenInAppLaunched {
            app,
            result: Err(error),
        },
        Command::ChangesDiff { seq, index, .. } => Event::ChangesDiffLoaded {
            seq,
            index,
            result: Err(error),
        },
        Command::ScheduleCatalog => Event::SchedulesLoaded(Err(error)),
        Command::ScheduleDelete { session, id } => Event::ScheduleDeleted {
            session,
            id,
            result: Err(error),
        },
        Command::ScheduleUpdate { session, .. } => Event::ScheduleUpdated {
            session,
            id: String::new(),
            result: Err(error),
        },
        Command::ScheduleHistory { session, id, .. } => Event::ScheduleHistoryLoaded {
            session,
            id,
            result: Err(error),
        },
        Command::Goal { session, .. } => Event::GoalUpdated {
            session,
            result: Err(error),
        },
        Command::GoalActivationRead { session } => Event::GoalActivationRead {
            session,
            result: Err(error),
        },
        Command::Search {
            generation, serial, ..
        } => Event::Searched {
            generation,
            serial,
            result: Err(error),
        },
        Command::UpdateQueue(submission) => Event::QueueUpdated {
            submission,
            result: Err(crate::queue_update::Failure::NotSent),
        },
        Command::JobOutput { .. } => Event::Fault("Invalid native command state".into()),
        Command::PermissionDefaultSet { .. } => Event::PermissionDefaultSet(Err(error)),
        Command::Account(effect) => {
            let ticket = effect.ticket();
            drop(effect);
            Event::AccountCompleted {
                ticket,
                outcome: crate::settings::account::Outcome::NotSent,
            }
        }
        Command::Cloudflare(effect) => {
            let ticket = effect.ticket();
            drop(effect);
            Event::CloudflareCompleted {
                ticket,
                outcome: crate::settings::cloudflare::Outcome::NotSent,
            }
        }
        Command::StageFile(submission) => Event::FileStaged {
            ticket: submission.ticket,
            outcome: attachments::Outcome::NotSent,
        },
        Command::PromptFile { tickets, request } => Event::FilePrompt {
            tickets,
            request_id: request.request_id,
            outcome: attachments::PromptOutcome::NotSent,
        },
        Command::DiscardFile(ticket) => Event::FileStaged {
            ticket,
            outcome: attachments::Outcome::NotSent,
        },
        Command::Codex(effect) => Event::CodexCompleted {
            ticket: codex_ticket(&effect),
            outcome: crate::codex::Outcome::NotSent,
        },
        Command::Export(submission) => {
            let (receipt, destination) = submission.into_parts();
            drop(destination);
            Event::Exported {
                receipt,
                outcome: crate::exporter::Outcome::NotSent,
            }
        }
        Command::PluginManagerRead => Event::PluginManagerLoaded(Err(error)),
        Command::PluginBundleEnabled { name, enabled } => Event::PluginBundleChanged {
            name,
            enabled,
            result: Err(error),
        },
        Command::PluginRemove { name } => Event::PluginRemoved {
            name,
            result: Err(error),
        },
        Command::PluginInspect { .. } => Event::PluginInspected(Err(error)),
        Command::PluginRegistries => Event::PluginRegistriesLoaded(Err(error)),
        Command::PluginManagerPlugins => Event::PluginManagerPluginsLoaded(Err(error)),
        Command::PluginEntryEnabled { entry_id, enabled } => Event::PluginEntryChanged {
            entry_id,
            enabled,
            result: Err(error),
        },
        Command::PluginInstall { .. } => Event::PluginInstallSettled(Err(error)),
        Command::PluginWaitForInstall { .. } => Event::PluginWaitForInstall(Err(error)),
        Command::PluginCancelInstall { .. } => Event::PluginInstallCancelAsked(Err(error)),
        Command::PluginsRead(ticket) => Event::PluginsLoaded {
            ticket,
            result: Err(crate::plugins::ReadFailure::Unavailable),
        },
        Command::PluginSave { ticket, .. } => Event::PluginSaved {
            ticket,
            result: crate::plugins::SaveOutcome::NotSent,
        },
        Command::Manage(submission) => Event::Managed {
            submission,
            result: Err(()),
        },
        Command::Rename(submission) => Event::Renamed {
            submission,
            result: Err(()),
        },
        Command::CommandsList(session_id) => Event::CommandsListed {
            session_id,
            result: Err(CommandListFailureOutcome::NotAttempted),
        },
        Command::SkillsList(session_id) => Event::SkillsListed {
            session_id,
            result: Err(()),
        },
        Command::CommandsExecute { session_id, line } => Event::CommandsExecuted {
            session_id,
            line,
            result: Err(()),
        },
        Command::Fork(submission) => Event::Forked {
            submission,
            result: Err(crate::fork::Failure::Unknown),
        },
        Command::KeyMetadata(ticket) => Event::KeyMetadata {
            ticket,
            result: Err(()),
        },
        Command::SaveKey { ticket, secret } => {
            drop(secret);
            Event::KeySaved {
                ticket,
                result: crate::settings::SaveResult::NotSent,
            }
        }
        Command::Decision(submission) => Event::DecisionResult {
            submission,
            result: Err(error),
        },
        Command::Create { .. } => Event::Created(Err(error)),
        Command::OpenWorkspace(_) => Event::WorkspaceOpened(Err(error)),
        Command::Catalog(ticket) => Event::Catalog {
            ticket,
            result: Err(error),
        },
        Command::SelectModel { generation, .. } => Event::Model {
            generation,
            result: Err(error),
        },
        Command::Prompt {
            generation,
            request,
        } => Event::PromptNotSent {
            ticket: prompt_worker::ticket(generation, &request),
        },
        Command::CheckSubmission(ticket) => Event::SubmissionChecked {
            ticket,
            value: None,
        },
        Command::RejoinObservations { epoch } => Event::ObservationsRejoined {
            epoch,
            result: Err("Observation rejoin not admitted".into()),
        },
        Command::Cancel { generation, .. } => Event::Cancel {
            generation,
            result: Err(error),
        },
        Command::Page { generation, .. } => Event::Page {
            generation,
            result: Err(error),
        },
        Command::Inspect => Event::Inspection(Err(error)),
        Command::Select { generation, .. } => Event::FollowError { generation, error },
        Command::TerminalRead(ticket) => Event::TerminalLoaded {
            ticket,
            result: Err(crate::terminal::ReadFailure::Rejected),
        },
        Command::TerminalFollow(ticket) => Event::TerminalFollowEnded {
            ticket,
            failure: Some(crate::terminal::ReadFailure::Rejected),
        },
        Command::TerminalUnfollow => Event::Notice(error),
        Command::TerminalCreate(submission) => Event::TerminalCreated {
            submission,
            result: Err(crate::terminal::CreateFailure::NotSent),
        },
        Command::TerminalRename(submission) => Event::TerminalRenamed {
            submission,
            result: Err(crate::terminal::RenameOutcome::NotSent),
        },
        Command::TerminalResize(submission) => Event::TerminalResized {
            submission,
            result: Err(crate::terminal::ResizeOutcome::NotSent),
        },
        Command::TerminalClose(submission) => Event::TerminalClosed {
            submission,
            result: Err(crate::terminal::CloseFailure::NotSent),
        },
        Command::TerminalWrite(submission) => Event::TerminalWritten {
            submission,
            result: Err(crate::terminal::WriteFailure::NotSent),
        },
        Command::McpRead(ticket) => Event::McpLoaded {
            ticket,
            loaded: crate::mcp::Loaded::refused(
                "The native worker refused the MCP read before it ran.",
            ),
        },
        Command::McpWrite(submission) => Event::McpWritten {
            submission,
            outcome: crate::mcp::WriteOutcome::NotSent,
        },
        Command::Shutdown => Event::Fault(error),
    }
}
async fn catalog_read_outcome<F, R, S>(
    ticket: catalog_read::Ticket,
    smoke: bool,
    stopping: S,
    rpc: F,
) -> Event
where
    F: FnOnce() -> R,
    R: Future<Output = Result<ModelCatalog, String>>,
    S: FnOnce() -> bool,
{
    // Recheck live shutdown/delivery when first polled, not at task construction.
    // This boundary check cannot un-send a read that has already started.
    let result = if ticket.epoch() != TRANSPORT_EPOCH || smoke || stopping() {
        Err(
            "Model catalog read not admitted: stale transport or disabled read; no request sent"
                .into(),
        )
    } else {
        rpc().await
    };
    Event::Catalog { ticket, result }
}
fn file_receiver(header: &SessionWireHeader, selected: Option<&SessionId>) -> Option<SessionId> {
    // Header lineage is not Agent ownership. This is a supported ordinary-header scope, not a live lease.
    (header.version == 4 && Some(&header.id) == selected && header.origin.is_none())
        .then(|| header.id.clone())
}
fn file_canceled(
    local: &watch::Receiver<bool>,
    closing: &watch::Receiver<bool>,
    events: &mpsc::Sender<Event>,
) -> bool {
    *local.borrow()
        || local.has_changed().is_err()
        || *closing.borrow()
        || closing.has_changed().is_err()
        || events.is_closed()
}
async fn file_prompt_outcome<F, E>(
    closing: &mut watch::Receiver<bool>,
    events: &mpsc::Sender<Event>,
    rpc: F,
) -> attachments::PromptOutcome
where
    F: std::future::Future<Output = Result<Accepted, E>>,
{
    if *closing.borrow() || events.is_closed() {
        return attachments::PromptOutcome::Unknown;
    }
    tokio::select! {biased;
        _=closing.changed()=>attachments::PromptOutcome::Unknown,
        _=events.closed()=>attachments::PromptOutcome::Unknown,
        result=rpc=>match result {Ok(value) if value.accepted=>attachments::PromptOutcome::Accepted,_=>attachments::PromptOutcome::Unknown},
    }
}
fn read_and_stage_file(
    client: dsh_native_transport::NativeClient,
    submission: attachments::Submission,
    mut cancellation: watch::Receiver<bool>,
    mut closing: watch::Receiver<bool>,
    events: mpsc::Sender<Event>,
    runtime: tokio::runtime::Handle,
) -> attachments::Completion {
    let ticket = submission.ticket;
    let canceled = || file_canceled(&cancellation, &closing, &events);
    let result = match submission.source.read(canceled) {
        Err(error) => Err(attachments::Failure::Read(error)),
        Ok(file) if canceled() => { drop(file); Err(attachments::Failure::Read(file_intake::Failure::Canceled)) },
        Ok(file) => runtime.block_on(async {
            tokio::select! {
                biased;
                _ = cancellation.changed() => Err(attachments::Failure::Upload),
                _ = closing.changed() => Err(attachments::Failure::Upload),
                _ = events.closed() => Err(attachments::Failure::Upload),
                result = client.upload_file(&ticket.target, file) => result.map_err(|_error| attachments::Failure::Upload),
            }
        }),
    };
    attachments::Completion { ticket, result }
}
fn decision_guard(epoch: u64, smoke: bool) -> Result<(), String> {
    if smoke {
        return Err("Decision replies disabled during real keyless smoke; no reply sent".into());
    }
    if epoch != TRANSPORT_EPOCH {
        return Err("Stale native transport generation; no reply sent".into());
    }
    Ok(())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PluginOperation {
    Read(crate::plugins::ReadTicket),
    Save(crate::plugins::WriteTicket),
}
impl PluginOperation {
    fn epoch(self) -> u64 {
        match self {
            Self::Read(ticket) => ticket.epoch,
            Self::Save(ticket) => ticket.epoch,
        }
    }
}
fn admit_plugin(
    active: &mut Option<PluginOperation>,
    operation: PluginOperation,
    smoke: bool,
    stopping: bool,
    tasks: usize,
    key_saving: bool,
) -> bool {
    if active.is_some()
        || smoke
        || stopping
        || key_saving
        || tasks >= MAX_OPERATIONS
        || operation.epoch() != TRANSPORT_EPOCH
    {
        return false;
    }
    *active = Some(operation);
    true
}
fn settle_plugin(active: &mut Option<PluginOperation>, event: &Event) {
    let completed = match event {
        Event::PluginsLoaded { ticket, .. } => Some(PluginOperation::Read(*ticket)),
        Event::PluginSaved { ticket, .. } => Some(PluginOperation::Save(*ticket)),
        _ => None,
    };
    if active.is_some() && *active == completed {
        *active = None;
    }
}
fn plugin_read_failure(error: dsh_native_transport::Error) -> crate::plugins::ReadFailure {
    use dsh_native_transport::Error;
    match error {
        Error::SettingsRejected | Error::Authentication => crate::plugins::ReadFailure::Refused,
        Error::InvalidDto | Error::InvalidJson | Error::Oversize | Error::Correlation => {
            crate::plugins::ReadFailure::Malformed
        }
        _ => crate::plugins::ReadFailure::Unavailable,
    }
}
async fn read_plugins(
    client: dsh_native_transport::NativeClient,
    ticket: crate::plugins::ReadTicket,
) -> Event {
    let result = match tokio::try_join!(client.plugin_inventory(), client.plugin_settings()) {
        Ok((inventory, settings)) => Ok(crate::plugins::Snapshot {
            inventory,
            settings,
        }),
        Err(error) => Err(plugin_read_failure(error)),
    };
    Event::PluginsLoaded { ticket, result }
}
fn plugin_save_outcome(
    result: dsh_native_transport::Result<dsh_native_transport::plugin::SettingsNamespaceView>,
) -> crate::plugins::SaveOutcome {
    use crate::plugins::SaveOutcome;
    match result {
        Ok(namespace) => SaveOutcome::Confirmed(namespace),
        Err(dsh_native_transport::Error::SettingsConflict) => SaveOutcome::Conflict,
        Err(dsh_native_transport::Error::SettingsRejected) => SaveOutcome::Refused,
        Err(_) => SaveOutcome::Indeterminate,
    }
}
async fn save_plugin(
    client: dsh_native_transport::NativeClient,
    ticket: crate::plugins::WriteTicket,
    namespace: dsh_native_transport::plugin::SettingsNamespaceView,
    field: dsh_native_transport::plugin::SettingsField,
    value: dsh_native_transport::plugin::SettingsScalar,
) -> Event {
    use crate::plugins::SaveOutcome;
    let result = if ticket.expected_revision != namespace.revision {
        SaveOutcome::NotSent
    } else {
        plugin_save_outcome(client.set_plugin_field(&namespace, &field, value).await)
    };
    Event::PluginSaved { ticket, result }
}
fn key_guard(ticket: crate::settings::Ticket, smoke: bool) -> bool {
    !smoke && ticket.epoch == TRANSPORT_EPOCH
}
fn key_save_guard(
    ticket: crate::settings::Ticket,
    smoke: bool,
    key_saving: bool,
    plugins_active: bool,
    export_active: bool,
) -> bool {
    !key_saving && !plugins_active && !export_active && key_guard(ticket, smoke)
}
fn key_metadata(backend: &Backend) -> Result<dsh_native_core::OnboardingMetadata, ()> {
    match backend.request(CoreCommand::OnboardingRead, Duration::from_secs(5)) {
        Ok(PublicReply::Onboarding(meta)) => Ok(meta),
        _ => Err(()),
    }
}
fn save_key(
    backend: &Backend,
    ticket: crate::settings::Ticket,
    secret: dsh_native_core::SecretApiKey,
    smoke: bool,
) -> Event {
    let result =
        if !key_guard(ticket, smoke) || !key_metadata(backend).is_ok_and(|meta| meta.writable) {
            drop(secret);
            crate::settings::SaveResult::NotSent
        } else {
            crate::settings::SaveResult::from_core(
                backend.save_api_key(secret, Duration::from_secs(10)),
            )
        };
    Event::KeySaved { ticket, result }
}
/// Reserve one registry mutation without activating an Agent or replaying an operation.
fn admit_management(
    active: &mut Option<crate::management::Submission>,
    submission: &crate::management::Submission,
    admission: &Admission,
) -> bool {
    if admission.refused() || active.is_some() || submission.ticket.epoch != TRANSPORT_EPOCH {
        return false;
    }
    *active = Some(submission.clone());
    true
}
fn settle_management(active: &mut Option<crate::management::Submission>, event: &Event) {
    if let Event::Managed { submission, .. } = event
        && active.as_ref() == Some(submission)
    {
        *active = None;
    }
}
/// Reserve one title rename without activating an Agent. Exactly one session-management
/// write (registry mutation, rename, fork, terminal, MCP composition or export)
/// may be in flight.
/// Which of the six single-flight commands the worker currently holds. Exactly one
/// registry mutation, rename, fork, terminal create/close, MCP composition write or
/// export may be in flight: each refuses while *any* flight is held, including its
/// own slot, so no call site has to restate the peers it must not run beside.
#[derive(Clone, Copy, Default)]
struct Flights {
    management: bool,
    rename: bool,
    fork: bool,
    export: bool,
    terminal: bool,
    mcp: bool,
}

impl Flights {
    /// True when any flight is held.
    fn any(self) -> bool {
        self.management || self.rename || self.fork || self.export || self.terminal || self.mcp
    }
}

/// Commands that change Host or app-owned state. While observation is lost or an
/// admitted submission is still busy, none of them may start and each is answered
/// with its own refusal.
///
/// The reads, `DiscardFile` and `Command::McpWrite` are deliberately absent and
/// still run. An MCP write needs neither the observation stream nor a submission
/// slot: it rewrites the app-owned patch file through its own reviewed,
/// atomic, backed-up replacement, and the Host reads that file at its next start
/// rather than during the running turn this fence protects.
fn mutates_host_state(command: &Command) -> bool {
    match command {
        Command::Prompt { .. }
        | Command::PromptFile { .. }
        | Command::StageFile(_)
        | Command::SaveKey { .. }
        | Command::PluginSave { .. }
        | Command::Export(_)
        | Command::Manage(_)
        | Command::Rename(_)
        | Command::Fork(_)
        | Command::TerminalCreate(_)
        | Command::TerminalClose(_)
        | Command::TerminalWrite(_)
        | Command::SelectModel { .. }
        | Command::Create { .. }
        | Command::UpdateQueue(_)
        | Command::JobKill { .. }
        | Command::AgentPresetDefault(_)
        | Command::OpenWorkspace(_) => true,
        Command::Account(effect) => matches!(
            effect,
            crate::settings::account::Effect::Start { .. }
                | crate::settings::account::Effect::Cancel { .. }
                | crate::settings::account::Effect::SignOut { .. }
        ),
        Command::Cloudflare(effect) => matches!(
            effect,
            crate::settings::cloudflare::Effect::Save { .. }
                | crate::settings::cloudflare::Effect::Disable { .. }
                | crate::settings::cloudflare::Effect::AuthMode { .. }
                | crate::settings::cloudflare::Effect::Setup { .. }
                | crate::settings::cloudflare::Effect::Start { .. }
                | crate::settings::cloudflare::Effect::Disconnect { .. }
        ),
        Command::Codex(effect) => matches!(
            effect,
            crate::codex::Effect::RuntimeSave { .. }
                | crate::codex::Effect::UsageSave { .. }
                | crate::codex::Effect::StartLogin { .. }
                | crate::codex::Effect::AddAccount { .. }
                | crate::codex::Effect::SelectAccount { .. }
                | crate::codex::Effect::SignOutAccount { .. }
                | crate::codex::Effect::SignOutAll { .. }
                | crate::codex::Effect::EnableModels { .. }
                | crate::codex::Effect::SubmitCallback { .. }
        ),
        _ => false,
    }
}

/// One admission question for that flight: the worker's own lifecycle fences, the
/// flights already held, and any extra gate the command adds. A refusal answers the
/// caller with that command's own `NotSent` outcome and reserves nothing.
struct Admission {
    smoke: bool,
    stopping: bool,
    operations: usize,
    flights: Flights,
    extra: bool,
}

impl Admission {
    fn new(smoke: bool, stopping: bool, operations: usize, flights: Flights) -> Self {
        Self {
            smoke,
            stopping,
            operations,
            flights,
            extra: false,
        }
    }
    /// A gate this command has beyond the shared flight, such as a key save or a
    /// plugin write already in progress.
    fn with_extra(self, extra: bool) -> Self {
        Self { extra, ..self }
    }
    /// True when this command must not be admitted.
    fn refused(&self) -> bool {
        self.smoke
            || self.stopping
            || self.extra
            || self.flights.any()
            || self.operations >= MAX_OPERATIONS
    }
}

fn admit_rename(
    active: &mut Option<crate::rename::Submission>,
    submission: &crate::rename::Submission,
    admission: &Admission,
) -> bool {
    if admission.refused() || active.is_some() || submission.receipt.ticket.epoch != TRANSPORT_EPOCH
    {
        return false;
    }
    *active = Some(submission.clone());
    true
}
/// A failed read: which defect, and the transport error that proves it. The error
/// is kept so the composer can report its code or status without exposing any
/// provider message.
#[derive(Clone, Debug)]
pub enum CommandListFailureOutcome {
    /// The worker refused the read before sending it.
    NotAttempted,
    Failed {
        kind: CommandListFailure,
        error: dsh_native_transport::Error,
    },
}

impl CommandListFailureOutcome {
    /// The sentence the status line shows.
    pub fn message(&self) -> String {
        match self {
            Self::NotAttempted => {
                "No command list was requested from the Host; slash completion is unavailable"
                    .to_owned()
            }
            Self::Failed { kind, error } => kind.message(error),
        }
    }
}
/// Why the command advertisement could not be read. Each case is a different
/// defect, so the composer reports which one instead of one generic sentence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandListFailure {
    /// The method is absent from this Host's remote surface.
    MethodMissing,
    /// The Host answered with something this build cannot read.
    Unreadable,
    /// The gateway could not resolve the session's context, so nothing is
    /// advertised for it. A different defect from an unreadable reply.
    ContextMissing,
    /// Authentication, transport or correlation failed.
    Unreachable,
}

impl CommandListFailure {
    fn of(error: &dsh_native_transport::Error) -> Self {
        use dsh_native_transport::Error as E;
        match error {
            E::HttpStatus(404) => Self::MethodMissing,
            E::RemoteCode("gateway/context-not-found" | "gateway/context-unavailable") => {
                Self::ContextMissing
            }
            E::InvalidJson | E::InvalidDto => Self::Unreadable,
            _ => Self::Unreachable,
        }
    }
    /// The sentence the status line shows. Names the defect and, where the
    /// transport reported one, the code that proves it -- a code or status number
    /// carries no session content, unlike a provider message.
    pub fn message(self, error: &dsh_native_transport::Error) -> String {
        let code = match error {
            dsh_native_transport::Error::HttpStatus(status) => format!("http {status}"),
            other => other.code().to_owned(),
        };
        // `RemoteCode` already *is* the Host's code, so it is reported verbatim.
        let reason = match self {
            Self::MethodMissing => "this Host does not expose the command list",
            Self::Unreadable => "the Host's command list could not be read",
            Self::ContextMissing => "this session has no agent context the Host can read",
            Self::Unreachable => "no command list from the Host",
        };
        format!("{reason} ({code}); slash completion is unavailable")
    }
}
/// Read the Host's advertised commands. Descriptions travel too: the composer
/// menu shows them, and they are the Host's own words rather than ours.
async fn commands_list(client: dsh_native_transport::NativeClient, session_id: SessionId) -> Event {
    let result = client.commands_list(&session_id).await.map_err(|error| {
        CommandListFailureOutcome::Failed {
            kind: CommandListFailure::of(&error),
            error,
        }
    });
    Event::CommandsListed { session_id, result }
}
/// Invoke one command through the Host. The outcome of the command is the Host's
/// own; this worker neither interprets it nor retries it.
async fn commands_execute(
    client: dsh_native_transport::NativeClient,
    session_id: SessionId,
    line: String,
) -> Event {
    let request = dsh_native_transport::dto::CommandExecuteRequest {
        session_id: session_id.clone(),
        line: line.clone(),
        submitted_attachments: Vec::new(),
    };
    let result = client.commands_execute(&request).await.map_err(|_| ());
    Event::CommandsExecuted {
        session_id,
        line,
        result,
    }
}
fn settle_rename(active: &mut Option<crate::rename::Submission>, event: &Event) {
    if let Event::Renamed { submission, .. } = event
        && active.as_ref() == Some(submission)
    {
        *active = None;
    }
}
async fn rename(
    client: dsh_native_transport::NativeClient,
    submission: crate::rename::Submission,
) -> Event {
    // The request carries only the operator target and draft title; the returned
    // normalized value is the only acknowledged title.
    let request = SessionRenameRequest {
        session_id: submission.receipt.ticket.target.clone(),
        title: submission.title().to_owned(),
    };
    let result = client.session_rename(&request).await.map_err(|_| ());
    Event::Renamed { submission, result }
}
/// Reserve one Session fork without activating an Agent. A fork creates a Session, so it
/// shares the same single management/rename/export flight.
fn admit_fork(
    active: &mut Option<crate::fork::Submission>,
    submission: &crate::fork::Submission,
    admission: &Admission,
) -> bool {
    if admission.refused() || active.is_some() || submission.receipt.ticket.epoch != TRANSPORT_EPOCH
    {
        return false;
    }
    *active = Some(submission.clone());
    true
}
fn settle_fork(active: &mut Option<crate::fork::Submission>, event: &Event) {
    if let Event::Forked { submission, .. } = event
        && active.as_ref() == Some(submission)
    {
        *active = None;
    }
}
async fn fork(
    client: dsh_native_transport::NativeClient,
    submission: crate::fork::Submission,
) -> Event {
    use dsh_native_transport::Error;
    // The request carries the reviewed source and its optional inclusive cut; an omitted
    // atSeq selects the latest completed-turn prefix. Only the ACK child id may become
    // visible state.
    let request = SessionForkRequest {
        session_id: submission.receipt.ticket.target.clone(),
        at_seq: submission.cut(),
    };
    let result = client
        .session_fork(&request)
        .await
        .map_err(|error| match error {
            Error::ForkUnavailable => crate::fork::Failure::Unavailable,
            Error::ForkAttachFailed => crate::fork::Failure::Unattached,
            _ => crate::fork::Failure::Unknown,
        });
    Event::Forked { submission, result }
}
/// Read the Host terminal list, shell profiles and limits for one panel open.
/// A read never activates an Agent and never creates a process.
async fn terminal_read(
    client: dsh_native_transport::NativeClient,
    ticket: crate::terminal::ReadTicket,
) -> Event {
    use crate::terminal::ReadFailure;
    let list = match client.terminal_list(&ticket.target).await {
        Ok(list) => list,
        Err(_) => {
            return Event::TerminalLoaded {
                ticket,
                result: Err(ReadFailure::Rejected),
            };
        }
    };
    // Shell profiles and limits are best-effort: a Session with no registered Agent
    // still has Host-confirmed history, but create and write stay unavailable.
    let shells = client.terminal_shells(&ticket.target).await;
    let environment = client.terminal_environment(&ticket.target).await.ok();
    Event::TerminalLoaded {
        ticket,
        result: Ok(crate::terminal::Loaded {
            list,
            shells: shells.as_deref().unwrap_or_default().to_vec(),
            // A failed read is not an empty discovery: the panel says which one it saw.
            shells_failed: shells.is_err(),
            environment,
        }),
    }
}
/// Ask the Host to allocate the reviewed terminal. Only the ACK identity is authority.
async fn terminal_create(
    client: dsh_native_transport::NativeClient,
    submission: crate::terminal::CreateSubmission,
) -> Event {
    use crate::terminal::CreateFailure;
    use dsh_native_transport::Error;
    let request = dsh_native_transport::terminal::TerminalCreateRequest {
        shell_path: submission.shell_path.clone(),
        id: submission.receipt.id.clone(),
        cols: submission.cols,
        rows: submission.rows,
    };
    let result = client
        .terminal_create(&submission.receipt.ticket.target, &request)
        .await
        .map_err(|error| match error {
            Error::TerminalLimitReached => CreateFailure::LimitReached,
            Error::TerminalUnavailable => CreateFailure::Unavailable,
            _ => CreateFailure::Unknown,
        });
    Event::TerminalCreated { submission, result }
}
/// Ask the Host to close the reviewed terminal. The app owns no process teardown.
async fn terminal_resize(
    client: dsh_native_transport::NativeClient,
    submission: crate::terminal::ResizeSubmission,
) -> Event {
    use crate::terminal::ResizeOutcome;
    use dsh_native_transport::Error;
    let result = client
        .terminal_resize(
            &submission.receipt.ticket.target,
            &submission.receipt.id,
            &submission.receipt.attachment,
            submission.receipt.cols,
            submission.receipt.rows,
        )
        .await
        .map_err(|error| match error {
            Error::TerminalUnavailable => ResizeOutcome::Unavailable,
            _ => ResizeOutcome::Unknown,
        });
    Event::TerminalResized { submission, result }
}
async fn terminal_rename(
    client: dsh_native_transport::NativeClient,
    submission: crate::terminal::RenameSubmission,
) -> Event {
    use crate::terminal::RenameOutcome;
    use dsh_native_transport::Error;
    let result = client
        .terminal_rename(
            &submission.receipt.ticket.target,
            &submission.receipt.id,
            submission.title(),
        )
        .await
        .map_err(|error| match error {
            Error::TerminalUnavailable => RenameOutcome::Unavailable,
            _ => RenameOutcome::Unknown,
        });
    Event::TerminalRenamed { submission, result }
}
async fn terminal_close(
    client: dsh_native_transport::NativeClient,
    submission: crate::terminal::CloseSubmission,
) -> Event {
    use crate::terminal::CloseFailure;
    use dsh_native_transport::Error;
    let result = client
        .terminal_close(&submission.receipt.ticket.target, &submission.receipt.id)
        .await
        .map_err(|error| match error {
            Error::TerminalUnavailable => CloseFailure::Unavailable,
            _ => CloseFailure::Unknown,
        });
    Event::TerminalClosed { submission, result }
}
/// Deliver one admitted input write to the exact Host attachment.
async fn terminal_write(
    client: dsh_native_transport::NativeClient,
    submission: crate::terminal::WriteSubmission,
) -> Event {
    use crate::terminal::WriteFailure;
    use dsh_native_transport::Error;
    let result = client
        .terminal_write(
            &submission.receipt.ticket.target,
            &submission.receipt.id,
            &submission.receipt.attachment,
            submission.data(),
        )
        .await
        .map_err(|error| match error {
            Error::TerminalControlUnavailable => WriteFailure::ControlUnavailable,
            Error::TerminalUnavailable => WriteFailure::Unavailable,
            _ => WriteFailure::Unknown,
        });
    Event::TerminalWritten { submission, result }
}
/// One terminal management write (create or close). Only one may own the process.
#[derive(Clone, Debug, PartialEq, Eq)]
enum TerminalFlight {
    Create(crate::terminal::CreateSubmission),
    Close(crate::terminal::CloseSubmission),
    Rename(crate::terminal::RenameReceipt),
    Resize(crate::terminal::ResizeReceipt),
}
/// Reserve the terminal flight, which shares the single session-management flight.
fn admit_terminal(
    active: &mut Option<TerminalFlight>,
    flight: &TerminalFlight,
    admission: &Admission,
) -> bool {
    if admission.refused() || active.is_some() {
        return false;
    }
    *active = Some(flight.clone());
    true
}
fn settle_terminal(active: &mut Option<TerminalFlight>, event: &Event) {
    let flight = match event {
        Event::TerminalCreated { submission, .. } => TerminalFlight::Create(submission.clone()),
        Event::TerminalRenamed { submission, .. } => {
            TerminalFlight::Rename(submission.receipt.clone())
        }
        Event::TerminalClosed { submission, .. } => TerminalFlight::Close(submission.clone()),
        _ => return,
    };
    if active.as_ref() == Some(&flight) {
        *active = None;
    }
}
fn settle_terminal_write(active: &mut Option<crate::terminal::WriteReceipt>, event: &Event) {
    if let Event::TerminalWritten { submission, .. } = event
        && active.as_ref() == Some(&submission.receipt)
    {
        *active = None;
    }
}
/// Reserve the app-owned MCP composition write, which shares the single
/// management flight: it may not run beside a registry mutation, rename, fork,
/// terminal operation or export, and it obeys the same smoke, shutdown and
/// in-flight limits.
fn admit_mcp(
    active: &mut Option<crate::mcp::WriteTicket>,
    ticket: &crate::mcp::WriteTicket,
    admission: &Admission,
) -> bool {
    if admission.refused() || active.is_some() || ticket.epoch != TRANSPORT_EPOCH {
        return false;
    }
    *active = Some(*ticket);
    true
}
fn settle_mcp(active: &mut Option<crate::mcp::WriteTicket>, event: &Event) {
    if let Event::McpWritten { submission, .. } = event
        && active.as_ref() == Some(&submission.ticket)
    {
        *active = None;
    }
}
/// Read the app-owned patch file locally and the Host's MCP inventory over the
/// transport. The inventory is advisory: a failed read still returns the file.
async fn read_mcp(
    client: dsh_native_transport::NativeClient,
    home: std::path::PathBuf,
    ticket: crate::mcp::ReadTicket,
) -> Event {
    let (inventory, inventory_read) = match client.plugin_inventory().await {
        Ok(snapshot) => (crate::mcp::inventory(&snapshot.entries), true),
        Err(_) => (Vec::new(), false),
    };
    Event::McpLoaded {
        ticket,
        loaded: crate::mcp::load(&home, inventory, inventory_read),
    }
}
/// Replace the app-owned patch file. Blocking file work runs off the async
/// runtime and returns its own submission, so the receipt is never inferred.
async fn write_mcp(home: std::path::PathBuf, submission: crate::mcp::Submission) -> Event {
    let joined = tokio::task::spawn_blocking(move || {
        let outcome = crate::mcp::commit(&home, submission.ticket.expected, &submission.entries);
        (submission, outcome)
    })
    .await;
    match joined {
        Ok((submission, outcome)) => Event::McpWritten {
            submission,
            outcome,
        },
        Err(_) => Event::Fault("Native MCP composition worker failed".into()),
    }
}
async fn manage(
    client: dsh_native_transport::NativeClient,
    submission: crate::management::Submission,
) -> Event {
    use crate::management::Operation;
    // The request type has no stopActivity field. Returned registry sets are
    // deliberately discarded: workspace follow remains authoritative.
    let request = SessionRegistryRequest {
        session_id: submission.ticket.target.clone(),
    };
    let result = match submission.operation {
        Operation::Pin => client.pin_session(&request).await.map(|_| ()),
        Operation::Unpin => client.unpin_session(&request).await.map(|_| ()),
        Operation::Archive => client.archive_session(&request).await.map(|_| ()),
        Operation::Restore => client.unarchive_session(&request).await.map(|_| ()),
    }
    .map_err(|_| ());
    Event::Managed { submission, result }
}
type ExportDownload = JoinHandle<
    Result<
        (
            crate::export_file::Destination,
            dsh_native_transport::export::SessionArchive,
        ),
        crate::exporter::Outcome,
    >,
>;

fn operation_count(tasks: usize, export_active: bool) -> usize {
    tasks.saturating_add(usize::from(export_active))
}
fn selected_export_target(address: &SessionAddress) -> Option<SessionId> {
    match address {
        SessionAddress::Session { session_id } => Some(session_id.clone()),
        // Ordinary GUI session export does not grant lineage/Subagent/Agent authority.
        SessionAddress::Subagent { .. } => None,
    }
}
/// The Session whose transcript an address displays: the child for a subagent. Followers that
/// scope to the visible transcript (the jobs list) use this, never the export target, which is
/// intentionally absent for a subagent.
fn followed_session(address: &SessionAddress) -> &SessionId {
    match address {
        SessionAddress::Session { session_id } => session_id,
        SessionAddress::Subagent {
            child_session_id, ..
        } => child_session_id,
    }
}
fn admit_export(
    active: &mut Option<crate::exporter::Receipt>,
    receipt: &crate::exporter::Receipt,
    admission: &Admission,
    generation: u64,
    target: Option<&SessionId>,
) -> bool {
    if admission.refused()
        || active.is_some()
        || receipt.ticket.epoch != TRANSPORT_EPOCH
        || receipt.ticket.generation != generation
        || target != Some(&receipt.ticket.target)
    {
        return false;
    }
    *active = Some(receipt.clone());
    true
}
fn settle_export(active: &mut Option<crate::exporter::Receipt>, event: &Event) {
    if let Event::Exported { receipt, .. } = event
        && active.as_ref() == Some(receipt)
    {
        *active = None;
    }
}
async fn download_export(
    client: dsh_native_transport::NativeClient,
    destination: crate::export_file::Destination,
    target: SessionId,
) -> Result<
    (
        crate::export_file::Destination,
        dsh_native_transport::export::SessionArchive,
    ),
    crate::exporter::Outcome,
> {
    let archive = client
        .export_session(&target)
        .await
        .map_err(|_| crate::exporter::Outcome::DownloadFailed)?;
    Ok((destination, archive))
}
async fn next_download<T>(
    download: &mut Option<JoinHandle<T>>,
) -> Result<T, tokio::task::JoinError> {
    match download {
        Some(download) => download.await,
        None => std::future::pending().await,
    }
}
async fn cancel_download<T>(download: &mut Option<JoinHandle<T>>) {
    if let Some(download) = download.take() {
        download.abort();
        let _ = download.await;
    }
}
fn download_completion<T>(
    result: Result<Result<T, crate::exporter::Outcome>, tokio::task::JoinError>,
) -> Result<T, crate::exporter::Outcome> {
    result.unwrap_or(Err(crate::exporter::Outcome::DownloadFailed))
}
fn save_completion(
    receipt: crate::exporter::Receipt,
    result: Result<Event, tokio::task::JoinError>,
) -> Event {
    result.unwrap_or(Event::Exported {
        receipt,
        outcome: crate::exporter::Outcome::MayRemain,
    })
}
async fn drain_saves<T: 'static>(saves: &mut JoinSet<T>) {
    while saves.join_next().await.is_some() {}
}
fn save_can_start(closing: &watch::Receiver<bool>, events: &mpsc::Sender<Event>) -> bool {
    !*closing.borrow() && closing.has_changed().is_ok() && !events.is_closed()
}
fn spawn_owned_save<T: Send + 'static>(
    saves: &mut JoinSet<T>,
    closing: &watch::Receiver<bool>,
    events: &mpsc::Sender<Event>,
    save: impl FnOnce() -> T + Send + 'static,
) -> bool {
    if !save_can_start(closing, events) {
        return false;
    }
    saves.spawn_blocking(save);
    true
}
fn save_export(
    receipt: crate::exporter::Receipt,
    destination: crate::export_file::Destination,
    archive: dsh_native_transport::export::SessionArchive,
) -> Event {
    use crate::{export_file::Failure, exporter::Outcome};
    let outcome = match destination.prepare() {
        Ok(destination) => match destination.save(archive) {
            Ok(saved) => Outcome::Saved { bytes: saved.bytes },
            Err(Failure::InvalidPath | Failure::NotCreated) => Outcome::NotCreated,
            Err(Failure::MayRemain) => Outcome::MayRemain,
        },
        Err(_) => Outcome::NotCreated,
    };
    Event::Exported { receipt, outcome }
}

// `select!` expands each branch into one expression, and rustfmt leaves the
// resulting `if …{}if …{}` on one line because it cannot meaningfully break
// inside the macro. The lint reads that as a missing `else`; there is none.
#[allow(clippy::possible_missing_else)]
async fn run(
    options: RustBackendOptions,
    mut commands: mpsc::Receiver<Command>,
    mut priority: mpsc::Receiver<Command>,
    events: mpsc::Sender<Event>,
    mut closing: watch::Receiver<bool>,
    smoke: bool,
) {
    // The private native home is the only location the MCP panel may rewrite, and
    // Core consumes `options` below, so the path is captured here.
    let mcp_home = options.native_home.clone();
    if *closing.borrow() || events.is_closed() {
        let _ = publish(&events, Event::NoBackendStarted);
        return;
    }
    let (backend, core_events) = match Backend::start(options) {
        Ok(value) => value,
        Err(e) => {
            let _ = publish(&events, Event::Fault(format!("Core start: {e}")));
            let _ = publish(
                &events,
                Event::Stopped(Err("Core did not start; inspect startup error".into())),
            );
            return;
        }
    };
    let backend = Arc::new(backend);
    let ready_backend = backend.clone();
    let mut ready_task = tokio::task::spawn_blocking(move || ready_backend.wait_ready());
    let ready = match operation(&mut closing, &events, &mut ready_task).await {
        Some(Ok(Ok(ready))) => ready,
        Some(Ok(Err(e))) => {
            finish_core(backend, &events, Some(format!("Core ready: {e}"))).await;
            return;
        }
        Some(Err(_)) => {
            finish_core(
                backend,
                &events,
                Some("Core readiness worker failed".into()),
            )
            .await;
            return;
        }
        None => {
            let result = backend.stop().map_err(|e| format!("Core stop: {e}"));
            let _ = ready_task.await;
            let _ = events.send(Event::Stopped(result)).await;
            return;
        }
    };
    let limits = Limits {
        max_result_bytes: 4 * 1024 * 1024,
        queue_bytes: 1024 * 1024,
        max_streams: 8,
        max_concurrent_rpc: 4,
        max_pending_events: 64,
        max_json_items: 100_000,
        timeout: Duration::from_secs(15),
    };
    let mut client = match operation(&mut closing, &events, backend.connect_transport(limits)).await
    {
        Some(Ok(client)) => client,
        Some(Err(e)) => {
            finish_core(backend, &events, Some(format!("Core transport: {e}"))).await;
            return;
        }
        None => {
            finish_core(backend, &events, None).await;
            return;
        }
    };
    let mux = match operation(&mut closing, &events, client.connect_mux()).await {
        Some(Ok(mux)) => mux,
        Some(Err(e)) => {
            client.close().await;
            finish_core(backend, &events, Some(format!("Native mux: {e}"))).await;
            return;
        }
        None => {
            client.close().await;
            finish_core(backend, &events, None).await;
            return;
        }
    };
    let root = operation(&mut closing, &events, mux.events()).await;
    let workspaces = operation(&mut closing, &events, mux.workspace_follow()).await;
    // The control stream is the only channel the Host pushes projections on. A
    // goal mutation, a retitle or any other projection changes nothing on screen
    // without it, because a follow snapshot is read once when a session opens.
    let control = operation(&mut closing, &events, mux.control()).await;
    let (root, workspaces, control) = match (root, workspaces, control) {
        (Some(Ok(root)), Some(Ok(workspaces)), Some(Ok(control))) => (root, workspaces, control),
        _ => {
            mux.close().await;
            client.close().await;
            let fault = (!*closing.borrow() && !events.is_closed())
                .then(|| "Native root subscriptions failed".into());
            finish_core(backend, &events, fault).await;
            return;
        }
    };
    let mut mux = Some(mux);
    let mut root = Some(root);
    let mut workspaces = Some(workspaces);
    let mut control = Some(control);
    let mut observations_live = true;
    let mut live_decisions = Vec::new();
    let (core_tx, mut core_rx) = mpsc::channel(32);
    let forwarder = thread::spawn(move || forward_core(core_events, core_tx));
    let mut tasks = JoinSet::new();
    let mut stop_tasks = JoinSet::new();
    let mut rejoin_jobs = JoinSet::new();
    let mut submissions = prompt_worker::Coordinator::default();
    let roster = client.clone();
    tasks.spawn(async move {
        Event::Roster(
            roster
                .session_list()
                .await
                .map_err(|e| format!("Session list: {e}")),
        )
    });
    let _ = publish(&events, Event::Ready(ready.version));
    let mut selected: Option<NativeStream<SessionFollowFrame>> = None;
    let mut selected_target: Option<SessionId> = None;
    // The selected session's background jobs; the list stream is whole-set replacement and
    // the follow stream updates one job at a time.
    let mut jobs: Option<NativeStream<dsh_native_transport::dto::JobListFrame>> = None;
    // The per-job output stream: opened by the command arm below, because an `output` frame
    // carries no job id and only this loop knows which stream it read.
    let mut job_output: Option<NativeStream<dsh_native_transport::dto::JobFollowFrame>> = None;
    let mut job_output_for: Option<String> = None;
    let mut cancel_target: Option<SessionId> = None;
    // File intake needs a correlated, supported ordinary header, not just a Session address.
    let mut selected_file_target: Option<SessionId> = None;
    let mut generation = 0;
    let mut key_saving: Option<crate::settings::Ticket> = None;
    let mut cloudflare = cloudflare_worker::Coordinator::default();
    let mut plugins_active: Option<PluginOperation> = None;
    let mut codex = CodexCoordinator::default();
    let mut management_active: Option<crate::management::Submission> = None;
    let mut rename_active: Option<crate::rename::Submission> = None;
    let mut fork_active: Option<crate::fork::Submission> = None;
    let mut terminal_active: Option<TerminalFlight> = None;
    let mut mcp_active: Option<crate::mcp::WriteTicket> = None;
    let mut terminal_write_active: Option<crate::terminal::WriteReceipt> = None;
    let mut terminal: Option<NativeStream<dsh_native_transport::terminal::TerminalFrame>> = None;
    let mut terminal_ticket: Option<crate::terminal::FollowTicket> = None;
    let mut export_active: Option<crate::exporter::Receipt> = None;
    let mut export_download: Option<ExportDownload> = None;
    // Separate ownership: aborting the async download must never detach an admitted save.
    let mut export_saves: JoinSet<Event> = JoinSet::new();
    let mut files = attachments::Coordinator::default();
    let mut file_jobs: JoinSet<attachments::Completion> = JoinSet::new();
    let mut decisions = DecisionCoordinator::<Arc<dsh_native_transport::EventDelivery>>::default();
    let fault = loop {
        if *closing.borrow() {
            break None;
        }
        tokio::select! {
            biased;
            _=closing.changed()=>break None,
            _=events.closed()=>break None,
            command=async {tokio::select! {biased; command=priority.recv()=>command,command=commands.recv()=>command}}=>{
                let Some(command)=command else {break None;};if matches!(command,Command::Shutdown){break None;}
                if let Command::RejoinObservations{epoch}=command {
                    if epoch!=TRANSPORT_EPOCH || observations_live || smoke || !rejoin_jobs.is_empty() || operation_count(tasks.len()+file_jobs.len()+rejoin_jobs.len(),export_active.is_some())>=MAX_OPERATIONS {
                        if !publish(&events,Event::ObservationsRejoined{epoch,result:Err("Rejoin not admitted: only one same-owned read within the normal operation budget is allowed".into())}){break Some("Native UI delivery queue overflow".into());}continue;
                    }
                    let backend=backend.clone();
                    // An owned normal-budget read must never park priority Stop behind HTTP permits.
                    rejoin_jobs.spawn(async move {let result=async {
                        let verified=backend.probe_owned_host(Duration::from_secs(5)).await.map_err(|_|"Original Host identity could not be verified; no restart or Agent follow".to_owned())?;
                        let fresh=verified.connect_mux().await.map_err(|_|"Same-owned observation connection unavailable".to_owned())?;
                        let root=fresh.events().await.map_err(|_|"Root rejoin unavailable".to_owned())?;
                        let workspace=fresh.workspace_follow().await.map_err(|_|"Workspace rejoin unavailable".to_owned())?;
                        let control=fresh.control().await.map_err(|_|"Control rejoin unavailable".to_owned())?;
                        let roster=verified.session_list().await.map_err(|_|"Cold roster read unavailable; no Agent resumed".to_owned());
                        Ok::<_,String>((verified,fresh,root,workspace,control,roster))
                    }.await;(epoch,result)});
                    continue;
                }
                // An expand press reaches the loop here, where the mux and the publisher are in
                // scope; the spawned command task can open no stream (it has neither).
                if let Command::JobOutput{session,job}=command {
                    let job_session=session.as_str().to_owned();
                    let job_id=job.clone();
                    job_output_for=Some(job_id.clone());
                    match operation(&mut closing,&events,mux.as_ref().expect("live observation mux").jobs_follow(dsh_native_transport::dto::JobFollowRequest{session_id:Some(job_session),job_id:job_id.clone(),from:None})).await {
                        Some(Ok(stream))=>job_output=Some(stream),
                        Some(Err(e))=>{
                            job_output=None;job_output_for=None;
                            if !publish(&events,Event::JobsError{generation,error:format!("Job output: {e}")}){break Some("Native UI delivery queue overflow".into());}
                        }
                        None=>break None,
                    }
                    continue;
                }
                if let Command::Select{generation:next,address}=command {
                    if !observations_live || submissions.busy() {if !publish(&events,Event::FollowError{generation:next,error:"Original submission unresolved; no Agent follow dispatched".into()}){break Some("Native UI delivery queue overflow".into());}continue;}
                    files.invalidate(); selected_file_target=None;
                    if let Some(stream)=selected.as_mut(){stream.cancel().await;}selected=None;selected_target=None;generation=next;
                    // A panel follow belongs to the previous Session; retire it.
                    if let Some(stream)=terminal.as_mut(){stream.cancel().await;}terminal=None;terminal_ticket=None;
                    // The same rule the panel follow follows: a jobs stream belongs to the
                    // session it was opened for, so it retires with the selection.
                    if let Some(stream)=jobs.as_mut(){stream.cancel().await;}jobs=None;if let Some(stream)=job_output.as_mut(){stream.cancel().await;}job_output=None;job_output_for=None;
                    let target=selected_export_target(&address);cancel_target=target.clone();
                    // The jobs list scopes to the transcript on screen, which for a subagent is
                    // its child session — not the export target, which is absent there.
                    let job_session=followed_session(&address).as_str().to_owned();
                    match operation(&mut closing,&events,mux.as_ref().expect("live observation mux").session_follow(SessionFollowRequest{address,max_messages:Some(50),turn_window:None,assistant_stream:Some(true)})).await {
                        Some(Ok(stream))=>{
                            // A failure here leaves the panel empty rather than failing a follow
                            // that has already succeeded.
                            selected=Some(stream);selected_target=target;
                            match operation(&mut closing,&events,mux.as_ref().expect("live observation mux").jobs_list(&job_session)).await {
                                Some(Ok(stream))=>jobs=Some(stream),
                                Some(Err(e))=>if !publish(&events,Event::JobsError{generation,error:format!("Jobs list: {e}")}){break Some("Native UI delivery queue overflow".into());},
                                None=>break None,
                            }
                        },
                        Some(Err(e))=>if !publish(&events,Event::FollowError{generation,error:format!("Session follow: {e}")}){break Some("Native UI delivery queue overflow".into());},None=>break None,
                    }
                    continue;
                }
                if submissions.duplicate(&command) {continue;}
                let command = match command {
                    Command::Cancel{generation:stop_generation,session}=>{
                        if submissions.stop(stop_generation,&session,generation,cancel_target.as_ref(),!smoke && !*closing.borrow() && !events.is_closed()) {
                            let client=client.clone();stop_tasks.spawn(async move {Event::Cancel{generation:stop_generation,result:client.session_cancel(&session).await.map_err(|e|format!("Stop: {e}; ACK is not idle"))}});
                        } else if !publish(&events,Event::Cancel{generation:stop_generation,result:Err("Stop not admitted: stale target or another Stop is pending; no cancellation sent".into())}){break Some("Native UI delivery queue overflow".into());}
                        continue;
                    },
                    command if (!observations_live || submissions.busy()) && mutates_host_state(&command) => {
                        if !publish(&events, rejected(command)) {
                            break Some("Native UI delivery queue overflow".into());
                        }
                        continue;
                    },
                    Command::DiscardFile(ticket) => { files.discard(&ticket); continue; },
                    Command::StageFile(submission) => {
                        let enabled = !smoke && !*closing.borrow() && !events.is_closed() && operation_count(tasks.len()+file_jobs.len()+rejoin_jobs.len(),export_active.is_some()) < MAX_OPERATIONS;
                        if let Some(cancellation) = files.admit(&submission.ticket,TRANSPORT_EPOCH,generation,selected_file_target.as_ref(),enabled) {
                            let client = client.clone(); let runtime = tokio::runtime::Handle::current();
                            let file_closing=closing.clone(); let file_events=events.clone();
                            file_jobs.spawn_blocking(move || read_and_stage_file(client,submission,cancellation,file_closing,file_events,runtime));
                        } else if !publish(&events,rejected(Command::StageFile(submission))) { break Some("Native UI delivery queue overflow".into()); }
                        continue;
                    },
                    command => command,
                };
                if matches!(&command,Command::Decision(submission) if decisions.is_active(submission)){continue;}
                // One reading of the single management flight per command, so no arm restates
                // the peers it must not run beside.
                let flights = Flights {
                    management: management_active.is_some(),
                    rename: rename_active.is_some(),
                    fork: fork_active.is_some(),
                    export: export_active.is_some(),
                    terminal: terminal_active.is_some(),
                    mcp: mcp_active.is_some(),
                };
                let operations = operation_count(tasks.len() + file_jobs.len() + rejoin_jobs.len(), export_active.is_some());
                if prompt_worker::normal_capacity_rejects(&command,operations) {if !publish(&events,rejected(command)){break Some("Native UI delivery queue overflow".into());}continue;}
                let command=match command {
                    Command::PromptFile{tickets,request}=>{
                        let request_id=request.request_id.clone();
                        let enabled=!smoke && !*closing.borrow() && !events.is_closed();
                        // The first ticket only names the shared generation/session ownership; the
                        // whole exact set is checked again against the private staged receipts.
                        let Some(owner_ticket)=tickets.first().cloned() else {if !publish(&events,Event::FilePrompt{tickets,request_id,outcome:attachments::PromptOutcome::NotSent}){break Some("Native UI delivery queue overflow".into());}continue;};
                        let owner=prompt_worker::ticket(owner_ticket.generation,&request);
                        if !submissions.admit(owner.clone(),generation,selected_file_target.as_ref(),enabled) {if !publish(&events,Event::FilePrompt{tickets,request_id,outcome:attachments::PromptOutcome::NotSent}){break Some("Native UI delivery queue overflow".into());}continue;}
                        match files.prepare_prompt(&tickets,request,TRANSPORT_EPOCH,generation,selected_file_target.as_ref(),enabled) {
                            Ok(request)=>{
                                if !submissions.freeze(&owner,&request) {break Some("Original file prompt ownership lost before dispatch".into());}
                                let client=client.clone();let mut prompt_closing=closing.clone();let prompt_events=events.clone();
                                tasks.spawn(async move {
                                    let outcome=file_prompt_outcome(&mut prompt_closing,&prompt_events,client.session_prompt(request)).await;
                                    Event::FilePrompt{tickets,request_id,outcome}
                                });
                            },
                            Err(())=>{submissions.settle(&Event::PromptNotSent{ticket:owner});if !publish(&events,Event::FilePrompt{tickets,request_id,outcome:attachments::PromptOutcome::NotSent}) {break Some("Native UI delivery queue overflow".into());}},
                        }
                        continue;
                    },
                    Command::Prompt{generation,request} if smoke || request.content.iter().any(|part|matches!(part,PromptContentPart::File{..}))=>{
                        if !publish(&events,Event::PromptNotSent{ticket:prompt_worker::ticket(generation,&request)}) {break Some("Native UI delivery queue overflow".into());}
                        continue;
                    },
                    Command::Prompt{generation:prompt_generation,request}=>{
                        let owner=prompt_worker::ticket(prompt_generation,&request);
                        if submissions.admit(owner.clone(),generation,selected_target.as_ref(),observations_live && selected.is_some() && !smoke && !*closing.borrow() && !events.is_closed()) {
                            if !submissions.freeze(&owner,&request) {break Some("Original prompt ownership lost before dispatch".into());}
                            let client=client.clone();let request_id=request.request_id.clone();tasks.spawn(async move {Event::Prompt{generation:prompt_generation,request_id,result:client.session_prompt(request).await.map_err(|e|format!("Send outcome unknown: {e}; do not resend"))}});
                        } else if !publish(&events,Event::PromptNotSent{ticket:owner}){break Some("Native UI delivery queue overflow".into());}
                        continue;
                    },
                    Command::CheckSubmission(ticket)=>{
                        if submissions.check(&ticket) {
                            let backend=backend.clone();tasks.spawn(async move {let request=dsh_native_transport::prompt_admission::PromptAdmissionRequest{session_id:ticket.submission.session.clone(),request_id:ticket.submission.request_id.clone()};let value=match backend.probe_owned_host(Duration::from_secs(5)).await {Ok(client)=>client.session_prompt_admission(request).await.ok(),Err(_)=>None};Event::SubmissionChecked{ticket,value}});
                        } else if !publish(&events,Event::SubmissionChecked{ticket,value:None}){break Some("Native UI delivery queue overflow".into());}
                        continue;
                    },
                    Command::Decision(submission)=>{
                        let enabled=decision_guard(submission.key.epoch,smoke).is_ok() && !*closing.borrow() && decisions.delivery_live(&submission.key,|delivery|delivery.is_live());
                        match decisions.admit(&submission,TRANSPORT_EPOCH,enabled) {
                            DecisionAdmission::Send(delivery)=>{
                                let client=client.clone();
                                tasks.spawn(async move {
                                    let result=match &submission.reply {
                                        crate::interactions::Reply::Approval(outcome)=>client.reply_approval_for(&delivery,outcome.clone()).await,
                                        crate::interactions::Reply::Question(answer)=>client.reply_question_for(&delivery,answer.clone()).await,
                                        crate::interactions::Reply::CancelQuestion=>client.cancel_question_for(&delivery).await,
                                    }.map_err(|error|format!("Decision reply: {error}; an admitted transmission may be indeterminate"));
                                    Event::DecisionResult{submission,result}
                                });
                            },
                            DecisionAdmission::Reject=>if !publish(&events,Event::DecisionResult{submission,result:Err("Decision not admitted: unsupported/stale delivery, invalid answer, concurrent operation or disabled replies; no reply sent".into())}){break Some("Native UI delivery queue overflow".into());},
                            DecisionAdmission::Ignore=>{},
                        }
                        continue;
                    },
                    Command::Codex(effect)=>{
                        let ticket=codex_ticket(&effect);
                        if codex.admit(&effect,smoke,*closing.borrow(),operation_count(tasks.len()+file_jobs.len()+rejoin_jobs.len(),export_active.is_some()),key_saving.is_some()||cloudflare.busy()||plugins_active.is_some()||export_active.is_some()) {
                            let backend=backend.clone();
                            tasks.spawn_blocking(move||Event::CodexCompleted{ticket,outcome:execute_codex(&backend,effect)});
                        } else {
                            drop(effect);
                            if !publish(&events,Event::CodexCompleted{ticket,outcome:crate::codex::Outcome::NotSent}) {break Some("Native UI delivery queue overflow".into());}
                        }
                        continue;
                    },
                    Command::Export(submission)=>{
                        let (receipt,destination)=submission.into_parts();
                        // A key save, Codex operation or plugin write is not a flight, so
                        // it rides the extra gate rather than the flight table.
                        if admit_export(&mut export_active,&receipt,&Admission::new(smoke,*closing.borrow() || events.is_closed(),tasks.len(),flights).with_extra(key_saving.is_some()||cloudflare.busy()||codex.busy()||plugins_active.is_some()),generation,selected_target.as_ref()) {
                            export_download=Some(tokio::spawn(download_export(client.clone(),destination,receipt.ticket.target.clone())));
                        } else {
                            drop(destination);
                            if !publish(&events,Event::Exported{receipt,outcome:crate::exporter::Outcome::NotSent}) {break Some("Native UI delivery queue overflow".into());}
                        }
                        continue;
                    },
                    Command::PluginsRead(ticket)=>{
                        if admit_plugin(&mut plugins_active,PluginOperation::Read(ticket),smoke,*closing.borrow(),operation_count(tasks.len()+file_jobs.len()+rejoin_jobs.len(),export_active.is_some()),key_saving.is_some()||cloudflare.busy()||export_active.is_some()||codex.busy()) {
                            tasks.spawn(read_plugins(client.clone(),ticket));
                        } else if !publish(&events,Event::PluginsLoaded{ticket,result:Err(crate::plugins::ReadFailure::Unavailable)}) {break Some("Native UI delivery queue overflow".into());}
                        continue;
                    },
                    Command::PluginSave{ticket,namespace,field,value}=>{
                        if admit_plugin(&mut plugins_active,PluginOperation::Save(ticket),smoke,*closing.borrow(),operation_count(tasks.len()+file_jobs.len()+rejoin_jobs.len(),export_active.is_some()),key_saving.is_some()||cloudflare.busy()||export_active.is_some()||codex.busy()) {
                            tasks.spawn(save_plugin(client.clone(),ticket,*namespace,field,value));
                        } else if !publish(&events,Event::PluginSaved{ticket,result:crate::plugins::SaveOutcome::NotSent}) {break Some("Native UI delivery queue overflow".into());}
                        continue;
                    },
                    Command::Manage(submission)=>{
                        if admit_management(&mut management_active,&submission,&Admission::new(smoke,*closing.borrow(),operations,flights)) {
                            tasks.spawn(manage(client.clone(),submission));
                        } else if !publish(&events,Event::Managed{submission,result:Err(())}) {
                            break Some("Native UI delivery queue overflow".into());
                        }
                        continue;
                    },
                    Command::Rename(submission)=>{
                        if admit_rename(&mut rename_active,&submission,&Admission::new(smoke,*closing.borrow(),operations,flights)) {
                            tasks.spawn(rename(client.clone(),submission));
                        } else if !publish(&events,Event::Renamed{submission,result:Err(())}) {
                            break Some("Native UI delivery queue overflow".into());
                        }
                        continue;
                    },
                    Command::Fork(submission)=>{
                        if admit_fork(&mut fork_active,&submission,&Admission::new(smoke,*closing.borrow(),operations,flights)) {
                            tasks.spawn(fork(client.clone(),submission));
                        } else if !publish(&events,Event::Forked{submission,result:Err(crate::fork::Failure::Unknown)}) {
                            break Some("Native UI delivery queue overflow".into());
                        }
                        continue;
                    },
                    Command::CommandsList(session_id)=>{
                        // Read-only: it advertises commands and reserves nothing.
                        tasks.spawn(commands_list(client.clone(),session_id));
                        continue;
                    },
                    Command::CommandsExecute{session_id,line}=>{
                        // A Host mutation with no model turn, so it is refused while
                        // observations are down and never retried after an
                        // indeterminate reply: the dispatch itself is the request.
                        if observations_live && !submissions.busy() {
                            tasks.spawn(commands_execute(client.clone(),session_id,line));
                        } else if !publish(&events,Event::CommandsExecuted{session_id,line,result:Err(())}) {
                            break Some("Native UI delivery queue overflow".into());
                        }
                        continue;
                    },
                    Command::TerminalRead(ticket)=>{
                        // Read-only: it does not reserve the management flight or spawn a shell.
                        tasks.spawn(terminal_read(client.clone(),ticket));
                        continue;
                    },
                    Command::TerminalFollow(ticket)=>{
                        if !observations_live || mux.is_none() {
                            if !publish(&events,Event::TerminalFollowEnded{ticket,failure:Some(crate::terminal::ReadFailure::Unavailable)}) {break Some("Native UI delivery queue overflow".into());}
                            continue;
                        }
                        if let Some(stream)=terminal.as_mut(){stream.cancel().await;}
                        terminal=None;terminal_ticket=None;
                        match operation(&mut closing,&events,mux.as_ref().expect("live observation mux").terminal_follow(&ticket.target,&ticket.id,&ticket.attachment)).await {
                            Some(Ok(stream))=>{terminal=Some(stream);terminal_ticket=Some(ticket);},
                            Some(Err(_))=>if !publish(&events,Event::TerminalFollowEnded{ticket,failure:Some(crate::terminal::ReadFailure::Rejected)}) {break Some("Native UI delivery queue overflow".into());},
                            None=>break None,
                        }
                        continue;
                    },
                    Command::TerminalUnfollow=>{
                        // The Host attachment and its frames end here, not when the next panel opens.
                        if let Some(stream)=terminal.as_mut(){stream.cancel().await;}
                        terminal=None;terminal_ticket=None;
                        continue;
                    },
                    Command::TerminalCreate(submission)=>{
                        let flight=TerminalFlight::Create(submission.clone());
                        if submission.receipt.ticket.epoch==TRANSPORT_EPOCH && admit_terminal(&mut terminal_active,&flight,&Admission::new(smoke,*closing.borrow(),operations,flights)) {
                            tasks.spawn(terminal_create(client.clone(),submission));
                        } else if !publish(&events,Event::TerminalCreated{submission,result:Err(crate::terminal::CreateFailure::NotSent)}) {
                            break Some("Native UI delivery queue overflow".into());
                        }
                        continue;
                    },
                    Command::TerminalResize(submission)=>{
                        let flight=TerminalFlight::Resize(submission.receipt.clone());
                        if submission.receipt.ticket.epoch==TRANSPORT_EPOCH && admit_terminal(&mut terminal_active,&flight,&Admission::new(smoke,*closing.borrow(),operations,flights)) {
                            tasks.spawn(terminal_resize(client.clone(),submission));
                        } else if !publish(&events,Event::TerminalResized{submission,result:Err(crate::terminal::ResizeOutcome::NotSent)}) {
                            break Some("Native UI delivery queue overflow".into());
                        }
                        continue;
                    },
                    Command::TerminalRename(submission)=>{
                        let flight=TerminalFlight::Rename(submission.receipt.clone());
                        if submission.receipt.ticket.epoch==TRANSPORT_EPOCH && admit_terminal(&mut terminal_active,&flight,&Admission::new(smoke,*closing.borrow(),operations,flights)) {
                            tasks.spawn(terminal_rename(client.clone(),submission));
                        } else if !publish(&events,Event::TerminalRenamed{submission,result:Err(crate::terminal::RenameOutcome::NotSent)}) {
                            break Some("Native UI delivery queue overflow".into());
                        }
                        continue;
                    },
                    Command::TerminalClose(submission)=>{
                        let flight=TerminalFlight::Close(submission.clone());
                        if submission.receipt.ticket.epoch==TRANSPORT_EPOCH && admit_terminal(&mut terminal_active,&flight,&Admission::new(smoke,*closing.borrow(),operations,flights)) {
                            tasks.spawn(terminal_close(client.clone(),submission));
                        } else if !publish(&events,Event::TerminalClosed{submission,result:Err(crate::terminal::CloseFailure::NotSent)}) {
                            break Some("Native UI delivery queue overflow".into());
                        }
                        continue;
                    },
                    Command::TerminalWrite(submission)=>{
                        // One input write per terminal at a time; never retried automatically.
                        let enabled=!smoke && !*closing.borrow() && !events.is_closed()
                            && submission.receipt.ticket.epoch==TRANSPORT_EPOCH
                            && terminal_write_active.is_none()
                            && terminal_active.is_none()
                            && mcp_active.is_none()
                            && operation_count(tasks.len()+file_jobs.len()+rejoin_jobs.len(),export_active.is_some())<MAX_OPERATIONS;
                        if enabled {
                            terminal_write_active=Some(submission.receipt.clone());
                            tasks.spawn(terminal_write(client.clone(),submission));
                        } else if !publish(&events,Event::TerminalWritten{submission,result:Err(crate::terminal::WriteFailure::NotSent)}) {
                            break Some("Native UI delivery queue overflow".into());
                        }
                        continue;
                    },
                    Command::McpRead(ticket)=>{
                        // A local file read plus a read-only inventory call; it
                        // reserves no management flight and mutates nothing.
                        tasks.spawn(read_mcp(client.clone(),mcp_home.clone(),ticket));
                        continue;
                    },
                    Command::McpWrite(submission)=>{
                        if admit_mcp(&mut mcp_active,&submission.ticket,&Admission::new(smoke,*closing.borrow(),operations,flights)) {
                            tasks.spawn(write_mcp(mcp_home.clone(),submission));
                        } else if !publish(&events,Event::McpWritten{submission,outcome:crate::mcp::WriteOutcome::NotSent}) {
                            break Some("Native UI delivery queue overflow".into());
                        }
                        continue;
                    },
                    Command::Catalog(ticket)=>{
                        let client=client.clone();
                        let closing_at_poll=closing.clone();
                        let events_at_poll=events.clone();
                        tasks.spawn(catalog_read_outcome(ticket,smoke,move||*closing_at_poll.borrow() || closing_at_poll.has_changed().is_err() || events_at_poll.is_closed(),move||async move {
                            client.model_catalog().await.map_err(|e|format!("Model catalog: {e}"))
                        }));
                        continue;
                    },
                    Command::Cloudflare(effect)=>{
                        let ticket=effect.ticket();
                        let cancel=matches!(&effect,crate::settings::cloudflare::Effect::CancelLogin {..});
                        if cancel {backend.revoke_cloudflare_browser();}
                        let allowed=cloudflare.admit(&effect,smoke,*closing.borrow() || events.is_closed(),
                            operation_count(tasks.len()+file_jobs.len()+rejoin_jobs.len(),export_active.is_some()),
                            key_saving.is_some() || plugins_active.is_some() || codex.busy() || export_active.is_some());
                        if !allowed {if !publish(&events,rejected(Command::Cloudflare(effect))){break Some("Native UI delivery queue overflow".into());}}
                        else if cloudflare.defer_cancel(&effect) {cloudflare_worker::dispatch_cancel(&mut cloudflare,&backend,&mut tasks);}
                        else {
                            let backend=backend.clone();let closing_at_poll=closing.clone();let events_at_poll=events.clone();
                            tasks.spawn_blocking(move||{
                                if *closing_at_poll.borrow() || closing_at_poll.has_changed().is_err() || events_at_poll.is_closed() {return rejected(Command::Cloudflare(effect));}
                                Event::CloudflareCompleted {ticket,outcome:execute_cloudflare(&backend,effect)}
                            });
                        }
                        continue;
                    },
                    Command::Account(effect)=>{let backend=backend.clone();let ticket=effect.ticket();tasks.spawn_blocking(move||Event::AccountCompleted{ticket,outcome:execute_account(&backend,effect)});continue;},
                    Command::KeyMetadata(ticket)=>{let backend=backend.clone();tasks.spawn_blocking(move||Event::KeyMetadata{ticket,result:if key_guard(ticket,smoke){key_metadata(&backend)}else{Err(())}});continue;},
                    Command::SaveKey{ticket,secret}=>{if codex.busy()||cloudflare.busy()||!key_save_guard(ticket,smoke,key_saving.is_some(),plugins_active.is_some(),export_active.is_some()){drop(secret);if !publish(&events,Event::KeySaved{ticket,result:crate::settings::SaveResult::NotSent}){break Some("Native UI delivery queue overflow".into());}}else{key_saving=Some(ticket);let backend=backend.clone();tasks.spawn_blocking(move||save_key(&backend,ticket,secret,smoke));}continue;},
                    command=>command,
                };
                if matches!(command,Command::PermissionSettingsRead){let backend=backend.clone();tasks.spawn_blocking(move||Event::PermissionSettings(match backend.request(CoreCommand::PermissionSettings,Duration::from_secs(5)){Ok(PublicReply::PermissionSettings(state))=>Ok(state),Ok(_)=>Err("Invalid Core permission settings reply".into()),Err(e)=>Err(format!("Core permission settings: {e}"))}));continue;}
                if matches!(command,Command::AgentPresetsRead){let backend=backend.clone();tasks.spawn_blocking(move||Event::AgentPresets(match backend.request(CoreCommand::AgentPresets,Duration::from_secs(5)){Ok(PublicReply::AgentPresets(roster))=>Ok(roster),Ok(_)=>Err("Invalid Core agent preset reply".into()),Err(e)=>Err(format!("Core agent presets: {e}"))}));continue;}
                if let Command::PermissionDefaultSet{name,expected_revision}=&command{let backend=backend.clone();let (name,expected_revision)=(name.clone(),*expected_revision);tasks.spawn_blocking(move||Event::PermissionDefaultSet(match backend.request(CoreCommand::PermissionDefaultSet{name,expected_revision},Duration::from_secs(5)){Ok(PublicReply::PermissionDefaultSet)=>Ok(()),Ok(_)=>Err("Invalid Core permission default reply".into()),Err(e)=>Err(format!("Core permission default: {e}"))}));continue;}
                if matches!(command,Command::PermissionCatalogRead){let backend=backend.clone();tasks.spawn_blocking(move||Event::PermissionCatalog(match backend.request(CoreCommand::PermissionCatalog,Duration::from_secs(5)){Ok(PublicReply::PermissionCatalog(catalog))=>Ok(catalog),Ok(_)=>Err("Invalid Core permission catalog reply".into()),Err(e)=>Err(format!("Core permission catalog: {e}"))}));continue;}
                if matches!(command,Command::Inspect){let backend=backend.clone();tasks.spawn_blocking(move||Event::Inspection(match backend.request(CoreCommand::Inspect,Duration::from_secs(5)){Ok(PublicReply::Inspection(value))=>Ok(value),Ok(_)=>Err("Invalid Core inspection reply".into()),Err(e)=>Err(format!("Core inspection: {e}"))}));continue;}
                let client=client.clone();tasks.spawn(async move {match command {
                    Command::TerminalUnfollow=>Event::Fault("Terminal unfollow bypassed priority dispatch".into()),
                    Command::TerminalRename(_)=>Event::Fault("Terminal rename bypassed priority dispatch".into()),
                    Command::TerminalResize(_)=>Event::Fault("Terminal resize bypassed priority dispatch".into()),
                    Command::PermissionCatalogRead=>Event::Fault("Permission catalog bypassed priority dispatch".into()),
                    Command::PermissionSettingsRead=>Event::Fault("Permission settings bypassed priority dispatch".into()),
                    Command::AgentPresetsRead=>Event::Fault("Agent presets bypassed priority dispatch".into()),
                    Command::PermissionDefaultSet{..}=>Event::Fault("Permission default bypassed priority dispatch".into()),
                    Command::Create{workspace,cwd}=>Event::Created(client.session_create(SessionCreateRequest{workspace_id:workspace,cwd,..Default::default()}).await.map_err(|e|format!("New session: {e}"))),
                    Command::AgentPresetSelect{session,preset}=>Event::AgentPresetSelected(client.agent_preset_select(&session,&preset).await.map_err(|error|format!("Agent preset: {error}"))),
                    Command::AgentPresetDefault(preset)=>Event::AgentPresetDefault{
                        result:client.agent_preset_default(&preset).await.map(|_|()).map_err(|error|format!("Agent preset default: {error}")),
                        preset,
                    },
                    Command::JobKill{session,job}=>Event::JobKilled(client.job_kill(session.as_str(),&job).await.map_err(|error|format!("Job kill: {error}"))),
                    Command::ChangesSummary{session,seq}=>Event::ChangesLoaded{
                        seq,
                        result:client.changes_summary(&session,seq).await.map_err(|error|format!("Changed files: {error}")),
                    },
                    Command::ListDirectory{path}=>Event::DirectoryListed(client.list_directory(path.as_deref()).await.map_err(|error|format!("Directory listing: {error}"))),
                    Command::PickDirectory=>Event::DirectoryPicked(client.pick_directory().await.map_err(|error|format!("Directory picker: {error}"))),
                    Command::FeedbackPut{session,message_id,rating,if_version,note,category}=>Event::FeedbackWritten{
                        result:client.message_feedback_put(&session,&message_id,&rating,if_version.as_deref(),note.as_deref(),category.as_deref()).await.map_err(|error|format!("Feedback: {error}")),
                        session,
                        message_id,
                    },
                    Command::SessionFeedbackRecord{session,text,category}=>Event::SessionFeedbackRecorded{
                        result:client.session_feedback_record(&session,text.as_deref(),category.as_deref()).await.map_err(|error|format!("Session feedback: {error}")),
                        session,
                    },
                    Command::FeedbackDelete{session,message_id,version}=>Event::FeedbackDeleted{
                        result:client.message_feedback_delete(&session,&message_id,&version).await.map_err(|error|format!("Feedback: {error}")),
                        session,
                        message_id,
                    },
                    Command::OpenWorkspacePath{path}=>Event::WorkspacePathOpened{
                        result:client.open_workspace_path(&path).await.map_err(|error|format!("Open path: {error}")),
                        path,
                    },
                    Command::PluginWaitForInstall{request_id}=>Event::PluginWaitForInstall(
                        client.plugin_manager_wait_for_install(&request_id).await.map_err(|error|format!("Plugins: {error}"))
                    ),
                    Command::PluginCancelInstall{request_id}=>Event::PluginInstallCancelAsked(
                        client.plugin_manager_cancel_install(&request_id).await.map_err(|error|format!("Plugins: {error}"))
                    ),
                    Command::PluginInstall{spec,registry,request_id}=>Event::PluginInstallSettled(
                        client.plugin_manager_install(&spec,registry.as_deref(),&request_id).await.map_err(|error|format!("Plugins: {error}"))
                    ),
                    Command::PluginEntryEnabled{entry_id,enabled}=>Event::PluginEntryChanged{
                        result:client.plugin_manager_set_plugin_enabled(&entry_id,enabled).await.map_err(|error|format!("Plugins: {error}")),
                        entry_id,
                        enabled,
                    },
                    Command::PluginManagerPlugins=>Event::PluginManagerPluginsLoaded(
                        client.plugin_manager_plugins().await.map_err(|error|format!("Plugins: {error}"))
                    ),
                    Command::PluginRegistries=>Event::PluginRegistriesLoaded(
                        client.plugin_manager_registries().await.map_err(|error|format!("Plugins: {error}"))
                    ),
                    Command::PluginInspect{spec,registry}=>Event::PluginInspected(
                        client.plugin_manager_inspect(&spec,registry.as_deref()).await.map_err(|error|format!("Plugins: {error}"))
                    ),
                    Command::PluginRemove{name}=>Event::PluginRemoved{
                        result:client.plugin_manager_remove_bundle(&name).await.map_err(|error|format!("Plugins: {error}")),
                        name,
                    },
                    Command::PluginBundleEnabled{name,enabled}=>Event::PluginBundleChanged{
                        result:client.plugin_manager_set_bundle_enabled(&name,enabled).await.map_err(|error|format!("Plugins: {error}")),
                        name,
                        enabled,
                    },
                    Command::PluginManagerRead=>Event::PluginManagerLoaded(async{
                        let inventory=client.plugin_inventory().await.map_err(|error|format!("Plugins: {error}"))?;
                        if !inventory.management_available{return Ok((false,Vec::new()));}
                        let bundles=client.plugin_manager_bundles().await.map_err(|error|format!("Plugins: {error}"))?;
                        Ok((true,bundles))
                    }.await),
                    Command::OpenInAppApps=>Event::OpenInAppApps(
                        client.open_in_app_apps().await.map_err(|error|format!("Open in app: {error}")),
                    ),
                    Command::OpenInAppOpen{app,path}=>Event::OpenInAppLaunched{
                        result:client.open_in_app_open(&app,&path).await.map_err(|error|format!("Open in app: {error}")),
                        app,
                    },
                    Command::SessionReferences{session,query}=>Event::SessionReferencesLoaded{
                        result:client.session_reference_candidates(&session,&query).await.map_err(|error|format!("Session references: {error}")),
                        query,
                    },
                    Command::FileReferences{session,query}=>Event::FileReferencesLoaded{
                        result:client.file_references(&session,&query).await.map_err(|error|format!("File references: {error}")),
                        query,
                    },
                    Command::ChangesDiff{session,seq,index}=>Event::ChangesDiffLoaded{
                        seq,
                        index,
                        result:client.changes_diff(&session,seq,index).await.map_err(|error|format!("Changed file: {error}")),
                    },
                    Command::ScheduleCatalog=>Event::SchedulesLoaded(client.schedule_catalog().await.map_err(|error|format!("Schedules: {error}"))),
                    Command::RosterRefresh=>Event::Roster(client.session_list().await.map_err(|error|format!("Session list: {error}"))),
                    Command::ScheduleDelete{session,id}=>{
                        let result=client.schedule_delete(&session,&id).await.map_err(|error|format!("Schedule delete: {error}"));
                        Event::ScheduleDeleted{session,id,result}
                    },
                    Command::ScheduleUpdate{session,expected,title,prompt,change}=>{
                        let id=expected.get("id").and_then(serde_json::Value::as_str).unwrap_or("").to_owned();
                        let result=client.schedule_update(&session,&expected,title.as_deref(),prompt.as_deref(),change.as_ref()).await.map_err(|error|format!("Schedule update: {error}"));
                        Event::ScheduleUpdated{session,id,result}
                    },
                    Command::ScheduleHistory{session,id,limit,before}=>{
                        let result=client.schedule_history(&session,&id,limit,before.as_deref()).await.map_err(|error|format!("Schedule history: {error}"));
                        Event::ScheduleHistoryLoaded{session,id,result}
                    },
                    Command::Goal{session,operation}=>{
                        use crate::goal_ui::Operation;
                        let result=match operation{
                            Operation::Edit{reference,objective}=>client.goal_edit(&session,&reference,&objective).await.map(|_|()),
                            Operation::Pause(reference)=>client.goal_pause(&session,&reference).await.map(|_|()),
                            Operation::Resume(reference)=>client.goal_resume(&session,&reference).await.map(|_|()),
                            Operation::Clear(reference)=>client.goal_clear(&session,&reference).await.map(|_|()),
                        }.map_err(|error|format!("Goal: {error}"));
                        Event::GoalUpdated{session,result}
                    },
                    Command::GoalActivationRead{session}=>Event::GoalActivationRead{
                        session:session.clone(),
                        result:client.goal_get(&session).await.map_err(|error|format!("Goal activation: {error}")),
                    },
                    Command::Search{generation,serial,query}=>Event::Searched{
                        generation,
                        serial,
                        result:client.session_search(&query).await.map_err(|error|format!("Search: {error}")),
                    },
                    Command::UpdateQueue(submission)=>{
                        let request=dsh_native_transport::dto::SessionUpdateQueueRequest{
                            session_id:submission.session.clone(),
                            item_id:submission.item_id.clone(),
                            action:submission.action.clone(),
                        };
                        let result=client.session_update_queue(request).await.map(|_|()).map_err(crate::queue_update::failure);
                        Event::QueueUpdated{submission,result}
                    },
                    Command::OpenWorkspace(path)=>Event::WorkspaceOpened(client.workspace_create(path).await.map_err(|e|format!("Open workspace: {e}"))),
                    Command::Catalog(ticket)=>rejected(Command::Catalog(ticket)),
                    Command::Decision(_)=>Event::Fault("Decision bypassed worker admission".into()),
                    Command::Account(_)=>Event::Fault("Account command bypassed worker admission".into()),
                    Command::SelectModel{generation,request}=>Event::Model{generation,result:client.select_model(request).await.map_err(|e|format!("Select model: {e}"))},
                    Command::Prompt{..}|Command::CheckSubmission(_)|Command::RejoinObservations{..}=>Event::Fault("Prompt bypassed worker ownership".into()),
                    Command::Cancel{..}=>Event::Fault("Cancellation bypassed priority dispatch".into()),
                    Command::Page{generation,request}=>Event::Page{generation,result:client.session_page(request).await.map_err(|e|format!("History page: {e}"))},
                    Command::CommandsList(session_id)=>Event::CommandsListed{session_id:session_id.clone(),result:client.commands_list(&session_id).await.map_err(|error|CommandListFailureOutcome::Failed{kind:CommandListFailure::of(&error),error})},
                    Command::SkillsList(session_id)=>Event::SkillsListed{session_id:session_id.clone(),result:client.skills_list(&session_id).await.map_err(|_|())},
                    Command::CommandsExecute{session_id,line}=>Event::CommandsExecuted{session_id:session_id.clone(),line:line.clone(),result:client.commands_execute(&CommandExecuteRequest{session_id:session_id.clone(),line,submitted_attachments:Vec::new()}).await.map_err(|_|())},
                    Command::Cloudflare(_)|Command::PromptFile{..}|Command::StageFile(_)|Command::DiscardFile(_)|Command::Codex(_)|Command::Export(_)|Command::PluginsRead(_)|Command::PluginSave{..}|Command::Manage(_)|Command::Rename(_)|Command::Fork(_)|Command::TerminalRead(_)|Command::TerminalFollow(_)|Command::TerminalCreate(_)|Command::TerminalClose(_)|Command::TerminalWrite(_)|Command::McpRead(_)|Command::McpWrite(_)|Command::KeyMetadata(_)|Command::SaveKey{..}|Command::Inspect|Command::Shutdown|Command::JobOutput{..}=>Event::Fault("Invalid native command state".into()),Command::Select{..}=>Event::Fault("Invalid native command state".into()),
                }});
            }
            event=next_root(&mut root),if root.is_some()=>match event {
                Some(Ok(owned))=>{
                    // A refused decision is one request the coordinator would not hold, not a
                    // transport fault: report it and keep every other pending decision reachable.
                    let (deliver,decision_key)=match &owned.frame {
                        RemoteEventFrame::Waterfall{..}=>{
                            let Some(delivery)=owned.delivery else {break Some("Live decision delivery owner missing".into());};
                            match decisions.observe(TRANSPORT_EPOCH,&owned.frame,Arc::new(delivery)) {
                                Ok(key)=>{live_decisions.push(key.clone());(true,Some(key))},
                                Err(error)=>{
                                    if !publish(&events,Event::Notice(format!("Decision not admitted: {error}"))){break Some("Native UI delivery queue overflow".into());}
                                    (false,None)
                                }
                            }
                        },
                        RemoteEventFrame::Cancel{event_id}=>{decisions.cancel(event_id);live_decisions.retain(|key|&key.event_id!=event_id);(true,None)},
                        _=>(true,None),
                    };
                    if deliver && !publish(&events,Event::Root{frame:owned.frame,decision_key}){break Some("Native UI delivery queue overflow".into());}
                },
                Some(Err(_))|None=>{drop_observations(&mut root,&mut workspaces,&mut control,&mut selected,&mut terminal,&mut mux).await;observations_live=false;files.invalidate();selected_file_target=None;selected_target=None;for key in live_decisions.drain(..){decisions.cancel(&key.event_id);}if !publish(&events,Event::ObservationsLost){break Some("Native UI delivery queue overflow".into());}},
            },
            event=next_workspaces(&mut workspaces),if workspaces.is_some()=>match event {Some(Ok(event))=>if !publish(&events,Event::Workspace(event)){break Some("Native UI delivery queue overflow".into());},Some(Err(_))|None=>{drop_observations(&mut root,&mut workspaces,&mut control,&mut selected,&mut terminal,&mut mux).await;observations_live=false;files.invalidate();selected_file_target=None;selected_target=None;for key in live_decisions.drain(..){decisions.cancel(&key.event_id);}if !publish(&events,Event::ObservationsLost){break Some("Native UI delivery queue overflow".into());}}},
            event=next_control(&mut control),if control.is_some()=>match event {Some(Ok(frame))=>if !publish(&events,Event::Control(frame)){break Some("Native UI delivery queue overflow".into());},Some(Err(_))|None=>{drop_observations(&mut root,&mut workspaces,&mut control,&mut selected,&mut terminal,&mut mux).await;observations_live=false;files.invalidate();selected_file_target=None;selected_target=None;for key in live_decisions.drain(..){decisions.cancel(&key.event_id);}if !publish(&events,Event::ObservationsLost){break Some("Native UI delivery queue overflow".into());}}},
            event=next_selected(&mut selected)=>match event {Some(Ok(frame))=>{if let SessionFollowFrame::Snapshot{header,..}=&frame {selected_file_target=file_receiver(header,selected_target.as_ref());if selected_file_target.is_none(){files.invalidate();}}if !publish(&events,Event::Selected{generation,frame}){break Some("Native UI delivery queue overflow".into());}},Some(Err(e))=>{files.invalidate();selected_file_target=None;selected=None;selected_target=None;jobs=None;job_output=None;job_output_for=None;if !publish(&events,Event::FollowError{generation,error:format!("Session stream: {e}")}){break Some("Native UI delivery queue overflow".into());}},None=>{files.invalidate();selected_file_target=None;selected=None;selected_target=None;jobs=None;job_output=None;job_output_for=None;if !publish(&events,Event::FollowError{generation,error:"Session stream closed; reload session".into()}){break Some("Native UI delivery queue overflow".into());}}},
            jobs_event=next_jobs(&mut jobs),if jobs.is_some()=>match jobs_event {Some(Ok(frame))=>{if !publish(&events,Event::JobsRows(frame.jobs)){break Some("Native UI delivery queue overflow".into());}},Some(Err(e))=>{jobs=None;if !publish(&events,Event::JobsError{generation,error:format!("Jobs list: {e}")}){break Some("Native UI delivery queue overflow".into());}},None=>{jobs=None;}},
            job_event=next_job_output(&mut job_output),if job_output.is_some()=>match job_event {Some(Ok(frame))=>{let job=job_output_for.clone().unwrap_or_default();if !publish(&events,Event::JobsFollow{job,frame}){break Some("Native UI delivery queue overflow".into());}},Some(Err(e))=>{job_output=None;job_output_for=None;if !publish(&events,Event::JobsError{generation,error:format!("Job output: {e}")}){break Some("Native UI delivery queue overflow".into());}},None=>{job_output=None;job_output_for=None;}},
            event=next_terminal(&mut terminal),if terminal.is_some()=>match event {Some(Ok(frame))=>{if let Some(ticket)=terminal_ticket.clone()&& !publish(&events,Event::TerminalFrame{ticket,frame}){break Some("Native UI delivery queue overflow".into());}},Some(Err(_))|None=>{let ticket=terminal_ticket.take();terminal=None;if let Some(ticket)=ticket&& !publish(&events,Event::TerminalFollowEnded{ticket,failure:Some(crate::terminal::ReadFailure::Unavailable)}){break Some("Native UI delivery queue overflow".into());}}},
            event=core_rx.recv()=>match event {Some(event)=>{if let PublicEvent::Codex(metadata)=&event {codex.observe(*metadata);}if let PublicEvent::CloudflareAuth(status)=&event {cloudflare.observe(status.clone());cloudflare_worker::dispatch_cancel(&mut cloudflare,&backend,&mut tasks);}let exited=matches!(&event,PublicEvent::Exited(_));if !publish(&events,Event::Core(event)){break Some("Native UI delivery queue overflow".into());}if exited {break Some("Owned Core exited".into());}},None=>break Some("Core event stream closed".into())},
            download=next_download(&mut export_download),if export_download.is_some()=>{
                export_download.take();
                let Some(receipt)=export_active.clone() else {break Some("Native export ownership lost".into());};
                match download_completion(download) {
                    Ok((destination,archive))=>{
                        let save_closing=closing.clone();
                        let save_events=events.clone();
                        if !spawn_owned_save(&mut export_saves,&closing,&events,move|| {
                            // A queued blocking closure rechecks shutdown before any filesystem work.
                            if !save_can_start(&save_closing,&save_events) {
                                Event::Exported{receipt,outcome:crate::exporter::Outcome::NotSent}
                            } else {save_export(receipt,destination,archive)}
                        }) {break None;}
                    },
                    Err(outcome)=>{
                        let event=Event::Exported{receipt,outcome};
                        settle_export(&mut export_active,&event);
                        if !publish(&events,event) {break Some("Native UI delivery queue overflow".into());}
                    },
                }
            },
            file=file_jobs.join_next(),if !file_jobs.is_empty()=>{
                let completion = match file { Some(Ok(completion)) => completion, Some(Err(_)) => break Some("Native file worker failed".into()), None => continue };
                let ticket = completion.ticket.clone();
                let outcome = files.settle(completion,TRANSPORT_EPOCH,generation,selected_file_target.as_ref());
                if !publish(&events,Event::FileStaged{ticket,outcome}) {break Some("Native UI delivery queue overflow".into());}
            },
            saved=export_saves.join_next(),if !export_saves.is_empty()=>{
                let event=match saved {
                    Some(result)=>{
                        let Some(receipt)=export_active.clone() else {break Some("Native export ownership lost".into());};
                        save_completion(receipt,result)
                    },
                    None=>continue,
                };
                settle_export(&mut export_active,&event);
                if !publish(&events,event) {break Some("Native UI delivery queue overflow".into());}
            },
            rejoined=rejoin_jobs.join_next(),if !rejoin_jobs.is_empty()=>if let Some(result)=rejoined {
                match result {
                    Ok((epoch,Ok((verified,fresh,root_stream,workspace_stream,control_stream,roster))))=>{
                        client=verified;mux=Some(fresh);root=Some(root_stream);workspaces=Some(workspace_stream);control=Some(control_stream);observations_live=true;
                        if !publish(&events,Event::Roster(roster)) || !publish(&events,Event::ObservationsRejoined{epoch,result:Ok(())}){break Some("Native UI delivery queue overflow".into());}
                    },
                    Ok((epoch,Err(error)))=>if !publish(&events,Event::ObservationsRejoined{epoch,result:Err(error)}){break Some("Native UI delivery queue overflow".into());},
                    Err(_)=>if !publish(&events,Event::ObservationsRejoined{epoch:TRANSPORT_EPOCH,result:Err("Owned observation read failed; no automatic restart or Agent resume".into())}){break Some("Native UI delivery queue overflow".into());},
                }
            },
            stopped=stop_tasks.join_next(),if !stop_tasks.is_empty()=>if let Some(result)=stopped {match result {Ok(event)=>{submissions.settle(&event);if !publish(&events,event){break Some("Native UI delivery queue overflow".into());}},Err(_)=>break Some("Reserved Stop worker failed".into())}},
            task=tasks.join_next(),if !tasks.is_empty()=>if let Some(task)=task {match task {Ok(event)=>{submissions.settle(&event);if let Event::FilePrompt{tickets,request_id,..}=&event&& !tickets.iter().any(|ticket| files.settle_prompt(ticket,request_id)) {break Some("Native file prompt ownership lost".into());}if let Event::DecisionResult{submission,result}=&event{decisions.settle(submission,result.is_ok());if result.is_ok(){live_decisions.retain(|key|key!=&submission.key);}}if let Event::CodexCompleted{ticket,outcome}=&event{codex.settle(*ticket,outcome.clone());}if let Event::CloudflareCompleted{ticket,outcome}=&event{cloudflare.settle(*ticket,outcome);cloudflare_worker::dispatch_cancel(&mut cloudflare,&backend,&mut tasks);}settle_plugin(&mut plugins_active,&event);settle_management(&mut management_active,&event);settle_rename(&mut rename_active,&event);settle_fork(&mut fork_active,&event);settle_terminal(&mut terminal_active,&event);settle_terminal_write(&mut terminal_write_active,&event);settle_mcp(&mut mcp_active,&event);if let Event::KeySaved{ticket,..}=&event&& key_saving==Some(*ticket){key_saving=None;}if !publish(&events,event){break Some("Native UI delivery queue overflow".into());}},Err(_)=>break Some("Native operation worker failed".into())}},
        }
    };
    // Revoke private control authority and request owned stop before awaiting
    // carrier closure or non-abortable blocking saves. An admitted write may commit.
    files.invalidate();
    let _stop_requested = backend.stop_async();
    client.invalidate();
    // The only abortable export carrier owns no filesystem task or admitted file handle.
    cancel_download(&mut export_download).await;
    rejoin_jobs.abort_all();
    while rejoin_jobs.join_next().await.is_some() {}
    stop_tasks.abort_all();
    while stop_tasks.join_next().await.is_some() {}
    tasks.abort_all();
    if let Some(stream) = selected.as_mut() {
        stream.cancel().await;
    }
    if let Some(root) = root.as_mut() {
        root.cancel().await;
    }
    if let Some(workspaces) = workspaces.as_mut() {
        workspaces.cancel().await;
    }
    if let Some(control) = control.as_mut() {
        control.cancel().await;
    }
    if let Some(mux) = mux.take() {
        mux.close().await;
    }
    client.close().await;
    let result = backend.stop().map_err(|e| format!("Core stop: {e}"));
    // Stop wakes blocking Core reads/writes before joining their non-abortable tasks.
    while tasks.join_next().await.is_some() {}
    // Never abort or detach admitted saves. An unhealthy filesystem can delay shutdown;
    // no global hard stop deadline or rollback of an admitted file is promised.
    drain_saves(&mut export_saves).await;
    // A regular-file read cannot be aborted safely: retain its slot and join it even after selection/stop.
    while file_jobs.join_next().await.is_some() {}
    drop(core_rx);
    drop(backend);
    // A terminal Core event ends the bridge even while its public sender remains alive.
    // An uncertain stop leaves containment to Core's persistent owner, not a GUI join.
    if result.as_ref().is_ok_and(|s| s.exited) {
        let _ = forwarder.join();
    }
    if let Some(fault) = fault {
        let _ = events.send(Event::Fault(fault)).await;
    }
    let _ = events.send(Event::Stopped(result)).await;
}

#[cfg(any(test, feature = "public-layout-fixture"))]
pub fn test_channels() -> (
    Handle,
    Feed,
    mpsc::Receiver<Command>,
    mpsc::Sender<Event>,
    watch::Receiver<bool>,
) {
    let (commands, receiver) = mpsc::channel(32);
    let (events, feed) = mpsc::channel(32);
    let (shutdown, closing) = watch::channel(false);
    (
        Handle {
            priority: commands.clone(),
            commands,
            shutdown,
        },
        Feed {
            receiver: Arc::new(Mutex::new(Some(feed))),
        },
        receiver,
        events,
        closing,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_subagent_transcript_follows_its_child_for_job_scope() {
        let parent = SessionId::new("PUBLIC-parent").unwrap();
        let child = SessionId::new("PUBLIC-child").unwrap();
        let subagent = SessionAddress::Subagent {
            parent_session_id: parent,
            child_session_id: child.clone(),
            mode: SubagentMode::Continuable,
        };
        // The jobs list scopes to the transcript on screen; the export target stays
        // absent because a subagent grants no export/lineage authority.
        assert_eq!(followed_session(&subagent), &child);
        assert_eq!(selected_export_target(&subagent), None);
        let session = SessionId::new("PUBLIC-session").unwrap();
        assert_eq!(
            followed_session(&SessionAddress::Session {
                session_id: session.clone(),
            }),
            &session
        );
        assert_eq!(
            selected_export_target(&SessionAddress::Session {
                session_id: session
            }),
            Some(SessionId::new("PUBLIC-session").unwrap())
        );
    }
    #[test]
    fn queue_mutations_and_job_kills_join_the_mutation_fence() {
        let session = SessionId::new("PUBLIC-session").unwrap();
        assert!(mutates_host_state(&Command::UpdateQueue(
            crate::queue_update::Submission {
                session: session.clone(),
                item_id: "PUBLIC-item".into(),
                action: QueueAction::Remove,
            }
        )));
        assert!(mutates_host_state(&Command::JobKill {
            session,
            job: "PUBLIC-job".into(),
        }));
    }
    /// A clean single-flight question: no lifecycle fence, no flight held and no
    /// extra gate, so a test adds exactly the one condition it pins.
    fn admission(smoke: bool, stopping: bool, operations: usize, flights: Flights) -> Admission {
        Admission::new(smoke, stopping, operations, flights)
    }
    #[test]
    fn every_flight_excludes_every_other_flight() {
        // One row per command: while any flight is held, none may be taken.
        for held in [
            Flights {
                management: true,
                ..Flights::default()
            },
            Flights {
                rename: true,
                ..Flights::default()
            },
            Flights {
                fork: true,
                ..Flights::default()
            },
            Flights {
                export: true,
                ..Flights::default()
            },
            Flights {
                terminal: true,
                ..Flights::default()
            },
            Flights {
                mcp: true,
                ..Flights::default()
            },
        ] {
            assert!(held.any());
            assert!(admission(false, false, 0, held).refused());
        }
        assert!(!admission(false, false, 0, Flights::default()).refused());
        // The lifecycle fences, the capacity bound and a command's own extra gate
        // each refuse on their own, with no flight involved.
        assert!(admission(true, false, 0, Flights::default()).refused());
        assert!(admission(false, true, 0, Flights::default()).refused());
        assert!(admission(false, false, MAX_OPERATIONS, Flights::default()).refused());
        assert!(!admission(false, false, MAX_OPERATIONS - 1, Flights::default()).refused());
        assert!(
            Admission::new(false, false, 0, Flights::default())
                .with_extra(true)
                .refused()
        );
    }
    #[test]
    fn catalog_rejection_echoes_the_exact_read_ticket() {
        let mut reads = catalog_read::State::default();
        let ticket = reads.begin(TRANSPORT_EPOCH).unwrap();
        let Event::Catalog {
            ticket: received,
            result,
        } = rejected(Command::Catalog(ticket))
        else {
            panic!("catalog rejection lost its receipt kind")
        };
        assert_eq!(received, ticket);
        assert!(result.is_err());
    }
    #[tokio::test]
    async fn catalog_wrong_epoch_smoke_and_shutdown_never_construct_the_read() {
        for (epoch, smoke, stopping) in [
            (TRANSPORT_EPOCH + 1, false, false),
            (TRANSPORT_EPOCH, true, false),
            (TRANSPORT_EPOCH, false, true),
        ] {
            let mut reads = catalog_read::State::default();
            let ticket = reads.begin(epoch).unwrap();
            let rpc = || -> std::future::Ready<Result<ModelCatalog, String>> {
                panic!("catalog read constructed after rejected ownership")
            };
            let Event::Catalog {
                ticket: received,
                result,
            } = catalog_read_outcome(ticket, smoke, || stopping, rpc).await
            else {
                panic!("catalog admission lost its receipt kind")
            };
            assert_eq!(received, ticket);
            assert_eq!(
                result.err().unwrap(),
                "Model catalog read not admitted: stale transport or disabled read; no request sent"
            );
        }
    }
    #[tokio::test]
    async fn catalog_shutdown_or_lost_delivery_between_construction_and_poll_denies_rpc() {
        for mode in 0..3 {
            let ticket = catalog_read::State::default()
                .begin(TRANSPORT_EPOCH)
                .unwrap();
            let (closing_tx, closing_rx) = watch::channel(false);
            let (events_tx, events_rx) = mpsc::channel::<Event>(8);
            let future = catalog_read_outcome(
                ticket,
                false,
                move || {
                    *closing_rx.borrow()
                        || closing_rx.has_changed().is_err()
                        || events_tx.is_closed()
                },
                || -> std::future::Ready<Result<ModelCatalog, String>> {
                    panic!("catalog RPC constructed after pre-poll loss")
                },
            );
            if mode == 1 {
                drop(events_rx);
            } else if mode == 2 {
                drop(closing_tx);
            } else {
                closing_tx.send(true).unwrap();
            }
            let Event::Catalog {
                ticket: received,
                result,
            } = future.await
            else {
                panic!("catalog pre-poll rejection lost its ticket")
            };
            assert_eq!(received, ticket);
            assert!(result.is_err());
        }
    }
    #[tokio::test]
    async fn admitted_catalog_failure_keeps_its_ticket_without_a_client() {
        let mut reads = catalog_read::State::default();
        let ticket = reads.begin(TRANSPORT_EPOCH).unwrap();
        let constructed = std::cell::Cell::new(false);
        let Event::Catalog {
            ticket: received,
            result,
        } = catalog_read_outcome(
            ticket,
            false,
            || false,
            || {
                constructed.set(true);
                std::future::ready(Err("PUBLIC catalog fixture failure".into()))
            },
        )
        .await
        else {
            panic!("catalog completion lost its receipt kind")
        };
        assert!(constructed.get());
        assert_eq!(received, ticket);
        assert_eq!(result.err().unwrap(), "PUBLIC catalog fixture failure");
    }
    #[tokio::test]
    async fn file_prompt_global_closing_lost_owner_and_lost_delivery_win_before_rpc_poll() {
        for mode in 0..3 {
            let (sender, mut closing) = watch::channel(false);
            let mut sender = Some(sender);
            let (events, receiver) = mpsc::channel(1);
            let mut receiver = Some(receiver);
            match mode {
                0 => {
                    sender.as_ref().unwrap().send_replace(true);
                }
                1 => {
                    sender.take();
                }
                _ => {
                    receiver.take();
                }
            }
            let rpc = std::future::poll_fn(|_| -> std::task::Poll<Result<Accepted, ()>> {
                panic!("RPC polled after stop/delivery loss")
            });
            assert_eq!(
                file_prompt_outcome(&mut closing, &events, rpc).await,
                attachments::PromptOutcome::Unknown
            );
        }
    }
    #[tokio::test]
    async fn file_prompt_only_positive_ack_is_known_and_transport_failures_stay_unknown() {
        let (_sender, mut closing) = watch::channel(false);
        let (events, _receiver) = mpsc::channel(1);
        for (result, want) in [
            (
                Ok(Accepted { accepted: true }),
                attachments::PromptOutcome::Accepted,
            ),
            (
                Ok(Accepted { accepted: false }),
                attachments::PromptOutcome::Unknown,
            ),
            (Err(()), attachments::PromptOutcome::Unknown),
        ] {
            assert_eq!(
                file_prompt_outcome(&mut closing, &events, std::future::ready(result)).await,
                want
            );
        }
    }
    #[tokio::test]
    async fn file_prompt_cancel_after_dispatch_drops_owned_future_without_reusing_receipt() {
        struct Joined(Arc<std::sync::atomic::AtomicBool>);
        impl Drop for Joined {
            fn drop(&mut self) {
                self.0.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let dropped = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let joined = Joined(dropped.clone());
        let (sender, mut closing) = watch::channel(false);
        let (events, _receiver) = mpsc::channel(1);
        let (started, wait) = tokio::sync::oneshot::channel();
        let rpc = async move {
            let _joined = joined;
            let _ = started.send(());
            std::future::pending::<Result<Accepted, ()>>().await
        };
        let (outcome, ()) = tokio::join!(
            file_prompt_outcome(&mut closing, &events, rpc),
            async move {
                wait.await.unwrap();
                sender.send_replace(true);
            }
        );
        assert_eq!(outcome, attachments::PromptOutcome::Unknown);
        assert!(dropped.load(std::sync::atomic::Ordering::SeqCst));
    }
    #[test]
    fn file_receiver_requires_supported_correlated_ordinary_snapshot_not_address_or_lineage() {
        let mut header: SessionWireHeader = serde_json::from_value(
            serde_json::json!({"version":4,"id":"PUBLIC_selected","createdAt":0,"isSeeded":false}),
        )
        .unwrap();
        let target = header.id.clone();
        assert!(file_receiver(&header, None).is_none());
        assert_eq!(file_receiver(&header, Some(&target)), Some(target.clone()));
        header.parent_session = Some(SessionId::new("PUBLIC_lineage").unwrap());
        assert_eq!(file_receiver(&header, Some(&target)), Some(target.clone()));
        header.origin = Some(SubagentOrigin::Subagent);
        assert!(file_receiver(&header, Some(&target)).is_none());
        header.origin = None;
        header.version = 5;
        assert!(file_receiver(&header, Some(&target)).is_none());
        header.version = 4;
        header.id = SessionId::new("PUBLIC_other").unwrap();
        assert!(file_receiver(&header, Some(&target)).is_none());
    }
    #[test]
    fn file_guard_rechecks_local_global_shutdown_and_delivery_even_before_queued_read() {
        let (local_tx, local) = watch::channel(false);
        let (closing_tx, closing) = watch::channel(false);
        let (events, rx) = mpsc::channel(1);
        assert!(!file_canceled(&local, &closing, &events));
        closing_tx.send_replace(true);
        assert!(file_canceled(&local, &closing, &events));
        let source =
            file_intake::SelectedFile::new(std::path::PathBuf::from("/PUBLIC unavailable file"))
                .unwrap();
        assert_eq!(
            source
                .read(|| file_canceled(&local, &closing, &events))
                .unwrap_err(),
            file_intake::Failure::Canceled
        );
        closing_tx.send_replace(false);
        local_tx.send_replace(true);
        assert!(file_canceled(&local, &closing, &events));
        local_tx.send_replace(false);
        drop(rx);
        assert!(file_canceled(&local, &closing, &events));
    }
    #[test]
    fn file_guard_rejects_lost_cancel_or_shutdown_owner() {
        let (local_tx, local) = watch::channel(false);
        let (closing_tx, closing) = watch::channel(false);
        let (events, _rx) = mpsc::channel(1);
        drop(local_tx);
        assert!(file_canceled(&local, &closing, &events));
        let (_replacement_tx, replacement) = watch::channel(false);
        drop(closing_tx);
        assert!(file_canceled(&replacement, &closing, &events));
    }
    #[test]
    fn file_read_and_export_share_the_aggregate_eight_operation_budget() {
        assert_eq!(operation_count(6 + 1, true), MAX_OPERATIONS);
        assert_eq!(operation_count(7 + 1, true), MAX_OPERATIONS + 1);
        assert_eq!(operation_count(7 + 1, false), MAX_OPERATIONS);
        assert_eq!(operation_count(6 + 1, false), MAX_OPERATIONS - 1);
    }
    fn management_submission(
        operation: crate::management::Operation,
    ) -> crate::management::Submission {
        crate::management::Submission {
            ticket: crate::management::Ticket {
                epoch: TRANSPORT_EPOCH,
                generation: 3,
                serial: 5,
                target: SessionId::new("session-worker-fixture").unwrap(),
            },
            operation,
        }
    }
    fn rename_submission() -> crate::rename::Submission {
        use crate::rename::{Action, Context, Rename};
        let view = crate::management::View {
            epoch: TRANSPORT_EPOCH,
            generation: 3,
            target: SessionId::new("session-worker-rename").unwrap(),
        };
        let context = Context {
            view: Some(view.clone()),
            allowed: true,
        };
        let mut rename = Rename::default();
        rename.apply(
            Action::Open {
                view,
                title: "PUBLIC worker title".into(),
            },
            &context,
        );
        let ticket = rename.ticket().unwrap();
        rename.apply(Action::Review(ticket), &context);
        let review = rename.review().unwrap().clone();
        rename
            .apply(Action::Confirm(review), &context)
            .expect("reviewed public rename")
    }
    #[test]
    fn management_epoch_smoke_and_stopping_guards_do_not_reserve() {
        use crate::management::Operation;
        for operation in [
            Operation::Pin,
            Operation::Unpin,
            Operation::Archive,
            Operation::Restore,
        ] {
            for (epoch, smoke, stopping) in [
                (TRANSPORT_EPOCH, true, false),
                (TRANSPORT_EPOCH, false, true),
                (0, false, false),
                (TRANSPORT_EPOCH + 1, false, false),
            ] {
                let mut submission = management_submission(operation);
                submission.ticket.epoch = epoch;
                let mut active = None;
                assert!(!admit_management(
                    &mut active,
                    &submission,
                    &admission(smoke, stopping, 0, Flights::default())
                ));
                assert!(active.is_none());
            }
            let submission = management_submission(operation);
            let mut active = None;
            assert!(admit_management(
                &mut active,
                &submission,
                &admission(false, false, 0, Flights::default())
            ));
            assert_eq!(active, Some(submission));
        }
    }
    #[test]
    fn management_capacity_boundary_keeps_one_bounded_join_slot() {
        let submission = management_submission(crate::management::Operation::Pin);
        for operations in [MAX_OPERATIONS, MAX_OPERATIONS + 1, usize::MAX] {
            let mut active = None;
            assert!(!admit_management(
                &mut active,
                &submission,
                &admission(false, false, operations, Flights::default())
            ));
            assert!(active.is_none());
        }
        let mut active = None;
        assert!(admit_management(
            &mut active,
            &submission,
            &admission(false, false, MAX_OPERATIONS - 1, Flights::default())
        ));
        assert_eq!(active, Some(submission));
    }
    #[test]
    fn management_single_flight_rejects_even_a_different_target() {
        let first = management_submission(crate::management::Operation::Pin);
        let mut second = management_submission(crate::management::Operation::Restore);
        second.ticket.target = SessionId::new("session-worker-other").unwrap();
        second.ticket.serial += 1;
        let mut active = None;
        assert!(admit_management(
            &mut active,
            &first,
            &admission(false, false, 0, Flights::default())
        ));
        // The occupied slot refuses a second management write even at a live
        // generation, and so does any other held flight.
        assert!(!admit_management(
            &mut active,
            &first,
            &admission(false, false, 0, Flights::default())
        ));
        assert!(!admit_management(
            &mut active,
            &second,
            &admission(
                false,
                false,
                0,
                Flights {
                    export: true,
                    ..Flights::default()
                }
            )
        ));
        assert_eq!(active, Some(first.clone()));
        settle_management(
            &mut active,
            &Event::Managed {
                submission: first,
                result: Err(()),
            },
        );
        assert!(active.is_none());
        assert!(admit_management(
            &mut active,
            &second,
            &admission(false, false, 0, Flights::default())
        ));
        assert_eq!(active, Some(second));
    }
    #[test]
    fn management_only_its_exact_receipt_releases_the_slot() {
        let first = management_submission(crate::management::Operation::Archive);
        for result in [Ok(()), Err(())] {
            let mut active = Some(first.clone());
            settle_management(&mut active, &Event::Ready("public-fixture".into()));
            assert_eq!(active, Some(first.clone()));
            let mut stale = first.clone();
            stale.ticket.serial += 1;
            settle_management(
                &mut active,
                &Event::Managed {
                    submission: stale,
                    result,
                },
            );
            assert_eq!(active, Some(first.clone()));
            let mut stale = first.clone();
            stale.ticket.generation += 1;
            settle_management(
                &mut active,
                &Event::Managed {
                    submission: stale,
                    result,
                },
            );
            assert_eq!(active, Some(first.clone()));
            let mut stale = first.clone();
            stale.ticket.epoch += 1;
            settle_management(
                &mut active,
                &Event::Managed {
                    submission: stale,
                    result,
                },
            );
            assert_eq!(active, Some(first.clone()));
            let mut stale = first.clone();
            stale.ticket.target = SessionId::new("session-worker-other").unwrap();
            settle_management(
                &mut active,
                &Event::Managed {
                    submission: stale,
                    result,
                },
            );
            assert_eq!(active, Some(first.clone()));
            let mut stale = first.clone();
            stale.operation = crate::management::Operation::Restore;
            settle_management(
                &mut active,
                &Event::Managed {
                    submission: stale,
                    result,
                },
            );
            assert_eq!(active, Some(first.clone()));
            settle_management(
                &mut active,
                &Event::Managed {
                    submission: first.clone(),
                    result,
                },
            );
            assert!(active.is_none());
            let mut next = first.clone();
            next.ticket.serial += 1;
            assert!(admit_management(
                &mut active,
                &next,
                &admission(false, false, 0, Flights::default())
            ));
            settle_management(
                &mut active,
                &Event::Managed {
                    submission: first.clone(),
                    result,
                },
            );
            assert_eq!(active, Some(next));
        }
    }
    #[test]
    fn management_capacity_rejection_is_redacted_and_keeps_full_submission() {
        use crate::management::Operation;
        for operation in [
            Operation::Pin,
            Operation::Unpin,
            Operation::Archive,
            Operation::Restore,
        ] {
            let submission = management_submission(operation);
            let event = rejected(Command::Manage(submission.clone()));
            assert_eq!(format!("{event:?}"), "NativeWorkerEvent(redacted)");
            assert!(
                matches!(event, Event::Managed { submission: returned, result: Err(()) } if returned == submission)
            );
        }
    }
    #[test]
    fn management_queue_rejection_returns_full_command_and_shutdown_bypasses_it() {
        let (handle, _, receiver, _events, closing) = test_channels();
        for _ in 0..32 {
            assert!(handle.send(Command::Inspect).is_ok());
        }
        let submission = management_submission(crate::management::Operation::Unpin);
        assert!(
            matches!(handle.send(Command::Manage(submission.clone())), Err(Command::Manage(returned)) if returned == submission)
        );
        assert!(handle.send(Command::Shutdown).is_ok());
        assert!(*closing.borrow());
        drop(receiver);
        assert!(
            matches!(handle.send(Command::Manage(submission.clone())), Err(Command::Manage(returned)) if returned == submission)
        );
    }
    #[test]
    fn rename_epoch_smoke_stopping_and_cross_flight_guards_do_not_reserve() {
        let submission = rename_submission();
        for (epoch, context) in [
            (0, admission(false, false, 0, Flights::default())),
            (
                TRANSPORT_EPOCH,
                admission(true, false, 0, Flights::default()),
            ),
            (
                TRANSPORT_EPOCH,
                admission(false, true, 0, Flights::default()),
            ),
            (
                TRANSPORT_EPOCH,
                admission(
                    false,
                    false,
                    0,
                    Flights {
                        management: true,
                        ..Flights::default()
                    },
                ),
            ),
            (
                TRANSPORT_EPOCH,
                admission(
                    false,
                    false,
                    0,
                    Flights {
                        fork: true,
                        ..Flights::default()
                    },
                ),
            ),
            (
                TRANSPORT_EPOCH,
                admission(
                    false,
                    false,
                    0,
                    Flights {
                        export: true,
                        ..Flights::default()
                    },
                ),
            ),
            (
                TRANSPORT_EPOCH,
                admission(
                    false,
                    false,
                    0,
                    Flights {
                        terminal: true,
                        ..Flights::default()
                    },
                ),
            ),
            (
                TRANSPORT_EPOCH,
                admission(
                    false,
                    false,
                    0,
                    Flights {
                        mcp: true,
                        ..Flights::default()
                    },
                ),
            ),
            (
                TRANSPORT_EPOCH,
                admission(false, false, MAX_OPERATIONS, Flights::default()),
            ),
        ] {
            let mut candidate = submission.clone();
            candidate.receipt.ticket.epoch = epoch;
            let mut active = None;
            assert!(!admit_rename(&mut active, &candidate, &context));
            assert!(active.is_none());
        }
        let mut active = None;
        assert!(admit_rename(
            &mut active,
            &submission,
            &admission(false, false, 0, Flights::default())
        ));
        assert_eq!(active.as_ref(), Some(&submission));
        // One rename flight across sessions: a fresh serial on another target still refuses.
        let mut other = submission.clone();
        other.receipt.ticket.target = SessionId::new("session-worker-other").unwrap();
        other.receipt.ticket.serial += 1;
        assert!(!admit_rename(
            &mut active,
            &other,
            &admission(false, false, 0, Flights::default())
        ));
        assert_eq!(active.as_ref(), Some(&submission));
    }
    #[test]
    fn rename_only_its_exact_receipt_releases_the_slot() {
        let first = rename_submission();
        for result in [
            Ok(SessionRenameValue {
                title: "PUBLIC ack".into(),
                seq: 5,
            }),
            Err(()),
        ] {
            let mut active = Some(first.clone());
            settle_rename(&mut active, &Event::Ready("public-fixture".into()));
            assert_eq!(active.as_ref(), Some(&first));
            for stale in [
                {
                    let mut stale = first.clone();
                    stale.receipt.attempt += 1;
                    stale
                },
                {
                    let mut stale = first.clone();
                    stale.receipt.editor += 1;
                    stale
                },
                {
                    let mut stale = first.clone();
                    stale.receipt.ticket.target = SessionId::new("session-worker-other").unwrap();
                    stale
                },
            ] {
                settle_rename(
                    &mut active,
                    &Event::Renamed {
                        submission: stale,
                        result: result.clone(),
                    },
                );
                assert_eq!(active.as_ref(), Some(&first));
            }
            settle_rename(
                &mut active,
                &Event::Renamed {
                    submission: first.clone(),
                    result,
                },
            );
            assert!(active.is_none());
            let mut next = first.clone();
            next.receipt.attempt += 1;
            assert!(admit_rename(
                &mut active,
                &next,
                &admission(false, false, 0, Flights::default())
            ));
            settle_rename(
                &mut active,
                &Event::Renamed {
                    submission: first.clone(),
                    result: Err(()),
                },
            );
            assert_eq!(active.as_ref(), Some(&next));
        }
    }
    #[test]
    fn rename_queue_rejection_is_redacted_and_returns_the_full_submission() {
        let submission = rename_submission();
        let event = rejected(Command::Rename(submission.clone()));
        assert_eq!(format!("{event:?}"), "NativeWorkerEvent(redacted)");
        assert!(
            matches!(event, Event::Renamed { submission: returned, result: Err(()) } if returned == submission)
        );
        let (handle, _, receiver, _events, closing) = test_channels();
        for _ in 0..32 {
            assert!(handle.send(Command::Inspect).is_ok());
        }
        assert!(
            matches!(handle.send(Command::Rename(submission.clone())), Err(Command::Rename(returned)) if returned == submission)
        );
        assert!(handle.send(Command::Shutdown).is_ok());
        assert!(*closing.borrow());
        drop(receiver);
        assert!(
            matches!(handle.send(Command::Rename(submission.clone())), Err(Command::Rename(returned)) if returned == submission)
        );
    }
    fn fork_submission() -> crate::fork::Submission {
        use crate::fork::{Action, Context, Fork};
        let view = crate::management::View {
            epoch: TRANSPORT_EPOCH,
            generation: 3,
            target: SessionId::new("session-worker-fork").unwrap(),
        };
        let context = Context {
            view: Some(view.clone()),
            allowed: true,
        };
        let mut fork = Fork::default();
        fork.apply(Action::Open { view, cut: None }, &context);
        let ticket = fork.ticket().unwrap();
        fork.apply(Action::Review(ticket), &context);
        let review = fork.review().unwrap().clone();
        fork.apply(Action::Confirm(review), &context)
            .expect("reviewed public fork")
    }
    #[test]
    fn fork_epoch_smoke_stopping_and_cross_flight_guards_do_not_reserve() {
        let submission = fork_submission();
        for (epoch, context) in [
            (0, admission(false, false, 0, Flights::default())),
            (
                TRANSPORT_EPOCH,
                admission(true, false, 0, Flights::default()),
            ),
            (
                TRANSPORT_EPOCH,
                admission(false, true, 0, Flights::default()),
            ),
            (
                TRANSPORT_EPOCH,
                admission(
                    false,
                    false,
                    0,
                    Flights {
                        management: true,
                        ..Flights::default()
                    },
                ),
            ),
            (
                TRANSPORT_EPOCH,
                admission(
                    false,
                    false,
                    0,
                    Flights {
                        rename: true,
                        ..Flights::default()
                    },
                ),
            ),
            (
                TRANSPORT_EPOCH,
                admission(
                    false,
                    false,
                    0,
                    Flights {
                        export: true,
                        ..Flights::default()
                    },
                ),
            ),
            (
                TRANSPORT_EPOCH,
                admission(
                    false,
                    false,
                    0,
                    Flights {
                        terminal: true,
                        ..Flights::default()
                    },
                ),
            ),
            (
                TRANSPORT_EPOCH,
                admission(
                    false,
                    false,
                    0,
                    Flights {
                        mcp: true,
                        ..Flights::default()
                    },
                ),
            ),
            (
                TRANSPORT_EPOCH,
                admission(false, false, MAX_OPERATIONS, Flights::default()),
            ),
        ] {
            let mut candidate = submission.clone();
            candidate.receipt.ticket.epoch = epoch;
            let mut active = None;
            assert!(!admit_fork(&mut active, &candidate, &context));
            assert!(active.is_none());
        }
        let mut active = None;
        assert!(admit_fork(
            &mut active,
            &submission,
            &admission(false, false, 0, Flights::default())
        ));
        assert_eq!(active.as_ref(), Some(&submission));
        // One fork flight across sources: a fresh serial on another target still refuses.
        let mut other = submission.clone();
        other.receipt.ticket.target = SessionId::new("session-worker-other").unwrap();
        other.receipt.ticket.serial += 1;
        assert!(!admit_fork(
            &mut active,
            &other,
            &admission(false, false, 0, Flights::default())
        ));
        assert_eq!(active.as_ref(), Some(&submission));
    }
    #[test]
    fn fork_only_its_exact_receipt_releases_the_slot() {
        let first = fork_submission();
        for result in [
            Ok(SessionForkValue {
                session_id: SessionId::new("session-worker-child").unwrap(),
            }),
            Err(crate::fork::Failure::Unavailable),
        ] {
            let mut active = Some(first.clone());
            settle_fork(&mut active, &Event::Ready("public-fixture".into()));
            assert_eq!(active.as_ref(), Some(&first));
            for stale in [
                {
                    let mut stale = first.clone();
                    stale.receipt.attempt += 1;
                    stale
                },
                {
                    let mut stale = first.clone();
                    stale.receipt.ticket.target = SessionId::new("session-worker-other").unwrap();
                    stale
                },
            ] {
                settle_fork(
                    &mut active,
                    &Event::Forked {
                        submission: stale,
                        result: result.clone(),
                    },
                );
                assert_eq!(active.as_ref(), Some(&first));
            }
            settle_fork(
                &mut active,
                &Event::Forked {
                    submission: first.clone(),
                    result,
                },
            );
            assert!(active.is_none());
            let mut next = first.clone();
            next.receipt.attempt += 1;
            assert!(admit_fork(
                &mut active,
                &next,
                &admission(false, false, 0, Flights::default())
            ));
            settle_fork(
                &mut active,
                &Event::Forked {
                    submission: first.clone(),
                    result: Err(crate::fork::Failure::Unknown),
                },
            );
            assert_eq!(active.as_ref(), Some(&next));
        }
    }
    #[test]
    fn fork_queue_rejection_is_redacted_and_returns_the_full_submission() {
        let submission = fork_submission();
        let event = rejected(Command::Fork(submission.clone()));
        assert_eq!(format!("{event:?}"), "NativeWorkerEvent(redacted)");
        assert!(
            matches!(event, Event::Forked { submission: returned, result: Err(crate::fork::Failure::Unknown) } if returned == submission)
        );
        let (handle, _, receiver, _events, closing) = test_channels();
        for _ in 0..32 {
            assert!(handle.send(Command::Inspect).is_ok());
        }
        assert!(
            matches!(handle.send(Command::Fork(submission.clone())), Err(Command::Fork(returned)) if returned == submission)
        );
        assert!(handle.send(Command::Shutdown).is_ok());
        assert!(*closing.borrow());
        drop(receiver);
        assert!(
            matches!(handle.send(Command::Fork(submission.clone())), Err(Command::Fork(returned)) if returned == submission)
        );
    }
    #[test]
    fn cloudflare_worker_admission_and_rejection_preserve_ticket_not_secret() {
        use crate::settings::cloudflare::{Effect, Outcome};
        let ticket = crate::settings::Ticket {
            epoch: TRANSPORT_EPOCH,
            panel: 3,
            serial: 8,
        };
        let effect = Effect::Save {
            ticket,
            revision: 0,
            account: String::new(),
            models: false,
            browser: false,
            free: false,
            token: Some(
                dsh_native_core::SecretApiKey::new("PUBLIC_FAKE_CLOUDFLARE".into()).unwrap(),
            ),
        };
        assert!(prompt_worker::normal_capacity_rejects(
            &Command::Cloudflare(effect),
            MAX_OPERATIONS
        ));
        for effect in [
            Effect::Read(ticket),
            Effect::Disable {
                ticket,
                revision: 0,
            },
            Effect::Save {
                ticket,
                revision: 0,
                account: String::new(),
                models: false,
                browser: false,
                free: false,
                token: Some(
                    dsh_native_core::SecretApiKey::new("PUBLIC_FAKE_CLOUDFLARE".into()).unwrap(),
                ),
            },
        ] {
            let event = rejected(Command::Cloudflare(effect));
            assert!(!format!("{event:?}").contains("PUBLIC_FAKE"));
            assert!(
                matches!(event,Event::CloudflareCompleted{ticket:received,outcome:Outcome::NotSent} if received==ticket)
            );
        }
        let (handle, _, receiver, _, closing) = test_channels();
        for _ in 0..32 {
            assert!(handle.send(Command::Inspect).is_ok());
        }
        let returned = handle.send(Command::Cloudflare(Effect::Disable {
            ticket,
            revision: 0,
        }));
        assert!(
            matches!(returned,Err(Command::Cloudflare(Effect::Disable{ticket:received,..})) if received==ticket)
        );
        assert!(handle.send(Command::Shutdown).is_ok());
        assert!(*closing.borrow());
        drop(receiver);
    }
    #[test]
    fn key_commands_are_epoch_smoke_fenced_and_rejection_is_metadata_only() {
        let ticket = crate::settings::Ticket {
            epoch: TRANSPORT_EPOCH,
            panel: 4,
            serial: 9,
        };
        assert!(key_guard(ticket, false));
        assert!(!key_guard(ticket, true));
        assert!(!key_guard(
            crate::settings::Ticket { epoch: 0, ..ticket },
            false
        ));
        let event = rejected(Command::SaveKey {
            ticket,
            secret: dsh_native_core::SecretApiKey::new("PUBLIC_FIXTURE_KEY".into()).unwrap(),
        });
        assert_eq!(format!("{event:?}"), "NativeWorkerEvent(redacted)");
        assert!(
            matches!(event, Event::KeySaved { ticket: returned, result: crate::settings::SaveResult::NotSent } if returned == ticket)
        );
    }
    #[test]
    fn worker_decision_epoch_and_smoke_fence() {
        assert!(decision_guard(TRANSPORT_EPOCH, false).is_ok());
        assert!(decision_guard(TRANSPORT_EPOCH, true).is_err());
        assert!(decision_guard(TRANSPORT_EPOCH + 1, false).is_err());
    }
    #[test]
    fn worker_operation_capacity_rejection_keeps_decision_identity() {
        let submission = crate::interactions::Submission {
            key: crate::interactions::Key {
                epoch: TRANSPORT_EPOCH,
                event_id: RemoteEventId::new("event-1").unwrap(),
                serial: 4,
            },
            attempt: 7,
            reply: crate::interactions::Reply::Approval(ApprovalOutcome::Rejected),
        };
        let Event::DecisionResult {
            submission: returned,
            result,
        } = rejected(Command::Decision(submission.clone()))
        else {
            panic!("wrong error event")
        };
        assert_eq!(returned.key, submission.key);
        assert_eq!(returned.attempt, 7);
        assert!(result.is_err());
    }
    use futures_util::StreamExt;
    #[test]
    fn priority_shutdown_bypasses_full_business_queue() {
        let (handle, _, _receiver, _events, closing) = test_channels();
        for _ in 0..32 {
            assert!(handle.send(Command::Inspect).is_ok());
        }
        assert!(handle.send(Command::Inspect).is_err());
        assert!(handle.send(Command::Shutdown).is_ok());
        assert!(*closing.borrow());
    }
    #[test]
    fn owner_drop_signals_before_join() {
        let (shutdown, mut closing) = watch::channel(false);
        let thread = thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    if !*closing.borrow() {
                        closing.changed().await.unwrap();
                    }
                    assert!(*closing.borrow());
                });
        });
        drop(Owner {
            shutdown,
            thread: Some(thread),
        });
    }
    #[test]
    fn terminal_metadata_ends_forwarder_with_publisher_alive() {
        let (sender, receiver) = std::sync::mpsc::sync_channel(2);
        let (forward, _sink) = mpsc::channel(2);
        let thread = thread::spawn(move || forward_core(receiver, forward));
        sender
            .send(PublicEvent::Exited(StopResult {
                exited: true,
                graceful: true,
                containment_unknown: false,
                observed_descendants_remaining: 0,
                exit_code: Some(0),
            }))
            .unwrap();
        thread.join().unwrap();
        drop(sender);
    }
    #[tokio::test]
    async fn unexpected_feed_close_is_an_uncertain_result() {
        let (_, feed, _, events, _) = test_channels();
        drop(events);
        let mut stream = stream(&feed);
        assert!(matches!(stream.next().await, Some(Event::Stopped(Err(_)))));
        assert!(stream.next().await.is_none());
    }
}
