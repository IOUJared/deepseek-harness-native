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
#[path = "codex_worker.rs"]
mod codex_worker;
#[cfg(test)]
#[path = "export_worker_tests.rs"]
mod export_worker_tests;
#[cfg(test)]
#[path = "plugin_worker_tests.rs"]
mod plugin_worker_tests;
use codex_worker::{CodexCoordinator, codex_ticket, execute_codex};
#[path = "attachments.rs"]
pub(crate) mod attachments;
#[path = "decision_worker.rs"]
mod decision_worker;
#[path = "file_intake.rs"]
pub(crate) mod file_intake;
use decision_worker::{Admission as DecisionAdmission, DecisionCoordinator};
pub enum Command {
    StageFile(attachments::Submission),
    DiscardFile(attachments::Ticket),
    PromptFile {
        ticket: attachments::Ticket,
        request: SessionPromptRequest,
    },
    Codex(crate::codex::Effect),
    Export(crate::exporter::Submission),
    PluginsRead(crate::plugins::ReadTicket),
    PluginSave {
        ticket: crate::plugins::WriteTicket,
        namespace: dsh_native_transport::plugin::SettingsNamespaceView,
        field: dsh_native_transport::plugin::SettingsField,
        value: dsh_native_transport::plugin::SettingsScalar,
    },
    Manage(crate::management::Submission),
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
    Catalog,
    SelectModel {
        generation: u64,
        request: SelectModelRequest,
    },
    Prompt {
        generation: u64,
        request: SessionPromptRequest,
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
    FileStaged {
        ticket: attachments::Ticket,
        outcome: attachments::Outcome,
    },
    FilePrompt {
        ticket: attachments::Ticket,
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
    Catalog(Result<ModelCatalog, String>),
    Model {
        generation: u64,
        result: Result<SelectModelValue, String>,
    },
    Prompt {
        generation: u64,
        result: Result<Accepted, String>,
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
    Inspection(Result<Inspection, String>),
    Fault(String),
    Stopped(Result<StopResult, String>),
    NoBackendStarted,
}
impl std::fmt::Debug for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeWorkerEvent(redacted)")
    }
}
#[derive(Clone)]
pub struct Handle {
    commands: mpsc::Sender<Command>,
    shutdown: watch::Sender<bool>,
}
impl std::fmt::Debug for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeWorkerHandle(redacted)")
    }
}
impl Handle {
    pub fn send(&self, command: Command) -> Result<(), Command> {
        if matches!(command, Command::Shutdown) {
            self.shutdown.send_replace(true);
            return Ok(());
        }
        self.commands.try_send(command).map_err(|e| e.into_inner())
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
            runtime.block_on(run(options, receiver, events, closing, smoke));
        })
        .map_err(|_| "Native worker thread unavailable".to_owned())?;
    Ok((
        Handle {
            commands,
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
async fn next_selected(
    stream: &mut Option<NativeStream<SessionFollowFrame>>,
) -> Option<dsh_native_transport::Result<SessionFollowFrame>> {
    match stream {
        Some(stream) => stream.next().await,
        None => std::future::pending().await,
    }
}
fn rejected(command: Command) -> Event {
    let error = "Native concurrent operation limit reached; retry explicitly".to_owned();
    match command {
        Command::StageFile(submission) => Event::FileStaged {
            ticket: submission.ticket,
            outcome: attachments::Outcome::NotSent,
        },
        Command::PromptFile { ticket, request } => Event::FilePrompt {
            ticket,
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
        Command::Catalog => Event::Catalog(Err(error)),
        Command::SelectModel { generation, .. } => Event::Model {
            generation,
            result: Err(error),
        },
        Command::Prompt { generation, .. } => Event::Prompt {
            generation,
            result: Err(error),
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
        Command::Shutdown => Event::Fault(error),
    }
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
    smoke: bool,
    stopping: bool,
    operations: usize,
) -> bool {
    if smoke
        || stopping
        || submission.ticket.epoch != TRANSPORT_EPOCH
        || active.is_some()
        || operations >= MAX_OPERATIONS
    {
        return false;
    }
    *active = Some(submission.clone());
    true
}
fn admit_management_without_export(
    active: &mut Option<crate::management::Submission>,
    submission: &crate::management::Submission,
    smoke: bool,
    stopping: bool,
    operations: usize,
    export_active: bool,
) -> bool {
    !export_active && admit_management(active, submission, smoke, stopping, operations)
}
fn settle_management(active: &mut Option<crate::management::Submission>, event: &Event) {
    if let Event::Managed { submission, .. } = event {
        if active.as_ref() == Some(submission) {
            *active = None;
        }
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

struct ExportAdmission<'a> {
    smoke: bool,
    stopping: bool,
    operations: usize,
    generation: u64,
    target: Option<&'a SessionId>,
    key_saving: bool,
    plugins_active: bool,
    management_active: bool,
}
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
fn admit_export(
    active: &mut Option<crate::exporter::Receipt>,
    receipt: &crate::exporter::Receipt,
    context: ExportAdmission<'_>,
) -> bool {
    if active.is_some()
        || context.smoke
        || context.stopping
        || context.operations >= MAX_OPERATIONS
        || receipt.ticket.epoch != TRANSPORT_EPOCH
        || receipt.ticket.generation != context.generation
        || context.target != Some(&receipt.ticket.target)
        || context.key_saving
        || context.plugins_active
        || context.management_active
    {
        return false;
    }
    *active = Some(receipt.clone());
    true
}
fn settle_export(active: &mut Option<crate::exporter::Receipt>, event: &Event) {
    if let Event::Exported { receipt, .. } = event {
        if active.as_ref() == Some(receipt) {
            *active = None;
        }
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

async fn run(
    options: RustBackendOptions,
    mut commands: mpsc::Receiver<Command>,
    events: mpsc::Sender<Event>,
    mut closing: watch::Receiver<bool>,
    smoke: bool,
) {
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
    let client = match operation(&mut closing, &events, backend.connect_transport(limits)).await {
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
    let (mut root, mut workspaces) = match (root, workspaces) {
        (Some(Ok(root)), Some(Ok(workspaces))) => (root, workspaces),
        _ => {
            mux.close().await;
            client.close().await;
            let fault = (!*closing.borrow() && !events.is_closed())
                .then(|| "Native root subscriptions failed".into());
            finish_core(backend, &events, fault).await;
            return;
        }
    };
    let (core_tx, mut core_rx) = mpsc::channel(32);
    let forwarder = thread::spawn(move || forward_core(core_events, core_tx));
    let mut tasks = JoinSet::new();
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
    // File intake needs a correlated, supported ordinary header, not just a Session address.
    let mut selected_file_target: Option<SessionId> = None;
    let mut generation = 0;
    let mut key_saving: Option<crate::settings::Ticket> = None;
    let mut plugins_active: Option<PluginOperation> = None;
    let mut codex = CodexCoordinator::default();
    let mut management_active: Option<crate::management::Submission> = None;
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
            command=commands.recv()=>{
                let Some(command)=command else {break None;};if matches!(command,Command::Shutdown){break None;}
                if let Command::Select{generation:next,address}=command {
                    files.invalidate(); selected_file_target=None;
                    if let Some(stream)=selected.as_mut(){stream.cancel().await;}selected=None;selected_target=None;generation=next;
                    let target=selected_export_target(&address);
                    match operation(&mut closing,&events,mux.session_follow(SessionFollowRequest{address,max_messages:Some(50),turn_window:None,assistant_stream:Some(true)})).await {
                        Some(Ok(stream))=>{selected=Some(stream);selected_target=target;},Some(Err(e))=>if !publish(&events,Event::FollowError{generation,error:format!("Session follow: {e}")}){break Some("Native UI delivery queue overflow".into());},None=>break None,
                    }
                    continue;
                }
                let command = match command {
                    Command::DiscardFile(ticket) => { files.discard(&ticket); continue; },
                    Command::StageFile(submission) => {
                        let enabled = !smoke && !*closing.borrow() && !events.is_closed() && operation_count(tasks.len()+file_jobs.len(),export_active.is_some()) < MAX_OPERATIONS;
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
                if operation_count(tasks.len()+file_jobs.len(),export_active.is_some())>=MAX_OPERATIONS {if !publish(&events,rejected(command)){break Some("Native UI delivery queue overflow".into());}continue;}
                let command=match command {
                    Command::PromptFile{ticket,request}=>{
                        let request_id=request.request_id.clone();
                        let enabled=!smoke && !*closing.borrow() && !events.is_closed();
                        match files.prepare_prompt(&ticket,request,TRANSPORT_EPOCH,generation,selected_file_target.as_ref(),enabled) {
                            Ok(request)=>{
                                let client=client.clone();let mut prompt_closing=closing.clone();let prompt_events=events.clone();
                                tasks.spawn(async move {
                                    let outcome=file_prompt_outcome(&mut prompt_closing,&prompt_events,client.session_prompt(request)).await;
                                    Event::FilePrompt{ticket,request_id,outcome}
                                });
                            },
                            Err(())=>if !publish(&events,Event::FilePrompt{ticket,request_id,outcome:attachments::PromptOutcome::NotSent}) {break Some("Native UI delivery queue overflow".into());},
                        }
                        continue;
                    },
                    Command::Prompt{generation,request} if smoke || request.content.iter().any(|part|matches!(part,PromptContentPart::File{..}))=>{
                        if !publish(&events,Event::Prompt{generation,result:Err("File receipts require exact private worker draft authority; prompt not sent".into())}) {break Some("Native UI delivery queue overflow".into());}
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
                        if codex.admit(&effect,smoke,*closing.borrow(),operation_count(tasks.len()+file_jobs.len(),export_active.is_some()),key_saving.is_some()||plugins_active.is_some()||export_active.is_some()) {
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
                        if admit_export(&mut export_active,&receipt,ExportAdmission {
                            smoke,stopping:*closing.borrow() || events.is_closed(),
                            operations:tasks.len(),generation,target:selected_target.as_ref(),
                            key_saving:key_saving.is_some()||codex.busy(),plugins_active:plugins_active.is_some(),management_active:management_active.is_some(),
                        }) {
                            export_download=Some(tokio::spawn(download_export(client.clone(),destination,receipt.ticket.target.clone())));
                        } else {
                            drop(destination);
                            if !publish(&events,Event::Exported{receipt,outcome:crate::exporter::Outcome::NotSent}) {break Some("Native UI delivery queue overflow".into());}
                        }
                        continue;
                    },
                    Command::PluginsRead(ticket)=>{
                        if admit_plugin(&mut plugins_active,PluginOperation::Read(ticket),smoke,*closing.borrow(),operation_count(tasks.len()+file_jobs.len(),export_active.is_some()),key_saving.is_some()||export_active.is_some()||codex.busy()) {
                            tasks.spawn(read_plugins(client.clone(),ticket));
                        } else if !publish(&events,Event::PluginsLoaded{ticket,result:Err(crate::plugins::ReadFailure::Unavailable)}) {break Some("Native UI delivery queue overflow".into());}
                        continue;
                    },
                    Command::PluginSave{ticket,namespace,field,value}=>{
                        if admit_plugin(&mut plugins_active,PluginOperation::Save(ticket),smoke,*closing.borrow(),operation_count(tasks.len()+file_jobs.len(),export_active.is_some()),key_saving.is_some()||export_active.is_some()||codex.busy()) {
                            tasks.spawn(save_plugin(client.clone(),ticket,namespace,field,value));
                        } else if !publish(&events,Event::PluginSaved{ticket,result:crate::plugins::SaveOutcome::NotSent}) {break Some("Native UI delivery queue overflow".into());}
                        continue;
                    },
                    Command::Manage(submission)=>{
                        if admit_management_without_export(&mut management_active,&submission,smoke,*closing.borrow(),operation_count(tasks.len()+file_jobs.len(),export_active.is_some()),export_active.is_some()) {
                            tasks.spawn(manage(client.clone(),submission));
                        } else if !publish(&events,Event::Managed{submission,result:Err(())}) {
                            break Some("Native UI delivery queue overflow".into());
                        }
                        continue;
                    },
                    Command::KeyMetadata(ticket)=>{let backend=backend.clone();tasks.spawn_blocking(move||Event::KeyMetadata{ticket,result:if key_guard(ticket,smoke){key_metadata(&backend)}else{Err(())}});continue;},
                    Command::SaveKey{ticket,secret}=>{if codex.busy()||!key_save_guard(ticket,smoke,key_saving.is_some(),plugins_active.is_some(),export_active.is_some()){drop(secret);if !publish(&events,Event::KeySaved{ticket,result:crate::settings::SaveResult::NotSent}){break Some("Native UI delivery queue overflow".into());}}else{key_saving=Some(ticket);let backend=backend.clone();tasks.spawn_blocking(move||save_key(&backend,ticket,secret,smoke));}continue;},
                    command=>command,
                };
                if matches!(command,Command::Inspect){let backend=backend.clone();tasks.spawn_blocking(move||Event::Inspection(match backend.request(CoreCommand::Inspect,Duration::from_secs(5)){Ok(PublicReply::Inspection(value))=>Ok(value),Ok(_)=>Err("Invalid Core inspection reply".into()),Err(e)=>Err(format!("Core inspection: {e}"))}));continue;}
                let client=client.clone();tasks.spawn(async move {match command {
                    Command::Create{workspace,cwd}=>Event::Created(client.session_create(SessionCreateRequest{workspace_id:workspace,cwd,..Default::default()}).await.map_err(|e|format!("New session: {e}"))),
                    Command::OpenWorkspace(path)=>Event::WorkspaceOpened(client.workspace_create(path).await.map_err(|e|format!("Open workspace: {e}"))),
                    Command::Catalog=>Event::Catalog(client.model_catalog().await.map_err(|e|format!("Model catalog: {e}"))),
                    Command::Decision(_)=>Event::Fault("Decision bypassed worker admission".into()),
                    Command::SelectModel{generation,request}=>Event::Model{generation,result:client.select_model(request).await.map_err(|e|format!("Select model: {e}"))},
                    Command::Prompt{generation,request}=>Event::Prompt{generation,result:client.session_prompt(request).await.map_err(|e|format!("Send: {e}"))},
                    Command::Cancel{generation,session}=>Event::Cancel{generation,result:client.session_cancel(&session).await.map_err(|e|format!("Stop: {e}"))},
                    Command::Page{generation,request}=>Event::Page{generation,result:client.session_page(request).await.map_err(|e|format!("History page: {e}"))},
                    Command::PromptFile{..}|Command::StageFile(_)|Command::DiscardFile(_)|Command::Codex(_)|Command::Export(_)|Command::PluginsRead(_)|Command::PluginSave{..}|Command::Manage(_)|Command::KeyMetadata(_)|Command::SaveKey{..}|Command::Inspect|Command::Shutdown|Command::Select{..}=>Event::Fault("Invalid native command state".into()),
                }});
            }
            event=root.next_event()=>match event {
                Some(Ok(owned))=>{
                    let decision_key=match &owned.frame {
                        RemoteEventFrame::Waterfall{..}=>{
                            let Some(delivery)=owned.delivery else {break Some("Live decision delivery owner missing".into());};
                            match decisions.observe(TRANSPORT_EPOCH,&owned.frame,Arc::new(delivery)) {
                                Ok(key)=>Some(key),Err(error)=>break Some(error.into()),
                            }
                        },
                        RemoteEventFrame::Cancel{event_id}=>{decisions.cancel(event_id);None},
                        _=>None,
                    };
                    if !publish(&events,Event::Root{frame:owned.frame,decision_key}){break Some("Native UI delivery queue overflow".into());}
                },
                Some(Err(e))=>break Some(format!("Root event stream: {e}")),None=>break Some("Root event stream closed; pending decisions invalid".into()),
            },
            event=workspaces.next()=>match event {Some(Ok(event))=>if !publish(&events,Event::Workspace(event)){break Some("Native UI delivery queue overflow".into());},Some(Err(e))=>break Some(format!("Workspace stream: {e}")),None=>break Some("Workspace stream closed".into())},
            event=next_selected(&mut selected)=>match event {Some(Ok(frame))=>{if let SessionFollowFrame::Snapshot{header,..}=&frame {selected_file_target=file_receiver(header,selected_target.as_ref());if selected_file_target.is_none(){files.invalidate();}}if !publish(&events,Event::Selected{generation,frame}){break Some("Native UI delivery queue overflow".into());}},Some(Err(e))=>{files.invalidate();selected_file_target=None;selected=None;selected_target=None;if !publish(&events,Event::FollowError{generation,error:format!("Session stream: {e}")}){break Some("Native UI delivery queue overflow".into());}},None=>{files.invalidate();selected_file_target=None;selected=None;selected_target=None;if !publish(&events,Event::FollowError{generation,error:"Session stream closed; reload session".into()}){break Some("Native UI delivery queue overflow".into());}}},
            event=core_rx.recv()=>match event {Some(event)=>{if let PublicEvent::Codex(metadata)=&event {codex.observe(*metadata);}let exited=matches!(&event,PublicEvent::Exited(_));if !publish(&events,Event::Core(event)){break Some("Native UI delivery queue overflow".into());}if exited {break Some("Owned Core exited".into());}},None=>break Some("Core event stream closed".into())},
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
            task=tasks.join_next(),if !tasks.is_empty()=>if let Some(task)=task {match task {Ok(event)=>{if let Event::FilePrompt{ticket,request_id,..}=&event {if !files.settle_prompt(ticket,request_id) {break Some("Native file prompt ownership lost".into());}}if let Event::DecisionResult{submission,result}=&event{decisions.settle(submission,result.is_ok());}if let Event::CodexCompleted{ticket,outcome}=&event{codex.settle(*ticket,*outcome);}settle_plugin(&mut plugins_active,&event);settle_management(&mut management_active,&event);if let Event::KeySaved{ticket,..}=&event{if key_saving==Some(*ticket){key_saving=None;}}if !publish(&events,event){break Some("Native UI delivery queue overflow".into());}},Err(_)=>break Some("Native operation worker failed".into())}},
        }
    };
    // Revoke private control authority and request owned stop before awaiting
    // carrier closure or non-abortable blocking saves. An admitted write may commit.
    files.invalidate();
    let _stop_requested = backend.stop_async();
    client.invalidate();
    // The only abortable export carrier owns no filesystem task or admitted file handle.
    cancel_download(&mut export_download).await;
    tasks.abort_all();
    if let Some(stream) = selected.as_mut() {
        stream.cancel().await;
    }
    root.cancel().await;
    workspaces.cancel().await;
    mux.close().await;
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
        Handle { commands, shutdown },
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
                    smoke,
                    stopping,
                    0
                ));
                assert!(active.is_none());
            }
            let submission = management_submission(operation);
            let mut active = None;
            assert!(admit_management(&mut active, &submission, false, false, 0));
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
                false,
                false,
                operations
            ));
            assert!(active.is_none());
        }
        let mut active = None;
        assert!(admit_management(
            &mut active,
            &submission,
            false,
            false,
            MAX_OPERATIONS - 1
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
        assert!(admit_management(&mut active, &first, false, false, 0));
        assert!(!admit_management(&mut active, &first, false, false, 1));
        assert!(!admit_management(&mut active, &second, false, false, 1));
        assert_eq!(active, Some(first.clone()));
        settle_management(
            &mut active,
            &Event::Managed {
                submission: first,
                result: Err(()),
            },
        );
        assert!(active.is_none());
        assert!(admit_management(&mut active, &second, false, false, 0));
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
            assert!(admit_management(&mut active, &next, false, false, 0));
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
