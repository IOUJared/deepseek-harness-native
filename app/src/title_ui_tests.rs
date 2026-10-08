//! PUBLIC title-only UI projection: no Host, title LLM, clipboard or business executor.
use super::*;
use serde_json::{Value, json};
fn event(seq: u64, kind: &str, data: Value) -> SessionFollowFrame {
    serde_json::from_value(
        json!({"type":"event","event":{"type":kind,"seq":seq,"time":0,"data":data}}),
    )
    .unwrap()
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
    app.follow_ready = true;
    let session: SessionSummary=serde_json::from_value(json!({"agentAvailable":true,"sessionId":"PUBLIC-title-session","updatedAt":0,"running":false,"blank":true,"cwd":"/PUBLIC/workspace","projections":{"kind":"cached","asOfSeq":-1,"values":{"title":"PUBLIC initial title"}}})).unwrap();
    app.selected = Some(session.session_id.clone());
    app.sessions.insert(session.session_id.clone(), session);
    let frame: SessionFollowFrame=serde_json::from_value(json!({"type":"snapshot","records":[],"cursor":-1,"hasMore":false,"header":{"version":4,"id":"PUBLIC-title-session","createdAt":0,"isSeeded":false},"projections":{"asOfSeq":-1,"values":{"title":"PUBLIC baseline title"}},"assistantStream":{"revision":0}})).unwrap();
    drop(app.update(Message::Worker(Event::Selected {
        generation: app.generation,
        frame,
    })));
    (app, queue)
}
fn heading(app: &App) -> String {
    app.session_title(app.sessions.get(app.selected.as_ref().unwrap()).unwrap())
}
fn deliver(app: &mut App, frame: SessionFollowFrame) {
    drop(app.update(Message::Worker(Event::Selected {
        generation: app.generation,
        frame,
    })))
}
#[test]
fn logged_titles_update_elements_not_chat_even_when_records_expanded() {
    let (mut app, mut queue) = app();
    assert_eq!(heading(&app), "PUBLIC baseline title");
    deliver(
        &mut app,
        event(
            0,
            "session/title",
            json!({"title":"PUBLIC first title","messageSeqs":[],"source":{"kind":"user"}}),
        ),
    );
    deliver(
        &mut app,
        event(
            1,
            "session/title-llm-request",
            json!({"title":"PUBLIC not the title","messages":[{"content":[{"type":"text","text":"PUBLIC opaque request"}]}]}),
        ),
    );
    deliver(
        &mut app,
        event(
            2,
            "session/title",
            json!({"title":"PUBLIC updated title","messageSeqs":[],"source":{"kind":"user"}}),
        ),
    );
    assert_eq!(heading(&app), "PUBLIC updated title");
    assert_eq!(app.transcript.rows().len(), 3);
    assert!(app.transcript.safe_to_send());
    for expanded in [false, true] {
        app.records_expanded = expanded;
        assert!(app.scroll_projection().0.is_empty());
        assert!(!app.transcript_active());
        let _ = app.transcript_view();
    }
    assert_eq!(app.transcript.display_rows(true).count(), 0);
    assert!(queue.try_recv().is_err());
}
#[test]
fn messages_with_title_event_names_are_not_hidden() {
    let (mut app, mut queue) = app();
    let mut frame = event(
        0,
        "user/message",
        json!({"content":[{"type":"text","text":"session/title session/title-llm-request"}]}),
    );
    if let SessionFollowFrame::Event { event } = &mut frame {
        event.surface_op = Some(json!("append"));
    }
    deliver(&mut app, frame);
    deliver(
        &mut app,
        event(1, "session/title", json!({"title":"PUBLIC new label"})),
    );
    deliver(
        &mut app,
        event(
            2,
            "session/title-changed",
            json!({"warning":"PUBLIC future"}),
        ),
    );
    assert_eq!(app.scroll_projection().0, vec![0, 2]);
    assert!(!app.transcript.safe_to_send());
    app.records_expanded = true;
    assert_eq!(app.scroll_projection().0, vec![0, 2]);
    assert!(queue.try_recv().is_err());
}
#[test]
fn failed_or_old_generation_title_delivery_cannot_update_elements() {
    let (mut app, mut queue) = app();
    let initial = heading(&app);
    drop(app.update(Message::Worker(Event::Selected {
        generation: app.generation + 1,
        frame: event(
            0,
            "session/title",
            json!({"title":"PUBLIC stale generation"}),
        ),
    })));
    assert_eq!(heading(&app), initial);
    assert!(app.transcript.rows().is_empty());
    deliver(
        &mut app,
        event(2, "session/title", json!({"title":"PUBLIC sequence gap"})),
    );
    assert_eq!(heading(&app), initial);
    assert!(!app.transcript.safe_to_send());
    assert!(queue.try_recv().is_err());
}
#[test]
fn title_metadata_is_bounded_and_request_or_bad_values_do_not_overwrite() {
    let (mut app, mut queue) = app();
    deliver(
        &mut app,
        event(0, "session/title", json!({"title":"PUBLIC valid title"})),
    );
    for (seq, value) in [
        (1, json!(false)),
        (2, json!({"title":"PUBLIC nested"})),
        (3, json!("   ")),
    ] {
        deliver(
            &mut app,
            event(seq, "session/title", json!({"title":value})),
        );
        assert_eq!(heading(&app), "PUBLIC valid title");
    }
    deliver(
        &mut app,
        event(4, "session/title", json!({"title":"世界".repeat(1000)})),
    );
    assert!(
        app.title_updates
            .values()
            .next()
            .unwrap()
            .1
            .as_str()
            .unwrap()
            .len()
            <= 1024
    );
    assert!(app.transcript.rows().last().unwrap().text.len() > 1024);
    assert!(app.scroll_projection().0.is_empty());
    assert!(queue.try_recv().is_err());
}
#[test]
fn newer_roster_projection_and_null_baseline_keep_normal_fallback() {
    let (mut app, mut queue) = app();
    deliver(
        &mut app,
        event(0, "session/title", json!({"title":"PUBLIC live title"})),
    );
    let id = app.selected.clone().unwrap();
    let summary = app.sessions.get_mut(&id).unwrap();
    let p = summary.projections.as_mut().unwrap();
    p.as_of_seq = 5;
    p.values
        .insert("title".into(), json!("PUBLIC newer roster title"));
    assert_eq!(heading(&app), "PUBLIC newer roster title");
    app.observe_title(6, Value::Null);
    assert_eq!(heading(&app), "New chat");
    app.observe_title(4, json!("PUBLIC older title"));
    assert_eq!(heading(&app), "New chat");
    app.title_updates.remove(&id);
    assert_eq!(heading(&app), "PUBLIC newer roster title");
    assert!(queue.try_recv().is_err());
}
#[test]
fn selected_title_cache_updates_sidebar_search_and_survives_local_selection_change() {
    let (mut app, mut queue) = app();
    deliver(
        &mut app,
        event(
            0,
            "session/title",
            json!({"title":"PUBLIC findable new title"}),
        ),
    );
    let id = app.selected.clone().unwrap();
    app.filter = "findable".into();
    assert_eq!(app.navigation_sessions().len(), 1);
    app.selected = None;
    assert_eq!(
        app.session_title(app.sessions.get(&id).unwrap()),
        "PUBLIC findable new title"
    );
    assert!(queue.try_recv().is_err());
}
