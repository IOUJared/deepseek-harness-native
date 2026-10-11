//! Model control metadata stays retained, but never becomes a visible chat record.
use super::*;
use serde_json::{Value, json};
fn record(seq: u64, kind: &str, data: Value) -> Value {
    json!({"type":"event","event":{"type":kind,"seq":seq,"time":0,"data":data}})
}
fn app(records: Vec<Value>, cursor: i64) -> (App, tokio::sync::mpsc::Receiver<Command>) {
    let (handle, feed, queue, _, _) = worker::test_channels();
    let options = crate::config::parse(
        [
            "--runtime",
            "/explicit-fork/apps/cli",
            "--expected-version",
            "0.2.1-alpha.1",
            "--home",
            "/PUBLIC-home",
            "--user-home",
            "/PUBLIC-user",
            "--cwd",
            "/tmp",
        ]
        .map(str::to_owned),
    )
    .unwrap()
    .unwrap();
    let (mut app, task) = App::boot(options, handle, feed);
    drop(task);
    app.ready = true;
    app.root_ready = true;
    app.roster_pending = false;
    let session: SessionSummary = serde_json::from_value(json!({"sessionId":"PUBLIC-model-session","agentAvailable":true,"updatedAt":0,"running":false,"blank":true,"cwd":"/PUBLIC/workspace"})).unwrap();
    app.selected = Some(session.session_id.clone());
    app.sessions.insert(session.session_id.clone(), session);
    let frame = serde_json::from_value(json!({"type":"snapshot","records":records,"cursor":cursor,"hasMore":true,"header":{"version":4,"id":"PUBLIC-model-session","createdAt":0,"isSeeded":false},"projections":{"asOfSeq":cursor,"values":{"modelSelection":{"next":{"provider":"PUBLIC-provider","model":"PUBLIC-model","reasoningEffort":"medium","serviceTier":"priority","textVerbosity":"low"}}}},"assistantStream":{"revision":0}})).unwrap();
    drop(app.update(Message::Worker(Event::Selected {
        generation: app.generation,
        frame,
    })));
    (app, queue)
}
#[test]
fn snapshot_and_live_model_metadata_are_hidden_in_both_chat_modes_without_erasure() {
    let value =
        json!({"provider":"PUBLIC-provider","model":"PUBLIC-model","reasoningEffort":"medium"});
    let (mut app, mut queue) = app(vec![record(0, "model/selection", value.clone())], 0);
    let baseline = app
        .model_selections
        .get(app.selected.as_ref().unwrap())
        .unwrap();
    assert_eq!(baseline.model, "PUBLIC-model");
    assert_eq!(baseline.reasoning_effort.as_deref(), Some("medium"));
    assert_eq!(baseline.service_tier.as_deref(), Some("priority"));
    assert_eq!(baseline.text_verbosity.as_deref(), Some("low"));
    let frame = serde_json::from_value(record(1, "model/selection", value)).unwrap();
    drop(app.update(Message::Worker(Event::Selected {
        generation: app.generation,
        frame,
    })));
    assert_eq!(app.transcript.rows().len(), 2);
    assert_eq!(app.transcript.cursor(), 1);
    assert!(app.transcript.safe_to_send());
    for expanded in [false, true] {
        app.records_expanded = expanded;
        assert!(app.scroll_projection().0.is_empty());
        assert!(!app.transcript_active());
        drop(app.transcript_view());
    }
    assert!(queue.try_recv().is_err());
}
#[test]
fn historical_model_metadata_is_hidden_without_changing_current_menu_selection() {
    let (mut app, mut queue) = app(
        vec![record(
            2,
            "user/message",
            json!({"content":[{"type":"text","text":"PUBLIC real message"}]}),
        )],
        2,
    );
    let page: SessionPage = serde_json::from_value(json!({"hasMore":false,"records":[record(0,"model/selection",json!({"provider":"PUBLIC-old","model":"PUBLIC-old"})),record(1,"session/title",json!({"title":"PUBLIC historical"}))]})).unwrap();
    assert!(app.transcript.page(app.generation, page).is_ok());
    assert_eq!(app.transcript.rows().len(), 3);
    assert_eq!(app.transcript.cursor(), 2);
    assert_eq!(
        app.model_selections
            .get(app.selected.as_ref().unwrap())
            .unwrap()
            .model,
        "PUBLIC-model"
    );
    for expanded in [false, true] {
        app.records_expanded = expanded;
        assert_eq!(app.scroll_projection().0, vec![2]);
    }
    assert!(queue.try_recv().is_err());
}
#[test]
fn literal_model_event_names_stay_messages_while_the_switch_inject_is_trajectory_only() {
    let records = vec![
        record(
            0,
            "model/selection",
            json!({"provider":"PUBLIC","model":"PUBLIC"}),
        ),
        // The host's own model-switch notice is a producer inject, not history.
        record(
            1,
            "user/message",
            json!({"source":{"kind":"model-selection"},"content":[{"type":"text","text":"model/selection · PUBLIC model-switch notice"}]}),
        ),
        // A human message that merely mentions the event name is still history.
        record(
            2,
            "user/message",
            json!({"source":{"kind":"user","rpcId":"PUBLIC-rpc","promptFingerprint":"PUBLIC-fingerprint"},"content":[{"type":"text","text":"Explain model/selection to me"}]}),
        ),
        record(
            3,
            "assistant/message",
            json!({"turn":1,"step":1,"stream":[{"type":"text-chunks","time0":0,"index":0,"dt":[],"texts":["Discuss model/selection literally"]}]}),
        ),
    ];
    let (mut app, mut queue) = app(records, 3);
    // Chat: the control event and the host inject are filtered; the human
    // message and the assistant text that name the event stay exactly as they
    // were, because a literal name is not a control event.
    app.records_expanded = false;
    assert_eq!(app.scroll_projection().0, vec![2, 3]);
    // Trajectory keeps the inject, and never renders the control event itself.
    app.records_expanded = true;
    assert_eq!(app.scroll_projection().0, vec![1, 2, 3]);
    assert!(queue.try_recv().is_err());
}
#[test]
fn lookalikes_and_future_required_warnings_are_not_hidden() {
    for kind in [
        "model/selection-changed",
        "model/selection/next",
        "Model/selection",
        "model/selection ",
        "subagent/model-selection-policy",
    ] {
        let (mut app, mut queue) = app(
            vec![record(0, kind, json!({"warning":"PUBLIC future record"}))],
            0,
        );
        assert_eq!(app.transcript.display_rows(true).count(), 1, "{kind}");
        assert_eq!(
            app.transcript.safe_to_send(),
            kind == "subagent/model-selection-policy"
        );
        app.records_expanded = true;
        assert_eq!(app.scroll_projection().0, vec![0]);
        assert!(queue.try_recv().is_err());
    }
}
#[test]
fn malformed_exact_control_metadata_is_still_retained_not_interpreted_as_chat() {
    for data in [
        Value::Null,
        json!({"model":false}),
        json!({"future":"PUBLIC"}),
    ] {
        let (app, mut queue) = app(vec![record(0, "model/selection", data)], 0);
        assert_eq!(app.transcript.rows().len(), 1);
        assert_eq!(app.transcript.display_rows(true).count(), 0);
        assert_eq!(app.transcript.display_rows(false).count(), 0);
        assert!(app.transcript.safe_to_send());
        assert!(queue.try_recv().is_err());
    }
}
