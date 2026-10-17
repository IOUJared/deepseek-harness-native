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
id!(CommandId);
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
/// One operator-proposed Session title, serialized as exactly `{sessionId, title}`.
/// Extra authority (`agent`, `agentId`, `stopActivity`, `seq`) is rejected, not ignored.
/// One command invocation. The native app sends no attachments, and the Host
/// rejects an invocation whose command cannot accept them.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandExecuteRequest {
    pub session_id: SessionId,
    pub line: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub submitted_attachments: Vec<Value>,
}
/// Host-advertised command metadata. `name` is lowercase and carries no slash.
///
/// Unknown fields are **ignored**, not rejected. The Host's descriptor is a
/// plugin-owned shape under active development, and a command that advertises an
/// optional free-form `input` hint — precisely the case where completion matters
/// most — must not fail the whole list. Rejecting one unrecognized field cost the
/// composer its entire menu once already.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandDescriptor {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub definition_id: Option<String>,
    pub name: String,
    pub description: String,
}
/// One handler's normalized outcome. `success` may omit its text; `error` must
/// carry one, because the app shows that text rather than inventing a reason.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum CommandResult {
    Success {
        #[serde(skip_serializing_if = "Option::is_none", default)]
        text: Option<String>,
    },
    Error {
        text: String,
    },
}
/// A settled command execution: the pairing identity plus the outcome.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandExecution {
    pub command_id: CommandId,
    pub result: CommandResult,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionRenameRequest {
    pub session_id: SessionId,
    pub title: String,
}
/// Host-normalized title and the durable event seq that committed it.
/// The app applies this acknowledged value, never a locally predicted title.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionRenameValue {
    pub title: String,
    pub seq: i64,
}
/// One operator-requested Session fork, serialized as exactly `{sessionId, atSeq?}`.
/// `atSeq` is the inclusive source event cut; omitting it selects the latest
/// completed-turn prefix. Extra authority is rejected, not ignored.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionForkRequest {
    pub session_id: SessionId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at_seq: Option<i64>,
}
/// The new child Session identity, and the only authoritative source of that id.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionForkValue {
    pub session_id: SessionId,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_verbosity: Option<String>,
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
pub struct RequestControlChoice {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelServiceTier {
    pub tiers: Vec<RequestControlChoice>,
    pub default_tier: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelTextVerbosity {
    pub levels: Vec<RequestControlChoice>,
    pub default_level: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogModel {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub reasoning: Option<ModelReasoning>,
    pub service_tier: Option<ModelServiceTier>,
    pub text_verbosity: Option<ModelTextVerbosity>,
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
/// One pending queue mutation (`QueueAction` in `api/session-controller/src/types.ts:183-190`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum QueueAction {
    /// Replace the queued item's content with non-empty text-only blocks.
    Edit { content: Vec<QueueTextBlock> },
    /// Drop the queued item.
    Remove,
    /// Steer the queued item into the running turn.
    Steer,
}
/// One text block in a queue edit; the Host accepts no other block type.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueueTextBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: String,
}
/// One goal revision identity (`packages/goal/goal/src/types.ts:18-26`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GoalRef {
    pub id: String,
    pub revision: u64,
}
/// Durable continuation phase (`types.ts:39-44`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GoalPhase {
    Active,
    Paused,
    Blocked,
    Complete,
}
/// Process-local continuation eligibility (`types.ts:70-72`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GoalActivation {
    Armed,
    Disarmed,
}
/// A blocked goal's machine and human explanation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GoalBlockReason {
    pub code: String,
    pub message: String,
}
/// The goal view every mutation returns (`types.ts:90-107`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GoalView {
    pub id: String,
    pub revision: u64,
    pub objective: String,
    pub phase: GoalPhase,
    pub blocked_reason: Option<GoalBlockReason>,
    pub max_goal_rounds: u64,
    pub rounds_started: u64,
    pub created_at: i64,
    pub updated_at: i64,
    pub activation: GoalActivation,
}
/// Fields changed by one goal edit; a bounded objective only.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EditGoalRequest {
    pub objective: String,
}
/// One full-text Session search request (`api/session-controller/src/types.ts:297-305`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionSearchRequest {
    pub query: String,
}
/// One content search hit: the Session and a bounded snippet.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionSearchItem {
    pub session_id: SessionId,
    pub snippet: String,
}
/// One search page. `hasMore` reports a truncated result set, not another page cursor.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionSearchValue {
    pub items: Vec<SessionSearchItem>,
    pub has_more: bool,
}
/// One pending queue mutation for one Session item.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionUpdateQueueRequest {
    pub session_id: SessionId,
    pub item_id: String,
    pub action: QueueAction,
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
/// One background job as the Host reports it (`packages/jobs/jobs/src/view.ts:65-81`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobView {
    pub id: String,
    pub kind: String,
    /// The producer-supplied one-line label.
    pub label: String,
    #[serde(rename = "owner", skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// The lifecycle state: running, stopping, completed, killed or failed.
    pub status: String,
}

