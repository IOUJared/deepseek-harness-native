//! The selected session's goal projection and the one mutation the composer's goal strip can
//! carry. The projection is durable state; the mutation verbs are the canonical `goals/*`
//! transport calls (`packages/goal/goal/src/index.ts:328-440`).
use dsh_native_transport::dto::{GoalPhase, GoalRef};
use serde_json::Value;

/// One current durable goal, read from the `goal` projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Goal {
    pub reference: GoalRef,
    pub objective: String,
    pub phase: GoalPhase,
    /// Present exactly while the phase is blocked.
    pub blocked: Option<String>,
    pub max_goal_rounds: u64,
}

/// The goal projection value is `{goal:{id,revision,objective,phase,blockedReason?,
/// maxGoalRounds}, roundsStarted, createdAt, updatedAt}` (`types.ts:109-125`). A malformed or
/// absent goal yields `None`; the strip is simply not drawn.
pub fn parse(value: &Value) -> Option<Goal> {
    let goal = value.get("goal")?.as_object()?;
    let id = goal.get("id")?.as_str()?;
    if id.is_empty() || id.len() > 256 {
        return None;
    }
    let revision = goal.get("revision")?.as_u64()?;
    if revision == 0 {
        return None;
    }
    let objective = goal.get("objective")?.as_str()?;
    if objective.trim().is_empty() || objective.len() > 4096 {
        return None;
    }
    let phase = serde_json::from_value(goal.get("phase")?.clone()).ok()?;
    let max_goal_rounds = goal.get("maxGoalRounds")?.as_u64()?;
    let blocked = goal
        .get("blockedReason")
        .and_then(Value::as_object)
        .and_then(|reason| reason.get("message"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    Some(Goal {
        reference: GoalRef {
            id: id.to_owned(),
            revision,
        },
        objective: objective.to_owned(),
        phase,
        blocked,
        max_goal_rounds,
    })
}

pub fn phase_label(phase: GoalPhase) -> &'static str {
    match phase {
        GoalPhase::Active => "Ongoing Goal",
        GoalPhase::Paused => "Paused Goal",
        GoalPhase::Blocked => "Blocked Goal",
        GoalPhase::Complete => "Complete Goal",
    }
}

/// One admitted goal mutation, fenced to the session it was built for.
#[derive(Clone, Debug)]
pub enum Operation {
    Edit {
        reference: GoalRef,
        objective: String,
    },
    Pause(GoalRef),
    Resume(GoalRef),
    Clear(GoalRef),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn projection(phase: &str) -> Value {
        json!({"goal":{"id":"PUBLIC-goal","revision":2,"objective":"PUBLIC objective",
            "phase":phase,"maxGoalRounds":64,"roundsStarted":3},
            "roundsStarted":3,"createdAt":1000,"updatedAt":2000})
    }

    #[test]
    fn the_goal_projection_parses_and_refuses_malformed_refs() {
        let goal = parse(&projection("active")).unwrap();
        assert_eq!(goal.reference.id, "PUBLIC-goal");
        assert_eq!(goal.reference.revision, 2);
        assert_eq!(goal.phase, GoalPhase::Active);
        assert_eq!(phase_label(goal.phase), "Ongoing Goal");
        assert_eq!(parse(&json!({})), None);
        assert_eq!(parse(&json!({"goal":{"id":"","revision":1}})), None);
        let mut zero = projection("paused");
        zero["goal"]["revision"] = json!(0);
        assert_eq!(parse(&zero), None);
        let mut blocked = projection("blocked");
        blocked["goal"]["blockedReason"] = json!({"code":"PUBLIC","message":"PUBLIC blocked"});
        assert_eq!(
            parse(&blocked).unwrap().blocked.as_deref(),
            Some("PUBLIC blocked")
        );
    }
}
