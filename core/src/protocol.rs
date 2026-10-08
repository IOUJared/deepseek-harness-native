use crate::{
    AccountMetadata, AccountStatus, AttemptPhase, Command, Error, Inspection, OnboardingMetadata,
    PublicReply,
};
use serde_json::{Value, json};
use url::Url;

pub(crate) const PREFIX: &[u8] = b"DSH_TAURI:";
pub(crate) const MAX_LINE: usize = 65_536;

// Deliberately no Debug/Display/Serialize on process-private events or credentials.
pub(crate) struct PrivateEndpoint {
    pub origin: String,
    pub launch_url: String,
}
pub(crate) struct NativeAccountSessionTokenNativePrivateState {
    pub origin: String,
    pub token_value: String,
    pub request_headers: Vec<(String, String)>,
}
pub(crate) enum Event {
    Ready(PrivateEndpoint),
    Auth(Option<NativeAccountSessionTokenNativePrivateState>),
    Account(AccountMetadata),
    Codex(
        crate::CodexMetadata,
        Option<crate::browser::BrowserCapability>,
    ),
    Reply {
        id: String,
        command: Option<String>,
        reply: Result<PublicReply, Error>,
    },
    Acknowledged,
    Fatal,
    AccountSubscriptionError,
    Ignored,
}

fn launch(value: &str) -> Result<PrivateEndpoint, Error> {
    let url = Url::parse(value).map_err(|_| Error::Protocol)?;
    let query: Vec<_> = url.query_pairs().collect();
    if value.len() > 16_384
        || url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || url.port().is_none_or(|port| port == 0)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.fragment().is_some()
        || query.len() != 1
        || query[0].0 != "token"
        || query[0].1.is_empty()
    {
        return Err(Error::Protocol);
    }
    Ok(PrivateEndpoint {
        origin: url.origin().ascii_serialization(),
        launch_url: value.to_owned(),
    })
}

fn session(value: &Value) -> Result<Option<NativeAccountSessionTokenNativePrivateState>, Error> {
    if value.is_null() {
        return Ok(None);
    }
    let origin = value["origin"].as_str().ok_or(Error::Protocol)?;
    let url = Url::parse(origin).map_err(|_| Error::Protocol)?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || url.origin().ascii_serialization() != origin
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(Error::Protocol);
    }
    let token = value["token"].as_str().ok_or(Error::Protocol)?;
    if token.is_empty() || token.len() > 16_384 || !token.bytes().all(|b| (33..=126).contains(&b)) {
        return Err(Error::Protocol);
    }
    let mut headers = Vec::new();
    if let Some(raw) = value.get("requestHeaders") {
        let raw = raw.as_object().ok_or(Error::Protocol)?;
        if raw.len() > 32 {
            return Err(Error::Protocol);
        }
        for (key, val) in raw {
            let val = val.as_str().ok_or(Error::Protocol)?;
            if key.is_empty()
                || key.len() > 128
                || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                || val.len() > 4096
                || !val.bytes().all(|b| (32..=126).contains(&b))
            {
                return Err(Error::Protocol);
            }
            headers.push((key.clone(), val.to_owned()));
        }
    }
    Ok(Some(NativeAccountSessionTokenNativePrivateState {
        origin: origin.to_owned(),
        token_value: token.to_owned(),
        request_headers: headers,
    }))
}

/// Narrow projection: no links, authorization URL, attempt ID, user ID, free text, tokens, or extra keys.
fn account(value: &Value) -> Result<AccountMetadata, Error> {
    let status = match value["status"].as_str() {
        Some("signed-out") => AccountStatus::SignedOut,
        Some("credential-stored") => AccountStatus::CredentialStored,
        _ => return Err(Error::Protocol),
    };
    let attempt = value.get("attempt").ok_or(Error::Protocol)?;
    let phase = if attempt.is_null() {
        None
    } else {
        Some(match attempt["phase"].as_str() {
            Some("initializing") => AttemptPhase::Initializing,
            Some("waiting-browser") => AttemptPhase::WaitingBrowser,
            Some("exchanging") => AttemptPhase::Exchanging,
            Some("committing") => AttemptPhase::Committing,
            Some("succeeded") => AttemptPhase::Succeeded,
            Some("cancelled") => AttemptPhase::Cancelled,
            Some("expired") => AttemptPhase::Expired,
            Some("failed") => AttemptPhase::Failed,
            _ => return Err(Error::Protocol),
        })
    };
    Ok(AccountMetadata { status, phase })
}