/// The `jobs/list` request: a required session whose visible set the frame replaces wholesale
/// (`packages/api/job-controller/src/types.ts:15-24`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobListRequest {
    #[serde(rename = "sessionId")]
    pub session_id: String,
}

/// One `jobs/list` frame: the complete set the session can see. **Whole-set replacement**, so a
/// reconnect's first frame is already the truth (`types.ts:19-27`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobListFrame {
    pub jobs: Vec<JobView>,
}

/// One output chunk as the Host reports it (`packages/jobs/jobs/src/view.ts:49-58`): an absolute
/// offset, the text as appended, an optional channel label, and whether bytes immediately before
/// it were lost.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobChunk {
    pub at: i64,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(rename = "gapBefore", skip_serializing_if = "Option::is_none")]
    pub gap_before: Option<bool>,
}

/// The `job/kill` request: the session whose job list carries the job, and the job
/// (`api/job-controller/src/types.ts:30-34`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobKillRequest {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "jobId")]
    pub job_id: String,
}

/// The registry's receipt after accepting a human kill request (`types.ts:36-39`): the only two
/// outcomes the reference distinguishes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JobKillOutcome {
    Requested,
    AlreadyFinished,
}

/// The `jobs/follow` request: an optional owning session, a required job, and an absolute byte
/// offset to resume from (`types.ts:49-60`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobFollowRequest {
    #[serde(rename = "sessionId", skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(rename = "jobId")]
    pub job_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u64>,
}

/// One `jobs/follow` frame: a job opening, its output, or a status change (`types.ts:72-89`).
/// Chunks now carry their own fields (`view.ts:49-58`), including `gapBefore`, so an evicted range
/// can be shown as a gap rather than joined silently.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum JobFollowFrame {
    Opened {
        job: JobView,
        from: i64,
    },
    Output {
        chunks: Vec<JobChunk>,
        next: i64,
        #[serde(skip_serializing_if = "Option::is_none")]
        lossy: Option<bool>,
    },
    Status {
        job: JobView,
    },
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

#[cfg(test)]
mod job_frame_tests {
    use super::{JobFollowFrame, JobListFrame, JobView};
    use serde_json::json;

    fn view() -> serde_json::Value {
        json!({"id": "shell-1", "kind": "shell", "label": "Run tests", "status": "running"})
    }

    #[test]
    fn a_list_frame_is_the_session_whole_set() {
        let frame: JobListFrame =
            serde_json::from_value(json!({"type": "rows", "jobs": [view()]})).unwrap();
        assert_eq!(frame.jobs.len(), 1);
        assert_eq!(frame.jobs[0].label, "Run tests");
        assert_eq!(frame.jobs[0].status, "running");
        assert_eq!(frame.jobs[0].owner, None);
        // An empty set is a valid whole-set replacement, not a missing field.
        let empty: JobListFrame =
            serde_json::from_value(json!({"type": "rows", "jobs": []})).unwrap();
        assert!(empty.jobs.is_empty());
    }

    #[test]
    fn every_follow_variant_round_trips_by_its_own_tag() {
        let opened: JobFollowFrame =
            serde_json::from_value(json!({"type": "opened", "job": view(), "from": 0})).unwrap();
        assert!(matches!(opened, JobFollowFrame::Opened { from: 0, .. }));
        let output: JobFollowFrame = serde_json::from_value(
            json!({"type": "output", "chunks": [{"at": 0, "text": "hi", "channel": "stdout"}], "next": 2, "lossy": true}),
        )
        .unwrap();
        match output {
            JobFollowFrame::Output {
                chunks,
                next,
                lossy,
            } => {
                assert_eq!(chunks.len(), 1);
                assert_eq!(next, 2);
                assert_eq!(lossy, Some(true));
            }
            other => panic!("expected output, got {other:?}"),
        }
        let status: JobFollowFrame =
            serde_json::from_value(json!({"type": "status", "job": view()})).unwrap();
        assert!(matches!(status, JobFollowFrame::Status { .. }));
        // A view without a label is refused rather than defaulted.
        assert!(
            serde_json::from_value::<JobView>(json!({"id": "a", "kind": "k", "status": "running"}))
                .is_err()
        );
    }
}
