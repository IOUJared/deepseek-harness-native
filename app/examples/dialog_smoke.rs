//! Render/exercise the actual native decision component using PUBLIC local fixtures only.
//! No Backend::start, authenticated client, Node, model/provider, tool execution or reply RPC.
#![allow(dead_code)]
#[path = "../src/codex.rs"]
mod codex;
#[path = "support/codex_public_fixture.rs"]
mod codex_public_fixture;
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
#[path = "../src/ui.rs"]
mod ui;
#[path = "../src/worker.rs"]
mod worker;
use dsh_native_transport::dto::{AgentId, ApprovalOutcome, RemoteEventId, WaterfallKind};
use iced::widget::{column, container};
use iced::{Element, Length, Subscription, Task, window};
use interactions::{Action, Decisions, Key, Reply};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::PathBuf,
    time::Duration,
};

// Explicit PUBLIC projection fixtures: never acquire a client, Host or raw config.
fn public_plugin_snapshot() -> plugins::Snapshot {
    use dsh_native_transport::plugin::*;
    plugins::Snapshot {
        inventory: PluginInventorySnapshot {
            management_available: false,
            entries: vec![
                PluginEntry {
                    entry_id: "tree/PUBLIC-entry-a".into(),
                    module_name: "PUBLIC-example-module".into(),
                    enabled: true,
                    fiber_phase: FiberPhase::Active,
                },
                PluginEntry {
                    entry_id: "tree/PUBLIC-entry-b".into(),
                    module_name: "PUBLIC-readonly-module".into(),
                    enabled: false,
                    fiber_phase: FiberPhase::Absent,
                },
            ],
        },
        settings: SettingsDescribeValue {
            writable: true,
            namespaces: vec![SettingsNamespaceView {
                ns: "PUBLIC-live-namespace-not-inventory-id".into(),
                revision: 7,
                auto_generate: true,
                unsupported_fields: 2,
                secret_fields: 1,
                fields: vec![
                    SettingsField {
                        path: vec!["displayTitle".into()],
                        label: "PUBLIC display title".into(),
                        kind: SettingsFieldKind::String { max_length: 80 },
                        value: Some(SettingsScalar::String("PUBLIC_OLD_TITLE".into())),
                        overridden: false,
                    },
                    SettingsField {
                        path: vec!["showHints".into()],
                        label: "PUBLIC show hints".into(),
                        kind: SettingsFieldKind::Bool,
                        value: Some(SettingsScalar::Bool(true)),
                        overridden: false,
                    },
                    SettingsField {
                        path: vec!["bufferSize".into()],
                        label: "PUBLIC buffer size".into(),
                        kind: SettingsFieldKind::Number {
                            min: Some(1.0),
                            max: Some(128.0),
                            integer: true,
                        },
                        value: Some(SettingsScalar::Number(32.0)),
                        overridden: false,
                    },
                ],
            }],
        },
    }
}
fn plugin_action(
    settings: &mut settings::Settings,
    action: plugins::Action,
) -> Option<settings::Effect> {
    settings.handle(settings::Action::Plugins(action), 1, true)
}
fn load_public_plugins(settings: &mut settings::Settings, review: bool) {
    let read = settings.plugin_read_ticket();
    let Some(settings::Effect::Plugins(plugins::Effect::Read(ticket))) =
        plugin_action(settings, plugins::Action::Refresh(read))
    else {
        panic!("explicit local refresh missing")
    };
    settings.plugins_loaded(ticket, Ok(public_plugin_snapshot()));
    if review {
        let read = settings.plugin_read_ticket();
        assert!(
            plugin_action(
                settings,
                plugins::Action::Begin {
                    ticket: read,
                    field: 0
                }
            )
            .is_none()
        );
        let ticket = settings.plugin_draft_ticket().expect("public title editor");
        assert!(
            plugin_action(
                settings,
                plugins::Action::Text {
                    ticket,
                    input: plugins::Input::new("PUBLIC_REVIEW_TITLE".into())
                }
            )
            .is_none()
        );
        let ticket = settings.plugin_draft_ticket().unwrap();
        assert!(plugin_action(settings, plugins::Action::Review(ticket)).is_none());
        assert_ne!(settings.plugin_draft_ticket(), Some(ticket)); // independent review generation
    } else {
        let ticket = settings.plugin_read_ticket();
        assert!(plugin_action(settings, plugins::Action::ToggleInventory(ticket)).is_none());
        assert!(plugin_action(settings, plugins::Action::Search("PUBLIC".into())).is_none());
    }
}

