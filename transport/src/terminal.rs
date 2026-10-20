//! Exact `terminal/*` BFF request/value DTOs and fail-closed projections.
//!
//! The Host owns every shell process; this transport is a client only and never
//! spawns a child. Requests are validated before they are sent, and every ACK or
//! stream frame is projected here before the app may publish it as terminal state.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

/// Maximum UTF-8 bytes in a terminal or attachment identity (`[\w-]{1,128}`).
pub const MAX_ID_BYTES: usize = 128;
/// Native outbound cap for one raw input write; the deployment limit is smaller.
pub const MAX_INPUT_BYTES: usize = 256 * 1024;
/// Absolute bounds on one requested PTY dimension (the deployment limit is smaller).
pub const MIN_COLS: u64 = 2;
pub const MIN_ROWS: u64 = 1;
pub const MAX_DIMENSION: u64 = 65_535;
/// Bound on one listed-terminal collection.
pub const MAX_TERMINALS: usize = 4096;
/// Bound on one discovered-shell collection and its argument vectors.
pub const MAX_SHELLS: usize = 256;
pub const MAX_SHELL_ARGS: usize = 64;
/// Bound on one title, working directory, shell path or shell name.
pub const MAX_TEXT_BYTES: usize = 4096;

macro_rules! terminal_id {
    ($name:ident, $expected:literal) => {
        #[doc = $expected]
        #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String")]
        pub struct $name(String);
        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self> {
                let value = value.into();
                if value.is_empty()
                    || value.len() > MAX_ID_BYTES
                    || !value
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                {
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
terminal_id!(
    WebTerminalId,
    "A terminal identity scoped to one Session and Host lifetime."
);
terminal_id!(
    TerminalAttachmentId,
    "One attachment identity allowed to write and resize a terminal."
);

/// An executable shell verified in the subprocess provider's execution environment.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalShell {
    pub path: String,
    pub args: Vec<String>,
    pub name: String,
}

/// Working directory and limits shared by new and restored terminals.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalEnvironment {
    pub cwd: String,
    pub max_input_bytes: u64,
    pub max_cols: u64,
    pub max_rows: u64,
    pub scrollback: u64,
}

/// Host-owned terminal lifecycle; process exit never creates a replacement shell.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TerminalState {
    Running,
    Exited,
    Failed,
}

/// Host-owned terminal state. A public DTO never carries a locally invented terminal.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebTerminalInfo {
    pub id: WebTerminalId,
    pub title: String,
    pub shell: TerminalShell,
    pub cwd: String,
    pub cols: u64,
    pub rows: u64,
    pub state: TerminalState,
    pub exit_code: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controller_id: Option<TerminalAttachmentId>,
}

/// Create is idempotent for an open identity; closed identities cannot be recreated.
/// Serialized as exactly `{shellPath?, id, cols, rows}`; extra authority is rejected.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalCreateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_path: Option<String>,
    pub id: WebTerminalId,
    pub cols: u64,
    pub rows: u64,
}

/// Every attachment begins with a complete bounded screen, then ordered output.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TerminalFrame {
    Snapshot {
        sequence: i64,
        screen: String,
        info: WebTerminalInfo,
    },
    Output {
        sequence: i64,
        data: String,
    },
    State {
        info: WebTerminalInfo,
    },
}

fn bounded(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_TEXT_BYTES && !value.chars().any(char::is_control)
}

/// Refuse a locally impossible PTY dimension before it reaches the Host.
pub fn validate_dimensions(cols: u64, rows: u64) -> Result<()> {
    if !(MIN_COLS..=MAX_DIMENSION).contains(&cols) || !(MIN_ROWS..=MAX_DIMENSION).contains(&rows) {
        return Err(Error::InvalidDto);
    }
    Ok(())
}

/// Refuse a create request whose identity, shell hint or dimensions cannot be valid.
pub fn validate_create(request: &TerminalCreateRequest) -> Result<()> {
    validate_dimensions(request.cols, request.rows)?;
    if request
        .shell_path
        .as_deref()
        .is_some_and(|path| !bounded(path))
    {
        return Err(Error::InvalidDto);
    }
    Ok(())
}

/// The Host's own title rule for a rename: 1–120 characters after trimming, with no control
/// characters (`terminal-controller/src/index.ts` `rename`: `title.trim().length === 0 ||
/// title.length > 120`). The request is refused here so the panel never sends what the Host would
/// reject.
pub fn validate_title(title: &str) -> Result<()> {
    let trimmed = title.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 120 || trimmed.chars().any(char::is_control)
    {
        return Err(Error::InvalidDto);
    }
    Ok(())
}

/// Refuse one raw input write that cannot fit the native outbound envelope.
pub fn validate_input(data: &str) -> Result<()> {
    if data.len() > MAX_INPUT_BYTES {
        return Err(Error::InvalidDto);
    }
    Ok(())
}

fn valid_shell(shell: &TerminalShell) -> bool {
    bounded(&shell.path)
        && bounded(&shell.name)
        && shell.args.len() <= MAX_SHELL_ARGS
        && shell.args.iter().all(|arg| arg.len() <= MAX_TEXT_BYTES)
}

