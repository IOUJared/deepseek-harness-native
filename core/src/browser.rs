use crate::{Error, secret::SecretBytes};
use std::{
    ffi::OsString,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use url::Url;

/// Attempt-scoped browser capability; no public access, Debug, Clone or serde.
pub(crate) struct BrowserCapability(SecretBytes);
impl BrowserCapability {
    pub(crate) fn new(raw: String) -> Result<Self, Error> {
        let bytes = SecretBytes::new(raw.into_bytes());
        let text = std::str::from_utf8(bytes.as_slice()).map_err(|_| Error::Protocol)?;
        validate(text)?;
        Ok(Self(bytes))
    }
    pub(crate) fn spawn(self) -> Result<Child, Error> {
        let text = std::str::from_utf8(self.0.as_slice()).map_err(|_| Error::Protocol)?;
        validate(text)?;
        // Preserve only native desktop identity, not credentials or opener shell overrides.
        Command::new("/usr/bin/xdg-open")
            .env_clear()
            .envs(environment(std::env::vars_os()))
            .arg(text)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| Error::Spawn)
    }
}
pub(crate) fn environment(
    input: impl IntoIterator<Item = (OsString, OsString)>,
) -> Vec<(OsString, OsString)> {
    const ALLOWED: &[&str] = &[
        "HOME",
        "LANG",
        "LC_ALL",
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XDG_RUNTIME_DIR",
        "DBUS_SESSION_BUS_ADDRESS",
        "XDG_CURRENT_DESKTOP",
        "DESKTOP_SESSION",
        "XDG_SESSION_TYPE",
        "HYPRLAND_INSTANCE_SIGNATURE",
        "XDG_CONFIG_HOME",
        "XDG_CACHE_HOME",
        "XDG_DATA_HOME",
        "XDG_STATE_HOME",
    ];
    let mut selected: Vec<_> = input
        .into_iter()
        .filter(|(key, _)| key.to_str().is_some_and(|key| ALLOWED.contains(&key)))
        .collect();
    selected.push(("PATH".into(), "/usr/bin:/bin".into()));
    selected
}

pub(crate) fn validate(raw: &str) -> Result<(), Error> {
    if raw.len() > 16_384 || raw.bytes().any(|byte| byte <= 0x20 || byte == 0x7f) {
        return Err(Error::Protocol);
    }
    let url = Url::parse(raw).map_err(|_| Error::Protocol)?;
    if url.scheme() != "https"
        || url.host_str() != Some("auth.openai.com")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/oauth/authorize"
        || url.fragment().is_some()
    {
        return Err(Error::Protocol);
    }
    let expected = [
        ("response_type", "code"),
        ("client_id", "app_EMoamEEZ73f0CkXaXp7hrann"),
        ("redirect_uri", "http://localhost:1455/auth/callback"),
        ("scope", "openid profile email offline_access"),
        ("code_challenge_method", "S256"),
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
        ("originator", "pi"),
    ];
    let pairs: Vec<_> = url.query_pairs().collect();
    if pairs.len() != expected.len() + 2 {
        return Err(Error::Protocol);
    }
    for (key, value) in expected {
        if pairs.iter().filter(|(k, v)| k == key && v == value).count() != 1 {
            return Err(Error::Protocol);
        }
    }
    for (key, size) in [("state", 32), ("code_challenge", 43)] {
        let values: Vec<_> = pairs.iter().filter(|(k, _)| k == key).collect();
        if values.len() != 1
            || values[0].1.len() != size
            || !values[0].1.bytes().all(|b| {
                if key == "state" {
                    b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
                } else {
                    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
                }
            })
        {
            return Err(Error::Protocol);
        }
    }
    Ok(())
}
pub(crate) fn wait(mut child: Child) -> Result<(), Error> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(())
                } else {
                    Err(Error::RequestFailed)
                };
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                // Retain reaping ownership without an unbounded worker/UI wait.
                thread::spawn(move || {
                    let _ = child.wait();
                });
                return Err(Error::RequestTimeout);
            }
        }
    }
}
