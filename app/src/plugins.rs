//! Native, explicitly loaded primitive plugin settings. No raw JSON or credential editor.
use crate::design;
use crate::ui::{ACCENT, DANGER, MUTED, TEXT, label};
use dsh_native_transport::plugin::{
    PluginInventorySnapshot, SettingsDescribeValue, SettingsField, SettingsFieldKind,
    SettingsNamespaceView, SettingsScalar,
};
use iced::widget::{button, column, container, row, text_input};
use iced::{Element, Length};
use std::sync::{
    Arc, Mutex,
    atomic::{Ordering, compiler_fence},
};

const MAX_INPUT: usize = 4096;
const MAX_NAMESPACES: usize = 128;
const MAX_FIELDS: usize = 128;
const MAX_ENTRIES: usize = 512;
const SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadTicket {
    pub epoch: u64,
    pub panel: u64,
    pub serial: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DraftTicket {
    pub epoch: u64,
    pub panel: u64,
    pub description: u64,
    pub namespace: usize,
    pub field: usize,
    pub editor: u64,
    pub revision: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WriteTicket {
    pub epoch: u64,
    pub panel: u64,
    pub serial: u64,
    pub description: u64,
    pub namespace: usize,
    pub expected_revision: u64,
    pub draft_revision: u64,
}
#[derive(Clone, Copy)]
pub struct Context {
    pub epoch: u64,
    pub visible: bool,
    pub enabled: bool,
}
#[derive(Clone)]
pub struct Snapshot {
    pub inventory: PluginInventorySnapshot,
    pub settings: SettingsDescribeValue,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadFailure {
    Unavailable,
    Refused,
    Malformed,
}
#[derive(Clone)]
pub enum SaveOutcome {
    Confirmed(SettingsNamespaceView),
    Conflict,
    Refused,
    Indeterminate,
    NotSent,
}
pub enum Effect {
    Read(ReadTicket),
    Save {
        ticket: WriteTicket,
        namespace: SettingsNamespaceView,
        field: SettingsField,
        value: SettingsScalar,
    },
}
struct Editor(String);
impl Drop for Editor {
    fn drop(&mut self) {
        // Owned UTF-8 is overwritten without reading spare capacity; zero preserves UTF-8.
        unsafe {
            let bytes = self.0.as_mut_vec();
            for i in 0..bytes.capacity() {
                bytes.as_mut_ptr().add(i).write_volatile(0);
            }
        }
        compiler_fence(Ordering::SeqCst);
    }
}
enum InputValue {
    Valid(Editor),
    Rejected,
}
#[derive(Clone)]
pub struct Input(Arc<Mutex<Option<InputValue>>>);
impl Input {
    pub fn new(raw: String) -> Self {
        let value = if raw.len() > MAX_INPUT {
            drop(Editor(raw));
            InputValue::Rejected
        } else {
            InputValue::Valid(Editor(raw))
        };
        Self(Arc::new(Mutex::new(Some(value))))
    }
    fn take(&self) -> Option<InputValue> {
        self.0.lock().ok()?.take()
    }
}
#[derive(Clone)]
pub enum Action {
    Refresh(ReadTicket),
    SelectNamespace { ticket: ReadTicket, index: usize },
    Begin { ticket: ReadTicket, field: usize },
    Text { ticket: DraftTicket, input: Input },
    Bool { ticket: DraftTicket, value: bool },
    Review(DraftTicket),
    CancelReview(DraftTicket),
    Confirm(DraftTicket),
    Discard(DraftTicket),
    Search(String),
    ToggleInventory(ReadTicket),
    ToggleNamespaces(ReadTicket),
}
macro_rules! redacted_debug {
    ($($ty:ty),*) => { $(impl std::fmt::Debug for $ty {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("NativePluginSettings([REDACTED])") }
    })* };
}
redacted_debug!(Action, Input, Effect, Snapshot, SaveOutcome, Editor);
struct Draft {
    field: usize,
    raw: Editor,
}
struct Pending {
    ticket: WriteTicket,
    ns: String,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Notice {
    Idle,
    Loaded,
    ReadFailed,
    Invalid,
    Confirmed,
    Conflict,
    Refused,
    Indeterminate,
    NotSent,
    Earlier,
}
impl Notice {
    fn text(self) -> &'static str {
        match self {
            Self::Idle => {
                "Refresh explicitly to load installed plugins and live settings. Opening or selecting a section performs no request."
            }
            Self::Loaded => {
                "Loaded live primitive settings. Unsupported and secret fields are not editable; no raw configuration is displayed."
            }
            Self::ReadFailed => {
                "Plugin settings could not be loaded. Refresh explicitly; no automatic retry."
            }
            Self::Invalid => "Invalid, unchanged or unsupported field value. No write was sent.",
            Self::Confirmed => {
                "Settings mutation acknowledged. This does not confirm plugin reload or runtime behavior."
            }
            Self::Conflict => {
                "Settings changed remotely (revision conflict). Draft cleared. Refresh explicitly before editing; no retry was sent."
            }
            Self::Refused => {
                "Backend reported refusal; persistence may be uncertain. Draft cleared. Refresh explicitly before editing; no automatic retry."
            }
            Self::Indeterminate => {
                "Write outcome indeterminate: it may already have applied. Draft cleared. Refresh explicitly; never retry automatically."
            }
            Self::NotSent => {
                "Write was not sent: worker, queue or lifecycle refused it. Draft cleared. Refresh explicitly."
            }
            Self::Earlier => {
                "An earlier panel's write returned. It is not applied to this panel or draft. Refresh explicitly; no automatic retry."
            }
        }
    }
}
pub struct Controller {
    open: bool,
    epoch: u64,
    panel: u64,
    serial: u64,
    description: u64,
    editor: u64,
    draft_revision: u64,
    snapshot: Option<Snapshot>,
    fresh: bool,
    selected: Option<usize>,
    draft: Option<Draft>,
    review: Option<DraftTicket>,
    reading: Option<ReadTicket>,
    pending: Option<Pending>,
    search: Editor,
    inventory_open: bool,
    namespaces_open: bool,
    notice: Notice,
}
impl Default for Controller {
    fn default() -> Self {
        Self {
            open: false,
            epoch: 0,
            panel: 0,
            serial: 0,
            description: 0,
            editor: 0,
            draft_revision: 0,
            snapshot: None,
            fresh: false,
            selected: None,
            draft: None,
            review: None,
            reading: None,
            pending: None,
            search: Editor(String::new()),
            inventory_open: false,
            namespaces_open: false,
            notice: Notice::Idle,
        }
    }
}
impl Controller {
    pub fn read_ticket(&self) -> ReadTicket {
        ReadTicket {
            epoch: self.epoch,
            panel: self.panel,
            serial: self.serial,
        }
    }
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }
    pub fn enter(&mut self, epoch: u64) {
        self.leave();
        self.open = true;
        self.epoch = epoch;
    }
    pub fn leave(&mut self) {
        self.clear_draft();
        self.open = false;
        self.panel += 1;
        self.serial += 1;
        self.description += 1;
        self.snapshot = None;
        self.selected = None;
        self.fresh = false;
        self.reading = None;
        self.search = Editor(String::new());
        self.inventory_open = false;
        self.namespaces_open = false;
        self.notice = Notice::Idle;
        // Closing is not cancellation: an admitted write can still complete.
    }
    pub fn adopt_epoch(&mut self, epoch: u64) {
        if epoch == self.epoch {
            return;
        }
        let open = self.open;
        self.leave();
        self.open = open;
        self.epoch = epoch;
        if self.pending.take().is_some() {
            self.notice = Notice::Indeterminate;
        }
    }
    pub fn disconnect(&mut self) {
        let open = self.open;
        self.leave();
        self.open = open;
        self.epoch = 0;
        self.notice = if self.pending.take().is_some() {
            Notice::Indeterminate
        } else {
            Notice::ReadFailed
        };
    }
    fn clear_draft(&mut self) {
        self.draft = None;
        self.review = None;
        self.editor += 1;
        self.draft_revision += 1;
    }
    fn allowed(&self, c: Context) -> bool {
        self.open && c.visible && c.enabled && c.epoch == self.epoch && self.epoch != 0
    }
    fn field(&self) -> Option<(&SettingsNamespaceView, &SettingsField)> {
        let ns = self
            .snapshot
            .as_ref()?
            .settings
            .namespaces
            .get(self.selected?)?;
        let field = ns.fields.get(self.draft.as_ref()?.field)?;
        Some((ns, field))
    }
    pub fn draft_ticket(&self) -> Option<DraftTicket> {
        Some(DraftTicket {
            epoch: self.epoch,
            panel: self.panel,
            description: self.description,
            namespace: self.selected?,
            field: self.draft.as_ref()?.field,
            editor: self.editor,
            revision: self.draft_revision,
        })
    }
    fn editor_matches(&self, t: DraftTicket) -> bool {
        self.draft_ticket().is_some_and(|now| {
            DraftTicket {
                revision: t.revision,
                ..now
            } == t
        })
    }
    fn editable(&self) -> bool {
        self.fresh
            && self.reading.is_none()
            && !self.pending()
            && self.snapshot.as_ref().is_some_and(|s| {
                s.settings.writable
                    && self
                        .selected
                        .and_then(|i| s.settings.namespaces.get(i))
                        .is_some_and(|ns| ns.auto_generate)
            })
    }
    pub fn handle(&mut self, action: Action, context: Context) -> Option<Effect> {
        // Always consume stale input holders, even when hidden, stopped or smoke-fenced.
        if let Action::Text { ticket, input } = &action {
            let value = input.take();
            if !self.allowed(context)
                || !self.editable()
                || self.review.is_some()
                || !self.editor_matches(*ticket)
            {
                return None;
            }
            match value {
                Some(InputValue::Valid(raw)) => {
                    self.draft.as_mut()?.raw = raw;
                    self.draft_revision += 1;
                    self.notice = Notice::Loaded;
                }
                Some(InputValue::Rejected) => {
                    self.clear_draft();
                    self.notice = Notice::Invalid;
                }
                None => {}
            }
            return None;
        }
        if !self.allowed(context) {
            return None;
        }
        match action {
            Action::Search(raw) if raw.len() <= 128 => {
                self.search = Editor(raw);
            }
            Action::ToggleInventory(ticket) if ticket == self.read_ticket() => {
                self.inventory_open = !self.inventory_open;
            }
            Action::ToggleNamespaces(ticket) if ticket == self.read_ticket() && !self.pending() => {
                self.namespaces_open = !self.namespaces_open;
            }
            Action::Refresh(ticket)
                if ticket == self.read_ticket() && !self.pending() && self.reading.is_none() =>
            {
                self.clear_draft();
                self.fresh = false;
                self.snapshot = None;
                self.selected = None;
                self.description += 1;
                self.serial += 1;
                let ticket = self.read_ticket();
                self.reading = Some(ticket);
                return Some(Effect::Read(ticket));
            }
            Action::SelectNamespace { ticket, index }
                if ticket == self.read_ticket() && !self.pending() && self.reading.is_none() =>
            {
                if self
                    .snapshot
                    .as_ref()
                    .is_some_and(|s| index < s.settings.namespaces.len())
                {
                    self.clear_draft();
                    self.selected = Some(index);
                    self.namespaces_open = false;
                    self.serial += 1;
                }
            }
            Action::Begin { ticket, field }
                if ticket == self.read_ticket() && self.editable() && self.draft.is_none() =>
            {
                let ns = self
                    .snapshot
                    .as_ref()?
                    .settings
                    .namespaces
                    .get(self.selected?)?;
                let entry = ns.fields.get(field)?;
                if !safe_field(entry) {
                    self.notice = Notice::Invalid;
                    return None;
                }
                let value = entry.value.as_ref()?;
                if !valid(entry, value) {
                    self.notice = Notice::Invalid;
                    return None;
                }
                let raw = match value {
                    SettingsScalar::Bool(v) => v.to_string(),
                    SettingsScalar::Number(v) => v.to_string(),
                    SettingsScalar::String(v) => v.clone(),
                };
                self.clear_draft();
                self.draft = Some(Draft {
                    field,
                    raw: Editor(raw),
                });
            }
            Action::Bool { ticket, value }
                if self.editable() && self.review.is_none() && self.editor_matches(ticket) =>
            {
                if matches!(self.field()?.1.kind, SettingsFieldKind::Bool) {
                    self.draft.as_mut()?.raw = Editor(value.to_string());
                    self.draft_revision += 1;
                }
            }
            Action::Review(ticket) if self.editable() && self.draft_ticket() == Some(ticket) => {
                if self.proposed().is_some() {
                    // A fresh review generation prevents a canceled review's Confirm replay.
                    self.draft_revision += 1;
                    self.review = self.draft_ticket();
                } else {
                    self.notice = Notice::Invalid;
                }
            }
            Action::CancelReview(ticket) if self.review == Some(ticket) => {
                self.review = None;
            }
            Action::Discard(ticket) if self.editor_matches(ticket) && !self.pending() => {
                self.clear_draft();
            }
            Action::Confirm(ticket)
                if self.editable()
                    && self.review == Some(ticket)
                    && self.draft_ticket() == Some(ticket) =>
            {
                let value = self.proposed()?;
                let (namespace, field) = self.field()?;
                let namespace = namespace.clone();
                let field = field.clone();
                self.serial += 1;
                let write = WriteTicket {
                    epoch: self.epoch,
                    panel: self.panel,
                    serial: self.serial,
                    description: self.description,
                    namespace: self.selected?,
                    expected_revision: namespace.revision,
                    draft_revision: self.draft_revision,
                };
                self.clear_draft();
                self.fresh = false;
                self.pending = Some(Pending {
                    ticket: write,
                    ns: namespace.ns.clone(),
                });
                return Some(Effect::Save {
                    ticket: write,
                    namespace,
                    field,
                    value,
                });
            }
            _ => {}
        }
        None
    }
    fn proposed(&self) -> Option<SettingsScalar> {
        let (_, field) = self.field()?;
        let raw = &self.draft.as_ref()?.raw.0;
        let value = match field.kind {
            SettingsFieldKind::Bool => SettingsScalar::Bool(raw.parse().ok()?),
            SettingsFieldKind::Number { .. } => SettingsScalar::Number(raw.parse().ok()?),
            SettingsFieldKind::String { .. } => SettingsScalar::String(raw.clone()),
        };
        (safe_field(field) && valid(field, &value) && field.value.as_ref() != Some(&value))
            .then_some(value)
    }
    pub fn loaded(&mut self, ticket: ReadTicket, result: Result<Snapshot, ReadFailure>) {
        if !self.open
            || self.reading != Some(ticket)
            || ticket.epoch != self.epoch
            || ticket.panel != self.panel
        {
            return;
        }
        self.reading = None;
        self.clear_draft();
        self.description += 1;
        match result {
            Ok(snapshot) if safe_snapshot(&snapshot) => {
                self.selected = (!snapshot.settings.namespaces.is_empty()).then_some(0);
                self.snapshot = Some(snapshot);
                self.fresh = true;
                self.notice = Notice::Loaded;
            }
            _ => {
                self.snapshot = None;
                self.selected = None;
                self.fresh = false;
                self.notice = Notice::ReadFailed;
            }
        }
    }
    pub fn saved(&mut self, ticket: WriteTicket, result: SaveOutcome) {
        if self.pending.as_ref().is_none_or(|p| p.ticket != ticket) {
            return;
        }
        let pending = self.pending.take().expect("matching receipt");
        self.fresh = false;
        if !self.open
            || self.epoch != ticket.epoch
            || self.panel != ticket.panel
            || self.description != ticket.description
        {
            self.notice = Notice::Earlier;
            return;
        }
        self.notice = match result {
            SaveOutcome::Confirmed(namespace)
                if namespace.ns == pending.ns
                    && namespace.revision > ticket.expected_revision
                    && safe_namespace(&namespace) =>
            {
                if let Some(old) = self
                    .snapshot
                    .as_mut()
                    .and_then(|s| s.settings.namespaces.get_mut(ticket.namespace))
                {
                    if old.ns == namespace.ns {
                        *old = namespace;
                        self.description += 1;
                        self.fresh = true;
                        Notice::Confirmed
                    } else {
                        Notice::Indeterminate
                    }
                } else {
                    Notice::Indeterminate
                }
            }
            SaveOutcome::Confirmed(_) | SaveOutcome::Indeterminate => Notice::Indeterminate,
            SaveOutcome::Conflict => Notice::Conflict,
            SaveOutcome::Refused => Notice::Refused,
            SaveOutcome::NotSent => Notice::NotSent,
        };
    }
    pub fn view(&self, context: Context) -> Element<'_, Action> {
        let allowed = self.allowed(context);
        let mut body = column![label("Plugins", TEXT).size(24), label("Installed inventory & live settings", ACCENT).size(16),
            label("Inventory is informational. This panel does not install, enable or disable plugins. Only supported nonsecret live primitive fields can be changed.", MUTED),
            button(if self.reading.is_some() { "Loading…" } else { "Refresh plugins & live settings" })
                .padding([10,14]).style(design::button_style)
                .on_press_maybe((allowed && self.reading.is_none() && !self.pending()).then_some(Action::Refresh(self.read_ticket()))),
            label(self.notice.text(), if matches!(self.notice,Notice::Conflict|Notice::Indeterminate|Notice::Invalid|Notice::ReadFailed) { DANGER } else { MUTED })
        ].spacing(16);
        if self.pending() {
            body = body.push(label("Write submitted; awaiting actual acknowledgment. Closing or switching sections cannot undo it.", DANGER));
        }
        if !allowed {
            body = body.push(label("Read-only: settings page, backend lifecycle or keyless smoke does not permit requests.", DANGER));
        }
        let Some(snapshot) = &self.snapshot else {
            return body.into();
        };
        body = body.push(
            button(label(
                format!(
                    "Configured plugins · {} · {}",
                    snapshot.inventory.entries.len(),
                    if self.inventory_open {
                        "Hide inventory"
                    } else {
                        "Show inventory"
                    }
                ),
                ACCENT,
            ))
            .padding([8, 12])
            .style(design::button_style)
            .on_press_maybe(allowed.then_some(Action::ToggleInventory(self.read_ticket()))),
        );
        if self.inventory_open {
            let search = text_input("Search installed plugin names", &self.search.0)
                .padding(10)
                .style(design::input_style);
            body = body.push(if allowed {
                search.on_input(Action::Search)
            } else {
                search
            });
            body = body.push(label(
                format!(
                    "Configured plugins · {} entries",
                    snapshot.inventory.entries.len()
                ),
                ACCENT,
            ));
            let query = self.search.0.to_lowercase();
            for entry in snapshot
                .inventory
                .entries
                .iter()
                .filter(|e| {
                    query.is_empty()
                        || e.entry_id.to_lowercase().contains(&query)
                        || e.module_name.to_lowercase().contains(&query)
                })
                .take(80)
            {
                body = body.push(
                    label(
                        format!(
                            "{} · {} · configured {} · {:?}",
                            short(&entry.entry_id),
                            short(&entry.module_name),
                            if entry.enabled { "enabled" } else { "disabled" },
                            entry.fiber_phase
                        ),
                        MUTED,
                    )
                    .size(12),
                );
            }
        }
        body = body.push(
            button(label(
                format!(
                    "Live settings · {} namespaces · {}",
                    snapshot.settings.namespaces.len(),
                    if self.namespaces_open {
                        "Hide choices"
                    } else {
                        "Change namespace"
                    }
                ),
                ACCENT,
            ))
            .padding([8, 12])
            .style(design::button_style)
            .on_press_maybe(
                (allowed && !self.pending())
                    .then_some(Action::ToggleNamespaces(self.read_ticket())),
            ),
        );
        if self.namespaces_open {
            for (index, ns) in snapshot.settings.namespaces.iter().enumerate() {
                body = body.push(
                    button(label(short(&ns.ns), TEXT))
                        .padding([8, 12])
                        .style(move |theme, status| {
                            design::navigation(theme, status, self.selected == Some(index))
                        })
                        .on_press_maybe(
                            (allowed && !self.pending() && self.reading.is_none()).then_some(
                                Action::SelectNamespace {
                                    ticket: self.read_ticket(),
                                    index,
                                },
                            ),
                        ),
                );
            }
        }
        let Some(ns) = self
            .selected
            .and_then(|i| snapshot.settings.namespaces.get(i))
        else {
            return body.push(label("No live settings namespace available. Inventory entries without descriptors remain read-only.", MUTED)).into();
        };
        body = body.push(
            label(
                format!("{} · revision {}", short(&ns.ns), ns.revision),
                TEXT,
            )
            .size(17),
        );
        if !ns.auto_generate {
            body = body.push(label("Read-only: this namespace opts out of autogenerated settings and requires its custom interface. No draft or write is available.", MUTED));
        }
        if ns.unsupported_fields > 0 || ns.secret_fields > 0 {
            body = body.push(label(format!("{} unsupported fields read-only; {} sensitive fields excluded. No hidden values are round-tripped.",ns.unsupported_fields,ns.secret_fields), MUTED));
        }
        // Focus the single active edit; discarding or settling restores the field list.
        let active = self.draft.as_ref().map(|d| d.field);
        for index in (0..ns.fields.len()).filter(|i| active.is_none_or(|active| *i == active)) {
            let field = &ns.fields[index];
            let selected = self.draft.as_ref().is_some_and(|d| d.field == index);
            let mut card = column![label(short(&field.label), TEXT)].spacing(10);
            if selected {
                let ticket = self.draft_ticket().expect("visible draft");
                let draft = self.draft.as_ref().expect("visible draft");
                let input_allowed = allowed && self.editable() && self.review.is_none();
                if matches!(field.kind, SettingsFieldKind::Bool) {
                    card = card.push(
                        row![
                            button("True")
                                .padding([8, 12])
                                .style(design::button_style)
                                .on_press_maybe(input_allowed.then_some(Action::Bool {
                                    ticket,
                                    value: true
                                })),
                            button("False")
                                .padding([8, 12])
                                .style(design::button_style)
                                .on_press_maybe(input_allowed.then_some(Action::Bool {
                                    ticket,
                                    value: false
                                })),
                            label(&draft.raw.0, TEXT)
                        ]
                        .spacing(12),
                    );
                } else {
                    let input = text_input("New field value", &draft.raw.0)
                        .padding(12)
                        .style(design::input_style);
                    card = card.push(if input_allowed {
                        input.on_input(move |raw| Action::Text {
                            ticket,
                            input: Input::new(raw),
                        })
                    } else {
                        input
                    });
                }
                if self.review.is_some() {
                    card = card.push(label(format!("{} / {} · reviewed revision {}", ns.ns, field.path.join(" / "), ns.revision), ACCENT).size(12))
                        .push(label("Confirm this single field write against the loaded namespace revision. Other fields and secret values will not be replaced. Runtime reload success is not implied.",DANGER))
                        .push(button("Confirm setting write").padding([10,14]).style(design::primary_button).on_press_maybe((allowed && self.editable()).then_some(Action::Confirm(ticket))))
                        .push(button("Cancel review").padding([8,12]).style(design::button_style).on_press_maybe(allowed.then_some(Action::CancelReview(ticket))));
                } else {
                    card = card.push(
                        button("Review change")
                            .padding([10, 14])
                            .style(design::primary_button)
                            .on_press_maybe(
                                (allowed && self.editable() && self.proposed().is_some())
                                    .then_some(Action::Review(ticket)),
                            ),
                    );
                }
                card = card.push(
                    button("Discard draft")
                        .padding([8, 12])
                        .style(design::button_style)
                        .on_press_maybe(allowed.then_some(Action::Discard(ticket))),
                );
            } else {
                card = card
                    .push(label(
                        field
                            .value
                            .as_ref()
                            .map(display)
                            .unwrap_or_else(|| "Value unavailable · read-only".into()),
                        MUTED,
                    ))
                    .push(
                        button("Edit field")
                            .padding([8, 12])
                            .style(design::button_style)
                            .on_press_maybe(
                                (allowed
                                    && self.editable()
                                    && self.draft.is_none()
                                    && safe_field(field)
                                    && field.value.as_ref().is_some_and(|v| valid(field, v)))
                                .then_some(Action::Begin {
                                    ticket: self.read_ticket(),
                                    field: index,
                                }),
                            ),
                    );
            }
            body = body.push(
                container(card)
                    .padding(14)
                    .width(Length::Fill)
                    .style(|_| design::card(design::OVERLAY, 12.0)),
            );
        }
        body.into()
    }
}
fn short(raw: &str) -> String {
    raw.chars().take(160).collect()
}
fn display(value: &SettingsScalar) -> String {
    match value {
        SettingsScalar::Bool(v) => v.to_string(),
        SettingsScalar::Number(v) => v.to_string(),
        SettingsScalar::String(v) => short(v),
    }
}
fn sensitive(segment: &str) -> bool {
    let key: String = segment
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    [
        "secret",
        "password",
        "credential",
        "authorization",
        "apikey",
        "token",
        "passwd",
        "cookie",
        "auth",
        "proto",
        "constructor",
        "prototype",
        "jsexpr",
        "preset",
        "restrict",
        "guard",
        "permission",
        "approval",
        "sandbox",
        "policy",
        "privatekey",
        "accesskey",
    ]
    .iter()
    .any(|word| key.contains(word))
        || key == "token"
}
fn safe_field(field: &SettingsField) -> bool {
    !field.path.is_empty()
        && field.path.len() <= 8
        && field
            .path
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 128 && !sensitive(p))
        && !sensitive(&field.label)
}
fn valid(field: &SettingsField, value: &SettingsScalar) -> bool {
    match (&field.kind, value) {
        (SettingsFieldKind::Bool, SettingsScalar::Bool(_)) => true,
        (SettingsFieldKind::Number { min, max, integer }, SettingsScalar::Number(v)) => {
            v.is_finite()
                && min.is_none_or(|m| m.is_finite() && *v >= m)
                && max.is_none_or(|m| m.is_finite() && *v <= m)
                && (!integer || (v.fract() == 0.0 && v.abs() <= SAFE_INTEGER))
        }
        (SettingsFieldKind::String { max_length }, SettingsScalar::String(v)) => {
            v.len() <= MAX_INPUT && v.len() <= *max_length
        }
        _ => false,
    }
}
fn safe_namespace(ns: &SettingsNamespaceView) -> bool {
    !ns.ns.is_empty()
        && ns.ns.len() <= 256
        && ns.fields.len() <= MAX_FIELDS
        && ns
            .fields
            .iter()
            .all(|f| safe_field(f) && f.value.as_ref().is_none_or(|v| valid(f, v)))
        && ns
            .fields
            .iter()
            .enumerate()
            .all(|(i, f)| !ns.fields[..i].iter().any(|old| old.path == f.path))
}
fn safe_snapshot(snapshot: &Snapshot) -> bool {
    snapshot.inventory.entries.len() <= MAX_ENTRIES
        && snapshot.settings.namespaces.len() <= MAX_NAMESPACES
        && snapshot.inventory.entries.iter().all(|e| {
            !e.entry_id.is_empty() && e.entry_id.len() <= 256 && e.module_name.len() <= 256
        })
        && snapshot.settings.namespaces.iter().all(safe_namespace)
        && snapshot
            .settings
            .namespaces
            .iter()
            .enumerate()
            .all(|(i, ns)| {
                !snapshot.settings.namespaces[..i]
                    .iter()
                    .any(|old| old.ns == ns.ns)
            })
}
#[cfg(test)]
#[path = "plugins_tests.rs"]
mod tests;
