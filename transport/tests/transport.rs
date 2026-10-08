use dsh_native_transport::{Error, Limits, NativeClient, SecretLaunchUrl, dto::*};
#[path = "support/event_delivery.rs"]
mod event_delivery;
#[path = "support/question_claim.rs"]
mod question_claim;

#[tokio::test]
async fn plugin_projection_and_revisioned_scalar_write_use_exact_private_wire() {
    use dsh_native_transport::plugin::*;
    let mut fixture = NodeHostFixture::start("normal").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let inventory = client.plugin_inventory().await.unwrap();
    assert_eq!(inventory.entries[0].entry_id, "include:probe");
    assert!(!inventory.management_available);
    assert!(!format!("{inventory:?}").contains("PRIVATE"));
    let settings = client.plugin_settings().await.unwrap();
    let namespace = &settings.namespaces[0];
    assert_eq!(namespace.fields.len(), 1);
    let field = &namespace.fields[0];
    assert_eq!(field.path, ["count"]);
    assert!(!format!("{settings:?}").contains("PRIVATE"));
    let changed = client
        .set_plugin_field(namespace, field, SettingsScalar::Number(5.0))
        .await
        .unwrap();
    assert_eq!(changed.revision, 4);
    assert_eq!(changed.fields[0].value, Some(SettingsScalar::Number(5.0)));
    // A stale review cannot become another write, even if a caller repeats the action.
    assert_eq!(
        client
            .set_plugin_field(namespace, field, SettingsScalar::Number(5.0))
            .await,
        Err(Error::SettingsConflict)
    );
    // Constructing a public DTO cannot grant access to a private unknown/secret field.
    let mut forged = changed.clone();
    let extra = SettingsField {
        path: vec!["hiddenValue".into()],
        label: "hiddenValue".into(),
        kind: SettingsFieldKind::Bool,
        value: Some(SettingsScalar::Bool(false)),
        overridden: false,
    };
    forged.fields.push(extra.clone());
    assert_eq!(
        client
            .set_plugin_field(&forged, &extra, SettingsScalar::Bool(true))
            .await,
        Err(Error::SettingsRejected)
    );
    client.close().await;
    assert_eq!(client.plugin_inventory().await, Err(Error::Closed));
    fixture.stop().await;
}

