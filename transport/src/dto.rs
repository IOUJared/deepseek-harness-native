//! Exact alpha BFF DTO fields; plugin-owned projection/event data remains bounded JSON.
//! Registered Agent authority cannot be fabricated by coercing a Session identity:
//! ```compile_fail
//! use dsh_native_transport::dto::{AgentId, SessionId};
//! fn wrong_authority(session: &SessionId) { let _: &AgentId = session; }
//! ```
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

macro_rules! id {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String")]
        pub struct $name(String);
        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self> {
                let value = value.into();
                if value.is_empty() || value.len() > 4096 {
                    return Err(Error::InvalidDto);
                }
                Ok(Self(value))
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl TryFrom<String> for $name {
            type Error = Error;
            fn try_from(value: String) -> Result<Self> {
                Self::new(value)
            }
        }
    };
}
id!(SessionId);
// Registered Agent authority, not an inferred session identifier.
id!(AgentId);
id!(WorkspaceId);
id!(SessionRequestId);
id!(ToolCallId);
id!(FileUploadReceiptId);
id!(AttachmentId);
id!(RemoteEventId);
id!(RemoteEventClientId);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub agent_available: bool,
    pub session_id: SessionId,
    pub updated_at: i64,
    pub running: bool,
    pub blank: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_session_id: Option<SessionId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<SubagentOrigin>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projections: Option<ProjectionHints>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubagentOrigin {
    Subagent,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectionKind {
    Cached,
    Sequenced,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectionHints {
    pub kind: ProjectionKind,
    pub as_of_seq: i64,
    pub values: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectionBaseline {
    pub as_of_seq: i64,
    pub values: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionListValue {
    pub items: Vec<SessionSummary>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionCreateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<WorkspaceId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<SessionId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_preset: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCreateValue {
    pub session_id: SessionId,
    pub agent_preset: Option<String>,
}
/// One operator-requested registry mutation. No Agent authority or activity-stop option.
///
/// JSON must be an object containing only `sessionId`; positional arrays and extra
/// `stopActivity`, `agent`, or `signal` fields are rejected rather than ignored.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionRegistryRequest {
    pub session_id: SessionId,
}
impl<'de> Deserialize<'de> for SessionRegistryRequest {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        Ok(Self {
            session_id: registry_field(deserializer, "sessionId")?,
        })
    }
}

/// Complete registry-global pin list, in Host order (most recently pinned first).
/// This receipt has no stream revision and must not replace a newer follow state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PinnedSessionsValue {
    pub pinned_session_ids: Vec<SessionId>,
}
impl<'de> Deserialize<'de> for PinnedSessionsValue {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        Ok(Self {
            pinned_session_ids: registry_field(deserializer, "pinnedSessionIds")?,
        })
    }
}

/// Complete registry-global archive list, preserving the Host's returned order.
/// Archival is not deletion; this receipt has no comparable follow revision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArchivedSessionsValue {
    pub archived_session_ids: Vec<SessionId>,
}
impl<'de> Deserialize<'de> for ArchivedSessionsValue {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        Ok(Self {
            archived_session_ids: registry_field(deserializer, "archivedSessionIds")?,
        })
    }
}

