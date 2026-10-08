use super::*;

fn writable() -> OnboardingMetadata {
    OnboardingMetadata {
        logged_in: false,
        has_api_key: false,
        writable: true,
    }
}
fn opened() -> Settings {
    let mut state = Settings::default();
    let Some(Effect::Read(ticket)) = state.handle(Action::Open, 1, true) else {
        panic!("read expected")
    };
    state.metadata(ticket, Ok(writable()));
    state
}
fn edit(state: &mut Settings, text: &str) {
    let ticket = state.input_ticket();
    assert!(
        state
            .handle(
                Action::Edit {
                    ticket,
                    input: Input::new(text.into())
                },
                1,
                true
            )
            .is_none()
    );
}
fn submit(state: &mut Settings) -> Ticket {
    edit(state, "PUBLIC_FAKE_KEY");
    let ticket = state.ticket();
    assert!(state.handle(Action::Confirm(ticket), 1, true).is_none());
    state.handle(Action::Review(ticket), 1, true);
    let Some(Effect::Save { ticket, secret }) = state.handle(Action::Confirm(ticket), 1, true)
    else {
        panic!("explicit confirmed save required")
    };
    assert_eq!(format!("{secret:?}"), "SecretApiKey([REDACTED])");
    drop(secret);
    assert!(state.edit.editor.0.is_empty());
    ticket
}
#[test]
fn fresh_batched_edits_share_editor_fence_not_operation_serial() {
    let mut state = opened();
    let ticket = state.input_ticket();
    for text in ["PUBLIC_A", "PUBLIC_AB", "PUBLIC_ABC"] {
        state.handle(
            Action::Edit {
                ticket,
                input: Input::new(text.into()),
            },
            1,
            true,
        );
    }
    assert_eq!(state.edit.editor.0, "PUBLIC_ABC");
    assert_ne!(state.ticket().serial, ticket.serial);
}
#[test]
fn input_holder_clone_is_one_take_and_does_not_replay_old_draft() {
    let mut state = opened();
    let ticket = state.input_ticket();
    let input = Input::new("PUBLIC_A".into());
    let replay = input.clone();
    assert_eq!(format!("{input:?}"), "SecretInput([REDACTED])");
    state.handle(Action::Edit { ticket, input }, 1, true);
    edit(&mut state, "PUBLIC_NEW");
    state.handle(
        Action::Edit {
            ticket,
            input: replay.clone(),
        },
        1,
        true,
    );
    assert!(replay.take().is_none());
    assert_eq!(state.edit.editor.0, "PUBLIC_NEW");
}
#[test]
fn rejected_input_clears_old_masked_key_instead_of_submitting_it() {
    for text in [
        "PUBLIC BAD".to_string(),
        "PUBLIC\nBAD".into(),
        "非ASCII".into(),
        "X".repeat(MAX_DRAFT + 1),
    ] {
        let mut state = opened();
        edit(&mut state, "PUBLIC_OLD");
        edit(&mut state, &text);
        assert!(state.edit.editor.0.is_empty());
        assert!(!state.can_save(true));
        assert!(
            state
                .handle(Action::Review(state.ticket()), 1, true)
                .is_none()
        );
        assert!(!matches!(state.save, SavePhase::Confirm(_)));
    }
}
#[test]
fn input_fence_rejects_stale_drafts_after_submit_ack_and_close() {
    let mut state = opened();
    let old = state.input_ticket();
    let saved = submit(&mut state);
    let input = Input::new("PUBLIC_OLD_REPLAY".into());
    state.saved(saved, SaveResult::Confirmed);
    state.handle(
        Action::Edit {
            ticket: old,
            input: input.clone(),
        },
        1,
        true,
    );
    assert!(input.take().is_none());
    assert!(state.edit.editor.0.is_empty());
    state.close();
    assert!(!state.is_open());
    assert!(state.edit.editor.0.is_empty());
}
#[test]
fn edit_invalidates_old_review_and_requires_new_confirmation() {
    let mut state = opened();
    edit(&mut state, "PUBLIC_A");
    let reviewed = state.ticket();
    state.handle(Action::Review(reviewed), 1, true);
    edit(&mut state, "PUBLIC_AB");
    assert!(state.handle(Action::Confirm(reviewed), 1, true).is_none());
    assert!(matches!(state.save, SavePhase::Idle));
}
#[test]
fn startup_panel_adopts_ready_epoch_but_never_reads_or_writes_automatically() {
    let mut state = Settings::default();
    assert!(state.handle(Action::Open, 0, false).is_none());
    let old = state.input_ticket();
    state.adopt_epoch(1);
    assert!(state.is_open());
    assert!(state.metadata.is_none());
    assert!(state.reading.is_none());
    assert_ne!(old, state.input_ticket());
    let Some(Effect::Read(ticket)) = state.handle(Action::Refresh(state.ticket()), 1, true) else {
        panic!("refresh after readiness must work")
    };
    state.metadata(ticket, Ok(writable()));
    edit(&mut state, "PUBLIC_READY");
    assert!(state.can_save(true));
}
#[test]
fn unknown_readonly_smoke_and_wrong_epoch_never_admit_save() {
    for (meta, enabled, epoch) in [
        (None, true, 1),
        (
            Some(OnboardingMetadata {
                writable: false,
                ..writable()
            }),
            true,
            1,
        ),
        (Some(writable()), false, 1),
        (Some(writable()), true, 2),
    ] {
        let mut state = opened();
        state.metadata = meta;
        edit(&mut state, "PUBLIC_KEY");
        let t = state.ticket();
        state.handle(Action::Review(t), epoch, enabled);
        assert!(state.handle(Action::Confirm(t), epoch, enabled).is_none());
    }
}
#[test]
fn metadata_does_not_overwrite_typing_or_make_indeterminate_save_confirmed() {
    let mut state = opened();
    let Some(Effect::Read(read)) = state.handle(Action::Refresh(state.ticket()), 1, true) else {
        panic!()
    };
    edit(&mut state, "PUBLIC_TYPED");
    state.metadata(read, Ok(writable()));
    assert_eq!(state.edit.editor.0, "PUBLIC_TYPED");
    let saved = submit(&mut state);
    state.saved(saved, SaveResult::Indeterminate);
    let Some(Effect::Read(read)) = state.handle(Action::Refresh(state.ticket()), 1, true) else {
        panic!()
    };
    state.metadata(
        read,
        Ok(OnboardingMetadata {
            has_api_key: true,
            ..writable()
        }),
    );
    assert!(matches!(
        state.save,
        SavePhase::Done(SaveResult::Indeterminate)
    ));
    assert!(!state.can_save(true));
}
#[test]
fn close_reopen_does_not_apply_old_save_or_old_metadata_to_new_panel() {
    let mut state = opened();
    let saved = submit(&mut state);
    state.close();
    let Some(Effect::Read(read)) = state.handle(Action::Open, 1, true) else {
        panic!()
    };
    state.saved(saved, SaveResult::Confirmed);
    assert!(matches!(state.save, SavePhase::EndedElsewhere));
    state.metadata(
        Ticket {
            panel: read.panel - 1,
            ..read
        },
        Ok(writable()),
    );
    assert!(state.metadata.is_none());
    state.metadata(read, Ok(writable()));
    assert!(state.metadata.is_some());
}
#[test]
fn disconnect_ignores_late_success_and_clears_draft() {
    let mut state = opened();
    let saved = submit(&mut state);
    state.disconnect();
    state.saved(saved, SaveResult::Confirmed);
    assert!(matches!(
        state.save,
        SavePhase::Done(SaveResult::Indeterminate)
    ));
    assert!(state.edit.editor.0.is_empty());
    assert!(state.metadata.is_none());
    assert_eq!(
        SaveResult::from_core(Err(dsh_native_core::Error::RequestTimeout)),
        SaveResult::Indeterminate
    );
    assert_eq!(SaveResult::from_core(Ok(false)), SaveResult::Refused);
}
#[test]
fn private_owned_editor_wipes_full_allocation_and_debug_is_redacted() {
    let mut raw = String::with_capacity(128);
    raw.push_str("PUBLIC_WIPE_FIXTURE");
    let mut editor = Editor(raw);
    assert_eq!(format!("{editor:?}"), "SecretEditor([REDACTED])");
    editor.wipe();
    // wipe initialized every owned byte, including spare capacity, before this observation.
    let bytes = unsafe { std::slice::from_raw_parts(editor.0.as_ptr(), editor.0.capacity()) };
    assert!(bytes.iter().all(|byte| *byte == 0));
}

