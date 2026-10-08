//! Developer-only composed UI -> worker -> Core -> isolated default provider test.
//! Fixed PUBLIC dummy input only; no model/catalog/sign-in or installed profile access.
//! Manual widget messages are dropped. This is scripted UI integration, NOT native input proof.
#![allow(dead_code)]
#[path = "../src/codex.rs"]
mod codex;
#[path = "../src/config.rs"]
mod config;
#[path = "../src/design.rs"]
mod design;
#[path = "../src/export_file.rs"]
mod export_file;
#[path = "../src/exporter.rs"]
mod exporter;
#[path = "../src/interactions.rs"]
mod interactions;
#[path = "../src/known.rs"]
mod known;
#[path = "../src/management.rs"]
mod management;
#[path = "../src/plugins.rs"]
mod plugins;
#[path = "../src/reducer.rs"]
mod reducer;
#[path = "../src/settings.rs"]
mod settings;
#[path = "../src/smoke.rs"]
mod smoke;
#[path = "../src/worker.rs"]
mod worker;
// Lexical test-only adapter: real UI source/handlers/view, no production helper or secret getter.
mod ui {
    include!("../src/ui.rs");
    pub(super) fn fixture_tickets(app: &App) -> (crate::settings::Ticket, crate::settings::Ticket) {
        (app.settings.input_ticket(), app.settings.ticket())
    }
}
use iced::widget::{column, container};
use iced::{Element, Length, Subscription, Task, window};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::Duration,
};
const PUBLIC_VALUE: &str = "PUBLIC_NATIVE_UI_WORKER_SAVE_SMOKE_ONLY";
#[derive(Clone)]
struct Options {
    output: PathBuf,
    app: config::Options,
}
fn private_empty(path: &Path, boundary: &Path) -> Result<(), &'static str> {
    let meta = fs::symlink_metadata(path).map_err(|_| "private-directory-required")?;
    let canonical = path
        .canonicalize()
        .map_err(|_| "private-directory-required")?;
    if !path.is_absolute()
        || !meta.is_dir()
        || meta.file_type().is_symlink()
        || meta.permissions().mode() & 0o777 != 0o700
        || !canonical.starts_with(boundary)
        || fs::read_dir(path)
            .map_err(|_| "directory-unavailable")?
            .next()
            .is_some()
    {
        return Err("new-empty-private-directory-required");
    }
    Ok(())
}
fn options() -> Result<Options, &'static str> {
    let mut args = std::env::args().skip(1);
    let mut output = None;
    let mut scale = None;
    while let Some(key) = args.next() {
        let value = args.next().ok_or("missing-value")?;
        match key.as_str() {
            "--output" if output.is_none() => output = Some(PathBuf::from(value)),
            "--scale" if scale.is_none() => {
                scale = Some(value.parse::<f32>().map_err(|_| "invalid-scale")?)
            }
            _ => return Err("invalid-or-duplicate-argument"),
        }
    }
    let output = output.ok_or("output-required")?;
    let scale = scale.unwrap_or(1.0);
    if ![1.0, 1.25].contains(&scale) {
        return Err("invalid-scale");
    }
    let boundary = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("evidence")
        .canonicalize()
        .map_err(|_| "boundary-unavailable")?;
    let meta = fs::symlink_metadata(&output).map_err(|_| "output-unavailable")?;
    let canonical = output.canonicalize().map_err(|_| "output-unavailable")?;
    if !output.is_absolute()
        || !meta.is_dir()
        || meta.file_type().is_symlink()
        || meta.permissions().mode() & 0o777 != 0o700
        || !canonical.starts_with(&boundary)
    {
        return Err("new-private-evidence-output-required");
    }
    let names = [
        "harness",
        "workspace",
        "user",
        "config",
        "cache",
        "data",
        "state",
    ];
    for entry in fs::read_dir(&output).map_err(|_| "output-unavailable")? {
        let entry = entry.map_err(|_| "output-unavailable")?;
        if !names.iter().any(|n| entry.file_name() == *n) {
            return Err("unexpected-output-content");
        }
    }
    for name in names {
        private_empty(&output.join(name), &canonical)?;
    }
    let runtime = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../deepseek-harness-linux/apps/cli")
        .canonicalize()
        .map_err(|_| "explicit-owned-alpha-fork-required")?;
    let values = vec![
        "--runtime".into(),
        runtime.to_string_lossy().into_owned(),
        "--expected-version".into(),
        "0.2.1-alpha.1".into(),
        "--home".into(),
        canonical.join("harness").to_string_lossy().into_owned(),
        "--cwd".into(),
        canonical.join("workspace").to_string_lossy().into_owned(),
        "--user-home".into(),
        canonical.join("user").to_string_lossy().into_owned(),
        "--scale".into(),
        scale.to_string(),
    ];
    let app = config::parse(values)
        .map_err(|_| "fixture-options-invalid")?
        .ok_or("fixture-options-required")?;
    Ok(Options {
        output: canonical,
        app,
    })
}
#[derive(Clone)]
enum Message {
    Ui(ui::Message),
    Capture(window::Id),
    Screenshot(window::Screenshot),
    Saved(bool, [u32; 2]),
    Deadline,
}
impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ComposedFixtureMessage(redacted)")
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Ready,
    BeforeMetadata(settings::Ticket),
    PaintReview,
    Save(settings::Ticket),
    AfterMetadata(settings::Ticket),
    Stopping,
}
struct Fixture {
    options: Options,
    app: ui::App,
    stage: Stage,
    own_window: Option<window::Id>,
    native_wayland: bool,
    report: Value,
    failed: bool,
    completed: bool,
}
impl Fixture {
    fn boot(options: Options, handle: worker::Handle, feed: worker::Feed) -> (Self, Task<Message>) {
        let (app, task) = ui::App::boot(options.app.clone(), handle, feed);
        let deadline = Task::perform(
            async {
                tokio::time::sleep(Duration::from_secs(25)).await;
            },
            |_| Message::Deadline,
        );
        let report = json!({"scope":"scripted actual native UI/worker/Core/default-provider public-value persistence", "pid":std::process::id(),"publicFixtureOnly":true,"scriptedUiMessages":true,"nativeKeyboardPointerInputQualified":false,"liveHostDecisionQualified":false,"realCredentialsUsed":false,"modelPrompts":0,"modelCatalogRequested":false,"browserSignInRequested":false,"manualBusinessMessagesForwarded":false,"applicationScale":options.app.scale,"parentPaintInspectionRequired":true});
        (
            Self {
                options,
                app,
                stage: Stage::Ready,
                own_window: None,
                native_wayland: false,
                report,
                failed: false,
                completed: false,
            },
            Task::batch([task.map(Message::Ui), deadline]),
        )
    }
    fn dispatch(&mut self, action: settings::Action) -> Task<Message> {
        self.app
            .update(ui::Message::Settings(action))
            .map(Message::Ui)
    }
    fn stop(&mut self, error: Option<&'static str>) -> Task<Message> {
        if let Some(error) = error {
            self.failed = true;
            self.report["errorCode"] = json!(error);
        }
        self.stage = Stage::Stopping;
        self.app.update(ui::Message::ConfirmClose).map(Message::Ui)
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Ui(ui::Message::NativeOpened(id)) => {
                self.own_window = Some(id);
                return self
                    .app
                    .update(ui::Message::NativeOpened(id))
                    .map(Message::Ui);
            }
            Message::Ui(message @ ui::Message::NativeInfo { .. }) => {
                if let ui::Message::NativeInfo { backend, .. } = &message {
                    self.native_wayland = *backend == "wayland";
                    self.report["windowBackend"] = json!(backend);
                }
                return self.app.update(message).map(Message::Ui);
            }
            Message::Ui(ui::Message::Close) => {
                return self.stop(Some("manual-close-before-fixture-complete"));
            }
            Message::Ui(ui::Message::Worker(event)) => {
                // Apply the actual production reducer FIRST; all secret actions use its real command path.
                let task = self
                    .app
                    .update(ui::Message::Worker(event.clone()))
                    .map(Message::Ui);
                match event {
                    worker::Event::Ready(_) if self.stage == Stage::Ready => {
                        self.report["realWorkerReady"] = json!(true);
                        let open = self.dispatch(settings::Action::Open);
                        let (_, expected) = ui::fixture_tickets(&self.app);
                        self.stage = Stage::BeforeMetadata(expected);
                        return Task::batch([task, open]);
                    }
                    worker::Event::KeyMetadata { ticket, result }
                        if self.stage == Stage::BeforeMetadata(ticket) =>
                    {
                        let Ok(meta) = result else {
                            return self.stop(Some("initial-metadata-unavailable"));
                        };
                        if meta.logged_in || meta.has_api_key || !meta.writable {
                            return self.stop(Some("new-writable-keyless-provider-required"));
                        }
                        self.report["initialCredentialAbsent"] = json!(true);
                        let (input, _) = ui::fixture_tickets(&self.app);
                        let edit = self.dispatch(settings::Action::Edit {
                            ticket: input,
                            input: settings::Input::new(PUBLIC_VALUE.into()),
                        });
                        let (_, operation) = ui::fixture_tickets(&self.app);
                        let review = self.dispatch(settings::Action::Review(operation));
                        let Some(id) = self.own_window else {
                            return self.stop(Some("own-window-unavailable"));
                        };
                        self.stage = Stage::PaintReview;
                        let capture = Task::perform(
                            async {
                                tokio::time::sleep(Duration::from_millis(1000)).await;
                            },
                            move |_| Message::Capture(id),
                        );
                        return Task::batch([task, edit, review, capture]);
                    }
                    worker::Event::KeySaved { ticket, result }
                        if self.stage == Stage::Save(ticket) =>
                    {
                        if result != settings::SaveResult::Confirmed {
                            return self
                                .stop(Some("save-not-confirmed-outcome-may-be-indeterminate"));
                        }
                        self.report["actualPersistenceAcknowledged"] = json!(true);
                        let (_, operation) = ui::fixture_tickets(&self.app);
                        let refresh = self.dispatch(settings::Action::Refresh(operation));
                        let (_, expected) = ui::fixture_tickets(&self.app);
                        self.stage = Stage::AfterMetadata(expected);
                        return Task::batch([task, refresh]);
                    }
                    worker::Event::KeyMetadata { ticket, result }
                        if self.stage == Stage::AfterMetadata(ticket) =>
                    {
                        let Ok(meta) = result else {
                            return self.stop(Some("post-save-metadata-unavailable"));
                        };
                        if !meta.has_api_key || meta.logged_in {
                            return self.stop(Some("post-save-metadata-invalid"));
                        }
                        self.report["metadataPresenceSeparatelyVerified"] = json!(true);
                        self.report["browserAccountStillSignedOut"] = json!(true);
                        if !private_public_value_file(&self.options.app.backend.native_home) {
                            return self
                                .stop(Some("public-value-private-file-verification-failed"));
                        }
                        self.report["defaultProviderPrivateLocalFile"] = json!(true);
                        self.report["publicValueVerifiedWithoutPrinting"] = json!(true);
                        return Task::batch([task, self.stop(None)]);
                    }
                    worker::Event::Fault(_) => return self.stop(Some("fixed-worker-failure")),
                    worker::Event::NoBackendStarted => {
                        self.failed = true;
                        self.finish(false);
                        return iced::exit();
                    }
                    worker::Event::Stopped(result) => {
                        let clean = match result {
                            Ok(stop) => {
                                self.report["stop"] = json!({"exited":stop.exited,"exitCode":stop.exit_code,"graceful":stop.graceful,"containmentUnknown":stop.containment_unknown,"observedDescendantsRemaining":stop.observed_descendants_remaining});
                                stop.exited
                                    && stop.exit_code == Some(0)
                                    && stop.graceful
                                    && !stop.containment_unknown
                                    && stop.observed_descendants_remaining == 0
                            }
                            Err(_) => false,
                        };
                        let passed = clean
                            && !self.failed
                            && self.native_wayland
                            && self.report["actualPersistenceAcknowledged"] == true
                            && self.report["publicValueVerifiedWithoutPrinting"] == true;
                        self.finish(passed);
                        return Task::batch([task, iced::exit()]);
                    }
                    _ => return task,
                }
            }
            Message::Capture(id)
                if self.stage == Stage::PaintReview && self.own_window == Some(id) =>
            {
                return window::screenshot(id).map(Message::Screenshot);
            }
            Message::Screenshot(screenshot) if self.stage == Stage::PaintReview => {
                let path = self.options.output.join("own-window.png");
                let size = [screenshot.size.width, screenshot.size.height];
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            smoke::save_screenshot(&path, &screenshot).is_ok()
                        })
                        .await
                        .unwrap_or(false)
                    },
                    move |success| Message::Saved(success, size),
                );
            }
            Message::Saved(success, size) if self.stage == Stage::PaintReview => {
                self.report["screenshotSaved"] = json!(success);
                self.report["screenshotPhysicalSize"] = json!(size);
                if !success {
                    return self.stop(Some("own-renderer-capture-failed"));
                }
                let (_, operation) = ui::fixture_tickets(&self.app);
                let confirm = self.dispatch(settings::Action::Confirm(operation));
                let (_, expected) = ui::fixture_tickets(&self.app);
                self.stage = Stage::Save(expected);
                return confirm;
            }
            Message::Deadline if !self.completed => {
                return self.stop(Some("composed-fixture-deadline"));
            }
            _ => {} // User widget/business messages are dropped, including opaque secret edits.
        }
        Task::none()
    }
    fn finish(&mut self, passed: bool) {
        if self.completed {
            return;
        }
        self.completed = true;
        self.report["status"] = json!(if passed { "passed" } else { "failed" });
        if let Ok(file) = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(self.options.output.join("app.json"))
        {
            let _ = serde_json::to_writer_pretty(&file, &self.report);
            let _ = file.sync_all();
        }
    }
    fn view(&self) -> Element<'_, Message> {
        container(column![ui::label("PUBLIC COMPOSED FIXTURE · fixed dummy only · manual actions blocked · no model/sign-in",ui::DANGER),self.app.view().map(Message::Ui)].spacing(12)).padding(16).width(Length::Fill).height(Length::Fill).into()
    }
    fn subscription(&self) -> Subscription<Message> {
        self.app.subscription().map(Message::Ui)
    }
}
fn private_public_value_file(home: &Path) -> bool {
    let path = home.join(".credentials.yaml");
    let Ok(meta) = fs::symlink_metadata(&path) else {
        return false;
    };
    if !meta.is_file()
        || meta.file_type().is_symlink()
        || meta.permissions().mode() & 0o777 != 0o600
        || meta.len() > 65536
    {
        return false;
    }
    let (Ok(file), Ok(home)) = (path.canonicalize(), home.canonicalize()) else {
        return false;
    };
    if file.parent() != Some(home.as_path()) {
        return false;
    }
    use std::io::Read;
    let Ok(file) = fs::File::open(file) else {
        return false;
    };
    let mut bytes = Vec::new();
    file.take(65537).read_to_end(&mut bytes).is_ok()
        && bytes.len() <= 65536
        && bytes
            .windows(PUBLIC_VALUE.len())
            .any(|part| part == PUBLIC_VALUE.as_bytes())
}
fn main() -> iced::Result {
    let options = options().unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2)
    });
    let report = options.output.join("app.json");
    unsafe {
        std::env::set_var("ICED_BACKEND", "wgpu");
    }
    let (handle, feed, owner) =
        worker::start(options.app.backend.clone(), false).unwrap_or_else(|_| {
            eprintln!("Owned fixture worker unavailable");
            std::process::exit(1)
        });
    let result = iced::application(
        move || Fixture::boot(options.clone(), handle.clone(), feed.clone()),
        Fixture::update,
        Fixture::view,
    )
    .title("Harness native composed settings · PUBLIC fixture")
    .window(window::Settings {
        size: iced::Size::new(1200.0, 800.0),
        min_size: Some(iced::Size::new(900.0, 600.0)),
        platform_specific: window::settings::PlatformSpecific {
            application_id: "ai.deepseek.harness.native.settings-composed-fixture".into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .exit_on_close_request(false)
    .settings(iced::Settings {
        default_text_size: iced::Pixels(14.0),
        ..Default::default()
    })
    .default_font(ui::FONT)
    .theme(|_: &Fixture| ui::native_theme())
    .scale_factor(|f: &Fixture| f.options.app.scale)
    .subscription(Fixture::subscription)
    .run();
    drop(owner);
    let passed = fs::read(report)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .is_some_and(|value| value["status"] == "passed");
    if !passed {
        eprintln!("Composed public-value fixture failed or report unavailable");
        std::process::exit(1)
    }
    result
}

#[cfg(test)]
mod composed_tests {
    use super::*;
    fn app_options() -> config::Options {
        config::Options {
            backend: dsh_native_core::RustBackendOptions {
                runtime: PathBuf::from("/EXPLICIT-NONSTARTED-FIXTURE/apps/cli"),
                expected_version: "0.2.1-alpha.1".into(),
                native_home: PathBuf::from("/EXPLICIT-NONSTARTED-FIXTURE/harness"),
                working_directory: PathBuf::from("/tmp"),
                user_home: PathBuf::from("/EXPLICIT-NONSTARTED-FIXTURE/user"),
                absolute_node_path: None,
                startup_timeout: Duration::from_secs(15),
                stop_policy: Default::default(),
            },
            scale: 1.0,
            smoke: None,
        }
    }
    #[test]
    fn manual_actions_cannot_enter_command_path_but_scripted_confirmation_moves_one_secret() {
        let (handle, feed, mut commands, _events, _closing) = worker::test_channels();
        let (mut fixture, _) = Fixture::boot(
            Options {
                output: PathBuf::from("/NONWRITTEN-TEST-OUTPUT"),
                app: app_options(),
            },
            handle,
            feed,
        );
        fixture.own_window = Some(window::Id::unique());
        let _ = fixture.update(Message::Ui(ui::Message::Worker(worker::Event::Ready(
            "0.2.1-alpha.1".into(),
        ))));
        let command = commands.try_recv().expect("one explicit settings read");
        let worker::Command::KeyMetadata(ticket) = command else {
            panic!("expected metadata-only command")
        };
        let _ = fixture.update(Message::Ui(ui::Message::Worker(
            worker::Event::KeyMetadata {
                ticket,
                result: Ok(dsh_native_core::OnboardingMetadata {
                    logged_in: false,
                    has_api_key: false,
                    writable: true,
                }),
            },
        )));
        assert!(fixture.stage == Stage::PaintReview);
        let before = ui::fixture_tickets(&fixture.app);
        let _ = fixture.update(Message::Ui(ui::Message::Settings(settings::Action::Edit {
            ticket: before.0,
            input: settings::Input::new("PUBLIC_MANUAL_ACTION_MUST_DROP".into()),
        })));
        let _ = fixture.update(Message::Ui(ui::Message::Settings(
            settings::Action::Confirm(before.1),
        )));
        let _ = fixture.update(Message::Ui(ui::Message::LoadModels));
        assert!(ui::fixture_tickets(&fixture.app) == before);
        assert!(commands.try_recv().is_err());
        let _ = fixture.update(Message::Saved(true, [100, 100]));
        let worker::Command::SaveKey { ticket, secret } =
            commands.try_recv().expect("one actual UI save command")
        else {
            panic!("expected moved secret command")
        };
        assert!(fixture.stage == Stage::Save(ticket));
        assert!(!format!("{secret:?}").contains(PUBLIC_VALUE));
        drop(secret); // Not sent to Core in this pure unit test.
        assert!(commands.try_recv().is_err());
    }
    #[test]
    fn wrong_metadata_ticket_cannot_advance_closed_fixture_stage() {
        let (handle, feed, mut commands, _events, _closing) = worker::test_channels();
        let (mut fixture, _) = Fixture::boot(
            Options {
                output: PathBuf::from("/NONWRITTEN-TEST-OUTPUT"),
                app: app_options(),
            },
            handle,
            feed,
        );
        let _ = fixture.update(Message::Ui(ui::Message::Worker(worker::Event::Ready(
            "0.2.1-alpha.1".into(),
        ))));
        let worker::Command::KeyMetadata(expected) = commands.try_recv().unwrap() else {
            panic!("metadata command required")
        };
        let wrong = settings::Ticket {
            serial: expected.serial + 1,
            ..expected
        };
        let _ = fixture.update(Message::Ui(ui::Message::Worker(
            worker::Event::KeyMetadata {
                ticket: wrong,
                result: Ok(dsh_native_core::OnboardingMetadata {
                    logged_in: false,
                    has_api_key: false,
                    writable: true,
                }),
            },
        )));
        assert!(fixture.stage == Stage::BeforeMetadata(expected));
        assert!(fixture.report.get("initialCredentialAbsent").is_none());
        assert!(commands.try_recv().is_err());
    }
    fn directory() -> PathBuf {
        use std::os::unix::fs::DirBuilderExt;
        let path = std::env::temp_dir().join(format!(
            "dsh-public-composed-file-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        path
    }
    fn public_file(home: &Path, bytes: &[u8], mode: u32) {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(home.join(".credentials.yaml"))
            .unwrap();
        file.write_all(bytes).unwrap();
        // Test the requested mode even under the harness's restrictive ambient umask.
        file.set_permissions(fs::Permissions::from_mode(mode))
            .unwrap();
    }
    #[test]
    fn persisted_public_value_check_is_bounded_private_and_never_follows_symlink() {
        let good = directory();
        public_file(&good, PUBLIC_VALUE.as_bytes(), 0o600);
        assert!(private_public_value_file(&good));
        let broad = directory();
        public_file(&broad, PUBLIC_VALUE.as_bytes(), 0o644);
        assert!(!private_public_value_file(&broad));
        let link = directory();
        std::os::unix::fs::symlink(
            good.join(".credentials.yaml"),
            link.join(".credentials.yaml"),
        )
        .unwrap();
        assert!(!private_public_value_file(&link));
        let large = directory();
        public_file(&large, &vec![b'X'; 65537], 0o600);
        assert!(!private_public_value_file(&large));
        // Fresh private PUBLIC-only test homes intentionally retained; never remove arbitrary computed paths.
    }
}
