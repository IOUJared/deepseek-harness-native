//! Read-only plan-mode indicator from the Host's `plan` projection.
//!
//! Plan mode is soft guidance the deployment renders as the `plan:policy` prompt
//! section: the harness's own state, logged as `plan/mode` and published to
//! clients as exactly `{ active, pending }`. This module presents that state and
//! nothing else — it sends no command, and no control here can enter or leave
//! plan mode, because the native app has no selection authority for it.
use serde_json::{Map, Value};

/// What the Host reports about plan mode for the selected session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Active and settled: the next request carries the plan guidance.
    Active,
    /// A selection is waiting for an accepted in-turn pre-step to be appended.
    /// It is not active yet, so the label must not claim that it is.
    Applying,
    /// Inactive with a pending selection: the guidance is still in force until
    /// that selection lands.
    Leaving,
}

impl Mode {
    /// The composer's indicator text. Deliberately short: this is a standing
    /// state, not an action.
    pub fn label(self) -> &'static str {
        match self {
            Self::Active => "Plan mode",
            Self::Applying => "Plan mode · applying",
            Self::Leaving => "Leaving plan mode · applying",
        }
    }
}

fn flag(object: &Map<String, Value>, key: &str) -> Option<bool> {
    object.get(key).map(Value::as_bool).and_then(|value| value)
}

/// Read the `plan` projection. `None` when the Host published no such projection
/// (a deployment without the plan-mode plugin) or when it is malformed, so an
/// absent indicator means "not reported" rather than "inactive".
pub fn mode(projections: &std::collections::BTreeMap<String, Value>) -> Option<Mode> {
    let object = projections.get("plan")?.as_object()?;
    let active = flag(object, "active")?;
    let pending = flag(object, "pending").unwrap_or(false);
    Some(match (active, pending) {
        (true, false) => Mode::Active,
        (true, true) => Mode::Applying,
        (false, true) => Mode::Leaving,
        // Never reached: an inactive settled selection is no plan state at all.
        (false, false) => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeMap;

    fn projections(value: Value) -> BTreeMap<String, Value> {
        BTreeMap::from([("plan".into(), value)])
    }

    #[test]
    fn only_reported_plan_state_is_presented() {
        assert_eq!(
            mode(&projections(json!({"active": true, "pending": false}))),
            Some(Mode::Active)
        );
        assert_eq!(
            mode(&projections(json!({"active": true, "pending": true}))),
            Some(Mode::Applying)
        );
        assert_eq!(
            mode(&projections(json!({"active": false, "pending": true}))),
            Some(Mode::Leaving)
        );
        // Inactive and settled is the ordinary case: no indicator, no chip.
        assert_eq!(
            mode(&projections(json!({"active": false, "pending": false}))),
            None
        );
        assert_eq!(Mode::Active.label(), "Plan mode");
        assert_eq!(Mode::Applying.label(), "Plan mode · applying");
        assert_eq!(Mode::Leaving.label(), "Leaving plan mode · applying");
    }

    #[test]
    fn absent_or_malformed_projection_reports_nothing() {
        assert_eq!(mode(&BTreeMap::new()), None);
        // A deployment without the plan-mode plugin publishes no projection.
        assert_eq!(
            mode(&BTreeMap::from([(
                "contextPressure".into(),
                json!({"contextWindow": 128000})
            )])),
            None
        );
        for bad in [
            json!(null),
            json!("active"),
            json!([]),
            json!({}),
            json!({"active": "true", "pending": true}),
            json!({"active": null, "pending": true}),
        ] {
            assert_eq!(mode(&projections(bad.clone())), None, "{bad}");
        }
        // A missing `pending` is the settled reading, not a malformed one.
        assert_eq!(
            mode(&projections(json!({"active": true}))),
            Some(Mode::Active)
        );
    }
}
