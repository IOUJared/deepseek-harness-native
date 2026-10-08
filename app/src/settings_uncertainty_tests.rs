use super::*;

fn metadata() -> OnboardingMetadata {
    OnboardingMetadata {
        logged_in: false,
        has_api_key: false,
        writable: true,
    }
}
fn open() -> Settings {
    let mut settings = Settings::default();
    let Some(Effect::Read(ticket)) = settings.handle(Action::Open, 1, true) else {
        panic!("read");
    };
    settings.metadata(ticket, Ok(metadata()));
    settings
}
fn submitted(settings: &mut Settings) -> Ticket {
    settings.handle(
        Action::Edit {
            ticket: settings.input_ticket(),
            input: Input::new("PUBLIC_UNCERTAIN_FIXTURE".into()),
        },
        1,
        true,
    );
    let ticket = settings.ticket();
    settings.handle(Action::Review(ticket), 1, true);
    let Some(Effect::Save { ticket, secret }) = settings.handle(Action::Confirm(ticket), 1, true)
    else {
        panic!("save");
    };
    drop(secret);
    ticket
}
fn refreshed(settings: &mut Settings, epoch: u64, present: bool) {
    let Some(Effect::Read(ticket)) =
        settings.handle(Action::Refresh(settings.ticket()), epoch, true)
    else {
        panic!("explicit read still available");
    };
    settings.metadata(
        ticket,
        Ok(OnboardingMetadata {
            has_api_key: present,
            ..metadata()
        }),
    );
}
fn blocked(settings: &mut Settings, epoch: u64, receipt: Ticket) {
    assert_eq!(settings.uncertain_save, Some(receipt));
    assert!(!settings.pending());
    let input = Input::new("PUBLIC_REPLACEMENT_MUST_NOT_ENTER".into());
    assert!(
        settings
            .handle(
                Action::Edit {
                    ticket: settings.input_ticket(),
                    input: input.clone()
                },
                epoch,
                true
            )
            .is_none()
    );
    assert!(input.take().is_none());
    assert!(settings.edit.editor.0.is_empty());
    assert!(!settings.can_save(true));
    assert!(
        settings
            .handle(Action::Review(settings.ticket()), epoch, true)
            .is_none()
    );
    assert!(!matches!(settings.save, SavePhase::Confirm(_)));
    assert!(
        settings
            .handle(Action::Confirm(settings.ticket()), epoch, true)
            .is_none()
    );
    assert_eq!(settings.uncertain_save, Some(receipt));
}

#[test]
fn uncertain_receipt_blocks_fresh_edits_and_writes_despite_presence_metadata() {
    let mut settings = open();
    let receipt = submitted(&mut settings);
    settings.saved(receipt, SaveResult::Indeterminate);
    for present in [false, true] {
        refreshed(&mut settings, 1, present);
        blocked(&mut settings, 1, receipt);
    }
}

#[test]
fn hidden_closed_or_reopened_panel_keeps_original_indeterminate_receipt() {
    for mode in 0..3 {
        let mut settings = open();
        let receipt = submitted(&mut settings);
        match mode {
            0 => settings.close(),
            1 => {
                settings.handle(Action::SelectPage(Page::General), 1, true);
            }
            _ => {
                settings.close();
                settings.handle(Action::Open, 1, true);
            }
        }
        settings.saved(receipt, SaveResult::Indeterminate);
        assert_eq!(settings.uncertain_save, Some(receipt));
        settings.close();
        settings.handle(Action::Open, 1, true);
        settings.handle(Action::SelectPage(Page::ApiLogin), 1, true);
        refreshed(&mut settings, 1, true);
        blocked(&mut settings, 1, receipt);
    }
}

#[test]
fn pending_disconnect_or_epoch_adoption_latches_exact_old_receipt() {
    for disconnect in [false, true] {
        let mut settings = open();
        let receipt = submitted(&mut settings);
        if disconnect {
            settings.disconnect();
        } else {
            settings.adopt_epoch(2);
        }
        assert_eq!(settings.uncertain_save, Some(receipt));
        settings.close();
        settings.handle(Action::Open, 2, true);
        refreshed(&mut settings, 2, true);
        blocked(&mut settings, 2, receipt);
        for result in [
            SaveResult::Confirmed,
            SaveResult::Refused,
            SaveResult::NotSent,
            SaveResult::Indeterminate,
        ] {
            settings.saved(receipt, result);
            settings.saved(
                Ticket {
                    serial: receipt.serial + 1,
                    ..receipt
                },
                result,
            );
            assert_eq!(settings.uncertain_save, Some(receipt));
        }
    }
}

#[test]
fn uncertain_receipt_survives_navigation_refresh_failures_reopen_and_ready_epochs() {
    let mut settings = open();
    let receipt = submitted(&mut settings);
    settings.saved(receipt, SaveResult::Indeterminate);
    for page in [Page::General, Page::Plugins, Page::Codex, Page::ApiLogin] {
        assert!(settings.handle(Action::SelectPage(page), 1, true).is_none());
        assert_eq!(settings.uncertain_save, Some(receipt));
    }
    let Some(Effect::Read(ticket)) = settings.handle(Action::Refresh(settings.ticket()), 1, true)
    else {
        panic!("read");
    };
    settings.metadata(ticket, Err(()));
    assert_eq!(settings.uncertain_save, Some(receipt));
    settings.adopt_epoch(3);
    settings.close();
    settings.handle(Action::Open, 3, true);
    refreshed(&mut settings, 3, false);
    blocked(&mut settings, 3, receipt);
    settings.saved(receipt, SaveResult::Confirmed);
    assert_eq!(settings.uncertain_save, Some(receipt));
    assert!(Settings::default().uncertain_save.is_none()); // Fresh app, not a same-Host reset protocol.
}

#[test]
fn unrelated_indeterminate_cannot_latch_and_known_results_allow_fresh_explicit_save() {
    for result in [
        SaveResult::Confirmed,
        SaveResult::Refused,
        SaveResult::NotSent,
    ] {
        let mut settings = open();
        let receipt = submitted(&mut settings);
        settings.saved(
            Ticket {
                serial: receipt.serial + 1,
                ..receipt
            },
            SaveResult::Indeterminate,
        );
        assert!(settings.uncertain_save.is_none());
        assert!(settings.pending());
        settings.saved(receipt, result);
        assert!(settings.uncertain_save.is_none());
        refreshed(&mut settings, 1, true);
        let next = submitted(&mut settings);
        assert_ne!(next, receipt);
        assert!(settings.uncertain_save.is_none());
    }
}
