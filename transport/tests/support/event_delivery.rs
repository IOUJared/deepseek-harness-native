//! Real native transport against an owned fake BFF, not a real Agent/Host composition.
use super::*;
use dsh_native_transport::{EventDelivery, Mux, NativeStream};
use serde_json::Value;
struct Fixture {
    child: Child,
    launch: Option<SecretLaunchUrl>,
}
impl Fixture {
    async fn start(mode: &str) -> Self {
        let mut child = Command::new("/usr/bin/node")
            .env_clear()
            .arg(format!(
                "{}/tests/support/event_delivery_fixture.mjs",
                env!("CARGO_MANIFEST_DIR")
            ))
            .arg(mode)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let url = tokio::time::timeout(Duration::from_secs(5), lines.next_line())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        Self {
            child,
            launch: Some(SecretLaunchUrl::new(url).unwrap()),
        }
    }
    async fn connect(&mut self) -> NativeClient {
        NativeClient::connect(
            self.launch.take().unwrap(),
            Limits {
                max_concurrent_rpc: 1,
                ..Limits::default()
            },
        )
        .await
        .unwrap()
    }
    async fn stop(mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}
async fn bounded<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(5), future)
        .await
        .expect("owned fixture operation deadline")
}
async fn root(mux: &Mux) -> NativeStream<RemoteEventFrame> {
    let mut root = mux.events().await.unwrap();
    let ready = bounded(root.next_event()).await.unwrap().unwrap();
    assert!(matches!(ready.frame, RemoteEventFrame::Ready { .. }));
    assert!(ready.delivery.is_none());
    root
}
async fn delivery(root: &mut NativeStream<RemoteEventFrame>) -> EventDelivery {
    let event = bounded(root.next_event()).await.unwrap().unwrap();
    assert!(matches!(event.frame, RemoteEventFrame::Waterfall { .. }));
    event.delivery.unwrap()
}
async fn trigger_reuse(mux: &Mux) {
    let mut barrier = mux.workspace_follow().await.unwrap();
    assert!(matches!(
        bounded(barrier.next()).await,
        Some(Ok(WorkspaceFollowFrame::Baseline { .. }))
    ));
    barrier.cancel().await;
}
async fn reused(root: &mut NativeStream<RemoteEventFrame>) -> EventDelivery {
    let cancel = bounded(root.next_event()).await.unwrap().unwrap();
    assert!(matches!(cancel.frame, RemoteEventFrame::Cancel { .. }));
    assert!(cancel.delivery.is_none());
    delivery(root).await
}
async fn stats(client: &NativeClient) -> Value {
    let value = client.session_list().await.unwrap();
    serde_json::from_str(value.items[0].cwd.as_deref().unwrap()).unwrap()
}

