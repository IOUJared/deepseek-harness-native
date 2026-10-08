//! PUBLIC model-only detail admission tests; no business or clipboard executor.
use super::*;
use detail_panel::{Mode, Stamp};
use serde_json::json;
fn app() -> (App, tokio::sync::mpsc::Receiver<Command>) {
    let (handle, feed, queue, _, _) = worker::test_channels();
    let options = crate::config::parse(
        [
            "--runtime",
            "/PUBLIC-never-start/apps/cli",
            "--expected-version",
            "0.2.1-alpha.1",
            "--home",
            "/PUBLIC-detail-home",
            "--user-home",
            "/PUBLIC-detail-user",
            "--cwd",
            "/tmp",
        ]
        .map(str::to_owned),
    )
    .unwrap()
    .unwrap();
    let (mut app, task) = App::boot(options, handle, feed);
    drop(task);
    app.management.baseline(vec![], vec![]);
    (app, queue)
}
fn snapshot(app: &mut App, kind: &str, source: &str) {
    let data = if kind == "assistant/message" {
        json!({"turn":1,"step":1,"stream":[{"type":"text-chunks","time0":1001,"index":0,"dt":[],"texts":[source]}]})
    } else {
        json!({"content":[{"type":"text","text":source}]})
    };
    let frame=serde_json::from_value(json!({"type":"snapshot","records":[{"type":"event","event":{"type":kind,"seq":0,"time":1001,"data":data,"surfaceOp":"append"}}],"cursor":0,"hasMore":false,"header":{"version":4,"id":"PUBLIC-details","createdAt":1000,"isSeeded":false},"projections":{"asOfSeq":0,"values":{}},"assistantStream":{"revision":0}})).unwrap();
    let _ = app.transcript.apply(app.generation, frame);
}
fn open(app: &mut App) -> Stamp {
    drop(app.update(app.detail_request(0).unwrap()));
    app.detail.as_ref().unwrap().ticket.unwrap()
}
#[test]
fn local_format_source_preserves_frozen_bytes_draft_geometry_and_authority() {
    let (mut app, mut queue) = app();
    let source = format!(
        "# PUBLIC title\n\n**Bold** and *italic*.\n\n```rust\nlet public = 1;\n```\n\n{}",
        "PUBLIC beyond excerpt. ".repeat(40)
    );
    snapshot(&mut app, "assistant/message", &source);
    app.editor = text_editor::Content::with_text("PUBLIC unsent draft");
    app.offset = 14.0;
    app.follow_bottom = false;
    let ticket = open(&mut app);
    let identity = (
        app.generation,
        app.scroll_revision,
        app.scroll_feedback,
        app.offset,
        app.follow_bottom,
    );
    let error = app.transcript.safe_to_send();
    let heights = app.scroll_projection().1;
    assert_eq!(app.detail.as_ref().unwrap().source, source);
    assert!(app.detail.as_ref().unwrap().formatted_available());
    assert_eq!(app.detail.as_ref().unwrap().mode, Mode::Source);
    for mode in [Mode::Formatted, Mode::Source] {
        drop(app.update(Message::DetailMode(ticket, mode)));
        assert_eq!(app.detail.as_ref().unwrap().mode, mode);
        drop(app.view());
        assert_eq!(app.detail.as_ref().unwrap().source, source);
    }
    assert_eq!(
        (
            app.generation,
            app.scroll_revision,
            app.scroll_feedback,
            app.offset,
            app.follow_bottom
        ),
        identity
    );
    assert_eq!(app.scroll_projection().1, heights);
    assert_eq!(app.transcript.safe_to_send(), error);
    assert_eq!(app.editor.text(), "PUBLIC unsent draft");
    assert!(queue.try_recv().is_err());
}
#[test]
fn format_never_adopts_unknown_assistant_role_or_tools_and_never_clears_error() {
    for kind in ["Assistant", "tool/result", "user/message", "system/message"] {
        let (mut app, mut queue) = app();
        snapshot(&mut app, kind, "# PUBLIC pretend heading");
        let before = app.transcript.safe_to_send();
        let ticket = open(&mut app);
        drop(app.update(Message::DetailMode(ticket, Mode::Formatted)));
        assert!(!app.detail.as_ref().unwrap().formatted_available());
        assert_eq!(app.detail.as_ref().unwrap().mode, Mode::Source);
        assert_eq!(app.transcript.safe_to_send(), before);
        assert!(queue.try_recv().is_err());
    }
}
#[test]
fn stale_ticket_cannot_change_reopened_same_record_or_session() {
    let (mut app, mut queue) = app();
    snapshot(&mut app, "assistant/message", "# PUBLIC");
    let old = open(&mut app);
    drop(app.update(app.detail_back().unwrap()));
    let fresh = open(&mut app);
    assert_ne!(old, fresh);
    drop(app.update(Message::DetailMode(old, Mode::Formatted)));
    assert_eq!(app.detail.as_ref().unwrap().mode, Mode::Source);
    drop(app.update(Message::DetailMode(fresh, Mode::Formatted)));
    assert_eq!(app.detail.as_ref().unwrap().mode, Mode::Formatted);
    drop(app.update(app.detail_back().unwrap()));
    app.generation += 1;
    app.transcript = Transcript::new(app.generation);
    snapshot(&mut app, "assistant/message", "# PUBLIC other session");
    let next = open(&mut app);
    drop(app.update(Message::DetailMode(fresh, Mode::Formatted)));
    assert_eq!(app.detail.as_ref().unwrap().mode, Mode::Source);
    assert_ne!(next, fresh);
    assert!(queue.try_recv().is_err());
}
#[test]
fn covered_or_closing_detail_controls_are_inert() {
    for cover in 0..5 {
        let (mut app, mut queue) = app();
        snapshot(&mut app, "assistant/message", "# PUBLIC");
        let ticket = open(&mut app);
        match cover {
            0 => app.conversation_menu = true,
            1 => app.capabilities = true,
            2 => app.sidebar_sheet = true,
            3 => app.close = Close::Confirm("PUBLIC".into()),
            _ => drop(app.update(Message::Settings(crate::settings::Action::Open))),
        }
        assert!(!app.detail_visible());
        assert!(background_message(&Message::CopyDetailSource(ticket)));
        assert!(app.detail_copy_source(ticket).is_none());
        assert_eq!(app.update(Message::CopyDetailSource(ticket)).units(), 0);
        let message = Message::DetailMode(ticket, Mode::Formatted);
        assert!(background_message(&message));
        drop(app.update(message));
        assert_eq!(app.detail.as_ref().unwrap().mode, Mode::Source);
        drop(app.update(Message::HideDetails(ticket)));
        assert!(app.detail.is_some());
        assert!(queue.try_recv().is_err());
    }
}
#[test]
fn unsupported_document_preserves_complete_source_and_nonce_exhaustion_disables_actions() {
    let (mut app, mut queue) = app();
    let source = "# PUBLIC\n\n![no fetch](file:///PUBLIC/no-image.png)\n\n<p>literal html</p>";
    snapshot(&mut app, "assistant/message", source);
    let ticket = open(&mut app);
    assert!(!app.detail.as_ref().unwrap().formatted_available());
    assert!(
        app.detail
            .as_ref()
            .unwrap()
            .notice()
            .contains("unsupported")
    );
    drop(app.update(Message::DetailMode(ticket, Mode::Formatted)));
    assert_eq!(app.detail.as_ref().unwrap().source, source);
    drop(app.update(app.detail_back().unwrap()));
    app.detail_opening = Some(u64::MAX);
    open_without_ticket(&mut app);
    assert!(app.detail.is_none());
    assert!(app.detail_opening.is_none());
    assert!(app.detail_request(0).is_none());
    drop(app.update(Message::DetailMode(ticket, Mode::Formatted)));
    drop(app.update(Message::HideDetails(ticket)));
    assert!(app.detail.is_none());
    assert!(queue.try_recv().is_err());
}
fn open_without_ticket(app: &mut App) {
    drop(app.update(Message::Details(detail_panel::Opening {
        generation: app.generation,
        prior_opening: u64::MAX,
        key: 0,
    })));
}
#[test]
fn disconnected_read_only_controls_remain_local_and_snapshot_is_not_rebased() {
    let (mut app, mut queue) = app();
    snapshot(&mut app, "assistant/message", "# PUBLIC old snapshot");
    let ticket = open(&mut app);
    let source = app.detail.as_ref().unwrap().source.clone();
    app.ready = false;
    app.follow_ready = false;
    snapshot(&mut app, "assistant/message", "# PUBLIC new snapshot");
    drop(app.update(Message::DetailMode(ticket, Mode::Formatted)));
    assert_eq!(app.detail.as_ref().unwrap().mode, Mode::Formatted);
    assert_eq!(app.detail.as_ref().unwrap().source, source);
    assert!(queue.try_recv().is_err());
}
#[test]
fn management_export_and_decision_cover_reject_queued_format_messages() {
    for cover in 0..3 {
        let (mut app, mut queue) = app();
        snapshot(&mut app, "assistant/message", "# PUBLIC snapshot");
        let ticket = open(&mut app);
        let view = crate::management::View {
            epoch: app.transport_epoch,
            generation: app.generation,
            target: SessionId::new("PUBLIC-details").unwrap(),
        };
        match cover {
            0 => {
                let context = crate::management::Context {
                    view: Some(view.clone()),
                    allowed: true,
                };
                assert!(
                    app.management
                        .update(crate::management::Action::Open(view), &context)
                        .is_none()
                );
                assert!(app.management.is_open());
            }
            1 => {
                let context = crate::exporter::Context {
                    view: Some(view.clone()),
                    allowed: true,
                };
                assert!(
                    app.exporter
                        .apply(crate::exporter::Action::Open(view), &context)
                        .is_none()
                );
                assert!(app.exporter.is_open());
            }
            _ => {
                app.ready = true;
                app.root_ready = true;
                let key = crate::interactions::Key {
                    epoch: app.transport_epoch,
                    event_id: RemoteEventId::new("PUBLIC-detail-cover").unwrap(),
                    serial: 1,
                };
                drop(app.receive(Event::Root{frame:RemoteEventFrame::Waterfall{event:WaterfallKind::Approval,event_id:key.event_id.clone(),agent_id:AgentId::new("PUBLIC-detail-agent").unwrap(),request:json!({"toolName":"PUBLIC never-executed","callId":"PUBLIC call","reason":"PUBLIC cover"})},decision_key:Some(key.clone())}));
                drop(app.update(Message::Interaction(crate::interactions::Action::Open(key))));
                assert!(app.decisions.is_open());
            }
        }
        assert!(!app.detail_visible());
        assert!(app.detail_copy_source(ticket).is_none());
        assert_eq!(app.update(Message::CopyDetailSource(ticket)).units(), 0);
        drop(app.update(Message::DetailMode(ticket, Mode::Formatted)));
        assert_eq!(app.detail.as_ref().unwrap().mode, Mode::Source);
        drop(app.update(Message::HideDetails(ticket)));
        assert!(app.detail.is_some());
        // Model a queued opener from the still-current shell under the same cover.
        app.detail = None;
        let before = app.detail_opening;
        drop(app.update(app.detail_request(0).unwrap()));
        assert!(app.detail.is_none());
        assert_eq!(app.detail_opening, before);
        assert!(queue.try_recv().is_err());
    }
}
#[test]
fn opener_from_another_generation_cannot_adopt_same_record_key() {
    let (mut app, mut queue) = app();
    snapshot(&mut app, "assistant/message", "# PUBLIC first generation");
    let stale = app.detail_request(0).unwrap();
    app.generation += 1;
    app.transcript = Transcript::new(app.generation);
    snapshot(&mut app, "assistant/message", "# PUBLIC second generation");
    drop(app.update(stale));
    assert!(app.detail.is_none());
    assert_eq!(app.detail_opening, Some(0));
    let fresh = open(&mut app);
    assert_eq!(fresh.generation, app.generation);
    assert_eq!(
        app.detail.as_ref().unwrap().source,
        "# PUBLIC second generation"
    );
    assert!(queue.try_recv().is_err());
}
#[test]
fn opener_is_consumed_once_and_cannot_reopen_after_back() {
    let (mut app, mut queue) = app();
    snapshot(&mut app, "assistant/message", "# PUBLIC initial");
    let Message::Details(stale) = app.detail_request(0).unwrap() else {
        panic!()
    };
    drop(app.update(Message::Details(stale)));
    let first = app.detail.as_ref().unwrap().ticket.unwrap();
    drop(app.update(Message::HideDetails(first)));
    snapshot(&mut app, "assistant/message", "# PUBLIC later");
    drop(app.update(Message::Details(stale)));
    assert!(app.detail.is_none());
    assert_eq!(app.detail_opening, Some(first.opening));
    let fresh = open(&mut app);
    assert!(fresh.opening > first.opening);
    assert_eq!(app.detail.as_ref().unwrap().source, "# PUBLIC later");
    assert!(queue.try_recv().is_err());
}
#[test]
fn stale_back_cannot_close_reopened_panel_or_current_generation_panel() {
    let (mut app, mut queue) = app();
    snapshot(&mut app, "assistant/message", "# PUBLIC");
    let old = open(&mut app);
    drop(app.update(Message::HideDetails(old)));
    let current = open(&mut app);
    drop(app.update(Message::HideDetails(old)));
    assert_eq!(app.detail.as_ref().unwrap().ticket, Some(current));
    drop(app.update(Message::HideDetails(Stamp {
        generation: app.generation + 1,
        ..current
    })));
    assert_eq!(app.detail.as_ref().unwrap().ticket, Some(current));
    app.ready = false;
    drop(app.update(Message::HideDetails(current)));
    assert!(app.detail.is_none());
    assert!(queue.try_recv().is_err());
}
#[test]
fn missing_key_or_wrong_opening_lease_never_consumes_panel_identity() {
    let (mut app, mut queue) = app();
    snapshot(&mut app, "assistant/message", "# PUBLIC");
    for (prior_opening, key) in [(1, 0), (0, 100), (0, u64::MAX)] {
        drop(app.update(Message::Details(detail_panel::Opening {
            generation: app.generation,
            prior_opening,
            key,
        })));
        assert!(app.detail.is_none());
        assert_eq!(app.detail_opening, Some(0));
    }
    let current = open(&mut app);
    assert_eq!(current.opening, 1);
    assert!(background_message(&Message::HideDetails(current)));
    assert!(queue.try_recv().is_err());
}
#[test]
fn exhaustion_preserves_readable_last_panel_and_its_valid_back_ticket() {
    let (mut app, mut queue) = app();
    snapshot(&mut app, "assistant/message", "# PUBLIC retained");
    app.detail_opening = Some(u64::MAX - 1);
    let last = open(&mut app);
    assert_eq!(last.opening, u64::MAX);
    assert!(app.detail_request(0).is_none());
    app.open_detail(
        "PUBLIC not adopted".into(),
        "PUBLIC replacement".into(),
        None,
        false,
    );
    assert!(app.detail_opening.is_none());
    assert_eq!(app.detail.as_ref().unwrap().source, "# PUBLIC retained");
    drop(app.update(Message::DetailMode(last, Mode::Formatted)));
    assert_eq!(app.detail.as_ref().unwrap().mode, Mode::Formatted);
    drop(app.update(Message::HideDetails(last)));
    assert!(app.detail.is_none());
    assert!(app.detail_back().is_none());
    app.generation += 1;
    assert!(app.detail_request(0).is_none());
    assert!(queue.try_recv().is_err());
}
#[test]
fn escape_is_stamp_bound_and_stateless_dismiss_never_closes_details() {
    let (mut app, mut queue) = app();
    snapshot(&mut app, "assistant/message", "# PUBLIC");
    let key = keyboard::Key::Named(keyboard::key::Named::Escape);
    let first = open(&mut app);
    let old_escape = panel_shortcut(
        &key,
        keyboard::Modifiers::empty(),
        app.detail_escape_ticket(),
    )
    .unwrap();
    assert!(matches!(old_escape,Message::HideDetails(ticket) if ticket==first));
    drop(app.update(app.detail_back().unwrap()));
    let second = open(&mut app);
    drop(app.update(old_escape));
    drop(app.update(Message::DismissPanel));
    assert_eq!(app.detail.as_ref().unwrap().ticket, Some(second));
    let current_escape = panel_shortcut(
        &key,
        keyboard::Modifiers::empty(),
        app.detail_escape_ticket(),
    )
    .unwrap();
    drop(app.update(current_escape));
    assert!(app.detail.is_none());
    assert!(queue.try_recv().is_err());
}
#[test]
fn escape_closes_only_the_cover_and_preserves_hidden_details() {
    for cover in 0..3 {
        let (mut app, mut queue) = app();
        snapshot(&mut app, "assistant/message", "# PUBLIC");
        let ticket = open(&mut app);
        match cover {
            0 => app.capabilities = true,
            1 => app.sidebar_sheet = true,
            _ => app.conversation_menu = true,
        }
        assert!(app.detail_escape_ticket().is_none());
        drop(app.update(Message::HideDetails(ticket)));
        assert_eq!(app.detail.as_ref().unwrap().ticket, Some(ticket));
        let escape = panel_shortcut(
            &keyboard::Key::Named(keyboard::key::Named::Escape),
            keyboard::Modifiers::empty(),
            app.detail_escape_ticket(),
        )
        .unwrap();
        assert!(matches!(escape, Message::DismissPanel));
        drop(app.update(escape));
        assert!(app.detail_visible());
        assert_eq!(app.detail.as_ref().unwrap().ticket, Some(ticket));
        assert!(queue.try_recv().is_err());
    }
}
#[test]
fn queued_openers_do_not_open_through_shell_or_modal_covers() {
    for cover in 0..5 {
        let (mut app, mut queue) = app();
        snapshot(&mut app, "assistant/message", "# PUBLIC");
        let request = app.detail_request(0).unwrap();
        match cover {
            0 => app.conversation_menu = true,
            1 => app.capabilities = true,
            2 => app.sidebar_sheet = true,
            3 => app.close = Close::Confirm("PUBLIC".into()),
            _ => drop(app.update(Message::Settings(crate::settings::Action::Open))),
        }
        drop(app.update(request));
        assert!(app.detail.is_none());
        assert_eq!(app.detail_opening, Some(0));
        assert!(queue.try_recv().is_err());
    }
}
#[test]
fn actual_select_and_reload_routes_retire_old_openers_and_back() {
    for reload in [false, true] {
        let (mut app, mut queue) = app();
        snapshot(&mut app, "assistant/message", "# PUBLIC before");
        let stale_open = app.detail_request(0).unwrap();
        let stale_back = open(&mut app);
        app.ready = true;
        app.selected = Some(SessionId::new("PUBLIC-details").unwrap());
        let previous_generation = app.generation;
        drop(app.update(if reload {
            Message::Reload
        } else {
            Message::Select(SessionId::new("PUBLIC-other").unwrap())
        }));
        assert!(app.generation > previous_generation);
        assert!(
            matches!(queue.try_recv().unwrap(),Command::Select{generation,..} if generation==app.generation)
        );
        snapshot(&mut app, "assistant/message", "# PUBLIC after");
        drop(app.update(stale_open));
        assert!(app.detail.is_none());
        let current = open(&mut app);
        drop(app.update(Message::HideDetails(stale_back)));
        assert_eq!(app.detail.as_ref().unwrap().ticket, Some(current));
        assert!(queue.try_recv().is_err());
    }
}
#[test]
fn detail_debug_redacts_heading_source_and_plan() {
    let detail = detail_panel::Detail::new(
        "PUBLIC sensitive heading".into(),
        "# PUBLIC sensitive source".into(),
        None,
        true,
        Some(Stamp {
            generation: 1,
            opening: 1,
        }),
    );
    let debug = format!("{detail:?}");
    assert!(!debug.contains("sensitive"));
    assert!(!debug.contains("PUBLIC"));
}