/// Refuse terminal metadata that could not describe a real Host terminal, and
/// keep the state/exit-code/error fields mutually consistent.
pub fn validate_info(info: &WebTerminalInfo) -> Result<()> {
    validate_dimensions(info.cols, info.rows)?;
    if !bounded(&info.title) || !bounded(&info.cwd) || !valid_shell(&info.shell) {
        return Err(Error::InvalidDto);
    }
    if info
        .error
        .as_deref()
        .is_some_and(|error| error.len() > MAX_TEXT_BYTES)
    {
        return Err(Error::InvalidDto);
    }
    match info.state {
        TerminalState::Running => {
            if info.exit_code.is_some() || info.error.is_some() {
                return Err(Error::InvalidDto);
            }
        }
        TerminalState::Exited => {
            if info.exit_code.is_none() || info.error.is_some() {
                return Err(Error::InvalidDto);
            }
        }
        TerminalState::Failed => {
            if info.exit_code.is_some() {
                return Err(Error::InvalidDto);
            }
        }
    }
    Ok(())
}

/// Validate a complete list and reject duplicate identities.
pub fn project_list(items: Vec<WebTerminalInfo>) -> Result<Vec<WebTerminalInfo>> {
    if items.len() > MAX_TERMINALS {
        return Err(Error::InvalidDto);
    }
    for (index, info) in items.iter().enumerate() {
        validate_info(info)?;
        if items[..index].iter().any(|known| known.id == info.id) {
            return Err(Error::InvalidDto);
        }
    }
    Ok(items)
}

/// Validate a discovered-shell collection; a malformed entry invalidates the whole set.
pub fn project_shells(items: Vec<TerminalShell>) -> Result<Vec<TerminalShell>> {
    if items.len() > MAX_SHELLS || !items.iter().all(valid_shell) {
        return Err(Error::InvalidDto);
    }
    Ok(items)
}