#[tokio::test]
async fn plugin_errors_never_echo_host_messages_or_config() {
    for mode in ["failure", "wrong-rpc", "plugin-readonly"] {
        let mut fixture = NodeHostFixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await.unwrap();
        if mode == "plugin-readonly" {
            let s = client.plugin_settings().await.unwrap();
            let n = &s.namespaces[0];
            assert_eq!(
                client
                    .set_plugin_field(
                        n,
                        &n.fields[0],
                        dsh_native_transport::plugin::SettingsScalar::Number(5.0)
                    )
                    .await,
                Err(Error::SettingsRejected)
            );
        } else {
            let e = client.plugin_settings().await.unwrap_err();
            assert_eq!(
                e,
                if mode == "failure" {
                    Error::RemoteFailure
                } else {
                    Error::Correlation
                }
            );
            assert!(!format!("{e:?} {e}").contains("SECRET"));
        }
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn plugin_mutation_classifies_only_fixed_refusal_codes_and_keeps_postcommit_unknown() {
    use dsh_native_transport::plugin::SettingsScalar;
    for (mode, expected) in [
        ("plugin-conflict", Error::SettingsConflict),
        ("plugin-rejected", Error::SettingsRejected),
        ("plugin-postcommit", Error::RemoteFailure),
    ] {
        let mut fixture = NodeHostFixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await.unwrap();
        let s = client.plugin_settings().await.unwrap();
        let n = &s.namespaces[0];
        let e = client
            .set_plugin_field(n, &n.fields[0], SettingsScalar::Number(5.0))
            .await
            .unwrap_err();
        assert_eq!(e, expected);
        assert!(!format!("{e:?} {e}").contains("PRIVATE"));
        // This fixture commits before reporting gateway/internal; no automatic retry occurs.
        if mode == "plugin-postcommit" {
            assert_eq!(
                client.plugin_settings().await.unwrap().namespaces[0].revision,
                4
            );
        }
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn plugin_missing_changed_or_custom_page_values_never_send_mutation_posts() {
    use dsh_native_transport::plugin::*;
    for mode in [
        "plugin-missing",
        "plugin-changed-value",
        "plugin-custom-page",
    ] {
        let mut fixture = NodeHostFixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await.unwrap();
        let mut n = client.plugin_settings().await.unwrap().namespaces.remove(0);
        if mode == "plugin-missing" {
            assert_eq!(n.fields[0].value, None);
            assert_eq!(
                client
                    .set_plugin_field(&n, &n.fields[0], SettingsScalar::Number(5.0))
                    .await,
                Err(Error::InvalidDto)
            );
            n.fields[0].value = Some(SettingsScalar::Number(4.0));
        }
        if mode == "plugin-custom-page" {
            assert!(!n.auto_generate);
            assert!(n.fields.is_empty());
            n.auto_generate = true;
            n.fields.push(SettingsField {
                path: vec!["count".into()],
                label: "count".into(),
                kind: SettingsFieldKind::Number {
                    min: Some(1.0),
                    max: Some(8.0),
                    integer: true,
                },
                value: Some(SettingsScalar::Number(4.0)),
                overridden: true,
            });
        }
        assert_eq!(
            client
                .set_plugin_field(&n, &n.fields[0], SettingsScalar::Number(5.0))
                .await,
            Err(Error::SettingsRejected)
        );
        // Fixture counts every mutation POST before payload checking, including malformed writes.
        assert_eq!(
            client.plugin_inventory().await.unwrap().entries[0].module_name,
            "probe-mutations-0"
        );
        client.close().await;
        fixture.stop().await;
    }
}

fn registry_request() -> SessionRegistryRequest {
    SessionRegistryRequest {
        session_id: SessionId::new("session / registry opaque:%").unwrap(),
    }
}

async fn registry_results(
    client: &NativeClient,
    request: &SessionRegistryRequest,
) -> [Result<Vec<SessionId>, Error>; 4] {
    [
        client
            .pin_session(request)
            .await
            .map(|v| v.pinned_session_ids),
        client
            .unpin_session(request)
            .await
            .map(|v| v.pinned_session_ids),
        client
            .archive_session(request)
            .await
            .map(|v| v.archived_session_ids),
        client
            .unarchive_session(request)
            .await
            .map(|v| v.archived_session_ids),
    ]
}

#[test]
fn registry_request_is_object_only_and_has_no_stop_or_agent_authority() {
    use serde_json::{Value, json};
    let request = registry_request();
    let wire = json!({"sessionId":"session / registry opaque:%"});
    assert_eq!(serde_json::to_value(&request).unwrap(), wire);
    assert_eq!(
        serde_json::from_value::<SessionRegistryRequest>(wire.clone()).unwrap(),
        request
    );
    for field in ["stopActivity", "agent", "signal", "workspaceId"] {
        let mut extra = wire.clone();
        extra[field] = json!(false);
        assert!(serde_json::from_value::<SessionRegistryRequest>(extra).is_err());
    }
    for malformed in [
        Value::Null,
        json!({}),
        json!(["session / registry opaque:%"]),
        json!({"session_id":"session / registry opaque:%"}),
        json!({"sessionId":""}),
        json!({"sessionId":[]}),
        json!({"sessionId":null}),
    ] {
        assert!(serde_json::from_value::<SessionRegistryRequest>(malformed).is_err());
    }
    assert!(
        serde_json::from_str::<SessionRegistryRequest>(
            r#"{"sessionId":"first","sessionId":"second"}"#
        )
        .is_err()
    );
}

#[test]
fn registry_receipts_require_objects_and_preserve_complete_ordered_vectors() {
    use serde_json::json;
    let ids = json!([
        "session-z-global",
        "session-a-global",
        "session / registry opaque:%"
    ]);
    let pin = json!({"pinnedSessionIds":ids});
    let archive = json!({"archivedSessionIds":ids});
    let pinned: PinnedSessionsValue = serde_json::from_value(pin.clone()).unwrap();
    let archived: ArchivedSessionsValue = serde_json::from_value(archive.clone()).unwrap();
    assert_eq!(pinned.pinned_session_ids, archived.archived_session_ids);
    assert_eq!(pinned.pinned_session_ids[0].as_str(), "session-z-global");
    assert_eq!(pinned.pinned_session_ids[1].as_str(), "session-a-global");
    assert_eq!(serde_json::to_value(pinned).unwrap(), pin);
    assert_eq!(serde_json::to_value(archived).unwrap(), archive);
    assert!(serde_json::from_value::<PinnedSessionsValue>(json!({"pinnedSessionIds":[]})).is_ok());
    assert!(
        serde_json::from_value::<ArchivedSessionsValue>(json!({"archivedSessionIds":[]})).is_ok()
    );
    for malformed in [
        json!([ids]),
        json!({}),
        json!({"pinnedSessionIds":[""]}),
        json!({"pinnedSessionIds":null}),
        json!({"pinnedSessionIds":ids,"agent":{}}),
    ] {
        assert!(serde_json::from_value::<PinnedSessionsValue>(malformed).is_err());
    }
    for malformed in [
        json!([ids]),
        json!({}),
        json!({"archivedSessionIds":[""]}),
        json!({"archivedSessionIds":null}),
        json!({"archivedSessionIds":ids,"signal":{}}),
    ] {
        assert!(serde_json::from_value::<ArchivedSessionsValue>(malformed).is_err());
    }
}

#[tokio::test]
async fn registry_four_methods_use_exact_authenticated_wire_and_global_ordered_receipts() {
    let mut fixture = NodeHostFixture::start("normal").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let results = registry_results(&client, &registry_request()).await;
    for (index, result) in results.into_iter().enumerate() {
        let expected = if index % 2 == 0 {
            vec![
                "session / registry opaque:%",
                "session-z-global",
                "session-a-global",
            ]
        } else {
            vec!["session-z-global", "session-a-global"]
        };
        assert_eq!(
            result
                .unwrap()
                .iter()
                .map(SessionId::as_str)
                .collect::<Vec<_>>(),
            expected
        );
    }
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn registry_remote_refusal_and_correlation_errors_remain_redacted() {
    for (mode, expected) in [
        ("failure", Error::RemoteFailure),
        ("wrong-rpc", Error::Correlation),
    ] {
        let mut fixture = NodeHostFixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await.unwrap();
        for result in registry_results(&client, &registry_request()).await {
            let error = result.unwrap_err();
            assert_eq!(error, expected);
            assert!(!format!("{error:?} {error}").contains("SECRET_SENTINEL"));
        }
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn registry_invalid_response_shapes_and_complete_body_bounds_fail_closed() {
    for mode in [
        "registry-array",
        "registry-missing",
        "registry-empty-id",
        "registry-wrong-list",
        "registry-extra",
    ] {
        let mut fixture = NodeHostFixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await.unwrap();
        for result in registry_results(&client, &registry_request()).await {
            assert_eq!(result.unwrap_err(), Error::InvalidDto);
        }
        client.close().await;
        fixture.stop().await;
    }
    let mut fixture = NodeHostFixture::start("normal").await;
    let client = fixture
        .connect(Limits {
            max_result_bytes: 96,
            ..Default::default()
        })
        .await
        .unwrap();
    for result in registry_results(&client, &registry_request()).await {
        assert_eq!(result.unwrap_err(), Error::Oversize);
    }
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn registry_mutation_timeouts_and_retired_clones_never_retry() {
    let mut fixture = NodeHostFixture::start("slow").await;
    let client = fixture
        .connect(Limits {
            timeout: Duration::from_millis(100),
            ..Default::default()
        })
        .await
        .unwrap();
    for result in registry_results(&client, &registry_request()).await {
        assert_eq!(result.unwrap_err(), Error::Timeout);
    }
    let copy = client.clone();
    client.invalidate();
    for result in registry_results(&copy, &registry_request()).await {
        assert_eq!(result.unwrap_err(), Error::Closed);
    }
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn registry_inflight_requests_cancel_on_client_close() {
    let mut fixture = NodeHostFixture::start("slow").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let copy = client.clone();
    let task = tokio::spawn(async move {
        let request = registry_request();
        tokio::join!(
            copy.pin_session(&request),
            copy.unpin_session(&request),
            copy.archive_session(&request),
            copy.unarchive_session(&request)
        )
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    tokio::time::timeout(Duration::from_secs(1), client.close())
        .await
        .unwrap();
    let (pin, unpin, archive, unarchive) = task.await.unwrap();
    assert_eq!(pin.unwrap_err(), Error::Closed);
    assert_eq!(unpin.unwrap_err(), Error::Closed);
    assert_eq!(archive.unwrap_err(), Error::Closed);
    assert_eq!(unarchive.unwrap_err(), Error::Closed);
    fixture.stop().await;
}

#[tokio::test]
async fn self_reply_ack_retires_only_our_delivery_without_host_cancel() {
    let mut fixture = NodeHostFixture::start("self-reply-series").await;
    let client = fixture
        .connect(Limits {
            max_pending_events: 2,
            ..Default::default()
        })
        .await
        .unwrap();
    let mux = client.connect_mux().await.unwrap();
    let mut events = mux.events().await.unwrap();
    assert!(matches!(
        events.next().await,
        Some(Ok(RemoteEventFrame::Ready { .. }))
    ));
    for _ in 0..70 {
        let Some(Ok(RemoteEventFrame::Waterfall { event_id, .. })) = events.next().await else {
            panic!("self-reply event capacity exhausted")
        };
        client
            .reply_approval(&event_id, ApprovalOutcome::AllowedOnce)
            .await
            .unwrap();
        assert!(matches!(
            client
                .reply_approval(&event_id, ApprovalOutcome::AllowedOnce)
                .await,
            Err(Error::Correlation)
        ));
    }
    mux.close().await;
    client.close().await;
    fixture.stop().await;

    let mut fixture = NodeHostFixture::start("reply-failure").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let mux = client.connect_mux().await.unwrap();
    let mut events = mux.events().await.unwrap();
    let _ = events.next().await;
    let Some(Ok(RemoteEventFrame::Waterfall { event_id, .. })) = events.next().await else {
        panic!("failed reply fixture missing")
    };
    for _ in 0..2 {
        assert!(matches!(
            client
                .reply_approval(&event_id, ApprovalOutcome::Rejected)
                .await,
            Err(Error::HttpStatus(503))
        ));
    }
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn forwarded_waterfalls_reject_arrays_and_reserved_authority_fields() {
    for mode in ["event-array", "event-agent", "event-signal"] {
        let mut fixture = NodeHostFixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await.unwrap();
        let mux = client.connect_mux().await.unwrap();
        let mut events = mux.events().await.unwrap();
        // The actor may invalidate even before the queued Ready is consumed.
        let mut rejected = false;
        while let Some(frame) = events.next().await {
            if frame.is_err() {
                rejected = true;
                break;
            }
            assert!(matches!(frame, Ok(RemoteEventFrame::Ready { .. })));
        }
        assert!(rejected);
        assert!(matches!(
            client
                .reply_approval(
                    &RemoteEventId::new("fixture-event").unwrap(),
                    ApprovalOutcome::AllowedOnce
                )
                .await,
            Err(Error::Correlation) | Err(Error::Closed)
        ));
        mux.close().await;
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn dropped_queued_open_does_not_poison_other_streams() {
    use std::{
        future::Future,
        task::{Context, Poll},
    };
    let mut fixture = NodeHostFixture::start("normal").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let mux = client.connect_mux().await.unwrap();
    // Current-thread executor: enqueue the open without letting the socket actor run.
    let mut opening = Box::pin(mux.workspace_follow());
    let mut context = Context::from_waker(futures_util::task::noop_waker_ref());
    assert!(matches!(opening.as_mut().poll(&mut context), Poll::Pending));
    drop(opening);
    let mut control = mux.control().await.unwrap();
    assert!(matches!(
        control.next().await,
        Some(Ok(ControlFrame::Baseline { .. }))
    ));
    control.cancel().await;
    let mut workspace = mux.workspace_follow().await.unwrap();
    assert!(matches!(
        workspace.next().await,
        Some(Ok(WorkspaceFollowFrame::Baseline { .. }))
    ));
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn retaining_cancelled_stream_releases_shared_queue_budget() {
    let mut fixture = NodeHostFixture::start("queued-budget").await;
    let client = fixture
        .connect(Limits {
            queue_bytes: 1024,
            ..Default::default()
        })
        .await
        .unwrap();
    let mux = client.connect_mux().await.unwrap();
    let mut old = mux.workspace_follow().await.unwrap();
    assert!(matches!(
        old.next().await,
        Some(Ok(WorkspaceFollowFrame::Baseline { .. }))
    ));
    tokio::time::sleep(Duration::from_millis(30)).await;
    old.cancel().await; // Keep this object alive while opening the replacement.
    assert!(old.next().await.is_none());
    let mut new = mux.workspace_follow().await.unwrap();
    assert!(matches!(
        new.next().await,
        Some(Ok(WorkspaceFollowFrame::Baseline { .. }))
    ));
    assert!(matches!(
        new.next().await,
        Some(Ok(WorkspaceFollowFrame::Upsert { .. }))
    ));
    assert!(old.next().await.is_none());
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn fixed_question_cancel_requires_question_authority_and_exact_wire() {
    let mut fixture = NodeHostFixture::start("question-events").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let mux = client.connect_mux().await.unwrap();
    let mut events = mux.events().await.unwrap();
    assert!(matches!(
        events.next().await,
        Some(Ok(RemoteEventFrame::Ready { .. }))
    ));
    let Some(Ok(RemoteEventFrame::Waterfall {
        event,
        event_id,
        agent_id,
        ..
    })) = events.next().await
    else {
        panic!("question fixture waterfall missing")
    };
    assert_eq!(event, WaterfallKind::Question);
    assert_eq!(agent_id, AgentId::new("agent-fixture").unwrap());
    assert!(matches!(
        client
            .reply_approval(&event_id, ApprovalOutcome::AllowedOnce)
            .await,
        Err(Error::Correlation)
    ));
    assert!(matches!(
        client
            .cancel_question(&RemoteEventId::new("unreceived").unwrap())
            .await,
        Err(Error::Correlation)
    ));
    client.cancel_question(&event_id).await.unwrap();
    mux.close().await;
    client.close().await;
    fixture.stop().await;

    let mut fixture = NodeHostFixture::start("normal").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let mux = client.connect_mux().await.unwrap();
    let mut events = mux.events().await.unwrap();
    let _ = events.next().await;
    let Some(Ok(RemoteEventFrame::Waterfall { event_id, .. })) = events.next().await else {
        panic!("approval fixture missing")
    };
    assert!(matches!(
        client.cancel_question(&event_id).await,
        Err(Error::Correlation)
    ));
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn page_and_follow_coordinates_reject_unsafe_javascript_integers() {
    const MAX: u64 = 9_007_199_254_740_991;
    let mut fixture = NodeHostFixture::start("normal").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let base = SessionPageRequest {
        address: SessionAddress::Session {
            session_id: SessionId::new("session-new").unwrap(),
        },
        through_seq: -1,
        before_seq: None,
        max_messages: None,
        turn_window: None,
    };
    for request in [
        SessionPageRequest {
            through_seq: -2,
            ..base.clone()
        },
        SessionPageRequest {
            through_seq: MAX as i64 + 1,
            ..base.clone()
        },
        SessionPageRequest {
            before_seq: Some(MAX + 1),
            ..base.clone()
        },
        SessionPageRequest {
            max_messages: Some(MAX + 1),
            ..base.clone()
        },
        SessionPageRequest {
            max_messages: Some(0),
            ..base.clone()
        },
        SessionPageRequest {
            turn_window: Some(TurnWindow {
                min_messages: 1,
                min_turns: MAX + 1,
            }),
            ..base.clone()
        },
        SessionPageRequest {
            turn_window: Some(TurnWindow {
                min_messages: 51,
                min_turns: 1,
            }),
            ..base.clone()
        },
    ] {
        assert!(matches!(
            client.session_page(request).await,
            Err(Error::InvalidDto)
        ));
    }
    let page = client
        .session_page(SessionPageRequest {
            through_seq: MAX as i64,
            before_seq: Some(MAX),
            max_messages: Some(MAX),
            turn_window: Some(TurnWindow {
                min_messages: MAX,
                min_turns: MAX,
            }),
            ..base.clone()
        })
        .await
        .unwrap();
    assert!(!page.has_more);
    assert!(!client.session_page(base.clone()).await.unwrap().has_more);
    let mux = client.connect_mux().await.unwrap();
    assert!(matches!(
        mux.session_follow(SessionFollowRequest {
            address: base.address,
            max_messages: None,
            turn_window: Some(TurnWindow {
                min_messages: 1,
                min_turns: MAX + 1
            }),
            assistant_stream: Some(true)
        })
        .await,
        Err(Error::InvalidDto)
    ));
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn http_secure_cookie_is_not_native_authority() {
    let mut fixture = NodeHostFixture::start("secure-cookie").await;
    assert!(matches!(
        fixture.connect(Limits::default()).await,
        Err(Error::CookieScope)
    ));
    fixture.stop().await;
}

#[tokio::test]
async fn close_quiesces_in_progress_websocket_handshake() {
    let mut fixture = NodeHostFixture::start("slow-upgrade").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let copy = client.clone();
    let task = tokio::spawn(async move { copy.connect_mux().await });
    // A bounded fixture settling delay lets the local server retain its upgrade socket.
    tokio::time::sleep(Duration::from_millis(30)).await;
    tokio::time::timeout(Duration::from_secs(1), client.close())
        .await
        .unwrap();
    assert!(matches!(task.await.unwrap(), Err(Error::Closed)));
    assert!(matches!(client.connect_mux().await, Err(Error::Closed)));
    fixture.stop().await;
}

#[tokio::test]
async fn waterfall_pending_registry_is_bounded() {
    let mut fixture = NodeHostFixture::start("event-flood").await;
    let client = fixture
        .connect(Limits {
            max_pending_events: 2,
            ..Default::default()
        })
        .await
        .unwrap();
    let mux = client.connect_mux().await.unwrap();
    let mut events = mux.events().await.unwrap();
    let failure = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            match events.next().await {
                Some(Err(error)) => break error,
                None => panic!("flood closed without diagnosed failure"),
                Some(Ok(_)) => {}
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(failure, Error::QueueFull);
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn invalidate_revokes_every_clone_and_existing_stream() {
    let mut fixture = NodeHostFixture::start("normal").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let copy = client.clone();
    let mux = client.connect_mux().await.unwrap();
    let mut stream = mux.workspace_follow().await.unwrap();
    assert!(matches!(
        stream.next().await,
        Some(Ok(WorkspaceFollowFrame::Baseline { .. }))
    ));
    client.invalidate();
    assert!(matches!(copy.session_list().await, Err(Error::Closed)));
    assert!(matches!(stream.next().await, Some(Err(Error::Closed))));
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}

use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
};

/// Plain Node loopback fixture with private ready pipe. Never runs the real Harness.
struct NodeHostFixture {
    child: Child,
    launch: Option<SecretLaunchUrl>,
}
impl NodeHostFixture {
    async fn start(mode: &str) -> Self {
        let file = format!("{}/tests/node_host_fixture.mjs", env!("CARGO_MANIFEST_DIR"));
        let mut child = Command::new("/usr/bin/node")
            .env_clear()
            .arg(file)
            .arg(mode)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .expect("start local fixture");
        let mut lines = BufReader::new(child.stdout.take().expect("private stdout")).lines();
        let url = tokio::time::timeout(Duration::from_secs(5), lines.next_line())
            .await
            .expect("fixture readiness timeout")
            .expect("private readiness line")
            .expect("fixture readiness");
        let launch =
            SecretLaunchUrl::new(url).unwrap_or_else(|_| panic!("fixture readiness rejected"));
        Self {
            child,
            launch: Some(launch),
        }
    }
    async fn connect(&mut self, limits: Limits) -> dsh_native_transport::Result<NativeClient> {
        NativeClient::connect(self.launch.take().expect("one exchange"), limits).await
    }
    async fn stop(mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}
fn follow() -> SessionFollowRequest {
    SessionFollowRequest {
        address: SessionAddress::Session {
            session_id: SessionId::new("session / opaque:%").unwrap(),
        },
        max_messages: Some(50),
        turn_window: None,
        assistant_stream: None,
    }
}

#[test]
fn launch_rejects_noncanonical_and_unsafe_targets() {
    for url in [
        "http://localhost:80/?token=x",
        "http://127.0.0.2:8000/?token=x",
        "http://127.0.0.1:8000/?token=x&token=y",
        "http://127.0.0.1:8000/?token=",
        "http://127.0.0.1:8000/?token=x#f",
        "http://user@127.0.0.1:8000/?token=x",
        "https://127.0.0.1:8000/?token=x",
        "http://2130706433:8000/?token=x",
        "http://127.0.0.1:8000/other?token=x",
    ] {
        assert!(matches!(
            SecretLaunchUrl::new(url.into()),
            Err(Error::InvalidLaunch)
        ));
    }
    assert!(SecretLaunchUrl::new("http://127.0.0.1:8000/?token=x".into()).is_ok());
}
#[test]
fn opaque_ids_and_exact_dto_names() {
    let id = SessionId::new("session / opaque:%").unwrap();
    assert_eq!(
        serde_json::to_value(&id).unwrap(),
        serde_json::json!("session / opaque:%")
    );
    let request = follow();
    let json = serde_json::to_value(request).unwrap();
    assert_eq!(json["address"]["sessionId"], "session / opaque:%");
    assert!(json.get("cursor").is_none());
    assert!(json.get("assistantStream").is_none());
    let missing = serde_json::json!({"sessionId":"s","updatedAt":0,"running":false,"blank":true});
    assert!(serde_json::from_value::<SessionSummary>(missing).is_err());
    for error in [
        Error::RemoteFailure,
        Error::InvalidJson,
        Error::CookieScope,
        Error::Network,
        Error::HttpStatus(401),
    ] {
        assert!(!format!("{error:?} {error}").contains("SECRET_SENTINEL"));
    }
}
#[tokio::test]
async fn cookie_exchange_and_all_unary_wire_dtos() {
    let mut fixture = NodeHostFixture::start("normal").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let roster = client.session_list().await.unwrap();
    assert!(!roster.items[0].agent_available);
    let session_id = roster.items[0].session_id.clone();
    assert_eq!(session_id.as_str(), "session / opaque:%");
    let created = client
        .session_create(SessionCreateRequest {
            session_id: Some(session_id.clone()),
            agent_preset: Some("standard".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(created.session_id, session_id);
    assert!(
        client
            .session_prompt(SessionPromptRequest {
                session_id: session_id.clone(),
                request_id: SessionRequestId::new("p1").unwrap(),
                mode: PromptMode::Queue,
                content: vec![PromptContentPart::Text {
                    text: "fixture-only".into()
                }],
                client_time_zone: Some("UTC".into())
            })
            .await
            .unwrap()
            .accepted
    );
    assert!(client.session_cancel(&session_id).await.unwrap().accepted);
    assert!(client.model_catalog().await.unwrap().groups.is_empty());
    assert_eq!(
        client
            .select_model(SelectModelRequest {
                session_id: session_id.clone(),
                selection: ModelSelection {
                    provider: "example".into(),
                    model: "model".into(),
                    reasoning_effort: None
                }
            })
            .await
            .unwrap()
            .selected
            .model,
        "model"
    );
    assert!(
        client
            .session_page(SessionPageRequest {
                address: follow().address,
                through_seq: -1,
                before_seq: None,
                max_messages: Some(50),
                turn_window: None
            })
            .await
            .unwrap()
            .records
            .is_empty()
    );
    assert!(
        client
            .session_projections(&session_id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        client.account_state().await.unwrap().status,
        AccountStatus::SignedOut
    ));
    assert!(
        client
            .answer_continued(
                &AgentId::new("agent-fixture").unwrap(),
                &ToolCallId::new("call-1").unwrap(),
                QuestionAnswer { answers: vec![] }
            )
            .await
            .unwrap()
    );
    client.close().await;
    assert!(matches!(client.session_list().await, Err(Error::Closed)));
    fixture.stop().await;
}
#[tokio::test]
async fn cookie_scope_and_redirects_rejected() {
    for mode in [
        "bad-name",
        "bad-scope",
        "no-http-only",
        "no-samesite",
        "domain-cookie",
        "bad-location",
        "redirect-start",
    ] {
        let mut fixture = NodeHostFixture::start(mode).await;
        let error = match fixture.connect(Limits::default()).await {
            Ok(client) => {
                client.close().await;
                panic!("invalid cookie admitted");
            }
            Err(error) => error,
        };
        assert!(matches!(error, Error::CookieScope | Error::Authentication));
        fixture.stop().await;
    }
}
#[tokio::test]
async fn invalid_http_and_complete_body_limits() {
    for (mode, expected) in [
        ("wrong-rpc", Error::Correlation),
        ("redirect-rpc", Error::HttpStatus(307)),
        ("missing-available", Error::InvalidDto),
        ("bad-media", Error::InvalidFrame),
        ("bad-json", Error::InvalidJson),
        ("failure", Error::RemoteFailure),
        ("oversize-length", Error::Oversize),
        ("oversize-chunked", Error::Oversize),
    ] {
        let mut fixture = NodeHostFixture::start(mode).await;
        let client = fixture
            .connect(Limits {
                max_result_bytes: 4096,
                ..Default::default()
            })
            .await
            .unwrap();
        let actual = match client.session_list().await {
            Ok(_) => panic!("invalid fixture result accepted"),
            Err(error) => error,
        };
        assert_eq!(actual, expected);
        assert!(!actual.to_string().contains("SECRET_SENTINEL"));
        client.close().await;
        fixture.stop().await;
    }
}
#[tokio::test]
async fn typed_streams_events_reply_void_and_logical_cancel() {
    let mut fixture = NodeHostFixture::start("normal").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let mux = client.connect_mux().await.unwrap();
    assert!(matches!(
        client.connect_mux().await,
        Err(Error::StreamLimit)
    ));
    let mut workspace = mux.workspace_follow().await.unwrap();
    assert!(matches!(
        workspace.next().await.unwrap().unwrap(),
        WorkspaceFollowFrame::Baseline { .. }
    ));
    let mut history = mux.session_follow(follow()).await.unwrap();
    assert!(matches!(
        history.next().await.unwrap().unwrap(),
        SessionFollowFrame::Snapshot { cursor: -1, .. }
    ));
    assert!(matches!(
        history.next().await.unwrap().unwrap(),
        SessionFollowFrame::Event { .. }
    ));
    let mut control = mux.control().await.unwrap();
    assert!(matches!(
        control.next().await.unwrap().unwrap(),
        ControlFrame::Baseline { .. }
    ));
    let mut events = mux.events().await.unwrap();
    assert!(matches!(
        events.next().await.unwrap().unwrap(),
        RemoteEventFrame::Ready { .. }
    ));
    let id = match events.next().await.unwrap().unwrap() {
        RemoteEventFrame::Waterfall { event_id, .. } => event_id,
        _ => panic!("waterfall expected"),
    };
    // Actual void result has no value field; () decoding must accept omission.
    client
        .reply_approval(&id, ApprovalOutcome::Rejected)
        .await
        .unwrap();
    assert!(matches!(
        client
            .reply_approval(
                &RemoteEventId::new("unreceived").unwrap(),
                ApprovalOutcome::AllowedOnce
            )
            .await,
        Err(Error::Correlation)
    ));
    let mut claim = mux
        .question_wait(&AgentId::new("s").unwrap(), &ToolCallId::new("c").unwrap())
        .await
        .unwrap();
    assert_eq!(claim.next().await.unwrap().unwrap().remaining_ms, 1234);
    history.cancel().await;
    assert!(history.next().await.is_none());
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}
#[tokio::test]
async fn invalid_ws_frames_and_sequence_rebaseline_failure() {
    for (mode, expected) in [
        ("bad-frame", Error::InvalidFrame),
        ("binary", Error::InvalidFrame),
        ("wrong-stream", Error::Correlation),
        ("ws-oversize", Error::Oversize),
        ("remote-stream-error", Error::RemoteFailure),
        ("sequence-gap", Error::Sequence),
    ] {
        let mut fixture = NodeHostFixture::start(mode).await;
        let client = fixture
            .connect(Limits {
                max_result_bytes: 4096,
                ..Default::default()
            })
            .await
            .unwrap();
        let mux = client.connect_mux().await.unwrap();
        let mut stream = mux.session_follow(follow()).await.unwrap();
        let mut error = None;
        for _ in 0..3 {
            match tokio::time::timeout(Duration::from_secs(3), stream.next())
                .await
                .unwrap()
            {
                Some(Err(found)) => {
                    error = Some(found);
                    break;
                }
                None => break,
                _ => {}
            }
        }
        assert_eq!(error, Some(expected));
        mux.close().await;
        client.close().await;
        fixture.stop().await;
    }
}
#[tokio::test]
async fn ping_is_answered_automatically() {
    let mut fixture = NodeHostFixture::start("ping").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let mux = client.connect_mux().await.unwrap();
    let mut workspace = mux.workspace_follow().await.unwrap();
    workspace.next().await.unwrap().unwrap();
    let update = tokio::time::timeout(Duration::from_secs(3), workspace.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(matches!(update, WorkspaceFollowFrame::Upsert { .. }));
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}
#[tokio::test]
async fn queue_bound_and_shutdown_quiescence() {
    let mut fixture = NodeHostFixture::start("normal").await;
    let client = fixture
        .connect(Limits {
            queue_bytes: 16,
            ..Default::default()
        })
        .await
        .unwrap();
    let mux = client.connect_mux().await.unwrap();
    let mut stream = mux.workspace_follow().await.unwrap();
    assert!(matches!(stream.next().await, Some(Err(Error::QueueFull))));
    mux.close().await;
    client.close().await;
    fixture.stop().await;
    let mut fixture = NodeHostFixture::start("slow").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let copy = client.clone();
    let task = tokio::spawn(async move { copy.session_list().await });
    tokio::task::yield_now().await;
    tokio::time::timeout(Duration::from_secs(2), client.close())
        .await
        .unwrap();
    assert!(matches!(task.await.unwrap(), Err(Error::Closed)));
    fixture.stop().await;
}
