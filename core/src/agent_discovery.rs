//! Read-only native registry observation, never a retained Agent lease or authorization grant.
use crate::{Error, PublicReply};
use dsh_native_transport::dto::{AgentId, SessionId};
use serde_json::Value;

/// One correlated registry read. None means no Agent registered at this read, not an idle/resumable Agent.
/// A returned ID must still be resolved by the Gateway for each later scoped operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentDiscovery {
    pub session_id: SessionId,
    pub agent_id: Option<AgentId>,
}

pub(crate) fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && !value
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_control())
}

pub(crate) fn decode(value: &Value) -> Result<PublicReply, Error> {
    let object = value.as_object().ok_or(Error::Protocol)?;
    if object.len() != 2 || !object.contains_key("sessionId") || !object.contains_key("agentId") {
        return Err(Error::Protocol);
    }
    let session = object["sessionId"]
        .as_str()
        .filter(|id| valid_id(id))
        .ok_or(Error::Protocol)?;
    let agent_id = match &object["agentId"] {
        Value::Null => None,
        Value::String(id) if valid_id(id) => {
            Some(AgentId::new(id.clone()).map_err(|_| Error::Protocol)?)
        }
        _ => return Err(Error::Protocol),
    };
    Ok(PublicReply::AgentDiscovery(AgentDiscovery {
        session_id: SessionId::new(session).map_err(|_| Error::Protocol)?,
        agent_id,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn discovery_preserves_distinct_semantic_ids_and_explicit_absence() {
        let PublicReply::AgentDiscovery(result) =
            decode(&json!({"sessionId":"session-a","agentId":"agent-b"})).unwrap()
        else {
            panic!("discovery");
        };
        assert_eq!(result.session_id.as_str(), "session-a");
        assert_eq!(result.agent_id.unwrap().as_str(), "agent-b");
        let PublicReply::AgentDiscovery(result) =
            decode(&json!({"sessionId":"session-a","agentId":null})).unwrap()
        else {
            panic!("discovery");
        };
        assert!(result.agent_id.is_none());
    }
    #[test]
    fn discovery_rejects_partial_extra_invalid_and_oversized_fields() {
        for value in [
            json!({}),
            json!({"sessionId":"a"}),
            json!({"sessionId":"a","agentId":false}),
            json!({"sessionId":"a","agentId":null,"token":"PUBLIC_EXTRA"}),
            json!({"sessionId":"a","agentId":""}),
            json!({"sessionId":"a","agentId":"x".repeat(513)}),
            json!({"sessionId":"a","agentId":"x\n"}),
            json!({"sessionId":"a","agentId":"x\u{0085}"}),
            json!({"sessionId":"","agentId":null}),
        ] {
            assert_eq!(decode(&value), Err(Error::Protocol));
        }
        assert!(valid_id(&"é".repeat(256)));
        assert!(!valid_id(&"é".repeat(257)));
    }
    #[test]
    fn command_is_closed_and_request_ids_are_not_agent_ids() {
        let command = crate::Command::DiscoverSessionAgent(SessionId::new("selected").unwrap());
        assert_eq!(
            command.encode("request-7").unwrap(),
            json!({"type":"discover-session-agent","requestId":"request-7","sessionId":"selected"})
        );
        assert_eq!(
            crate::Command::DiscoverSessionAgent(SessionId::new("a b").unwrap())
                .encode("request-8"),
            Err(Error::InvalidCommand)
        );
    }
}
