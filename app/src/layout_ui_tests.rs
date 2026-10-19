//! Public fake App routing tests; no worker/Host/account/browser/model execution.
use super::*;
fn app() -> (App, tokio::sync::mpsc::Receiver<Command>) {
    let (handle, feed, receiver, _, _) = worker::test_channels();
    let options = crate::config::parse(
        [
            "--runtime",
            "/explicit-fork/apps/cli",
            "--expected-version",
            "0.2.1-alpha.1",
            "--home",
            "/PUBLIC-layout-native-home",
            "--user-home",
            "/PUBLIC-layout-user-home",
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
    (app, receiver)
}
#[test]
fn menu_is_view_only_preserves_draft_and_disables_composition() {
    let (mut app, mut queue) = app();
    app.editor = text_editor::Content::with_text("PUBLIC unsent draft");
    assert!(app.composer_visible());
    drop(app.update(Message::ToggleConversationMenu));
    assert!(app.conversation_menu && !app.composer_visible());
    drop(app.view());
    assert_eq!(app.editor.text(), "PUBLIC unsent draft");
    drop(app.update(Message::ToggleConversationMenu));
    assert!(!app.conversation_menu && app.composer_visible());
    assert!(queue.try_recv().is_err());
}
#[test]
fn escape_closes_menu_without_dismissing_underlying_information() {
    let (mut app, mut queue) = app();
    drop(app.update(Message::Capabilities));
    drop(app.update(Message::ToggleConversationMenu));
    drop(app.update(Message::DismissPanel));
    assert!(app.capabilities && !app.conversation_menu);
    assert!(queue.try_recv().is_err());
}
#[test]
fn information_disclosures_are_local_collapsed_and_ignore_hidden_messages() {
    let (mut app, mut queue) = app();
    drop(app.update(Message::ToggleInfoDetails(InfoSection::Diagnostics)));
    assert_eq!(app.info_expanded, 0);
    drop(app.update(Message::Capabilities));
    for section in [InfoSection::Features, InfoSection::Diagnostics] {
        drop(app.update(Message::ToggleInfoDetails(section)));
        assert_ne!(app.info_expanded & section.bit(), 0);
        drop(app.view());
    }
    drop(app.update(Message::Capabilities));
    assert_eq!(app.info_expanded, 0);
    assert!(queue.try_recv().is_err());
}
#[test]
fn settings_closes_menu_and_fences_stale_menu_disclosure_actions() {
    let (mut app, mut queue) = app();
    drop(app.update(Message::ToggleConversationMenu));
    drop(app.update(Message::Settings(crate::settings::Action::Open)));
    assert!(app.settings.is_open() && !app.conversation_menu);
    for message in [
        Message::ToggleConversationMenu,
        Message::ToggleInfoDetails(InfoSection::Diagnostics),
    ] {
        assert!(background_message(&message));
        drop(app.update(message));
    }
    assert!(!app.conversation_menu && app.info_expanded == 0);
    assert!(queue.try_recv().is_err());
}
#[test]
fn resize_dismisses_menu_without_work_and_supports_small_logical_frames() {
    let (mut app, mut queue) = app();
    for size in [
        iced::Size::new(1040.0, 700.0),
        iced::Size::new(760.0, 560.0),
        iced::Size::new(608.0, 448.0),
    ] {
        drop(app.update(Message::ToggleConversationMenu));
        drop(app.update(Message::Resized(size)));
        assert!(!app.conversation_menu);
        assert_eq!(app.layout_size(), size);
        drop(app.view());
    }
    assert!(queue.try_recv().is_err());
}
#[test]
fn closing_state_rejects_local_menu_and_info_changes() {
    let (mut app, mut queue) = app();
    app.close = Close::Stopping;
    app.capabilities = true;
    drop(app.update(Message::ToggleConversationMenu));
    drop(app.update(Message::ToggleInfoDetails(InfoSection::Features)));
    assert!(!app.conversation_menu && app.info_expanded == 0);
    assert!(queue.try_recv().is_err());
}
#[cfg(feature = "public-layout-fixture")]
#[test]
fn a_search_reaches_a_tool_body_and_reveals_it() {
    let (size, _scale) = super::layout_fixture::preset_size("tool-activity-terminal").unwrap();
    let (mut app, _queue) = super::layout_fixture::build(
        super::layout_fixture::test_options(),
        "tool-activity-terminal",
        size,
        None,
    );
    // The terminal card's own output is part of what the user can search, as the reference lets the
    // browser's find reach it.
    app.search_open = true;
    app.search_query = "terminal output line two".into();
    app.search_cursor = None;
    let hits = app.search_hits();
    assert!(!hits.is_empty(), "a tool body is searched");
    // Jumping to the hit opens the row so the match is visible, not merely counted.
    app.jump_search(true, false);
    let key = app.search_cursor.expect("a hit was selected");
    assert!(
        app.activity_expanded.contains(&key),
        "the hit's row is revealed"
    );
}

#[cfg(feature = "public-layout-fixture")]
#[test]
fn the_trajectory_filter_narrows_records_by_all_of_its_terms() {
    let (size, _scale) = super::layout_fixture::preset_size("tool-activity").unwrap();
    let (mut app, _queue) = super::layout_fixture::build(
        super::layout_fixture::test_options(),
        "tool-activity",
        size,
        None,
    );
    // Chat shows conversation rows; the filter is a Trajectory control and must not touch them.
    assert!(app.visible_rows().len() >= 2);
    let chat_rows = app.visible_rows().len();
    app.trajectory_query = "no-such-record".into();
    assert_eq!(app.visible_rows().len(), chat_rows, "Chat is not filtered");

    app.trajectory_query.clear();
    app.records_expanded = true;
    let all = app.visible_rows().len();
    assert!(all >= 2, "Trajectory lists the records: {all}");
    // A term that no record carries narrows the list away, and every term must be present.
    app.trajectory_query = "PUBLIC nonexistent".into();
    assert!(app.visible_rows().is_empty());
    app.trajectory_query = "tool/call".into();
    let calls = app.visible_rows().len();
    assert!(
        calls >= 1 && calls <= all,
        "a record kind matches: {calls} of {all}"
    );
    // The filter is a real narrowing, not a no-op: it keeps fewer rows than the unfiltered tab.
    app.trajectory_query.clear();
    let unfiltered = app.visible_rows().len();
    assert!(calls < unfiltered, "{calls} of {unfiltered}");
    // Case does not matter.
    app.trajectory_query = "TOOL/CALL".into();
    assert_eq!(app.visible_rows().len(), calls);
    // Two terms are conjunctive: a kind plus a word only some calls carry.
    app.trajectory_query = "tool/call Public-note".into();
    assert!(app.visible_rows().len() <= calls);
}

#[cfg(feature = "public-layout-fixture")]
#[test]
fn a_failed_history_read_offers_a_retry_that_asks_again() {
    use crate::worker::Command;
    let (size, _scale) = super::layout_fixture::preset_size("schedules-history-error").unwrap();
    let (mut app, mut queue) = super::layout_fixture::build(
        super::layout_fixture::test_options(),
        "schedules-history-error",
        size,
        None,
    );
    let (session, id) = app.schedule_history.clone().expect("the history is open");
    assert!(app.schedule_history_error.is_some());
    assert!(app.schedule_history_page.is_none());
    while queue.try_recv().is_ok() {}
    // The row's Retry re-runs the read that failed.
    drop(app.update(Message::ScheduleHistory(session.clone(), id.clone())));
    let Command::ScheduleHistory {
        session: asked,
        id: asked_id,
        ..
    } = queue.try_recv().expect("the retry asks the Host again")
    else {
        panic!("a retry is a history read");
    };
    assert_eq!(asked, session);
    assert_eq!(asked_id, id);
    assert!(app.schedule_history_loading, "the retry is in flight");
}

#[cfg(feature = "public-layout-fixture")]
#[test]
fn steer_all_walks_the_queue_one_item_at_a_time() {
    use dsh_native_transport::dto::QueueAction;
    // The fixture's running session with two queued messages: a real queue, no Host.
    let (size, _scale) = super::layout_fixture::preset_size("working-queue").unwrap();
    let (mut app, mut queue) = super::layout_fixture::build(
        super::layout_fixture::test_options(),
        "working-queue",
        size,
        None,
    );
    assert!(app.queue_steer_enabled(), "the session is running");
    assert_eq!(app.transcript.queued().len(), 2, "two queued rows");
    while queue.try_recv().is_ok() {}
    app.show_queue_steer_all();
    // The first steer is dispatched; the second waits for the first to settle.
    let Command::UpdateQueue(submission) = queue.try_recv().expect("the first steer") else {
        panic!("a steer is an updateQueue call");
    };
    assert!(matches!(submission.action, QueueAction::Steer));
    assert!(queue.try_recv().is_err(), "one at a time");
    let first = submission.item_id.clone();
    drop(app.receive(crate::worker::Event::QueueUpdated {
        submission,
        result: Ok(()),
    }));
    let Command::UpdateQueue(second) = queue.try_recv().expect("the second steer follows") else {
        panic!("a steer is an updateQueue call");
    };
    assert_ne!(second.item_id, first, "each queued item steers once");
    // A refusal ends the run rather than pressing on.
    drop(app.receive(crate::worker::Event::QueueUpdated {
        submission: second,
        result: Err(crate::queue_update::Failure::Rejected),
    }));
    assert!(queue.try_recv().is_err(), "a refusal stops steer-all");
    assert!(app.queue_error.is_some(), "the refusal is reported");
}

#[test]
fn a_command_that_takes_arguments_states_what_to_write() {
    let (mut app, _queue) = app();
    app.ready = true;
    app.selected = Some(serde_json::from_value(serde_json::json!("PUBLIC-hint")).unwrap());
    app.editor = iced::widget::text_editor::Content::with_text("plain text");
    assert_eq!(app.composer_hint(), None, "ordinary prose has no hint");
    app.editor = iced::widget::text_editor::Content::with_text("/plan");
    assert_eq!(
        app.composer_hint(),
        Some("describe your task to generate plan")
    );
    // A trailing argument keeps the hint: the command is still the one being written.
    app.editor = iced::widget::text_editor::Content::with_text("/plan fix the fold");
    assert!(app.composer_hint().is_some());
    // An unknown command has no hint of its own.
    app.editor = iced::widget::text_editor::Content::with_text("/compact everything");
    assert_eq!(app.composer_hint(), None, "no hint is invented");
}

#[test]
fn a_pending_advertisement_shows_a_loading_row() {
    let (mut app, _queue) = app();
    app.ready = true;
    app.selected = Some(serde_json::from_value(serde_json::json!("PUBLIC-loading")).unwrap());
    assert!(!app.awaiting_advertisement(), "no command position yet");
    app.editor = iced::widget::text_editor::Content::with_text("/");
    app.commands_pending = true;
    assert!(
        app.awaiting_advertisement(),
        "a slash with a read in flight shows the loading row"
    );
    // An answered advertisement is not loading, even while the skill read continues.
    app.commands_pending = false;
    app.commands = Some(vec![crate::ui::command_menu::Candidate::command(
        "plan".into(),
        "Explore".into(),
    )]);
    app.skills_pending = true;
    assert!(
        !app.awaiting_advertisement(),
        "the command menu has rows to show"
    );
    // Prose is not a command position, so there is nothing to load.
    app.editor = iced::widget::text_editor::Content::with_text("plain text");
    assert!(!app.awaiting_advertisement());
}

#[test]
fn a_hovered_drop_draws_the_overlay_and_a_drop_clears_it() {
    let (mut app, _queue) = app();
    let window = window::Id::unique();
    app.own_window = Some(window);
    app.ready = true;
    app.selected = Some(serde_json::from_value(serde_json::json!("PUBLIC-drop")).unwrap());
    assert!(
        !app.drop_hover,
        "no overlay before a file hovers the window"
    );
    drop(app.update(Message::FileHovered(
        window,
        "/PUBLIC-dropped-file.txt".into(),
    )));
    assert!(app.drop_hover, "a hovered file draws the overlay");
    // A drop consumes the file and clears the overlay.
    drop(app.update(Message::FileDropped(
        window,
        "/PUBLIC-dropped-file.txt".into(),
    )));
    assert!(!app.drop_hover, "the drop clears the overlay");
    // Leaving the window clears it too.
    drop(app.update(Message::FileHovered(window, "/PUBLIC-another.txt".into())));
    assert!(app.drop_hover);
    drop(app.update(Message::FileDropCancelled(window)));
    assert!(!app.drop_hover, "leaving the window clears the overlay");
    // A drag in another window is ignored.
    drop(app.update(Message::FileHovered(
        window::Id::unique(),
        "/PUBLIC-elsewhere.txt".into(),
    )));
    assert!(!app.drop_hover);
}

#[test]
fn the_attach_menu_offers_files_and_commands() {
    let (mut app, mut queue) = app();
    app.ready = true;
    app.selected = Some(serde_json::from_value(serde_json::json!("PUBLIC-attach-menu")).unwrap());
    assert!(!app.attach_menu, "the menu starts closed");
    drop(app.update(Message::ToggleAttachMenu(true)));
    assert!(app.attach_menu);
    // Opening the menu starts no request: the entries do.
    while queue.try_recv().is_ok() {}
    // The command entry puts a slash in the draft, so the completion menu takes over.
    app.commands = Some(vec![crate::ui::command_menu::Candidate::command(
        "compact".into(),
        "PUBLIC".into(),
    )]);
    drop(app.update(Message::AttachMenuCommand));
    assert!(!app.attach_menu, "choosing an entry closes the menu");
    assert_eq!(app.editor.text(), "/");
    assert!(
        app.completion().is_some(),
        "the slash opens the command menu"
    );
    assert!(
        queue.try_recv().is_err(),
        "nothing was requested to open a menu"
    );
}

#[test]
fn queue_steering_is_offered_only_while_the_agent_runs() {
    let (mut app, _queue) = app();
    app.ready = true;
    app.selected = Some(serde_json::from_value(serde_json::json!("PUBLIC-steer-gate")).unwrap());
    // Unknown roster state keeps steering available, matching the Stop policy: unknown is not idle.
    assert!(app.queue_steer_enabled());
    let idle: SessionSummary = serde_json::from_value(serde_json::json!({
        "agentAvailable":true, "sessionId":"PUBLIC-steer-gate", "updatedAt":1000,
        "running":false, "blank":false, "cwd":"/PUBLIC",
        "projections":{"kind":"cached", "asOfSeq":-1, "values":{}}
    }))
    .unwrap();
    app.sessions.insert(idle.session_id.clone(), idle);
    assert!(!app.queue_steer_enabled(), "an idle session cannot steer");
    let running: SessionSummary = serde_json::from_value(serde_json::json!({
        "agentAvailable":true, "sessionId":"PUBLIC-steer-gate", "updatedAt":1000,
        "running":true, "blank":false, "cwd":"/PUBLIC",
        "projections":{"kind":"cached", "asOfSeq":-1, "values":{}}
    }))
    .unwrap();
    app.sessions.insert(running.session_id.clone(), running);
    assert!(app.queue_steer_enabled(), "a running session can steer");
}

#[test]
fn stop_is_drawn_only_while_the_session_runs() {
    let (mut app, _queue) = app();
    app.ready = true;
    app.selected = Some(serde_json::from_value(serde_json::json!("PUBLIC-stop-visible")).unwrap());
    // A missing roster entry is unknown, not idle: Stop stays drawn and reachable.
    assert!(app.stop_visible() && app.stop_allowed());
    // A summary that says the session is not running hides the control, while the roster keeps the
    // rest of the composer as it is.
    let idle: SessionSummary = serde_json::from_value(serde_json::json!({
        "agentAvailable":true, "sessionId":"PUBLIC-stop-visible", "updatedAt":1000,
        "running":false, "blank":false, "cwd":"/PUBLIC",
        "projections":{"kind":"cached", "asOfSeq":-1, "values":{}}
    }))
    .unwrap();
    app.sessions.insert(idle.session_id.clone(), idle);
    assert!(!app.stop_visible(), "a known idle session draws no Stop");
    // ... but the press path is unchanged, so a stale view never blocks a real cancellation.
    assert!(app.stop_allowed());
    let running: SessionSummary = serde_json::from_value(serde_json::json!({
        "agentAvailable":true, "sessionId":"PUBLIC-stop-visible", "updatedAt":1000,
        "running":true, "blank":false, "cwd":"/PUBLIC",
        "projections":{"kind":"cached", "asOfSeq":-1, "values":{}}
    }))
    .unwrap();
    app.sessions.insert(running.session_id.clone(), running);
    assert!(app.stop_visible(), "a running session draws Stop");
}

#[test]
fn cancellation_remains_available_when_summary_running_is_unknown_or_false() {
    let (mut app, mut queue) = app();
    app.ready = true;
    app.selected = Some(serde_json::from_value(serde_json::json!("PUBLIC-selected")).unwrap());
    assert!(app.stop_allowed()); // Missing roster entry is not evidence of idle.
    drop(app.update(Message::ToggleConversationMenu));
    assert!(app.panel_stop_visible() && app.stop_allowed());
    app.cancel_pending = true;
    assert!(app.panel_stop_visible() && !app.stop_allowed());
    let accepted = serde_json::from_value(serde_json::json!({"accepted":true})).unwrap();
    drop(app.receive(Event::Cancel {
        generation: app.generation,
        result: Ok(accepted),
    }));
    assert!(app.panel_stop_visible() && app.stop_allowed()); // ACK before actual Host status.
    assert!(queue.try_recv().is_err());
}
#[test]
fn smoke_stop_is_muted_and_rejects_cancel_at_ui_admission() {
    let (mut app, mut queue) = app();
    app.ready = true;
    app.selected = Some(SessionId::new("PUBLIC-smoke-stop").unwrap());
    app.options.smoke = Some(crate::config::Smoke {
        exit_after: std::time::Duration::from_secs(1),
        report: "/PUBLIC/report".into(),
        screenshot: None,
    });
    assert!(!app.stop_allowed());
    assert_eq!(app.stop_color(), MUTED);
    drop(app.stop_button());
    drop(app.update(Message::Stop));
    assert!(!app.cancel_pending);
    assert!(queue.try_recv().is_err());
    app.cancel_pending = true;
    assert_eq!(app.stop_color(), DANGER); // A genuine admitted cancellation remains visible.
}
#[test]
fn disabled_stop_is_muted_but_normal_cancel_does_not_depend_on_roster_running() {
    let (mut app, mut queue) = app();
    assert_eq!(app.stop_color(), MUTED);
    app.ready = true;
    app.selected = Some(SessionId::new("PUBLIC-normal-stop").unwrap());
    assert!(app.stop_allowed());
    assert_eq!(app.stop_color(), DANGER);
    app.close = Close::Stopping;
    assert!(!app.stop_allowed());
    assert_eq!(app.stop_color(), MUTED);
    assert!(queue.try_recv().is_err());
}
#[test]
fn routine_status_is_an_exact_allowlist_not_an_error_filter() {
    let success = "Live real session; UTC timestamps; no generation started";
    assert!(routine_status(success));
    assert!(routine_status("Workspace opened; no prompt submitted"));
    for status in [
        format!("{success} · warning"),
        format!(" {success}"),
        success.to_lowercase(),
        "Owned Core 0.2.1-alpha.1; authenticated native transport ready".into(),
        "Model catalog loaded; provider failures: timeout".into(),
        "Host did not accept prompt; draft retained".into(),
        "Core warning: unknown outcome".into(),
        "Requesting cancellation…".into(),
        "Reply not acknowledged; request retained".into(),
        "Unknown future notice".into(),
    ] {
        assert!(!routine_status(&status), "must surface: {status}");
    }
}
#[test]
fn stale_more_in_navigation_does_not_consume_done() {
    let (mut app, mut queue) = app();
    drop(app.update(Message::Resized(iced::Size::new(608.0, 448.0))));
    drop(app.update(Message::ToggleSidebar));
    assert!(app.sidebar_sheet);
    drop(app.update(Message::ToggleConversationMenu));
    assert!(!app.conversation_menu);
    drop(app.update(Message::DismissPanel));
    assert!(!app.sidebar_sheet);
    assert!(queue.try_recv().is_err());
}
#[test]
fn covered_information_disclosure_cannot_change_hidden_bits() {
    let (mut app, mut queue) = app();
    drop(app.update(Message::Capabilities));
    drop(app.update(Message::ToggleConversationMenu));
    drop(app.update(Message::ToggleInfoDetails(InfoSection::Diagnostics)));
    assert_eq!(app.info_expanded, 0);
    drop(app.update(Message::Resized(iced::Size::new(608.0, 448.0))));
    drop(app.update(Message::ToggleSidebar));
    assert!(app.sidebar_sheet && app.capabilities);
    drop(app.update(Message::ToggleInfoDetails(InfoSection::Features)));
    assert_eq!(app.info_expanded, 0);
    assert!(queue.try_recv().is_err());
}
#[test]
fn settings_constructor_has_one_semantic_registration_source_guard() {
    // Actual rendered multiplicity/bounds are checked by the feature fixture operation.
    assert_eq!(
        include_str!("ui.rs")
            .matches(".id(\"native-settings-entry\")")
            .count(),
        1
    );
}
#[test]
fn the_window_box_matches_the_reference() {
    // The DMG's window options are 1280x820 with a 520x600 minimum
    // (`PARITY-INVENTORY.md` §Windows); the created window and its floor must both
    // come from those four numbers rather than drifting apart.
    assert_eq!(design::WINDOW_WIDTH, 1280.0);
    assert_eq!(design::WINDOW_HEIGHT, 820.0);
    assert_eq!(design::WINDOW_MIN_WIDTH, 520.0);
    assert_eq!(design::WINDOW_MIN_HEIGHT, 600.0);
    // Stable fragments: rustfmt may wrap the option lines, so the assertion pins
    // the four constants' use rather than one exact line shape.
    let main = include_str!("main.rs");
    assert!(main.contains("design::WINDOW_WIDTH, design::WINDOW_HEIGHT"));
    assert!(main.contains("min_size: Some(Size::new("));
    assert!(main.contains("design::WINDOW_MIN_WIDTH"));
    assert!(main.contains("design::WINDOW_MIN_HEIGHT"));
}
#[test]
fn conversation_column_is_adaptive_and_fits_the_padded_pane() {
    let (mut app, mut queue) = app();
    for (width, height) in [
        (760.0, 560.0),
        (1200.0, 800.0),
        (1980.0, 1319.0),
        (2560.0, 1440.0),
    ] {
        drop(app.update(Message::Resized(iced::Size::new(width, height))));
        let sidebar = app.sidebar_motion.value();
        let pane = width - sidebar;
        let lane = app.chat_lane();
        // The web insets the transcript by `--dsh-composer-side-clearance + 16px`
        // per side inside the scroller and caps it at `--dsh-chat-content-width`.
        let inset = 2.0 * design::TRANSCRIPT_INSET;
        let expected = (pane - inset).min(app.chat_cap());
        assert!(
            (lane - expected).abs() < 0.001,
            "width {width}: lane {lane} != expected {expected}"
        );
        assert!(
            lane <= pane - inset + 0.001,
            "width {width}: lane {lane} must fit the padded pane {pane}"
        );
        assert!(lane <= design::MAX_CHAT + 0.001);
        // The composer card rides the same cap plus its own 32px overhang, so it
        // is exactly `COMPOSER_CARD_OVERHANG` wider than the transcript lane
        // wherever the seat's 16px side padding leaves room.
        let seat = design::COMPOSER_SEAT_PAD.left + design::COMPOSER_SEAT_PAD.right;
        let card = (app.chat_cap() + design::COMPOSER_CARD_OVERHANG).min(pane - seat);
        assert!(
            (card - lane - design::COMPOSER_CARD_OVERHANG).abs() < 0.001,
            "width {width}: card {card} is not the overhang wider than lane {lane}"
        );
        // The rendered view uses the same lane, so its virtual row heights agree.
        drop(app.view());
    }
    assert!(queue.try_recv().is_err());
}
#[test]
fn composer_editor_grows_with_the_draft_up_to_the_web_line_cap() {
    let (mut app, mut queue) = app();
    drop(app.update(Message::Resized(iced::Size::new(1200.0, 800.0))));
    // The composer draws and reserves the body line box plus the input's own
    // chrome (4px top pad + the 8px breath below a single line), so one line is
    // exactly the web's 36px `min-height`.
    let line = design::LINE_BODY;
    let chrome = design::COMPOSER_INPUT_CHROME;
    assert_eq!(line + chrome, design::COMPOSER_INPUT_MIN);
    // The web caps the composer text at 336px, which is fourteen of these lines.
    assert_eq!(14.0 * line, 336.0);
    app.editor = text_editor::Content::with_text("one line");
    assert_eq!(app.composer_editor_height(), (line + chrome).round());
    app.editor = text_editor::Content::with_text("a\nb\nc\nd\ne");
    assert_eq!(app.composer_editor_height(), (5.0 * line + chrome).round());
    // The web's fourteen-line cap holds; a short window keeps a smaller cap.
    app.editor = text_editor::Content::with_text(&"line\n".repeat(40));
    assert_eq!(app.composer_editor_height(), (14.0 * line + chrome).round());
    drop(app.update(Message::Resized(iced::Size::new(760.0, 560.0))));
    assert_eq!(app.composer_editor_height(), (8.0 * line + chrome).round());
    // An empty draft rests on the same floor rather than a taller stack.
    app.editor = text_editor::Content::new();
    assert_eq!(app.composer_editor_height(), design::COMPOSER_INPUT_MIN);
    drop(app.view());
    assert!(queue.try_recv().is_err());
}
#[test]
fn settings_paste_messages_are_inert_outside_a_live_models_draft() {
    let (mut app, mut queue) = app();
    let ticket = app.settings.input_ticket();
    drop(app.update(Message::Settings(crate::settings::Action::Paste { ticket })));
    drop(
        app.update(Message::Settings(crate::settings::Action::Pasted {
            ticket,
            input: crate::settings::pasted_key_input("PUBLIC".into()),
        })),
    );
    assert!(queue.try_recv().is_err());
}
#[test]
fn settings_paste_availability_messages_are_inert_outside_a_live_models_draft() {
    let (mut app, mut queue) = app();
    let ticket = app.settings.input_ticket();
    drop(app.update(Message::Settings(
        crate::settings::Action::PasteUnavailable { ticket },
    )));
    assert!(queue.try_recv().is_err());
}
#[test]
fn paste_composer_text_lands_in_the_draft_only_for_a_visible_composer() {
    let (mut app, mut queue) = app();
    app.ready = true;
    app.root_ready = true;
    app.follow_ready = true;
    app.selected = Some(SessionId::new("fixture").unwrap());
    assert!(app.composer_visible());
    drop(app.update(Message::PasteComposerText(Some("PUBLIC paste".into()))));
    assert_eq!(app.editor.text(), "PUBLIC paste");
    // A composer hidden behind another panel ignores a late paste result.
    app.capabilities = true;
    drop(app.update(Message::PasteComposerText(Some("PUBLIC late".into()))));
    assert_eq!(app.editor.text(), "PUBLIC paste");
    // Empty and missing clipboard text never touch the draft.
    app.capabilities = false;
    drop(app.update(Message::PasteComposerText(None)));
    drop(app.update(Message::PasteComposerText(Some(String::new()))));
    assert_eq!(app.editor.text(), "PUBLIC paste");
    assert!(queue.try_recv().is_err());
}
#[test]
fn the_editor_menu_history_undoes_and_redoes_only_edits_it_saw() {
    let (mut app, _queue) = app();
    app.ready = true;
    app.root_ready = true;
    app.follow_ready = true;
    app.selected = Some(SessionId::new("fixture").unwrap());
    assert!(app.composer_visible());
    // Nothing to undo before an edit, and Undo on an empty history is a no-op.
    drop(app.update(Message::EditorMenuAction(editor_menu::Action::Undo)));
    assert_eq!(app.editor.text(), "");
    assert!(!app.editor_menu);
    // One history step per edit, so Undo restores the text as it was before it.
    drop(app.update(Message::Editor(text_editor::Action::Edit(
        text_editor::Edit::Insert('a'),
    ))));
    drop(app.update(Message::Editor(text_editor::Action::Edit(
        text_editor::Edit::Insert('b'),
    ))));
    assert_eq!(app.editor.text(), "ab");
    drop(app.update(Message::EditorMenuAction(editor_menu::Action::Undo)));
    assert_eq!(app.editor.text(), "a");
    drop(app.update(Message::EditorMenuAction(editor_menu::Action::Undo)));
    assert_eq!(app.editor.text(), "");
    drop(app.update(Message::EditorMenuAction(editor_menu::Action::Redo)));
    assert_eq!(app.editor.text(), "a");
    // Select all marks the draft, so the next typed character replaces it, and a
    // fresh edit drops the redo branch the way an editor's own history does.
    drop(app.update(Message::EditorMenuAction(editor_menu::Action::SelectAll)));
    assert_eq!(app.editor.selection().as_deref(), Some("a"));
    drop(app.update(Message::Editor(text_editor::Action::Edit(
        text_editor::Edit::Insert('c'),
    ))));
    assert_eq!(app.editor.text(), "c");
    assert!(app.editor_redo.is_empty());
    drop(app.update(Message::EditorMenuAction(editor_menu::Action::Redo)));
    assert_eq!(app.editor.text(), "c");
}
#[test]
fn settings_route_facts_project_deepseek_catalog_rows_without_inventing_models() {
    let (mut app, mut queue) = app();
    let model = |provider: &str, id: &str, name: &str| Model {
        provider: provider.into(),
        id: id.into(),
        name: name.into(),
        label: name.into(),
        efforts: Vec::new(),
        default_effort: None,
        service_tiers: Vec::new(),
        verbosity_levels: Vec::new(),
    };
    app.models = vec![
        model("deepseek-official", "deepseek-flash", "DeepSeek-V41-Flash"),
        model("openai-codex", "gpt-6", "GPT-6"),
        model("deepseek-official", "deepseek-v4-pro", "DeepSeek-V4-Pro"),
    ];
    let facts = app.settings_route_facts();
    assert_eq!(facts.models.len(), 2, "only DeepSeek rows are reported");
    assert!(
        facts
            .models
            .iter()
            .any(|(name, id, _)| name == "DeepSeek-V41-Flash" && id == "deepseek-flash")
    );
    assert_eq!(facts.loading, app.catalog_read.pending());
    // The DeepSeek page renders real route facts and dispatches nothing.
    drop(app.update(Message::Settings(crate::settings::Action::Open)));
    drop(app.view());
    assert!(queue.try_recv().is_err());
}

#[cfg(feature = "public-layout-fixture")]
#[test]
fn only_a_running_job_offers_a_stop_and_any_row_opens_it() {
    let (size, _scale) = super::layout_fixture::preset_size("conversation").unwrap();
    let (mut app, _queue) = super::layout_fixture::build(
        super::layout_fixture::test_options(),
        "conversation",
        size,
        None,
    );
    // The fixture hook seeds one running shell job and three others in other states.
    app.set_jobs();
    let statuses: Vec<String> = app.jobs.iter().map(|job| job.status.clone()).collect();
    assert!(statuses.iter().any(|status| status == "running"));
    assert_eq!(
        statuses
            .iter()
            .filter(|status| *status == "running")
            .count(),
        1,
        "exactly one row can be stopped: {statuses:?}"
    );
    // A press on the row's own body toggles it, and a press on another row moves the expansion.
    let other = app
        .jobs
        .iter()
        .find(|job| job.id != "shell-1")
        .map(|job| job.id.clone())
        .expect("a settled row");
    drop(app.update(Message::ToggleJobOutput(other.clone())));
    assert_eq!(app.jobs_expanded.as_deref(), Some(other.as_str()));
    drop(app.update(Message::ToggleJobOutput(other.clone())));
    assert!(app.jobs_expanded.is_none(), "a second press folds it");
}
