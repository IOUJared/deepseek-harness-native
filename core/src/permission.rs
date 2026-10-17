//! The permission-preset family's typed client: the catalog the Host publishes and the
//! acknowledgement of a preset switch. Bounded like every other family here — the reply
//! carries only the fields a client renders, and nothing on the wire reaches this module
//! unvalidated (`docs/NATIVE-UI-PARITY.md`, permission select).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Error;

/// One selectable preset, reduced to the three fields a client renders.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresetOption {
    pub value: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// The deployment's catalog: what is selectable, what a fresh session defaults to, and
/// which of those the deployment marks as its default.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionCatalog {
    pub options: Vec<PresetOption>,
    pub default_options: Vec<PresetOption>,
    pub default_preset: String,
}

/// A preset name is the Host's own key: bounded, non-empty, no control or format
/// characters, and never longer than the bridge accepts.
pub fn valid_name(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= 128
        && value.len() <= 512
        && !value.chars().any(|c| {
            c.is_control()
                || matches!(
                    c,
                    '\u{2028}' | '\u{2029}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}'
                )
        })
}

const MAX_OPTIONS: usize = 64;
const MAX_TEXT: usize = 1024;

fn text(value: &Value, limit: usize) -> Option<String> {
    let text = value.as_str()?;
    (text.chars().count() <= limit && !text.is_empty()).then(|| text.to_owned())
}

fn option(value: &Value) -> Option<PresetOption> {
    let value_field = value.get("value").and_then(Value::as_str)?;
    if !valid_name(value_field) {
        return None;
    }
    let name = value
        .get("name")
        .and_then(|name| text(name, 256))
        .unwrap_or_else(|| value_field.to_owned());
    Some(PresetOption {
        value: value_field.to_owned(),
        name,
        description: value
            .get("description")
            .and_then(|description| text(description, MAX_TEXT)),
    })
}

fn options(value: &Value) -> Option<Vec<PresetOption>> {
    let list = value.as_array()?;
    if list.is_empty() || list.len() > MAX_OPTIONS {
        return None;
    }
    list.iter().map(option).collect()
}

/// Project one `permission-catalog` reply. A malformed reply fails closed rather than
/// presenting a partial catalog.
pub fn catalog(value: &Value) -> Result<PermissionCatalog, Error> {
    let catalog = value.get("catalog").ok_or(Error::Protocol)?;
    let options = catalog
        .get("options")
        .and_then(options)
        .ok_or(Error::Protocol)?;
    let default_options = catalog
        .get("defaultOptions")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(option).take(MAX_OPTIONS).collect())
        .unwrap_or_default();
    let default_preset = catalog
        .get("defaultPreset")
        .and_then(Value::as_str)
        .filter(|preset| valid_name(preset))
        .map(str::to_owned)
        .ok_or(Error::Protocol)?;
    // A default must name a selectable option; anything else is a malformed catalog.
    if !options.iter().any(|option| option.value == default_preset) {
        return Err(Error::Protocol);
    }
    Ok(PermissionCatalog {
        options,
        default_options,
        default_preset,
    })
}

/// The deployment's new-session default, as its settings descriptor reports it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionSettingsState {
    pub current_value: String,
    pub revision: u64,
    pub writable: bool,
}

/// Project one `permission-settings` reply. A malformed descriptor fails closed.
pub fn settings(value: &Value) -> Result<PermissionSettingsState, Error> {
    let settings = value.get("settings").ok_or(Error::Protocol)?;
    let current_value = settings
        .get("currentValue")
        .and_then(Value::as_str)
        .filter(|preset| valid_name(preset))
        .map(str::to_owned)
        .ok_or(Error::Protocol)?;
    let revision = settings
        .get("revision")
        .and_then(Value::as_u64)
        .filter(|revision| *revision <= crate::codex::MAX_SAFE_INTEGER)
        .ok_or(Error::Protocol)?;
    let writable = settings
        .get("writable")
        .and_then(Value::as_bool)
        .ok_or(Error::Protocol)?;
    Ok(PermissionSettingsState {
        current_value,
        revision,
        writable,
    })
}

/// A default write is acknowledged by `ok: true` alone; any other reply is refused.
pub fn default_ack(value: &Value) -> Result<(), Error> {
    set_ack(value)
}