#[test]
fn copy_admits_only_exact_frozen_source_without_metadata_or_formatted_rewrite() {
    let (mut app, mut queue) = app();
    let source = "# PUBLIC clipboard · café 世界\n\n**literal Markdown**\n\n[PUBLIC](https://example.invalid/PUBLIC)\n";
    snapshot(&mut app, "assistant/message", source);
    app.editor = text_editor::Content::with_text("PUBLIC unsent draft");
    app.follow_bottom = false;
    app.offset = 14.0;
    let ticket = open(&mut app);
    let before = (
        app.generation,
        app.scroll_revision,
        app.scroll_feedback,
        app.offset,
        app.follow_bottom,
    );
    let heights = app.scroll_projection().1;
    let authority = app.transcript.safe_to_send();
    for mode in [Mode::Source, Mode::Formatted] {
        drop(app.update(Message::DetailMode(ticket, mode)));
        assert_eq!(app.detail_copy_source(ticket).as_deref(), Some(source));
        let task = app.update(Message::CopyDetailSource(ticket));
        assert_eq!(task.units(), 1); // Inspect admission only; NEVER execute the clipboard task.
        drop(task);
    }
    assert_eq!(
        (
            app.generation,
            app.scroll_revision,
            app.scroll_feedback,
            app.offset,
            app.follow_bottom
        ),
        before
    );
    assert_eq!(app.scroll_projection().1, heights);
    assert_eq!(app.transcript.safe_to_send(), authority);
    assert_eq!(app.editor.text(), "PUBLIC unsent draft");
    assert!(
        !app.detail_copy_source(ticket)
            .unwrap()
            .contains("timestamp")
    );
    assert!(queue.try_recv().is_err());
}