/// Validate the deployment limits the app uses to bound input and dimensions.
pub fn validate_environment(environment: &TerminalEnvironment) -> Result<()> {
    if !bounded(&environment.cwd)
        || environment.max_input_bytes == 0
        || !(MIN_COLS..=MAX_DIMENSION).contains(&environment.max_cols)
        || !(MIN_ROWS..=MAX_DIMENSION).contains(&environment.max_rows)
    {
        return Err(Error::InvalidDto);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_title_is_trimmed_to_the_hosts_one_to_one_hundred_and_twenty_rule() {
        assert!(validate_title("PUBLIC terminal").is_ok());
        assert!(validate_title("  PUBLIC padded  ").is_ok());
        assert_eq!(validate_title(""), Err(Error::InvalidDto));
        assert_eq!(validate_title("   "), Err(Error::InvalidDto));
        assert_eq!(validate_title(&"x".repeat(121)), Err(Error::InvalidDto));
        assert!(validate_title(&"x".repeat(120)).is_ok());
        // A control character is refused rather than sent: the Host would reject it, and a
        // newline in a title would split the panel's own row.
        assert_eq!(validate_title("PUBLIC\nterminal"), Err(Error::InvalidDto));
    }

    fn id(value: &str) -> WebTerminalId {
        WebTerminalId::new(value).unwrap()
    }
    fn shell() -> TerminalShell {
        TerminalShell {
            path: "/bin/PUBLIC-shell".into(),
            args: vec!["-l".into()],
            name: "PUBLIC shell".into(),
        }
    }
    fn info(state: TerminalState, exit_code: Option<i64>) -> WebTerminalInfo {
        WebTerminalInfo {
            id: id("PUBLIC-terminal"),
            title: "PUBLIC title".into(),
            shell: shell(),
            cwd: "/PUBLIC/cwd".into(),
            cols: 80,
            rows: 24,
            state,
            exit_code,
            error: None,
            controller_id: None,
        }
    }

    #[test]
    fn identities_use_exactly_the_host_character_class_and_length() {
        for value in ["a", "A0_-", &"x".repeat(MAX_ID_BYTES)] {
            assert!(WebTerminalId::new(value).is_ok());
        }
        for value in ["", " ", "a/b", "a.b", "é", &"x".repeat(MAX_ID_BYTES + 1)] {
            assert!(WebTerminalId::new(value).is_err(), "{value:?}");
        }
        assert!(TerminalAttachmentId::new("attachment-1").is_ok());
    }

    #[test]
    fn create_request_is_exactly_the_four_named_fields() {
        let request = TerminalCreateRequest {
            shell_path: None,
            id: id("PUBLIC-terminal"),
            cols: 80,
            rows: 24,
        };
        assert_eq!(
            serde_json::to_value(&request).unwrap(),
            json!({"id":"PUBLIC-terminal","cols":80,"rows":24})
        );
        assert_eq!(
            serde_json::to_value(TerminalCreateRequest {
                shell_path: Some("/bin/PUBLIC-shell".into()),
                ..request.clone()
            })
            .unwrap(),
            json!({"shellPath":"/bin/PUBLIC-shell","id":"PUBLIC-terminal","cols":80,"rows":24})
        );
        for extra in ["agent", "agentId", "title", "enabled"] {
            let mut value = serde_json::to_value(&request).unwrap();
            value[extra] = json!(false);
            assert!(serde_json::from_value::<TerminalCreateRequest>(value).is_err());
        }
    }

    #[test]
    fn dimensions_refuse_host_impossible_and_non_integer_values() {
        for (cols, rows) in [(0, 24), (1, 24), (80, 0), (MAX_DIMENSION + 1, 24)] {
            assert!(validate_dimensions(cols, rows).is_err(), "{cols}x{rows}");
        }
        assert!(validate_dimensions(MIN_COLS, MIN_ROWS).is_ok());
        assert!(validate_dimensions(MAX_DIMENSION, MAX_DIMENSION).is_ok());
        assert!(
            serde_json::from_str::<TerminalCreateRequest>(r#"{"id":"a","cols":1.5,"rows":24}"#)
                .is_err()
        );
    }

    #[test]
    fn info_requires_state_consistent_exit_and_error_fields() {
        assert!(validate_info(&info(TerminalState::Running, None)).is_ok());
        assert!(validate_info(&info(TerminalState::Exited, Some(0))).is_ok());
        assert!(validate_info(&info(TerminalState::Running, Some(0))).is_err());
        assert!(validate_info(&info(TerminalState::Exited, None)).is_err());
        assert!(validate_info(&info(TerminalState::Failed, Some(1))).is_err());
        let mut error = info(TerminalState::Running, None);
        error.error = Some("PUBLIC".into());
        assert!(validate_info(&error).is_err());
        let mut failed = info(TerminalState::Failed, None);
        failed.error = Some("PUBLIC failure".into());
        assert!(validate_info(&failed).is_ok());
    }

    #[test]
    fn malformed_text_dimensions_and_shells_fail_closed() {
        let mut blank_title = info(TerminalState::Running, None);
        blank_title.title = String::new();
        assert!(validate_info(&blank_title).is_err());
        let mut control = info(TerminalState::Running, None);
        control.cwd = "PUBLIC\u{0}".into();
        assert!(validate_info(&control).is_err());
        let mut empty_shell = info(TerminalState::Running, None);
        empty_shell.shell.path = String::new();
        assert!(validate_info(&empty_shell).is_err());
        let mut many_args = shell();
        many_args.args = vec!["x".into(); MAX_SHELL_ARGS + 1];
        assert!(!valid_shell(&many_args));
    }

    #[test]
    fn list_rejects_oversize_and_duplicate_identities() {
        assert!(project_list(vec![]).is_ok());
        assert!(project_list(vec![info(TerminalState::Running, None)]).is_ok());
        assert!(
            project_list(vec![
                info(TerminalState::Running, None),
                info(TerminalState::Running, None)
            ])
            .is_err()
        );
        assert!(project_list(vec![info(TerminalState::Running, None); MAX_TERMINALS + 1]).is_err());
    }

    #[test]
    fn environment_limits_must_be_usable() {
        let environment = TerminalEnvironment {
            cwd: "/PUBLIC".into(),
            max_input_bytes: 65536,
            max_cols: 500,
            max_rows: 200,
            scrollback: 1000,
        };
        assert!(validate_environment(&environment).is_ok());
        assert!(
            validate_environment(&TerminalEnvironment {
                max_input_bytes: 0,
                ..environment.clone()
            })
            .is_err()
        );
        assert!(
            validate_environment(&TerminalEnvironment {
                max_cols: 1,
                ..environment.clone()
            })
            .is_err()
        );
        assert!(
            validate_environment(&TerminalEnvironment {
                cwd: String::new(),
                ..environment
            })
            .is_err()
        );
    }

    #[test]
    fn frames_decode_only_the_three_documented_tags() {
        let value = json!({"type":"snapshot","sequence":0,"screen":"PUBLIC","info":serde_json::to_value(info(TerminalState::Running, None)).unwrap()});
        assert!(matches!(
            serde_json::from_value::<TerminalFrame>(value).unwrap(),
            TerminalFrame::Snapshot { sequence: 0, .. }
        ));
        assert!(matches!(
            serde_json::from_value::<TerminalFrame>(
                json!({"type":"output","sequence":1,"data":"PUBLIC"})
            )
            .unwrap(),
            TerminalFrame::Output { sequence: 1, .. }
        ));
        assert!(matches!(
            serde_json::from_value::<TerminalFrame>(
                json!({"type":"state","info":serde_json::to_value(info(TerminalState::Running, None)).unwrap()})
            )
            .unwrap(),
            TerminalFrame::State { .. }
        ));
        assert!(
            serde_json::from_value::<TerminalFrame>(json!({"type":"retained"})).is_err(),
            "a retention frame is not terminal output"
        );
    }

    #[test]
    fn input_is_bounded_before_it_can_leave_the_native_process() {
        assert!(validate_input("").is_ok());
        assert!(validate_input(&"x".repeat(MAX_INPUT_BYTES)).is_ok());
        assert!(validate_input(&"x".repeat(MAX_INPUT_BYTES + 1)).is_err());
    }
}