/// A preset switch is acknowledged by `ok: true` alone; any other reply is refused.
pub fn set_ack(value: &Value) -> Result<(), Error> {
    match value.get("ok").and_then(Value::as_bool) {
        Some(true) => Ok(()),
        _ => Err(Error::Protocol),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn catalog_value() -> Value {
        json!({"ok": true, "catalog": {
            "options": [
                {"value": "read-only", "name": "Read only", "description": "Ask first"},
                {"value": "workspace-write", "name": "Workspace write"},
                {"value": "full-access", "name": "Full access"},
            ],
            "defaultOptions": [{"value": "workspace-write"}],
            "defaultPreset": "workspace-write",
            "secret": "PUBLIC_FAKE_SECRET",
        }})
    }

    #[test]
    fn the_catalog_keeps_only_rendered_fields_and_never_a_stray_payload() {
        let catalog = catalog(&catalog_value()).unwrap();
        assert_eq!(catalog.options.len(), 3);
        assert_eq!(catalog.options[0].description.as_deref(), Some("Ask first"));
        assert_eq!(catalog.options[1].description, None);
        assert_eq!(catalog.default_preset, "workspace-write");
        // A missing label falls back to the preset key, as the Host's own optionOf does.
        assert_eq!(catalog.default_options[0].name, "workspace-write");
        let encoded = serde_json::to_string(&catalog).unwrap();
        assert!(!encoded.contains("PUBLIC_FAKE_SECRET"));
        assert!(!encoded.contains("secret"));
    }

    #[test]
    fn each_permission_reply_matches_only_its_command() {
        use crate::{PublicReply, protocol::matches_reply};

        let catalog_reply = PublicReply::PermissionCatalog(catalog(&catalog_value()).unwrap());
        let settings_reply = PublicReply::PermissionSettings(
            settings(&json!({
                "settings": {"currentValue": "ask", "revision": 1, "writable": true},
            }))
            .unwrap(),
        );
        assert!(matches_reply("permission-catalog", &catalog_reply));
        assert!(matches_reply("permission-set", &PublicReply::PermissionSet));
        assert!(matches_reply("permission-settings", &settings_reply));
        assert!(matches_reply(
            "permission-default-set",
            &PublicReply::PermissionDefaultSet
        ));
        // A decoded reply for another command is refused, not accepted by family.
        assert!(!matches_reply("permission-set", &catalog_reply));
        assert!(!matches_reply(
            "permission-catalog",
            &PublicReply::PermissionSet
        ));
        assert!(!matches_reply(
            "permission-settings",
            &PublicReply::PermissionDefaultSet
        ));
    }

    #[test]
    fn malformed_catalogs_fail_closed() {
        assert!(catalog(&json!({})).is_err());
        assert!(catalog(&json!({"catalog": {}})).is_err());
        assert!(catalog(&json!({"catalog": {"options": []}})).is_err());
        assert!(catalog(&json!({"catalog": {"options": [{"name": "no value"}]}})).is_err());
        assert!(catalog(&json!({"catalog": {"options": [{"value": "bad\u{0}name"}]}})).is_err());
        // A default that names nothing selectable is malformed, not defaulted.
        assert!(
            catalog(&json!({"catalog": {
                "options": [{"value": "read-only"}],
                "defaultPreset": "missing"
            }}))
            .is_err()
        );
        let many = (0..200)
            .map(|index| json!({"value": format!("p{index}")}))
            .collect::<Vec<_>>();
        assert!(catalog(&json!({"catalog": {"options": many}})).is_err());
    }

    #[test]
    fn the_default_descriptor_projects_its_three_fields_and_fails_closed() {
        let state = settings(&json!({
            "ok": true,
            "settings": {"currentValue": "workspace-write", "revision": 9, "writable": true, "secret": "PUBLIC_FAKE_SECRET"},
        }))
        .unwrap();
        assert_eq!(
            state,
            PermissionSettingsState {
                current_value: "workspace-write".to_owned(),
                revision: 9,
                writable: true,
            }
        );
        let encoded = serde_json::to_string(&state).unwrap();
        assert!(!encoded.contains("PUBLIC_FAKE_SECRET"));
        // Every field is required and bounded; nothing is defaulted.
        assert!(settings(&json!({})).is_err());
        assert!(settings(&json!({"settings": {}})).is_err());
        assert!(
            settings(&json!({"settings": {"currentValue": "", "revision": 1, "writable": true}}))
                .is_err()
        );
        assert!(
            settings(&json!({"settings": {"currentValue": "a", "revision": -1, "writable": true}}))
                .is_err()
        );
        assert!(settings(&json!({"settings": {"currentValue": "a", "revision": 1}})).is_err());
        assert!(settings(&json!({"settings": {"currentValue": "a", "revision": 18446744073709551615u64, "writable": true}})).is_err());
        assert!(default_ack(&json!({"ok": true})).is_ok());
        assert!(default_ack(&json!({"ok": false})).is_err());
    }

    #[test]
    fn a_switch_is_acknowledged_by_ok_true_alone() {
        assert!(set_ack(&json!({"ok": true})).is_ok());
        assert!(set_ack(&json!({"ok": false})).is_err());
        assert!(set_ack(&json!({})).is_err());
        assert!(set_ack(&json!({"ok": "true"})).is_err());
    }

    #[test]
    fn preset_names_are_bounded_like_the_bridge_requires() {
        assert!(valid_name("workspace-write"));
        assert!(valid_name("auto"));
        assert!(!valid_name(""));
        assert!(!valid_name(&"x".repeat(129)));
        assert!(!valid_name("bad\u{0}name"));
        assert!(!valid_name("bad\u{2028}name"));
    }
}
