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

/// Backend-random UUIDv4 account identity, never an issuer id or credential.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct CodexAccountId(String);
impl CodexAccountId {
    pub fn new(value: String) -> Result<Self, Error> {
        let raw = value.as_bytes();
        if raw.len() != 44 || !value.starts_with("account-") {
            return Err(Error::InvalidCommand);
        }
        let uuid = &raw[8..];
        if uuid[14] != b'4'
            || !matches!(uuid[19], b'8' | b'9' | b'a' | b'b')
            || uuid.iter().enumerate().any(|(i, b)| {
                if matches!(i, 8 | 13 | 18 | 23) {
                    *b != b'-'
                } else {
                    !b.is_ascii_digit() && !(b'a'..=b'f').contains(b)
                }
            })
        {
            return Err(Error::InvalidCommand);
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for CodexAccountId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CodexAccountId([REDACTED])")
    }
}

/// Human-supplied display label; no control, format, surrogate or line/paragraph separators.
#[derive(Clone, PartialEq, Eq)]
pub struct CodexAccountLabel(String);
impl CodexAccountLabel {
    pub fn new(value: String) -> Result<Self, Error> {
        if value.is_empty()
            || value.len() > 512
            || value.chars().count() > 128
            || value.trim().is_empty()
            || value.chars().any(|c| {
                c.is_control()
                    || matches!(c as u32,
                0x00ad | 0x0600..=0x0605 | 0x061c | 0x06dd | 0x070f | 0x0890..=0x0891 |
                0x08e2 | 0x180e | 0x200b..=0x200f | 0x2028..=0x202e | 0x2060..=0x2064 |
                0x2066..=0x206f | 0xfeff | 0xfff9..=0xfffb | 0x110bd | 0x110cd |
                0x13430..=0x1343f | 0x1bca0..=0x1bca3 | 0x1d173..=0x1d17a |
                0xe0001 | 0xe0020..=0xe007f)
            })
        {
            return Err(Error::InvalidCommand);
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for CodexAccountLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CodexAccountLabel([REDACTED])")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodexAccountMode {
    Managed,
    Legacy,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodexLegacyGrant {
    Unadopted,
    Absent,
}
/// Public account metadata; unknown issuer facts are null, not decoded from credentials.
#[derive(Clone, PartialEq, Eq)]
pub struct CodexAccount {
    pub opaque_id: CodexAccountId,
    pub label: CodexAccountLabel,
    pub email: Option<String>,
    pub plan: Option<String>,
}
impl std::fmt::Debug for CodexAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CodexAccount([REDACTED])")
    }
}
/// Exact durable Add receipt, fenced to its native attempt. Add does not select this account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexAddedAccount {
    pub account_id: CodexAccountId,
    pub epoch: u64,
    pub attempt: u64,
}
/// Non-Copy account projection, separate from the existing fixed-flow metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexAccountsMetadata {
    pub mode: CodexAccountMode,
    pub accounts: Vec<CodexAccount>,
    pub selected: Option<CodexAccountId>,
    pub epoch: Option<u64>,
    pub legacy: CodexLegacyGrant,
    pub added: Option<CodexAddedAccount>,
}
fn exact(value: &Value, keys: &[&str]) -> Result<(), Error> {
    let row = value.as_object().ok_or(Error::Protocol)?;
    if row.len() != keys.len() || keys.iter().any(|key| !row.contains_key(*key)) {
        return Err(Error::Protocol);
    }
    Ok(())
}
fn account_id(value: &Value) -> Result<CodexAccountId, Error> {
    CodexAccountId::new(value.as_str().ok_or(Error::Protocol)?.to_owned())
        .map_err(|_| Error::Protocol)
}
pub(crate) fn accounts_metadata(value: &Value) -> Result<CodexAccountsMetadata, Error> {
    exact(
        value,
        &["mode", "accounts", "selected", "epoch", "legacy", "added"],
    )?;
    let mode = match value["mode"].as_str() {
        Some("managed") => CodexAccountMode::Managed,
        Some("legacy") => CodexAccountMode::Legacy,
        _ => return Err(Error::Protocol),
    };
    let legacy = match value["legacy"].as_str() {
        Some("unadopted") => CodexLegacyGrant::Unadopted,
        Some("absent") => CodexLegacyGrant::Absent,
        _ => return Err(Error::Protocol),
    };
    let raw = value["accounts"].as_array().ok_or(Error::Protocol)?;
    if raw.len() > 128
        || serde_json::to_vec(value)
            .map_err(|_| Error::Protocol)?
            .len()
            > 61_440
    {
        return Err(Error::Protocol);
    }
    let mut accounts: Vec<CodexAccount> = Vec::with_capacity(raw.len());
    for row in raw {
        exact(row, &["opaqueId", "label", "email", "plan"])?;
        let id = account_id(&row["opaqueId"])?;
        if accounts.iter().any(|a| a.opaque_id == id)
            || !row["email"].is_null()
            || !row["plan"].is_null()
        {
            return Err(Error::Protocol);
        }
        accounts.push(CodexAccount {
            opaque_id: id,
            label: CodexAccountLabel::new(row["label"].as_str().ok_or(Error::Protocol)?.to_owned())
                .map_err(|_| Error::Protocol)?,
            email: None,
            plan: None,
        });
    }
    let selected = if value["selected"].is_null() {
        None
    } else {
        Some(account_id(&value["selected"])?)
    };
    if selected
        .as_ref()
        .is_some_and(|id| !accounts.iter().any(|a| &a.opaque_id == id))
    {
        return Err(Error::Protocol);
    }
    let epoch = optional_number(&value["epoch"], false)?;
    let added = if value["added"].is_null() {
        None
    } else {
        let row = &value["added"];
        exact(row, &["accountId", "epoch", "attempt"])?;
        let id = account_id(&row["accountId"])?;
        let commit = optional_number(&row["epoch"], false)?.ok_or(Error::Protocol)?;
        if !accounts.iter().any(|a| a.opaque_id == id) || epoch.is_none_or(|n| commit > n) {
            return Err(Error::Protocol);
        }
        Some(CodexAddedAccount {
            account_id: id,
            epoch: commit,
            attempt: optional_number(&row["attempt"], true)?.ok_or(Error::Protocol)?,
        })
    };
    if mode == CodexAccountMode::Managed && epoch.is_none()
        || mode == CodexAccountMode::Legacy
            && (epoch.is_some()
                || !accounts.is_empty()
                || selected.is_some()
                || added.is_some()
                || legacy != CodexLegacyGrant::Absent)
    {
        return Err(Error::Protocol);
    }
    Ok(CodexAccountsMetadata {
        mode,
        accounts,
        selected,
        epoch,
        legacy,
        added,
    })
}

#[cfg(test)]
#[path = "codex_accounts_tests.rs"]
mod accounts_tests;
#[cfg(test)]
#[path = "codex_tests.rs"]
mod tests;
