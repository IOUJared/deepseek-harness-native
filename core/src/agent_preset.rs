//! The agent-preset family's typed client: the roster the Host publishes, bounded like every
//! other family here. Only the five fields `AgentPresetRow` declares are projected, and a
//! roster with no default fails closed, since without one a client cannot say which preset a
//! new session receives (`api-catalog.ts:4605-4611`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Error;

/// One declared preset, reduced to the fields a client renders. `broken` marks a declared
/// preset whose activation failed, which the reference keeps visible.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPresetRow {
    pub id: String,
    pub is_default: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub broken: Option<String>,
}

/// Every declared preset, exactly one of which the deployment marks as its default.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentPresetRoster {
    pub presets: Vec<AgentPresetRow>,
}

/// A preset id is the Host's own key: bounded, non-empty, no control or format characters.
pub fn valid_id(value: &str) -> bool {
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

const MAX_PRESETS: usize = 64;

fn text(value: &Value, limit: usize) -> Option<String> {
    let text = value.as_str()?;
    (text.chars().count() <= limit && !text.is_empty()).then(|| text.to_owned())
}

fn row(value: &Value) -> Option<AgentPresetRow> {
    let id = value.get("id").and_then(Value::as_str)?;
    if !valid_id(id) {
        return None;
    }
    Some(AgentPresetRow {
        id: id.to_owned(),
        is_default: value.get("isDefault").and_then(Value::as_bool) == Some(true),
        name: value.get("name").and_then(|name| text(name, 256)),
        description: value
            .get("description")
            .and_then(|description| text(description, 1024)),
        broken: value.get("broken").and_then(|broken| text(broken, 512)),
    })
}

/// Project one `agent-presets` reply. A malformed roster fails closed rather than leaving a
/// client to guess which preset a new session would receive.
pub fn roster(value: &Value) -> Result<AgentPresetRoster, Error> {
    let presets = value
        .get("roster")
        .and_then(|roster| roster.get("presets"))
        .and_then(Value::as_array)
        .ok_or(Error::Protocol)?;
    if presets.is_empty() || presets.len() > MAX_PRESETS {
        return Err(Error::Protocol);
    }
    let presets = presets
        .iter()
        .map(row)
        .collect::<Option<Vec<_>>>()
        .ok_or(Error::Protocol)?;
    if !presets.iter().any(|preset| preset.is_default) {
        return Err(Error::Protocol);
    }
    Ok(AgentPresetRoster { presets })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn reply() -> Value {
        json!({"ok": true, "roster": {"presets": [
            {"id": "implementer", "isDefault": true, "name": "Implementer", "description": "Writes code", "secret": "PUBLIC_FAKE_SECRET"},
            {"id": "reviewer", "isDefault": false, "name": "Reviewer"},
            {"id": "broken-one", "isDefault": false, "broken": "activation failed"},
        ], "secret": "PUBLIC_FAKE_SECRET"}})
    }

    #[test]
    fn the_roster_keeps_only_rendered_fields_and_never_a_stray_payload() {
        let roster = roster(&reply()).unwrap();
        assert_eq!(roster.presets.len(), 3);
        assert!(roster.presets[0].is_default);
        assert_eq!(
            roster.presets[0].description.as_deref(),
            Some("Writes code")
        );
        assert_eq!(roster.presets[1].name.as_deref(), Some("Reviewer"));
        assert_eq!(
            roster.presets[2].broken.as_deref(),
            Some("activation failed")
        );
        let encoded = serde_json::to_string(&roster).unwrap();
        assert!(!encoded.contains("PUBLIC_FAKE_SECRET"));
        assert!(!encoded.contains("secret"));
        // The JSON field the Host uses round-trips under its own name.
        assert!(encoded.contains("\"isDefault\":true"));
    }

    #[test]
    fn malformed_rosters_fail_closed_and_a_missing_default_is_malformed() {
        assert!(roster(&json!({})).is_err());
        assert!(roster(&json!({"roster": {}})).is_err());
        assert!(roster(&json!({"roster": {"presets": []}})).is_err());
        assert!(roster(&json!({"roster": {"presets": [{"isDefault": true}]}})).is_err());
        assert!(
            roster(&json!({"roster": {"presets": [{"id": "a\u{0}b", "isDefault": true}]}}))
                .is_err()
        );
        assert!(
            roster(&json!({"roster": {"presets": [{"id": "a", "isDefault": false}]}})).is_err()
        );
        let many = (0..200)
            .map(|index| json!({"id": format!("p{index}"), "isDefault": index == 0}))
            .collect::<Vec<_>>();
        assert!(roster(&json!({"roster": {"presets": many}})).is_err());
    }

    #[test]
    fn preset_ids_are_bounded_like_the_bridge_requires() {
        assert!(valid_id("implementer"));
        assert!(!valid_id(""));
        assert!(!valid_id(&"x".repeat(129)));
        assert!(!valid_id("bad\u{0}id"));
        assert!(!valid_id("bad\u{2028}id"));
    }
}
