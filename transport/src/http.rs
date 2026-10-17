use crate::{Error, Mux, Result, dto::*, validate_json};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use cookie::{Cookie, SameSite};
use reqwest::{
    Client,
    header::{CONTENT_TYPE, COOKIE, HeaderValue, LOCATION, ORIGIN, SET_COOKIE},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::Semaphore;
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use url::Url;

static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
const MAX_NATIVE_OUTBOUND: usize = 256 * 1024;

/// Owned private startup capability. No Debug, Serialize, Display, or URL accessor.
/// Construct only in the native Core worker, never a GUI model or callback.
pub struct SecretLaunchUrl(Url);
impl SecretLaunchUrl {
    pub fn new(value: String) -> Result<Self> {
        if value.len() > 16_384 {
            return Err(Error::InvalidLaunch);
        }
        let url = Url::parse(&value).map_err(|_| Error::InvalidLaunch)?;
        let port = url.port().ok_or(Error::InvalidLaunch)?;
        let origin = format!("http://127.0.0.1:{port}");
        let pairs: Vec<_> = url.query_pairs().collect();
        if port == 0
            || url.scheme() != "http"
            || url.host_str() != Some("127.0.0.1")
            || url.path() != "/"
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
            || !value.starts_with(&format!("{origin}/?token="))
            || pairs.len() != 1
            || pairs[0].0 != "token"
            || pairs[0].1.is_empty()
        {
            return Err(Error::InvalidLaunch);
        }
        Ok(Self(url))
    }
}

/// Complete result and queue limits; larger values require explicit native-owner policy.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_result_bytes: usize,
    pub max_json_items: usize,
    pub queue_bytes: usize,
    pub max_streams: usize,
    pub max_concurrent_rpc: usize,
    pub max_pending_events: usize,
    pub timeout: Duration,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_result_bytes: 32 * 1024 * 1024,
            max_json_items: 100_000,
            queue_bytes: 256 * 1024,
            max_streams: 16,
            max_concurrent_rpc: 8,
            max_pending_events: 128,
            timeout: Duration::from_secs(30),
        }
    }
}
impl Limits {
    pub(crate) fn validate(self) -> Result<Self> {
        if self.max_result_bytes == 0
            || self.max_result_bytes > 128 * 1024 * 1024
            || self.queue_bytes == 0
            || self.queue_bytes > 128 * 1024 * 1024
            || self.max_json_items == 0
            || self.max_json_items > 1_000_000
            || !(1..=64).contains(&self.max_streams)
            || !(1..=64).contains(&self.max_concurrent_rpc)
            || !(1..=1024).contains(&self.max_pending_events)
            || self.timeout.is_zero()
            || self.timeout > Duration::from_secs(300)
        {
            return Err(Error::InvalidLimits);
        }
        Ok(self)
    }
}

pub(crate) struct Authority {
    pub origin: Url,
    pub cookie: HeaderValue,
}
impl Authority {
    pub(crate) fn copy(&self) -> Self {
        Self {
            origin: self.origin.clone(),
            cookie: self.cookie.clone(),
        }
    }
}
pub(crate) struct Inner {
    pub http: Client,
    pub authority: Mutex<Option<Authority>>,
    pub limits: Limits,
    pub lifetime: CancellationToken,
    pub generation: u64,
    pub rpc: AtomicU64,
    pub rpc_slots: Semaphore,
    pub cancel_slot: Semaphore,
    pub upload_slots: Arc<Semaphore>,
    pub tasks: TaskTracker,
    pub owners: AtomicU64,
    pub mux_slot: Arc<Semaphore>,
    pub pending: Mutex<HashMap<RemoteEventId, Arc<crate::event::PendingEvent>>>,
}
impl Inner {
    pub(crate) fn authority(&self) -> Result<Authority> {
        if self.lifetime.is_cancelled() {
            return Err(Error::Closed);
        }
        self.authority
            .lock()
            .map_err(|_| Error::Closed)?
            .as_ref()
            .map(Authority::copy)
            .ok_or(Error::Closed)
    }
}

/// Cloneable native business client; credentials/origin are inaccessible and non-serializable.
pub struct NativeClient {
    pub(crate) inner: Arc<Inner>,
}
impl Clone for NativeClient {
    fn clone(&self) -> Self {
        self.inner.owners.fetch_add(1, Ordering::Relaxed);
        Self {
            inner: self.inner.clone(),
        }
    }
}
impl Drop for NativeClient {
    fn drop(&mut self) {
        if self.inner.owners.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.invalidate();
        }
    }
}

#[derive(Clone, Copy)]
enum Endpoint {
    SessionList,
    SessionCreate,
    SessionPrompt,
    PromptAdmission,
    ModelCatalog,
    SelectModel,
    RenameSession,
    ForkSession,
    Cancel,
    Page,
    AccountState,
    WorkspaceCreate,
    PinSession,
    UnpinSession,
    ArchiveSession,
    UnarchiveSession,
    Projections,
    QuestionAnswer,
    EventResult,
    PluginInventory,
    AgentPresetSelect,
    JobKill,
    PluginSettings,
    PluginMutate,
    TerminalCreate,
    TerminalList,
    TerminalShells,
    TerminalEnvironment,
    TerminalWrite,
    TerminalResize,
    TerminalClose,
    CommandsList,
    CommandsExecute,
    UpdateQueue,
    Search,
    GoalGet,
    GoalEdit,
    GoalPause,
    GoalResume,
    GoalClear,
    ScheduleCatalog,
    ScheduleDelete,
    ScheduleHistory,
    ScheduleUpdate,
    FileReferences,
    SessionReferences,
    OpenWorkspacePath,
    FeedbackList,
    FeedbackPut,
    FeedbackDelete,
    PickDirectory,
    ListDirectory,
}
impl Endpoint {
    fn name(self) -> &'static str {
        match self {
            Self::SessionList => "session/list",
            Self::SessionCreate => "session/create",
            Self::SessionPrompt => "session/prompt",
            Self::PromptAdmission => "session/promptAdmission",
            Self::ModelCatalog => "session/modelCatalog",
            Self::SelectModel => "session/selectModel",
            Self::RenameSession => "session/rename",
            Self::ForkSession => "session/fork",
            Self::Cancel => "session/cancel",
            Self::UpdateQueue => "session/updateQueue",
            Self::Search => "session/search",
            Self::GoalGet => "goals/get",
            Self::GoalEdit => "goals/edit",
            Self::GoalPause => "goals/pause",
            Self::GoalResume => "goals/resume",
            Self::GoalClear => "goals/clear",
            Self::ScheduleCatalog => "schedule/catalog",
            Self::ScheduleDelete => "schedule/delete",
            Self::ScheduleHistory => "schedule/history",
            Self::ScheduleUpdate => "schedule/update",
            Self::FileReferences => "fileReferences/list",
            Self::SessionReferences => "sessionReferenceResolver/candidates",
            Self::OpenWorkspacePath => "session/openWorkspacePath",
            Self::FeedbackList => "messageFeedback/list",
            Self::FeedbackPut => "messageFeedback/put",
            Self::FeedbackDelete => "messageFeedback/delete",
            Self::PickDirectory => "directoryPicker/pick",
            Self::ListDirectory => "directoryPicker/list",
            Self::Page => "session/page",
            Self::AccountState => "account/getState",
            Self::WorkspaceCreate => "workspace/create",
            Self::PinSession => "workspace/pinSession",
            Self::UnpinSession => "workspace/unpinSession",
            Self::ArchiveSession => "workspace/archiveSession",
            Self::UnarchiveSession => "workspace/unarchiveSession",
            Self::Projections => "session/projections",
            Self::QuestionAnswer => "userQuestions/answer",
            Self::EventResult => "$events/result",
            Self::PluginInventory => "pluginInventory/list",
            Self::AgentPresetSelect => "agentPresets/select",
            Self::JobKill => "job/kill",
            Self::PluginSettings => "settings/describe",
            Self::PluginMutate => "settings/mutate",
            Self::TerminalCreate => "terminal/create",
            Self::TerminalList => "terminal/list",
            Self::TerminalShells => "terminal/shells",
            Self::TerminalEnvironment => "terminal/environment",
            Self::TerminalWrite => "terminal/write",
            Self::TerminalResize => "terminal/resize",
            Self::TerminalClose => "terminal/close",
            Self::CommandsList => "commands/list",
            Self::CommandsExecute => "commands/execute",
        }
    }
}

impl NativeClient {
    /// Exchange the exact private ready URL. Never follows or fetches the redirected HTML.
    pub async fn connect(launch: SecretLaunchUrl, limits: Limits) -> Result<Self> {
        let limits = limits.validate()?;
        let http = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(limits.timeout)
            .build()
            .map_err(|_| Error::Network)?;
        let response = http
            .get(launch.0.clone())
            .send()
            .await
            .map_err(map_network)?;
        if response.status().as_u16() != 303
            || response
                .headers()
                .get(LOCATION)
                .and_then(|h| h.to_str().ok())
                != Some("./")
        {
            return Err(Error::Authentication);
        }
        let mut origin = launch.0;
        origin.set_query(None);
        let authority = origin.host_str().ok_or(Error::InvalidLaunch)?;
        let authority = format!("{authority}:{}", origin.port().ok_or(Error::InvalidLaunch)?);
        let expected = format!(
            "dsh-auth-{}",
            URL_SAFE_NO_PAD.encode(Sha256::digest(authority.as_bytes()))
        );
        let headers: Vec<_> = response.headers().get_all(SET_COOKIE).iter().collect();
        if headers.len() != 1 {
            return Err(Error::CookieScope);
        }
        let raw = headers[0].to_str().map_err(|_| Error::CookieScope)?;
        if raw.len() > 16_384 {
            return Err(Error::CookieScope);
        }
        let cookie = Cookie::parse(raw).map_err(|_| Error::CookieScope)?;
        if cookie.name() != expected
            || cookie.value().is_empty()
            || cookie.http_only() != Some(true)
            || cookie.same_site() != Some(SameSite::Strict)
            || cookie.path() != Some("/")
            || cookie.domain().is_some()
            || cookie.secure() == Some(true)
            || cookie.max_age().is_none_or(|a| a.whole_seconds() <= 0)
            || cookie
                .expires_datetime()
                .is_none_or(|a| a <= cookie::time::OffsetDateTime::now_utc())
        {
            return Err(Error::CookieScope);
        }
        let mut secret = HeaderValue::from_str(&format!("{}={}", cookie.name(), cookie.value()))
            .map_err(|_| Error::CookieScope)?;
        secret.set_sensitive(true);
        drop(response);
        Ok(Self {
            inner: Arc::new(Inner {
                http,
                authority: Mutex::new(Some(Authority {
                    origin,
                    cookie: secret,
                })),
                limits,
                lifetime: CancellationToken::new(),
                generation: NEXT_GENERATION.fetch_add(1, Ordering::Relaxed),
                rpc: AtomicU64::new(1),
                rpc_slots: Semaphore::new(limits.max_concurrent_rpc),
                cancel_slot: Semaphore::new(1),
                upload_slots: Arc::new(Semaphore::new(crate::upload::MAX_UPLOAD_JOBS)),
                tasks: TaskTracker::new(),
                owners: AtomicU64::new(1),
                mux_slot: Arc::new(Semaphore::new(1)),
                pending: Mutex::new(HashMap::new()),
            }),
        })
    }

