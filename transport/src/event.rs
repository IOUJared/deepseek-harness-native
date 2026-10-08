//! Exact actor-intake delivery ownership; not a business answer or timed-wait claim.
use crate::{Error, Result, dto::*, http::Inner};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub(crate) struct PendingEvent {
    pub event_id: RemoteEventId,
    pub client_id: RemoteEventClientId,
    pub kind: WaterfallKind,
    pub lifetime: CancellationToken,
}

/// Opaque non-Clone/non-serializable authority for one exact received delivery instance.
/// Root Cancel/retirement, successful reply ACK, stream teardown and generation closure
/// revoke it. Drop only discards this local handle; it never answers or delegates.
/// Revocation cannot roll back an already transmitted result or prove its Host outcome.
///
/// ```compile_fail
/// fn replay(delivery: dsh_native_transport::EventDelivery) { let copy = delivery.clone(); }
/// ```
pub struct EventDelivery {
    pub(crate) pending: Arc<PendingEvent>,
}
impl EventDelivery {
    pub(crate) fn new(pending: Arc<PendingEvent>) -> Self {
        Self { pending }
    }
    /// Delivery has not yet been retired; not exclusive answer or business-settlement proof.
    pub fn is_live(&self) -> bool {
        !self.pending.lifetime.is_cancelled()
    }
    /// Wait for any retirement, including this client's own successful RPC ACK.
    pub async fn cancelled(&self) {
        self.pending.lifetime.cancelled().await;
    }
    pub(crate) fn check(&self, inner: &Inner, expected: Option<WaterfallKind>) -> Result<()> {
        if inner.lifetime.is_cancelled() {
            return Err(Error::Closed);
        }
        if !self.is_live() || expected.is_some_and(|kind| kind != self.pending.kind) {
            return Err(Error::Correlation);
        }
        let pending = inner.pending.lock().map_err(|_| Error::Closed)?;
        if !pending
            .get(&self.pending.event_id)
            .is_some_and(|current| Arc::ptr_eq(current, &self.pending))
        {
            return Err(Error::Correlation);
        }
        Ok(())
    }
}

/// Native stream observation with the capability captured at actor intake, not at dequeue.
/// Only Waterfall has a delivery. Raw wire DTO/GUI state can remain separately cloneable.
/// The capability stays worker-private; no serialization, credential or socket accessors.
pub struct OwnedRemoteEvent {
    pub frame: RemoteEventFrame,
    pub delivery: Option<EventDelivery>,
}
