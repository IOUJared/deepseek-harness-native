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