    /// Immediately revoke this connection generation and discard its private cookie authority.
    /// The Core ownership thread uses this on Host exit, including unsolicited exits.
    /// In-flight operations observe cancellation; use close() to additionally await quiescence.
    pub fn invalidate(&self) {
        self.inner.lifetime.cancel();
        if let Ok(mut authority) = self.inner.authority.lock() {
            authority.take();
        }
        if let Ok(mut pending) = self.inner.pending.lock() {
            for delivery in pending.values() {
                delivery.lifetime.cancel();
            }
            pending.clear();
        }
    }

    /// Cancel observations/in-flight reads, await mux workers and RPC quiescence, then drop cookie state.
    /// This does not cancel or dispose any backend Agent/Session.
    pub async fn close(&self) {
        self.invalidate();
        self.inner.tasks.close();
        let permits = self
            .inner
            .rpc_slots
            .acquire_many(self.inner.limits.max_concurrent_rpc as u32)
            .await
            .ok();
        let cancel = self.inner.cancel_slot.acquire().await.ok();
        self.inner.tasks.wait().await;
        if let Ok(mut authority) = self.inner.authority.lock() {
            authority.take();
        }
        if let Ok(mut pending) = self.inner.pending.lock() {
            for delivery in pending.values() {
                delivery.lifetime.cancel();
            }
            pending.clear();
        }
        drop(cancel);
        drop(permits);
    }