#[test]
fn stale_copy_messages_cannot_clear_clipboard_or_copy_a_newly_opened_panel() {
    let (mut app, mut queue) = app();
    snapshot(&mut app, "assistant/message", "# PUBLIC old");
    let old = open(&mut app);
    drop(app.update(app.detail_back().unwrap()));
    assert!(app.detail_copy_source(old).is_none());
    assert_eq!(app.update(Message::CopyDetailSource(old)).units(), 0);
    snapshot(&mut app, "assistant/message", "# PUBLIC new");
    let current = open(&mut app);
    assert!(app.detail_copy_source(old).is_none());
    assert_eq!(app.update(Message::CopyDetailSource(old)).units(), 0);
    assert_eq!(
        app.detail_copy_source(current).as_deref(),
        Some("# PUBLIC new")
    );
    let wrong = Stamp {
        generation: current.generation + 1,
        ..current
    };
    assert_eq!(app.update(Message::CopyDetailSource(wrong)).units(), 0);
    app.generation += 1;
    assert!(app.detail_copy_source(current).is_none());
    assert_eq!(app.update(Message::CopyDetailSource(current)).units(), 0);
    assert!(queue.try_recv().is_err());
}

#[test]
fn copy_readable_offline_panel_keeps_source_frozen_without_backend_authority() {
    let (mut app, mut queue) = app();
    snapshot(&mut app, "assistant/message", "# PUBLIC frozen");
    let ticket = open(&mut app);
    app.ready = false;
    app.follow_ready = false;
    snapshot(&mut app, "assistant/message", "# PUBLIC later live wire");
    assert_eq!(
        app.detail_copy_source(ticket).as_deref(),
        Some("# PUBLIC frozen")
    );
    let task = app.update(Message::CopyDetailSource(ticket));
    assert_eq!(task.units(), 1);
    drop(task);
    assert!(queue.try_recv().is_err());
}