#[test]
fn category_navigation_is_local_and_clears_hidden_secret_edits() {
    let mut state = opened();
    edit(&mut state, "PUBLIC_UNSAVED");
    let stale = state.input_ticket();
    for page in [Page::General, Page::Codex, Page::Plugins, Page::ApiLogin] {
        assert!(state.handle(Action::SelectPage(page), 1, true).is_none());
        assert_eq!(state.page, page);
        assert!(state.edit.editor.0.is_empty());
    }
    let input = Input::new("PUBLIC_STALE_HIDDEN_INPUT".into());
    state.handle(
        Action::Edit {
            ticket: stale,
            input: input.clone(),
        },
        1,
        true,
    );
    assert!(input.take().is_none());
    assert!(state.edit.editor.0.is_empty());
}

#[test]
fn leaving_api_page_invalidates_review_and_does_not_replay_confirmation() {
    let mut state = opened();
    edit(&mut state, "PUBLIC_REVIEWED");
    let reviewed = state.ticket();
    state.handle(Action::Review(reviewed), 1, true);
    state.handle(Action::SelectPage(Page::Plugins), 1, true);
    state.handle(Action::SelectPage(Page::ApiLogin), 1, true);
    assert!(matches!(state.save, SavePhase::Idle));
    assert!(state.handle(Action::Confirm(reviewed), 1, true).is_none());
    assert!(!state.can_save(true));
}

#[test]
fn navigation_keeps_submitted_write_receipt_fenced_without_resubmission() {
    let mut state = opened();
    let submitted = submit(&mut state);
    assert!(
        state
            .handle(Action::SelectPage(Page::Codex), 1, true)
            .is_none()
    );
    assert!(matches!(state.save, SavePhase::Pending(ticket) if ticket == submitted));
    state.saved(submitted, SaveResult::Confirmed);
    assert!(matches!(state.save, SavePhase::Done(SaveResult::Confirmed)));
    assert!(
        state
            .handle(Action::SelectPage(Page::ApiLogin), 1, true)
            .is_none()
    );
    assert!(!state.can_save(true));
}

#[test]
fn readonly_category_navigation_never_opens_closed_settings_or_reads_backend() {
    let mut state = Settings::default();
    assert!(
        state
            .handle(Action::SelectPage(Page::Plugins), 0, false)
            .is_none()
    );
    assert!(!state.is_open());
    assert_eq!(state.page, Page::ApiLogin);
    assert!(state.handle(Action::Open, 0, false).is_none());
    assert!(
        state
            .handle(Action::SelectPage(Page::Plugins), 0, false)
            .is_none()
    );
    assert_eq!(state.page, Page::Plugins);
    assert!(state.reading.is_none());
}