    async fn invoke<T: DeserializeOwned>(&self, endpoint: Endpoint, args: Value) -> Result<T> {
        tokio::select! {
            biased;
            _ = self.inner.lifetime.cancelled() => Err(Error::Closed),
            result = self.invoke_active(endpoint, args) => result,
        }
    }
    async fn invoke_active<T: DeserializeOwned>(
        &self,
        endpoint: Endpoint,
        args: Value,
    ) -> Result<T> {
        self.invoke_active_checked(endpoint, args, None).await
    }
    async fn invoke_active_checked<T: DeserializeOwned>(
        &self,
        endpoint: Endpoint,
        args: Value,
        delivery: Option<&crate::EventDelivery>,
    ) -> Result<T> {
        // Only the fixed active-turn cancellation endpoint can use the independent lane.
        let slots = if matches!(endpoint, Endpoint::Cancel) {
            &self.inner.cancel_slot
        } else {
            &self.inner.rpc_slots
        };
        let _permit = slots.acquire().await.map_err(|_| Error::Closed)?;
        if let Some(delivery) = delivery {
            delivery.check(&self.inner, None)?;
        }
        let auth = self.inner.authority()?;
        let rpc_id = format!(
            "native-{}-{}",
            self.inner.generation,
            self.inner.rpc.fetch_add(1, Ordering::Relaxed)
        );
        let body = json!({"type":"client-request", "rpcId":rpc_id, "method":endpoint.name(), "payload":{"args":args}});
        validate_json(&body, self.inner.limits.max_json_items)?;
        let bytes = serde_json::to_vec(&body).map_err(|_| Error::InvalidDto)?;
        if bytes.len() > MAX_NATIVE_OUTBOUND {
            return Err(Error::Oversize);
        }
        let url = auth
            .origin
            .join(&format!("api/{}", endpoint.name()))
            .map_err(|_| Error::InvalidLaunch)?;
        if let Some(delivery) = delivery {
            delivery.check(&self.inner, None)?;
        }
        let response = self
            .inner
            .http
            .post(url)
            .header(ORIGIN, auth.origin.origin().ascii_serialization())
            .header(COOKIE, auth.cookie)
            .header(CONTENT_TYPE, "application/json")
            .body(bytes)
            .send()
            .await
            .map_err(map_network)?;
        let value = read_response(response, self.inner.limits).await?;
        let obj = value.as_object().ok_or(Error::InvalidFrame)?;
        if obj.get("type").and_then(Value::as_str) != Some("server-response") {
            return Err(Error::InvalidFrame);
        }
        if obj.get("rpcId").and_then(Value::as_str) != Some(rpc_id.as_str()) {
            return Err(Error::Correlation);
        }
        let result = obj
            .get("result")
            .and_then(Value::as_object)
            .ok_or(Error::InvalidFrame)?;
        match result.get("ok").and_then(Value::as_bool) {
            Some(false) => {
                let error = result
                    .get("error")
                    .and_then(Value::as_object)
                    .ok_or(Error::InvalidFrame)?;
                if !error.get("code").is_some_and(Value::is_string)
                    || !error.get("message").is_some_and(Value::is_string)
                    || !error.get("details").is_some_and(Value::is_object)
                {
                    return Err(Error::InvalidFrame);
                }
                Err(
                    match (endpoint, error.get("code").and_then(Value::as_str)) {
                        (
                            Endpoint::PluginSettings | Endpoint::PluginMutate,
                            Some("settings/conflict"),
                        ) => Error::SettingsConflict,
                        (
                            Endpoint::PluginSettings | Endpoint::PluginMutate,
                            Some("settings/rejected"),
                        ) => Error::SettingsRejected,
                        // A fork refusal before child creation is deterministic; the attach
                        // failure happens after creation and is therefore uncertain.
                        (Endpoint::ForkSession, Some("session/fork-unavailable")) => {
                            Error::ForkUnavailable
                        }
                        (Endpoint::ForkSession, Some("session/workspace-attach-failed")) => {
                            Error::ForkAttachFailed
                        }
                        // Terminal refusals keep their distinct meanings; every other code
                        // stays an unqualified remote failure with no retained message.
                        // A preset pick refused because the session already started: the
                        // seat announces this code, so it must survive as a named refusal
                        // rather than an unqualified remote failure.
                        (Endpoint::AgentPresetSelect, Some("agentPreset/locked")) => {
                            Error::RemoteCode("agentPreset/locked")
                        }
                        (Endpoint::UpdateQueue, Some("session/queue-item-not-found")) => {
                            Error::QueueItemNotFound
                        }
                        (
                            Endpoint::UpdateQueue,
                            Some("session/attachment-invalid") | Some("gateway/bad-request"),
                        ) => Error::QueueEditRejected,

                        (Endpoint::TerminalCreate, Some("terminal/limit-reached")) => {
                            Error::TerminalLimitReached
                        }
                        (
                            Endpoint::TerminalCreate
                            | Endpoint::TerminalWrite
                            | Endpoint::TerminalResize
                            | Endpoint::TerminalClose,
                            Some("terminal/unavailable"),
                        ) => Error::TerminalUnavailable,
                        (
                            Endpoint::TerminalWrite | Endpoint::TerminalResize,
                            Some("terminal/control-unavailable"),
                        ) => Error::TerminalControlUnavailable,
                        // The gateway's own context codes name a cause this app
                        // must tell apart: a request for a session whose context
                        // the gateway cannot resolve is a different defect from an
                        // unreadable reply, and only the code distinguishes them.
                        (_, Some("gateway/context-not-found")) => {
                            Error::RemoteCode("gateway/context-not-found")
                        }
                        (_, Some("gateway/context-unavailable")) => {
                            Error::RemoteCode("gateway/context-unavailable")
                        }
                        (_, Some("gateway/context-failed")) => {
                            Error::RemoteCode("gateway/context-failed")
                        }
                        (_, Some("gateway/arguments-invalid")) => {
                            Error::RemoteCode("gateway/arguments-invalid")
                        }
                        (_, Some("gateway/definition-unavailable")) => {
                            Error::RemoteCode("gateway/definition-unavailable")
                        }
                        _ => Error::RemoteFailure,
                    },
                )
            }
            Some(true) => {
                serde_json::from_value(result.get("value").cloned().unwrap_or(Value::Null))
                    .map_err(|_| Error::InvalidDto)
            }
            None => Err(Error::InvalidFrame),
        }
    }
    pub async fn session_list(&self) -> Result<SessionListValue> {
        self.invoke(Endpoint::SessionList, json!({"_request":{}}))
            .await
    }
    pub async fn session_create(
        &self,
        request: SessionCreateRequest,
    ) -> Result<SessionCreateValue> {
        if request.workspace_id.is_some() && request.cwd.is_some() {
            return Err(Error::InvalidDto);
        }
        self.invoke(Endpoint::SessionCreate, request_args(request)?)
            .await
    }
    pub async fn session_prompt(&self, request: SessionPromptRequest) -> Result<Accepted> {
        if !request.content.iter().any(|part| match part {
            PromptContentPart::Text { text } => !text.trim().is_empty(),
            _ => true,
        }) {
            return Err(Error::InvalidDto);
        }
        accepted(
            self.invoke(Endpoint::SessionPrompt, request_args(request)?)
                .await?,
        )
    }
    /// Read the original request identity without following/promoting an Agent or replaying a prompt.
    pub async fn session_prompt_admission(
        &self,
        request: crate::prompt_admission::PromptAdmissionRequest,
    ) -> Result<crate::prompt_admission::PromptAdmission> {
        crate::prompt_admission::project(
            self.invoke(Endpoint::PromptAdmission, request_args(request)?)
                .await?,
        )
    }
    pub async fn model_catalog(&self) -> Result<ModelCatalog> {
        self.invoke(Endpoint::ModelCatalog, json!({})).await
    }
    pub async fn select_model(&self, request: SelectModelRequest) -> Result<SelectModelValue> {
        self.invoke(Endpoint::SelectModel, request_args(request)?)
            .await
    }
    /// Read the commands the Host advertises for one session. A descriptor whose
    /// name is blank or carries a slash is not a command this app can address, so
    /// it is rejected rather than shown as invokable.
    pub async fn commands_list(&self, session_id: &SessionId) -> Result<Vec<CommandDescriptor>> {
        // Host contract: `commands/list: (agentId: SessionId)` — one flat named
        // argument, not a wrapped request object. Agent authority is resolved from
        // the registered Session; sending a differently shaped payload (for example
        // `{"request":{"sessionId":…}}`) is refused as gateway/arguments-invalid.
        let value: Vec<CommandDescriptor> = self
            .invoke(
                Endpoint::CommandsList,
                serde_json::json!({ "agentId": session_id }),
            )
            .await?;
        // A bounded list of non-empty names. Nothing here may reject a command
        // the Host actually offers: rejecting the list costs the user the whole
        // menu, so only what cannot be addressed at all is refused.
        if value.len() > MAX_COMMANDS || value.iter().any(|command| command.name.trim().is_empty())
        {
            return Err(Error::InvalidDto);
        }
        Ok(value)
    }
    /// Invoke one command without sending it to the model. The returned execution
    /// carries the handler's own outcome; a command the Host does not know settles
    /// as an error result, never as a silent success.
    pub async fn commands_execute(
        &self,
        request: &CommandExecuteRequest,
    ) -> Result<Option<CommandExecution>> {
        if request.line.trim().is_empty() || request.line.len() > MAX_COMMAND_LINE {
            return Err(Error::InvalidDto);
        }
        // Host contract: `commands/execute: (agentId, line, submittedAttachments)` —
        // flat named arguments, not a wrapped request object.
        let value: Option<CommandExecution> = self
            .invoke(
                Endpoint::CommandsExecute,
                serde_json::json!({
                    "agentId": request.session_id,
                    "line": request.line,
                    "submittedAttachments": request.submitted_attachments,
                }),
            )
            .await?;
        if let Some(execution) = &value
            && let CommandResult::Error { text } = &execution.result
            && text.trim().is_empty()
        {
            // An error with no text would render as an empty failure.
            return Err(Error::InvalidDto);
        }
        Ok(value)
    }
    /// Append one operator-owned Session title and return the Host-normalized value.
    /// An admitted failure or timeout is not proof of no write; never retry automatically.
    pub async fn session_rename(
        &self,
        request: &SessionRenameRequest,
    ) -> Result<SessionRenameValue> {
        let value: SessionRenameValue = self
            .invoke(Endpoint::RenameSession, request_args(request)?)
            .await?;
        // A durable event position cannot be negative and an accepted title is
        // never blank; reject a malformed ACK rather than publishing it.
        if value.seq < 0 || value.title.trim().is_empty() {
            return Err(Error::InvalidDto);
        }
        Ok(value)
    }
    /// Create one child Session from an exact inclusive source cut (or the latest
    /// completed-turn prefix when `atSeq` is omitted). The returned child id is the only
    /// acknowledged identity; an admitted failure is not proof that no child exists.
    pub async fn session_fork(&self, request: &SessionForkRequest) -> Result<SessionForkValue> {
        if request
            .at_seq
            .is_some_and(|seq| !(0..=MAX_SAFE_INTEGER as i64).contains(&seq))
        {
            return Err(Error::InvalidDto);
        }
        let value: SessionForkValue = self
            .invoke(Endpoint::ForkSession, request_args(request)?)
            .await?;
        // `SessionId` already refuses an empty or oversized value on deserialization; a
        // child equal to its own source is also impossible and must never be published.
        if value.session_id == request.session_id {
            return Err(Error::InvalidDto);
        }
        Ok(value)
    }
    /// Reserved one-operation lane. The BFF preserves queued inbox input; ACK is not idle.
    /// Mutate one pending queue item: edit its text, remove it, or steer it into the
    /// running turn. The receipt is the only acknowledgment; a missing item is a named
    /// refusal (`QueueItemNotFound`), never a reason to retry blind.
    pub async fn session_update_queue(
        &self,
        request: SessionUpdateQueueRequest,
    ) -> Result<Accepted> {
        validate_update_queue(&request)?;
        accepted(
            self.invoke(Endpoint::UpdateQueue, request_args(request)?)
                .await?,
        )
    }
    /// Full-text search over the Host's Session content. The query is trimmed and bounded; the
    /// reply's items and snippets are bounded too, and a malformed page is refused rather than
    /// truncated into confident nonsense.
    pub async fn session_search(&self, query: &str) -> Result<SessionSearchValue> {
        let query = query.trim();
        if query.is_empty() || query.chars().count() > 512 {
            return Err(Error::InvalidDto);
        }
        let value: SessionSearchValue = self
            .invoke(
                Endpoint::Search,
                request_args(SessionSearchRequest {
                    query: query.to_owned(),
                })?,
            )
            .await?;
        if value.items.len() > 20
            || value
                .items
                .iter()
                .any(|item| item.snippet.chars().count() > 240)
        {
            return Err(Error::InvalidDto);
        }
        Ok(value)
    }
    /// Read the Session's current goal view, or `None` when no goal is current.
    pub async fn goal_get(&self, session_id: &SessionId) -> Result<Option<GoalView>> {
        self.invoke(Endpoint::GoalGet, json!({"agentId": session_id.as_str()}))
            .await
    }
    /// Replace a goal's objective under its exact revision.
    pub async fn goal_edit(
        &self,
        session_id: &SessionId,
        reference: &GoalRef,
        objective: &str,
    ) -> Result<GoalView> {
        validate_goal_ref(reference)?;
        let objective = objective.trim();
        if objective.is_empty() || objective.len() > 4096 {
            return Err(Error::InvalidDto);
        }
        self.invoke(
            Endpoint::GoalEdit,
            json!({
                "agentId": session_id.as_str(),
                "ref": reference,
                "request": EditGoalRequest { objective: objective.to_owned() },
            }),
        )
        .await
    }
    /// Pause an active goal and disarm continuation.
    pub async fn goal_pause(
        &self,
        session_id: &SessionId,
        reference: &GoalRef,
    ) -> Result<GoalView> {
        validate_goal_ref(reference)?;
        self.invoke(
            Endpoint::GoalPause,
            json!({"agentId": session_id.as_str(), "ref": reference}),
        )
        .await
    }
    /// Resume a paused or blocked goal while its round budget allows.
    pub async fn goal_resume(
        &self,
        session_id: &SessionId,
        reference: &GoalRef,
    ) -> Result<GoalView> {
        validate_goal_ref(reference)?;
        self.invoke(
            Endpoint::GoalResume,
            json!({"agentId": session_id.as_str(), "ref": reference}),
        )
        .await
    }
    /// Clear the current goal under its exact revision; the receipt is the cleared ref.
    pub async fn goal_clear(&self, session_id: &SessionId, reference: &GoalRef) -> Result<GoalRef> {
        validate_goal_ref(reference)?;
        self.invoke(
            Endpoint::GoalClear,
            json!({"agentId": session_id.as_str(), "ref": reference}),
        )
        .await
    }
    /// The Host's whole schedule catalog, bounded to 4096 rows and validated for display.
    /// List `@` file-reference candidates for one session's working directory. The Host ranks
    /// them; native only bounds the reply.
    pub async fn file_references(
        &self,
        session_id: &SessionId,
        query: &str,
    ) -> Result<Vec<FileReferenceCandidate>> {
        if query.chars().count() > 512 || query.chars().any(char::is_control) {
            return Err(Error::InvalidDto);
        }
        let items: Vec<FileReferenceCandidate> = self
            .invoke(
                Endpoint::FileReferences,
                json!({
                    "agentId": session_id.as_str(),
                    "query": query,
                }),
            )
            .await?;
        bounded_file_references(items).ok_or(Error::InvalidDto)
    }
    /// Open one workspace path on the Host desktop through the canonical session method. The
    /// caller must intend the side effect: this launches the desktop's handler.
    pub async fn open_workspace_path(&self, path: &str) -> Result<bool> {
        let path = path.trim();
        if !valid_open_path(path) {
            return Err(Error::InvalidDto);
        }
        let value: Value = self
            .invoke(
                Endpoint::OpenWorkspacePath,
                json!({"request": {"path": path}}),
            )
            .await?;
        match value.get("opened").and_then(Value::as_bool) {
            Some(opened) => Ok(opened),
            None => Err(Error::InvalidDto),
        }
    }
    /// List `@` session-reference candidates for one session's query. Mentions are the canonical
    /// markdown form the Host expects in prompt text.
    pub async fn session_reference_candidates(
        &self,
        session_id: &SessionId,
        query: &str,
    ) -> Result<Vec<SessionReferenceCandidate>> {
        if query.chars().count() > 512 || query.chars().any(char::is_control) {
            return Err(Error::InvalidDto);
        }
        let items: Vec<SessionReferenceCandidate> = self
            .invoke(
                Endpoint::SessionReferences,
                json!({
                    "agentId": session_id.as_str(),
                    "query": query,
                }),
            )
            .await?;
        bounded_session_references(items).ok_or(Error::InvalidDto)
    }
    /// Open the Host desktop's native directory chooser. `None` means the operator cancelled.
    pub async fn pick_directory(&self) -> Result<Option<String>> {
        let value: Value = self.invoke(Endpoint::PickDirectory, json!({})).await?;
        match value {
            Value::Null => Ok(None),
            Value::String(path) if !path.is_empty() && path.len() <= 4096 => Ok(Some(path)),
            _ => Err(Error::InvalidDto),
        }
    }
    /// List one directory level for the in-app folder browser. `None` lists the Host home.
    pub async fn list_directory(&self, path: Option<&str>) -> Result<DirectoryListing> {
        if path.is_some_and(|path| {
            path.is_empty() || path.len() > 4096 || path.chars().any(char::is_control)
        }) {
            return Err(Error::InvalidDto);
        }
        // The Host's descriptor accepts an absent `path`; a present `null` fails its
        // boundary check, so the root request must omit the key entirely.
        let args = match path {
            Some(path) => json!({"path": path}),
            None => json!({}),
        };
        let value: Value = self.invoke(Endpoint::ListDirectory, args).await?;
        bounded_directory_listing(value).ok_or(Error::InvalidDto)
    }
    /// Read the current per-message feedback for one session.
    pub async fn message_feedback_list(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<MessageFeedbackItem>> {
        let value: Value = self
            .invoke(
                Endpoint::FeedbackList,
                json!({"request": {"sessionId": session_id.as_str()}}),
            )
            .await?;
        feedback_items(&value).ok_or(Error::InvalidDto)
    }
    /// Create or replace feedback for one assistant message after a version check.
    pub async fn message_feedback_put(
        &self,
        session_id: &SessionId,
        message_id: &str,
        rating: &str,
        if_version: Option<&str>,
        note: Option<&str>,
        category: Option<&str>,
    ) -> Result<MessageFeedbackPutOutcome> {
        if !valid_feedback_target(message_id, if_version)
            || !matches!(rating, "positive" | "negative")
            || note.is_some_and(|note| note.trim().is_empty() || note.len() > 4096)
            || category.is_some_and(|category| category.is_empty() || category.len() > 64)
        {
            return Err(Error::InvalidDto);
        }
        // The Host's boundary schema rejects explicit `null` for the two optional
        // strings; omitting the keys is what "absent keeps the current value" means.
        let mut request = serde_json::Map::new();
        request.insert("sessionId".into(), json!(session_id.as_str()));
        request.insert("messageId".into(), json!(message_id));
        request.insert("rating".into(), json!(rating));
        request.insert("ifVersion".into(), json!(if_version));
        if let Some(note) = note {
            request.insert("note".into(), json!(note));
        }
        if let Some(category) = category {
            request.insert("category".into(), json!(category));
        }
        let value: Value = self
            .invoke(Endpoint::FeedbackPut, json!({"request": request}))
            .await?;
        Ok(feedback_put_outcome(&value))
    }
    /// Delete feedback for one message after a version check.
    pub async fn message_feedback_delete(
        &self,
        session_id: &SessionId,
        message_id: &str,
        if_version: &str,
    ) -> Result<bool> {
        if !valid_feedback_target(message_id, Some(if_version)) {
            return Err(Error::InvalidDto);
        }
        let value: Value = self
            .invoke(
                Endpoint::FeedbackDelete,
                json!({"request": {
                    "sessionId": session_id.as_str(),
                    "messageId": message_id,
                    "ifVersion": if_version,
                }}),
            )
            .await?;
        match value.get("ok").and_then(Value::as_bool) {
            Some(true) => Ok(true),
            Some(false) => Ok(false),
            None => Err(Error::InvalidDto),
        }
    }
    /// One turn's changed-file summary from the authenticated GET route the deliverables card
    /// reads (`/api/changes.summary?sessionId&seq`). A 404 is an absent summary, not a transport
    /// failure: the caller shows no card.
    pub async fn changes_summary(
        &self,
        session_id: &SessionId,
        seq: u64,
    ) -> Result<ChangesSummary> {
        let value = self
            .get_json(
                "api/changes.summary",
                &[
                    ("sessionId", session_id.as_str()),
                    ("seq", &seq.to_string()),
                ],
            )
            .await?;
        parse_changes_summary(value).ok_or(Error::InvalidDto)
    }
    /// One listed file's turn-start/turn-end comparison from the same authenticated GET
    /// family (`/api/changes.diff?sessionId&seq&index`).
    pub async fn changes_diff(
        &self,
        session_id: &SessionId,
        seq: u64,
        index: usize,
    ) -> Result<ChangesDiff> {
        let index = index.to_string();
        let value = self
            .get_json(
                "api/changes.diff",
                &[
                    ("sessionId", session_id.as_str()),
                    ("seq", &seq.to_string()),
                    ("index", &index),
                ],
            )
            .await?;
        parse_changes_diff(value).ok_or(Error::InvalidDto)
    }
    async fn get_json(&self, path: &str, query: &[(&str, &str)]) -> Result<Value> {
        let auth = self.inner.authority()?;
        let mut url = auth.origin.join(path).map_err(|_| Error::InvalidLaunch)?;
        url.query_pairs_mut().extend_pairs(query.iter().copied());
        let response = self
            .inner
            .http
            .get(url)
            .header(ORIGIN, auth.origin.origin().ascii_serialization())
            .header(COOKIE, auth.cookie)
            .send()
            .await
            .map_err(map_network)?;
        read_response(response, self.inner.limits).await
    }
    pub async fn schedule_catalog(&self) -> Result<Vec<ScheduleCatalogItem>> {
        let items: Vec<ScheduleCatalogItem> =
            self.invoke(Endpoint::ScheduleCatalog, json!({})).await?;
        if items.len() > 4096
            || items.iter().any(|item| {
                item.id.is_empty()
                    || item.id.len() > 256
                    || item.title.is_empty()
                    || item.title.len() > 120
            })
        {
            return Err(Error::InvalidDto);
        }
        Ok(items)
    }
    /// Delete one reminder inside its original Session binding.
    pub async fn schedule_delete(
        &self,
        session_id: &SessionId,
        id: &str,
    ) -> Result<ScheduleDeleteOutcome> {
        if id.is_empty() || id.len() > 256 {
            return Err(Error::InvalidDto);
        }
        let value: Value = self
            .invoke(
                Endpoint::ScheduleDelete,
                json!({"request": {"sessionId": session_id.as_str(), "id": id}}),
            )
            .await?;
        schedule_delete_outcome(&value).ok_or(Error::InvalidDto)
    }
    /// Compare-and-update one task's title and/or prompt. `expected` is the complete record
    /// observed when editing began (the catalog item's reconstructed record), so the Host can
    /// refuse a concurrent change instead of overwriting it.
    pub async fn schedule_update(
        &self,
        session_id: &SessionId,
        expected: &Value,
        title: Option<&str>,
        prompt: Option<&str>,
        change: Option<&Value>,
    ) -> Result<ScheduleUpdateOutcome> {
        let Some(object) = expected.as_object() else {
            return Err(Error::InvalidDto);
        };
        let Some(id) = object.get("id").and_then(Value::as_str) else {
            return Err(Error::InvalidDto);
        };
        if id.is_empty() || id.len() > 256 || object.get("kind").and_then(Value::as_str).is_none() {
            return Err(Error::InvalidDto);
        }
        let title = title.map(str::trim).filter(|value| !value.is_empty());
        let prompt = prompt.map(str::trim).filter(|value| !value.is_empty());
        if title.is_none() && prompt.is_none() && change.is_none() {
            return Err(Error::InvalidDto);
        }
        if change.is_some_and(|value| !valid_timing_change(value)) {
            return Err(Error::InvalidDto);
        }
        if title.is_some_and(|value| value.len() > 120)
            || prompt.is_some_and(|value| value.len() > 4096)
        {
            return Err(Error::InvalidDto);
        }
        let mut request = serde_json::Map::new();
        request.insert("sessionId".into(), json!(session_id.as_str()));
        request.insert("id".into(), json!(id));
        request.insert("expected".into(), expected.clone());
        if let Some(title) = title {
            request.insert("title".into(), json!(title));
        }
        if let Some(prompt) = prompt {
            request.insert("prompt".into(), json!(prompt));
        }
        if let Some(change) = change {
            request.insert("change".into(), change.clone());
        }
        let value: Value = self
            .invoke(Endpoint::ScheduleUpdate, json!({"request": request}))
            .await?;
        schedule_update_outcome(&value).ok_or(Error::InvalidDto)
    }
    /// One bounded delivery-history page for a task in its original session binding.
    pub async fn schedule_history(
        &self,
        session_id: &SessionId,
        id: &str,
        limit: u64,
        before: Option<&str>,
    ) -> Result<ScheduleHistoryOutcome> {
        if id.is_empty()
            || id.len() > 256
            || limit == 0
            || limit > 100
            || before.is_some_and(|cursor| cursor.is_empty() || cursor.len() > 256)
        {
            return Err(Error::InvalidDto);
        }
        // The descriptor's `before` is an optional cursor: absent for the first
        // page, never `null` (the Host rejects that at its boundary).
        let mut request = serde_json::Map::new();
        request.insert("sessionId".into(), json!(session_id.as_str()));
        request.insert("id".into(), json!(id));
        request.insert("limit".into(), json!(limit));
        if let Some(before) = before {
            request.insert("before".into(), json!(before));
        }
        let value: Value = self
            .invoke(Endpoint::ScheduleHistory, json!({"request": request}))
            .await?;
        schedule_history_outcome(&value, limit).ok_or(Error::InvalidDto)
    }
    pub async fn session_cancel(&self, session_id: &SessionId) -> Result<Accepted> {
        accepted(
            self.invoke(
                Endpoint::Cancel,
                json!({"request":{"sessionId":session_id}}),
            )
            .await?,
        )
    }
    pub async fn session_page(&self, request: SessionPageRequest) -> Result<SessionPage> {
        validate_history(request.max_messages, request.turn_window.as_ref())?;
        if request.through_seq < -1
            || request.through_seq > MAX_SAFE_INTEGER as i64
            || request.before_seq.is_some_and(|seq| seq > MAX_SAFE_INTEGER)
        {
            return Err(Error::InvalidDto);
        }
        self.invoke(Endpoint::Page, request_args(request)?).await
    }
    /// Stage an agent preset for the session that starts next. The Host refuses a session
    /// that has already started with `agentPreset/locked`; the reply carries the committed
    /// identity. The call takes two flat named arguments, `agentId` (the descriptor's lookup
    /// parameter) and `agentPreset`, exactly like `commands/list`; a wrapped `request` body is
    /// refused as `gateway/arguments-invalid` before the registry runs
    /// (`agent-preset-registry/lib/typert.host.js`: `parameters[{wire:"agentId"},{wire:"agentPreset"}]`).
    pub async fn agent_preset_select(
        &self,
        session_id: &SessionId,
        agent_preset: &str,
    ) -> Result<String> {
        agent_preset_of(
            self.invoke(
                Endpoint::AgentPresetSelect,
                json!({"agentId": session_id, "agentPreset": agent_preset}),
            )
            .await?,
        )
    }

    /// Ask the Host to stop one background job. The receipt distinguishes a request the registry
    /// accepted from one that arrived after the job finished
    /// (`api/job-controller/src/types.ts:30-39`, endpoint `job/kill` per
    /// `lib/typert.host.js:130`).
    pub async fn job_kill(
        &self,
        session_id: &str,
        job_id: &str,
    ) -> Result<crate::dto::JobKillOutcome> {
        job_kill_outcome(
            self.invoke(
                Endpoint::JobKill,
                request_args(crate::dto::JobKillRequest {
                    session_id: session_id.to_owned(),
                    job_id: job_id.to_owned(),
                })?,
            )
            .await?,
        )
    }
    /// Metadata-only global Loader rows; no installation or enablement authority.
    pub async fn plugin_inventory(&self) -> Result<crate::plugin::PluginInventorySnapshot> {
        crate::plugin::project_inventory(self.invoke(Endpoint::PluginInventory, json!({})).await?)
    }
    /// Project private redacted config and Schemastery graphs before publishing any DTO.
    pub async fn plugin_settings(&self) -> Result<crate::plugin::SettingsDescribeValue> {
        crate::plugin::project_settings(self.invoke(Endpoint::PluginSettings, json!({})).await?)
    }
    /// Set one reviewed supported primitive with mandatory optimistic revision checking.
    /// Failure after admission is indeterminate; never automatically retry this write.
    pub async fn set_plugin_field(
        &self,
        namespace: &crate::plugin::SettingsNamespaceView,
        field: &crate::plugin::SettingsField,
        value: crate::plugin::SettingsScalar,
    ) -> Result<crate::plugin::SettingsNamespaceView> {
        let args = crate::plugin::mutation_args(namespace, field, &value)?;
        // Public DTO construction cannot grant authority to a hidden or secret field.
        let current = self.plugin_settings().await?;
        let current_namespace = current
            .namespaces
            .iter()
            .find(|n| n.ns == namespace.ns)
            .ok_or(Error::SettingsConflict)?;
        if current_namespace.revision != namespace.revision {
            return Err(Error::SettingsConflict);
        }
        if !current.writable
            || !current_namespace.auto_generate
            || !current_namespace.fields.iter().any(|f| {
                f.path == field.path
                    && f.kind == field.kind
                    && f.value == field.value
                    && f.value.as_ref().is_some_and(|v| f.kind.accepts(v))
            })
        {
            return Err(Error::SettingsRejected);
        }
        let result =
            crate::plugin::project_namespace(self.invoke(Endpoint::PluginMutate, args).await?)?;
        if result.ns != namespace.ns || result.revision < namespace.revision {
            return Err(Error::Correlation);
        }
        Ok(result)
    }
    pub async fn account_state(&self) -> Result<AccountView> {
        self.invoke(Endpoint::AccountState, json!({})).await
    }
    pub async fn workspace_create(&self, path: String) -> Result<WorkspaceCreateValue> {
        self.invoke(Endpoint::WorkspaceCreate, json!({"request":{"path":path}}))
            .await
    }
    /// Pin one known unarchived Session; return the complete global list in Host order.
    /// The receipt is not revisioned: keep workspace-follow state authoritative.
    pub async fn pin_session(
        &self,
        request: &SessionRegistryRequest,
    ) -> Result<PinnedSessionsValue> {
        self.invoke(Endpoint::PinSession, request_args(request)?)
            .await
    }
    /// Remove one pin idempotently; return the complete ordered global list.
    /// An admitted failure/timeout is not proof of no write; never retry automatically.
    pub async fn unpin_session(
        &self,
        request: &SessionRegistryRequest,
    ) -> Result<PinnedSessionsValue> {
        self.invoke(Endpoint::UnpinSession, request_args(request)?)
            .await
    }
    /// Archive without requesting any activity stop. Active work may be refused by Host.
    /// Logs remain retained; the global archive receipt must not replace follow state.
    pub async fn archive_session(
        &self,
        request: &SessionRegistryRequest,
    ) -> Result<ArchivedSessionsValue> {
        self.invoke(Endpoint::ArchiveSession, request_args(request)?)
            .await
    }
    /// Restore idempotently; return the complete global archive list, not a per-workspace set.
    /// Restoration does not restore a former pin and does not establish idle state.
    pub async fn unarchive_session(
        &self,
        request: &SessionRegistryRequest,
    ) -> Result<ArchivedSessionsValue> {
        self.invoke(Endpoint::UnarchiveSession, request_args(request)?)
            .await
    }
    pub async fn session_projections(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<ProjectionBaseline>> {
        self.invoke(
            Endpoint::Projections,
            json!({"request":{"sessionId":session_id}}),
        )
        .await
    }
    pub async fn answer_continued(
        &self,
        agent_id: &AgentId,
        call_id: &ToolCallId,
        answer: QuestionAnswer,
    ) -> Result<bool> {
        self.invoke(
            Endpoint::QuestionAnswer,
            json!({"agentId":agent_id,"callId":call_id,"answer":answer}),
        )
        .await
    }
    /// Legacy ID API captures the current intake instance when called, not at future polling.
    pub fn reply_approval<'a>(
        &'a self,
        event_id: &RemoteEventId,
        outcome: ApprovalOutcome,
    ) -> impl std::future::Future<Output = Result<()>> + Send + 'a + use<'a> {
        self.event_result(
            event_id,
            Some(WaterfallKind::Approval),
            json!({"kind":"result","value":outcome}),
        )
    }
    /// Legacy ID API captures the current intake instance synchronously; prefer worker-held authority.
    pub fn reply_question<'a>(
        &'a self,
        event_id: &RemoteEventId,
        answer: QuestionAnswer,
    ) -> impl std::future::Future<Output = Result<()>> + Send + 'a + use<'a> {
        self.event_result(
            event_id,
            Some(WaterfallKind::Question),
            json!({"kind":"result","value":answer}),
        )
    }
    pub fn delegate_event<'a>(
        &'a self,
        event_id: &RemoteEventId,
    ) -> impl std::future::Future<Output = Result<()>> + Send + 'a + use<'a> {
        self.event_result(event_id, None, json!({"kind":"next"}))
    }
    /// Reject only a currently correlated question waterfall with the fixed user-cancellation code.
    pub fn cancel_question<'a>(
        &'a self,
        event_id: &RemoteEventId,
    ) -> impl std::future::Future<Output = Result<()>> + Send + 'a + use<'a> {
        self.event_result(event_id, Some(WaterfallKind::Question), json!({"kind":"rejected","error":{
            "name":"UserQuestionError","message":"the user cancelled ask_user_question","code":"ASK_CANCELLED"}}))
    }
    pub fn question_timeout<'a>(
        &'a self,
        event_id: &RemoteEventId,
    ) -> impl std::future::Future<Output = Result<()>> + Send + 'a + use<'a> {
        self.event_result(event_id, Some(WaterfallKind::Question), json!({"kind":"rejected","error":{
            "name":"UserQuestionError","message":"ask_user_question timed out before the user answered","code":"ASK_TIMED_OUT"}}))
    }
    fn event_result<'a>(
        &'a self,
        event_id: &RemoteEventId,
        expected: Option<WaterfallKind>,
        outcome: Value,
    ) -> impl std::future::Future<Output = Result<()>> + Send + 'a + use<'a> {
        // Capture synchronously, before even the returned future is first polled.
        let captured = self
            .inner
            .pending
            .lock()
            .map_err(|_| Error::Closed)
            .and_then(|pending| pending.get(event_id).cloned().ok_or(Error::Correlation))
            .map(crate::EventDelivery::new);
        async move {
            let delivery = captured?;
            self.event_result_for(&delivery, expected, outcome).await
        }
    }
    async fn event_result_for(
        &self,
        delivery: &crate::EventDelivery,
        expected: Option<WaterfallKind>,
        outcome: Value,
    ) -> Result<()> {
        delivery.check(&self.inner, expected)?;
        tokio::select! {
            biased;
            _ = self.inner.lifetime.cancelled() => return Err(Error::Closed),
            _ = delivery.pending.lifetime.cancelled() => return Err(Error::Correlation),
            result = self.invoke_active_checked::<()>(Endpoint::EventResult,
                json!({"clientId":delivery.pending.client_id,"eventId":delivery.pending.event_id,"outcome":outcome}), Some(delivery)) => result?,
        }
        // ACK retires only this exact intake instance, never a reused ID's delivery.
        // It is not proof of answer/approval outcome or eventual tool execution.
        let mut pending = self.inner.pending.lock().map_err(|_| Error::Closed)?;
        if pending
            .get(&delivery.pending.event_id)
            .is_some_and(|current| Arc::ptr_eq(current, &delivery.pending))
        {
            pending.remove(&delivery.pending.event_id);
            delivery.pending.lifetime.cancel();
        }
        Ok(())
    }
    /// Reply to the exact worker-held approval delivery, not a fresh lookup by event ID.
    pub async fn reply_approval_for(
        &self,
        delivery: &crate::EventDelivery,
        outcome: ApprovalOutcome,
    ) -> Result<()> {
        self.event_result_for(
            delivery,
            Some(WaterfallKind::Approval),
            json!({"kind":"result","value":outcome}),
        )
        .await
    }
    /// Reply to the exact worker-held question delivery; caller separately validates its request/answer.
    pub async fn reply_question_for(
        &self,
        delivery: &crate::EventDelivery,
        answer: QuestionAnswer,
    ) -> Result<()> {
        self.event_result_for(
            delivery,
            Some(WaterfallKind::Question),
            json!({"kind":"result","value":answer}),
        )
        .await
    }
    /// Explicit fixed user cancellation for this exact question delivery; never a Turn cancellation.
    pub async fn cancel_question_for(&self, delivery: &crate::EventDelivery) -> Result<()> {
        self.event_result_for(delivery, Some(WaterfallKind::Question), json!({"kind":"rejected","error":{
            "name":"UserQuestionError","message":"the user cancelled ask_user_question","code":"ASK_CANCELLED"}})).await
    }
    /// List retained terminals for a displayed Session, including offline history.
    /// This never activates an Agent and creates no process.
    pub async fn terminal_list(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<crate::terminal::WebTerminalInfo>> {
        let items: Vec<crate::terminal::WebTerminalInfo> = self
            .invoke(Endpoint::TerminalList, json!({"sessionId":session_id}))
            .await?;
        crate::terminal::project_list(items)
    }
    /// Discover shells in the Session's execution environment. Read-only; spawns nothing.
    /// The Host selects the registered Agent; the manifest types this parameter as a Session identity.
    pub async fn terminal_shells(
        &self,
        agent_id: &SessionId,
    ) -> Result<Vec<crate::terminal::TerminalShell>> {
        let items: Vec<crate::terminal::TerminalShell> = self
            .invoke(Endpoint::TerminalShells, json!({"agentId":agent_id}))
            .await?;
        crate::terminal::project_shells(items)
    }
    /// Read the Host's actual input/dimension bounds without resolving a shell.
    pub async fn terminal_environment(
        &self,
        agent_id: &SessionId,
    ) -> Result<crate::terminal::TerminalEnvironment> {
        let environment: crate::terminal::TerminalEnvironment = self
            .invoke(Endpoint::TerminalEnvironment, json!({"agentId":agent_id}))
            .await?;
        crate::terminal::validate_environment(&environment)?;
        Ok(environment)
    }
    /// Ask the Host to allocate one user shell. The ACK identity must equal the requested
    /// identity; an admitted failure is not proof that no Host process exists.
    pub async fn terminal_create(
        &self,
        agent_id: &SessionId,
        request: &crate::terminal::TerminalCreateRequest,
    ) -> Result<crate::terminal::WebTerminalInfo> {
        crate::terminal::validate_create(request)?;
        let info: crate::terminal::WebTerminalInfo = self
            .invoke(
                Endpoint::TerminalCreate,
                json!({"agentId":agent_id,"request":request}),
            )
            .await?;
        crate::terminal::validate_info(&info)?;
        // Only an ACK for the exact requested identity may become visible terminal state.
        if info.id != request.id {
            return Err(Error::InvalidDto);
        }
        Ok(info)
    }
    /// Deliver one raw input write. Refusal is not proof the Host ignored it; never retry.
    pub async fn terminal_write(
        &self,
        agent_id: &SessionId,
        id: &crate::terminal::WebTerminalId,
        attachment_id: &crate::terminal::TerminalAttachmentId,
        data: &str,
    ) -> Result<()> {
        crate::terminal::validate_input(data)?;
        self.invoke(
            Endpoint::TerminalWrite,
            json!({"agentId":agent_id,"id":id,"attachmentId":attachment_id,"data":data}),
        )
        .await
    }
    /// Update the Host PTY dimensions for the current writable attachment.
    pub async fn terminal_resize(
        &self,
        agent_id: &SessionId,
        id: &crate::terminal::WebTerminalId,
        attachment_id: &crate::terminal::TerminalAttachmentId,
        cols: u64,
        rows: u64,
    ) -> Result<()> {
        crate::terminal::validate_dimensions(cols, rows)?;
        self.invoke(
            Endpoint::TerminalResize,
            json!({"agentId":agent_id,"id":id,"attachmentId":attachment_id,"cols":cols,"rows":rows}),
        )
        .await
    }
    /// Close one terminal and kill its process range idempotently. The Host, not the app,
    /// owns termination; an admitted failure/timeout is not proof no process remains.
    pub async fn terminal_close(
        &self,
        agent_id: &SessionId,
        id: &crate::terminal::WebTerminalId,
    ) -> Result<()> {
        self.invoke(Endpoint::TerminalClose, json!({"agentId":agent_id,"id":id}))
            .await
    }
    pub async fn connect_mux(&self) -> Result<Mux> {
        Mux::connect(self.inner.clone()).await
    }
}
fn accepted(value: Accepted) -> Result<Accepted> {
    if value.accepted {
        Ok(value)
    } else {
        Err(Error::InvalidDto)
    }
}
/// The kill receipt: a bare outcome string, or an object carrying one. Anything else is
/// refused rather than guessed.
fn job_kill_outcome(value: serde_json::Value) -> Result<crate::dto::JobKillOutcome> {
    let outcome = match &value {
        serde_json::Value::String(outcome) => outcome.as_str(),
        value => value
            .get("outcome")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default(),
    };
    match outcome {
        "requested" => Ok(crate::dto::JobKillOutcome::Requested),
        "already-finished" => Ok(crate::dto::JobKillOutcome::AlreadyFinished),
        _ => Err(Error::InvalidDto),
    }
}

