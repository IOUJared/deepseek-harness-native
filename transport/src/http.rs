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
    ModelCatalog,
    SelectModel,
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
    PluginSettings,
    PluginMutate,
}
impl Endpoint {
    fn name(self) -> &'static str {
        match self {
            Self::SessionList => "session/list",
            Self::SessionCreate => "session/create",
            Self::SessionPrompt => "session/prompt",
            Self::ModelCatalog => "session/modelCatalog",
            Self::SelectModel => "session/selectModel",
            Self::Cancel => "session/cancel",
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
            Self::PluginSettings => "settings/describe",
            Self::PluginMutate => "settings/mutate",
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
        let _permit = self
            .inner
            .rpc_slots
            .acquire()
            .await
            .map_err(|_| Error::Closed)?;
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
    pub async fn model_catalog(&self) -> Result<ModelCatalog> {
        self.invoke(Endpoint::ModelCatalog, json!({})).await
    }
    pub async fn select_model(&self, request: SelectModelRequest) -> Result<SelectModelValue> {
        self.invoke(Endpoint::SelectModel, request_args(request)?)
            .await
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
pub(crate) fn request_args(value: impl Serialize) -> Result<Value> {
    serde_json::to_value(value)
        .map(|request| json!({"request":request}))
        .map_err(|_| Error::InvalidDto)
}
// Alpha history.ts uses Number.isSafeInteger for every page/window coordinate.
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
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
