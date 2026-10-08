use super::*;
fn fixture() -> (Fixture, tokio::sync::mpsc::Receiver<worker::Command>) {
    let (handle, feed, commands, _events, _closing) = worker::test_channels();
    let options = config::Options {
        backend: dsh_native_core::RustBackendOptions {
            runtime: PathBuf::from("/NONSTARTED-FIXTURE/apps/cli"),
            expected_version: "0.2.1-alpha.1".into(),
            native_home: PathBuf::from("/NONWRITTEN-FIXTURE/harness"),
            working_directory: PathBuf::from("/tmp"),
            user_home: PathBuf::from("/NONWRITTEN-FIXTURE/user"),
            absolute_node_path: None,
            startup_timeout: Duration::from_secs(15),
            stop_policy: Default::default(),
        },
        scale: 1.0,
        smoke: None,
    };
    (
        Fixture::boot(options, PathBuf::from("/NONWRITTEN-FIXTURE"), handle, feed).0,
        commands,
    )
}
#[test]
fn physical_business_and_editor_actions_never_enter_production_update() {
    let (mut fixture, mut commands) = fixture();
    while commands.try_recv().is_ok() {}
    for message in [
        ui::Message::FileOpen,
        ui::Message::Send,
        ui::Message::LoadModels,
        ui::Message::NewSession,
        ui::Message::Editor(text_editor::Action::Edit(text_editor::Edit::Paste(
            std::sync::Arc::new("UNFORWARDED".into()),
        ))),
        ui::Message::Settings(settings::Action::Open),
    ] {
        let _ = fixture.update(Message::Ui(message));
    }
    assert!(commands.try_recv().is_err());
    assert_eq!(ui::composer_text(&fixture.app), "");
    assert!(!ui::review_open(&fixture.app));
    assert_eq!(ui::counters(&fixture.app), (0, 0, false));
}
#[test]
fn tick_before_real_native_worker_header_and_registry_readiness_is_inert() {
    let (mut fixture, mut commands) = fixture();
    while commands.try_recv().is_ok() {}
    let _ = fixture.tick();
    assert_eq!(fixture.phase, Phase::Startup);
    assert!(commands.try_recv().is_err());
    assert!(fixture.subject.is_none());
    assert!(!ui::send_ready(&fixture.app));
}
#[test]
fn wrong_capture_owner_is_ignored_without_file_write_or_stage() {
    let (mut fixture, mut commands) = fixture();
    while commands.try_recv().is_ok() {}
    let _ = fixture.update(Message::Capture(Capture::Review));
    assert!(commands.try_recv().is_err());
    assert!(fixture.capture.is_none());
    assert_eq!(fixture.report["actualFileStages"], 0);
}
#[test]
fn close_revokes_delayed_capture_and_drops_completed_render_before_any_write() {
    let (mut fixture, mut commands) = fixture();
    while commands.try_recv().is_ok() {}
    fixture.capture = Some(Capture::Review);
    let _ = fixture.stop(Some("unit-only-close"));
    assert!(*fixture.capture_cancel.borrow());
    let image = window::Screenshot::new(vec![25, 23, 36, 255], iced::Size::new(1, 1), 1.0);
    let _ = fixture.update(Message::Screenshot(Capture::Review, image));
    assert_eq!(fixture.phase, Phase::Stopping);
    assert!(fixture.report["captures"].as_array().unwrap().is_empty());
    assert!(commands.try_recv().is_err());
}
#[test]
fn native_window_layout_messages_are_forwarded_without_business_commands() {
    let (mut fixture, mut commands) = fixture();
    while commands.try_recv().is_ok() {}
    let _ = fixture.update(Message::Ui(ui::Message::WindowActive(true)));
    let _ = fixture.update(Message::Ui(ui::Message::Resized(iced::Size::new(
        608.0, 448.0,
    ))));
    assert!(commands.try_recv().is_err());
    assert_eq!(ui::counters(&fixture.app), (0, 0, false));
}
