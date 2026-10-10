//! Catalog-gated request choices. Omission preserves provider defaults, never implies Standard.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub(super) id: Option<String>,
    pub(super) name: String,
}

impl Choice {
    pub(crate) fn from_catalog(items: Vec<RequestControlChoice>, section: Section) -> Vec<Self> {
        choices(items, section)
    }
}

pub(super) fn choices(items: Vec<RequestControlChoice>, section: Section) -> Vec<Choice> {
    let mut choices = Vec::new();
    for item in items {
        let name = match (section, item.id.as_str()) {
            (Section::Speed, "default") => "Standard",
            (Section::Speed, "priority") => "Fast · premium",
            (Section::OutputDetail, "low") => "Concise",
            (Section::OutputDetail, "medium") => "Balanced",
            (Section::OutputDetail, "high") => "Detailed",
            _ => continue,
        };
        if choices
            .iter()
            .any(|c: &Choice| c.id.as_deref() == Some(&item.id))
        {
            continue;
        }
        choices.push(Choice {
            id: Some(item.id),
            name: name.into(),
        });
    }
    if !choices.is_empty() {
        choices.insert(
            0,
            Choice {
                id: None,
                name: "Provider default".into(),
            },
        );
    }
    choices
}

impl App {
    pub(super) fn request_choices(&self, section: Section) -> &[Choice] {
        self.chosen_model
            .as_ref()
            .map_or(&[], |model| match section {
                Section::Speed => model.service_tiers.as_slice(),
                Section::OutputDetail => model.verbosity_levels.as_slice(),
                _ => &[],
            })
    }
    pub(super) fn request_value(&self, section: Section) -> Option<&str> {
        match section {
            Section::Speed => self.chosen_service_tier.as_deref(),
            Section::OutputDetail => self.chosen_text_verbosity.as_deref(),
            _ => None,
        }
    }
    pub(super) fn request_caption(&self, section: Section) -> String {
        if self.chosen_model.is_none() {
            return if self.catalog_read.pending() {
                "Loading…"
            } else {
                "Choose model"
            }
            .into();
        }
        let value = self.request_value(section);
        if let Some(choice) = self
            .request_choices(section)
            .iter()
            .find(|c| c.id.as_deref() == value)
        {
            return choice.name.clone();
        }
        value.map_or_else(
            || "Unavailable".into(),
            |id| format!("{} (unavailable)", concise(id, 12)),
        )
    }
    pub(super) fn apply_request_choice(&mut self, section: Section, choice: Choice) {
        let Some(canonical) = self
            .request_choices(section)
            .iter()
            .find(|c| c.id == choice.id)
            .cloned()
        else {
            return;
        };
        match section {
            Section::Speed => self.chosen_service_tier = canonical.id,
            Section::OutputDetail => self.chosen_text_verbosity = canonical.id,
            _ => return,
        }
        self.model_controls.dismiss();
        self.select_model();
    }
    pub(super) fn reconcile_request_selection(&mut self) {
        let selection = self.effective_model_selection().cloned();
        self.chosen_service_tier = selection.as_ref().and_then(|s| s.service_tier.clone());
        self.chosen_text_verbosity = selection.and_then(|s| s.text_verbosity.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn supported() -> (App, tokio::sync::mpsc::Receiver<Command>) {
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
        app.selected = Some(SessionId::new("PUBLIC_controls").unwrap());
        app.management.baseline(vec![], vec![]);
        let controls = |ids: &[&str], section| {
            choices(
                ids.iter()
                    .map(|id| RequestControlChoice {
                        id: (*id).into(),
                        name: "PUBLIC metadata name".into(),
                        description: None,
                    })
                    .collect(),
                section,
            )
        };
        let model = Model {
            provider: "PUBLIC_provider".into(),
            id: "PUBLIC_model".into(),
            name: "PUBLIC demo".into(),
            label: "PUBLIC / Demo".into(),
            efforts: vec![],
            default_effort: None,
            service_tiers: controls(&["default", "priority"], Section::Speed),
            verbosity_levels: controls(&["low", "medium", "high"], Section::OutputDetail),
        };
        app.model_selections.insert(
            app.selected.clone().unwrap(),
            ModelSelection {
                provider: model.provider.clone(),
                model: model.id.clone(),
                reasoning_effort: None,
                service_tier: None,
                text_verbosity: None,
            },
        );
        app.models = vec![model];
        app.reconcile_model_selection();
        (app, rx)
    }
    fn browse(app: &mut App, section: Section) -> Stamp {
        app.model_controls
            .seed_menu(app.transport_epoch, app.generation);
        let stamp = app.model_controls.stamp().unwrap();
        drop(app.update(Message::ModelControls(Action::Browse(stamp, Some(section)))));
        app.model_controls.stamp().unwrap()
    }
    fn choose(app: &mut App, stamp: Stamp, section: Section, value: Option<&str>) {
        drop(app.update(Message::ModelControls(Action::Request(
            stamp,
            section,
            Choice {
                id: value.map(str::to_owned),
                name: "PUBLIC forged label".into(),
            },
        ))));
    }
    fn request(rx: &mut tokio::sync::mpsc::Receiver<Command>) -> SelectModelRequest {
        let Command::SelectModel { request, .. } = rx.try_recv().unwrap() else {
            panic!("wrong command")
        };
        request
    }
    #[test]
    fn unknown_metadata_cannot_invent_controls_or_hide_premium_label() {
        let item = |id: &str| RequestControlChoice {
            id: id.into(),
            name: "Free Fast".into(),
            description: None,
        };
        assert!(choices(vec![item("PUBLIC_unknown")], Section::Speed).is_empty());
        let values = choices(vec![item("priority"), item("priority")], Section::Speed);
        assert_eq!(values.len(), 2);
        assert_eq!(values[0].id, None);
        assert_eq!(values[1].name, "Fast · premium");
    }
    #[test]
    fn speed_is_explicit_and_preserves_output_and_effort_in_one_request() {
        let (mut app, mut rx) = supported();
        app.chosen_text_verbosity = Some("high".into());
        app.chosen_effort = Some(Effort {
            id: "PUBLIC_high".into(),
            name: "High".into(),
        });
        let stamp = browse(&mut app, Section::Speed);
        choose(&mut app, stamp, Section::Speed, Some("priority"));
        let selected = request(&mut rx).selection;
        assert_eq!(selected.service_tier.as_deref(), Some("priority"));
        assert_eq!(selected.text_verbosity.as_deref(), Some("high"));
        assert_eq!(selected.reasoning_effort.as_deref(), Some("PUBLIC_high"));
        assert!(app.model_pending);
        assert!(!app.model_controls.is_open());
        choose(&mut app, stamp, Section::Speed, Some("default"));
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn output_preserves_speed_and_provider_default_really_clears_the_value() {
        for value in [Some("high"), None] {
            let (mut app, mut rx) = supported();
            app.chosen_service_tier = Some("priority".into());
            app.chosen_text_verbosity = Some("low".into());
            let stamp = browse(&mut app, Section::OutputDetail);
            choose(&mut app, stamp, Section::OutputDetail, value);
            let selected = request(&mut rx).selection;
            assert_eq!(selected.service_tier.as_deref(), Some("priority"));
            assert_eq!(selected.text_verbosity.as_deref(), value);
        }
    }
    #[test]
    fn stale_wrong_section_unknown_unavailable_and_pending_choices_are_local() {
        for guard in 0..6 {
            let (mut app, mut rx) = supported();
            let mut stamp = browse(&mut app, Section::Speed);
            let section = if guard == 0 {
                Section::OutputDetail
            } else {
                Section::Speed
            };
            let id = if guard == 1 {
                "PUBLIC_unknown"
            } else {
                "priority"
            };
            match guard {
                2 => stamp.epoch += 1,
                3 => app.model_pending = true,
                4 => {
                    app.chosen_model.as_mut().unwrap().service_tiers.clear();
                }
                5 => app.follow_ready = false,
                _ => {}
            }
            choose(&mut app, stamp, section, Some(id));
            assert!(rx.try_recv().is_err());
            assert!(app.chosen_service_tier.is_none());
            assert!(app.chosen_text_verbosity.is_none());
        }
    }
    #[test]
    fn failure_restores_authoritative_preferences_and_ack_can_normalize_them() {
        for failed in [true, false] {
            let (mut app, mut rx) = supported();
            let stamp = browse(&mut app, Section::Speed);
            choose(&mut app, stamp, Section::Speed, Some("priority"));
            let mut selected = request(&mut rx).selection;
            selected.service_tier = Some("default".into());
            drop(app.receive(Event::Model {
                generation: app.generation,
                result: if failed {
                    Err("PUBLIC provider denied".into())
                } else {
                    Ok(SelectModelValue { selected })
                },
            }));
            assert!(!app.model_pending);
            assert_eq!(
                app.chosen_service_tier.as_deref(),
                if failed { None } else { Some("default") }
            );
            assert_eq!(
                app.request_caption(Section::Speed),
                if failed {
                    "Provider default"
                } else {
                    "Standard"
                }
            );
        }
    }
    #[test]
    fn model_route_change_clears_controls_but_reselecting_the_same_route_does_not() {
        for changed in [true, false] {
            let (mut app, mut rx) = supported();
            app.chosen_service_tier = Some("priority".into());
            app.chosen_text_verbosity = Some("high".into());
            let mut model = app.chosen_model.clone().unwrap();
            if changed {
                model.id = "PUBLIC_other_model".into();
            }
            drop(app.update(Message::Model(model)));
            let selected = request(&mut rx).selection;
            assert_eq!(
                selected.service_tier.as_deref(),
                if changed { None } else { Some("priority") }
            );
            assert_eq!(
                selected.text_verbosity.as_deref(),
                if changed { None } else { Some("high") }
            );
        }
    }
}