#[derive(Clone)]
struct Options {
    output: PathBuf,
    mode: String,
    scale: f32,
}
fn options() -> Result<Options, &'static str> {
    let mut args = std::env::args().skip(1);
    let mut output = None;
    let mut mode = None;
    let mut scale = None;
    while let Some(key) = args.next() {
        let value = args.next().ok_or("missing-value")?;
        match key.as_str() {
            "--output" if output.is_none() => output = Some(PathBuf::from(value)),
            "--mode" if mode.is_none() => mode = Some(value),
            "--scale" if scale.is_none() => {
                scale = Some(value.parse::<f32>().map_err(|_| "invalid-scale")?)
            }
            _ => return Err("invalid-or-duplicate-argument"),
        }
    }
    let output = output.ok_or("output-required")?;
    let mode = mode.ok_or("mode-required")?;
    let scale = scale.unwrap_or(1.0);
    let boundary = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("evidence")
        .canonicalize()
        .map_err(|_| "boundary-unavailable")?;
    let metadata = fs::symlink_metadata(&output).map_err(|_| "output-directory-required")?;
    let canonical = output
        .canonicalize()
        .map_err(|_| "output-directory-required")?;
    if !output.is_absolute()
        || !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || !canonical.starts_with(boundary)
        || metadata.permissions().mode() & 0o777 != 0o700
        || fs::read_dir(&output)
            .map_err(|_| "output-unavailable")?
            .next()
            .is_some()
    {
        return Err("new-empty-private-evidence-directory-required");
    }
    if ![
        "approval",
        "question",
        "unsupported",
        "settings",
        "settings-confirm",
        "settings-general",
        "settings-codex",
        "settings-codex-connect",
        "settings-codex-ready",
        "settings-codex-callback",
        "settings-codex-enable",
        "settings-plugins",
        "settings-plugins-loaded",
        "settings-plugins-confirm",
    ]
    .contains(&mode.as_str())
        || ![1.0, 1.25].contains(&scale)
    {
        return Err("invalid-fixture-mode-or-scale");
    }
    Ok(Options {
        output: canonical,
        mode,
        scale,
    })
}
#[derive(Clone)]
enum Message {
    Action(Action),
    Settings(settings::Action),
    Opened(window::Id),
    Native(&'static str),
    Capture(window::Id),
    Screenshot(window::Screenshot),
    Saved(bool, [u32; 2]),
    Deadline,
    Close,
}
impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PublicFixtureMessage(redacted)")
    }
}
struct Fixture {
    options: Options,
    decisions: Decisions,
    settings: settings::Settings,
    key: Key,
    report: Value,
    finished: bool,
}
impl Fixture {
    fn boot(options: Options) -> (Self, Task<Message>) {
        let event_id = RemoteEventId::new("PUBLIC-render-event").unwrap();
        let key = Key {
            epoch: 1,
            event_id: event_id.clone(),
            serial: 1,
        };
        let mut decisions = Decisions::default();
        let (kind, request) = if options.mode == "approval" {
            (
                WaterfallKind::Approval,
                json!({"toolName":"PUBLIC_fixture_noop", "callId":"PUBLIC-fixture-call", "reason":"Render-only request. No command, tool, Host or provider exists in this fixture window."}),
            )
        } else {
            let mut value = json!({"questions":[
                {"id":"choice", "header":"Public fixture choice", "question":"Which interface should this local fixture display?", "options":[{"label":"Keep native UI", "description":"Test the real Iced decision component without a backend."},{"label":"Skip this fixture", "description":"No real workflow is affected."}]},
                {"id":"note", "header":"Public fixture note", "question":"Enter a public test-only note.", "detail":"This panel uses the production request decoder, view and answer reducer. It never sends the resulting reply anywhere."}
            ]});
            if options.mode == "unsupported" {
                value["wait"] = json!({"callId":"PUBLIC-fixture-call", "timed":true});
            }
            (WaterfallKind::Question, value)
        };
        decisions
            .add_owned(
                key.clone(),
                AgentId::new("PUBLIC-Agent-NOT-Session").unwrap(),
                kind,
                request,
            )
            .unwrap();
        decisions.handle(Action::Open(key.clone()), 1, true);
        let mut settings = settings::Settings::default();
        if options.mode.starts_with("settings") && !codex_public_fixture::supports(&options.mode) {
            let Some(settings::Effect::Read(read)) =
                settings.handle(settings::Action::Open, 1, true)
            else {
                panic!("local fixture metadata effect missing")
            };
            settings.metadata(
                read,
                Ok(dsh_native_core::OnboardingMetadata {
                    logged_in: false,
                    has_api_key: false,
                    writable: true,
                }),
            );
            let input = settings.input_ticket();
            settings.handle(
                settings::Action::Edit {
                    ticket: input,
                    input: settings::Input::new("PUBLIC_FAKE_RENDER_KEY".into()),
                },
                1,
                true,
            );
            if options.mode == "settings-confirm" {
                settings.handle(settings::Action::Review(settings.ticket()), 1, true);
            }
            let page = match options.mode.as_str() {
                "settings-general" => Some(settings::Page::General),
                "settings-codex" => Some(settings::Page::Codex),
                "settings-plugins" | "settings-plugins-loaded" | "settings-plugins-confirm" => {
                    Some(settings::Page::Plugins)
                }
                _ => None,
            };
            if let Some(page) = page {
                assert!(
                    settings
                        .handle(settings::Action::SelectPage(page), 1, true)
                        .is_none()
                );
            }
        }
        if matches!(
            options.mode.as_str(),
            "settings-plugins-loaded" | "settings-plugins-confirm"
        ) {
            load_public_plugins(&mut settings, options.mode == "settings-plugins-confirm");
        }
        let mut report = json!({"scope":"actual-native-component-public-local-fixture-only", "mode":options.mode, "applicationScale":options.scale, "pid":std::process::id(), "backendStarted":false, "nodeStarted":false, "modelPrompts":0, "modelCatalogRequested":false, "replyRpcIssued":false, "toolExecutionRequested":false, "fixtureOnly":true, "scriptedLocalReducerActions":true, "nativeKeyboardPointerInputQualified":false, "parentPaintInspectionRequired":true});
        if codex_public_fixture::supports(&options.mode) {
            let result = codex_public_fixture::setup(&mut settings, &options.mode);
            assert_eq!(
                result["fixturePassed"],
                json!(true),
                "public Codex setup failed"
            );
            report
                .as_object_mut()
                .unwrap()
                .extend(result.as_object().unwrap().clone());
        }
        (
            Self {
                options,
                decisions,
                settings,
                key,
                report,
                finished: false,
            },
            Task::perform(
                async {
                    tokio::time::sleep(Duration::from_secs(8)).await;
                },
                |_| Message::Deadline,
            ),
        )
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Opened(id) => {
                let info = window::run(id, |w| match w.display_handle().map(|h| h.as_raw()) {
                    Ok(raw_window_handle::RawDisplayHandle::Wayland(_)) => "wayland",
                    _ => "not-wayland",
                })
                .map(Message::Native);
                let capture = Task::perform(
                    async {
                        tokio::time::sleep(Duration::from_millis(1000)).await;
                    },
                    move |_| Message::Capture(id),
                );
                let scroll = if matches!(
                    self.options.mode.as_str(),
                    "settings-plugins-loaded" | "settings-plugins-confirm"
                ) {
                    self.report["programmaticScrollToFieldControls"] = json!(true);
                    iced::widget::operation::snap_to_end("native-settings-body")
                } else {
                    Task::none()
                };
                return Task::batch([info, scroll, capture]);
            }
            Message::Native(backend) => self.report["windowBackend"] = json!(backend),
            Message::Capture(id) if !self.finished => {
                return window::screenshot(id).map(Message::Screenshot);
            }
            Message::Screenshot(screenshot) if !self.finished => {
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
                    move |ok| Message::Saved(ok, size),
                );
            }
            Message::Saved(saved, size) if !self.finished => {
                self.report["screenshotSaved"] = json!(saved);
                self.report["screenshotPhysicalSize"] = json!(size);
                let exercised = self.exercise();
                let passed = saved && exercised && self.report["windowBackend"] == "wayland";
                self.finish(passed);
                return iced::exit();
            }
            Message::Settings(action) => {
                if codex_public_fixture::supports(&self.options.mode) {
                    drop(action);
                    return Task::none();
                }
                if matches!(
                    self.options.mode.as_str(),
                    "settings-plugins-loaded" | "settings-plugins-confirm"
                ) {
                    drop(action);
                    self.report["manualPluginFixtureMessageDropped"] = json!(true);
                    return Task::none();
                }
                // Always local-only. Drop any constructed move-only fixture secret.
                let effect = self.settings.handle(action, 1, true);
                self.report["manualLocalSettingsEffectSeen"] = json!(effect.is_some());
                drop(effect);
            }
            Message::Action(action) => {
                let submitted = self.decisions.handle(action, 1, true);
                self.report["manualLocalSubmissionSeen"] = json!(submitted.is_some());
            }
            Message::Deadline | Message::Close if !self.finished => {
                self.report["deadlineOrManualCloseBeforeEvidence"] = json!(true);
                self.finish(false);
                return iced::exit();
            }
            _ => {}
        }
        Task::none()
    }
    fn exercise(&mut self) -> bool {
        if codex_public_fixture::supports(&self.options.mode) {
            let result = codex_public_fixture::exercise(&mut self.settings, &self.options.mode);
            let passed = result["fixturePassed"] == json!(true);
            self.report
                .as_object_mut()
                .unwrap()
                .extend(result.as_object().unwrap().clone());
            return passed;
        }
        if matches!(
            self.options.mode.as_str(),
            "settings-plugins-loaded" | "settings-plugins-confirm"
        ) {
            self.report["pluginInventoryAndDescribeInjectedPublicOnly"] = json!(true);
            self.report["pluginReadRpcIssued"] = json!(false);
            self.report["pluginWriteRpcIssued"] = json!(false);
            self.report["keyWriteIssued"] = json!(false);
            let confirm = self.options.mode == "settings-plugins-confirm";
            if !confirm {
                let read = self.settings.plugin_read_ticket();
                if plugin_action(
                    &mut self.settings,
                    plugins::Action::SelectNamespace {
                        ticket: read,
                        index: 0,
                    },
                )
                .is_some()
                {
                    return false;
                }
                let read = self.settings.plugin_read_ticket();
                plugin_action(
                    &mut self.settings,
                    plugins::Action::Begin {
                        ticket: read,
                        field: 2,
                    },
                );
                let Some(ticket) = self.settings.plugin_draft_ticket() else {
                    return false;
                };
                plugin_action(
                    &mut self.settings,
                    plugins::Action::Text {
                        ticket,
                        input: plugins::Input::new("64".into()),
                    },
                );
                let Some(ticket) = self.settings.plugin_draft_ticket() else {
                    return false;
                };
                plugin_action(&mut self.settings, plugins::Action::Review(ticket));
            }
            let Some(reviewed) = self.settings.plugin_draft_ticket() else {
                return false;
            };
            let Some(settings::Effect::Plugins(plugins::Effect::Save {
                ticket,
                namespace,
                field,
                value,
            })) = plugin_action(&mut self.settings, plugins::Action::Confirm(reviewed))
            else {
                return false;
            };
            let expected = if confirm {
                vec!["displayTitle".to_string()]
            } else {
                vec!["bufferSize".to_string()]
            };
            let public_value = if confirm {
                value
                    == dsh_native_transport::plugin::SettingsScalar::String(
                        "PUBLIC_REVIEW_TITLE".into(),
                    )
            } else {
                value == dsh_native_transport::plugin::SettingsScalar::Number(64.0)
            };
            let checked = namespace.revision == 7
                && ticket.expected_revision == 7
                && field.path == expected
                && public_value;
            let replay_blocked =
                plugin_action(&mut self.settings, plugins::Action::Confirm(reviewed)).is_none();
            // The effect is inspected and dropped locally, never admitted to a worker.
            drop((namespace, field, value));
            self.settings
                .plugin_saved(ticket, plugins::SaveOutcome::NotSent);
            self.report["pluginSingleFieldCasEffectCheckedLocally"] = json!(checked);
            self.report["pluginConfirmReplayBlocked"] = json!(replay_blocked);
            self.report["pluginEffectDroppedWithoutWorker"] = json!(true);
            self.report["nativeKeyboardPointerInputQualified"] = json!(false);
            return checked && replay_blocked;
        }
        if matches!(
            self.options.mode.as_str(),
            "settings-general" | "settings-codex" | "settings-plugins"
        ) {
            let local = [
                settings::Page::General,
                settings::Page::Codex,
                settings::Page::Plugins,
                settings::Page::ApiLogin,
            ]
            .into_iter()
            .all(|page| {
                self.settings
                    .handle(settings::Action::SelectPage(page), 1, true)
                    .is_none()
            });
            self.report["categoryNavigationLocalOnly"] = json!(local);
            self.report["keyWriteIssued"] = json!(false);
            return local;
        }
        if self.options.mode.starts_with("settings") {
            let ticket = self.settings.ticket();
            if self.options.mode == "settings" {
                self.settings
                    .handle(settings::Action::Review(ticket), 1, true);
            }
            let Some(settings::Effect::Save { ticket, secret }) =
                self.settings
                    .handle(settings::Action::Confirm(ticket), 1, true)
            else {
                return false;
            };
            let redacted = format!("{secret:?}") == "SecretApiKey([REDACTED])";
            drop(secret); // No worker, no pipe, no Host, no real persistence.
            self.settings
                .saved(ticket, settings::SaveResult::Indeterminate);
            let Some(settings::Effect::Read(read)) =
                self.settings
                    .handle(settings::Action::Refresh(self.settings.ticket()), 1, true)
            else {
                return false;
            };
            self.settings.metadata(
                read,
                Ok(dsh_native_core::OnboardingMetadata {
                    logged_in: false,
                    has_api_key: true,
                    writable: true,
                }),
            );
            self.report["explicitLocalSecretTransferRedacted"] = json!(redacted);
            self.report["localIndeterminateResultSimulated"] = json!(true);
            self.report["keyWriteIssued"] = json!(false);
            return redacted;
        }
        if self.options.mode == "unsupported" {
            let rejected = [
                Action::Approval(self.key.clone(), ApprovalOutcome::AllowedOnce),
                Action::Submit(self.key.clone()),
                Action::CancelQuestion(self.key.clone()),
            ]
            .into_iter()
            .all(|action| self.decisions.handle(action, 1, true).is_none());
            self.report["unsupportedActionsRefused"] = json!(rejected);
            return rejected;
        }
        let submission = if self.options.mode == "approval" {
            self.decisions.handle(
                Action::Approval(self.key.clone(), ApprovalOutcome::AllowedOnce),
                1,
                true,
            )
        } else {
            self.decisions.handle(
                Action::Toggle {
                    key: self.key.clone(),
                    question: 0,
                    option: 0,
                },
                1,
                true,
            );
            self.decisions.handle(
                Action::Custom {
                    key: self.key.clone(),
                    question: 1,
                    text: "PUBLIC_TEST_NOTE".into(),
                },
                1,
                true,
            );
            self.decisions
                .handle(Action::Submit(self.key.clone()), 1, true)
        };
        let Some(submission) = submission else {
            return false;
        };
        let exact = match &submission.reply {
            Reply::Approval(ApprovalOutcome::AllowedOnce) if self.options.mode == "approval" => {
                true
            }
            Reply::Question(answer) if self.options.mode == "question" => {
                answer.answers.len() == 2
                    && answer.answers[0].id == "choice"
                    && answer.answers[0].selected == ["Keep native UI"]
                    && answer.answers[0].custom.is_none()
                    && answer.answers[1].id == "note"
                    && answer.answers[1].selected.is_empty()
                    && answer.answers[1].custom.as_deref() == Some("PUBLIC_TEST_NOTE")
            }
            _ => false,
        };
        self.report["exactLocalSubmissionVerified"] = json!(exact);
        // This is an explicit local ACK simulation, not an HTTP or Host acknowledgement.
        self.decisions
            .acknowledge(&submission.key, submission.attempt, Ok(()));
        self.report["simulatedLocalAckRetiresDelivery"] = json!(self.decisions.len() == 0);
        exact && self.decisions.len() == 0
    }
    fn finish(&mut self, passed: bool) {
        self.finished = true;
        self.report["status"] = json!(if passed { "passed" } else { "failed" });
        let path = self.options.output.join("result.json");
        if let Ok(file) = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
        {
            let _ = serde_json::to_writer_pretty(&file, &self.report);
            let _ = file.sync_all();
        }
    }
    fn view(&self) -> Element<'_, Message> {
        let banner = ui::label(
            "PUBLIC LOCAL FIXTURE · no Host, credentials, model, tools or RPC",
            ui::DANGER,
        );
        let body = if self.options.mode.starts_with("settings") {
            let background = iced::widget::row![
                container(
                    column![
                        ui::label("Harness", ui::TEXT).size(22),
                        ui::label("PUBLIC conversation preview", ui::MUTED),
                        ui::label("New conversation", ui::TEXT),
                        ui::label("Recent conversations", ui::MUTED)
                    ]
                    .spacing(24)
                )
                .padding(24)
                .width(220)
                .height(Length::Fill)
                .style(|_| ui::panel(ui::SURFACE)),
                container(
                    column![
                        ui::label("PUBLIC local conversation", ui::TEXT).size(22),
                        iced::widget::Space::new().height(Length::Fill),
                        ui::label(
                            "A public render-only conversation stays visible behind settings.",
                            ui::TEXT
                        ),
                        iced::widget::Space::new().height(Length::Fill),
                        container(ui::label("Message Harness…", ui::MUTED))
                            .padding(24)
                            .width(Length::Fill)
                            .style(|_| design::card(ui::SURFACE, 20.0))
                    ]
                    .spacing(20)
                )
                .padding(24)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_| ui::panel(design::BASE))
            ]
            .height(Length::Fill)
            .into();
            ui::settings_overlay(
                background,
                self.settings
                    .view_sized(true, 900.0, 650.0)
                    .map(Message::Settings),
                Message::Settings(settings::Action::Close),
            )
        } else {
            self.decisions
                .dialog(true)
                .map(|e| e.map(Message::Action))
                .unwrap_or_else(|| ui::label("Fixture request no longer pending", ui::TEXT).into())
        };
        container(column![banner, body].spacing(12))
            .padding(16)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| ui::panel(ui::SURFACE))
            .into()
    }
    fn subscription(&self) -> Subscription<Message> {
        window::events().filter_map(|(id, event)| match event {
            window::Event::Opened { .. } => Some(Message::Opened(id)),
            window::Event::CloseRequested => Some(Message::Close),
            _ => None,
        })
    }
}
fn main() -> iced::Result {
    let options = options().unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2)
    });
    let report_path = options.output.join("result.json");
    unsafe {
        std::env::set_var("ICED_BACKEND", "wgpu");
    }
    let result = iced::application(
        move || Fixture::boot(options.clone()),
        Fixture::update,
        Fixture::view,
    )
    .title("Harness native decision component · PUBLIC fixture")
    .window(window::Settings {
        size: iced::Size::new(1200.0, 800.0),
        min_size: Some(iced::Size::new(900.0, 600.0)),
        platform_specific: window::settings::PlatformSpecific {
            application_id: "ai.deepseek.harness.native.dialog-fixture".into(),
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
    .scale_factor(|f: &Fixture| f.options.scale)
    .subscription(Fixture::subscription)
    .run();
    let passed = fs::read(report_path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .is_some_and(|v| v["status"] == "passed");
    if !passed {
        eprintln!("Public native component fixture failed");
        std::process::exit(1)
    }
    result
}
