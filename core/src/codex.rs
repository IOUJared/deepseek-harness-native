use crate::Error;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodexPhase {
    Idle,
    WaitingBrowser,
    Authorized,
    Cancelled,
    Failed,
    Indeterminate,
}

/// Local fixed-flow metadata only. Presence never proves validity or a particular write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodexMetadata {
    pub available: bool,
    pub credential_stored: bool,
    pub route_configured: bool,
    pub phase: CodexPhase,
    pub attempt: Option<u64>,
    pub prompt: Option<u64>,
    pub browser_available: bool,
    pub settings_revision: Option<u64>,
    pub retry_blocked: bool,
}

pub(crate) const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
pub(crate) fn valid_id(value: u64) -> bool {
    value > 0 && value <= MAX_SAFE_INTEGER
}
fn optional_number(value: &Value, positive: bool) -> Result<Option<u64>, Error> {
    if value.is_null() {
        return Ok(None);
    }
    let number = value.as_u64().ok_or(Error::Protocol)?;
    if number > MAX_SAFE_INTEGER || positive && number == 0 {
        return Err(Error::Protocol);
    }
    Ok(Some(number))
}
pub(crate) fn metadata(value: &Value) -> Result<CodexMetadata, Error> {
    let boolean = |key| value[key].as_bool().ok_or(Error::Protocol);
    let phase = match value["phase"].as_str() {
        Some("idle") => CodexPhase::Idle,
        Some("waiting-browser") => CodexPhase::WaitingBrowser,
        Some("authorized") => CodexPhase::Authorized,
        Some("cancelled") => CodexPhase::Cancelled,
        Some("failed") => CodexPhase::Failed,
        Some("indeterminate") => CodexPhase::Indeterminate,
        _ => return Err(Error::Protocol),
    };
    let metadata = CodexMetadata {
        available: boolean("available")?,
        credential_stored: boolean("credentialStored")?,
        route_configured: boolean("routeConfigured")?,
        phase,
        attempt: optional_number(&value["attempt"], true)?,
        prompt: optional_number(&value["prompt"], true)?,
        browser_available: boolean("browserAvailable")?,
        settings_revision: optional_number(&value["settingsRevision"], false)?,
        retry_blocked: boolean("retryBlocked")?,
    };
    if (metadata.prompt.is_some() || metadata.browser_available) && metadata.attempt.is_none() {
        return Err(Error::Protocol);
    }
    Ok(metadata)
}

#[cfg(test)]
#[path = "codex_tests.rs"]
mod tests;