/// The committed preset identity from an `agentPresets/select` reply: a bare string, or an
/// object carrying one. Anything else is refused rather than guessed.
fn agent_preset_of(value: serde_json::Value) -> Result<String> {
    let id = match &value {
        serde_json::Value::String(id) => Some(id.clone()),
        value => value
            .get("agentPreset")
            .or_else(|| value.get("id"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
    };
    id.filter(|id| !id.is_empty() && id.len() <= 512)
        .ok_or(crate::Error::InvalidDto)
}

/// Validate one queue mutation before it can leave the native queue. The Host enforces the
/// same rules (`api/session-controller/src/commands.ts:510-527`); rejecting here keeps a bad
/// edit from consuming an RPC slot.
fn validate_update_queue(request: &SessionUpdateQueueRequest) -> Result<()> {
    if request.item_id.is_empty()
        || request.item_id.len() > 512
        || request.item_id.chars().any(char::is_control)
    {
        return Err(Error::InvalidDto);
    }
    if let QueueAction::Edit { content } = &request.action {
        if content.is_empty() {
            return Err(Error::InvalidDto);
        }
        let mut total = 0usize;
        for block in content {
            if block.block_type != "text" || block.text.trim().is_empty() {
                return Err(Error::InvalidDto);
            }
            total = total
                .checked_add(block.text.len())
                .filter(|total| *total <= 32 * 1024)
                .ok_or(Error::InvalidDto)?;
        }
    }
    Ok(())
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryWire {
    id: String,
    records: Vec<ScheduleDeliveryRecord>,
    earlier_records_unavailable: bool,
    earlier_records_pruned: bool,
    retention: RetentionWire,
    #[serde(default)]
    next_before: Option<String>,
}
#[derive(serde::Deserialize)]
struct RetentionWire {
    days: u64,
    records: u64,
}
fn schedule_history_outcome(value: &Value, limit: u64) -> Option<ScheduleHistoryOutcome> {
    if let Ok(wire) = serde_json::from_value::<HistoryWire>(value.clone()) {
        if wire.records.len() > limit as usize {
            return None;
        }
        return Some(ScheduleHistoryOutcome::History(ScheduleHistory {
            id: wire.id,
            records: wire.records,
            earlier_records_unavailable: wire.earlier_records_unavailable,
            earlier_records_pruned: wire.earlier_records_pruned,
            retention_days: wire.retention.days,
            retention_records: wire.retention.records,
            next_before: wire.next_before,
        }));
    }
    value
        .as_object()
        .and_then(|object| object.get("code"))
        .and_then(Value::as_str)
        .map(|_| ScheduleHistoryOutcome::Missing)
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateWire {
    #[serde(default)]
    updated: Option<bool>,
    #[serde(default)]
    record: Option<UpdateRecordWire>,
    #[serde(default)]
    code: Option<String>,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateRecordWire {
    title: String,
    prompt: String,
}
/// The five timing-change shapes (`types.ts:402-407`), using the Host's snake_case input
/// fields. Bounds only; the Host validates the wall-clock/cron grammar and time zone.
fn valid_timing_change(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    let Some(kind) = object.get("kind").and_then(Value::as_str) else {
        return false;
    };
    match kind {
        "at" => object.get("at").is_some_and(|at| {
            at.as_str()
                .is_some_and(|text| !text.is_empty() && text.len() <= 64)
                || at.as_object().is_some_and(|local| {
                    local
                        .get("date")
                        .and_then(Value::as_str)
                        .is_some_and(|v| !v.is_empty() && v.len() <= 32)
                        && local
                            .get("time")
                            .and_then(Value::as_str)
                            .is_some_and(|v| !v.is_empty() && v.len() <= 32)
                        && local
                            .get("time_zone")
                            .and_then(Value::as_str)
                            .is_some_and(|v| !v.is_empty() && v.len() <= 64)
                })
        }),
        "every" => object
            .get("every_seconds")
            .and_then(Value::as_u64)
            .is_some_and(|seconds| (60..=u64::MAX).contains(&seconds)),
        "daily" => object.get("daily").is_some_and(|daily| {
            daily
                .get("time")
                .and_then(Value::as_str)
                .is_some_and(|value| !value.is_empty() && value.len() <= 32)
                && daily
                    .get("time_zone")
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.is_empty() && value.len() <= 64)
        }),
        "weekly" => object.get("weekly").is_some_and(|weekly| {
            weekly
                .get("time")
                .and_then(Value::as_str)
                .is_some_and(|value| !value.is_empty() && value.len() <= 32)
                && weekly
                    .get("time_zone")
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.is_empty() && value.len() <= 64)
                && weekly
                    .get("weekdays")
                    .and_then(Value::as_array)
                    .is_some_and(|days| {
                        (1..=7).contains(&days.len())
                            && days.iter().all(|day| matches!(day.as_u64(), Some(1..=7)))
                    })
        }),
        "cron" => object.get("cron").is_some_and(|cron| {
            cron.get("expression")
                .and_then(Value::as_str)
                .is_some_and(|value| !value.is_empty() && value.len() <= 256)
                && cron
                    .get("time_zone")
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.is_empty() && value.len() <= 64)
        }),
        _ => false,
    }
}
fn schedule_update_outcome(value: &Value) -> Option<ScheduleUpdateOutcome> {
    let wire: UpdateWire = serde_json::from_value(value.clone()).ok()?;
    match wire.updated {
        Some(true) => {
            let record = wire.record?;
            Some(ScheduleUpdateOutcome::Updated {
                title: record.title,
                prompt: record.prompt,
            })
        }
        Some(false) => Some(ScheduleUpdateOutcome::Missing),
        None => wire.code.map(|_| ScheduleUpdateOutcome::Refused),
    }
}
/// The delete union (`types.ts:345-350`): `{deleted:true}`, `{deleted:false,code}`, or a
/// non-mutating refusal carrying a code/message. Anything else is refused.
fn schedule_delete_outcome(value: &Value) -> Option<ScheduleDeleteOutcome> {
    let object = value.as_object()?;
    match object.get("deleted").and_then(Value::as_bool) {
        Some(true) => Some(ScheduleDeleteOutcome::Deleted),
        Some(false) => Some(ScheduleDeleteOutcome::Missing),
        None => object
            .get("code")
            .and_then(Value::as_str)
            .map(|_| ScheduleDeleteOutcome::Refused),
    }
}
fn parse_changes_summary(value: Value) -> Option<ChangesSummary> {
    let object = value.as_object()?;
    let number = |name: &str| object.get(name).and_then(Value::as_u64);
    let turn = number("turn")?;
    if turn == 0 {
        return None;
    }
    let total = number("total")?;
    let added = number("added")?;
    let deleted = number("deleted")?;
    let files = object
        .get("files")?
        .as_array()?
        .iter()
        .map(|file| {
            let file = file.as_object()?;
            let path = file.get("path")?.as_str()?;
            let display = file.get("display")?.as_str()?;
            if path.is_empty() || path.len() > 4096 || display.is_empty() || display.len() > 4096 {
                return None;
            }
            Some(ChangedFile {
                path: path.to_owned(),
                display: display.to_owned(),
                added: file.get("added")?.as_u64()?,
                deleted: file.get("deleted")?.as_u64()?,
                binary: file.get("binary").and_then(Value::as_bool).unwrap_or(false),
                oversized: file
                    .get("oversized")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    if files.len() > 4096 {
        return None;
    }
    Some(ChangesSummary {
        turn,
        files,
        total,
        added,
        deleted,
    })
}
fn valid_open_path(path: &str) -> bool {
    !path.is_empty() && path.len() <= 4096 && !path.chars().any(char::is_control)
}
fn bounded_session_references(
    items: Vec<SessionReferenceCandidate>,
) -> Option<Vec<SessionReferenceCandidate>> {
    if items.len() > 20
        || items.iter().any(|item| {
            item.session_id.is_empty()
                || item.session_id.len() > 256
                || item.label.is_empty()
                || item.label.len() > 256
                || item.mention.is_empty()
                || item.mention.len() > 4096
        })
    {
        return None;
    }
    Some(items)
}
fn bounded_file_references(
    items: Vec<FileReferenceCandidate>,
) -> Option<Vec<FileReferenceCandidate>> {
    if items.len() > 50
        || items
            .iter()
            .any(|item| item.path.is_empty() || item.path.len() > 4096)
        || items
            .iter()
            .any(|item| item.kind != "file" && item.kind != "directory")
    {
        return None;
    }
    Some(items)
}
fn parse_changes_diff(value: Value) -> Option<ChangesDiff> {
    let object = value.as_object()?;
    let path = object.get("path")?.as_str()?;
    let display = object.get("display")?.as_str()?;
    if path.is_empty() || path.len() > 4096 || display.is_empty() || display.len() > 4096 {
        return None;
    }
    match object.get("kind")?.as_str()? {
        "binary" => Some(ChangesDiff::Binary {
            path: path.to_owned(),
            display: display.to_owned(),
        }),
        "oversized" => Some(ChangesDiff::Oversized {
            path: path.to_owned(),
            display: display.to_owned(),
        }),
        "text" => {
            let before = object.get("before")?.as_bool()?;
            let after = object.get("after")?.as_bool()?;
            let coarse = object.get("coarse")?.as_bool()?;
            let hunks = object
                .get("hunks")?
                .as_array()?
                .iter()
                .map(|hunk| {
                    let hunk = hunk.as_object()?;
                    let lines = hunk
                        .get("lines")?
                        .as_array()?
                        .iter()
                        .map(|line| {
                            let line = line.as_str()?;
                            if !matches!(line.as_bytes().first(), Some(b'+' | b'-' | b' ')) {
                                return None;
                            }
                            Some(line.to_owned())
                        })
                        .collect::<Option<Vec<_>>>()?;
                    if lines.len() > 20_000 {
                        return None;
                    }
                    Some(ChangesHunk {
                        old_start: hunk.get("oldStart")?.as_u64()?,
                        old_lines: hunk.get("oldLines")?.as_u64()?,
                        new_start: hunk.get("newStart")?.as_u64()?,
                        new_lines: hunk.get("newLines")?.as_u64()?,
                        lines,
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            if hunks.len() > 4096 {
                return None;
            }
            Some(ChangesDiff::Text {
                path: path.to_owned(),
                display: display.to_owned(),
                before,
                after,
                coarse,
                hunks,
            })
        }
        _ => None,
    }
}
fn validate_goal_ref(reference: &GoalRef) -> Result<()> {
    if reference.id.is_empty() || reference.id.len() > 256 || reference.revision == 0 {
        return Err(Error::InvalidDto);
    }
    Ok(())
}
pub(crate) fn request_args(value: impl Serialize) -> Result<Value> {
    serde_json::to_value(value)
        .map(|request| json!({"request":request}))
        .map_err(|_| Error::InvalidDto)
}
// Alpha history.ts uses Number.isSafeInteger for every page/window coordinate.
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
/// A session's advertised command list is small; a large one is a malformed reply.
const MAX_COMMANDS: usize = 512;
/// One invocation line, matching the prompt ceiling the app already enforces.
const MAX_COMMAND_LINE: usize = 32 * 1024;
pub(crate) fn validate_history(max: Option<u64>, window: Option<&TurnWindow>) -> Result<()> {
    if max.is_some_and(|n| n == 0 || n > MAX_SAFE_INTEGER)
        || window.is_some_and(|w| {
            w.min_messages == 0
                || w.min_turns == 0
                || w.min_turns > MAX_SAFE_INTEGER
                || w.min_messages > max.unwrap_or(50)
        })
    {
        return Err(Error::InvalidDto);
    }
    Ok(())
}
fn map_network(error: reqwest::Error) -> Error {
    if error.is_timeout() {
        Error::Timeout
    } else {
        Error::Network
    }
}
pub(crate) async fn read_response(
    mut response: reqwest::Response,
    limits: Limits,
) -> Result<Value> {
    if response.status().as_u16() != 200 {
        return Err(Error::HttpStatus(response.status().as_u16()));
    }
    if response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(';').next())
        .map(str::trim)
        != Some("application/json")
    {
        return Err(Error::InvalidFrame);
    }
    if response
        .content_length()
        .is_some_and(|n| n > limits.max_result_bytes as u64)
    {
        return Err(Error::Oversize);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(map_network)? {
        if chunk.len() > limits.max_result_bytes.saturating_sub(bytes.len()) {
            return Err(Error::Oversize);
        }
        bytes.extend_from_slice(&chunk);
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| Error::InvalidJson)?;
    validate_json(&value, limits.max_json_items)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::agent_preset_of;
    use serde_json::json;

    #[test]
    fn a_select_reply_yields_the_committed_identity_or_is_refused() {
        // Both shapes the Host may use for one identity.
        assert_eq!(agent_preset_of(json!("cordis")).unwrap(), "cordis");
        assert_eq!(
            agent_preset_of(json!({"agentPreset": "cordis"})).unwrap(),
            "cordis"
        );
        assert_eq!(agent_preset_of(json!({"id": "cordis"})).unwrap(), "cordis");
        // Everything else is refused rather than guessed at.
        assert!(agent_preset_of(json!("")).is_err());
        assert!(agent_preset_of(json!({})).is_err());
        assert!(agent_preset_of(json!(7)).is_err());
        assert!(agent_preset_of(json!(null)).is_err());
        assert!(agent_preset_of(json!({"agentPreset": ""})).is_err());
        assert!(agent_preset_of(json!({"agentPreset": 7})).is_err());
        assert!(agent_preset_of(json!({"agentPreset": "x".repeat(513)})).is_err());
    }
}

#[cfg(test)]
mod job_kill_tests {
    use super::job_kill_outcome;
    use crate::dto::JobKillOutcome;
    use serde_json::json;

    #[test]
    fn a_kill_receipt_yields_one_of_the_two_outcomes_or_is_refused() {
        // Both shapes the Host may use for the receipt.
        assert_eq!(
            job_kill_outcome(json!("requested")).unwrap(),
            JobKillOutcome::Requested
        );
        assert_eq!(
            job_kill_outcome(json!({"outcome": "already-finished"})).unwrap(),
            JobKillOutcome::AlreadyFinished
        );
        // Everything else is refused rather than guessed, including the reference's own two
        // words in the wrong shape.
        assert!(job_kill_outcome(json!("")).is_err());
        assert!(job_kill_outcome(json!({})).is_err());
        assert!(job_kill_outcome(json!(7)).is_err());
        assert!(job_kill_outcome(json!(null)).is_err());
        assert!(job_kill_outcome(json!({"outcome": "requested-please"})).is_err());
        assert!(job_kill_outcome(json!({"outcome": 7})).is_err());
    }
}

#[cfg(test)]
mod search_tests {
    use crate::dto::{SessionId, SessionSearchItem, SessionSearchRequest, SessionSearchValue};

    #[test]
    fn a_search_request_serializes_to_the_canonical_shape() {
        assert_eq!(
            serde_json::to_value(SessionSearchRequest {
                query: "PUBLIC query".into()
            })
            .unwrap(),
            serde_json::json!({"query":"PUBLIC query"})
        );
        let page = SessionSearchValue {
            items: vec![SessionSearchItem {
                session_id: SessionId::new("PUBLIC-session").unwrap(),
                snippet: "PUBLIC snippet".into(),
            }],
            has_more: false,
        };
        assert_eq!(
            serde_json::to_value(page).unwrap(),
            serde_json::json!({"items":[{"sessionId":"PUBLIC-session","snippet":"PUBLIC snippet"}],
                "hasMore":false})
        );
    }
}

#[cfg(test)]
mod update_queue_tests {
    use super::validate_update_queue;
    use crate::dto::{QueueAction, QueueTextBlock, SessionId, SessionUpdateQueueRequest};
    use serde_json::json;

    fn request(item_id: &str, action: QueueAction) -> SessionUpdateQueueRequest {
        SessionUpdateQueueRequest {
            session_id: SessionId::new("PUBLIC-session").unwrap(),
            item_id: item_id.into(),
            action,
        }
    }

    fn edit(text: &str) -> SessionUpdateQueueRequest {
        request(
            "PUBLIC-item",
            QueueAction::Edit {
                content: vec![QueueTextBlock {
                    block_type: "text".into(),
                    text: text.into(),
                }],
            },
        )
    }

    #[test]
    fn a_queue_edit_serializes_to_the_canonical_shape() {
        assert_eq!(
            serde_json::to_value(edit("PUBLIC edit")).unwrap(),
            json!({
                "sessionId":"PUBLIC-session",
                "itemId":"PUBLIC-item",
                "action":{"kind":"edit","content":[{"type":"text","text":"PUBLIC edit"}]}
            })
        );
        assert_eq!(
            serde_json::to_value(request("PUBLIC-item", QueueAction::Remove)).unwrap()["action"],
            json!({"kind":"remove"})
        );
        assert_eq!(
            serde_json::to_value(request("PUBLIC-item", QueueAction::Steer)).unwrap()["action"],
            json!({"kind":"steer"})
        );
    }

    #[test]
    fn edits_are_text_only_nonempty_and_bounded() {
        assert!(validate_update_queue(&edit("PUBLIC edit")).is_ok());
        assert!(validate_update_queue(&request("PUBLIC-item", QueueAction::Remove)).is_ok());
        assert!(validate_update_queue(&request("PUBLIC-item", QueueAction::Steer)).is_ok());
        let empty = request(
            "PUBLIC-item",
            QueueAction::Edit {
                content: Vec::new(),
            },
        );
        assert!(validate_update_queue(&empty).is_err());
        let image = request(
            "PUBLIC-item",
            QueueAction::Edit {
                content: vec![QueueTextBlock {
                    block_type: "image".into(),
                    text: "PUBLIC".into(),
                }],
            },
        );
        assert!(validate_update_queue(&image).is_err());
        assert!(validate_update_queue(&edit("   \n\t")).is_err());
        assert!(validate_update_queue(&edit(&"x".repeat(32 * 1024))).is_ok());
        assert!(validate_update_queue(&edit(&"x".repeat(32 * 1024 + 1))).is_err());
    }

    #[test]
    fn queue_item_ids_are_bounded_and_control_free() {
        assert!(validate_update_queue(&edit("PUBLIC")).is_ok());
        for bad in ["", "line\nbreak", &"x".repeat(513)] {
            let mut request = edit("PUBLIC");
            request.item_id = bad.into();
            assert!(validate_update_queue(&request).is_err(), "{bad:?}");
        }
    }
}

#[cfg(test)]
mod goal_tests {
    use super::validate_goal_ref;
    use crate::dto::{GoalActivation, GoalPhase, GoalRef, GoalView};

    #[test]
    fn goal_refs_are_positive_and_bounded() {
        for (id, revision, ok) in [
            ("PUBLIC-goal", 1, true),
            ("PUBLIC-goal", 0, false),
            ("", 1, false),
            (&"x".repeat(257), 1, false),
        ] {
            let reference = GoalRef {
                id: id.into(),
                revision,
            };
            assert_eq!(
                validate_goal_ref(&reference).is_ok(),
                ok,
                "{id:?}/{revision}"
            );
        }
    }

    #[test]
    fn a_goal_view_round_trips_its_wire_shape() {
        let view = GoalView {
            id: "PUBLIC-goal".into(),
            revision: 2,
            objective: "PUBLIC objective".into(),
            phase: GoalPhase::Paused,
            blocked_reason: None,
            max_goal_rounds: 64,
            rounds_started: 3,
            created_at: 1000,
            updated_at: 2000,
            activation: GoalActivation::Disarmed,
        };
        let value = serde_json::to_value(&view).unwrap();
        assert_eq!(value["phase"], "paused");
        assert_eq!(value["blockedReason"], serde_json::Value::Null);
        assert_eq!(
            serde_json::from_value::<GoalView>(value).unwrap().revision,
            2
        );
    }
}

#[cfg(test)]
mod schedule_tests {
    use super::{ScheduleCatalogItem, schedule_delete_outcome};
    use crate::dto::ScheduleDeleteOutcome;
    use serde_json::json;

    #[test]
    fn the_delete_union_yields_deleted_missing_or_refused() {
        assert_eq!(
            schedule_delete_outcome(&json!({"id":"PUBLIC","deleted":true})),
            Some(ScheduleDeleteOutcome::Deleted)
        );
        assert_eq!(
            schedule_delete_outcome(&json!({"id":"PUBLIC","deleted":false,
                "code":"schedule_not_found"})),
            Some(ScheduleDeleteOutcome::Missing)
        );
        assert_eq!(
            schedule_delete_outcome(&json!({"code":"invalid_rule","message":"PUBLIC"})),
            Some(ScheduleDeleteOutcome::Refused)
        );
        for bad in [json!({}), json!({"deleted":"yes"}), json!(null), json!(7)] {
            assert_eq!(schedule_delete_outcome(&bad), None, "{bad}");
        }
    }

    #[test]
    fn timing_change_shapes_are_bounded_and_text_only() {
        use super::valid_timing_change;
        for good in [
            serde_json::json!({"kind":"daily","daily":{"time":"09:00:00","time_zone":"UTC"}}),
            serde_json::json!({"kind":"weekly","weekly":{"time":"09:00:00","time_zone":"UTC","weekdays":[1,3]}}),
            serde_json::json!({"kind":"every","every_seconds":60}),
            serde_json::json!({"kind":"cron","cron":{"expression":"0 9 * * *","time_zone":"UTC"}}),
            serde_json::json!({"kind":"at","at":"2026-10-17T09:00:00Z"}),
            serde_json::json!({"kind":"at","at":{"date":"2026-10-17","time":"09:00:00","time_zone":"UTC"}}),
        ] {
            assert!(valid_timing_change(&good), "{good}");
        }
        for bad in [
            serde_json::json!({"kind":"unknown"}),
            serde_json::json!({"kind":"every","every_seconds":59}),
            serde_json::json!({"kind":"daily","daily":{"time":"","time_zone":"UTC"}}),
            serde_json::json!({"kind":"weekly","weekly":{"time":"09:00:00","time_zone":"UTC","weekdays":[]}}),
            serde_json::json!({"kind":"cron","cron":{"expression":"","time_zone":"UTC"}}),
            serde_json::json!({"kind":"at","at":""}),
        ] {
            assert!(!valid_timing_change(&bad), "{bad}");
        }
    }

    #[test]
    fn a_schedule_update_parses_committed_missing_or_refused() {
        use super::schedule_update_outcome;
        use crate::dto::ScheduleUpdateOutcome;
        assert_eq!(
            schedule_update_outcome(&serde_json::json!({
                "id":"PUBLIC","updated":true,
                "record":{"id":"PUBLIC","title":"PUBLIC new","prompt":"PUBLIC prompt"}
            })),
            Some(ScheduleUpdateOutcome::Updated {
                title: "PUBLIC new".into(),
                prompt: "PUBLIC prompt".into(),
            })
        );
        assert_eq!(
            schedule_update_outcome(&serde_json::json!({
                "id":"PUBLIC","updated":false,"code":"schedule_conflict"
            })),
            Some(ScheduleUpdateOutcome::Missing)
        );
        assert_eq!(
            schedule_update_outcome(&serde_json::json!({
                "code":"invalid_rule","message":"PUBLIC"
            })),
            Some(ScheduleUpdateOutcome::Refused)
        );
        for bad in [serde_json::json!({}), serde_json::json!({"updated":true})] {
            assert_eq!(schedule_update_outcome(&bad), None, "{bad}");
        }
    }

    #[test]
    fn a_history_page_parses_or_reports_a_missing_task() {
        use super::schedule_history_outcome;
        use crate::dto::ScheduleHistoryOutcome;
        let page = serde_json::json!({
            "id":"PUBLIC-task",
            "records":[{"scheduledAt":"2026-10-16T09:00:00Z",
                "deliveredAt":"2026-10-16T09:00:01Z","messageId":"PUBLIC-m",
                "prompt":"PUBLIC prompt"}],
            "earlierRecordsUnavailable":true,"earlierRecordsPruned":false,
            "retention":{"days":30,"records":100},"nextBefore":"PUBLIC-m"
        });
        let Some(ScheduleHistoryOutcome::History(history)) = schedule_history_outcome(&page, 20)
        else {
            panic!("expected a history page");
        };
        assert_eq!(history.records.len(), 1);
        assert_eq!(history.retention_days, 30);
        assert!(history.earlier_records_unavailable);
        assert_eq!(history.next_before.as_deref(), Some("PUBLIC-m"));
        assert_eq!(
            schedule_history_outcome(
                &serde_json::json!({"id":"PUBLIC-task","code":"schedule_not_found"}),
                20
            ),
            Some(ScheduleHistoryOutcome::Missing)
        );
        assert_eq!(
            schedule_history_outcome(
                &serde_json::json!({"records":[1,2],"retention":{"days":1,"records":1}}),
                1
            ),
            None
        );
        assert_eq!(schedule_history_outcome(&serde_json::json!({}), 20), None);
    }

    #[test]
    fn a_catalog_row_ignores_the_timing_payload_and_parses_the_delivery_stamp() {
        let item: ScheduleCatalogItem = serde_json::from_value(json!({
            "id":"PUBLIC-task","title":"PUBLIC task","kind":"daily",
            "sessionId":"PUBLIC-session","status":"active",
            "daily":{"hour":9,"minute":0,"timeZone":"UTC"},
            "prompt":"PUBLIC prompt","scheduledAt":"2026-10-17T09:00:00Z",
            "lastDelivery":{"scheduledAt":"2026-10-16T09:00:00Z",
                "deliveredAt":"2026-10-16T09:00:01Z","messageId":"PUBLIC-message"}
        }))
        .unwrap();
        assert_eq!(item.id, "PUBLIC-task");
        assert_eq!(item.record["prompt"], "PUBLIC prompt");
        assert_eq!(item.session_id.as_str(), "PUBLIC-session");
        assert_eq!(
            item.last_delivery
                .as_ref()
                .map(|stamp| stamp.delivered_at.as_str()),
            Some("2026-10-16T09:00:01Z")
        );
    }
}

#[cfg(test)]
mod changes_tests {
    use super::parse_changes_summary;
    use serde_json::json;

    #[test]
    fn a_changes_diff_parses_text_binary_and_oversized() {
        use super::parse_changes_diff;
        use crate::dto::ChangesDiff;
        let text = parse_changes_diff(serde_json::json!({
            "kind":"text","path":"src/a.rs","display":"src/a.rs",
            "before":true,"after":true,"coarse":false,
            "hunks":[{"oldStart":1,"oldLines":1,"newStart":1,"newLines":1,
                "lines":["-old","+new"," context"]}]
        }))
        .unwrap();
        let ChangesDiff::Text { hunks, .. } = text else {
            panic!("text diff expected");
        };
        assert_eq!(hunks[0].lines.len(), 3);
        assert!(matches!(
            parse_changes_diff(serde_json::json!({"kind":"binary","path":"a","display":"a"})),
            Some(ChangesDiff::Binary { .. })
        ));
        assert!(matches!(
            parse_changes_diff(serde_json::json!({"kind":"oversized","path":"a","display":"a"})),
            Some(ChangesDiff::Oversized { .. })
        ));
        for bad in [
            serde_json::json!({}),
            serde_json::json!({"kind":"unknown","path":"a","display":"a"}),
            serde_json::json!({"kind":"text","path":"a","display":"a","before":true,
                "after":true,"coarse":false,
                "hunks":[{"oldStart":0,"oldLines":0,"newStart":0,"newLines":0,
                    "lines":["x"]}]}),
        ] {
            assert_eq!(parse_changes_diff(bad), None);
        }
    }

    #[test]
    fn a_changes_summary_parses_files_and_refuses_malformed_records() {
        let summary = parse_changes_summary(json!({
            "turn": 2, "total": 2, "added": 9, "deleted": 3,
            "files": [
                {"path":"src/a.rs","display":"src/a.rs","added":7,"deleted":1},
                {"path":"src/b.rs","display":"src/b.rs","added":2,"deleted":2,
                 "binary":true}
            ]
        }))
        .unwrap();
        assert_eq!(summary.turn, 2);
        assert_eq!(summary.files.len(), 2);
        assert!(summary.files[1].binary);
        for bad in [
            json!({}),
            json!({"turn":0,"total":0,"added":0,"deleted":0,"files":[]}),
            json!({"turn":1,"total":1,"added":0,"deleted":0,
                "files":[{"path":"","display":"x","added":0,"deleted":0}]}),
            json!({"turn":1,"total":1,"added":-1,"deleted":0,"files":[]}),
        ] {
            assert_eq!(parse_changes_summary(bad), None);
        }
    }
}

#[cfg(test)]
mod file_reference_tests {
    use super::bounded_file_references;
    use crate::dto::FileReferenceCandidate;

    fn candidate(path: &str, kind: &str) -> FileReferenceCandidate {
        FileReferenceCandidate {
            path: path.into(),
            kind: kind.into(),
        }
    }

    #[test]
    fn file_reference_candidates_are_bounded() {
        assert!(
            bounded_file_references(vec![
                candidate("src/main.rs", "file"),
                candidate("src/", "directory"),
            ])
            .is_some()
        );
        assert!(bounded_file_references(vec![candidate("", "file")]).is_none());
        assert!(bounded_file_references(vec![candidate("a", "socket")]).is_none());
        assert!(
            bounded_file_references(
                (0..51)
                    .map(|index| candidate(&format!("f{index}"), "file"))
                    .collect()
            )
            .is_none()
        );
    }
}

#[cfg(test)]
mod session_reference_tests {
    use super::bounded_session_references;
    use crate::dto::SessionReferenceCandidate;

    fn candidate(id: &str, mention: &str) -> SessionReferenceCandidate {
        SessionReferenceCandidate {
            session_id: id.into(),
            label: "PUBLIC".into(),
            display_title: None,
            mention: mention.into(),
        }
    }

    #[test]
    fn session_reference_candidates_are_bounded() {
        assert!(
            bounded_session_references(vec![candidate(
                "PUBLIC-session",
                "@[PUBLIC](dsh-session://abc)"
            )])
            .is_some()
        );
        assert!(bounded_session_references(vec![candidate("", "m")]).is_none());
        assert!(bounded_session_references(vec![candidate("s", "")]).is_none());
        assert!(
            bounded_session_references(
                (0..21)
                    .map(|index| candidate(&format!("s{index}"), "m"))
                    .collect()
            )
            .is_none()
        );
    }
}

#[cfg(test)]
mod open_path_tests {
    use super::valid_open_path;

    #[test]
    fn open_paths_are_bounded_and_control_free() {
        assert!(valid_open_path("src/main.rs"));
        assert!(!valid_open_path(""));
        assert!(!valid_open_path("src/\u{0007}.rs"));
        assert!(!valid_open_path(&"a".repeat(4097)));
    }
}

fn valid_feedback_target(message_id: &str, if_version: Option<&str>) -> bool {
    !message_id.is_empty()
        && message_id.len() <= 256
        && if_version.is_none_or(|version| !version.is_empty() && version.len() <= 256)
}
fn feedback_item(value: &Value) -> Option<MessageFeedbackItem> {
    let item: MessageFeedbackItem = serde_json::from_value(value.clone()).ok()?;
    if item.message_id.is_empty()
        || item.message_id.len() > 256
        || !matches!(item.rating.as_str(), "positive" | "negative")
        || item.version.is_empty()
        || item.version.len() > 256
    {
        return None;
    }
    Some(item)
}
fn feedback_items(value: &Value) -> Option<Vec<MessageFeedbackItem>> {
    if value.get("ok").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    let items = value
        .get("value")?
        .get("items")?
        .as_array()?
        .iter()
        .map(feedback_item)
        .collect::<Option<Vec<_>>>()?;
    if items.len() > 4096 {
        return None;
    }
    Some(items)
}
fn feedback_put_outcome(value: &Value) -> MessageFeedbackPutOutcome {
    match value.get("ok").and_then(Value::as_bool) {
        Some(true) => match value.get("value").and_then(feedback_item) {
            Some(item) => MessageFeedbackPutOutcome::Saved(item),
            None => MessageFeedbackPutOutcome::Refused("invalid-reply".into()),
        },
        Some(false) => {
            let Some(error) = value.get("error") else {
                return MessageFeedbackPutOutcome::Refused("invalid-reply".into());
            };
            let code = error
                .get("code")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            match code {
                "target-not-found" => MessageFeedbackPutOutcome::TargetMissing,
                "version-conflict" => MessageFeedbackPutOutcome::Conflict(
                    error
                        .get("current")
                        .filter(|current| !current.is_null())
                        .and_then(feedback_item),
                ),
                other => MessageFeedbackPutOutcome::Refused(other.to_owned()),
            }
        }
        None => MessageFeedbackPutOutcome::Refused("invalid-reply".into()),
    }
}

#[cfg(test)]
mod feedback_tests {
    use super::{feedback_items, feedback_put_outcome};
    use crate::dto::MessageFeedbackPutOutcome;

    fn item() -> serde_json::Value {
        serde_json::json!({
            "messageId":"PUBLIC-message","rating":"positive","version":"v1",
            "createdAt":1,"updatedAt":2
        })
    }

    #[test]
    fn feedback_lists_and_puts_parse_the_business_union() {
        let items = feedback_items(&serde_json::json!({
            "ok":true,"value":{"items":[item()]}
        }))
        .unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].rating, "positive");
        assert!(feedback_items(&serde_json::json!({"ok":false,"error":{"code":"x"}})).is_none());
        assert!(
            feedback_items(&serde_json::json!({"ok":true,"value":{"items":[{"messageId":""}]}}))
                .is_none()
        );
        match feedback_put_outcome(&serde_json::json!({"ok":true,"value":item()})) {
            MessageFeedbackPutOutcome::Saved(saved) => assert_eq!(saved.version, "v1"),
            other => panic!("{other:?}"),
        }
        match feedback_put_outcome(&serde_json::json!({
            "ok":false,"error":{"code":"version-conflict","current":item()}
        })) {
            MessageFeedbackPutOutcome::Conflict(Some(current)) => {
                assert_eq!(current.message_id, "PUBLIC-message");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            feedback_put_outcome(&serde_json::json!({
                "ok":false,"error":{"code":"target-not-found"}
            })),
            MessageFeedbackPutOutcome::TargetMissing
        );
        assert_eq!(
            feedback_put_outcome(&serde_json::json!({
                "ok":false,"error":{"code":"note-blank"}
            })),
            MessageFeedbackPutOutcome::Refused("note-blank".into())
        );
    }
}

fn bounded_directory_listing(value: Value) -> Option<DirectoryListing> {
    let listing: DirectoryListing = serde_json::from_value(value).ok()?;
    let valid = |entry: &DirectoryEntry| {
        !entry.name.is_empty()
            && entry.name.len() <= 512
            && !entry.path.is_empty()
            && entry.path.len() <= 4096
            && !entry.path.chars().any(char::is_control)
    };
    if listing.path.is_empty()
        || listing.path.len() > 4096
        || listing.home.is_empty()
        || listing.home.len() > 4096
        || listing.crumbs.len() > 256
        || !listing.crumbs.iter().all(valid)
        || listing.entries.len() > 4096
        || !listing.entries.iter().all(valid)
    {
        return None;
    }
    Some(listing)
}

#[cfg(test)]
mod directory_listing_tests {
    use super::bounded_directory_listing;

    fn entry(name: &str) -> serde_json::Value {
        serde_json::json!({"name":name,"path":format!("/{name}"),"hidden":false})
    }

    #[test]
    fn directory_listings_are_bounded() {
        let listing = bounded_directory_listing(serde_json::json!({
            "path":"/","home":"/home/PUBLIC",
            "crumbs":[entry("root")],"entries":[entry("src"),entry("docs")],
            "truncated":false
        }))
        .unwrap();
        assert_eq!(listing.entries.len(), 2);
        assert!(
            bounded_directory_listing(serde_json::json!({
                "path":"/","home":"/home/PUBLIC","crumbs":[],"entries":[{}],"truncated":false
            }))
            .is_none()
        );
        assert!(
            bounded_directory_listing(serde_json::json!({
                "path":"","home":"/home/PUBLIC","crumbs":[],"entries":[],"truncated":false
            }))
            .is_none()
        );
    }
}
