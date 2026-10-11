//! PUBLIC local actions: no clipboard, Host or worker executor is run.
use super::*;
use serde_json::json;
pub(super) fn app() -> (App, tokio::sync::mpsc::Receiver<Command>) {
    let (handle, feed, queue, _, _) = worker::test_channels();
    let options = crate::config::parse(
        [
            "--runtime",
            "/PUBLIC-never-start/apps/cli",
            "--expected-version",
            "0.2.1-alpha.1",
            "--home",
            "/PUBLIC-native-home",
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
    app.selected = Some(SessionId::new("PUBLIC-session").unwrap());
    app.management.baseline(vec![], vec![]);
    let frame=serde_json::from_value(json!({"type":"snapshot","header":{"version":4,"id":"PUBLIC-session","createdAt":1000,"isSeeded":false},"cursor":1,"hasMore":false,"projections":{"asOfSeq":1,"values":{}},"assistantStream":{"revision":0},"records":[
        {"type":"event","event":{"type":"user/message","seq":0,"time":1000,"surfaceOp":"append","data":{"id":"PUBLIC-user","role":"user","source":{"kind":"user"},"content":[{"type":"text","text":"PUBLIC original question"}]}}},
        {"type":"event","event":{"type":"assistant/message","seq":1,"time":1001,"surfaceOp":"append","data":{"turn":1,"step":1,"stream":[{"type":"reasoning-chunks","time0":1001,"index":0,"dt":[],"texts":["PUBLIC supplied thoughts"]},{"type":"text-chunks","time0":1001,"index":1,"dt":[],"texts":["# PUBLIC answer\n\n```rust\nlet value = 42;\n```\n\nFinal paragraph."]}]}}}
    ]})).unwrap();
    drop(app.update(Message::Worker(Event::Selected {
        generation: app.generation,
        frame,
    })));
    (app, queue)
}
#[test]
fn thoughts_are_local_collapsed_and_geometry_and_copy_use_the_same_parts() {
    let (mut app, mut queue) = app();
    let prepared = app.reply_parts(1).unwrap();
    let ticket = prepared.ticket.unwrap();
    assert!(!app.thoughts_expanded.contains(&1));
    assert_eq!(prepared.reasoning, "PUBLIC supplied thoughts");
    let old = app.scroll_projection().1;
    let cursor = app.transcript.cursor();
    let safe = app.transcript.safe_to_send();
    assert!(
        !app.row_copy_source(1)
            .unwrap()
            .contains("supplied thoughts")
    );
    drop(app.update(Message::ToggleThoughts(ticket)));
    assert!(app.thoughts_expanded.contains(&1));
    assert!(app.scroll_projection().1[1] > old[1]);
    let _ = app.transcript_view();
    drop(app.update(Message::ToggleThoughts(ticket)));
    assert_eq!(app.scroll_projection().1, old);
    assert_eq!(app.transcript.cursor(), cursor);
    assert_eq!(app.transcript.safe_to_send(), safe);
    assert!(queue.try_recv().is_err());
}
#[test]
fn edit_only_populates_local_composer_and_existing_drafts_require_stamped_consent() {
    let (mut app, mut queue) = app();
    let generation = app.generation;
    drop(app.update(Message::EditReply { generation, key: 0 }));
    assert_eq!(app.editor.text(), "PUBLIC original question");
    assert_eq!(app.draft_serial, 1);
    assert!(queue.try_recv().is_err());
    app.editor = text_editor::Content::with_text("PUBLIC unsent draft");
    drop(app.update(Message::EditReply { generation, key: 0 }));
    let (_, key, draft_serial) = app.edit_review.unwrap();
    assert_eq!(app.editor.text(), "PUBLIC unsent draft");
    drop(app.update(Message::ConfirmEdit {
        generation,
        key,
        draft_serial: draft_serial + 1,
    }));
    assert_eq!(app.editor.text(), "PUBLIC unsent draft");
    drop(app.update(Message::ConfirmEdit {
        generation,
        key,
        draft_serial,
    }));
    assert_eq!(app.editor.text(), "PUBLIC original question");
    assert!(app.edit_review.is_none());
    assert!(
        queue.try_recv().is_err(),
        "Edit must never dispatch or replay"
    );
}
#[test]
fn stale_modal_and_changed_code_actions_cannot_touch_new_sources_or_drafts() {
    let (mut app, mut queue) = app();
    let generation = app.generation;
    let ticket = app.reply_parts(1).unwrap().ticket.unwrap();
    let mut stale = ticket;
    stale.generation += 1;
    drop(app.update(Message::ToggleThoughts(stale)));
    assert!(!app.thoughts_expanded.contains(&1));
    drop(app.update(Message::EditReply {
        generation: generation + 1,
        key: 0,
    }));
    assert!(app.editor.text().is_empty());
    app.reply_content.clear();
    let changed = app.reply_parts(1).unwrap().ticket.unwrap();
    assert_ne!(changed, ticket);
    drop(app.update(Message::CopyCode(ticket, 1))); // dropped Task, and stale source cannot issue copy
    let ready = app.ready;
    app.ready = false;
    drop(app.update(Message::Settings(crate::settings::Action::Open)));
    app.ready = ready;
    assert!(app.settings.is_open());
    drop(app.update(Message::ToggleThoughts(changed)));
    assert!(!app.thoughts_expanded.contains(&1));
    drop(app.update(Message::EditReply { generation, key: 0 }));
    assert!(app.editor.text().is_empty());
    assert!(queue.try_recv().is_err());
}
#[test]
fn file_receipts_and_queue_text_are_never_edit_replay_sources() {
    let (mut app, mut queue) = app();
    let generation = app.generation;
    let frame=serde_json::from_value(json!({"type":"snapshot","header":{"version":4,"id":"PUBLIC-session","createdAt":1000,"isSeeded":false},"cursor":1,"hasMore":false,"projections":{"asOfSeq":1,"values":{}},"assistantStream":{"revision":0},"records":[
        {"type":"event","event":{"type":"user/message","seq":0,"time":1000,"surfaceOp":"append","data":{"id":"PUBLIC-file","role":"user","source":{"kind":"user"},"content":[{"type":"file","attachment":{"name":"PUBLIC.txt","attachmentId":"PUBLIC-receipt","bytes":1}}]}}},
        {"type":"event","event":{"type":"agent/inbox/spliced","seq":1,"time":1001,"data":{"target":"next-turn","start":0,"inserted":[{"id":"PUBLIC-queued","role":"user","source":{"kind":"user"},"content":[{"type":"text","text":"PUBLIC queued text"}]}]}}}
    ]})).unwrap();
    drop(app.update(Message::Worker(Event::Selected { generation, frame })));
    for key in [0, 1] {
        assert!(app.transcript.editable_text(key).is_none());
        drop(app.update(Message::EditReply { generation, key }));
    }
    assert!(app.editor.text().is_empty());
    assert!(queue.try_recv().is_err());
}
