//! Read-only plan-mode indicator from the Host's `plan` projection.
//!
//! Plan mode is soft guidance the deployment renders as the `plan:policy` prompt
//! section: the harness's own state, logged as `plan/mode` and published to
//! clients as exactly `{ active, pending }`. This module presents that state and
//! nothing else — it sends no command, and no control here can enter or leave
//! plan mode, because the native app has no selection authority for it.
use serde_json::{Map, Value};

/// What the Host reports about plan mode for the selected session.
///
/// The Host publishes `{ active, pending }` where `active` is the **logged** state and `pending`
/// says a selection differs from it (`plan-mode/src/index.ts:165`, `:396-399`). The reference
/// client turns that into the effective target `pending ? !active : active`
/// (`ui-plan/src/client/PlanModeControl.tsx:32-33`), which is what [`Mode::effective`] reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Active and settled: the next request carries the plan guidance.
    Active,
    /// Inactive with a pending selection: the guidance is **not** in force yet, and the pending
    /// selection is what will put it in force.
    Applying,
    /// Active with a pending selection: the guidance is still in force until that selection
    /// (which turns it off) lands.
    Leaving,
}

impl Mode {
    /// Whether plan mode is the state this session will be in — the reference's
    /// `target = pending ? !active : active`. `Leaving` is on its way out, so it is not.
    pub fn effective(self) -> bool {
        matches!(self, Self::Active | Self::Applying)
    }
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

/// Whether the Host published the `plan` projection at all.
///
/// Presence is the discovery signal: a deployment without the plan-mode plugin
/// publishes no `plan` key, and one that mounts it publishes the key even while
/// the mode is inactive (`active: false, pending: false`), where [`mode`] returns
/// `None` because there is no mode to show.
pub fn published(projections: &std::collections::BTreeMap<String, Value>) -> bool {
    projections
        .get("plan")
        .is_some_and(|value| value.is_object())
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
        // Logged active with a pending change: the guidance is still in force, and the
        // reference hides its chip in exactly this state because the target is off.
        (true, true) => Mode::Leaving,
        // Logged inactive with a pending change: the target is plan mode, so the reference keeps
        // its exit control visible and pressing it cancels the pending enter.
        (false, true) => Mode::Applying,
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
        // Logged active with a pending change: still in force, on its way out.
        assert_eq!(
            mode(&projections(json!({"active": true, "pending": true}))),
            Some(Mode::Leaving)
        );
        // Logged inactive with a pending change: not in force yet, on its way in.
        assert_eq!(
            mode(&projections(json!({"active": false, "pending": true}))),
            Some(Mode::Applying)
        );
        assert!(Mode::Leaving.effective() == false);
        assert!(Mode::Applying.effective());
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