// Derive alone accepts positional struct arrays. This map visitor accepts exactly
// one named field and rejects duplicate/extra keys without echoing their contents.
fn registry_field<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
    name: &'static str,
) -> std::result::Result<T, D::Error> {
    struct Field<T>(&'static str, std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for Field<T> {
        type Value = T;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("an exact one-field registry object")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(
            self,
            mut map: M,
        ) -> std::result::Result<T, M::Error> {
            if map.next_key::<String>()?.as_deref() != Some(self.0) {
                return Err(serde::de::Error::custom("invalid registry object"));
            }
            let value = map.next_value()?;
            if map.next_key::<String>()?.is_some() {
                return Err(serde::de::Error::custom("invalid registry object"));
            }
            Ok(value)
        }
    }
    deserializer.deserialize_map(Field(name, std::marker::PhantomData))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSelection {
    pub provider: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectModelRequest {
    pub session_id: SessionId,
    #[serde(flatten)]
    pub selection: ModelSelection,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelectModelValue {
    pub selected: ModelSelection,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReasoningEffort {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelReasoning {
    pub efforts: Vec<ReasoningEffort>,
    pub default_effort: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogModel {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub reasoning: Option<ModelReasoning>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelGroup {
    pub id: String,
    pub name: String,
    pub models: Vec<CatalogModel>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogFailure {
    pub id: String,
    pub name: String,
    pub message: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalog {
    pub default: ModelSelection,
    pub routable_providers: Vec<String>,
    pub groups: Vec<ModelGroup>,
    pub failures: Vec<CatalogFailure>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptMode {
    Queue,
    Steer,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ImageMediaType {
    #[serde(rename = "image/png")]
    Png,
    #[serde(rename = "image/jpeg")]
    Jpeg,
    #[serde(rename = "image/webp")]
    Webp,
    #[serde(rename = "image/gif")]
    Gif,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PromptContentPart {
    Text {
        text: String,
    },
    Image {
        #[serde(rename = "mediaType")]
        media_type: ImageMediaType,
        data: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    File {
        #[serde(rename = "receiptId")]
        receipt_id: FileUploadReceiptId,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPromptRequest {
    pub request_id: SessionRequestId,
    pub session_id: SessionId,
    pub mode: PromptMode,
    pub content: Vec<PromptContentPart>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_time_zone: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Accepted {
    pub accepted: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SessionAddress {
    Session {
        #[serde(rename = "sessionId")]
        session_id: SessionId,
    },
    Subagent {
        #[serde(rename = "parentSessionId")]
        parent_session_id: SessionId,
        #[serde(rename = "childSessionId")]
        child_session_id: SessionId,
        mode: SubagentMode,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubagentMode {
    OneShot,
    Continuable,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnWindow {
    pub min_messages: u64,
    pub min_turns: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionFollowRequest {
    pub address: SessionAddress,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_messages: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_window: Option<TurnWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assistant_stream: Option<bool>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPageRequest {
    pub address: SessionAddress,
    pub through_seq: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_seq: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_messages: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_window: Option<TurnWindow>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionWireHeader {
    pub version: u64,
    pub id: SessionId,
    pub created_at: i64,
    pub cwd: Option<String>,
    pub parent_session: Option<SessionId>,
    pub is_seeded: bool,
    pub origin: Option<SubagentOrigin>,
    pub delegation_depth: Option<u64>,
    pub agent_preset: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionWireEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub seq: u64,
    pub time: i64,
    pub data: Value,
    pub ignorable: Option<bool>,
    pub source_event_seqs: Option<Value>,
    pub surface_op: Option<Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum HistoryRecord {
    Event { event: SessionWireEvent },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPage {
    pub records: Vec<HistoryRecord>,
    pub has_more: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantAttempt {
    pub attempt_id: String,
    pub started_after_seq: i64,
    pub turn: u64,
    pub step: u64,
    pub next_index: u64,
    pub stream: Vec<Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantBaseline {
    pub revision: u64,
    pub active_attempt: Option<AssistantAttempt>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum AssistantOutcome {
    Committed {
        #[serde(rename = "eventType")]
        event_type: AssistantSettlement,
        seq: u64,
    },
    Abandoned,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AssistantSettlement {
    #[serde(rename = "assistant/message")]
    Message,
    #[serde(rename = "assistant/attempt")]
    Attempt,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AssistantFrame {
    Start {
        #[serde(rename = "attemptId")]
        attempt_id: String,
        revision: u64,
        #[serde(rename = "startedAfterSeq")]
        started_after_seq: i64,
        turn: u64,
        step: u64,
    },
    Chunk {
        #[serde(rename = "attemptId")]
        attempt_id: String,
        revision: u64,
        index: u64,
        time: i64,
        chunk: Value,
    },
    End {
        #[serde(rename = "attemptId")]
        attempt_id: String,
        revision: u64,
        index: u64,
        outcome: AssistantOutcome,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SessionFollowFrame {
    #[serde(rename = "snapshot")]
    Snapshot {
        header: SessionWireHeader,
        cursor: i64,
        records: Vec<HistoryRecord>,
        #[serde(rename = "hasMore")]
        has_more: bool,
        projections: ProjectionBaseline,
        #[serde(rename = "assistantStream")]
        assistant_stream: Option<AssistantBaseline>,
    },
    #[serde(rename = "event")]
    Event { event: SessionWireEvent },
    #[serde(rename = "assistant-stream")]
    AssistantStream { frame: AssistantFrame },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceView {
    pub workspace_id: WorkspaceId,
    pub path: String,
    pub title: String,
    pub session_ids: Vec<SessionId>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceBaseline {
    pub items: Vec<WorkspaceView>,
    pub archived_session_ids: Vec<SessionId>,
    pub pinned_session_ids: Vec<SessionId>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum WorkspaceFollowFrame {
    Baseline {
        value: WorkspaceBaseline,
    },
    Upsert {
        workspace: WorkspaceView,
    },
    Remove {
        #[serde(rename = "workspaceId")]
        workspace_id: WorkspaceId,
    },
    Order {
        #[serde(rename = "workspaceIds")]
        workspace_ids: Vec<WorkspaceId>,
    },
    Archived {
        #[serde(rename = "archivedSessionIds")]
        archived_session_ids: Vec<SessionId>,
    },
    Pinned {
        #[serde(rename = "pinnedSessionIds")]
        pinned_session_ids: Vec<SessionId>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceCreateValue {
    pub workspace: WorkspaceView,
    pub created: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ControlFrame {
    Baseline {
        value: ControlBaseline,
    },
    Projection {
        #[serde(rename = "sessionId")]
        session_id: SessionId,
        key: String,
        value: Value,
        seq: u64,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ControlBaseline {
    pub projections: BTreeMap<SessionId, ProjectionBaseline>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemoteHost {
    pub home: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WaterfallKind {
    #[serde(rename = "approval/request")]
    Approval,
    #[serde(rename = "user-questions/request")]
    Question,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RemoteEventFrame {
    Ready {
        #[serde(rename = "clientId")]
        client_id: RemoteEventClientId,
        host: RemoteHost,
    },
    Emit {
        event: String,
        args: Vec<Value>,
    },
    Waterfall {
        event: WaterfallKind,
        #[serde(rename = "eventId")]
        event_id: RemoteEventId,
        #[serde(rename = "agentId")]
        agent_id: AgentId,
        request: Value,
    },
    Cancel {
        #[serde(rename = "eventId")]
        event_id: RemoteEventId,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApprovalOutcome {
    AllowedOnce,
    Rejected,
    Cancelled,
    Unavailable,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuestionAnswerItem {
    pub id: String,
    pub selected: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuestionAnswer {
    pub answers: Vec<QuestionAnswerItem>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionWait {
    pub remaining_ms: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccountStatus {
    SignedOut,
    CredentialStored,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SignInPhase {
    Initializing,
    WaitingBrowser,
    Exchanging,
    Committing,
    Succeeded,
    Cancelled,
    Expired,
    Failed,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SignInError {
    NoResponse,
    Network,
    Protocol,
    Expired,
    Storage,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignInAttempt {
    pub id: String,
    pub phase: SignInPhase,
    pub authorize_url: Option<String>,
    pub expires_at: Option<i64>,
    pub error_code: Option<SignInError>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountLinks {
    pub usage_url: String,
    pub top_up_url: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountView {
    pub status: AccountStatus,
    pub links: AccountLinks,
    pub attempt: Option<SignInAttempt>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileAttachmentRef {
    #[serde(rename = "attachmentId")]
    pub attachment_id: AttachmentId,
    pub name: String,
    pub bytes: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileUploadValue {
    #[serde(rename = "receiptId")]
    pub receipt_id: FileUploadReceiptId,
    pub file: FileAttachmentRef,
}
