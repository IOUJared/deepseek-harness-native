//! Local account controls. Only explicit receipts authorize writes; labels are not issuer identity.
use super::*;
use dsh_native_core::{
    CodexAccountId, CodexAccountLabel, CodexAccountMode, CodexAccountsMetadata, CodexAddedAccount,
    CodexLegacyGrant,
};
use iced::widget::column;

#[derive(Default)]
pub(super) struct Managed {
    pub(super) metadata: Option<CodexAccountsMetadata>,
    pub(super) fresh: bool,
    pub(super) uncertain: bool,
    pub(super) label: String,
    pub(super) review: Option<AccountReview>,
    pub(super) added: Option<CodexAddedAccount>,
}
#[derive(Clone, PartialEq, Eq)]
pub(super) enum AccountReview {
    Select {
        ticket: Ticket,
        id: CodexAccountId,
        epoch: u64,
    },
    SignOut {
        ticket: Ticket,
        id: CodexAccountId,
        epoch: u64,
    },
    All {
        ticket: Ticket,
        epoch: u64,
    },
}
impl Controller {
    pub(super) fn managed_mode(&self) -> bool {
        self.managed
            .metadata
            .as_ref()
            .is_some_and(|m| m.mode == CodexAccountMode::Managed)
    }
    pub(super) fn managed_reset_panel(&mut self) {
        self.managed.fresh = false;
        self.managed.label.clear();
        self.managed.review = None;
        self.managed.added = None;
    }
    fn has_accounts(&self) -> bool {
        self.managed
            .metadata
            .as_ref()
            .is_some_and(|m| !m.accounts.is_empty())
    }
    fn can_select_account(&self, id: &CodexAccountId) -> bool {
        self.managed.metadata.as_ref().is_some_and(|m| {
            m.selected.as_ref() != Some(id) && m.accounts.iter().any(|a| &a.opaque_id == id)
        })
    }
    fn account_epoch(&self) -> Option<u64> {
        if !self.managed.fresh || self.managed.uncertain || self.pending() {
            return None;
        }
        self.managed
            .metadata
            .as_ref()
            .filter(|m| m.mode == CodexAccountMode::Managed)?
            .epoch
    }
    pub(super) fn account_receipt(
        &mut self,
        operation: Operation,
        metadata: CodexAccountsMetadata,
    ) {
        // Only explicit nonmutating reconciliation can release uncertainty, never a late write ACK.
        if operation == Operation::Accounts
            && !self.detached_login
            && self
                .metadata
                .is_some_and(|m| !active(m.phase) && !m.retry_blocked)
        {
            self.managed.uncertain = false;
        }
        self.managed.metadata = Some(metadata);
        self.managed.fresh = true;
        self.managed.review = None;
    }
    pub(super) fn handle_account(&mut self, action: &Action, context: Context) -> Option<Effect> {
        let current = match action {
            Action::RefreshAccounts(t)
            | Action::AddAccount(t)
            | Action::ConfirmAccount(t)
            | Action::DismissAccount(t)
            | Action::ReviewSignOutAll(t) => *t == self.ticket(),
            Action::EditLabel { ticket, .. }
            | Action::ReviewSelect { ticket, .. }
            | Action::ReviewSignOut { ticket, .. } => *ticket == self.ticket(),
            _ => return None,
        };
        if !current || !self.allowed(context) || self.tab != Tab::Account {
            return None;
        }
        self.connection = None;
        self.managed.added = None;
        if !self.ready() {
            return None;
        }
        match action {
            Action::RefreshAccounts(_) => {
                self.managed.review = None;
                // A refused, failed or unsent read cannot republish the cached roster as current.
                self.managed.fresh = false;
                Some(Effect::AccountsRead {
                    ticket: self.begin(Operation::Accounts),
                })
            }
            Action::EditLabel { label, .. } if self.account_epoch().is_some() => {
                self.managed.review = None;
                // Invalid public labels never become an unbounded retained widget draft.
                self.managed.label = CodexAccountLabel::new(label.clone())
                    .map_or_else(|_| String::new(), |_| label.clone());
                None
            }
            Action::AddAccount(_) => {
                let expected_epoch = self.account_epoch()?;
                let label = CodexAccountLabel::new(self.managed.label.clone()).ok()?;
                self.managed.label.clear();
                self.clear_draft();
                let ticket = self.begin(Operation::Add);
                self.connection = Some(Connection {
                    epoch: self.epoch,
                    panel: self.panel,
                    step: ConnectStep::Adding(ticket),
                    auto_select: false,
                });
                Some(Effect::AddAccount {
                    ticket,
                    label,
                    expected_epoch,
                })
            }
            Action::ReviewSelect { account_id, .. } | Action::ReviewSignOut { account_id, .. } => {
                let epoch = self.account_epoch()?;
                let selecting = matches!(action, Action::ReviewSelect { .. });
                let eligible = if selecting {
                    self.can_select_account(account_id)
                } else {
                    self.managed
                        .metadata
                        .as_ref()?
                        .accounts
                        .iter()
                        .any(|a| &a.opaque_id == account_id)
                };
                if !eligible {
                    return None;
                }
                let ticket = self.ticket();
                self.managed.review = Some(if selecting {
                    AccountReview::Select {
                        ticket,
                        id: account_id.clone(),
                        epoch,
                    }
                } else {
                    AccountReview::SignOut {
                        ticket,
                        id: account_id.clone(),
                        epoch,
                    }
                });
                None
            }
            Action::ReviewSignOutAll(_) => {
                let epoch = self.account_epoch()?;
                if !self.has_accounts() {
                    return None;
                }
                self.managed.review = Some(AccountReview::All {
                    ticket: self.ticket(),
                    epoch,
                });
                None
            }
            Action::DismissAccount(_) => {
                self.managed.review = None;
                None
            }
            Action::ConfirmAccount(t) => {
                let epoch = self.account_epoch()?;
                let review = self.managed.review.take()?;
                match review {
                    AccountReview::Select {
                        ticket,
                        id,
                        epoch: expected,
                    } if ticket == *t && expected == epoch && self.can_select_account(&id) => {
                        Some(Effect::SelectAccount {
                            ticket: self.begin(Operation::Select),
                            account_id: id,
                            expected_epoch: expected,
                        })
                    }
                    AccountReview::SignOut {
                        ticket,
                        id,
                        epoch: expected,
                    } if ticket == *t && expected == epoch => Some(Effect::SignOutAccount {
                        ticket: self.begin(Operation::SignOut),
                        account_id: id,
                        expected_epoch: expected,
                    }),
                    AccountReview::All {
                        ticket,
                        epoch: expected,
                    } if ticket == *t && expected == epoch && self.has_accounts() => {
                        Some(Effect::SignOutAll {
                            ticket: self.begin(Operation::SignOutAll),
                            expected_epoch: expected,
                        })
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }
    pub(super) fn accounts_view(&self, context: Context) -> Element<'_, Action> {
        let t = self.ticket();
        let allowed = self.allowed(context) && self.ready();
        let writable = allowed && self.account_epoch().is_some();
        let mut body = column![
            button("Refresh accounts").padding([8,12]).style(design::ghost)
                .on_press_maybe(allowed.then_some(Action::RefreshAccounts(t))),
            label("Local labels and selection are not verified OAuth identity or entitlement. Quota: Unavailable.", MUTED).size(12),
        ].spacing(10);
        if self.managed.uncertain {
            body = body.push(label("Account write outcome unknown. No retry. Refresh explicitly after sign-in settles to reconcile local state.", DANGER).size(12));
        }
        let Some(m) = self
            .managed
            .metadata
            .as_ref()
            .filter(|_| self.managed.fresh)
        else {
            return body
                .push(
                    label(
                        "Account list unavailable. Opening settings performs no request.",
                        MUTED,
                    )
                    .size(12),
                )
                .into();
        };
        if m.mode == CodexAccountMode::Legacy {
            return body
                .push(
                    label(
                        "Legacy sign-in mode; managed account controls unavailable.",
                        MUTED,
                    )
                    .size(12),
                )
                .into();
        }
        if m.legacy == CodexLegacyGrant::Unadopted {
            body = body.push(
                label(
                    "A legacy stored grant is unadopted; it is never imported into this list.",
                    DANGER,
                )
                .size(12),
            );
        }
        for account in &m.accounts {
            let selected = m.selected.as_ref() == Some(&account.opaque_id);
            body = body.push(
                row![
                    label(
                        format!(
                            "{}{}",
                            account.label.as_str(),
                            if selected { " · Selected locally" } else { "" }
                        ),
                        TEXT
                    )
                    .size(13)
                    .width(Length::Fill),
                    button("Switch")
                        .padding([6, 8])
                        .style(design::ghost)
                        .on_press_maybe(
                            (writable && self.can_select_account(&account.opaque_id)).then_some(
                                Action::ReviewSelect {
                                    ticket: t,
                                    account_id: account.opaque_id.clone()
                                }
                            )
                        ),
                    button("Sign out")
                        .padding([6, 8])
                        .style(design::danger)
                        .on_press_maybe(writable.then_some(Action::ReviewSignOut {
                            ticket: t,
                            account_id: account.opaque_id.clone()
                        })),
                ]
                .spacing(6),
            );
        }
        let input = text_input("Local account label", &self.managed.label)
            .padding(10)
            .style(design::field);
        body = body.push(if writable { input.on_input(move |label| Action::EditLabel { ticket:t, label }) } else { input })
            .push(row![
                button("Add account").padding([8,12]).style(design::ghost).on_press_maybe((writable && !self.managed.label.is_empty()).then_some(Action::AddAccount(t))),
                button("Sign out all").padding([8,12]).style(design::danger).on_press_maybe((writable && self.has_accounts()).then_some(Action::ReviewSignOutAll(t))),
            ].spacing(8))
            .push(label("Add stores a grant without selecting it. Local sign-out removes local grants; it does not revoke them at OpenAI.", MUTED).size(12));
        body.into()
    }
    /// Consent replaces the account list/connection controls, so Confirm is visible without scrolling.
    pub(super) fn managed_review_view(&self, context: Context) -> Option<Element<'_, Action>> {
        let review = self.managed.review.as_ref()?;
        let t = self.ticket();
        let allowed = self.allowed(context) && self.ready();
        let (description, target, ticket, epoch) = match review {
            AccountReview::Select { ticket, id, epoch } => (
                "Confirm switching the locally selected account?",
                Some(id),
                ticket,
                epoch,
            ),
            AccountReview::SignOut { ticket, id, epoch } => {
                ("Confirm local sign-out?", Some(id), ticket, epoch)
            }
            AccountReview::All { ticket, epoch } => (
                "Confirm signing out all local accounts?",
                None,
                ticket,
                epoch,
            ),
        };
        let selectable = match review {
            AccountReview::Select { id, .. } => self.can_select_account(id),
            _ => true,
        };
        let writable =
            allowed && *ticket == t && self.account_epoch() == Some(*epoch) && selectable;
        let target_label = target.map_or_else(
            || "All locally stored accounts".to_owned(),
            |id| {
                self.managed
                    .metadata
                    .as_ref()
                    .and_then(|m| m.accounts.iter().find(|a| &a.opaque_id == id))
                    .map_or_else(
                        || "Local account label unavailable".to_owned(),
                        |a| format!("Local label: {}", a.label.as_str()),
                    )
            },
        );
        Some(column![
            label(description, DANGER).size(15),
            label(target_label, TEXT).size(13),
            label("Local labels are not verified OAuth identity or entitlement. Local sign-out removes stored grants, not OpenAI authorization. Current account epoch is checked.", MUTED).size(12),
            row![
                button("Confirm").padding([8,12]).style(design::danger)
                    .on_press_maybe(writable.then_some(Action::ConfirmAccount(t))),
                button("Cancel review").padding([8,12]).style(design::ghost)
                    .on_press_maybe(self.allowed(context).then_some(Action::DismissAccount(t))),
            ].spacing(8),
        ].spacing(12).into())
    }
}

#[cfg(test)]
#[path = "codex_managed_tests.rs"]
mod tests;
