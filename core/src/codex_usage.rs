use crate::Error;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CodexComposerDisplay {
    #[default]
    Off,
    Percent,
    ProgressBar,
    Runway,
}
impl CodexComposerDisplay {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Percent => "percent",
            Self::ProgressBar => "progressBar",
            Self::Runway => "runway",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "off" => Some(Self::Off),
            "percent" => Some(Self::Percent),
            "progressBar" => Some(Self::ProgressBar),
            "runway" => Some(Self::Runway),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CodexUsageAlert {
    #[default]
    Off,
    Percent20,
    EarlyWarning,
    Custom,
}
impl CodexUsageAlert {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Percent20 => "percent20",
            Self::EarlyWarning => "earlyWarning",
            Self::Custom => "custom",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "off" => Some(Self::Off),
            "percent20" => Some(Self::Percent20),
            "earlyWarning" => Some(Self::EarlyWarning),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

/// Finite inclusive 0..=100, retaining fractions without coercion or truncation.
/// Private normalized bits preserve Eq for commands/replies without admitting NaN or -0.
///
/// ```compile_fail
/// let invalid = dsh_native_core::CodexRemainingPercent(f64::NAN.to_bits());
/// ```
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CodexRemainingPercent(u64);
impl CodexRemainingPercent {
    pub fn new(value: f64) -> Result<Self, Error> {
        if !value.is_finite() || !(0.0..=100.0).contains(&value) {
            return Err(Error::InvalidCommand);
        }
        Ok(Self(if value == 0.0 { 0 } else { value.to_bits() }))
    }
    pub const fn as_f64(self) -> f64 {
        f64::from_bits(self.0)
    }
}
impl Default for CodexRemainingPercent {
    fn default() -> Self {
        Self(20.0_f64.to_bits())
    }
}
impl std::fmt::Debug for CodexRemainingPercent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CodexRemainingPercent")
            .field(&self.as_f64())
            .finish()
    }
}

/// Five nonsecret local preferences; no account identity, quota observations or authority.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CodexUsagePreferences {
    pub composer_display: CodexComposerDisplay,
    pub usage_alert: CodexUsageAlert,
    pub custom_remaining_percent: CodexRemainingPercent,
    pub retry_after_reset: bool,
    pub experimental_quota: bool,
}
impl CodexUsagePreferences {
    pub(crate) fn encode(self) -> Value {
        json!({
            "composerDisplay": self.composer_display.as_str(),
            "usageAlert": self.usage_alert.as_str(),
            "customRemainingPercent": self.custom_remaining_percent.as_f64(),
            "retryAfterReset": self.retry_after_reset,
            "experimentalQuota": self.experimental_quota,
        })
    }
}

/// A private-worker persistence observation, not a quota or policy activation receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodexUsageSnapshot {
    pub version: u8,
    pub revision: u64,
    pub preferences: CodexUsagePreferences,
    pub persisted: bool,
}
impl Default for CodexUsageSnapshot {
    fn default() -> Self {
        Self {
            version: 1,
            revision: 0,
            preferences: CodexUsagePreferences::default(),
            persisted: false,
        }
    }
}

fn exact(value: &Value, keys: &[&str]) -> Result<(), Error> {
    let object = value.as_object().ok_or(Error::Protocol)?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(Error::Protocol);
    }
    Ok(())
}
fn preferences(value: &Value) -> Result<CodexUsagePreferences, Error> {
    exact(
        value,
        &[
            "composerDisplay",
            "usageAlert",
            "customRemainingPercent",
            "retryAfterReset",
            "experimentalQuota",
        ],
    )?;
    Ok(CodexUsagePreferences {
        composer_display: CodexComposerDisplay::parse(
            value["composerDisplay"].as_str().ok_or(Error::Protocol)?,
        )
        .ok_or(Error::Protocol)?,
        usage_alert: CodexUsageAlert::parse(value["usageAlert"].as_str().ok_or(Error::Protocol)?)
            .ok_or(Error::Protocol)?,
        custom_remaining_percent: CodexRemainingPercent::new(
            value["customRemainingPercent"]
                .as_f64()
                .ok_or(Error::Protocol)?,
        )
        .map_err(|_| Error::Protocol)?,
        retry_after_reset: value["retryAfterReset"].as_bool().ok_or(Error::Protocol)?,
        experimental_quota: value["experimentalQuota"]
            .as_bool()
            .ok_or(Error::Protocol)?,
    })
}
fn revision(value: &Value) -> Result<u64, Error> {
    // Like Number.isSafeInteger: numeric 1.0 and -0 are valid; strings/bools are not.
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
pub(crate) fn snapshot(value: &Value) -> Result<CodexUsageSnapshot, Error> {
    exact(value, &["version", "revision", "preferences", "persisted"])?;
    if value["version"].as_f64() != Some(1.0) {
        return Err(Error::Protocol);
    }
    Ok(CodexUsageSnapshot {
        version: 1,
        revision: revision(&value["revision"])?,
        preferences: preferences(&value["preferences"])?,
        persisted: value["persisted"].as_bool().ok_or(Error::Protocol)?,
    })
}
