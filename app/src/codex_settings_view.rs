//! Codex subscription navigation; presentation never claims account or quota verification from grant presence.
use super::*;
use crate::ui::SURFACE;
use iced::widget::{Column, Space, column, toggler};
use dsh_native_core::{CodexComposerDisplay, CodexUsageAlert};

fn usage_switch(theme: &iced::Theme, status: toggler::Status) -> toggler::Style {
    let mut style = toggler::default(theme, status);
    let on = matches!(status, toggler::Status::Active { is_toggled: true }
        | toggler::Status::Hovered { is_toggled: true });
    style.background = (if on { ACCENT } else { SURFACE }).into();
    style.foreground = TEXT.into();
    style.text_color = Some(TEXT);
    style
}

fn card<'a>(title: &'static str, body: Column<'a, Action>) -> Element<'a, Action> {
    container(column![label(title, TEXT).size(15), body].spacing(14))
        .padding(16)
        .width(Length::Fill)
        .style(|_| design::card(design::BASE, 10.0))
        .into()
}

impl Controller {
    fn usage_view(&self, context: Context) -> Element<'_, Action> {
        let ticket = self.ticket();
        let enabled = self.allowed(context) && self.ready();
        let action = move |action| Action::Usage { ticket, action };
        let preferences = &self.usage.draft;
        let composer = [
            (CodexComposerDisplay::Off, "Off"),
            (CodexComposerDisplay::Percent, "Percent"),
            (CodexComposerDisplay::ProgressBar, "Progress bar"),
            (CodexComposerDisplay::Runway, "Runway · Beta"),
        ].into_iter().fold(row![].spacing(4), |controls, (mode, title)| {
            controls.push(button(label(title, if preferences.composer_display == mode { ACCENT } else { MUTED }).size(12))
                .width(Length::Fill).padding([8, 4]).style(design::button_style)
                .on_press_maybe((enabled && mode != preferences.composer_display && self.usage_surface_action_allowed(&UsageAction::Composer(mode))).then_some(action(UsageAction::Composer(mode)))))
        });
        let alerts = [
            (CodexUsageAlert::Off, "Off"),
            (CodexUsageAlert::Percent20, "20% remaining"),
            (CodexUsageAlert::EarlyWarning, "Early warning"),
            (CodexUsageAlert::Custom, "Custom"),
        ].into_iter().fold(row![].spacing(4), |controls, (mode, title)| {
            controls.push(button(label(title, if preferences.usage_alert == mode { ACCENT } else { MUTED }).size(12))
                .width(Length::Fill).padding([8, 4]).style(design::button_style)
                .on_press_maybe((enabled && mode != preferences.usage_alert && self.usage_surface_action_allowed(&UsageAction::Alert(mode))).then_some(action(UsageAction::Alert(mode)))))
        });
        let retry = toggler(preferences.retry_after_reset).label("Retry after reset · inactive local choice").text_size(12)
            .style(usage_switch)
            .on_toggle_maybe((enabled && preferences.retry_after_reset).then_some(move |value| action(UsageAction::Retry(value))));
        // OFF is still actionable during pending work or disabled backend/settings writes.
        let local_off = self.open && context.visible && context.epoch == self.epoch
            && preferences.experimental_quota;
        let consent = toggler(preferences.experimental_quota).label("Beta · Experimental quota consent").text_size(12)
            .style(usage_switch)
            .on_toggle_maybe(local_off.then_some(move |value| action(UsageAction::Consent(value))));
        let mut body = column![
            label("Quota display, alerts and reset retry unavailable · saved choices are read-only except opt-out", MUTED).size(12),
            label("Composer display", TEXT).size(12), composer,
            label("Usage alerts", TEXT).size(12), alerts,
        ].spacing(10);
        if preferences.usage_alert == CodexUsageAlert::Custom || !self.usage.valid_threshold() {
            body = body.push(row![
                label("Remaining threshold (%)", MUTED).size(12),
                // Saved threshold is read-only without a working alert executor.
                text_input("0–100", &self.usage.threshold).width(100).padding(8).style(design::input_style),
            ].spacing(10));
        }
        let storage = self.usage.snapshot.as_ref().map_or("Not read".to_owned(), |snapshot| {
            if snapshot.persisted { format!("Last read/saved revision {}", snapshot.revision) }
            else { "Unwritten defaults · revision 0 · not persisted".to_owned() }
        });
        body = body.push(retry).push(consent)
            .push(label("OFF takes effect locally immediately. Save opt-outs to persist. Enabling is unavailable; no quota query starts here.", MUTED).size(12))
            .push(label("Quota observer, notifications and retry executor are not yet connected. No usage, reset time or runway is available.", MUTED).size(12))
            .push(row![
                button("Save opt-outs").padding([8, 12]).style(design::primary_button)
                    .on_press_maybe((enabled && self.usage.can_save() && self.usage_surface_action_allowed(&UsageAction::Save)).then_some(action(UsageAction::Save))),
                button("Refresh preferences").padding([8, 12]).style(design::button_style)
                    .on_press_maybe(enabled.then_some(action(UsageAction::Refresh))),
            ].spacing(8))
            .push(label(storage, MUTED).size(12))
            .push(label(self.usage.status(), if self.usage.attention() { DANGER } else { MUTED }).size(12));
        card("Usage · unavailable", body)
    }
    pub fn view(&self, context: Context) -> Element<'_, Action> {
        if let Some(review) = self.review_view(context) {
            return if self.runtime.pending() || self.runtime.attention() {
                column![label(self.runtime.status(), if self.runtime.attention() { DANGER } else { MUTED }).size(12), review].spacing(12).into()
            } else { review };
        }
        if let Some(review) = self.managed_review_view(context) {
            let mut body = column![review];
            if self.runtime.pending() || self.runtime.attention() {
                body = body.push(label(self.runtime.status(), if self.runtime.attention() { DANGER } else { MUTED }).size(12));
            }
            return card("Review local account change", body);
        }
        let ticket = self.ticket();
        let can_navigate = self.open && context.visible && context.epoch == self.epoch;
        let tabs = Tab::ALL.into_iter().fold(row![].spacing(4), |tabs, tab| {
            tabs.push(
                button(label(tab.title(), if self.tab == tab { TEXT } else { MUTED }).size(12))
                    .width(Length::Fill)
                    .padding([10, 4])
                    .style(move |theme, status| {
                        let mut style = design::ghost(theme, status);
                        if self.tab == tab {
                            style.background = Some(design::BASE.into());
                        }
                        style
                    })
                    .on_press_maybe(can_navigate.then_some(Action::SelectTab { ticket, tab })),
            )
        });
        let body: Element<'_, Action> = match self.tab {
            Tab::Account => column![
                card("ChatGPT account", column![self.connection_view(context), self.accounts_view(context)].spacing(10)),
                card("Subscription quota", column![
                    row![label("Unavailable", MUTED).size(12), Space::new().width(Length::Fill)],
                    label("Account usage is separate from a stored credential. Quota, reset times and account identity are not available from the current login status.", MUTED).size(12),
                ].spacing(10)),
                self.usage_view(context),
            ].spacing(12).into(),
            Tab::Models => {
                let configured = self.metadata.filter(|_| self.fresh).is_some_and(|m| m.route_configured);
                card("Models & runtime", column![
                    label(if configured { "ChatGPT model route configured" } else { "ChatGPT model route not confirmed" }, TEXT),
                    label("Choose a model and its supported reasoning, speed and response options in the chat composer. Changing this route does not change your current session model.", MUTED).size(12),
                    button("Review model activation")
                        .padding([9, 12]).style(design::button_style)
                        .on_press_maybe((self.allowed(context) && self.ready() && self.may_enable())
                            .then_some(Action::ReviewEnable(ticket))),
                    self.runtime_view(context, false),
                ].spacing(12))
            }
            Tab::Images => card("Images", column![
                label("Image input", TEXT),
                label("Attach an image through the composer. The selected model must support image input; account sign-in alone does not establish that capability.", MUTED).size(12),
                label("These limits apply to image inputs, not image generation or sketch tools. No image service or model request starts here.", MUTED).size(12),
                self.runtime_view(context, true),
            ].spacing(12)),
            Tab::Maintenance => {
                let allowed = self.allowed(context) && self.ready();
                let mut body = column![
                    button("Refresh connection status")
                        .padding([9, 12]).style(design::button_style)
                        .on_press_maybe(allowed.then_some(Action::CheckStatus(ticket))),
                    label("Checks local authorization and model-route metadata; it does not validate a subscription or request a model reply.", MUTED).size(12),
                    button(if self.advanced { "Hide sign-in recovery" } else { "Show sign-in recovery" })
                        .padding([9, 12]).style(design::button_style)
                        .on_press_maybe(self.allowed(context).then_some(Action::ToggleAdvanced(ticket))),
                ].spacing(12);
                if self.advanced {
                    body = body.push(self.connection_view(context));
                }
                card("Maintenance", body)
            }
        };
        let mut panel = column![
            label("Codex subscription", TEXT).size(17),
            container(tabs)
                .padding(4)
                .width(Length::Fill)
                .style(|_| design::card(SURFACE, 9.0)),
        ]
        .spacing(16);
        if self.tab != Tab::Account
            && (self.connection.is_some() || !matches!(self.notice, Notice::Idle))
        {
            panel = panel.push(
                label(
                    self.connection
                        .map_or(self.notice.text(), Connection::progress),
                    MUTED,
                )
                .size(12),
            );
        }
        // Runtime writes/critical failures stay visible even away from either editor tab.
        if !matches!(self.tab, Tab::Models | Tab::Images)
            && (self.runtime.pending() || self.runtime.attention())
        {
            panel = panel.push(label(self.runtime.status(), if self.runtime.attention() { DANGER } else { MUTED }).size(12));
        }
        // Cancellation and indeterminate writes remain visible even outside the Account tab.
        if self.tab != Tab::Account && self.pending_account_operation() {
            panel = panel.push(label(
                "Sign-in or model configuration is pending. Tab changes stop automatic next steps, not admitted writes.",
                if self.owner_uncertain { DANGER } else { MUTED },
            ).size(12));
        }
        if self.tab != Tab::Account
            && let Some(attempt) = self.metadata.and_then(|m| {
                (active(m.phase) || self.owner_uncertain)
                    .then_some(m.attempt)
                    .flatten()
            })
        {
            panel = panel.push(
                button("Cancel attempt")
                    .padding([8, 12])
                    .style(design::danger)
                    .on_press_maybe(
                        self.allowed(context)
                            .then_some(Action::Cancel { ticket, attempt }),
                    ),
            );
        }
        panel.push(body).width(Length::Fill).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context(epoch: u64) -> Context {
        Context {
            epoch,
            visible: true,
            enabled: true,
        }
    }

    #[test]
    fn usage_controls_construct_for_all_modes_and_visible_failure_without_business_effects() {
        let mut state = Controller::default();
        state.enter(1);
        for mode in [CodexComposerDisplay::Off, CodexComposerDisplay::Percent,
            CodexComposerDisplay::ProgressBar, CodexComposerDisplay::Runway] {
            state.handle(Action::Usage { ticket: state.ticket(), action: UsageAction::Composer(mode) }, context(1));
            for alert in [CodexUsageAlert::Off, CodexUsageAlert::Percent20,
                CodexUsageAlert::EarlyWarning, CodexUsageAlert::Custom] {
                state.handle(Action::Usage { ticket: state.ticket(), action: UsageAction::Alert(alert) }, context(1));
                let _: Element<'_, Action> = state.view(context(1));
            }
        }
        let Some(Effect::UsageRead { ticket }) = state.handle(Action::Usage {
            ticket: state.ticket(), action: UsageAction::Refresh,
        }, context(1)) else { panic!("private read") };
        state.completed(ticket, Outcome::UsageUnavailable);
        assert!(state.usage.attention());
        assert!(state.usage.status().contains("No cached fallback"));
        let _: Element<'_, Action> = state.view(context(1));
        assert!(state.flight.is_none() && state.connection.is_none() && state.metadata.is_none());
        // Widget construction is not physical input, rasterized paint or notification permission evidence.
    }

    #[test]
    fn tab_navigation_is_local_and_does_not_admit_business_work() {
        let mut state = Controller::default();
        state.enter(1);
        for tab in Tab::ALL {
            assert!(
                state
                    .handle(
                        Action::SelectTab {
                            ticket: state.ticket(),
                            tab
                        },
                        context(1)
                    )
                    .is_none()
            );
            assert_eq!(state.tab, tab);
            assert!(
                state.flight.is_none() && state.connection.is_none() && state.metadata.is_none()
            );
            let _: Element<'_, Action> = state.view(context(1));
        }
    }

    #[test]
    fn offline_tabs_remain_navigable_but_stale_hidden_and_wrong_epoch_tabs_do_not() {
        let mut state = Controller::default();
        state.enter(0);
        assert!(
            state
                .handle(
                    Action::SelectTab {
                        ticket: state.ticket(),
                        tab: Tab::Images
                    },
                    Context {
                        epoch: 0,
                        visible: true,
                        enabled: false
                    }
                )
                .is_none()
        );
        assert_eq!(state.tab, Tab::Images);
        let old = state.ticket();
        state.leave();
        state.enter(1);
        for (ticket, context) in [
            (old, context(1)),
            (
                state.ticket(),
                Context {
                    epoch: 1,
                    visible: false,
                    enabled: true,
                },
            ),
            (state.ticket(), context(2)),
        ] {
            state.handle(
                Action::SelectTab {
                    ticket,
                    tab: Tab::Images,
                },
                context,
            );
            assert_eq!(state.tab, Tab::Account);
        }
    }

    #[test]
    fn tab_switch_clears_callback_and_review_without_retiring_an_admitted_operation() {
        let mut state = Controller::default();
        state.enter(1);
        state.draft = Editor("PUBLIC-callback-not-a-credential".into());
        let ticket = state.ticket();
        state.flight = Some(Flight {
            ticket,
            operation: Operation::Status,
            revision: None,
            follow_usage: false,
        });
        state.handle(
            Action::SelectTab {
                ticket,
                tab: Tab::Models,
            },
            context(1),
        );
        assert!(state.draft.0.is_empty());
        assert_eq!(state.flight.as_ref().map(|f| f.ticket), Some(ticket));
        assert_eq!(state.tab, Tab::Models);
        state.leave();
        assert_eq!(state.tab, Tab::Account);
    }
}
