use crate::Error;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodexRuntimeTransport {
    Sse,
    Websocket,
    WebsocketCached,
    Auto,
}
impl CodexRuntimeTransport {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sse => "sse",
            Self::Websocket => "websocket",
            Self::WebsocketCached => "websocket-cached",
            Self::Auto => "auto",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "sse" => Some(Self::Sse),
            "websocket" => Some(Self::Websocket),
            "websocket-cached" => Some(Self::WebsocketCached),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodexCacheRetention {
    None,
    Short,
    Long,
}
impl CodexCacheRetention {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Short => "short",
            Self::Long => "long",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "none" => Some(Self::None),
            "short" => Some(Self::Short),
            "long" => Some(Self::Long),
            _ => None,
        }
    }
}

/// A JavaScript-safe natural number, including zero (an explicit timeout value).
///
/// ```compile_fail
/// let invalid = dsh_native_core::CodexRuntimeNatural(u64::MAX);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodexRuntimeNatural(u64);
impl CodexRuntimeNatural {
    pub fn new(value: u64) -> Result<Self, Error> {
        if value > crate::codex::MAX_SAFE_INTEGER {
            return Err(Error::InvalidCommand);
        }
        Ok(Self(value))
    }
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A strictly positive JavaScript-safe image budget.
///
/// ```compile_fail
/// let invalid = dsh_native_core::CodexImageBudget(0);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodexImageBudget(u64);
impl CodexImageBudget {
    pub fn new(value: u64) -> Result<Self, Error> {
        if value == 0 || value > crate::codex::MAX_SAFE_INTEGER {
            return Err(Error::InvalidCommand);
        }
        Ok(Self(value))
    }
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Finite, strictly positive milliseconds up to 2147483647; fractions are retained.
/// Private bits admit neither NaN nor signed zero, preserving Eq for commands/replies.
///
/// ```compile_fail
/// let invalid = dsh_native_core::CodexIdleMillis(f64::NAN.to_bits());
/// ```
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CodexIdleMillis(u64);
impl CodexIdleMillis {
    pub fn new(value: f64) -> Result<Self, Error> {
        if !value.is_finite() || value <= 0.0 || value > 2_147_483_647.0 {
            return Err(Error::InvalidCommand);
        }
        Ok(Self(value.to_bits()))
    }
    pub const fn as_f64(self) -> f64 {
        f64::from_bits(self.0)
    }
}
impl std::fmt::Debug for CodexIdleMillis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CodexIdleMillis")
            .field(&self.as_f64())
            .finish()
    }
}

/// Eight nonsecret fixed-provider runtime fields. In a save, None unsets an override
/// and inherits its base; snapshots distinguish effective, base and explicit override values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CodexRuntimeValues {
    pub transport: Option<CodexRuntimeTransport>,
    pub cache_retention: Option<CodexCacheRetention>,
    pub timeout_ms: Option<CodexRuntimeNatural>,
    pub websocket_connect_timeout_ms: Option<CodexRuntimeNatural>,
    pub stream_idle_timeout_ms: Option<CodexIdleMillis>,
    pub request_image_pixel_budget: Option<CodexImageBudget>,
    pub request_image_max_bytes: Option<CodexImageBudget>,
    pub max_request_image_bytes: Option<CodexImageBudget>,
}
impl CodexRuntimeValues {
    pub(crate) fn encode(self) -> Value {
        json!({
            "transport": self.transport.map(CodexRuntimeTransport::as_str),
            "cacheRetention": self.cache_retention.map(CodexCacheRetention::as_str),
            "timeoutMs": self.timeout_ms.map(CodexRuntimeNatural::get),
            "websocketConnectTimeoutMs": self.websocket_connect_timeout_ms.map(CodexRuntimeNatural::get),
            "streamIdleTimeoutMs": self.stream_idle_timeout_ms.map(CodexIdleMillis::as_f64),
            "requestImagePixelBudget": self.request_image_pixel_budget.map(CodexImageBudget::get),
            "requestImageMaxBytes": self.request_image_max_bytes.map(CodexImageBudget::get),
            "maxRequestImageBytes": self.max_request_image_bytes.map(CodexImageBudget::get),
        })
    }
}

