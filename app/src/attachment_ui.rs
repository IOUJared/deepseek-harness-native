//! Native picker/drop/preview integration; local work is bounded, canceled and joined before exit.
#[cfg(test)]
#[path = "attachment_desktop_tests.rs"]
mod tests;
use super::*;
use attachment_picker::{Action, Ticket};
use iced::widget::column;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreviewTicket {
    pub stamp: attachment_draft::EditorStamp,
    pub serial: u64,
}

#[derive(Default)]
pub(super) struct LocalFiles {
    preview_serial: u64,
    preview_ticket: Option<PreviewTicket>,
    pub picker: attachment_picker::Picker,
    preview_stamp: Option<attachment_draft::EditorStamp>,
    uploaded: Option<worker::attachments::Ticket>,
    preview: Option<iced::widget::image::Handle>,
    notice: Option<&'static str>,
    pending: Option<(PreviewTicket, Arc<AtomicBool>)>,
}
impl LocalFiles {
    pub fn busy(&self) -> bool {
        self.picker.busy() || self.pending.is_some()
    }
    pub fn invalidate(&mut self) {
        self.picker.invalidate();
        self.clear_preview();
    }
    fn clear_preview(&mut self) {
        self.preview_ticket = None;
        self.preview_stamp = None;
        self.uploaded = None;
        self.preview = None;
        self.notice = None;
        if let Some((_, canceled)) = &self.pending {
            canceled.store(true, Ordering::Relaxed);
        }
    }
}
impl App {
    #[cfg(feature = "public-layout-fixture")]
    pub(super) fn public_local_preview_ready(&self) -> bool {
        self.preview_current() && self.local_files.preview.is_some()
    }
    #[cfg(feature = "public-layout-fixture")]
    pub(super) fn public_attachment_fixture(&mut self, picker: bool) {
        // Synthetic local metadata; discard Tasks so no disk read or upload occurs.
        if picker {
            drop(self.file_choose());
            let ticket = self.local_files.picker.ticket().expect("PUBLIC picker");
            drop(
                self.picker_completed(Arc::new(attachment_picker::Completion {
                    ticket,
                    result: Ok(vec![
                        attachment_picker::Entry {
                            name: "PUBLIC documents".into(),
                            path: "/PUBLIC/documents".into(),
                            directory: true,
                        },
                        attachment_picker::Entry {
                            name: "PUBLIC photo.png".into(),
                            path: "/PUBLIC/photo.png".into(),
                            directory: false,
                        },
                        attachment_picker::Entry {
                            name: "PUBLIC notes.txt".into(),
                            path: "/PUBLIC/notes.txt".into(),
                            directory: false,
                        },
                    ]),
                })),
            );
        } else {
            drop(self.choose_local_file("/PUBLIC-layout-fixture/PUBLIC photo.png".into()));
            let ticket = self.local_files.pending.as_ref().expect("PUBLIC preview").0;
            let rgba = (0..120)
                .flat_map(|y| (0..160).flat_map(move |x| [x as u8, y as u8, 180, 255]))
                .collect();
            drop(self.complete_file_preview(
                ticket,
                Arc::new(attachment_preview::Outcome::Ready {
                    width: 160,
                    height: 120,
                    rgba,
                }),
            ));
        }
    }
    fn preview_current(&self) -> bool {
        self.local_files.preview_stamp.is_some_and(|stamp| {
            self.files
                .editor()
                .is_some_and(|(current, _)| stamp == current)
                || self
                    .local_files
                    .uploaded
                    .as_ref()
                    .is_some_and(|owner| self.files.owns(owner))
        })
    }
    pub(super) fn reconcile_local_files(&mut self) {
        if self.local_files.picker.ticket().is_some_and(|ticket| {
            self.files
                .editor()
                .is_none_or(|(stamp, _)| stamp != ticket.stamp)
        }) {
            self.local_files.picker.invalidate();
        }
        if !self.preview_current() {
            self.local_files.clear_preview();
        }
    }
    pub(super) fn local_file_preview(&self) -> Element<'_, Message> {
        if !self.preview_current() {
            return Space::new().into();
        }
        let mut body = Column::new().spacing(3);
        if let Some(handle) = &self.local_files.preview {
            let (width, height) = if self.layout_size().height < 720.0 {
                (80, 60)
            } else {
                (160, 120)
            };
            body = body.push(
                iced::widget::image(handle.clone())
                    .width(width)
                    .height(height)
                    .content_fit(iced::ContentFit::Contain),
            );
            body =
                body.push(label("Local preview · file may change before upload", MUTED).size(10));
        }
        if let Some(notice) = self.local_files.notice {
            body = body.push(label(notice, MUTED).size(10));
        }
        container(body).id("native-file-preview").into()
    }
    pub(super) fn preview_uploaded(
        &mut self,
        stamp: attachment_draft::EditorStamp,
        ticket: worker::attachments::Ticket,
    ) {
        if self.local_files.preview_stamp == Some(stamp) {
            self.local_files.uploaded = Some(ticket);
        }
    }
    pub(super) fn start_file_preview(
        &mut self,
        stamp: attachment_draft::EditorStamp,
    ) -> Task<Message> {
        if !self.file_context().allowed
            || self.local_files.pending.is_some()
            || self
                .files
                .editor()
                .is_none_or(|(current, _)| current != stamp)
        {
            return Task::none();
        }
        let path = PathBuf::from(self.files.editor().unwrap().1);
        if worker::file_intake::SelectedFile::new(path.clone()).is_err() {
            self.status = "Choose a valid absolute file path before previewing.".into();
            return Task::none();
        }
        let Some(serial) = self.local_files.preview_serial.checked_add(1) else {
            self.status =
                "Preview authority exhausted. Restart before previewing another file.".into();
            return Task::none();
        };
        self.local_files.clear_preview();
        self.local_files.preview_serial = serial;
        let ticket = PreviewTicket { stamp, serial };
        self.local_files.preview_ticket = Some(ticket);
        let canceled = Arc::new(AtomicBool::new(false));
        self.local_files.pending = Some((ticket, canceled.clone()));
        self.local_files.preview_stamp = Some(stamp);
        self.local_files.notice = Some("Reading local image preview…");
        Task::perform(
            async move {
                let result = tokio::task::spawn_blocking(move || {
                    attachment_preview::load(path, || canceled.load(Ordering::Relaxed))
                })
                .await
                .unwrap_or(attachment_preview::Outcome::Rejected);
                (ticket, Arc::new(result))
            },
            |(ticket, result)| Message::FilePreviewRead(ticket, result),
        )
    }
    pub(super) fn complete_file_preview(
        &mut self,
        ticket: PreviewTicket,
        result: Arc<attachment_preview::Outcome>,
    ) -> Task<Message> {
        if self.local_files.pending.as_ref().map(|(owner, _)| *owner) != Some(ticket) {
            return Task::none();
        }
        self.local_files.pending = None;
        if self.local_files.preview_ticket == Some(ticket)
            && self.preview_current()
            && matches!(self.close, Close::None)
        {
            use attachment_preview::Outcome;
            match result.as_ref() {
                Outcome::Ready {
                    width,
                    height,
                    rgba,
                } => {
                    self.local_files.preview = Some(iced::widget::image::Handle::from_rgba(
                        *width,
                        *height,
                        rgba.clone(),
                    ));
                    self.local_files.notice = None;
                }
                Outcome::NotImage => {
                    self.local_files.notice = Some(
                        "No thumbnail · PNG and JPEG previews supported. Other files can still be uploaded.",
                    )
                }
                Outcome::Rejected => {
                    self.local_files.notice = Some(
                        "Preview unavailable or exceeds local image limits. Upload has its own file validation.",
                    )
                }
                Outcome::Canceled => self.local_files.notice = None,
            }
        }
        if self.draft_exit_requested {
            self.finish_draft_exit()
        } else {
            Task::none()
        }
    }
    pub(super) fn choose_local_file(&mut self, path: PathBuf) -> Task<Message> {
        if !self.file_context().allowed || self.observations_lost {
            return Task::none();
        }
        if self.files.at_capacity() {
            self.status = format!(
                "Attachment limit reached ({} files, 4 MiB each). Remove a file before choosing another.",
                attachment_draft::MAX_ATTACHMENTS
            );
            return Task::none();
        }
        if self.files.editor_open() || self.files.pending() || self.files.prompt_pending() {
            self.status =
                "Finish the current file selection or upload before choosing another.".into();
            return Task::none();
        }
        if self.local_files.busy() {
            self.status =
                "Previous local file read is finishing. Choose again after it completes.".into();
            return Task::none();
        }
        if worker::file_intake::SelectedFile::new(path.clone()).is_err() || path.to_str().is_none()
        {
            self.status =
                "Invalid dropped file path. Choose one local file with a UTF-8 path.".into();
            return Task::none();
        }
        let context = self.file_context();
        if !self.files.open(&context) {
            return Task::none();
        }
        self.review_local_path(path)
    }
    fn review_local_path(&mut self, path: PathBuf) -> Task<Message> {
        let context = self.file_context();
        let Some((stamp, _)) = self.files.editor() else {
            return Task::none();
        };
        let Some(path) = path.to_str() else {
            self.status = "File path must be UTF-8.".into();
            return Task::none();
        };
        if self.files.edit(stamp, path.to_owned(), &context) {
            self.status =
                "File selected locally. Review, then Upload; Send remains separate.".into();
            self.start_file_preview(self.files.editor().unwrap().0)
        } else {
            Task::none()
        }
    }
    fn picker_task(request: Option<attachment_picker::Request>) -> Task<Message> {
        request.map_or_else(Task::none, |request| {
            Task::perform(attachment_picker::operation(request), |result| {
                Message::FilePickerRead(Arc::new(result))
            })
        })
    }
    pub(super) fn picker_completed(
        &mut self,
        result: Arc<attachment_picker::Completion>,
    ) -> Task<Message> {
        self.reconcile_local_files();
        self.local_files.picker.complete(&result);
        if self.draft_exit_requested {
            self.finish_draft_exit()
        } else {
            Task::none()
        }
    }
    pub(super) fn file_choose(&mut self) -> Task<Message> {
        if !self.file_context().allowed || self.observations_lost || self.local_files.busy() {
            return Task::none();
        }
        let context = self.file_context();
        if !self.files.open(&context) {
            return Task::none();
        }
        self.file_picker_action(Action::Open(self.files.editor().unwrap().0))
    }
    pub(super) fn file_picker_action(&mut self, action: Action) -> Task<Message> {
        if let Action::Close(ticket) = action {
            if self.local_files.picker.ticket() == Some(ticket) {
                self.local_files.picker.close(ticket);
                if self
                    .files
                    .editor()
                    .is_some_and(|(stamp, path)| stamp == ticket.stamp && path.is_empty())
                {
                    self.files
                        .cancel_editor(ticket.stamp, &self.file_local_context());
                }
            }
            return Task::none();
        }
        if !self.file_context().allowed || self.observations_lost || !self.pending.is_empty() {
            return Task::none();
        }
        let stamp = match &action {
            Action::Open(stamp) => *stamp,
            Action::Path(t, _)
            | Action::Go(t)
            | Action::Up(t)
            | Action::Home(t)
            | Action::Hidden(t)
            | Action::Choose(t, _)
            | Action::Manual(t) => t.stamp,
            Action::Close(_) => unreachable!(),
        };
        if self
            .files
            .editor()
            .is_none_or(|(current, _)| stamp != current)
        {
            return Task::none();
        }
        let request = match action {
            Action::Open(stamp) => self
                .local_files
                .picker
                .open(stamp, self.options.backend.working_directory.clone()),
            Action::Path(ticket, path) => {
                self.local_files.picker.edit(ticket, path);
                None
            }
            Action::Go(ticket) => self
                .local_files
                .picker
                .navigate(ticket, PathBuf::from(self.local_files.picker.path())),
            Action::Up(ticket) => self
                .local_files
                .picker
                .directory()
                .parent()
                .map(PathBuf::from)
                .and_then(|path| self.local_files.picker.navigate(ticket, path)),
            Action::Home(ticket) => self
                .local_files
                .picker
                .navigate(ticket, self.options.backend.user_home.clone()),
            Action::Hidden(ticket) => self.local_files.picker.toggle_hidden(ticket),
            Action::Choose(ticket, index) => {
                let Some(entry) = self.local_files.picker.selected(ticket, index) else {
                    return Task::none();
                };
                if entry.directory {
                    self.local_files.picker.navigate(ticket, entry.path)
                } else {
                    self.local_files.picker.close(ticket);
                    return self.review_local_path(entry.path);
                }
            }
            Action::Manual(ticket) => {
                self.local_files.picker.close(ticket);
                None
            }
            Action::Close(_) => None,
        };
        Self::picker_task(request)
    }
    pub(super) fn picker_view(&self, ticket: Ticket) -> Element<'_, Message> {
        let picker = &self.local_files.picker;
        let enabled = !picker.busy() && self.file_context().allowed;
        let action = |a| Message::FilePicker(a);
        let mut listing = Column::new().spacing(3);
        for (index, entry) in picker.entries().iter().enumerate() {
            listing = listing.push(
                button(
                    label(
                        format!("{} {}", if entry.directory { "▸" } else { "·" }, entry.name),
                        TEXT,
                    )
                    .size(13),
                )
                .width(Length::Fill)
                .style(design::button_style)
                .padding([6, 9])
                .on_press_maybe(enabled.then_some(action(Action::Choose(ticket, index)))),
            );
        }
        if picker.busy() {
            listing = listing.push(label("Reading folder…", MUTED));
        } else if picker.entries().is_empty() && picker.notice().is_none() {
            listing = listing.push(label("No files in this folder", MUTED));
        }
        let mut body = column![
            row![
                label("Choose a file", TEXT).size(18),
                Space::new().width(Length::Fill),
                button("Cancel")
                    .style(design::button_style)
                    .on_press(action(Action::Close(ticket)))
            ]
            .spacing(10),
            label(
                "Up to 4 files · 4 MiB each. Selecting reads a local preview, not an upload.",
                MUTED
            )
            .size(11),
            row![
                button("Up")
                    .style(design::button_style)
                    .on_press_maybe(enabled.then_some(action(Action::Up(ticket)))),
                button("Home")
                    .style(design::button_style)
                    .on_press_maybe(enabled.then_some(action(Action::Home(ticket)))),
                button(if picker.hidden() {
                    "Hide hidden"
                } else {
                    "Show hidden"
                })
                .style(design::button_style)
                .on_press_maybe(enabled.then_some(action(Action::Hidden(ticket))))
            ]
            .spacing(6),
            row![
                text_input("Absolute folder", picker.path())
                    .on_input(move |path| Message::FilePicker(Action::Path(ticket, path)))
                    .on_submit(action(Action::Go(ticket)))
                    .style(design::input_style)
                    .padding(8),
                button("Go")
                    .style(design::button_style)
                    .on_press_maybe(enabled.then_some(action(Action::Go(ticket))))
            ]
            .spacing(6),
            scrollable(listing).height(Length::Fill),
            button(label("Enter a file path instead", MUTED).size(11))
                .style(design::button_style)
                .on_press(action(Action::Manual(ticket))),
        ]
        .spacing(12)
        .padding(20);
        if let Some(notice) = picker.notice() {
            body = body.push(label(notice.text(), DANGER).size(12));
        }
        container(body)
            .id("native-file-picker")
            .width((self.layout_size().width - 48.0).clamp(1.0, 720.0))
            .height((self.layout_size().height - 48.0).clamp(1.0, 600.0))
            .style(|_| container::Style {
                background: Some(SURFACE.into()),
                border: iced::Border {
                    radius: 12.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    }
}
