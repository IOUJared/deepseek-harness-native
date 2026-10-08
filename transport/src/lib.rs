//! Native-only, generation-owned transport for the DSH 0.2.1-alpha.1 public BFF.
//! No generic endpoint dispatch, JS evaluation, credential accessors, or GUI engine.
#![forbid(unsafe_code)]
pub mod dto;
mod event;
pub mod export;
pub use event::{EventDelivery, OwnedRemoteEvent};
mod http;
mod mux;
pub mod plugin;
mod question;
pub mod upload;
pub use http::{Limits, NativeClient, SecretLaunchUrl};
pub use mux::{Mux, NativeStream};
pub use question::QuestionClaim;

/// Redacted transport diagnosis. Never retains remote messages, URLs, JSON, or secrets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidLaunch,
    InvalidLimits,
    Authentication,
    CookieScope,
    HttpStatus(u16),
    Network,
    Timeout,
    Closed,
    Oversize,
    InvalidJson,
    InvalidDto,
    Correlation,
    RemoteFailure,
    SettingsConflict,
    SettingsRejected,
    InvalidFrame,
    Sequence,
    QueueFull,
    StreamLimit,
    QuestionUnavailable,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "native transport: {}", self.code())
    }
}
impl std::error::Error for Error {}
impl Error {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLaunch => "invalid-launch",
            Self::InvalidLimits => "invalid-limits",
            Self::Authentication => "authentication",
            Self::CookieScope => "cookie-scope",
            Self::HttpStatus(_) => "http-status",
            Self::Network => "network",
            Self::Timeout => "timeout",
            Self::Closed => "closed",
            Self::Oversize => "oversize",
            Self::InvalidJson => "invalid-json",
            Self::InvalidDto => "invalid-dto",
            Self::Correlation => "correlation",
            Self::RemoteFailure => "remote-failure",
            Self::SettingsConflict => "settings-conflict",
            Self::SettingsRejected => "settings-rejected",
            Self::InvalidFrame => "invalid-frame",
            Self::Sequence => "sequence",
            Self::QueueFull => "queue-full",
            Self::StreamLimit => "stream-limit",
            Self::QuestionUnavailable => "question-unavailable",
        }
    }
}
pub type Result<T> = std::result::Result<T, Error>;

/// Bound plugin-owned JSON by recursion and total container/value count.
pub(crate) fn validate_json(value: &serde_json::Value, max_items: usize) -> Result<()> {
    fn walk(v: &serde_json::Value, depth: usize, left: &mut usize) -> Result<()> {
        if depth > 64 || *left == 0 {
            return Err(Error::Oversize);
        }
        *left -= 1;
        match v {
            serde_json::Value::Array(items) => {
                for item in items {
                    walk(item, depth + 1, left)?;
                }
            }
            serde_json::Value::Object(items) => {
                for item in items.values() {
                    walk(item, depth + 1, left)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    let mut remaining = max_items;
    walk(value, 0, &mut remaining)
}
