//! One acquired timed-question stream, retained independently of reply RPC receipts.
use crate::{Error, NativeStream, Result, dto::QuestionWait};

/// A non-Clone, non-serializable worker-owned claim after its first validated Host item.
///
/// Any Host claim suppresses unattended expiry without rebasing its original deadline.
/// A reply RPC ACK, local draft submission, panel dismissal, or zero opening duration
/// does not release this owner. Poll `wait_closed` until the Host ends it, or explicitly
/// release during teardown. Stream end has no answer/timeout/cancellation reason.
/// This is not exclusive answer authority or an event/Agent identity discovery API.
///
/// ```compile_fail
/// fn duplicate(claim: dsh_native_transport::QuestionClaim) {
///     let replay = claim.clone();
/// }
/// ```
#[must_use = "Keep the acquired claim alive until Host stream end or explicit teardown"]
pub struct QuestionClaim {
    stream: NativeStream<QuestionWait>,
    opening_remaining_ms: u64,
    closed: Option<Result<()>>,
}
impl QuestionClaim {
    pub(crate) fn acquired(stream: NativeStream<QuestionWait>, opening_remaining_ms: u64) -> Self {
        Self {
            stream,
            opening_remaining_ms,
            closed: None,
        }
    }
    /// Initial Host duration sample; not a ticking countdown, renewed TTL or deadline.
    /// Zero can be valid while another client holds the original expired wait open.
    pub fn opening_remaining_ms(&self) -> u64 {
        self.opening_remaining_ms
    }
    /// Wait for logical Host stream end, without imposing a new answer deadline.
    ///
    /// Safe to abandon this read and poll again: the claim itself remains owned.
    /// Normal `Ok(())` means only stream end, never accepted answer/tool success.
    /// Errors preserve fixed transport classifications, not business timeout inference.
    /// Repeated reads preserve the recorded terminal result, even after later generation closure.
    /// Generation/mux closure ends a still-live claim with a fixed transport error.
    pub async fn wait_closed(&mut self) -> Result<()> {
        if let Some(result) = self.closed {
            return result;
        }
        let result = match self.stream.next().await {
            None => Ok(()),
            Some(Err(error)) => Err(error),
            Some(Ok(_)) => Err(Error::Sequence),
        };
        self.closed = Some(result);
        result
    }
    /// Consume and locally unsubscribe; no Host release ACK, question rejection or Turn cancellation.
    /// Last-claim release can immediately resume expiry against the original Host deadline.
    /// Drop requests the same local unsubscribe; generation close additionally awaits the mux actor.
    pub async fn release(mut self) {
        self.stream.cancel().await;
    }
}