fn inspection(value: &Value) -> Inspection {
    let active = value["activeTasks"].as_u64().filter(|n| *n <= 10000);
    let scheduled = value["scheduledTasks"].as_u64().filter(|n| *n <= 10000);
    Inspection {
        active_tasks: active.unwrap_or(0) as u32,
        scheduled_tasks: scheduled.unwrap_or(0) as u32,
        unknown: value["unknown"].as_bool().unwrap_or(true)
            || active.is_none()
            || scheduled.is_none(),
    }
}

fn public_result(command: &str, value: &Value) -> Result<PublicReply, Error> {
    match command {
        "codex-status" | "codex-start" | "codex-cancel" | "codex-enable-models" => {
            Ok(PublicReply::Codex(crate::codex::metadata(value)?))
        }
        "codex-callback" => Ok(PublicReply::CodexCallbackAccepted(
            value["ok"].as_bool().ok_or(Error::Protocol)?,
        )),
        "account-state" | "account-subscribe" => Ok(PublicReply::Account(account(value)?)),
        "account-unsubscribe" if value["subscribed"] == false => Ok(PublicReply::Unsubscribed),
        #[cfg(feature = "transport")]
        "discover-session-agent" => crate::agent_discovery::decode(value),
        "cancel-generation" => Ok(PublicReply::GenerationCancelled(
            value["cancelled"].as_bool().ok_or(Error::Protocol)?,
        )),
        "save-api-key" => Ok(PublicReply::ApiKeySaved(
            value["ok"].as_bool().ok_or(Error::Protocol)?,
        )),
        "onboarding-read" => Ok(PublicReply::Onboarding(OnboardingMetadata {
            logged_in: value["loggedIn"].as_bool().ok_or(Error::Protocol)?,
            has_api_key: value["hasApiKey"].as_bool().ok_or(Error::Protocol)?,
            writable: value["writable"].as_bool().ok_or(Error::Protocol)?,
        })),
        _ => Err(Error::Protocol),
    }
}