#[test]
fn empty_or_oversized_display_source_refuses_copy_without_truncation() {
    for bytes in [0, 32768, 32769] {
        let (mut app, mut queue) = app();
        let source = "P".repeat(bytes);
        app.open_detail(
            "PUBLIC local".into(),
            source.clone(),
            Some("PUBLIC metadata".into()),
            false,
        );
        let ticket = app.detail.as_ref().unwrap().ticket.unwrap();
        assert_eq!(app.detail.as_ref().unwrap().source, source);
        assert_eq!(
            app.detail.as_ref().unwrap().copy_available(),
            bytes == 32768
        );
        assert_eq!(app.detail_copy_source(ticket).is_some(), bytes == 32768);
        let task = app.update(Message::CopyDetailSource(ticket));
        assert_eq!(task.units(), usize::from(bytes == 32768));
        drop(task);
        assert_eq!(app.detail.as_ref().unwrap().source, source);
        assert!(queue.try_recv().is_err());
    }
}

#[test]
fn last_valid_ticket_still_copies_after_opening_counter_exhaustion() {
    let (mut app, mut queue) = app();
    snapshot(&mut app, "assistant/message", "# PUBLIC final");
    app.detail_opening = Some(u64::MAX - 1);
    let ticket = open(&mut app);
    assert_eq!(ticket.opening, u64::MAX);
    app.open_detail(
        "PUBLIC not admitted".into(),
        "PUBLIC wrong".into(),
        None,
        false,
    );
    assert!(app.detail_opening.is_none());
    assert_eq!(
        app.detail_copy_source(ticket).as_deref(),
        Some("# PUBLIC final")
    );
    let task = app.update(Message::CopyDetailSource(ticket));
    assert_eq!(task.units(), 1);
    drop(task);
    assert!(queue.try_recv().is_err());
}
