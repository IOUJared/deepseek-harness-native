//! Native click-open submenus; choices retain the owning popup ticket.
use super::*;
use iced::widget::column;
impl App {
    fn menu_row(&self, stamp: Stamp, section: Section, value: String) -> Element<'_, Message> {
        let active = self.model_controls.section == Some(section);
        let row = button(
            row![
                label(section.label(), TEXT).size(13).width(92),
                container(
                    label(concise(&value, 20), MUTED)
                        .size(12)
                        .wrapping(text::Wrapping::None)
                )
                .width(Length::Fill)
                .height(18)
                .clip(true),
                label("›", MUTED).size(14).width(10)
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        )
        .on_press(Message::ModelControls(Action::Browse(stamp, Some(section))))
        .style(move |theme, status| {
            let mut style = design::ghost(theme, status);
            if active {
                style.background = Some(OVERLAY.into());
            }
            style.border.radius = 7.0.into();
            style
        })
        .padding([11, 10])
        .width(Length::Fill);
        if section == Section::Effort {
            container(row).id("native-reasoning-control").into()
        } else {
            row.into()
        }
    }
    pub(super) fn model_menu_main(&self, stamp: Stamp) -> Element<'_, Message> {
        let name = self.model_choice_name().to_owned();
        container(
            column![
                self.menu_row(stamp, Section::Model, name),
                self.menu_row(stamp, Section::Effort, self.effort_caption()),
                self.menu_row(stamp, Section::Speed, self.request_caption(Section::Speed)),
                self.menu_row(
                    stamp,
                    Section::OutputDetail,
                    self.request_caption(Section::OutputDetail)
                ),
            ]
            .spacing(2),
        )
        .width(270)
        .padding(6)
        .style(|_| design::card(SURFACE, 10.0))
        .into()
    }
    fn model_menu_option<'a>(
        &self,
        name: String,
        selected: bool,
        action: Option<Action>,
    ) -> Element<'a, Message> {
        button(
            row![
                label(
                    concise(&name, 48),
                    if action.is_some() { TEXT } else { MUTED }
                )
                .size(13)
                .width(Length::Fill),
                label(if selected { "✓" } else { "" }, ACCENT).size(13)
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        )
        .on_press_maybe(action.map(Message::ModelControls))
        .padding([10, 9])
        .width(Length::Fill)
        .style(move |theme, status| {
            let mut style = design::ghost(theme, status);
            if selected {
                style.background = Some(OVERLAY.into());
            }
            style.border.radius = 6.0.into();
            style
        })
        .into()
    }
    fn model_menu_options(&self, stamp: Stamp, section: Section) -> Element<'_, Message> {
        let enabled = self.model_choice_allowed();
        let mut options = column![].spacing(2);
        match section {
            Section::Model => {
                let mut provider = None;
                for model in &self.models {
                    if provider != Some(&model.provider) {
                        provider = Some(&model.provider);
                        let suffix = format!(" / {}", model.name);
                        options = options.push(
                            container(
                                label(
                                    concise(
                                        model
                                            .label
                                            .strip_suffix(&suffix)
                                            .unwrap_or(&model.provider),
                                        42,
                                    ),
                                    MUTED,
                                )
                                .size(11),
                            )
                            .padding([6, 9]),
                        );
                    }
                    let selected = self
                        .effective_model_selection()
                        .is_some_and(|s| s.provider == model.provider && s.model == model.id);
                    options = options.push(self.model_menu_option(
                        model.name.clone(),
                        selected,
                        enabled.then(|| Action::Model(stamp, model.clone())),
                    ));
                }
                if !self.models.is_empty() && (!enabled || self.catalog_read.pending()) {
                    options = options.push(
                        label(
                            if self.model_pending || self.new_pending {
                                "Applying model…"
                            } else if !enabled {
                                "Waiting for chat readiness…"
                            } else {
                                "Refreshing models…"
                            },
                            MUTED,
                        )
                        .size(12),
                    );
                }
                if self.models.is_empty() {
                    options = options.push(label(self.catalog_placeholder(), MUTED).size(12));
                }
            }
            Section::Effort => {
                let efforts = self
                    .chosen_model
                    .as_ref()
                    .map(|m| m.efforts.as_slice())
                    .unwrap_or(&[]);
                let selection = self.effective_model_selection();
                if efforts.is_empty() {
                    options =
                        options.push(label("No effort options for this model.", MUTED).size(12));
                } else {
                    options = options.push(self.model_menu_option(
                        "Default".into(),
                        selection.is_some_and(|s| s.reasoning_effort.is_none()),
                        enabled.then_some(Action::DefaultEffort(stamp)),
                    ));
                    for effort in efforts {
                        options =
                            options.push(self.model_menu_option(
                                effort.name.clone(),
                                selection.is_some_and(|s| {
                                    s.reasoning_effort.as_ref() == Some(&effort.id)
                                }),
                                enabled.then(|| Action::Effort(stamp, effort.clone())),
                            ));
                    }
                }
            }
            Section::Speed | Section::OutputDetail => {
                let choices = self.request_choices(section);
                for choice in choices {
                    options = options.push(self.model_menu_option(
                        choice.name.clone(),
                        choice.id.as_deref() == self.request_value(section),
                        enabled.then(|| Action::Request(stamp, section, choice.clone())),
                    ));
                }
                let note = if choices.is_empty() {
                    if self.catalog_read.pending() {
                        "Loading options…"
                    } else if self.chosen_model.is_none() {
                        "Choose a model first."
                    } else {
                        "Unavailable for this model."
                    }
                } else if section == Section::Speed {
                    "Fast may cost more or use more quota; speed varies."
                } else {
                    "Answer detail, not token limits."
                };
                options = options.push(container(label(note, MUTED).size(11)).padding([6, 9]));
            }
        }
        let height = (self.layout_size().height * 0.48).clamp(100.0, 340.0);
        container(
            column![
                button(
                    row![
                        label("‹", MUTED).size(15),
                        label(section.label(), TEXT).size(12)
                    ]
                    .spacing(8)
                )
                .on_press(Message::ModelControls(Action::Browse(stamp, None)))
                .style(design::ghost)
                .padding([6, 9])
                .width(Length::Fill),
                iced::widget::rule::horizontal(1),
                container(scrollable(options).height(Length::Shrink)).max_height(height),
            ]
            .spacing(4),
        )
        .id("native-model-submenu")
        .width(280)
        .padding(6)
        .style(|_| design::card(SURFACE, 10.0))
        .into()
    }
    /// What the empty Model section says. Only an answered read may claim the deployment has no
    /// models: a disconnected app, a refused read and a read still in flight each say their own
    /// thing, so "Check Settings" cannot describe a request that was never sent.
    pub(super) fn catalog_placeholder(&self) -> &'static str {
        if !self.ready || self.observations_lost {
            "Models unavailable while disconnected."
        } else if self.model_controls.catalog_error {
            "Models unavailable. Reopen to retry."
        } else if self.catalog_read.pending() {
            "Loading models…"
        } else {
            "No models available. Check Settings."
        }
    }
    pub(super) fn model_popup(&self, stamp: Stamp) -> Element<'_, Message> {
        let panel: Element<'_, Message> = match self.model_controls.section {
            Some(section) if self.layout_size().width >= 720.0 => row![
                self.model_menu_options(stamp, section),
                self.model_menu_main(stamp)
            ]
            .spacing(8)
            .align_y(iced::Alignment::End)
            .into(),
            Some(section) => self.model_menu_options(stamp, section),
            None => self.model_menu_main(stamp),
        };
        container(panel).id("native-model-popup").into()
    }
}
