//! Authenticated fake-wire ownership checks, not a real Harness/timed-UI qualification.
use super::*;
use dsh_native_transport::{Mux, QuestionClaim};
use serde_json::{Value, json};

fn authority() -> (AgentId, ToolCallId) {
    (
        AgentId::new("agent-fixture").unwrap(),
        ToolCallId::new("call-1").unwrap(),
    )
}
async fn acquire(mux: &Mux) -> Result<QuestionClaim, Error> {
    let (agent, call) = authority();
    mux.hold_question(&agent, &call).await
}
async fn stats(client: &NativeClient) -> Value {
    let rows = client.session_list().await.unwrap();
    serde_json::from_str(rows.items[0].cwd.as_deref().unwrap()).unwrap()
}
fn metadata_only_requests(state: &Value) {
    assert!(
        state["methods"]
            .as_object()
            .unwrap()
            .keys()
            .all(|key| key == "session/list")
    );
}
async fn until(client: &NativeClient, key: &str, expected: u64) -> Value {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let state = stats(client).await;
            if state[key] == expected {
                return state;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("bounded fixture state convergence")
}
async fn end_fixture_claims(client: &NativeClient) {
    // Explicit fake-server end barrier, not an implicit QuestionClaim API operation.
    client
        .session_cancel(&SessionId::new("session / opaque:%").unwrap())
        .await
        .unwrap();
}
async fn question_event(
    mux: &Mux,
) -> (
    dsh_native_transport::NativeStream<RemoteEventFrame>,
    RemoteEventId,
) {
    let mut events = mux.events().await.unwrap();
    assert!(matches!(
        events.next().await,
        Some(Ok(RemoteEventFrame::Ready { .. }))
    ));
    let Some(Ok(RemoteEventFrame::Waterfall {
        event_id,
        agent_id,
        event: WaterfallKind::Question,
        request,
    })) = events.next().await
    else {
        panic!("expected explicit live fixture question authority")
    };
    assert_eq!(agent_id, authority().0);
    assert_eq!(request["wait"], json!({"callId":"call-1","timed":true}));
    (events, event_id)
}
fn answer() -> QuestionAnswer {
    QuestionAnswer {
        answers: vec![QuestionAnswerItem {
            id: "q".into(),
            selected: vec!["yes".into()],
            custom: None,
        }],
    }
}

#[tokio::test]
async fn question_claim_zero_and_nonzero_are_host_samples_not_local_expiry() {
    for mode in ["claim-zero", "claim-normal"] {
        let mut fixture = NodeHostFixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await.unwrap();
        let mux = client.connect_mux().await.unwrap();
        let mut claim = acquire(&mux).await.unwrap();
        assert_eq!(
            claim.opening_remaining_ms(),
            if mode == "claim-zero" { 0 } else { 1234 }
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(20), claim.wait_closed())
                .await
                .is_err()
        );
        assert_eq!(stats(&client).await["active"], 1);
        end_fixture_claims(&client).await;
        assert_eq!(claim.wait_closed().await, Ok(()));
        client.invalidate();
        assert_eq!(claim.wait_closed().await, Ok(())); // Observed terminal remains sticky.
        mux.close().await;
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn question_claim_end_before_first_is_unavailable_not_timeout_or_success() {
    for (mode, expected) in [
        ("claim-end-before", Error::QuestionUnavailable),
        ("claim-error-before", Error::RemoteFailure),
    ] {
        let mut fixture = NodeHostFixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await.unwrap();
        let mux = client.connect_mux().await.unwrap();
        assert_eq!(acquire(&mux).await.err(), Some(expected));
        assert!(!format!("{expected:?} {expected}").contains("PRIVATE"));
        assert_eq!(stats(&client).await["replyRequests"], 0);
        mux.close().await;
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn question_claim_opening_requires_one_object_with_unsigned_integer_only() {
    for mode in [
        "claim-array",
        "claim-empty",
        "claim-extra",
        "claim-negative",
        "claim-fraction",
        "claim-string",
        "claim-null",
        "claim-overflow",
    ] {
        let mut fixture = NodeHostFixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await.unwrap();
        let mux = client.connect_mux().await.unwrap();
        assert_eq!(acquire(&mux).await.err(), Some(Error::InvalidDto), "{mode}");
        let state = until(&client, "cancels", 1).await;
        assert_eq!(state["active"], 0);
        assert_eq!(state["replyRequests"], 0);
        metadata_only_requests(&state);
        let mut sibling = mux.workspace_follow().await.unwrap();
        assert!(matches!(
            sibling.next().await,
            Some(Ok(WorkspaceFollowFrame::Baseline { .. }))
        ));
        mux.close().await;
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn question_claim_duplicate_and_remote_terminal_errors_remain_sticky() {
    for (mode, expected) in [
        ("claim-duplicate", Error::Sequence),
        ("claim-error-after", Error::RemoteFailure),
    ] {
        let mut fixture = NodeHostFixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await.unwrap();
        let mux = client.connect_mux().await.unwrap();
        let mut claim = acquire(&mux).await.unwrap();
        assert_eq!(claim.wait_closed().await, Err(expected));
        assert_eq!(claim.wait_closed().await, Err(expected));
        if mode == "claim-duplicate" {
            assert_eq!(until(&client, "cancels", 1).await["active"], 0);
        }
        let mut sibling = mux.workspace_follow().await.unwrap();
        assert!(matches!(
            sibling.next().await,
            Some(Ok(WorkspaceFollowFrame::Baseline { .. }))
        ));
        mux.close().await;
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn question_claim_opening_and_end_can_already_be_buffered_together() {
    let mut fixture = NodeHostFixture::start("claim-immediate-end").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let mux = client.connect_mux().await.unwrap();
    let mut claim = acquire(&mux).await.unwrap();
    assert_eq!(claim.opening_remaining_ms(), 1234);
    assert_eq!(claim.wait_closed().await, Ok(()));
    assert_eq!(stats(&client).await["replyRequests"], 0);
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn question_claim_ack_without_end_does_not_release_or_prove_settlement() {
    let mut fixture = NodeHostFixture::start("claim-ack-first").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let mux = client.connect_mux().await.unwrap();
    let (_events, event) = question_event(&mux).await;
    let mut claim = acquire(&mux).await.unwrap();
    client.reply_question(&event, answer()).await.unwrap();
    assert_eq!(stats(&client).await["active"], 1);
    assert!(
        tokio::time::timeout(Duration::from_millis(20), claim.wait_closed())
            .await
            .is_err()
    );
    assert_eq!(
        client.reply_question(&event, answer()).await,
        Err(Error::Correlation)
    );
    assert_eq!(stats(&client).await["replyRequests"], 1);
    end_fixture_claims(&client).await;
    assert_eq!(claim.wait_closed().await, Ok(()));
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn question_claim_host_end_can_precede_reply_ack() {
    let mut fixture = NodeHostFixture::start("claim-end-first").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let mux = client.connect_mux().await.unwrap();
    let (_events, event) = question_event(&mux).await;
    let mut claim = acquire(&mux).await.unwrap();
    let reply = client.reply_question(&event, answer());
    tokio::pin!(reply);
    tokio::select! {
        biased;
        result = claim.wait_closed() => assert_eq!(result, Ok(())),
        result = &mut reply => panic!("fixture ACK must be held behind Host end: {result:?}"),
    }
    assert_eq!(reply.await, Ok(()));
    assert_eq!(claim.wait_closed().await, Ok(()));
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn question_claim_reply_failure_preserves_healthy_lease_and_correlation() {
    let mut fixture = NodeHostFixture::start("claim-reply-failure").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let mux = client.connect_mux().await.unwrap();
    let (_events, event) = question_event(&mux).await;
    let mut claim = acquire(&mux).await.unwrap();
    assert_eq!(
        client.reply_question(&event, answer()).await,
        Err(Error::HttpStatus(503))
    );
    assert_eq!(stats(&client).await["active"], 1);
    assert!(
        tokio::time::timeout(Duration::from_millis(20), claim.wait_closed())
            .await
            .is_err()
    );
    // No automatic retry is issued by the claim object; the pending delivery is not retired.
    assert_eq!(stats(&client).await["replyRequests"], 1);
    claim.release().await;
    assert_eq!(until(&client, "cancels", 1).await["rejects"], 0);
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn question_claim_drop_or_explicit_release_only_unsubscribes_its_stream() {
    for explicit in [false, true] {
        let mut fixture = NodeHostFixture::start("claim-normal").await;
        let client = fixture.connect(Limits::default()).await.unwrap();
        let mux = client.connect_mux().await.unwrap();
        let claim = acquire(&mux).await.unwrap();
        if explicit {
            claim.release().await;
        } else {
            drop(claim);
        }
        let state = until(&client, "cancels", 1).await;
        assert_eq!(state["active"], 0);
        assert_eq!(state["replyRequests"], 0);
        metadata_only_requests(&state);
        assert_eq!(state["rejects"], 0);
        let mut sibling = mux.workspace_follow().await.unwrap();
        assert!(matches!(
            sibling.next().await,
            Some(Ok(WorkspaceFollowFrame::Baseline { .. }))
        ));
        mux.close().await;
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn question_claim_acquisition_timeout_cancels_open_and_late_frame_is_retired() {
    let mut fixture = NodeHostFixture::start("claim-slow-first").await;
    let client = fixture
        .connect(Limits {
            timeout: Duration::from_millis(80),
            ..Limits::default()
        })
        .await
        .unwrap();
    let mux = client.connect_mux().await.unwrap();
    assert_eq!(acquire(&mux).await.err(), Some(Error::Timeout));
    assert_eq!(until(&client, "cancels", 1).await["active"], 0);
    until(&client, "lateFramesSent", 1).await; // Explicit fake-frame emission barrier before same-socket sibling.
    let mut sibling = mux.workspace_follow().await.unwrap();
    assert!(matches!(
        sibling.next().await,
        Some(Ok(WorkspaceFollowFrame::Baseline { .. }))
    ));
    assert_eq!(stats(&client).await["replyRequests"], 0);
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn question_claim_mux_and_generation_close_revoke_live_wait_without_success_inference() {
    for invalidation in [false, true] {
        let mut fixture = NodeHostFixture::start("claim-normal").await;
        let client = fixture.connect(Limits::default()).await.unwrap();
        let mux = client.connect_mux().await.unwrap();
        let mut claim = acquire(&mux).await.unwrap();
        if invalidation {
            client.invalidate();
        }
        mux.close().await;
        assert_eq!(claim.wait_closed().await, Err(Error::Closed));
        assert_eq!(claim.wait_closed().await, Err(Error::Closed));
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn question_claim_unpolled_or_abandoned_acquisition_never_orphans_or_replies() {
    let mut fixture = NodeHostFixture::start("claim-normal").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let mux = client.connect_mux().await.unwrap();
    drop(acquire(&mux));
    assert_eq!(stats(&client).await["opens"], 0);
    let mut opening = Box::pin(acquire(&mux));
    assert!(futures_util::poll!(&mut opening).is_pending());
    drop(opening);
    let mut sibling = mux.workspace_follow().await.unwrap();
    assert!(matches!(
        sibling.next().await,
        Some(Ok(WorkspaceFollowFrame::Baseline { .. }))
    ));
    let state = stats(&client).await;
    assert_eq!(state["active"], 0);
    assert_eq!(state["opens"], state["cancels"]);
    assert_eq!(state["replyRequests"], 0);
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn question_claim_stream_limit_does_not_release_or_reacquire_other_owner() {
    let mut fixture = NodeHostFixture::start("claim-normal").await;
    let client = fixture
        .connect(Limits {
            max_streams: 1,
            ..Limits::default()
        })
        .await
        .unwrap();
    let mux = client.connect_mux().await.unwrap();
    let claim = acquire(&mux).await.unwrap();
    assert_eq!(acquire(&mux).await.err(), Some(Error::StreamLimit));
    let state = stats(&client).await;
    assert_eq!(state["opens"], 1);
    assert_eq!(state["active"], 1);
    assert_eq!(state["cancels"], 0);
    claim.release().await;
    until(&client, "cancels", 1).await;
    let mut sibling = mux.workspace_follow().await.unwrap();
    assert!(matches!(
        sibling.next().await,
        Some(Ok(WorkspaceFollowFrame::Baseline { .. }))
    ));
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn question_claim_acquisition_budget_is_not_a_lease_ttl() {
    let mut fixture = NodeHostFixture::start("claim-zero").await;
    let client = fixture
        .connect(Limits {
            timeout: Duration::from_millis(80),
            ..Limits::default()
        })
        .await
        .unwrap();
    let mux = client.connect_mux().await.unwrap();
    let mut claim = acquire(&mux).await.unwrap();
    tokio::time::sleep(Duration::from_millis(120)).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(20), claim.wait_closed())
            .await
            .is_err()
    );
    assert_eq!(stats(&client).await["active"], 1);
    end_fixture_claims(&client).await;
    assert_eq!(claim.wait_closed().await, Ok(()));
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn question_claim_wrong_explicit_agent_or_call_cannot_acquire_fixture_authority() {
    let mut fixture = NodeHostFixture::start("claim-normal").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let mux = client.connect_mux().await.unwrap();
    for (agent, call) in [
        (AgentId::new("not-the-live-agent").unwrap(), authority().1),
        (authority().0, ToolCallId::new("not-this-call").unwrap()),
    ] {
        assert_eq!(
            mux.hold_question(&agent, &call).await.err(),
            Some(Error::RemoteFailure)
        );
    }
    assert_eq!(stats(&client).await["opens"], 0);
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}
