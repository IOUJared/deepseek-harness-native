//! Non-activating prompt identity observations. Unknown/rejection never grants resend authority.
use crate::{
    Error, Result,
    dto::{SessionId, SessionRequestId},
};
use serde::Serialize;
use serde_json::{Map, Value};

const MAX_BYTES: usize = 16_384;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptAdmissionRequest {
    pub session_id: SessionId,
    pub request_id: SessionRequestId,
}
#[derive(Clone, PartialEq, Eq)]
pub struct PromptMessageId(String);
impl PromptMessageId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmissionEvidence {
    Memory,
    Inbox,
    Journal,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmissionPhase {
    Queued,
    Claimed,
    Admitted,
    Recorded,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmissionRejection {
    BadRequest,
    InvalidTimeZone,
    AttachmentInvalid,
    NotFound,
    AgentBusy,
    PromptClosed,
    Internal,
}
#[derive(Clone, PartialEq, Eq)]
pub enum PromptAdmission {
    Admitting,
    Accepted {
        evidence: AdmissionEvidence,
        phase: AdmissionPhase,
        message_id: Option<PromptMessageId>,
        seq: Option<u64>,
    },
    Rejected(AdmissionRejection),
    Unknown,
}
macro_rules! redacted {
    ($($ty:ty),*) => { $(impl std::fmt::Debug for $ty {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("PromptAdmission([REDACTED])") }
    })* };
}
redacted!(PromptAdmissionRequest, PromptMessageId, PromptAdmission);
fn exact(row: &Map<String, Value>, required: &[&str], optional: &[&str]) -> Result<()> {
    if required.iter().any(|k| !row.contains_key(*k))
        || row
            .keys()
            .any(|k| !required.contains(&k.as_str()) && !optional.contains(&k.as_str()))
    {
        return Err(Error::InvalidDto);
    }
    Ok(())
}
pub(crate) fn project(value: Value) -> Result<PromptAdmission> {
    if serde_json::to_vec(&value)
        .map_err(|_| Error::InvalidDto)?
        .len()
        > MAX_BYTES
    {
        return Err(Error::Oversize);
    }
    let row = value.as_object().ok_or(Error::InvalidDto)?;
    match row.get("state").and_then(Value::as_str) {
        Some("admitting") => {
            exact(row, &["state"], &[])?;
            Ok(PromptAdmission::Admitting)
        }
        Some("unknown") => {
            exact(row, &["state"], &[])?;
            Ok(PromptAdmission::Unknown)
        }
        Some("accepted") => {
            exact(row, &["state", "evidence", "phase"], &["messageId", "seq"])?;
            let evidence = match row["evidence"].as_str() {
                Some("memory") => AdmissionEvidence::Memory,
                Some("inbox") => AdmissionEvidence::Inbox,
                Some("journal") => AdmissionEvidence::Journal,
                _ => return Err(Error::InvalidDto),
            };
            let phase = match row["phase"].as_str() {
                Some("queued") => AdmissionPhase::Queued,
                Some("claimed") => AdmissionPhase::Claimed,
                Some("admitted") => AdmissionPhase::Admitted,
                Some("recorded") => AdmissionPhase::Recorded,
                _ => return Err(Error::InvalidDto),
            };
            let message_id = match row.get("messageId") {
                None => None,
                Some(value) => {
                    let id = value
                        .as_str()
                        .filter(|id| !id.is_empty() && id.len() <= 4096)
                        .ok_or(Error::InvalidDto)?;
                    Some(PromptMessageId(id.to_owned()))
                }
            };
            let seq = match row.get("seq") {
                None => None,
                Some(value) => Some(
                    value
                        .as_u64()
                        .filter(|n| *n <= MAX_SAFE_INTEGER)
                        .ok_or(Error::InvalidDto)?,
                ),
            };
            Ok(PromptAdmission::Accepted {
                evidence,
                phase,
                message_id,
                seq,
            })
        }
        Some("rejected") => {
            exact(row, &["state", "code"], &[])?;
            let code = match row["code"].as_str() {
                Some("gateway/bad-request") => AdmissionRejection::BadRequest,
                Some("session/invalid-time-zone") => AdmissionRejection::InvalidTimeZone,
                Some("session/attachment-invalid") => AdmissionRejection::AttachmentInvalid,
                Some("session/not-found") => AdmissionRejection::NotFound,
                Some("session/agent-busy") => AdmissionRejection::AgentBusy,
                Some("session/prompt-closed") => AdmissionRejection::PromptClosed,
                Some("gateway/internal") => AdmissionRejection::Internal,
                _ => return Err(Error::InvalidDto),
            };
            Ok(PromptAdmission::Rejected(code))
        }
        _ => Err(Error::InvalidDto),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn fixed_states_phases_and_zero_sequence_are_metadata_only() {
        for evidence in ["memory", "inbox", "journal"] {
            for phase in ["queued", "claimed", "admitted", "recorded"] {
                assert!(matches!(
                    project(
                        json!({"state":"accepted","evidence":evidence,"phase":phase,"messageId":"PUBLIC opaque / message","seq":0})
                    ),
                    Ok(PromptAdmission::Accepted { seq: Some(0), .. })
                ));
            }
        }
        assert_eq!(
            project(json!({"state":"unknown"})),
            Ok(PromptAdmission::Unknown)
        );
        assert_eq!(
            project(json!({"state":"admitting"})),
            Ok(PromptAdmission::Admitting)
        );
    }
    #[test]
    fn extra_grant_fields_missing_evidence_unknown_codes_and_unsafe_numbers_refuse() {
        for value in [
            json!([]),
            json!({"state":"accepted"}),
            json!({"state":"unknown","grant":"PRIVATE"}),
            json!({"state":"rejected","code":"PRIVATE"}),
            json!({"state":"accepted","evidence":"journal","phase":"recorded","seq":9007199254740992u64}),
            json!({"state":"accepted","evidence":"journal","phase":"recorded","seq":-1}),
            json!({"state":"accepted","evidence":"journal","phase":"recorded","messageId":""}),
            json!({"state":"accepted","evidence":"journal","phase":"recorded","messageId":null}),
            json!({"state":"accepted","evidence":"journal","phase":"recorded","seq":null}),
        ] {
            assert_eq!(project(value), Err(Error::InvalidDto));
        }
        assert_eq!(
            project(json!({"state":"unknown","PRIVATE":"x".repeat(MAX_BYTES)})),
            Err(Error::Oversize)
        );
    }
}
