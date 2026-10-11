//! Owned PUBLIC native App -> worker -> Core -> unchanged full-alpha preference storage.
//! No fabricated receipts, OAuth/browser/route activation, catalog or model calls.
//! Accounts scope only selects/deletes new owned PUBLIC synthetic stored records.
//! The private adapter reserves an UNSENT catalog flight before real Ready solely to
//! suppress unrelated startup catalog I/O. Settings uses its normal non-smoke guards.
//! Scripted widget actions, not physical input; only our own renderer is captured.
#![allow(dead_code)]
#[path = "support/accounts_composed.rs"]
mod accounts_composed;
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
#[path = "../src/fork.rs"]
mod fork;
#[path = "../src/interactions.rs"]
mod interactions;
#[path = "../src/known.rs"]
mod known;
#[path = "../src/management.rs"]
mod management;
#[path = "../src/mcp.rs"]
mod mcp;
#[path = "../src/plugins.rs"]
mod plugins;
#[path = "../src/reducer.rs"]
mod reducer;
#[path = "../src/rename.rs"]
mod rename;
#[path = "support/runtime_composed.rs"]
mod runtime_composed;
#[path = "../src/secret_input.rs"]
mod secret_input;
#[path = "../src/settings.rs"]
mod settings;
#[path = "../src/smoke.rs"]
mod smoke;
#[path = "../src/terminal.rs"]
mod terminal;
#[path = "../src/work_panel.rs"]
mod work_panel;
#[path = "../src/worker.rs"]
mod worker;
mod ui {
    include!("../src/ui.rs");
    pub(super) fn reserve_unsent_catalog(app: &mut App) -> bool {
        // No fake successful catalog, no command interception or fabricated Ready.
        app.transport_epoch = crate::worker::TRANSPORT_EPOCH;
        app.catalog_read.begin(app.transport_epoch).is_some()
    }
    pub(super) fn usage_ticket(app: &App) -> crate::codex::Ticket {
        app.settings.codex_ticket()
    }
    pub(super) fn no_model_work(app: &App) -> bool {
        !app.smoke_evidence.catalog_requested && app.smoke_evidence.model_prompts == 0
    }
}
use dsh_native_core::{
    CodexComposerDisplay as Composer, CodexRemainingPercent, CodexUsageAlert as Alert,
    CodexUsagePreferences as Preferences, CodexUsageSnapshot as Snapshot,
};
use iced::{
    Element, Subscription, Task,
    widget::{column, container},
    window,
};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::Duration,
};
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Initial,
    Restored,
    Inherited,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    Usage,
    Runtime,
    Accounts,
}
impl Scope {
    fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "usage" => Ok(Self::Usage),
            "runtime" => Ok(Self::Runtime),
            "accounts" => Ok(Self::Accounts),
            _ => Err("invalid-scope"),
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Usage => "usage",
            Self::Runtime => "runtime",
            Self::Accounts => "accounts",
        }
    }
}
impl Phase {
    fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "initial" => Ok(Self::Initial),
            "restored" => Ok(Self::Restored),
            "inherited" => Ok(Self::Inherited),
            _ => Err("invalid-phase"),
        }
    }
}
#[derive(Clone)]
struct Options {
    output: PathBuf,
    phase: Phase,
    scope: Scope,
    app: config::Options,
}
fn private_directory(path: &Path, boundary: &Path, empty: bool) -> Result<PathBuf, &'static str> {
    let meta = fs::symlink_metadata(path).map_err(|_| "private-directory-required")?;
    let canonical = path
        .canonicalize()
        .map_err(|_| "private-directory-required")?;
    if !path.is_absolute()
        || canonical != path
        || !canonical.starts_with(boundary)
        || meta.file_type().is_symlink()
        || !meta.is_dir()
        || meta.permissions().mode() & 0o777 != 0o700
        || meta.uid()
            != fs::metadata(boundary)
                .map_err(|_| "boundary-unavailable")?
                .uid()
        || empty
            && fs::read_dir(path)
                .map_err(|_| "directory-unavailable")?
                .next()
                .is_some()
    {
        return Err("owned-private-directory-required");
    }
    Ok(canonical)
}
fn parse_options(values: impl IntoIterator<Item = String>) -> Result<Options, &'static str> {
    let mut fields = std::collections::BTreeMap::new();
    let mut values = values.into_iter();
    while let Some(key) = values.next() {
        if ![
            "--output",
            "--state-root",
            "--runtime",
            "--phase",
            "--scope",
        ]
        .contains(&key.as_str())
        {
            return Err("invalid-argument");
        }
        let value = values.next().ok_or("missing-value")?;
        if fields.insert(key, value).is_some() {
            return Err("duplicate-argument");
        }
    }
    let scope = Scope::parse(&fields.remove("--scope").unwrap_or_else(|| "usage".into()))?;
    let mut take = |key| fields.remove(key).ok_or("missing-required-argument");
    let output = PathBuf::from(take("--output")?);
    let root = PathBuf::from(take("--state-root")?);
    let runtime = take("--runtime")?;
    let phase = Phase::parse(&take("--phase")?)?;
    if scope == Scope::Usage && phase == Phase::Inherited {
        return Err("usage-inherited-phase-unavailable");
    }
    let boundary = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("evidence")
        .canonicalize()
        .map_err(|_| "evidence-required")?;
    let root = private_directory(&root, &boundary, false)?;
    let output = private_directory(&output, &root, true)?;
    if output == root {
        return Err("new-phase-output-required");
    }
    for name in [
        "harness",
        "workspace",
        "user",
        "config",
        "cache",
        "data",
        "state",
    ] {
        private_directory(&root.join(name), &root, phase == Phase::Initial)?;
    }
    let app = config::parse(vec![
        "--runtime".into(),
        runtime,
        "--expected-version".into(),
        "0.2.1-alpha.1".into(),
        "--home".into(),
        root.join("harness").to_string_lossy().into_owned(),
        "--cwd".into(),
        root.join("workspace").to_string_lossy().into_owned(),
        "--user-home".into(),
        root.join("user").to_string_lossy().into_owned(),
        "--node".into(),
        "/usr/bin/node".into(),
    ])
    .map_err(|_| "invalid-explicit-runtime-options")?
    .ok_or("runtime-required")?;
    Ok(Options {
        output,
        phase,
        scope,
        app,
    })
}
fn preferences(off: bool) -> Preferences {
    Preferences {
        composer_display: if off {
            Composer::Off
        } else {
            Composer::ProgressBar
        },
        usage_alert: if off { Alert::Off } else { Alert::Custom },
        custom_remaining_percent: CodexRemainingPercent::new(17.5).expect("fixed PUBLIC scalar"),
        retry_after_reset: !off,
        experimental_quota: !off,
    }
}
fn matches_snapshot(value: &Snapshot, off: bool) -> bool {
    value.version == 1
        && value.persisted
        && value.revision == if off { 5 } else { 4 }
        && value.preferences == preferences(off)
}
fn seed(options: &Options) -> Result<(), &'static str> {
    if options.phase != Phase::Initial {
        return Ok(());
    }
    if options.scope == Scope::Runtime {
        return runtime_composed::seed(options);
    }
    if options.scope == Scope::Accounts {
        return accounts_composed::seed(options);
    }
    let directory = options.app.backend.native_home.join("codex-usage");
    fs::create_dir(&directory).map_err(|_| "new-public-seed-directory-required")?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
        .map_err(|_| "seed-permission-failed")?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(directory.join("usage.v1.json"))
        .map_err(|_| "new-public-seed-required")?;
    // Fixed legacy PUBLIC record only; production Core owns all wire parsing/encoding.
    let bytes = serde_json::to_vec(&json!({"version":1,"revision":4,"preferences":{
        "composerDisplay":"progressBar","usageAlert":"custom","customRemainingPercent":17.5,
        "retryAfterReset":true,"experimentalQuota":true}}))
    .map_err(|_| "seed-encode-failed")?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "seed-write-failed")?;
    fs::File::open(directory)
        .and_then(|f| f.sync_all())
        .map_err(|_| "seed-directory-sync-failed")
}
#[derive(Clone)]
enum Message {
    Ui(ui::Message),
    Capture(window::Id),
    Screenshot(window::Screenshot),
    Saved(bool),
    ConsentCapture {
        id: window::Id,
        ticket: codex::Ticket,
        purpose: accounts_composed::ReviewCapture,
    },
    ConsentScreenshot {
        ticket: codex::Ticket,
        purpose: accounts_composed::ReviewCapture,
        image: window::Screenshot,
    },
    ConsentSaved {
        ticket: codex::Ticket,
        purpose: accounts_composed::ReviewCapture,
        saved: bool,
    },
    Deadline,
}
impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PublicUsageFixtureMessage(redacted)")
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Ready,
    Read(codex::Ticket),
    Save(codex::Ticket),
    Readback(codex::Ticket),
    Paint,
    Consent(codex::Ticket, accounts_composed::ReviewCapture),
    Stopping,
}
struct Fixture {
    options: Options,
    app: ui::App,
    stage: Stage,
    window: Option<window::Id>,
    paint_index: usize,
    report: Value,
    failed: bool,
    finished: bool,
    runtime_case: runtime_composed::Case,
    accounts_case: accounts_composed::Case,
}
impl Fixture {
    fn boot(options: Options, handle: worker::Handle, feed: worker::Feed) -> (Self, Task<Message>) {
        let (mut app, task) = ui::App::boot(options.app.clone(), handle, feed);
        let reserved = ui::reserve_unsent_catalog(&mut app);
        // Open real Settings BEFORE Ready so default API login cannot schedule a metadata read.
        let open = app.update(ui::Message::Settings(settings::Action::Open));
        let page = app.update(ui::Message::Settings(settings::Action::SelectPage(
            settings::Page::Codex,
        )));
        let tab = if options.scope == Scope::Runtime {
            app.update(ui::Message::Settings(settings::Action::Codex(
                codex::Action::SelectTab {
                    ticket: ui::usage_ticket(&app),
                    tab: codex::Tab::Models,
                },
            )))
        } else {
            Task::none()
        };
        let deadline = Task::perform(
            async { tokio::time::sleep(Duration::from_secs(25)).await },
            |_| Message::Deadline,
        );
        let report = json!({"scope":match options.scope { Scope::Runtime => "actual native App/worker/Core full-alpha SDK runtime settings persistence", Scope::Usage => "actual native App/worker/Core full-alpha local usage opt-out persistence", Scope::Accounts => "actual native App/worker/Core full-alpha PUBLIC synthetic local account lifecycle and Maintenance" },
            "phase":match options.phase {Phase::Initial=>"initial",Phase::Restored=>"restored",Phase::Inherited=>"inherited"},"configurationScope":options.scope.name(),"pid":std::process::id(),"publicFixtureOnly":true,
            "scriptedUiMessages":true,"manualBusinessMessagesForwarded":false,"startupCatalogSuppressedByPrivateUnsentFlight":reserved,
            "fakeReadyOrReceiptsUsed":false,"realCredentialsUsed":false,"providerRequestsIssued":false,"modelPrompts":0,"modelCatalogRequested":false,
            "accountOrAuthorizationCommands":0,"nativeKeyboardPointerInputQualified":false,"liveQuotaQualified":false,"parentPaintInspectionRequired":true});
        (
            Self {
                options,
                app,
                stage: Stage::Ready,
                window: None,
                paint_index: 0,
                report,
                failed: !reserved,
                finished: false,
                runtime_case: runtime_composed::Case::default(),
                accounts_case: accounts_composed::Case::default(),
            },
            Task::batch([
                task.map(Message::Ui),
                open.map(Message::Ui),
                page.map(Message::Ui),
                tab.map(Message::Ui),
                deadline,
            ]),
        )
    }
    fn usage(&mut self, action: codex::UsageAction) -> Task<Message> {
        let ticket = ui::usage_ticket(&self.app);
        self.app
            .update(ui::Message::Settings(settings::Action::Codex(
                codex::Action::Usage { ticket, action },
            )))
            .map(Message::Ui)
    }
    fn runtime(&mut self, action: codex::RuntimeAction) -> Task<Message> {
        let ticket = ui::usage_ticket(&self.app);
        self.app
            .update(ui::Message::Settings(settings::Action::Codex(
                codex::Action::Runtime { ticket, action },
            )))
            .map(Message::Ui)
    }
    fn account(&mut self, action: codex::Action) -> Task<Message> {
        self.app
            .update(ui::Message::Settings(settings::Action::Codex(action)))
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
                self.window = Some(id);
                return self
                    .app
                    .update(ui::Message::NativeOpened(id))
                    .map(Message::Ui);
            }
            Message::Ui(message @ ui::Message::NativeInfo { .. }) => {
                if let ui::Message::NativeInfo { backend, .. } = &message {
                    self.report["windowBackend"] = json!(backend);
                }
                return self.app.update(message).map(Message::Ui);
            }
            // Real startup draft reads return through Iced, not the worker.
            // Forward owned completion so closing cannot wait on a discarded receipt.
            Message::Ui(message @ ui::Message::DraftOutcome(_)) => {
                return self.app.update(message).map(Message::Ui);
            }
            Message::Ui(ui::Message::Worker(event)) => {
                // Actual production App consumes the real receipt BEFORE the oracle observes it.
                let task = self
                    .app
                    .update(ui::Message::Worker(event.clone()))
                    .map(Message::Ui);
                if !ui::no_model_work(&self.app) {
                    return self.stop(Some("unexpected-model-or-catalog-admission"));
                }
                match event {
                    worker::Event::Ready(version) if self.stage == Stage::Ready => {
                        if version != "0.2.1-alpha.1" {
                            return self.stop(Some("wrong-alpha-version"));
                        }
                        self.report["realWorkerReady"] = json!(true);
                        let refresh = match self.options.scope {
                            Scope::Runtime => self.runtime(codex::RuntimeAction::Refresh),
                            Scope::Usage => self.usage(codex::UsageAction::Refresh),
                            Scope::Accounts => self.account(codex::Action::RefreshAccounts(
                                ui::usage_ticket(&self.app),
                            )),
                        };
                        self.stage = Stage::Read(ui::usage_ticket(&self.app));
                        return Task::batch([task, refresh]);
                    }
                    worker::Event::CodexCompleted { ticket, outcome } => {
                        if self.options.scope == Scope::Runtime {
                            return runtime_composed::completed(self, ticket, outcome, task);
                        }
                        if self.options.scope == Scope::Accounts {
                            return accounts_composed::completed(self, ticket, outcome, task);
                        }
                        if self.stage == Stage::Read(ticket) {
                            let codex::Outcome::UsageSnapshot(value) = outcome else {
                                return self.stop(Some("initial-preferences-unavailable"));
                            };
                            let restored = self.options.phase == Phase::Restored;
                            if !matches_snapshot(&value, restored) {
                                return self.stop(Some("wrong-owned-initial-snapshot"));
                            }
                            self.report["initialOwnedSnapshotMatched"] = json!(true);
                            if restored {
                                return Task::batch([task, self.paint()]);
                            }
                            // Unsupported enabling/threshold actions and their Save cannot change the ticket.
                            let before = ui::usage_ticket(&self.app);
                            let mut tasks = vec![task];
                            for action in [
                                codex::UsageAction::Composer(Composer::Percent),
                                codex::UsageAction::Alert(Alert::EarlyWarning),
                                codex::UsageAction::Retry(true),
                                codex::UsageAction::Consent(true),
                                codex::UsageAction::Threshold("1".into()),
                                codex::UsageAction::Save,
                            ] {
                                tasks.push(self.usage(action));
                            }
                            if before != ui::usage_ticket(&self.app) {
                                return self.stop(Some("unsupported-native-edit-was-admitted"));
                            }
                            self.report["unsupportedActionsRejected"] = json!(true);
                            for action in [
                                codex::UsageAction::Composer(Composer::Off),
                                codex::UsageAction::Alert(Alert::Off),
                                codex::UsageAction::Retry(false),
                                codex::UsageAction::Consent(false),
                            ] {
                                tasks.push(self.usage(action));
                            }
                            tasks.push(self.usage(codex::UsageAction::Save));
                            self.stage = Stage::Save(ui::usage_ticket(&self.app));
                            return Task::batch(tasks);
                        }
                        if self.stage == Stage::Save(ticket) {
                            let codex::Outcome::UsageSnapshot(value) = outcome else {
                                return self.stop(Some("real-save-not-acknowledged"));
                            };
                            if !matches_snapshot(&value, true) {
                                return self.stop(Some("wrong-persisted-save-ack"));
                            }
                            self.report["actualPersistenceAcknowledged"] = json!(true);
                            let refresh = self.usage(codex::UsageAction::Refresh);
                            self.stage = Stage::Readback(ui::usage_ticket(&self.app));
                            return Task::batch([task, refresh]);
                        }
                        if self.stage == Stage::Readback(ticket) {
                            let codex::Outcome::UsageSnapshot(value) = outcome else {
                                return self.stop(Some("real-readback-unavailable"));
                            };
                            if !matches_snapshot(&value, true) {
                                return self.stop(Some("wrong-committed-readback"));
                            }
                            self.report["sameProcessReadbackMatched"] = json!(true);
                            return Task::batch([task, self.paint()]);
                        }
                        return self.stop(Some("unexpected-preference-receipt"));
                    }
                    worker::Event::Catalog { .. }
                    | worker::Event::Model { .. }
                    | worker::Event::Prompt { .. }
                    | worker::Event::KeyMetadata { .. }
                    | worker::Event::KeySaved { .. } => {
                        return self.stop(Some("unexpected-nonpreference-command"));
                    }
                    worker::Event::Fault(_) => return self.stop(Some("owned-worker-failed")),
                    worker::Event::NoBackendStarted => {
                        self.failed = true;
                        self.report["errorCode"] = json!("owned-backend-not-started");
                        self.finish(false);
                        return iced::exit();
                    }
                    worker::Event::Stopped(result) => {
                        let clean = match result {
                            Ok(stop) => {
                                self.report["stop"] = json!({"exited":stop.exited,"graceful":stop.graceful,"exitCode":stop.exit_code,"containmentUnknown":stop.containment_unknown,"observedDescendantsRemaining":stop.observed_descendants_remaining});
                                stop.exited
                                    && stop.graceful
                                    && stop.exit_code == Some(0)
                                    && !stop.containment_unknown
                                    && stop.observed_descendants_remaining == 0
                            }
                            Err(_) => false,
                        };
                        let initial = self.options.phase == Phase::Initial;
                        let passed = clean
                            && !self.failed
                            && self.report["windowBackend"] == "wayland"
                            && self.report["screenshotSaved"] == true
                            && self.report["initialOwnedSnapshotMatched"] == true
                            && (!initial
                                || self.report["actualPersistenceAcknowledged"] == true
                                    && self.report["sameProcessReadbackMatched"] == true);
                        self.finish(passed);
                        return Task::batch([task, iced::exit()]);
                    }
                    _ => return task,
                }
            }
            Message::ConsentCapture {
                id,
                ticket,
                purpose,
            } => {
                return accounts_composed::capture(self, id, ticket, purpose);
            }
            Message::ConsentScreenshot {
                ticket,
                purpose,
                image,
            } => {
                return accounts_composed::screenshot(self, ticket, purpose, image);
            }
            Message::ConsentSaved {
                ticket,
                purpose,
                saved,
            } => {
                return accounts_composed::saved(self, ticket, purpose, saved);
            }
            Message::Capture(id) if self.stage == Stage::Paint && self.window == Some(id) => {
                return window::screenshot(id).map(Message::Screenshot);
            }
            Message::Screenshot(image) if self.stage == Stage::Paint => {
                let path = self.options.output.join(if self.paint_index == 0 {
                    "own-window.png"
                } else if self.options.scope == Scope::Accounts {
                    "own-maintenance.png"
                } else {
                    "own-images.png"
                });
                self.report["screenshotPhysicalSize"] =
                    json!([image.size.width, image.size.height]);
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            smoke::save_screenshot(&path, &image).is_ok()
                        })
                        .await
                        .unwrap_or(false)
                    },
                    Message::Saved,
                );
            }
            Message::Saved(ok) if self.stage == Stage::Paint => {
                self.report["screenshotSaved"] = json!(ok);
                if ok && self.options.scope == Scope::Accounts && self.paint_index == 0 {
                    self.report["accountScreenshotSaved"] = json!(true);
                    self.paint_index = 1;
                    return accounts_composed::maintenance(self);
                }
                if ok && self.options.scope == Scope::Runtime && self.paint_index == 0 {
                    self.report["modelsScreenshotSaved"] = json!(true);
                    self.paint_index = 1;
                    // Switch only after the actual read/save receipt has settled in App.
                    let tab = self
                        .app
                        .update(ui::Message::Settings(settings::Action::Codex(
                            codex::Action::SelectTab {
                                ticket: ui::usage_ticket(&self.app),
                                tab: codex::Tab::Images,
                            },
                        )))
                        .map(Message::Ui);
                    return Task::batch([tab, self.paint()]);
                }
                return self.stop((!ok).then_some("own-renderer-capture-failed"));
            }
            Message::Deadline if !self.finished => {
                return self.stop(Some("composed-fixture-deadline"));
            }
            Message::Ui(ui::Message::Close) => {
                return self.stop(Some("manual-close-before-completion"));
            }
            _ => {} // Manual widget/business messages never forwarded.
        }
        Task::none()
    }
    fn paint(&mut self) -> Task<Message> {
        self.stage = Stage::Paint;
        let Some(id) = self.window else {
            return self.stop(Some("own-window-unavailable"));
        };
        Task::batch([
            iced::widget::operation::snap_to(
                "native-settings-body",
                if self.options.scope == Scope::Accounts {
                    iced::widget::scrollable::RelativeOffset::START
                } else {
                    iced::widget::scrollable::RelativeOffset::END
                },
            ),
            Task::perform(
                async move {
                    tokio::time::sleep(Duration::from_millis(400)).await;
                    id
                },
                Message::Capture,
            ),
        ])
    }
    fn finish(&mut self, passed: bool) {
        if self.finished {
            return;
        }
        self.finished = true;
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
        container(
            column![
                ui::label(
                    "PUBLIC COMPOSED LOCAL CONFIG · keyless profile · manual actions blocked",
                    ui::DANGER
                ),
                self.app.view().map(Message::Ui)
            ]
            .spacing(12),
        )
        .padding(16)
        .into()
    }
    fn subscription(&self) -> Subscription<Message> {
        self.app.subscription().map(Message::Ui)
    }
}
fn main() -> iced::Result {
    let options = parse_options(std::env::args().skip(1)).unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2)
    });
    seed(&options).unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2)
    });
    let report = options.output.join("app.json");
    // SAFETY: no threads have been started by this fixture yet.
    unsafe {
        std::env::set_var("ICED_BACKEND", "wgpu");
    }
    let (handle, feed, owner) =
        worker::start(options.app.backend.clone(), false).unwrap_or_else(|_| {
            eprintln!("owned-worker-unavailable");
            std::process::exit(1)
        });
    let result = iced::application(
        move || Fixture::boot(options.clone(), handle.clone(), feed.clone()),
        Fixture::update,
        Fixture::view,
    )
    .title("Harness native usage opt-out · PUBLIC composition")
    .window(window::Settings {
        size: iced::Size::new(1200.0, 900.0),
        platform_specific: window::settings::PlatformSpecific {
            application_id: "ai.deepseek.harness.native.usage-composed-fixture".into(),
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
    .subscription(Fixture::subscription)
    .run();
    drop(owner);
    if !fs::read(report)
        .ok()
        .and_then(|v| serde_json::from_slice::<Value>(&v).ok())
        .is_some_and(|v| v["status"] == "passed")
    {
        eprintln!("composed-optout-fixture-failed");
        std::process::exit(1)
    }
    result
}
#[cfg(test)]
mod fixture_tests {
    use super::*;
    pub(super) fn unstarted_options() -> Options {
        Options {
            output: "/PUBLIC_NOT_STARTED/report".into(),
            phase: Phase::Initial,
            scope: Scope::Usage,
            app: config::Options {
                backend: dsh_native_core::RustBackendOptions {
                    runtime: "/PUBLIC_NOT_STARTED/apps/cli".into(),
                    expected_version: "0.2.1-alpha.1".into(),
                    native_home: "/PUBLIC_NOT_STARTED/harness".into(),
                    user_home: "/PUBLIC_NOT_STARTED/user".into(),
                    working_directory: "/PUBLIC_NOT_STARTED/workspace".into(),
                    absolute_node_path: Some("/usr/bin/node".into()),
                    startup_timeout: Duration::from_secs(10),
                    stop_policy: Default::default(),
                },
                scale: 1.0,
                // A fixture renders a public frame; it never announces itself.
                accessibility: false,
                smoke: None,
            },
        }
    }
    #[test]
    fn private_pre_ready_adapter_dispatches_only_preference_read() {
        let (handle, feed, mut commands, _events, _closing) = worker::test_channels();
        let (mut fixture, _tasks) = Fixture::boot(unstarted_options(), handle, feed);
        assert!(commands.try_recv().is_err());
        let _ = fixture.update(Message::Ui(ui::Message::Worker(worker::Event::Ready(
            "0.2.1-alpha.1".into(),
        ))));
        let worker::Command::Codex(codex::Effect::UsageRead { ticket }) =
            commands.try_recv().expect("real App usage read")
        else {
            panic!("unexpected account/catalog command")
        };
        assert!(fixture.stage == Stage::Read(ticket));
        assert!(commands.try_recv().is_err());
        assert!(ui::no_model_work(&fixture.app));
    }
    #[test]
    fn runtime_pre_ready_adapter_dispatches_only_real_app_runtime_read() {
        let (handle, feed, mut commands, _events, _closing) = worker::test_channels();
        let mut options = unstarted_options();
        options.scope = Scope::Runtime;
        let (mut fixture, _tasks) = Fixture::boot(options, handle, feed);
        assert!(commands.try_recv().is_err());
        let _ = fixture.update(Message::Ui(ui::Message::Worker(worker::Event::Ready(
            "0.2.1-alpha.1".into(),
        ))));
        let worker::Command::Codex(codex::Effect::RuntimeRead { ticket }) =
            commands.try_recv().expect("real App runtime read")
        else {
            panic!("unexpected account/catalog command")
        };
        assert!(fixture.stage == Stage::Read(ticket));
        assert!(commands.try_recv().is_err());
        assert!(ui::no_model_work(&fixture.app));
    }
    #[test]
    fn accounts_pre_ready_adapter_dispatches_only_actual_accounts_read() {
        let (handle, feed, mut commands, _events, _closing) = worker::test_channels();
        let mut options = unstarted_options();
        options.scope = Scope::Accounts;
        let (mut fixture, _tasks) = Fixture::boot(options, handle, feed);
        assert!(commands.try_recv().is_err());
        let _ = fixture.update(Message::Ui(ui::Message::Worker(worker::Event::Ready(
            "0.2.1-alpha.1".into(),
        ))));
        let worker::Command::Codex(codex::Effect::AccountsRead { ticket }) =
            commands.try_recv().expect("real App accounts read")
        else {
            panic!("unexpected OAuth/catalog command")
        };
        assert!(fixture.stage == Stage::Read(ticket));
        assert!(commands.try_recv().is_err());
        assert!(ui::no_model_work(&fixture.app));
    }
    #[test]
    fn scopes_are_closed_with_usage_default_compatibility() {
        assert!(Scope::parse("usage") == Ok(Scope::Usage));
        assert!(Scope::parse("runtime") == Ok(Scope::Runtime));
        for value in ["", "Runtime", "Accounts", "live"] {
            assert!(Scope::parse(value).is_err());
        }
    }
    #[test]
    fn phase_is_closed() {
        assert!(Phase::parse("initial").is_ok());
        assert!(Phase::parse("restored").is_ok());
        for v in ["", "live", "corrupt", "INITIAL"] {
            assert!(Phase::parse(v).is_err());
        }
    }
    #[test]
    fn snapshot_oracle_requires_exact_persisted_fraction_and_revision() {
        let good = Snapshot {
            version: 1,
            revision: 5,
            persisted: true,
            preferences: preferences(true),
        };
        assert!(matches_snapshot(&good, true));
        assert!(!matches_snapshot(&good, false));
        for bad in [
            Snapshot {
                persisted: false,
                ..good.clone()
            },
            Snapshot {
                revision: 6,
                ..good.clone()
            },
            Snapshot {
                version: 2,
                ..good.clone()
            },
            Snapshot {
                preferences: preferences(false),
                ..good
            },
        ] {
            assert!(!matches_snapshot(&bad, true));
        }
    }
}
