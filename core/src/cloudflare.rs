//! Closed Cloudflare configuration and OAuth metadata; credentials remain backend-owned.
use crate::Error;
use serde_json::{Value, json};
use url::Url;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CloudflareAuthMode {
    #[default]
    ApiToken,
    Oauth,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CloudflareAuthPhase {
    #[default]
    Idle,
    Starting,
    WaitingBrowser,
    Exchanging,
    Committing,
    Connected,
    Cancelled,
    Failed,
    Indeterminate,
}
impl CloudflareAuthPhase {
    pub fn active(self) -> bool {
        matches!(
            self,
            Self::Starting | Self::WaitingBrowser | Self::Exchanging | Self::Committing
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloudflareAuthError {
    SetupRequired,
    CallbackUnavailable,
    OauthDenied,
    OauthFailed,
    ReconnectRequired,
    Conflict,
    Indeterminate,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CloudflareAuthStatus {
    pub auth_mode: CloudflareAuthMode,
    pub setup_configured: bool,
    pub credential_stored: bool,
    pub credential_writable: bool,
    pub phase: CloudflareAuthPhase,
    pub attempt_id: Option<String>,
    pub retry_blocked: bool,
    pub error_code: Option<CloudflareAuthError>,
    pub auth_epoch: u64,
    pub sequence: u64,
    /// Derived from private capability admission, never an extra wire authStatus field.
    pub browser_available: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloudflareOauthSetup {
    pub client_id: String,
    pub scopes: Vec<String>,
    pub callback_port: u16,
}
impl Default for CloudflareOauthSetup {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            scopes: Vec::new(),
            callback_port: 8977,
        }
    }
}
impl CloudflareOauthSetup {
    pub fn valid(&self, configured: bool) -> bool {
        self.callback_port >= 1024
            && self.client_id.len() <= 256
            && self.client_id.bytes().all(|b| (33..=126).contains(&b))
            && self.scopes.len() <= 32
            && self.scopes.iter().all(|s| {
                !s.is_empty()
                    && s.len() <= 256
                    && s.bytes()
                        .all(|b| b == 33 || (35..=91).contains(&b) || (93..=126).contains(&b))
            })
            && self.scopes.join(" ").len() <= 2048
            && !self
                .scopes
                .iter()
                .enumerate()
                .any(|(i, s)| self.scopes[..i].contains(s))
            && (!configured || (!self.client_id.is_empty() && !self.scopes.is_empty()))
    }
    pub(crate) fn encode(&self) -> Value {
        json!({"clientId":self.client_id,"scopes":self.scopes,"callbackPort":self.callback_port})
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloudflareSnapshot {
    pub account_id: String,
    pub models_enabled: bool,
    pub browser_enabled: bool,
    pub free_plan_confirmed: bool,
    pub token_configured: bool,
    pub writable: bool,
    pub revision: u64,
    pub auth_status: CloudflareAuthStatus,
    pub oauth_setup: CloudflareOauthSetup,
}
fn exact(value: &Value, fields: &[&str]) -> Result<(), Error> {
    let obj = value.as_object().ok_or(Error::Protocol)?;
    if obj.len() != fields.len() || !fields.iter().all(|k| obj.contains_key(*k)) {
        return Err(Error::Protocol);
    }
    Ok(())
}
fn natural(value: &Value) -> Result<u64, Error> {
    value
        .as_u64()
        .filter(|v| *v <= crate::codex::MAX_SAFE_INTEGER)
        .ok_or(Error::Protocol)
}
pub(crate) fn valid_attempt(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && value.bytes().all(|b| (33..=126).contains(&b))
}
pub(crate) fn setup(value: &Value) -> Result<CloudflareOauthSetup, Error> {
    exact(value, &["clientId", "scopes", "callbackPort"])?;
    let setup = CloudflareOauthSetup {
        client_id: value["clientId"]
            .as_str()
            .ok_or(Error::Protocol)?
            .to_owned(),
        scopes: value["scopes"]
            .as_array()
            .filter(|v| v.len() <= 32)
            .ok_or(Error::Protocol)?
            .iter()
            .map(|v| v.as_str().map(str::to_owned).ok_or(Error::Protocol))
            .collect::<Result<_, _>>()?,
        callback_port: value["callbackPort"]
            .as_u64()
            .filter(|v| *v >= 1024 && *v <= 65535)
            .ok_or(Error::Protocol)? as u16,
    };
    if !setup.valid(false) {
        return Err(Error::Protocol);
    }
    Ok(setup)
}
pub(crate) fn auth(value: &Value) -> Result<CloudflareAuthStatus, Error> {
    exact(
        value,
        &[
            "authMode",
            "setupConfigured",
            "credentialStored",
            "credentialWritable",
            "phase",
            "attemptId",
            "retryBlocked",
            "errorCode",
            "authEpoch",
            "sequence",
        ],
    )?;
    let auth_mode = match value["authMode"].as_str() {
        Some("api-token") => CloudflareAuthMode::ApiToken,
        Some("oauth") => CloudflareAuthMode::Oauth,
        _ => return Err(Error::Protocol),
    };
    let phase = match value["phase"].as_str() {
        Some("idle") => CloudflareAuthPhase::Idle,
        Some("starting") => CloudflareAuthPhase::Starting,
        Some("waiting-browser") => CloudflareAuthPhase::WaitingBrowser,
        Some("exchanging") => CloudflareAuthPhase::Exchanging,
        Some("committing") => CloudflareAuthPhase::Committing,
        Some("connected") => CloudflareAuthPhase::Connected,
        Some("cancelled") => CloudflareAuthPhase::Cancelled,
        Some("failed") => CloudflareAuthPhase::Failed,
        Some("indeterminate") => CloudflareAuthPhase::Indeterminate,
        _ => return Err(Error::Protocol),
    };
    let attempt_id = if value["attemptId"].is_null() {
        None
    } else {
        Some(
            value["attemptId"]
                .as_str()
                .filter(|v| valid_attempt(v))
                .ok_or(Error::Protocol)?
                .to_owned(),
        )
    };
    let error_code = match value["errorCode"].as_str() {
        None if value["errorCode"].is_null() => None,
        Some("SETUP_REQUIRED") => Some(CloudflareAuthError::SetupRequired),
        Some("CALLBACK_UNAVAILABLE") => Some(CloudflareAuthError::CallbackUnavailable),
        Some("OAUTH_DENIED") => Some(CloudflareAuthError::OauthDenied),
        Some("OAUTH_FAILED") => Some(CloudflareAuthError::OauthFailed),
        Some("RECONNECT_REQUIRED") => Some(CloudflareAuthError::ReconnectRequired),
        Some("CONFLICT") => Some(CloudflareAuthError::Conflict),
        Some("INDETERMINATE") => Some(CloudflareAuthError::Indeterminate),
        _ => return Err(Error::Protocol),
    };
    let status = CloudflareAuthStatus {
        auth_mode,
        phase,
        attempt_id,
        error_code,
        setup_configured: value["setupConfigured"].as_bool().ok_or(Error::Protocol)?,
        credential_stored: value["credentialStored"].as_bool().ok_or(Error::Protocol)?,
        credential_writable: value["credentialWritable"]
            .as_bool()
            .ok_or(Error::Protocol)?,
        retry_blocked: value["retryBlocked"].as_bool().ok_or(Error::Protocol)?,
        auth_epoch: natural(&value["authEpoch"])?,
        sequence: natural(&value["sequence"])?,
        browser_available: false,
    };
    if status.phase.active() && status.attempt_id.is_none() {
        return Err(Error::Protocol);
    }
    Ok(status)
}
pub(crate) fn account_valid(account: &str) -> bool {
    account.is_empty()
        || (account.len() == 32
            && account
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
}
pub(crate) fn validate_save(
    revision: u64,
    account: &str,
    models: bool,
    browser: bool,
    free: bool,
) -> Result<(), Error> {
    if revision >= crate::codex::MAX_SAFE_INTEGER
        || !account_valid(account)
        || ((models || browser) && (account.is_empty() || !free))
    {
        return Err(Error::InvalidCommand);
    }
    Ok(())
}
pub(crate) fn snapshot(value: &Value) -> Result<CloudflareSnapshot, Error> {
    exact(
        value,
        &[
            "accountId",
            "modelsEnabled",
            "browserEnabled",
            "freePlanConfirmed",
            "tokenConfigured",
            "writable",
            "revision",
            "authStatus",
            "oauthSetup",
        ],
    )?;
    let result = CloudflareSnapshot {
        account_id: value["accountId"]
            .as_str()
            .filter(|v| account_valid(v))
            .ok_or(Error::Protocol)?
            .to_owned(),
        models_enabled: value["modelsEnabled"].as_bool().ok_or(Error::Protocol)?,
        browser_enabled: value["browserEnabled"].as_bool().ok_or(Error::Protocol)?,
        free_plan_confirmed: value["freePlanConfirmed"]
            .as_bool()
            .ok_or(Error::Protocol)?,
        token_configured: value["tokenConfigured"].as_bool().ok_or(Error::Protocol)?,
        writable: value["writable"].as_bool().ok_or(Error::Protocol)?,
        revision: natural(&value["revision"])?,
        auth_status: auth(&value["authStatus"])?,
        oauth_setup: setup(&value["oauthSetup"])?,
    };
    if (result.models_enabled || result.browser_enabled)
        && (result.account_id.is_empty() || !result.free_plan_confirmed)
        || !result
            .oauth_setup
            .valid(result.auth_status.setup_configured)
    {
        return Err(Error::Protocol);
    }
    Ok(result)
}
/// The trusted setup projection, not supplied URL parameters, defines browser authority.
pub(crate) fn validate_url(raw: &str, setup: &CloudflareOauthSetup) -> Result<(), Error> {
    if !setup.valid(true) || raw.len() > 16384 || raw.bytes().any(|b| b <= 32 || b == 127) {
        return Err(Error::Protocol);
    }
    let url = Url::parse(raw).map_err(|_| Error::Protocol)?;
    if url.scheme() != "https"
        || url.host_str() != Some("dash.cloudflare.com")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/oauth2/auth"
        || url.fragment().is_some()
    {
        return Err(Error::Protocol);
    }
    let pairs: Vec<_> = url.query_pairs().collect();
    let redirect = format!(
        "http://127.0.0.1:{}/cloudflare/callback",
        setup.callback_port
    );
    let scopes = setup.scopes.join(" ");
    let expected = [
        ("response_type", "code"),
        ("client_id", setup.client_id.as_str()),
        ("redirect_uri", redirect.as_str()),
        ("scope", scopes.as_str()),
        ("code_challenge_method", "S256"),
    ];
    if pairs.len() != 7 {
        return Err(Error::Protocol);
    }
    for (k, v) in expected {
        if pairs
            .iter()
            .filter(|(key, val)| key == k && val == v)
            .count()
            != 1
        {
            return Err(Error::Protocol);
        }
    }
    for k in ["state", "code_challenge"] {
        let vals: Vec<_> = pairs.iter().filter(|(key, _)| key == k).collect();
        if vals.len() != 1
            || (k == "code_challenge" && vals[0].1.len() != 43)
            || !(32..=128).contains(&vals[0].1.len())
            || !vals[0]
                .1
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(Error::Protocol);
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fixture() -> Value {
        json!({"accountId":"0123456789abcdef0123456789abcdef","modelsEnabled":true,"browserEnabled":false,"freePlanConfirmed":true,"tokenConfigured":true,"writable":true,"revision":2,"authStatus":{"authMode":"api-token","setupConfigured":false,"credentialStored":true,"credentialWritable":true,"phase":"idle","attemptId":null,"retryBlocked":false,"errorCode":null,"authEpoch":0,"sequence":0},"oauthSetup":{"clientId":"","scopes":[],"callbackPort":8977}})
    }
    #[test]
    fn parser_requires_exact_nonsecret_fields_and_safe_types() {
        let valid = fixture();
        assert_eq!(snapshot(&valid).unwrap().revision, 2);
        let mut deleted_token = valid.clone();
        deleted_token["tokenConfigured"] = json!(false);
        assert!(!snapshot(&deleted_token).unwrap().token_configured);
        for key in valid.as_object().unwrap().keys() {
            let mut missing = valid.clone();
            missing.as_object_mut().unwrap().remove(key);
            assert_eq!(snapshot(&missing), Err(Error::Protocol));
            let mut null = valid.clone();
            null[key] = Value::Null;
            assert_eq!(snapshot(&null), Err(Error::Protocol));
        }
        for (key, bad) in [
            ("apiToken", json!("PUBLIC_FAKE_TOKEN")),
            ("accountId", json!("ABCDEF0123456789abcdef0123456789ab")),
            ("revision", json!(9007199254740992u64)),
            ("revision", json!(1.5)),
            ("freePlanConfirmed", json!(false)),
        ] {
            let mut value = valid.clone();
            value[key] = bad;
            assert_eq!(snapshot(&value), Err(Error::Protocol));
        }
    }
    #[test]
    fn protocol_projects_each_command_and_nonsecret_encoding_is_closed() {
        use crate::{Command, PublicReply, protocol};
        assert_eq!(
            Command::CloudflareRead.encode("r").unwrap(),
            json!({"type":"cloudflare-read","requestId":"r"})
        );
        assert_eq!(
            Command::CloudflareDisable {
                expected_revision: 4
            }
            .encode("r")
            .unwrap(),
            json!({"type":"cloudflare-disable","requestId":"r","expectedRevision":4})
        );
        assert!(
            Command::CloudflareDisable {
                expected_revision: crate::codex::MAX_SAFE_INTEGER
            }
            .encode("r")
            .is_err()
        );
        for command in ["cloudflare-read", "cloudflare-save", "cloudflare-disable"] {
            let raw = format!(
                "DSH_TAURI:{}",
                json!({"type":"native-result","command":command,"requestId":"r","value":fixture()})
            );
            let protocol::Event::Reply {
                reply: Ok(PublicReply::Cloudflare(snapshot)),
                ..
            } = protocol::parse(raw.as_bytes(), "0.2.1-alpha.1").unwrap()
            else {
                panic!("closed metadata reply")
            };
            assert!(protocol::matches_reply(
                command,
                &PublicReply::Cloudflare(snapshot)
            ));
            assert!(!protocol::matches_reply(
                "save-api-key",
                &PublicReply::Cloudflare(super::snapshot(&fixture()).unwrap())
            ));
        }
    }
    #[test]
    fn save_requires_valid_account_consent_and_successor_revision() {
        assert!(validate_save(0, "", false, false, false).is_ok());
        assert!(validate_save(0, "", true, false, true).is_err());
        assert!(validate_save(0, "0123456789abcdef0123456789abcdef", false, true, false).is_err());
        assert!(validate_save(crate::codex::MAX_SAFE_INTEGER, "", false, false, false).is_err());
    }
    #[test]
    fn oauth_setup_and_status_are_closed_bounded_and_nonsecret() {
        let mut value = fixture();
        let setup_value =
            json!({"clientId":"PUBLIC-client","scopes":["PUBLIC:scope"],"callbackPort":8977});
        assert!(setup(&setup_value).unwrap().valid(true));
        for bad in [
            json!({"clientId":"PUBLIC-client","scopes":["a","a"],"callbackPort":8977}),
            json!({"clientId":"PUBLIC-client","scopes":["has space"],"callbackPort":8977}),
            json!({"clientId":"PUBLIC-client","scopes":["quote\""],"callbackPort":8977}),
            json!({"clientId":"PUBLIC-client","scopes":[],"callbackPort":1023}),
        ] {
            assert!(setup(&bad).is_err());
        }
        for field in ["tokenValue", "refreshToken", "clientSecret"] {
            value["authStatus"][field] = json!("PUBLIC-secret");
            assert!(snapshot(&value).is_err());
            value["authStatus"].as_object_mut().unwrap().remove(field);
        }
        value["authStatus"]["phase"] = json!("new-phase");
        assert!(snapshot(&value).is_err());
        value["authStatus"]["phase"] = json!("waiting-browser");
        assert!(snapshot(&value).is_err());
    }
    fn oauth_url(setup: &CloudflareOauthSetup) -> String {
        let mut u = Url::parse("https://dash.cloudflare.com/oauth2/auth").unwrap();
        u.query_pairs_mut().extend_pairs([
            ("response_type", "code"),
            ("client_id", setup.client_id.as_str()),
            (
                "redirect_uri",
                &format!(
                    "http://127.0.0.1:{}/cloudflare/callback",
                    setup.callback_port
                ),
            ),
            ("scope", &setup.scopes.join(" ")),
            ("state", &"A".repeat(43)),
            ("code_challenge", &"B".repeat(43)),
            ("code_challenge_method", "S256"),
        ]);
        u.to_string()
    }
    #[test]
    fn oauth_browser_authority_uses_registered_setup_not_url_claims() {
        let setup = CloudflareOauthSetup {
            client_id: "PUBLIC-client".into(),
            scopes: vec!["PUBLIC:scope".into()],
            callback_port: 8977,
        };
        let valid = oauth_url(&setup);
        assert!(validate_url(&valid, &setup).is_ok());
        for bad in [
            valid.replace("dash.cloudflare.com", "dash.cloudflare.com.evil.invalid"),
            valid.replace("PUBLIC-client", "other-client"),
            format!("{valid}&scope=PUBLIC%3Ascope"),
            format!("{valid}&extra=1"),
            format!("{valid}#fragment"),
            valid.replace("S256", "plain"),
            valid.replace("8977", "8978"),
            valid.replace("PUBLIC%3Ascope", "other"),
        ] {
            assert!(validate_url(&bad, &setup).is_err(), "{bad}");
        }
        let command = crate::Command::CloudflareSetupSave {
            expected_revision: 1,
            setup: setup.clone(),
        };
        assert_eq!(
            command.encode("r").unwrap(),
            json!({"type":"cloudflare-setup-save","requestId":"r","expectedRevision":1,"clientId":"PUBLIC-client","scopes":["PUBLIC:scope"],"callbackPort":8977})
        );
        assert!(
            crate::Command::CloudflareOauthCancel {
                attempt_id: "has space".into()
            }
            .encode("r")
            .is_err()
        );
    }
}