/// A persistence/configuration observation, not evidence of a provider request or policy activation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodexRuntimeSnapshot {
    pub version: u8,
    pub revision: u64,
    pub configured: bool,
    pub writable: bool,
    pub values: CodexRuntimeValues,
    pub base_values: CodexRuntimeValues,
    /// Explicit user pins, independently projected; equality with base does not mean inheritance.
    pub overrides: CodexRuntimeValues,
}

fn exact(value: &Value, keys: &[&str]) -> Result<(), Error> {
    let object = value.as_object().ok_or(Error::Protocol)?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(Error::Protocol);
    }
    Ok(())
}
fn optional<T>(
    value: &Value,
    parse: impl FnOnce(&Value) -> Result<T, Error>,
) -> Result<Option<T>, Error> {
    if value.is_null() {
        Ok(None)
    } else {
        parse(value).map(Some)
    }
}
fn natural(value: &Value) -> Result<u64, Error> {
    // Check integer literals before f64 conversion: rounding must not admit an unsafe u64.
    // Number.isSafeInteger also accepts numeric 1.0 and -0, but not strings or booleans.
    if let Some(integer) = value.as_u64() {
        return (integer <= crate::codex::MAX_SAFE_INTEGER)
            .then_some(integer)
            .ok_or(Error::Protocol);
    }
    let number = value.as_f64().ok_or(Error::Protocol)?;
    if !number.is_finite()
        || number < 0.0
        || number > crate::codex::MAX_SAFE_INTEGER as f64
        || number.fract() != 0.0
    {
        return Err(Error::Protocol);
    }
    Ok(number as u64)
}
fn timeout(value: &Value) -> Result<CodexRuntimeNatural, Error> {
    CodexRuntimeNatural::new(natural(value)?).map_err(|_| Error::Protocol)
}
fn budget(value: &Value) -> Result<CodexImageBudget, Error> {
    CodexImageBudget::new(natural(value)?).map_err(|_| Error::Protocol)
}
fn values(value: &Value) -> Result<CodexRuntimeValues, Error> {
    exact(
        value,
        &[
            "transport",
            "cacheRetention",
            "timeoutMs",
            "websocketConnectTimeoutMs",
            "streamIdleTimeoutMs",
            "requestImagePixelBudget",
            "requestImageMaxBytes",
            "maxRequestImageBytes",
        ],
    )?;
    Ok(CodexRuntimeValues {
        transport: optional(&value["transport"], |value| {
            CodexRuntimeTransport::parse(value.as_str().ok_or(Error::Protocol)?)
                .ok_or(Error::Protocol)
        })?,
        cache_retention: optional(&value["cacheRetention"], |value| {
            CodexCacheRetention::parse(value.as_str().ok_or(Error::Protocol)?)
                .ok_or(Error::Protocol)
        })?,
        timeout_ms: optional(&value["timeoutMs"], timeout)?,
        websocket_connect_timeout_ms: optional(&value["websocketConnectTimeoutMs"], timeout)?,
        stream_idle_timeout_ms: optional(&value["streamIdleTimeoutMs"], |value| {
            CodexIdleMillis::new(value.as_f64().ok_or(Error::Protocol)?)
                .map_err(|_| Error::Protocol)
        })?,
        request_image_pixel_budget: optional(&value["requestImagePixelBudget"], budget)?,
        request_image_max_bytes: optional(&value["requestImageMaxBytes"], budget)?,
        max_request_image_bytes: optional(&value["maxRequestImageBytes"], budget)?,
    })
}
pub(crate) fn snapshot(value: &Value) -> Result<CodexRuntimeSnapshot, Error> {
    exact(
        value,
        &[
            "version",
            "revision",
            "configured",
            "writable",
            "values",
            "baseValues",
            "overrides",
        ],
    )?;
    if value["version"].as_f64() != Some(1.0) {
        return Err(Error::Protocol);
    }
    let revision = natural(&value["revision"])?;
    let configured = value["configured"].as_bool().ok_or(Error::Protocol)?;
    let writable = value["writable"].as_bool().ok_or(Error::Protocol)?;
    if writable && (!configured || revision == crate::codex::MAX_SAFE_INTEGER) {
        return Err(Error::Protocol);
    }
    Ok(CodexRuntimeSnapshot {
        version: 1,
        revision,
        configured,
        writable,
        values: values(&value["values"])?,
        base_values: values(&value["baseValues"])?,
        overrides: values(&value["overrides"])?,
    })
}