pub(crate) fn parse(bytes: &[u8], expected_version: &str) -> Result<Event, Error> {
    let Some(bytes) = bytes.strip_prefix(PREFIX) else {
        return Ok(Event::Ignored);
    };
    let value: Value = serde_json::from_slice(bytes).map_err(|_| Error::Protocol)?;
    let kind = value["type"].as_str().ok_or(Error::Protocol)?;
    Ok(match kind {
        "ready" if value["version"] == expected_version && value["profile"] == "desktop" => {
            Event::Ready(launch(value["url"].as_str().ok_or(Error::Protocol)?)?)
        }
        "ready" => return Err(Error::Protocol),
        "codex-state" => {
            let metadata = crate::codex::metadata(&value["value"])?;
            let browser = match &value["browserUrl"] {
                Value::Null => None,
                Value::String(raw)
                    if metadata.browser_available
                        && metadata.phase == crate::CodexPhase::WaitingBrowser
                        && !metadata.retry_blocked =>
                {
                    Some(crate::browser::BrowserCapability::new(raw.to_owned())?)
                }
                _ => return Err(Error::Protocol),
            };
            if metadata.browser_available != browser.is_some() {
                return Err(Error::Protocol);
            }
            Event::Codex(metadata, browser)
        }
        "platform-session" => Event::Auth(session(value.get("session").ok_or(Error::Protocol)?)?),
        "account-state" => Event::Account(account(&value["state"])?),
        "shutdown-complete" => Event::Acknowledged,
        "fatal" => Event::Fatal,
        "account-subscription-error" => Event::AccountSubscriptionError,
        "inspection" | "workspace-added" | "native-result" | "request-error" => {
            let id = value["requestId"]
                .as_str()
                .filter(|id| {
                    !id.is_empty()
                        && id.len() <= 128
                        && id
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                })
                .ok_or(Error::Protocol)?;
            let command = value["command"].as_str().map(str::to_owned);
            let reply = match kind {
                "inspection" => Ok(PublicReply::Inspection(inspection(&value))),
                "workspace-added" => Ok(PublicReply::WorkspaceAdded),
                "native-result" => {
                    public_result(command.as_deref().ok_or(Error::Protocol)?, &value["value"])
                }
                _ => Err(Error::RequestFailed),
            };
            Event::Reply {
                id: id.to_owned(),
                command,
                reply,
            }
        }
        _ => Event::Ignored,
    })
}

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::CodexStatus => "codex-status",
            Self::CodexStart => "codex-start",
            Self::CodexCancel { .. } => "codex-cancel",
            Self::CodexEnableModels { .. } => "codex-enable-models",
            Self::Inspect => "inspect",
            Self::OpenWorkspace(_) => "open-workspace",
            Self::OnboardingRead => "onboarding-read",
            Self::AccountState => "account-state",
            Self::AccountSubscribe => "account-subscribe",
            Self::AccountUnsubscribe => "account-unsubscribe",
            #[cfg(feature = "transport")]
            Self::DiscoverSessionAgent(_) => "discover-session-agent",
            Self::CancelGeneration(_) => "cancel-generation",
        }
    }
    pub(crate) fn encode(&self, id: &str) -> Result<Value, Error> {
        let mut value = json!({"type": self.name(), "requestId": id});
        match self {
            Self::CodexCancel { attempt } => {
                if !crate::codex::valid_id(*attempt) {
                    return Err(Error::InvalidCommand);
                }
                value["attempt"] = json!(attempt);
            }
            Self::CodexEnableModels { expected_revision } => {
                if *expected_revision > crate::codex::MAX_SAFE_INTEGER {
                    return Err(Error::InvalidCommand);
                }
                value["expectedRevision"] = json!(expected_revision);
            }
            Self::OpenWorkspace(path) => {
                if !crate::options::absolute(path) || path.as_os_str().len() > 32768 {
                    return Err(Error::InvalidCommand);
                }
                value["path"] = json!(path.to_str().ok_or(Error::InvalidCommand)?);
            }
            #[cfg(feature = "transport")]
            Self::DiscoverSessionAgent(session) => {
                if !crate::agent_discovery::valid_id(session.as_str()) {
                    return Err(Error::InvalidCommand);
                }
                value["sessionId"] = json!(session);
            }
            Self::CancelGeneration(id) => {
                if id.is_empty()
                    || id.len() > 512
                    || id.chars().any(|c| c.is_whitespace() || c.is_control())
                {
                    return Err(Error::InvalidCommand);
                }
                value["sessionId"] = json!(id);
            }
            _ => {}
        }
        Ok(value)
    }
}

pub(crate) fn matches_reply(name: &str, reply: &PublicReply) -> bool {
    #[cfg(feature = "transport")]
    if name == "discover-session-agent" {
        return matches!(reply, PublicReply::AgentDiscovery(_));
    }
    matches!(
        (name, reply),
        (
            "codex-status" | "codex-start" | "codex-cancel" | "codex-enable-models",
            PublicReply::Codex(_)
        ) | ("codex-callback", PublicReply::CodexCallbackAccepted(_))
            | ("inspect", PublicReply::Inspection(_))
            | ("open-workspace", PublicReply::WorkspaceAdded)
            | ("onboarding-read", PublicReply::Onboarding(_))
            | (
                "account-state" | "account-subscribe",
                PublicReply::Account(_)
            )
            | ("account-unsubscribe", PublicReply::Unsubscribed)
            | ("cancel-generation", PublicReply::GenerationCancelled(_))
            | ("save-api-key", PublicReply::ApiKeySaved(_))
    )
}
