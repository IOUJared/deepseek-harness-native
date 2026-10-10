//! PUBLIC selected-follow regressions: tasks are dropped, no worker/runtime/model runs.
use super::*;
use serde_json::{Value, json};

fn event(seq: u64, kind: &str, data: Value) -> Value {
    json!({"type":"event","event":{"type":kind,"seq":seq,"time":1000+seq,"data":data}})
}
fn user_message(id: &str, text: &str) -> Value {
    json!({"id":id,"role":"user","source":{"kind":"user","rpcId":"PUBLIC-request","promptFingerprint":"PUBLIC-opaque-fingerprint"},"content":[{"type":"text","text":text}]})
}
fn user(seq: u64, id: &str, text: &str) -> Value {
    let mut value = event(seq, "user/message", user_message(id, text));
    value["event"]["surfaceOp"] = json!("append");
    value
}
fn snapshot(records: Vec<Value>) -> Value {
    let cursor = records
        .last()
        .map_or(-1, |value| value["event"]["seq"].as_i64().unwrap());
    json!({"type":"snapshot","records":records,"cursor":cursor,"hasMore":false,"header":{"version":4,"id":"PUBLIC-chat-flow","createdAt":1000,"isSeeded":false},"projections":{"asOfSeq":cursor,"values":{}},"assistantStream":{"revision":0}})
}
fn app() -> (App, tokio::sync::mpsc::Receiver<Command>) {
    let (handle, feed, queue, _, _) = worker::test_channels();
    let options = crate::config::parse(
        [
            "--runtime",
            "/explicit-fork/apps/cli",
            "--expected-version",
            "0.2.1-alpha.1",
            "--home",
            "/PUBLIC-native-home",
            "--user-home",
            "/PUBLIC-user-home",
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
    app.selected = Some(serde_json::from_value(json!("PUBLIC-chat-flow")).unwrap());
    (app, queue)
}
fn deliver(app: &mut App, value: Value) {
    let generation = app.generation;
    drop(app.update(Message::Worker(Event::Selected {
        generation,
        frame: serde_json::from_value(value).unwrap(),
    })));
}
fn texts(app: &App, role: &str) -> Vec<String> {
    app.transcript
        .display_rows(false)
        .filter(|row| row.role == role)
        .map(|row| row.text.clone())
        .collect()
}
fn assert_visible(app: &App, keys: &[u64]) {
    assert!(app.transcript_active());
    let (actual, heights, _) = app.scroll_projection();
    assert_eq!(actual, keys);
    assert_eq!(heights.len(), keys.len());
    assert!(heights.iter().all(|height| *height > 0.0));
    let _ = app.transcript_view();
}

#[test]
fn selected_queued_snapshot_is_chat_then_same_id_canonical_hands_off_once() {
    let (mut app, mut queue) = app();
    let queued = event(
        0,
        "agent/inbox/spliced",
        json!({"target":"next-turn","start":0,"inserted":[user_message("PUBLIC-user", "PUBLIC submitted text")]}),
    );
    deliver(&mut app, snapshot(vec![queued]));
    assert_visible(&app, &[0]);
    assert_eq!(texts(&app, "You"), ["PUBLIC submitted text"]);
    deliver(
        &mut app,
        event(
            1,
            "agent/inbox/spliced",
            json!({"target":"next-turn","start":0,"removedCount":1,"inserted":[]}),
        ),
    );
    assert_visible(&app, &[0]);
    assert_eq!(texts(&app, "You"), ["PUBLIC submitted text"]);
    deliver(&mut app, user(2, "PUBLIC-user", "PUBLIC canonical text"));
    assert_visible(&app, &[2]);
    assert_eq!(texts(&app, "You"), ["PUBLIC canonical text"]);
    assert!(app.transcript.safe_to_send());
    assert!(
        queue.try_recv().is_err(),
        "follow delivery must not dispatch commands"
    );
}

#[test]
fn selected_supplied_reasoning_and_answer_are_visible_then_durably_commit_once() {
    let (mut app, mut queue) = app();
    deliver(
        &mut app,
        snapshot(vec![user(0, "PUBLIC-user", "PUBLIC question")]),
    );
    deliver(
        &mut app,
        json!({"type":"assistant-stream","frame":{"type":"start","attemptId":"PUBLIC-attempt","revision":1,"startedAfterSeq":0,"turn":1,"step":1}}),
    );
    let chunks = [
        json!({"type":"reasoning-delta","index":0,"text":"PUBLIC supplied reasoning"}),
        json!({"type":"block-end","index":0,"block":{"type":"reasoning","text":"PUBLIC supplied reasoning"}}),
        json!({"type":"text-delta","index":1,"text":"PUBLIC supplied answer"}),
        json!({"type":"block-end","index":1,"block":{"type":"text","text":"PUBLIC supplied answer"}}),
    ];
    for (index, chunk) in chunks.iter().enumerate() {
        deliver(
            &mut app,
            json!({"type":"assistant-stream","frame":{"type":"chunk","attemptId":"PUBLIC-attempt","revision":index+2,"index":index,"time":1000,"chunk":chunk}}),
        );
    }
    let expected = "PUBLIC supplied reasoning\nPUBLIC supplied answer";
    assert_eq!(app.transcript.partial().as_deref(), Some(expected));
    assert_visible(&app, &[0]);
    assert!(
        app.scroll_projection().2 > 0.0,
        "live assistant has visible trailing geometry"
    );
    assert!(texts(&app, "Assistant").is_empty());
    let mut durable = event(
        1,
        "assistant/message",
        json!({"turn":1,"step":1,"message":{"id":"PUBLIC-assistant","role":"assistant","source":{"kind":"model","provider":"PUBLIC","model":"PUBLIC"},"content":[{"type":"reasoning","text":"PUBLIC supplied reasoning"},{"type":"text","text":"PUBLIC supplied answer"}]},"stream":[{"type":"reasoning-chunks","time0":1000,"index":0,"dt":[],"texts":["PUBLIC supplied reasoning"]},{"type":"chunk","time":1000,"chunk":chunks[1]},{"type":"text-chunks","time0":1000,"index":1,"dt":[],"texts":["PUBLIC supplied answer"]},{"type":"chunk","time":1000,"chunk":chunks[3]}]}),
    );
    durable["event"]["surfaceOp"] = json!("append");
    deliver(&mut app, durable);
    assert_eq!(app.transcript.partial().as_deref(), Some(expected));
    assert!(texts(&app, "Assistant").is_empty());
    deliver(
        &mut app,
        json!({"type":"assistant-stream","frame":{"type":"end","attemptId":"PUBLIC-attempt","revision":6,"index":4,"outcome":{"kind":"committed","eventType":"assistant/message","seq":1}}}),
    );
    assert!(app.transcript.partial().is_none());
    assert_visible(&app, &[0, 1]);
    assert_eq!(app.scroll_projection().2, 0.0);
    assert_eq!(texts(&app, "Assistant"), [expected]);
    deliver(&mut app, event(2, "step/end", json!({"turn":1,"step":1})));
    assert_eq!(texts(&app, "Assistant"), [expected]);
    assert!(app.transcript.safe_to_send());
    assert!(
        queue.try_recv().is_err(),
        "stream delivery must not dispatch commands"
    );
}

#[test]
fn selected_preparation_failure_keeps_input_and_visible_error_not_empty_welcome() {
    let (mut app, mut queue) = app();
    deliver(
        &mut app,
        snapshot(vec![event(
            0,
            "agent/inbox/spliced",
            json!({"target":"next-turn","start":0,"inserted":[user_message("PUBLIC-user", "PUBLIC submitted before failure")]}),
        )]),
    );
    for value in [
        event(1, "turn/start", json!({"turn":1})),
        event(
            2,
            "agent/inbox/spliced",
            json!({"target":"next-turn","start":0,"removedCount":1,"inserted":[]}),
        ),
        event(3, "step/start", json!({"turn":1,"step":1})),
        event(4, "step/end", json!({"turn":1,"step":1})),
        event(
            5,
            "turn/end",
            json!({"turn":1,"reason":{"kind":"error","error":{"code":"PUBLIC_ROUTE","message":"PUBLIC route unavailable"}}}),
        ),
    ] {
        deliver(&mut app, value);
    }
    assert_visible(&app, &[0, 5]);
    assert_eq!(texts(&app, "You"), ["PUBLIC submitted before failure"]);
    assert!(
        app.transcript
            .display_rows(false)
            .any(|row| row.key == 5 && row.text.contains("PUBLIC route unavailable"))
    );
    assert!(app.transcript.partial().is_none());
    assert!(app.transcript.safe_to_send());
    assert!(
        queue.try_recv().is_err(),
        "failure delivery must not dispatch commands"
    );
}