#[tokio::test]
async fn event_delivery_metadata_is_captured_at_actor_intake_not_late_dequeue() {
    let mut fixture = Fixture::start("dequeue").await;
    let client = fixture.connect().await;
    let mux = client.connect_mux().await.unwrap();
    let mut root = root(&mux).await;
    trigger_reuse(&mux).await; // Actor has processed Cancel+reuse before old Waterfall is dequeued.
    let old = delivery(&mut root).await;
    tokio::time::timeout(Duration::from_secs(2), old.cancelled())
        .await
        .unwrap();
    assert!(!old.is_live());
    let new = reused(&mut root).await;
    assert!(new.is_live());
    assert_eq!(
        client
            .reply_approval_for(&old, ApprovalOutcome::Rejected)
            .await,
        Err(Error::Correlation)
    );
    assert_eq!(stats(&client).await["posts"], 0);
    client
        .reply_approval_for(&new, ApprovalOutcome::Rejected)
        .await
        .unwrap();
    assert!(!new.is_live());
    assert_eq!(stats(&client).await["posts"], 1);
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn event_delivery_legacy_future_captures_before_first_poll_and_reuse() {
    let mut fixture = Fixture::start("dequeue").await;
    let client = fixture.connect().await;
    let mux = client.connect_mux().await.unwrap();
    let mut root = root(&mux).await;
    let old = delivery(&mut root).await;
    let reply = client.reply_approval(
        &RemoteEventId::new("delivery-event").unwrap(),
        ApprovalOutcome::Rejected,
    );
    trigger_reuse(&mux).await;
    let new = reused(&mut root).await;
    assert!(!old.is_live());
    assert_eq!(reply.await, Err(Error::Correlation));
    assert_eq!(stats(&client).await["posts"], 0);
    client
        .reply_approval_for(&new, ApprovalOutcome::Rejected)
        .await
        .unwrap();
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn event_delivery_cancel_reuse_while_legacy_reply_queues_never_sends_old_result() {
    let mut fixture = Fixture::start("queued").await;
    let client = fixture.connect().await;
    let mux = client.connect_mux().await.unwrap();
    let mut root = root(&mux).await;
    let old = delivery(&mut root).await;
    let probe = client.clone();
    let blocker = tokio::spawn(async move { probe.session_list().await });
    let held = bounded(root.next_event()).await.unwrap().unwrap();
    assert!(matches!(held.frame, RemoteEventFrame::Emit { .. }));
    let mut reply = Box::pin(client.reply_approval(
        &RemoteEventId::new("delivery-event").unwrap(),
        ApprovalOutcome::AllowedOnce,
    ));
    assert!(futures_util::poll!(&mut reply).is_pending());
    trigger_reuse(&mux).await;
    let new = reused(&mut root).await;
    bounded(blocker).await.unwrap().unwrap();
    assert!(!old.is_live());
    assert_eq!(reply.await, Err(Error::Correlation));
    assert_eq!(stats(&client).await["posts"], 0);
    client
        .reply_approval_for(&new, ApprovalOutcome::Rejected)
        .await
        .unwrap();
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn event_delivery_old_admitted_response_cannot_retire_new_same_id_client_kind() {
    let mut fixture = Fixture::start("ack").await;
    let client = fixture.connect().await;
    let mux = client.connect_mux().await.unwrap();
    let mut root = root(&mux).await;
    let old = delivery(&mut root).await;
    let sender = client.clone();
    let task = tokio::spawn(async move {
        sender
            .reply_approval_for(&old, ApprovalOutcome::Rejected)
            .await
    });
    let held = bounded(root.next_event()).await.unwrap().unwrap();
    assert!(matches!(held.frame, RemoteEventFrame::Emit { .. }));
    trigger_reuse(&mux).await;
    let new = reused(&mut root).await;
    let result = bounded(task).await.unwrap();
    assert!(matches!(result, Ok(()) | Err(Error::Correlation)));
    assert!(new.is_live()); // Whether old ACK raced cancellation, new entry is independently retained.
    client
        .reply_approval_for(&new, ApprovalOutcome::Rejected)
        .await
        .unwrap();
    assert_eq!(stats(&client).await["posts"], 2);
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn event_delivery_kind_rejection_sends_nothing_and_exact_question_cancel_retires_once() {
    let mut fixture = Fixture::start("question").await;
    let client = fixture.connect().await;
    let mux = client.connect_mux().await.unwrap();
    let mut root = root(&mux).await;
    let question = delivery(&mut root).await;
    assert_eq!(
        client
            .reply_approval_for(&question, ApprovalOutcome::AllowedOnce)
            .await,
        Err(Error::Correlation)
    );
    assert!(question.is_live());
    assert_eq!(stats(&client).await["posts"], 0);
    client.cancel_question_for(&question).await.unwrap();
    assert!(!question.is_live());
    assert_eq!(
        client.cancel_question_for(&question).await,
        Err(Error::Correlation)
    );
    assert_eq!(stats(&client).await["posts"], 1);
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn event_delivery_remote_failure_retains_instance_until_explicit_retry_ack() {
    let mut fixture = Fixture::start("failure").await;
    let client = fixture.connect().await;
    let mux = client.connect_mux().await.unwrap();
    let mut root = root(&mux).await;
    let owner = delivery(&mut root).await;
    assert_eq!(
        client
            .reply_approval_for(&owner, ApprovalOutcome::Rejected)
            .await,
        Err(Error::RemoteFailure)
    );
    assert!(owner.is_live());
    client
        .reply_approval_for(&owner, ApprovalOutcome::Rejected)
        .await
        .unwrap();
    assert!(!owner.is_live());
    assert_eq!(stats(&client).await["posts"], 2);
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn event_delivery_close_revokes_owner_and_legacy_lookup_cannot_send() {
    for generation in [false, true] {
        let mut fixture = Fixture::start("plain").await;
        let client = fixture.connect().await;
        let mux = client.connect_mux().await.unwrap();
        let mut root = root(&mux).await;
        let owner = delivery(&mut root).await;
        if generation {
            client.invalidate();
        }
        mux.close().await;
        tokio::time::timeout(Duration::from_secs(2), owner.cancelled())
            .await
            .unwrap();
        assert!(!owner.is_live());
        assert!(matches!(
            client
                .reply_approval_for(&owner, ApprovalOutcome::Rejected)
                .await,
            Err(Error::Closed) | Err(Error::Correlation)
        ));
        assert!(matches!(
            client
                .reply_approval(
                    &RemoteEventId::new("delivery-event").unwrap(),
                    ApprovalOutcome::Rejected
                )
                .await,
            Err(Error::Closed) | Err(Error::Correlation)
        ));
        client.close().await;
        fixture.stop().await;
    }
}
