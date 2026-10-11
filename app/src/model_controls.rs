//! Native model popup: one automatic catalog read per opening; stamped choices update the chat.
use super::*;
#[path = "model_menu_view.rs"]
mod menu;
#[path = "model_request_controls.rs"]
mod request;
pub use request::Choice;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    epoch: u64,
    generation: u64,
    opening: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Model,
    Effort,
    Speed,
    OutputDetail,
}
impl Section {
    fn label(self) -> &'static str {
        match self {
            Self::Model => "Model",
            Self::Effort => "Effort",
            Self::Speed => "Speed",
            Self::OutputDetail => "Output detail",
        }
    }
}
#[derive(Clone)]
pub enum Action {
    Toggle,
    Close(Stamp),
    Browse(Stamp, Option<Section>),
    Model(Stamp, Model),
    Effort(Stamp, Effort),
    DefaultEffort(Stamp),
    Request(Stamp, Section, Choice),
}
#[derive(Default)]
pub struct State {
    open: Option<Stamp>,
    section: Option<Section>,
    catalog_requested: bool,
    catalog_error: bool,
    pub(super) catalog_default: Option<ModelSelection>,
    pub(super) new_selection: Option<(u64, u64, ModelSelection)>,
    serial: u64,
}
impl State {
    pub fn dismiss(&mut self) {
        self.open = None;
        self.section = None;
        self.catalog_requested = false;
    }
    pub(super) fn catalog_failed(&mut self, failed: bool) {
        self.catalog_error = failed;
    }
    pub(super) fn catalog_is_failed(&self) -> bool {
        self.catalog_error
    }
    #[cfg(any(test, feature = "public-layout-fixture"))]
    pub(super) fn seed_menu(&mut self, epoch: u64, generation: u64) {
        // PUBLIC fixtures seed a completed catalog read; they never start a Host.
        self.toggle(epoch, generation);
        self.catalog_requested = true;
    }
    pub(super) fn browse(&mut self, section: Option<Section>) {
        if let (Some(stamp), Some(serial)) = (self.open, self.serial.checked_add(1)) {
            self.serial = serial;
            self.open = Some(Stamp {
                opening: serial,
                ..stamp
            });
            self.section = if section.is_some() && self.section == section {
                None
            } else {
                section
            };
        }
    }
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }
    /// Presentation-only acknowledgement stamp; the mutation path keeps its own.
    #[allow(dead_code)]
    pub(super) fn stamp(&self) -> Option<Stamp> {
        self.open
    }
    /// Which anchored submenu is open, for presentation that needs the section
    /// rather than only the open state.
    #[allow(dead_code)]
    pub(super) fn section(&self) -> Option<Section> {
        self.section
    }
    fn toggle(&mut self, epoch: u64, generation: u64) {
        if self.is_open() {
            self.dismiss();
        } else if let Some(serial) = self.serial.checked_add(1) {
            self.serial = serial;
            self.open = Some(Stamp {
                epoch,
                generation,
                opening: serial,
            });
        }
    }
}
fn concise(value: &str, max: usize) -> String {
    let mut chars = value.chars();
    let mut label: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        label.push('…');
    }
    label
}
impl App {
    fn model_choice_allowed(&self) -> bool {
        self.options.smoke.is_none()
            && self.ready
            && self.root_ready
            && self.composer_visible()
            && !self.model_pending
            && !self.new_pending
            && self.management.registry.ready()
            && self.selected.as_ref().is_none_or(|id| {
                self.follow_ready
                    && !self.management.registry.archived(id)
                    && self.sessions.get(id).is_none_or(|s| s.origin.is_none())
            })
    }
    fn model_stamp_current(&self, stamp: Stamp) -> bool {
        self.model_controls.open == Some(stamp)
            && stamp.epoch == self.transport_epoch
            && stamp.generation == self.generation
            && self.composer_visible()
    }
    fn ensure_model_catalog(&mut self) -> Task<Message> {
        if self.model_controls.is_open()
            && !self.model_controls.catalog_requested
            && self.options.smoke.is_none()
            && self.ready
            && self.composer_visible()
            && !self.model_pending
        {
            self.model_controls.catalog_requested = true;
            if !self.catalog_read.pending() {
                return self.update(Message::LoadModels);
            }
        }
        Task::none()
    }
    pub(super) fn model_control_action(&mut self, action: Action) -> Task<Message> {
        match action {
            Action::Toggle if self.composer_visible() => {
                self.model_controls
                    .toggle(self.transport_epoch, self.generation);
                return self.ensure_model_catalog();
            }
            Action::Close(stamp) if self.model_stamp_current(stamp) => {
                self.model_controls.dismiss()
            }
            Action::Browse(stamp, section) if self.model_stamp_current(stamp) => {
                self.model_controls.browse(section);
                return self.ensure_model_catalog();
            }
            Action::DefaultEffort(stamp)
                if self.model_stamp_current(stamp)
                    && self.model_choice_allowed()
                    && self.model_controls.section == Some(Section::Effort)
                    && self
                        .chosen_model
                        .as_ref()
                        .is_some_and(|m| !m.efforts.is_empty()) =>
            {
                self.chosen_effort = None;
                self.model_controls.dismiss();
                self.select_model();
            }
            Action::Request(stamp, section, choice)
                if self.model_stamp_current(stamp)
                    && self.model_choice_allowed()
                    && self.model_controls.section == Some(section)
                    && matches!(section, Section::Speed | Section::OutputDetail) =>
            {
                self.apply_request_choice(section, choice);
            }
            Action::Model(stamp, choice)
                if self.model_stamp_current(stamp)
                    && self.model_choice_allowed()
                    && self.model_controls.section == Some(Section::Model) =>
            {
                // Treat popup payloads as identity, not authority over effort metadata.
                if let Some(model) = self
                    .models
                    .iter()
                    .find(|m| m.provider == choice.provider && m.id == choice.id)
                    .cloned()
                {
                    self.model_controls.dismiss();
                    return self.update(Message::Model(model));
                }
            }
            Action::Effort(stamp, choice)
                if self.model_stamp_current(stamp)
                    && self.model_choice_allowed()
                    && self.model_controls.section == Some(Section::Effort) =>
            {
                if let Some(effort) = self
                    .chosen_model
                    .as_ref()
                    .and_then(|m| m.efforts.iter().find(|e| e.id == choice.id))
                    .cloned()
                {
                    self.model_controls.dismiss();
                    return self.update(Message::Effort(effort));
                }
            }
            _ => {}
        }
        Task::none()
    }
    pub(super) fn effective_model_selection(&self) -> Option<&ModelSelection> {
        self.selected
            .as_ref()
            .and_then(|id| self.model_selections.get(id))
            .or(self.model_controls.catalog_default.as_ref())
    }
    pub(super) fn reconcile_model_selection(&mut self) {
        if self.model_pending {
            return;
        }
        let selected = self.effective_model_selection().cloned();
        self.chosen_model = selected
            .as_ref()
            .and_then(|s| {
                self.models
                    .iter()
                    .find(|m| m.provider == s.provider && m.id == s.model)
            })
            .cloned();
        self.chosen_effort = self
            .chosen_model
            .as_ref()
            .and_then(|m| {
                selected.as_ref().and_then(|s| {
                    m.efforts
                        .iter()
                        .find(|e| Some(&e.id) == s.reasoning_effort.as_ref())
                })
            })
            .cloned();
        self.reconcile_request_selection();
    }
    pub(super) fn active_model_name(&self) -> Option<&str> {
        self.effective_model_selection()
            .map(|selection| {
                self.models
                    .iter()
                    .find(|model| {
                        model.provider == selection.provider && model.id == selection.model
                    })
                    .map_or(selection.model.as_str(), |model| model.name.as_str())
            })
            .or_else(|| self.chosen_model.as_ref().map(|model| model.name.as_str()))
    }
    fn model_choice_name(&self) -> &str {
        self.active_model_name().unwrap_or_else(|| {
            if !self.ready || self.model_controls.catalog_error {
                "Model unavailable"
            } else if self.catalog_read.pending() {
                "Loading models…"
            } else {
                "Choose model"
            }
        })
    }
    pub(super) fn model_caption(&self) -> String {
        let selection = self.effective_model_selection();
        let name = self.model_choice_name();
        if self.model_pending || self.model_controls.new_selection.is_some() {
            return format!("{} · applying…", concise(name, 24));
        }
        let effort = self
            .chosen_effort
            .as_ref()
            .map(|e| e.name.as_str())
            .or_else(|| selection.and_then(|s| s.reasoning_effort.as_deref()));
        match effort {
            Some(effort) => format!("{} · {}", concise(name, 24), concise(effort, 12)),
            None => concise(name, 24),
        }
    }
    fn effort_caption(&self) -> String {
        if let Some(effort) = &self.chosen_effort {
            return concise(&effort.name, 24);
        }
        self.effective_model_selection()
            .and_then(|s| s.reasoning_effort.as_deref())
            .map_or_else(
                || "Default effort".into(),
                |id| format!("{} (unavailable)", concise(id, 16)),
            )
    }
    pub(super) fn model_control(&self) -> Element<'_, Message> {
        let pill = container(
            button(
                row![
                    label("◇", ACCENT).size(12),
                    label(self.model_caption(), MUTED).size(12),
                    label(
                        if self.model_controls.is_open() {
                            "⌃"
                        } else {
                            "⌄"
                        },
                        MUTED
                    )
                    .size(12)
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            )
            .on_press_maybe(
                self.composer_visible()
                    .then_some(Message::ModelControls(Action::Toggle)),
            )
            .style(|theme, status| {
                let mut style = design::ghost(theme, status);
                style.background = Some(OVERLAY.into());
                style.border.radius = 20.0.into();
                style
            })
            .padding([6, 10]),
        )
        .id("native-model-controls");
        let popup = self
            .model_controls
            .open
            .filter(|_| self.composer_visible())
            .map(|stamp| self.model_popup(stamp));
        let dismiss = self
            .model_controls
            .open
            .map_or(Message::ModelControls(Action::Toggle), |s| {
                Message::ModelControls(Action::Close(s))
            });
        anchored_popup::anchored(pill.into(), popup, dismiss)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> (App, tokio::sync::mpsc::Receiver<Command>) {
        let (handle, feed, rx, _, _) = worker::test_channels();
        let options = crate::config::parse(
            [
                "--runtime",
                "/PUBLIC/apps/cli",
                "--expected-version",
                "0.2.1-alpha.1",
                "--home",
                "/PUBLIC/home",
                "--user-home",
                "/PUBLIC/user",
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
        app.transport_epoch = worker::TRANSPORT_EPOCH;
        app.selected = Some(SessionId::new("PUBLIC_popup").unwrap());
        app.management.baseline(vec![], vec![]);
        let effort = Effort {
            id: "PUBLIC_high".into(),
            name: "High".into(),
        };
        app.models = vec![Model {
            provider: "PUBLIC_provider".into(),
            id: "PUBLIC_exact_model".into(),
            name: "Demo model".into(),
            label: "PUBLIC / Demo model".into(),
            efforts: vec![effort.clone()],
            default_effort: Some(effort.id),
            service_tiers: vec![],
            verbosity_levels: vec![],
        }];
        (app, rx)
    }
    fn receive_catalog(app: &mut App, result: Result<ModelCatalog, String>) {
        // PUBLIC reducer fixtures explicitly own a read; no real worker/provider runs.
        let ticket = app
            .catalog_read
            .current()
            .unwrap_or_else(|| app.catalog_read.begin(app.transport_epoch).unwrap());
        drop(app.receive(Event::Catalog { ticket, result }));
    }
    fn open(app: &mut App) -> Stamp {
        app.model_controls
            .seed_menu(app.transport_epoch, app.generation);
        let stamp = app.model_controls.open.unwrap();
        drop(app.update(Message::ModelControls(Action::Browse(
            stamp,
            Some(Section::Model),
        ))));
        app.model_controls.open.unwrap()
    }
    fn catalog() -> ModelCatalog {
        serde_json::from_value(serde_json::json!({
            "default":{"provider":"PUBLIC_provider","model":"PUBLIC_new_model"},
            "routableProviders":["PUBLIC_provider"],
            "groups":[{"id":"PUBLIC_provider","name":"PUBLIC","models":[{
                "id":"PUBLIC_new_model","name":"New model",
                "reasoning":{"defaultEffort":"PUBLIC_high","efforts":[{"id":"PUBLIC_high","name":"High"}]},
                "serviceTier":{"tiers":[{"id":"default","name":"Standard"},{"id":"priority","name":"Fast"}]},
                "textVerbosity":{"levels":[{"id":"low","name":"Concise"},{"id":"medium","name":"Balanced"},{"id":"high","name":"Detailed"}]}
            }]}],"failures":[]
        })).unwrap()
    }
    #[test]
    fn missing_choice_is_neutral_without_a_read_and_unavailable_beats_loading() {
        let (mut app, mut rx) = app();
        app.models.clear();
        assert_eq!(app.model_choice_name(), "Choose model");
        assert_eq!(app.model_caption(), "Choose model");
        app.model_controls
            .seed_menu(app.transport_epoch, app.generation);
        drop(app.model_control());
        assert!(rx.try_recv().is_err()); // Views never initiate another catalog request.
        app.catalog_read.begin(app.transport_epoch).unwrap();
        assert_eq!(app.model_choice_name(), "Loading models…");
        app.model_controls.catalog_error = true;
        assert_eq!(app.model_choice_name(), "Model unavailable");
        app.model_controls.catalog_error = false;
        app.ready = false;
        assert_eq!(app.model_choice_name(), "Model unavailable");
        app.ready = true;
        app.catalog_read.retire();
        assert_eq!(app.model_choice_name(), "Choose model"); // Empty success is not a pending read.
    }
    #[test]
    fn known_choice_and_applying_status_survive_catalog_refresh_or_failure() {
        let (mut app, mut rx) = app();
        app.chosen_model = Some(app.models[0].clone());
        app.catalog_read.begin(app.transport_epoch).unwrap();
        app.model_controls.catalog_error = true;
        app.ready = false;
        assert_eq!(app.model_choice_name(), "Demo model");
        app.model_pending = true;
        assert_eq!(app.model_caption(), "Demo model · applying…");
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn opening_fetches_once_without_a_load_button_and_click_changes_current_chat() {
        let (mut app, mut rx) = app();
        app.models.clear();
        app.editor = text_editor::Content::with_text("PUBLIC draft");
        drop(app.update(Message::ModelControls(Action::Toggle)));
        assert!(matches!(rx.try_recv().unwrap(), Command::Catalog(_)));
        assert!(app.catalog_read.pending());
        let stamp = app.model_controls.stamp().unwrap();
        drop(app.update(Message::ModelControls(Action::Browse(
            stamp,
            Some(Section::Model),
        ))));
        assert!(rx.try_recv().is_err());
        receive_catalog(&mut app, Ok(catalog()));
        assert_eq!(app.models[0].name, "New model");
        assert_eq!(app.active_model_name(), Some("New model"));
        assert!(rx.try_recv().is_err()); // Catalog resolves the existing default; it never writes a selection.
        let session = app.selected.clone().unwrap();
        let stamp = app.model_controls.stamp().unwrap();
        drop(app.update(Message::ModelControls(Action::Model(
            stamp,
            app.models[0].clone(),
        ))));
        let Command::SelectModel {
            generation,
            request,
        } = rx.try_recv().unwrap()
        else {
            panic!("wrong command")
        };
        assert_eq!(request.session_id, session);
        assert_eq!(request.selection.model, "PUBLIC_new_model");
        assert!(app.model_pending);
        assert!(!app.send_allowed());
        assert!(!app.model_controls.is_open());
        drop(app.receive(Event::Model {
            generation,
            result: Ok(SelectModelValue {
                selected: request.selection,
            }),
        }));
        assert_eq!(app.model_selections[&session].model, "PUBLIC_new_model");
        assert_eq!(app.model_caption(), "New model · High");
        assert!(app.send_allowed());
        assert_eq!(app.editor.text(), "PUBLIC draft");
        assert!(rx.try_recv().is_err()); // No prompt or second selection step.
    }
    #[test]
    fn backend_catalog_enables_speed_and_output_then_ack_updates_the_menu() {
        let (mut app, mut rx) = app();
        let session = app.selected.clone().unwrap();
        app.model_selections.insert(
            session.clone(),
            ModelSelection {
                provider: "PUBLIC_provider".into(),
                model: "PUBLIC_new_model".into(),
                reasoning_effort: Some("PUBLIC_high".into()),
                service_tier: None,
                text_verbosity: None,
            },
        );
        drop(app.update(Message::ModelControls(Action::Toggle)));
        assert!(matches!(rx.try_recv().unwrap(), Command::Catalog(_)));
        receive_catalog(&mut app, Ok(catalog()));
        assert_eq!(app.request_choices(Section::Speed).len(), 3);
        assert_eq!(app.request_choices(Section::OutputDetail).len(), 4);
        for (section, id, caption) in [
            (Section::Speed, "priority", "Fast · premium"),
            (Section::OutputDetail, "high", "Detailed"),
        ] {
            if !app.model_controls.is_open() {
                app.model_controls
                    .seed_menu(app.transport_epoch, app.generation);
            }
            let stamp = app.model_controls.stamp().unwrap();
            drop(app.update(Message::ModelControls(Action::Browse(stamp, Some(section)))));
            let stamp = app.model_controls.stamp().unwrap();
            let choice = app
                .request_choices(section)
                .iter()
                .find(|c| c.id.as_deref() == Some(id))
                .unwrap()
                .clone();
            drop(app.update(Message::ModelControls(Action::Request(
                stamp, section, choice,
            ))));
            let Command::SelectModel {
                generation,
                request,
            } = rx.try_recv().unwrap()
            else {
                panic!("wrong command")
            };
            assert_eq!(request.session_id, session);
            assert_eq!(
                request.selection.reasoning_effort.as_deref(),
                Some("PUBLIC_high")
            );
            assert_eq!(request.selection.service_tier.as_deref(), Some("priority"));
            if section == Section::OutputDetail {
                assert_eq!(request.selection.text_verbosity.as_deref(), Some("high"));
            }
            assert!(app.model_pending);
            drop(app.receive(Event::Model {
                generation,
                result: Ok(SelectModelValue {
                    selected: request.selection,
                }),
            }));
            assert_eq!(app.request_caption(section), caption);
            assert!(!app.model_pending);
            assert!(rx.try_recv().is_err());
        }
        assert_eq!(
            app.model_selections[&session].service_tier.as_deref(),
            Some("priority")
        );
        assert_eq!(
            app.model_selections[&session].text_verbosity.as_deref(),
            Some("high")
        );
    }
    #[test]
    fn welcome_model_click_creates_one_chat_and_applies_the_clicked_model() {
        let (mut app, mut rx) = app();
        app.selected = None;
        app.follow_ready = false;
        let mut wire = catalog();
        wire.default.model = "PUBLIC_active_before_click".into();
        receive_catalog(&mut app, Ok(wire));
        assert_eq!(app.active_model_name(), Some("PUBLIC_active_before_click"));
        assert!(!app.model_caption().contains("Model default"));
        let stamp = open(&mut app);
        assert!(app.model_choice_allowed());
        drop(app.update(Message::ModelControls(Action::Model(
            stamp,
            app.models[0].clone(),
        ))));
        assert!(matches!(rx.try_recv().unwrap(), Command::Create { .. }));
        assert!(app.new_pending);
        assert!(rx.try_recv().is_err());
        let created = SessionId::new("PUBLIC_created_chat").unwrap();
        drop(app.receive(Event::Created(Ok(SessionCreateValue {
            session_id: created.clone(),
            agent_preset: None,
        }))));
        assert!(matches!(rx.try_recv().unwrap(), Command::Select { .. }));
        let Command::SelectModel {
            generation,
            request,
        } = rx.try_recv().unwrap()
        else {
            panic!("wrong command")
        };
        assert_eq!(request.session_id, created);
        assert_eq!(request.selection.model, "PUBLIC_new_model");
        assert!(app.model_pending);
        assert!(app.model_caption().starts_with("PUBLIC_active_before"));
        drop(app.receive(Event::Model {
            generation,
            result: Ok(SelectModelValue {
                selected: request.selection,
            }),
        }));
        assert_eq!(app.active_model_name(), Some("New model"));
        assert!(!app.new_pending && !app.model_pending);
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn failed_welcome_chat_creation_restores_the_active_model_without_a_prompt() {
        let (mut app, mut rx) = app();
        app.selected = None;
        app.follow_ready = false;
        let mut wire = catalog();
        wire.default.model = "PUBLIC_active".into();
        receive_catalog(&mut app, Ok(wire));
        let stamp = open(&mut app);
        drop(app.update(Message::ModelControls(Action::Model(
            stamp,
            app.models[0].clone(),
        ))));
        assert!(matches!(rx.try_recv().unwrap(), Command::Create { .. }));
        let duplicate = open(&mut app);
        drop(app.update(Message::ModelControls(Action::Model(
            duplicate,
            app.models[0].clone(),
        ))));
        assert!(rx.try_recv().is_err());
        drop(app.receive(Event::Created(Err("PUBLIC creation failure".into()))));
        assert!(!app.new_pending && !app.model_pending);
        assert!(app.model_controls.new_selection.is_none());
        assert_eq!(app.active_model_name(), Some("PUBLIC_active"));
        assert!(app.selected.is_none());
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn catalog_default_names_the_active_model_and_does_not_override_chat_selection() {
        let (mut app, mut rx) = app();
        app.model_selections.clear();
        receive_catalog(&mut app, Ok(catalog()));
        assert_eq!(app.active_model_name(), Some("New model"));
        assert!(app.model_caption().starts_with("New model"));
        let id = app.selected.clone().unwrap();
        app.model_selections.insert(
            id,
            ModelSelection {
                provider: "PUBLIC_provider".into(),
                model: "PUBLIC_chat_specific".into(),
                reasoning_effort: None,
                service_tier: None,
                text_verbosity: None,
            },
        );
        app.reconcile_model_selection();
        assert_eq!(app.active_model_name(), Some("PUBLIC_chat_specific"));
        app.catalog_read.begin(app.transport_epoch).unwrap();
        assert!(app.model_choice_allowed());
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn failed_or_empty_catalog_does_not_loop_and_reopening_retries() {
        for failed in [true, false] {
            let (mut app, mut rx) = app();
            app.models.clear();
            drop(app.update(Message::ModelControls(Action::Toggle)));
            assert!(matches!(rx.try_recv().unwrap(), Command::Catalog(_)));
            let mut empty = catalog();
            empty.groups.clear();
            receive_catalog(
                &mut app,
                if failed {
                    Err("PUBLIC unavailable".into())
                } else {
                    Ok(empty)
                },
            );
            let stamp = app.model_controls.stamp().unwrap();
            drop(app.update(Message::ModelControls(Action::Browse(
                stamp,
                Some(Section::Model),
            ))));
            drop(app.view());
            assert!(rx.try_recv().is_err());
            assert_eq!(app.model_controls.catalog_error, failed);
            drop(app.update(Message::ModelControls(Action::Toggle))); // Close.
            assert!(rx.try_recv().is_err());
            drop(app.update(Message::ModelControls(Action::Toggle))); // Retry/refresh.
            assert!(matches!(rx.try_recv().unwrap(), Command::Catalog(_)));
            assert!(!app.model_controls.catalog_error);
            assert!(rx.try_recv().is_err());
        }
    }
    #[test]
    fn cached_catalog_refresh_and_existing_read_preserve_session_selection() {
        let (mut app, mut rx) = app();
        let selection = ModelSelection {
            provider: "PUBLIC_provider".into(),
            model: "PUBLIC_exact_model".into(),
            reasoning_effort: None,
            service_tier: None,
            text_verbosity: None,
        };
        let session = app.selected.clone().unwrap();
        app.model_selections.insert(session.clone(), selection);
        app.reconcile_model_selection();
        drop(app.update(Message::ModelControls(Action::Toggle)));
        assert!(matches!(rx.try_recv().unwrap(), Command::Catalog(_)));
        assert_eq!(app.models[0].name, "Demo model");
        let stamp = app.model_controls.stamp().unwrap();
        drop(app.update(Message::ModelControls(Action::Close(stamp))));
        drop(app.update(Message::ModelControls(Action::Toggle)));
        assert!(rx.try_recv().is_err()); // Join the already pending read.
        receive_catalog(&mut app, Ok(catalog()));
        assert_eq!(app.model_selections[&session].model, "PUBLIC_exact_model");
        assert_eq!(app.model_caption(), "PUBLIC_exact_model");
        assert!(!app.model_pending);
    }
    #[test]
    fn auto_catalog_respects_smoke_disconnect_modal_and_applying_guards() {
        for guard in 0..4 {
            let (mut app, mut rx) = app();
            match guard {
                0 => {
                    app.options.smoke = Some(crate::config::Smoke {
                        exit_after: std::time::Duration::from_secs(1),
                        report: "/PUBLIC/report".into(),
                        screenshot: None,
                    })
                }
                1 => app.ready = false,
                2 => app.capabilities = true,
                _ => app.model_pending = true,
            }
            drop(app.update(Message::ModelControls(Action::Toggle)));
            assert!(rx.try_recv().is_err());
            assert!(!app.catalog_read.pending());
        }
    }
    #[test]
    fn closed_queue_catalog_and_model_failures_never_fake_a_chat_switch() {
        let (mut app, rx) = app();
        drop(rx);
        drop(app.update(Message::ModelControls(Action::Toggle)));
        assert!(!app.catalog_read.pending());
        assert!(app.model_controls.catalog_error);
        let stamp = app.model_controls.stamp().unwrap();
        drop(app.update(Message::ModelControls(Action::Browse(
            stamp,
            Some(Section::Model),
        ))));
        let stamp = app.model_controls.stamp().unwrap();
        drop(app.update(Message::ModelControls(Action::Model(
            stamp,
            app.models[0].clone(),
        ))));
        assert!(!app.model_pending);
        assert!(app.chosen_model.is_none());
        assert!(app.model_selections.is_empty());
    }
    #[test]
    fn clicking_each_active_row_toggles_its_panel_closed_and_open_again() {
        let (mut app, mut rx) = app();
        app.model_controls
            .seed_menu(app.transport_epoch, app.generation);
        for section in [
            Section::Model,
            Section::Effort,
            Section::Speed,
            Section::OutputDetail,
        ] {
            let mut stamp = app.model_controls.stamp().unwrap();
            for expected in [Some(section), None, Some(section), None] {
                drop(app.update(Message::ModelControls(Action::Browse(stamp, Some(section)))));
                let next = app.model_controls.stamp().unwrap();
                assert_ne!(next, stamp);
                stamp = next;
                assert_eq!(app.model_controls.section(), expected);
                assert!(app.model_controls.is_open());
            }
        }
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn active_panel_toggle_retires_old_choices_and_does_not_repeat_catalog_reads() {
        let (mut app, mut rx) = app();
        drop(app.update(Message::ModelControls(Action::Toggle)));
        assert!(matches!(rx.try_recv().unwrap(), Command::Catalog(_)));
        let root = app.model_controls.stamp().unwrap();
        drop(app.update(Message::ModelControls(Action::Browse(
            root,
            Some(Section::Model),
        ))));
        let opened = app.model_controls.stamp().unwrap();
        drop(app.update(Message::ModelControls(Action::Browse(
            opened,
            Some(Section::Model),
        ))));
        let closed = app.model_controls.stamp().unwrap();
        receive_catalog(&mut app, Ok(catalog()));
        drop(app.update(Message::ModelControls(Action::Model(
            opened,
            app.models[0].clone(),
        ))));
        drop(app.update(Message::ModelControls(Action::Browse(
            opened,
            Some(Section::Model),
        ))));
        assert_eq!(app.model_controls.stamp(), Some(closed));
        assert_eq!(app.model_controls.section(), None);
        assert!(rx.try_recv().is_err());
        drop(app.update(Message::ModelControls(Action::Toggle)));
        assert!(!app.model_controls.is_open());
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn cached_summary_rows_open_locally_and_back_restores_summary() {
        let (mut app, mut rx) = app();
        app.model_controls
            .seed_menu(app.transport_epoch, app.generation);
        assert_eq!(app.model_controls.section(), None);
        for section in [
            Section::Model,
            Section::Effort,
            Section::Speed,
            Section::OutputDetail,
        ] {
            let previous = app.model_controls.stamp().unwrap();
            drop(app.update(Message::ModelControls(Action::Browse(
                previous,
                Some(section),
            ))));
            let current = app.model_controls.stamp().unwrap();
            assert_ne!(previous, current);
            assert_eq!(app.model_controls.section(), Some(section));
            drop(app.view());
            drop(app.update(Message::ModelControls(Action::Browse(current, None))));
            assert_eq!(app.model_controls.section(), None);
        }
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn pane_changes_retire_queued_choices_back_and_close_messages() {
        let (mut app, mut rx) = app();
        let model_stamp = open(&mut app);
        drop(app.update(Message::ModelControls(Action::Browse(
            model_stamp,
            Some(Section::Effort),
        ))));
        let effort_stamp = app.model_controls.stamp().unwrap();
        for action in [
            Action::Model(model_stamp, app.models[0].clone()),
            Action::Model(effort_stamp, app.models[0].clone()),
            Action::Close(model_stamp),
            Action::Browse(model_stamp, None),
        ] {
            drop(app.update(Message::ModelControls(action)));
        }
        assert_eq!(app.model_controls.stamp(), Some(effort_stamp));
        assert_eq!(app.model_controls.section(), Some(Section::Effort));
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn effort_submenu_sends_exact_catalog_id_or_omits_it_for_default() {
        for default in [false, true] {
            let (mut app, mut rx) = app();
            app.chosen_model = Some(app.models[0].clone());
            app.chosen_effort = Some(app.models[0].efforts[0].clone());
            let stamp = open(&mut app);
            drop(app.update(Message::ModelControls(Action::Browse(
                stamp,
                Some(Section::Effort),
            ))));
            let stamp = app.model_controls.stamp().unwrap();
            let action = if default {
                Action::DefaultEffort(stamp)
            } else {
                Action::Effort(stamp, app.models[0].efforts[0].clone())
            };
            drop(app.update(Message::ModelControls(action)));
            let Command::SelectModel { request, .. } = rx.try_recv().unwrap() else {
                panic!("wrong command")
            };
            assert_eq!(
                request.selection.reasoning_effort.as_deref(),
                if default { None } else { Some("PUBLIC_high") }
            );
            assert!(!app.model_controls.is_open());
            assert!(app.model_pending);
        }
    }
    #[test]
    fn unsupported_speed_and_output_panels_never_select_models() {
        let (mut app, mut rx) = app();
        let mut stamp = open(&mut app);
        for section in [Section::Speed, Section::OutputDetail] {
            drop(app.update(Message::ModelControls(Action::Browse(stamp, Some(section)))));
            stamp = app.model_controls.stamp().unwrap();
            drop(app.view());
            drop(app.update(Message::ModelControls(Action::DefaultEffort(stamp))));
            drop(app.update(Message::ModelControls(Action::Model(
                stamp,
                app.models[0].clone(),
            ))));
            assert_eq!(app.model_controls.section(), Some(section));
        }
        assert!(rx.try_recv().is_err());
        assert!(!app.model_pending);
    }
    #[test]
    fn navigation_serial_exhaustion_keeps_last_panel_readable() {
        let (mut app, mut rx) = app();
        let stamp = open(&mut app);
        app.model_controls.serial = u64::MAX;
        drop(app.update(Message::ModelControls(Action::Browse(
            stamp,
            Some(Section::Speed),
        ))));
        assert_eq!(app.model_controls.stamp(), Some(stamp));
        assert_eq!(app.model_controls.section(), Some(Section::Model));
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn opening_and_dismissing_is_local_and_preserves_text() {
        let (mut app, mut rx) = app();
        app.editor = text_editor::Content::with_text("PUBLIC draft");
        let stamp = open(&mut app);
        assert!(!app.send_allowed());
        drop(app.view());
        drop(app.update(Message::Editor(text_editor::Action::Edit(
            text_editor::Edit::Insert('x'),
        ))));
        drop(app.update(Message::KeyboardSend));
        drop(app.update(Message::ModelControls(Action::Close(stamp))));
        assert!(!app.model_controls.is_open());
        assert_eq!(app.editor.text(), "PUBLIC draft");
        assert!(rx.try_recv().is_err());
        assert!(app.send_allowed());
    }
    #[test]
    fn choices_are_canonical_catalog_metadata_with_exact_route_and_default_effort() {
        let (mut app, mut rx) = app();
        let stamp = open(&mut app);
        let mut submitted = app.models[0].clone();
        submitted.default_effort = Some("PUBLIC_forged".into());
        submitted.efforts.clear();
        drop(app.update(Message::ModelControls(Action::Model(stamp, submitted))));
        let Command::SelectModel { request, .. } = rx.try_recv().unwrap() else {
            panic!("wrong command")
        };
        assert_eq!(request.selection.provider, "PUBLIC_provider");
        assert_eq!(request.selection.model, "PUBLIC_exact_model");
        assert_eq!(
            request.selection.reasoning_effort.as_deref(),
            Some("PUBLIC_high")
        );
        assert!(!app.model_controls.is_open());
        assert!(app.model_pending);
        assert!(app.model_caption().ends_with(" · applying…"));
        assert_ne!(app.model_caption(), "Applying model…");
    }
    #[test]
    fn unknown_model_or_effort_cannot_generate_a_request() {
        let (mut app, mut rx) = app();
        let stamp = open(&mut app);
        let mut unknown = app.models[0].clone();
        unknown.provider = "PUBLIC_wrong_provider".into();
        drop(app.update(Message::ModelControls(Action::Model(stamp, unknown))));
        app.chosen_model = Some(app.models[0].clone());
        drop(app.update(Message::ModelControls(Action::Browse(
            stamp,
            Some(Section::Effort),
        ))));
        let stamp = app.model_controls.open.unwrap();
        drop(app.update(Message::ModelControls(Action::Effort(
            stamp,
            Effort {
                id: "PUBLIC_unknown".into(),
                name: "High".into(),
            },
        ))));
        assert!(rx.try_recv().is_err());
        assert!(!app.model_pending);
        assert!(app.model_controls.is_open());
    }
    #[test]
    fn stale_openings_epochs_and_generations_cannot_select_or_close_new_popup() {
        for stale in 0..3 {
            let (mut app, mut rx) = app();
            let first = open(&mut app);
            drop(app.update(Message::ModelControls(Action::Close(first))));
            let current = open(&mut app);
            let old = match stale {
                0 => first,
                1 => Stamp {
                    epoch: current.epoch + 1,
                    ..current
                },
                _ => Stamp {
                    generation: current.generation + 1,
                    ..current
                },
            };
            drop(app.update(Message::ModelControls(Action::Model(
                old,
                app.models[0].clone(),
            ))));
            drop(app.update(Message::ModelControls(Action::Close(old))));
            assert_eq!(app.model_controls.open, Some(current));
            assert!(rx.try_recv().is_err());
        }
    }
    #[test]
    fn hidden_disconnected_or_pending_choices_are_inert_and_settings_dismisses_popup() {
        let (mut app, mut rx) = app();
        let stamp = open(&mut app);
        app.ready = false;
        drop(app.update(Message::ModelControls(Action::Model(
            stamp,
            app.models[0].clone(),
        ))));
        assert!(rx.try_recv().is_err());
        app.ready = true;
        app.model_pending = true;
        drop(app.update(Message::ModelControls(Action::Model(
            stamp,
            app.models[0].clone(),
        ))));
        assert!(rx.try_recv().is_err());
        app.model_pending = false;
        drop(app.update(Message::Settings(crate::settings::Action::Open)));
        assert!(!app.model_controls.is_open());
        // The existing Settings opener explicitly reads onboarding metadata.
        assert!(matches!(rx.try_recv().unwrap(), Command::KeyMetadata(_)));
        drop(app.update(Message::ModelControls(Action::Model(
            stamp,
            app.models[0].clone(),
        ))));
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn catalog_and_normalized_ack_resolve_authoritative_selection_not_optimistic_label() {
        let (mut app, mut rx) = app();
        let model = app.models[0].clone();
        let selected = ModelSelection {
            provider: model.provider.clone(),
            model: model.id.clone(),
            reasoning_effort: Some("PUBLIC_high".into()),
            service_tier: None,
            text_verbosity: None,
        };
        app.model_selections
            .insert(app.selected.clone().unwrap(), selected.clone());
        app.chosen_model = None;
        app.reconcile_model_selection();
        assert_eq!(app.model_caption(), "Demo model · High");
        let stamp = open(&mut app);
        drop(app.update(Message::ModelControls(Action::Model(stamp, model))));
        assert!(rx.try_recv().is_ok());
        let normalized = ModelSelection {
            reasoning_effort: None,
            ..selected
        };
        drop(app.receive(Event::Model {
            generation: app.generation,
            result: Ok(SelectModelValue {
                selected: normalized,
            }),
        }));
        assert_eq!(app.model_caption(), "Demo model");
        assert!(app.chosen_effort.is_none());
    }
    #[test]
    fn startup_epoch_adoption_retires_early_popup_and_old_actions() {
        let (mut app, mut rx) = app();
        app.ready = false;
        app.transport_epoch = 0;
        let early = open(&mut app);
        drop(app.update(Message::Worker(Event::Ready("PUBLIC version".into()))));
        assert!(!app.model_controls.is_open());
        assert!(matches!(rx.try_recv().unwrap(), Command::Catalog(_)));
        assert!(rx.try_recv().is_err());
        let current = open(&mut app);
        assert_ne!(early, current);
        for action in [
            Action::Close(early),
            Action::Model(early, app.models[0].clone()),
        ] {
            drop(app.update(Message::ModelControls(action)));
        }
        assert_eq!(app.model_controls.open, Some(current));
        assert!(rx.try_recv().is_err());
        drop(app.update(Message::ModelControls(Action::Close(current))));
        assert!(!app.model_controls.is_open());
    }
    #[test]
    fn unavailable_stored_effort_is_not_misrepresented_as_model_default() {
        let (mut app, mut rx) = app();
        let model = &app.models[0];
        app.model_selections.insert(
            app.selected.clone().unwrap(),
            ModelSelection {
                provider: model.provider.clone(),
                model: model.id.clone(),
                reasoning_effort: Some("PUBLIC_old_high".into()),
                service_tier: None,
                text_verbosity: None,
            },
        );
        app.reconcile_model_selection();
        assert!(app.chosen_effort.is_none());
        assert_eq!(app.effort_caption(), "PUBLIC_old_high (unavailable)");
        assert_eq!(app.model_caption(), "Demo model · PUBLIC_old_h…");
        drop(app.view());
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn menu_serial_exhaustion_and_long_unicode_labels_are_bounded() {
        let (mut app, mut rx) = app();
        app.model_controls.serial = u64::MAX;
        drop(app.update(Message::ModelControls(Action::Toggle)));
        assert!(!app.model_controls.is_open());
        assert_eq!(concise("世界é0123456789", 3), "世界é…");
        assert!(rx.try_recv().is_err());
    }
}
