//! Native-local sidebar organization; never rewrites Session cwd, stops work or erases logs.
use super::*;
use iced::widget::column;
use sidebar_organization::{Location, Mutation, Stamp};

// The Iced task and the real-IO regression test execute the same blocking owner.
pub(super) async fn save_sidebar(
    write: sidebar_organization::Write,
    store: Option<std::sync::Arc<std::sync::Mutex<draft_store::Store>>>,
) -> (u64, Result<(), draft_store::Failure>) {
    let ticket = write.ticket;
    let result = tokio::task::spawn_blocking(move || {
        store
            .ok_or(draft_store::Failure::NotSaved)?
            .lock()
            .map_err(|_| draft_store::Failure::Indeterminate)?
            .save_checked(&write.proposed, &write.expected)
    })
    .await
    .unwrap_or(Err(draft_store::Failure::Indeterminate));
    (ticket, result)
}

#[derive(Clone)]
pub enum Action {
    Open(crate::management::View),
    Close(Stamp),
    ReviewTrash(Stamp),
    Apply(Stamp, Mutation),
    Step(Stamp, bool),
    Registry(Stamp, crate::management::Operation),
    Rename(Stamp),
    Export(Stamp),
    Reload,
    TrashView,
}

impl App {
    pub(super) fn sidebar_open_message(&self, target: &SessionId) -> Message {
        Message::Sidebar(Action::Open(crate::management::View {
            epoch: self.transport_epoch,
            generation: self.generation,
            target: target.clone(),
        }))
    }
    pub(super) fn sidebar_target_allowed(&self, target: &SessionId) -> bool {
        self.options.smoke.is_none()
            && self.ready
            && self.root_ready
            && !self.roster_pending
            && !self.observations_lost
            && self.pending.is_empty()
            && !self.stopped
            && !self.new_pending
            && !self.workspace_pending
            && !self.local_files.picker.is_open()
            && self
                .sessions
                .get(target)
                .is_some_and(|session| session.origin.is_none())
            && !self.settings.is_open()
            && !self.decisions.is_open()
            && !self.management.is_open()
            && !self.management.pending()
            && !self.rename.is_open()
            && self.rename.pending().is_none()
            && !self.exporter.is_open()
            && self.exporter.pending().is_none()
            && matches!(self.close, Close::None)
    }
    fn sidebar_stamp_allowed(&self, stamp: &Stamp) -> bool {
        self.sidebar_organization.stamp().as_ref() == Some(stamp)
            && stamp.epoch == self.transport_epoch
            && stamp.generation == self.generation
            && self.sidebar_target_allowed(&stamp.target)
    }
    pub(super) fn sidebar_action(&mut self, action: Action) -> Task<Message> {
        match action {
            Action::Open(view)
                if view.epoch == self.transport_epoch
                    && view.generation == self.generation
                    && self.sidebar_target_allowed(&view.target)
                    && !self.sidebar_organization.busy() =>
            {
                self.sidebar_organization
                    .open(view.epoch, view.generation, view.target);
                self.model_controls.dismiss();
                self.conversation_menu = false;
                self.sidebar_sheet = false;
                self.capabilities = false;
            }
            Action::Close(stamp) => self.sidebar_organization.close(&stamp),
            Action::ReviewTrash(stamp) if self.sidebar_stamp_allowed(&stamp) => {
                self.sidebar_organization.review(&stamp)
            }
            Action::Apply(stamp, mutation) if self.sidebar_stamp_allowed(&stamp) => {
                let valid = match &mutation {
                    Mutation::Move(Location::Workspace(id)) => {
                        self.workspaces.iter().any(|w| &w.workspace_id == id)
                    }
                    Mutation::Reorder(_) => false, // Only the owned Step path constructs an order.
                    _ => true,
                };
                if valid {
                    return self.sidebar_save(stamp, mutation);
                }
            }
            Action::Step(stamp, down) if self.sidebar_stamp_allowed(&stamp) => {
                if let Some(order) = self.sidebar_step_order(&stamp.target, down) {
                    return self.sidebar_save(stamp, Mutation::Reorder(order));
                }
            }
            Action::Export(stamp)
                if self.sidebar_stamp_allowed(&stamp) && !self.sidebar_organization.busy() =>
            {
                let view = crate::management::View {
                    epoch: stamp.epoch,
                    generation: stamp.generation,
                    target: stamp.target,
                };
                let context = self.export_context_for(Some(&view.target));
                if context.allowed {
                    self.sidebar_organization.dismiss();
                    self.exporter
                        .apply(crate::exporter::Action::Open(view), &context);
                }
            }
            Action::Registry(stamp, operation)
                if self.sidebar_stamp_allowed(&stamp) && !self.sidebar_organization.busy() =>
            {
                let view = crate::management::View {
                    epoch: stamp.epoch,
                    generation: stamp.generation,
                    target: stamp.target,
                };
                self.sidebar_organization.dismiss();
                let context = self.management_context_for(Some(&view.target));
                self.management
                    .update(crate::management::Action::Open(view), &context);
                if let Some(ticket) = self.management.ticket() {
                    let context = self.management_context();
                    self.management.update(
                        crate::management::Action::Review { ticket, operation },
                        &context,
                    );
                }
            }
            Action::Rename(stamp)
                if self.sidebar_stamp_allowed(&stamp) && !self.sidebar_organization.busy() =>
            {
                let view = crate::management::View {
                    epoch: stamp.epoch,
                    generation: stamp.generation,
                    target: stamp.target,
                };
                let context = self.rename_context_for(Some(&view.target));
                let title = context
                    .allowed
                    .then(|| self.sessions.get(&view.target))
                    .flatten()
                    .map(|session| self.session_title(session));
                if let Some(title) = title {
                    self.sidebar_organization.dismiss();
                    self.rename
                        .apply(crate::rename::Action::Open { view, title }, &context);
                }
            }
            Action::Reload
                if !self.sidebar_organization.is_open()
                    || self
                        .sidebar_organization
                        .stamp()
                        .is_some_and(|stamp| self.sidebar_stamp_allowed(&stamp)) =>
            {
                return self.load_sidebar(true);
            }
            Action::TrashView if self.layout_controls_enabled() => {
                self.show_trash = !self.show_trash;
                if self.show_trash {
                    self.show_archived = false;
                }
            }
            _ => {}
        }
        Task::none()
    }
    pub(super) fn load_sidebar(&mut self, explicit: bool) -> Task<Message> {
        if self.options.smoke.is_some() {
            return Task::none();
        }
        let Some(ticket) = self.sidebar_organization.start_load(explicit) else {
            return Task::none();
        };
        let store = self.sidebar_store.clone();
        Task::perform(
            async move {
                let result = tokio::task::spawn_blocking(move || {
                    store
                        .ok_or(draft_store::Failure::Unavailable)?
                        .lock()
                        .map_err(|_| draft_store::Failure::Unavailable)?
                        .load()
                })
                .await
                .unwrap_or(Err(draft_store::Failure::Unavailable));
                (ticket, result)
            },
            |(ticket, result)| Message::SidebarLoaded { ticket, result },
        )
    }
    fn sidebar_save(&mut self, stamp: Stamp, mutation: Mutation) -> Task<Message> {
        let Some(write) = self.sidebar_organization.propose(&stamp, mutation) else {
            return Task::none();
        };
        let store = self.sidebar_store.clone();
        Task::perform(save_sidebar(write, store), |(ticket, result)| {
            Message::SidebarSaved { ticket, result }
        })
    }
    pub(super) fn sidebar_step_order(
        &self,
        target: &SessionId,
        down: bool,
    ) -> Option<Vec<SessionId>> {
        if self.show_trash
            || !self.management.registry.ready()
            || self.management.registry.pinned(target)
        {
            return None;
        }
        // Resolve the current view once; avoid scanning every workspace for every row.
        let mut membership = workspace_membership::Membership::new(self.navigation_workspace_ids());
        let mut sessions: Vec<_> = self
            .sessions
            .values()
            .filter(|s| {
                !self.sidebar_organization.trashed(&s.session_id)
                    && self.management.registry.archived(&s.session_id)
                        == self.management.registry.archived(target)
                    && !self.management.registry.pinned(&s.session_id)
                    && (self.workspace.is_none()
                        || match self.sidebar_organization.location(&s.session_id) {
                            Location::Original => membership.contains(&s.session_id),
                            Location::Workspace(id) => self.workspace.as_ref() == Some(&id),
                            Location::Ungrouped => false,
                        })
            })
            .collect();
        sessions.sort_by(|a, b| self.navigation_order(a, b));
        let mut order: Vec<_> = sessions.iter().map(|s| s.session_id.clone()).collect();
        let index = order.iter().position(|id| id == target)?;
        let other = if down {
            index.checked_add(1).filter(|next| *next < order.len())?
        } else {
            index.checked_sub(1)?
        };
        order.swap(index, other);
        Some(order)
    }
    pub(super) fn sidebar_group(&self, target: &SessionId) -> Option<WorkspaceId> {
        match self.sidebar_organization.location(target) {
            Location::Original => self
                .workspaces
                .iter()
                .find(|w| w.session_ids.contains(target))
                .map(|w| w.workspace_id.clone()),
            Location::Workspace(id) => Some(id),
            Location::Ungrouped => None,
        }
    }
    pub(super) fn sidebar_panel(&self) -> Element<'_, Message> {
        use crate::management::Operation;
        let stamp = self
            .sidebar_organization
            .stamp()
            .expect("visible sidebar actions");
        let session = self.sessions.get(&stamp.target);
        let title = session
            .map(|s| self.session_title(s))
            .unwrap_or_else(|| "Unavailable conversation".into());
        let enabled = self.sidebar_stamp_allowed(&stamp)
            && self.sidebar_organization.ready()
            && !self.sidebar_organization.busy()
            && self.sidebar_organization.failure().is_none();
        let trashed = self.sidebar_organization.trashed(&stamp.target);
        let mut body = column![
            row![
                label("Conversation options", TEXT)
                    .size(22)
                    .width(Length::Fill),
                button("Done")
                    .style(design::ghost)
                    .on_press(Message::Sidebar(Action::Close(stamp.clone())))
            ],
            label(short(&title, 80), TEXT).size(15),
            label(
                "Sidebar organization only · working directory and saved history stay unchanged.",
                MUTED
            )
            .size(12),
        ]
        .spacing(12);
        if self.sidebar_organization.busy() {
            body = body.push(
                label(
                    "Saving organization… Closing does not cancel the save.",
                    MUTED,
                )
                .size(12),
            );
        }
        if self.sidebar_organization.failure().is_some() {
            body=body.push(label("Organization could not be confirmed. Reload before making another change; no automatic retry.",DANGER).size(12))
                .push(button("Reload organization").style(design::ghost).on_press_maybe((!self.sidebar_organization.busy()).then_some(Message::Sidebar(Action::Reload))));
        } else if !self.sidebar_organization.ready() {
            body = body.push(label("Loading organization…", MUTED).size(12));
        }
        if trashed {
            body = body
                .push(
                    label(
                        "In Trash. History is retained; background work may still be running.",
                        DANGER,
                    )
                    .size(12),
                )
                .push(
                    button("Restore from Trash")
                        .style(design::primary)
                        .on_press_maybe(enabled.then_some(Message::Sidebar(Action::Apply(
                            stamp.clone(),
                            Mutation::Restore,
                        )))),
                );
        } else if self.sidebar_organization.review_trash() {
            body=body.push(label("Move this conversation to Trash? You can restore it. This does not stop agents, jobs or schedules, and does not erase history.",DANGER).size(13))
                .push(button("Move to Trash").style(design::danger).on_press_maybe(enabled.then_some(Message::Sidebar(Action::Apply(stamp.clone(),Mutation::Trash)))));
        } else {
            let pinned = self.management.registry.pinned(&stamp.target);
            let archived = self.management.registry.archived(&stamp.target);
            let registry_enabled = self.sidebar_stamp_allowed(&stamp)
                && self.management.registry.ready()
                && !self.sidebar_organization.busy();
            body = body.push(
                row![
                    button(if pinned { "Unpin" } else { "Pin" })
                        .style(design::ghost)
                        .on_press_maybe((registry_enabled && !archived).then_some(
                            Message::Sidebar(Action::Registry(
                                stamp.clone(),
                                if pinned {
                                    Operation::Unpin
                                } else {
                                    Operation::Pin
                                }
                            ))
                        )),
                    button(if archived {
                        "Restore from archive"
                    } else {
                        "Archive…"
                    })
                    .style(design::ghost)
                    .on_press_maybe(registry_enabled.then_some(Message::Sidebar(
                        Action::Registry(
                            stamp.clone(),
                            if archived {
                                Operation::Restore
                            } else {
                                Operation::Archive
                            }
                        )
                    ))),
                    button("Export…").style(design::ghost).on_press_maybe(
                        registry_enabled.then_some(Message::Sidebar(Action::Export(stamp.clone())))
                    ),
                    button("Rename…").style(design::ghost).on_press_maybe(
                        registry_enabled.then_some(Message::Sidebar(Action::Rename(stamp.clone())))
                    ),
                ]
                .spacing(8),
            );
            body = body.push(label("ORDER", MUTED).size(10)).push(
                row![
                    button("Move up").style(design::ghost).on_press_maybe(
                        (enabled && self.sidebar_step_order(&stamp.target, false).is_some())
                            .then_some(Message::Sidebar(Action::Step(stamp.clone(), false)))
                    ),
                    button("Move down").style(design::ghost).on_press_maybe(
                        (enabled && self.sidebar_step_order(&stamp.target, true).is_some())
                            .then_some(Message::Sidebar(Action::Step(stamp.clone(), true)))
                    )
                ]
                .spacing(8),
            );
            if pinned {
                body = body.push(
                    label(
                        "Unpin to change manual order. Pinned sessions keep the Host pin order.",
                        MUTED,
                    )
                    .size(11),
                );
            }
            let group = self.sidebar_group(&stamp.target);
            let group_name = group
                .as_ref()
                .map(|id| {
                    self.workspaces
                        .iter()
                        .find(|w| &w.workspace_id == id)
                        .map(|w| w.title.as_str())
                        .unwrap_or("Unavailable workspace")
                })
                .unwrap_or("Ungrouped");
            body = body.push(label(format!("SIDEBAR WORKSPACE · {group_name}"), MUTED).size(10));
            let mut destinations = column![
                button("Original workspace")
                    .style(design::ghost)
                    .on_press_maybe(enabled.then_some(Message::Sidebar(Action::Apply(
                        stamp.clone(),
                        Mutation::Move(Location::Original)
                    )))),
                button("Ungrouped")
                    .style(design::ghost)
                    .on_press_maybe(enabled.then_some(Message::Sidebar(Action::Apply(
                        stamp.clone(),
                        Mutation::Move(Location::Ungrouped)
                    ))))
            ]
            .spacing(3);
            for workspace in self.workspaces.iter().take(128) {
                destinations = destinations.push(
                    button(label(short(&workspace.title, 52), TEXT).size(12))
                        .style(design::ghost)
                        .on_press_maybe(enabled.then_some(Message::Sidebar(Action::Apply(
                            stamp.clone(),
                            Mutation::Move(Location::Workspace(workspace.workspace_id.clone())),
                        ))))
                        .width(Length::Fill),
                );
            }
            body = body.push(scrollable(destinations).height(150));
            body = body.push(
                button("Delete · move to Trash…")
                    .style(design::danger)
                    .on_press_maybe(
                        enabled.then_some(Message::Sidebar(Action::ReviewTrash(stamp.clone()))),
                    ),
            );
        }
        let size = settings_modal_size(self.layout_size());
        container(scrollable(body).height(Length::Fill))
            .id("native-sidebar-options")
            .padding(22)
            .width(size.width.min(600.0))
            .height(size.height.min(590.0))
            .style(|_| panel(SURFACE))
            .into()
    }
}
