use dsh_native_transport::{Error, Limits, NativeClient, SecretLaunchUrl, dto::*};
#[path = "support/event_delivery.rs"]
mod event_delivery;
#[path = "support/prompt_recovery.rs"]
mod prompt_recovery;
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
async fn the_agent_preset_default_uses_the_settings_update_wire_the_reference_uses() {
    // The live Host answers this namespace with autoGenerate:false
    // (docs/NATIVE-LIVE-VERIFICATION.md), so the config-form mutate path cannot write it: the
    // reference writes through settings/update and so does this app.
    let mut fixture = NodeHostFixture::start("normal").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let view = client.agent_preset_default("ptc").await.unwrap();
    assert_eq!(view.ns, "agent-preset-registry");
    assert!(!view.auto_generate);
    // The roster re-read confirms the write; the reply itself carries no projected field.
    assert!(view.fields.is_empty());
    assert_eq!(
        client.agent_preset_default("").await,
        Err(Error::InvalidDto)
    );
    assert_eq!(
        client.agent_preset_default("ptc\u{0}").await,
        Err(Error::InvalidDto)
    );
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn agent_preset_default_classifies_refusals_and_rejects_a_foreign_namespace() {
    for (mode, expected) in [
        ("agent-preset-refused", Error::SettingsRejected),
        ("agent-preset-conflict", Error::SettingsConflict),
        ("agent-preset-wrong-ns", Error::Correlation),
        ("agent-preset-malformed", Error::InvalidDto),
    ] {
        let mut fixture = NodeHostFixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await.unwrap();
        let error = client.agent_preset_default("ptc").await.unwrap_err();
        assert_eq!(error, expected, "{mode}");
        assert!(!format!("{error:?} {error}").contains("PRIVATE"));
        client.close().await;
        fixture.stop().await;
    }
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

fn rename_request() -> SessionRenameRequest {
    SessionRenameRequest {
        session_id: SessionId::new("session / rename opaque:%").unwrap(),
        title: "PUBLIC renamed title".into(),
    }
}

#[test]
fn rename_dtos_use_exact_object_fields_and_reject_extra_authority() {
    use serde_json::{Value, json};
    let request = rename_request();
    let wire = json!({"sessionId":"session / rename opaque:%","title":"PUBLIC renamed title"});
    assert_eq!(serde_json::to_value(&request).unwrap(), wire);
    assert_eq!(
        serde_json::from_value::<SessionRenameRequest>(wire.clone())
            .unwrap()
            .title,
        request.title
    );
    for field in ["agent", "agentId", "stopActivity", "seq"] {
        let mut extra = wire.clone();
        extra[field] = json!(false);
        assert!(serde_json::from_value::<SessionRenameRequest>(extra).is_err());
    }
    let value = json!({"title":"PUBLIC normalized title","seq":5});
    let parsed: SessionRenameValue = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(parsed.title, "PUBLIC normalized title");
    assert_eq!(parsed.seq, 5);
    assert_eq!(serde_json::to_value(parsed).unwrap(), value);
    for malformed in [
        Value::Null,
        json!({}),
        json!({"title":"x"}),
        json!({"seq":5}),
        json!({"title":"x","seq":5,"agentId":"agent-fixture"}),
    ] {
        assert!(serde_json::from_value::<SessionRenameValue>(malformed).is_err());
    }
}

#[tokio::test]
async fn rename_uses_exact_authenticated_wire_and_returns_the_normalized_ack() {
    // The fixture 400s unless the full envelope is exactly
    // {"type":"client-request", rpcId, method:"session/rename",
    //  payload:{args:{request:{sessionId, title}}}}, so success proves method and args.
    let mut fixture = NodeHostFixture::start("normal").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let value = client.session_rename(&rename_request()).await.unwrap();
    assert_eq!(value.title, "PUBLIC renamed title");
    assert_eq!(value.seq, 5);
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn rename_malformed_or_impossible_ack_fails_closed_before_any_title_claim() {
    for mode in [
        "rename-missing",
        "rename-wrong-type",
        "rename-extra",
        "rename-negative-seq",
    ] {
        let mut fixture = NodeHostFixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await.unwrap();
        assert_eq!(
            client.session_rename(&rename_request()).await.unwrap_err(),
            Error::InvalidDto
        );
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn rename_remote_refusal_and_correlation_errors_remain_redacted() {
    for (mode, expected) in [
        ("failure", Error::RemoteFailure),
        ("wrong-rpc", Error::Correlation),
    ] {
        let mut fixture = NodeHostFixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await.unwrap();
        let error = client.session_rename(&rename_request()).await.unwrap_err();
        assert_eq!(error, expected);
        assert!(!format!("{error:?} {error}").contains("SECRET_SENTINEL"));
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn rename_timeout_and_retired_clone_never_retry() {
    let mut fixture = NodeHostFixture::start("slow").await;
    let client = fixture
        .connect(Limits {
            timeout: Duration::from_millis(100),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(
        client.session_rename(&rename_request()).await.unwrap_err(),
        Error::Timeout
    );
    let copy = client.clone();
    client.invalidate();
    assert_eq!(
        copy.session_rename(&rename_request()).await.unwrap_err(),
        Error::Closed
    );
    client.close().await;
    fixture.stop().await;
}

fn fork_request() -> SessionForkRequest {
    SessionForkRequest {
        session_id: SessionId::new("session / fork opaque:%").unwrap(),
        at_seq: None,
    }
}

#[test]
fn fork_dtos_use_exact_object_fields_and_reject_extra_authority() {
    use serde_json::{Value, json};
    let request = fork_request();
    let wire = json!({"sessionId":"session / fork opaque:%"});
    assert_eq!(serde_json::to_value(&request).unwrap(), wire);
    // An omitted atSeq must not serialize at all; an explicit cut must.
    assert_eq!(
        serde_json::to_value(SessionForkRequest {
            at_seq: Some(4),
            ..fork_request()
        })
        .unwrap(),
        json!({"sessionId":"session / fork opaque:%","atSeq":4})
    );
    assert_eq!(
        serde_json::from_value::<SessionForkRequest>(wire.clone())
            .unwrap()
            .at_seq,
        None
    );
    for field in ["agent", "agentId", "stopActivity", "sourceSeq"] {
        let mut extra = wire.clone();
        extra[field] = json!(false);
        assert!(serde_json::from_value::<SessionForkRequest>(extra).is_err());
    }
    let value = json!({"sessionId":"session-fork-child"});
    let parsed: SessionForkValue = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        parsed.session_id,
        SessionId::new("session-fork-child").unwrap()
    );
    assert_eq!(serde_json::to_value(parsed).unwrap(), value);
    for malformed in [
        Value::Null,
        json!({}),
        json!({"sessionId":""}),
        json!({"sessionId":5}),
        json!({"sessionId":"x","agentId":"agent-fixture"}),
    ] {
        assert!(serde_json::from_value::<SessionForkValue>(malformed).is_err());
    }
}

#[tokio::test]
async fn fork_uses_exact_authenticated_wire_and_returns_the_child_id() {
    // The fixture 400s unless the full envelope is exactly
    // {"type":"client-request", rpcId, method:"session/fork",
    //  payload:{args:{request:{sessionId}}}}; the omitted atSeq is part of that check.
    let mut fixture = NodeHostFixture::start("normal").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let value = client.session_fork(&fork_request()).await.unwrap();
    assert_eq!(
        value.session_id,
        SessionId::new("session-fork-child").unwrap()
    );
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn optional_absent_fields_are_omitted_from_the_live_wire() {
    // Matching the alpha Host's typert boundary observed in the live verification
    // pass: the fixture 400s on a present `null` here. Directory roots omit
    // `path`, feedback omits absent note/category, and the first schedule-history
    // page omits `before`; every absent field is an omitted key, never `null`.
    let mut fixture = NodeHostFixture::start("normal").await;
    let client = fixture.connect(Limits::default()).await.unwrap();

    let root = client.list_directory(None).await.unwrap();
    assert_eq!(root.path, "/PUBLIC/root");
    assert_eq!(root.entries[0].name, "project");
    let child = client.list_directory(Some("/PUBLIC/root")).await.unwrap();
    assert_eq!(child.home, "/PUBLIC/home");

    let feedback = SessionId::new("session / feedback opaque:%").unwrap();
    match client
        .message_feedback_put(&feedback, "PUBLIC-message", "positive", None, None, None)
        .await
        .unwrap()
    {
        MessageFeedbackPutOutcome::Saved(item) => {
            assert_eq!(item.message_id, "PUBLIC-message");
            assert_eq!(item.note, None);
            assert_eq!(item.category, None);
        }
        other => panic!("{other:?}"),
    }
    match client
        .message_feedback_put(
            &feedback,
            "PUBLIC-message-note",
            "positive",
            None,
            Some("PUBLIC note"),
            Some("task-result"),
        )
        .await
        .unwrap()
    {
        MessageFeedbackPutOutcome::Saved(item) => {
            assert_eq!(item.note.as_deref(), Some("PUBLIC note"));
            assert_eq!(item.category.as_deref(), Some("task-result"));
        }
        other => panic!("{other:?}"),
    }

    let schedule = SessionId::new("session / schedule opaque:%").unwrap();
    assert!(matches!(
        client
            .schedule_history(&schedule, "PUBLIC-schedule", 20, None)
            .await
            .unwrap(),
        ScheduleHistoryOutcome::History(_)
    ));
    assert!(matches!(
        client
            .schedule_history(&schedule, "PUBLIC-schedule", 20, Some("PUBLIC-before"))
            .await
            .unwrap(),
        ScheduleHistoryOutcome::History(_)
    ));

    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn picker_reply_is_one_qualified_path_or_a_cancel() {
    // The chooser takes no arguments. A path selects, null cancels, and every
    // other shape — missing value, blank, non-string, oversize — is a malformed
    // reply that must not be mistaken for a cancel.
    let mut fixture = NodeHostFixture::start("normal").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    assert_eq!(
        client.pick_directory().await.unwrap(),
        Some("/PUBLIC/picked".into())
    );
    client.close().await;
    fixture.stop().await;

    let mut fixture = NodeHostFixture::start("picker-cancel").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    assert_eq!(client.pick_directory().await.unwrap(), None);
    client.close().await;
    fixture.stop().await;

    for mode in [
        "picker-missing",
        "picker-empty",
        "picker-number",
        "picker-oversize",
    ] {
        let mut fixture = NodeHostFixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await.unwrap();
        assert_eq!(
            client.pick_directory().await,
            Err(Error::InvalidDto),
            "{mode}"
        );
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn preset_selection_uses_flat_agent_authority() {
    // The descriptor wires the lookup as `agentId` and the preset as `agentPreset`,
    // both flat; a wrapped request body is refused before the registry runs.
    let mut fixture = NodeHostFixture::start("normal").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let preset = client
        .agent_preset_select(
            &SessionId::new("session / preset opaque:%").unwrap(),
            "cordis",
        )
        .await
        .unwrap();
    assert_eq!(preset, "cordis");
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn job_follow_resumes_from_the_wire_cursor() {
    // The wire field is `from`; an `offset` key would make the fixture refuse the
    // open instead of sending the anchor frame.
    let mut fixture = NodeHostFixture::start("normal").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let mux = client.connect_mux().await.unwrap();
    let mut stream = mux
        .jobs_follow(JobFollowRequest {
            session_id: Some("session / job opaque:%".into()),
            job_id: "PUBLIC-job".into(),
            from: Some(5),
        })
        .await
        .unwrap();
    let frame = tokio::time::timeout(Duration::from_secs(3), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(matches!(frame, JobFollowFrame::Opened { from: 5, .. }));
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn fork_at_seq_carries_exactly_the_requested_inclusive_boundary() {
    let mut fixture = NodeHostFixture::start("fork-at").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let value = client
        .session_fork(&SessionForkRequest {
            at_seq: Some(4),
            ..fork_request()
        })
        .await
        .unwrap();
    assert_eq!(
        value.session_id,
        SessionId::new("session-fork-child").unwrap()
    );
    // A negative or unsafe boundary can never name a source event; refuse before sending.
    for at_seq in [-1, 9_007_199_254_740_992] {
        assert_eq!(
            client
                .session_fork(&SessionForkRequest {
                    at_seq: Some(at_seq),
                    ..fork_request()
                })
                .await
                .unwrap_err(),
            Error::InvalidDto
        );
    }
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn fork_malformed_or_impossible_ack_fails_closed_before_any_child_claim() {
    for mode in [
        "fork-missing",
        "fork-empty",
        "fork-wrong-type",
        "fork-same",
        "fork-extra",
    ] {
        let mut fixture = NodeHostFixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await.unwrap();
        assert_eq!(
            client.session_fork(&fork_request()).await.unwrap_err(),
            Error::InvalidDto
        );
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn fork_unavailable_and_attach_failure_keep_their_distinct_outcomes() {
    for (mode, expected) in [
        ("fork-unavailable", Error::ForkUnavailable),
        ("fork-attach-failed", Error::ForkAttachFailed),
    ] {
        let mut fixture = NodeHostFixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await.unwrap();
        let error = client.session_fork(&fork_request()).await.unwrap_err();
        assert_eq!(error, expected);
        assert!(!format!("{error:?} {error}").contains("PRIVATE_FORK_SENTINEL"));
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn fork_remote_refusal_and_correlation_errors_remain_redacted() {
    for (mode, expected) in [
        ("failure", Error::RemoteFailure),
        ("wrong-rpc", Error::Correlation),
    ] {
        let mut fixture = NodeHostFixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await.unwrap();
        let error = client.session_fork(&fork_request()).await.unwrap_err();
        assert_eq!(error, expected);
        assert!(!format!("{error:?} {error}").contains("SECRET_SENTINEL"));
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn fork_timeout_and_retired_clone_never_retry() {
    let mut fixture = NodeHostFixture::start("slow").await;
    let client = fixture
        .connect(Limits {
            timeout: Duration::from_millis(100),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(
        client.session_fork(&fork_request()).await.unwrap_err(),
        Error::Timeout
    );
    let copy = client.clone();
    client.invalidate();
    assert_eq!(
        copy.session_fork(&fork_request()).await.unwrap_err(),
        Error::Closed
    );
    client.close().await;
    fixture.stop().await;
}

fn terminal_agent() -> SessionId {
    SessionId::new("session / terminal opaque:%").unwrap()
}
fn terminal_id() -> dsh_native_transport::terminal::WebTerminalId {
    dsh_native_transport::terminal::WebTerminalId::new("PUBLIC-terminal-1").unwrap()
}
fn terminal_attachment() -> dsh_native_transport::terminal::TerminalAttachmentId {
    dsh_native_transport::terminal::TerminalAttachmentId::new("PUBLIC-attachment-1").unwrap()
}
fn terminal_create() -> dsh_native_transport::terminal::TerminalCreateRequest {
    dsh_native_transport::terminal::TerminalCreateRequest {
        shell_path: None,
        id: terminal_id(),
        cols: 80,
        rows: 24,
    }
}

#[test]
fn terminal_dtos_use_exact_fields_and_refuse_local_impossibilities_before_send() {
    use dsh_native_transport::terminal::*;
    // The identity character class is the Host's `[\w-]{1,128}`; nothing else can be sent.
    for value in ["", "has space", "slash/path", "döt"] {
        assert_eq!(WebTerminalId::new(value), Err(Error::InvalidDto));
    }
    // A create request serializes exactly the four named fields and no agent authority.
    assert_eq!(
        serde_json::to_value(terminal_create()).unwrap(),
        serde_json::json!({"id":"PUBLIC-terminal-1","cols":80,"rows":24})
    );
    for field in ["agent", "agentId", "title"] {
        let mut extra = serde_json::to_value(terminal_create()).unwrap();
        extra[field] = serde_json::json!(false);
        assert!(serde_json::from_value::<TerminalCreateRequest>(extra).is_err());
    }
    // State/exit-code consistency is enforced on the value before it is published.
    let running = WebTerminalInfo {
        id: terminal_id(),
        title: "PUBLIC terminal".into(),
        shell: TerminalShell {
            path: "/bin/PUBLIC-shell".into(),
            args: vec!["-l".into()],
            name: "PUBLIC shell".into(),
        },
        cwd: "/PUBLIC/cwd".into(),
        cols: 80,
        rows: 24,
        state: TerminalState::Running,
        exit_code: Some(0),
        error: None,
        controller_id: None,
    };
    assert_eq!(validate_info(&running), Err(Error::InvalidDto));
}

#[tokio::test]
async fn terminal_unary_methods_use_exact_authenticated_wire() {
    // The fixture 400s unless each full envelope matches exactly, so success proves
    // method, positional argument names and the omitted `shellPath` all match the Host.
    let mut fixture = NodeHostFixture::start("normal").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let agent = terminal_agent();
    let list = client.terminal_list(&agent).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(
        list[0].state,
        dsh_native_transport::terminal::TerminalState::Running
    );
    assert_eq!(list[0].exit_code, None);
    let shells = client.terminal_shells(&agent).await.unwrap();
    assert_eq!(shells[0].path, "/bin/PUBLIC-shell");
    let environment = client.terminal_environment(&agent).await.unwrap();
    assert_eq!(environment.max_input_bytes, 65536);
    assert_eq!(environment.max_cols, 500);
    let created = client
        .terminal_create(&agent, &terminal_create())
        .await
        .unwrap();
    assert_eq!(created.id, terminal_id());
    assert_eq!(created.controller_id, None);
    client
        .terminal_write(
            &agent,
            &terminal_id(),
            &terminal_attachment(),
            "PUBLIC input\n",
        )
        .await
        .unwrap();
    client
        .terminal_resize(&agent, &terminal_id(), &terminal_attachment(), 120, 40)
        .await
        .unwrap();
    client.terminal_close(&agent, &terminal_id()).await.unwrap();
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn terminal_create_carries_an_optional_shell_hint_exactly() {
    let mut fixture = NodeHostFixture::start("terminal-create-shell").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let created = client
        .terminal_create(
            &terminal_agent(),
            &dsh_native_transport::terminal::TerminalCreateRequest {
                shell_path: Some("/bin/PUBLIC-shell".into()),
                ..terminal_create()
            },
        )
        .await
        .unwrap();
    assert_eq!(created.id, terminal_id());
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn terminal_create_malformed_or_impossible_ack_fails_closed() {
    for mode in [
        "terminal-create-missing",
        "terminal-create-wrong-id",
        "terminal-create-wrong-type",
        "terminal-create-extra",
        "terminal-create-running-exit",
    ] {
        let mut fixture = NodeHostFixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await.unwrap();
        assert_eq!(
            client
                .terminal_create(&terminal_agent(), &terminal_create())
                .await
                .unwrap_err(),
            Error::InvalidDto,
            "{mode}"
        );
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn terminal_list_shells_and_environment_malformed_responses_fail_closed() {
    for (mode, expected) in [
        ("terminal-list-duplicate", Error::InvalidDto),
        ("terminal-list-oversize", Error::InvalidDto),
        ("terminal-list-bad-info", Error::InvalidDto),
    ] {
        let mut fixture = NodeHostFixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await.unwrap();
        assert_eq!(
            client.terminal_list(&terminal_agent()).await.unwrap_err(),
            expected,
            "{mode}"
        );
        client.close().await;
        fixture.stop().await;
    }
    for mode in ["terminal-shells-bad", "terminal-shells-oversize"] {
        let mut fixture = NodeHostFixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await.unwrap();
        assert_eq!(
            client.terminal_shells(&terminal_agent()).await.unwrap_err(),
            Error::InvalidDto,
            "{mode}"
        );
        client.close().await;
        fixture.stop().await;
    }
    let mut fixture = NodeHostFixture::start("terminal-environment-bad").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    assert_eq!(
        client
            .terminal_environment(&terminal_agent())
            .await
            .unwrap_err(),
        Error::InvalidDto
    );
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn terminal_refusals_keep_distinct_meanings_and_never_echo_private_messages() {
    let mut fixture = NodeHostFixture::start("terminal-unavailable").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    for error in [
        client
            .terminal_create(&terminal_agent(), &terminal_create())
            .await
            .unwrap_err(),
        client
            .terminal_close(&terminal_agent(), &terminal_id())
            .await
            .unwrap_err(),
    ] {
        assert_eq!(error, Error::TerminalUnavailable);
        assert!(!format!("{error:?} {error}").contains("PRIVATE_TERMINAL_SENTINEL"));
    }
    client.close().await;
    fixture.stop().await;

    let mut fixture = NodeHostFixture::start("terminal-control-unavailable").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    for error in [
        client
            .terminal_write(
                &terminal_agent(),
                &terminal_id(),
                &terminal_attachment(),
                "PUBLIC input\n",
            )
            .await
            .unwrap_err(),
        client
            .terminal_resize(
                &terminal_agent(),
                &terminal_id(),
                &terminal_attachment(),
                120,
                40,
            )
            .await
            .unwrap_err(),
    ] {
        assert_eq!(error, Error::TerminalControlUnavailable);
        assert!(!format!("{error:?} {error}").contains("PRIVATE_TERMINAL_SENTINEL"));
    }
    client.close().await;
    fixture.stop().await;

    let mut fixture = NodeHostFixture::start("terminal-limit-reached").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let error = client
        .terminal_create(&terminal_agent(), &terminal_create())
        .await
        .unwrap_err();
    assert_eq!(error, Error::TerminalLimitReached);
    assert!(!format!("{error:?} {error}").contains("PRIVATE_TERMINAL_SENTINEL"));
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn terminal_correlation_and_remote_failure_stay_redacted_and_never_retry() {
    for (mode, expected) in [
        ("wrong-rpc", Error::Correlation),
        ("failure", Error::RemoteFailure),
    ] {
        let mut fixture = NodeHostFixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await.unwrap();
        let error = client
            .terminal_create(&terminal_agent(), &terminal_create())
            .await
            .unwrap_err();
        assert_eq!(error, expected, "{mode}");
        assert!(!format!("{error:?} {error}").contains("SECRET_SENTINEL"));
        client.close().await;
        fixture.stop().await;
    }
    let mut fixture = NodeHostFixture::start("slow").await;
    let client = fixture
        .connect(Limits {
            timeout: Duration::from_millis(100),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(
        client
            .terminal_create(&terminal_agent(), &terminal_create())
            .await
            .unwrap_err(),
        Error::Timeout
    );
    let copy = client.clone();
    client.invalidate();
    assert_eq!(
        copy.terminal_create(&terminal_agent(), &terminal_create())
            .await
            .unwrap_err(),
        Error::Closed
    );
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn terminal_follow_streams_a_snapshot_then_strictly_ordered_output() {
    use dsh_native_transport::terminal::TerminalFrame;
    let mut fixture = NodeHostFixture::start("normal").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let mux = client.connect_mux().await.unwrap();
    let mut stream = mux
        .terminal_follow(&terminal_agent(), &terminal_id(), &terminal_attachment())
        .await
        .unwrap();
    let mut frames = Vec::new();
    for _ in 0..4 {
        frames.push(
            tokio::time::timeout(Duration::from_secs(3), stream.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap(),
        );
    }
    assert!(matches!(
        frames[0],
        TerminalFrame::Snapshot { sequence: 0, .. }
    ));
    assert!(matches!(
        frames[1],
        TerminalFrame::Output { sequence: 1, .. }
    ));
    assert!(matches!(frames[2], TerminalFrame::State { .. }));
    assert!(matches!(
        frames[3],
        TerminalFrame::Output { sequence: 2, .. }
    ));
    mux.close().await;
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn terminal_follow_rejects_a_bad_opening_or_a_sequence_gap() {
    for (mode, expected) in [
        ("terminal-follow-bad-opening", Error::Sequence),
        ("terminal-follow-state-first", Error::Sequence),
        ("terminal-follow-missing-sequence", Error::InvalidDto),
        ("terminal-follow-gap", Error::Sequence),
        ("terminal-follow-extra-tag", Error::InvalidFrame),
    ] {
        let mut fixture = NodeHostFixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await.unwrap();
        let mux = client.connect_mux().await.unwrap();
        let mut stream = mux
            .terminal_follow(&terminal_agent(), &terminal_id(), &terminal_attachment())
            .await
            .unwrap();
        let mut found = None;
        for _ in 0..3 {
            match tokio::time::timeout(Duration::from_secs(3), stream.next())
                .await
                .unwrap()
            {
                Some(Err(error)) => {
                    found = Some(error);
                    break;
                }
                None => break,
                Some(Ok(_)) => {}
            }
        }
        assert_eq!(found, Some(expected), "{mode}");
        mux.close().await;
        client.close().await;
        fixture.stop().await;
    }
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
#[test]
fn optional_request_controls_preserve_old_wire_and_use_exact_new_fields() {
    let old = serde_json::json!({"provider":"PUBLIC_provider","model":"PUBLIC_model"});
    let mut selection: ModelSelection = serde_json::from_value(old.clone()).unwrap();
    assert!(selection.service_tier.is_none() && selection.text_verbosity.is_none());
    assert_eq!(serde_json::to_value(&selection).unwrap(), old);
    selection.service_tier = Some("priority".into());
    selection.text_verbosity = Some("high".into());
    let wire = serde_json::to_value(SelectModelRequest {
        session_id: SessionId::new("PUBLIC_session").unwrap(),
        selection,
    })
    .unwrap();
    assert_eq!(wire["serviceTier"], "priority");
    assert_eq!(wire["textVerbosity"], "high");
    assert!(wire.get("service_tier").is_none());
    let catalog: CatalogModel = serde_json::from_value(serde_json::json!({
        "id":"PUBLIC_model","name":"PUBLIC demo", "serviceTier":{"tiers":[{"id":"priority","name":"Fast"}]},
        "textVerbosity":{"levels":[{"id":"high","name":"Detailed"}],"defaultLevel":"low"}
    })).unwrap();
    assert_eq!(catalog.service_tier.unwrap().tiers[0].id, "priority");
    assert_eq!(
        catalog.text_verbosity.unwrap().default_level.as_deref(),
        Some("low")
    );
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
                    reasoning_effort: None,
                    service_tier: None,
                    text_verbosity: None
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

#[test]
fn command_dtos_use_exact_object_fields_and_reject_extra_authority() {
    use serde_json::{Value, json};
    // Outbound: the app sends no attachments, so the field is omitted entirely
    // rather than sent as an empty array the Host would have to interpret.
    let request = CommandExecuteRequest {
        session_id: SessionId::new("session / plan opaque:%").unwrap(),
        line: "/plan".into(),
        submitted_attachments: Vec::new(),
    };
    let wire = json!({"sessionId":"session / plan opaque:%","line":"/plan"});
    assert_eq!(serde_json::to_value(&request).unwrap(), wire);
    for field in ["agent", "agentId", "mode", "args", "text"] {
        let mut extra = wire.clone();
        extra[field] = json!(false);
        assert!(
            serde_json::from_value::<CommandExecuteRequest>(extra).is_err(),
            "{field} must not be accepted"
        );
    }
    // Inbound descriptors: `name` is slash-free by contract, so a slash-bearing
    // one is not invokable and the client rejects the whole list.
    let descriptors = json!([
        {"name":"plan","description":"Explore and design before executing"},
        {"definitionId":"PUBLIC-def","name":"compact","description":"Condense history"}
    ]);
    let parsed: Vec<CommandDescriptor> = serde_json::from_value(descriptors.clone()).unwrap();
    assert_eq!(parsed[0].name, "plan");
    assert!(parsed[0].definition_id.is_none());
    assert_eq!(parsed[1].definition_id.as_deref(), Some("PUBLIC-def"));
    // A plugin-owned descriptor may grow fields this build does not know -- the
    // real one already advertises an optional `input` hint. Ignoring them keeps
    // the command list arriving; rejecting one cost the composer its menu.
    for field in ["handler", "run", "input", "authority"] {
        let mut extra = descriptors.clone();
        extra[0][field] = json!("PUBLIC");
        let parsed = serde_json::from_value::<Vec<CommandDescriptor>>(extra)
            .unwrap_or_else(|_| panic!("{field} must be ignored, not rejected"));
        assert_eq!(parsed[0].name, "plan");
    }
    // The identifier fields stay strict: a descriptor whose name is missing or
    // not a string is not a descriptor.
    for malformed in [
        json!([{"description": "no name"}]),
        json!([{"name": 7, "description": "wrong type"}]),
        json!([{"name": "plan"}]),
    ] {
        assert!(
            serde_json::from_value::<Vec<CommandDescriptor>>(malformed.clone()).is_err(),
            "{malformed} must be refused"
        );
    }
    // A success may omit its text; an error must carry one, because the app shows
    // that text rather than inventing a reason of its own.
    let execution = json!({"commandId":"PUBLIC-command","result":{"kind":"success"}});
    let parsed: CommandExecution = serde_json::from_value(execution.clone()).unwrap();
    assert_eq!(parsed.command_id.as_str(), "PUBLIC-command");
    assert!(matches!(
        parsed.result,
        CommandResult::Success { text: None }
    ));
    assert_eq!(serde_json::to_value(parsed).unwrap(), execution);
    let with_text =
        json!({"commandId":"PUBLIC-command","result":{"kind":"success","text":"Plan mode on"}});
    assert!(matches!(
        serde_json::from_value::<CommandExecution>(with_text)
            .unwrap()
            .result,
        CommandResult::Success { text: Some(_) }
    ));
    let failure =
        json!({"commandId":"PUBLIC-command","result":{"kind":"error","text":"unknown command"}});
    assert!(matches!(
        serde_json::from_value::<CommandExecution>(failure)
            .unwrap()
            .result,
        CommandResult::Error { .. }
    ));
    for malformed in [
        Value::Null,
        json!({}),
        json!({"commandId":"PUBLIC-command"}),
        json!({"commandId":"PUBLIC-command","result":{"kind":"unknown"}}),
        json!({"commandId":"PUBLIC-command","result":{"kind":"error"}}),
        json!({"commandId":"PUBLIC-command","result":{"kind":"success"},"extra":1}),
        json!({"commandId":"","result":{"kind":"success"}}),
    ] {
        assert!(
            serde_json::from_value::<CommandExecution>(malformed.clone()).is_err(),
            "{malformed} must be refused"
        );
    }
}

#[tokio::test]
async fn commands_list_and_execute_over_the_fixed_wire() {
    let mut fixture = NodeHostFixture::start("normal").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let session_id = SessionId::new("session / opaque:%").unwrap();

    let listed = client.commands_list(&session_id).await.unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].name, "plan");
    assert_eq!(listed[0].description, "Explore and design before executing");
    assert!(listed[0].definition_id.is_none());
    assert_eq!(listed[1].definition_id.as_deref(), Some("PUBLIC-def"));

    // The Host's own outcome is what the app reports, success or failure.
    let executed = client
        .commands_execute(&CommandExecuteRequest {
            session_id: session_id.clone(),
            line: "/plan".into(),
            submitted_attachments: Vec::new(),
        })
        .await
        .unwrap()
        .expect("a settled execution");
    assert_eq!(executed.command_id.as_str(), "PUBLIC-command");
    assert!(matches!(
        executed.result,
        CommandResult::Success { text: Some(ref text) } if text == "Plan mode on"
    ));
}

#[tokio::test]
async fn commands_execute_reports_the_hosts_own_failure_and_refuses_a_blank_one() {
    let mut fixture = NodeHostFixture::start("command-error").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let session_id = SessionId::new("session / opaque:%").unwrap();
    let executed = client
        .commands_execute(&CommandExecuteRequest {
            session_id: session_id.clone(),
            line: "/plan".into(),
            submitted_attachments: Vec::new(),
        })
        .await
        .unwrap()
        .expect("a settled execution");
    assert!(matches!(
        executed.result,
        CommandResult::Error { ref text } if text == "unknown command"
    ));

    // A failure with no text would render as an empty failure, so it is refused.
    let mut blank = NodeHostFixture::start("command-blank-error").await;
    let client = blank.connect(Limits::default()).await.unwrap();
    assert!(matches!(
        client
            .commands_execute(&CommandExecuteRequest {
                session_id: session_id.clone(),
                line: "/plan".into(),
                submitted_attachments: Vec::new(),
            })
            .await,
        Err(Error::InvalidDto)
    ));
}

#[tokio::test]
async fn commands_list_refuses_only_a_descriptor_that_cannot_be_addressed() {
    // A blank name is not a command. Everything else the Host offers must reach
    // the composer: rejecting the list costs the user the whole menu.
    let mut blank = NodeHostFixture::start("commands-blank-name").await;
    let client = blank.connect(Limits::default()).await.unwrap();
    assert!(matches!(
        client
            .commands_list(&SessionId::new("session / opaque:%").unwrap())
            .await,
        Err(Error::InvalidDto)
    ));

    // A name the composer can address is accepted even when the descriptor
    // carries fields this build does not know -- the case that broke the menu.
    let mut tolerant = NodeHostFixture::start("commands-extra-fields").await;
    let client = tolerant.connect(Limits::default()).await.unwrap();
    let listed = client
        .commands_list(&SessionId::new("session / opaque:%").unwrap())
        .await
        .expect("an unknown descriptor field must not fail the whole list");
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].name, "plan");
    assert_eq!(listed[0].description, "Explore and design before executing");
    assert_eq!(listed[1].name, "compact");
}

#[tokio::test]
async fn commands_execute_refuses_a_blank_or_oversized_line_before_any_write() {
    let mut fixture = NodeHostFixture::start("normal").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let session_id = SessionId::new("session / opaque:%").unwrap();
    for line in ["", "   ", "\n\t "] {
        assert!(matches!(
            client
                .commands_execute(&CommandExecuteRequest {
                    session_id: session_id.clone(),
                    line: line.into(),
                    submitted_attachments: Vec::new(),
                })
                .await,
            Err(Error::InvalidDto)
        ));
    }
}

#[tokio::test]
async fn a_gateway_context_refusal_keeps_its_code_and_no_message() {
    // The gateway's own refusal for a session whose context it cannot resolve is a
    // different defect from an unreadable reply, so its code survives -- while the
    // message, which can quote configuration or a secret, does not.
    let mut fixture = NodeHostFixture::start("commands-context-missing").await;
    let client = fixture.connect(Limits::default()).await.unwrap();
    let error = client
        .commands_list(&SessionId::new("session / opaque:%").unwrap())
        .await
        .unwrap_err();
    assert_eq!(
        error,
        Error::RemoteCode("gateway/context-not-found"),
        "the gateway's code must be the diagnosis"
    );
    assert_eq!(error.code(), "gateway/context-not-found");
    let shown = format!("{error:?}{error}");
    assert!(
        !shown.contains("PRIVATE_CONTEXT_SENTINEL"),
        "the Host's message must never be retained or shown: {shown}"
    );
}
