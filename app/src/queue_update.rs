//! One operator queue mutation: edit, remove or steer a pending next-turn item. The transport
//! endpoint is canonical (`session/updateQueue`); this module only carries the request and the
//! redacted failure classes the dock announces.
use dsh_native_transport::{
    Error,
    dto::{QueueAction, SessionId},
};

/// Redacted failure classes for one queue mutation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// `session/queue-item-not-found`: the item started sending or was removed elsewhere.
    NotFound,
    /// The Host refused the edit (non-text, empty or malformed).
    Rejected,
    /// The command never left the native queue.
    NotSent,
    /// Admitted, but the outcome is unknown; never retry automatically.
    Unknown,
}

impl Failure {
    pub fn message(self) -> &'static str {
        match self {
            Self::NotFound => "This message may have already started sending.",
            Self::Rejected => "The edit was refused; the queued message is unchanged.",
            Self::NotSent => "Queue change not sent; no automatic retry.",
            Self::Unknown => {
                "Queue change not confirmed; the message may already have started sending."
            }
        }
    }
}

/// One admitted queue mutation, fenced to the session it was built for.
#[derive(Clone, Debug)]
pub struct Submission {
    pub session: SessionId,
    pub item_id: String,
    pub action: QueueAction,
}

pub fn failure(error: Error) -> Failure {
    match error {
        Error::QueueItemNotFound => Failure::NotFound,
        Error::QueueEditRejected => Failure::Rejected,
        Error::Closed | Error::InvalidDto => Failure::NotSent,
        _ => Failure::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_failures_keep_their_distinct_meanings() {
        assert_eq!(failure(Error::QueueItemNotFound), Failure::NotFound);
        assert_eq!(failure(Error::QueueEditRejected), Failure::Rejected);
        assert_eq!(failure(Error::Closed), Failure::NotSent);
        assert_eq!(failure(Error::Network), Failure::Unknown);
        assert_eq!(
            Failure::NotFound.message(),
            "This message may have already started sending."
        );
    }
}
