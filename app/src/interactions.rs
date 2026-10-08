//! Live Remote Events only. Agent identity is never inferred from a Session id.
use crate::design;
use crate::ui::{ACCENT, DANGER, MUTED, SURFACE, TEXT, label};
use dsh_native_transport::dto::{
    AgentId, ApprovalOutcome, QuestionAnswer, QuestionAnswerItem, RemoteEventId, ToolCallId,
    WaterfallKind,
};
use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Element, Length};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const MAX_PENDING: usize = 64;
const MAX_REQUEST: usize = 32 * 1024;
const MAX_RETAINED: usize = 512 * 1024;
const MAX_CUSTOM: usize = 2048;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Key {
    pub epoch: u64,
    pub event_id: RemoteEventId,
    pub serial: u64,
}
#[derive(Clone)]
pub enum Reply {
    Approval(ApprovalOutcome),
    Question(QuestionAnswer),
    CancelQuestion,
}
#[derive(Clone)]
pub struct Submission {
    pub key: Key,
    pub attempt: u64,
    pub reply: Reply,
}
#[derive(Clone)]
pub enum Action {
    List,
    Open(Key),
    Close,
    Approval(Key, ApprovalOutcome),
    Submit(Key),
    CancelQuestion(Key),
    Toggle {
        key: Key,
        question: usize,
        option: usize,
    },
    Custom {
        key: Key,
        question: usize,
        text: String,
    },
    Skip {
        key: Key,
        question: usize,
    },
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Approval {
    tool_name: String,
    call_id: Option<ToolCallId>,
    reason: Option<String>,
    display_reason: Option<BTreeMap<String, String>>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Questions {
    questions: Vec<Question>,
    wait: Option<Wait>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Wait {
    call_id: ToolCallId,
    timed: Option<bool>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Question {
    id: String,
    question: String,
    header: Option<String>,
    detail: Option<String>,
    options: Option<Vec<OptionItem>>,
    multi_select: Option<bool>,
    intent: Option<Value>,
}
#[derive(Clone, Deserialize)]
struct OptionItem {
    label: String,
    description: Option<String>,
}
#[derive(Clone)]
enum Payload {
    Approval(Approval),
    Question(Questions),
    Unsupported {
        reason: &'static str,
        preview: String,
    },
}
#[derive(Default)]
struct Draft {
    selected: BTreeSet<usize>,
    custom: String,
    skipped: bool,
}
enum Phase {
    Ready,
    Submitting(u64),
    Disconnected,
}
struct Pending {
    key: Key,
    agent: AgentId,
    payload: Payload,
    drafts: Vec<Draft>,
    phase: Phase,
    error: Option<String>,
    bytes: usize,
}
#[derive(Default)]
pub struct Decisions {
    pending: BTreeMap<RemoteEventId, Pending>,
    active: Option<Key>,
    list_open: bool,
    serial: u64,
    attempt: u64,
}
fn unsupported(value: &Value, reason: &'static str) -> Payload {
    Payload::Unsupported {
        reason,
        preview: crate::reducer::bounded(&crate::reducer::safe_json(value), 8192),
    }
}
fn nonempty(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max
}
fn map_request(kind: &WaterfallKind, value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    if object.contains_key("agent") || object.contains_key("signal") {
        return false;
    }
    match kind {
        WaterfallKind::Approval => object.get("displayReason").is_none_or(Value::is_object),
        WaterfallKind::Question => {
            if !object.get("wait").is_none_or(Value::is_object) {
                return false;
            }
            let Some(questions) = object.get("questions").and_then(Value::as_array) else {
                return false;
            };
            questions.iter().all(|item| {
                let Some(item) = item.as_object() else {
                    return false;
                };
                if !item.get("intent").is_none_or(Value::is_object) {
                    return false;
                }
                item.get("options").is_none_or(|options| {
                    options
                        .as_array()
                        .is_some_and(|options| options.iter().all(Value::is_object))
                })
            })
        }
    }
}
fn decode(kind: WaterfallKind, value: &Value) -> Payload {
    if !map_request(&kind, value) {
        return unsupported(
            value,
            "Invalid request JSON objects or reserved agent/signal fields; no reply sent",
        );
    }
    if serde_json::to_vec(value).map_or(true, |v| v.len() > MAX_REQUEST) {
        return unsupported(
            value,
            "Request exceeds the native 32 KiB limit; no reply sent",
        );
    }
    match kind {
        WaterfallKind::Approval => {
            let Ok(mut request) = serde_json::from_value::<Approval>(value.clone()) else {
                return unsupported(value, "Malformed approval request; no reply sent");
            };
            if !nonempty(&request.tool_name, 256) {
                return unsupported(value, "Invalid approval tool name; no reply sent");
            }
            if let Some(translations) = &mut request.display_reason {
                translations.retain(|locale, _| locale == "en");
            }
            Payload::Approval(request)
        }
        WaterfallKind::Question => {
            let Ok(request) = serde_json::from_value::<Questions>(value.clone()) else {
                return unsupported(value, "Malformed question request; no reply sent");
            };
            if request.wait.as_ref().is_some_and(|w| w.timed == Some(true)) {
                return unsupported(
                    value,
                    "Timed foreground questions require an owned claim/deadline adapter: unsupported; no answer, claim or delegation sent",
                );
            }
            if request.questions.is_empty() || request.questions.len() > 8 {
                return unsupported(value, "Native question batch limit is 1–8; no reply sent");
            }
            let mut ids = BTreeSet::new();
            for question in &request.questions {
                if !nonempty(&question.id, 256)
                    || !nonempty(&question.question, 8192)
                    || !ids.insert(&question.id)
                {
                    return unsupported(
                        value,
                        "Question ids/text invalid or duplicated; no reply sent",
                    );
                }
                let options = question.options.as_deref().unwrap_or_default();
                let mut labels = BTreeSet::new();
                if options.len() > 32
                    || options
                        .iter()
                        .any(|o| !nonempty(&o.label, 512) || !labels.insert(&o.label))
                {
                    return unsupported(
                        value,
                        "Question option labels invalid, duplicate or exceed 32 options; no reply sent",
                    );
                }
                if let Some(intent) = &question.intent {
                    if intent.get("kind").and_then(Value::as_str) == Some("plan-review")
                        && (question.detail.as_ref().is_none_or(|s| s.trim().is_empty())
                            || !intent
                                .get("approve")
                                .and_then(Value::as_str)
                                .is_some_and(|s| options.iter().any(|o| o.label == s)))
                    {
                        return unsupported(
                            value,
                            "Plan review has no visible detail or matching approval option; no reply sent",
                        );
                    }
                }
            }
            // Intent affects presentation only; validation needs no retained opaque JSON tree.
            let mut request = request;
            for question in &mut request.questions {
                question.intent = None;
            }
            Payload::Question(request)
        }
    }
}
/// Validated request retained privately by the worker for queued reply admission.
pub(super) struct ReplySpec(Payload);
pub(super) fn reply_spec(kind: WaterfallKind, value: &Value) -> (Option<ReplySpec>, usize) {
    let bytes = serde_json::to_vec(value).map_or(MAX_REQUEST, |bytes| bytes.len().min(MAX_REQUEST));
    let payload = decode(kind, value);
    match payload {
        Payload::Unsupported { .. } => (None, 0),
        payload => (Some(ReplySpec(payload)), bytes),
    }
}
impl ReplySpec {
    pub(super) fn allows(&self, reply: &Reply) -> bool {
        match (&self.0, reply) {
            (Payload::Approval(_), Reply::Approval(outcome)) => {
                !matches!(outcome, ApprovalOutcome::Unavailable)
            }
            (Payload::Question(_), Reply::CancelQuestion) => true,
            (Payload::Question(request), Reply::Question(answer)) => {
                if answer.answers.len() != request.questions.len()
                    || serde_json::to_vec(answer).map_or(true, |bytes| bytes.len() > MAX_REQUEST)
                {
                    return false;
                }
                let mut ids = BTreeSet::new();
                answer.answers.iter().all(|answer| {
                    let Some(question) = request
                        .questions
                        .iter()
                        .find(|question| question.id == answer.id)
                    else {
                        return false;
                    };
                    if !ids.insert(&answer.id)
                        || answer
                            .custom
                            .as_ref()
                            .is_some_and(|text| !nonempty(text, MAX_CUSTOM))
                    {
                        return false;
                    }
                    let mut labels = BTreeSet::new();
                    let offered = question.options.as_deref().unwrap_or_default();
                    answer.selected.iter().all(|label| {
                        labels.insert(label) && offered.iter().any(|option| &option.label == label)
                    }) && (question.multi_select == Some(true)
                        || (answer.selected.len() <= 1
                            && (answer.selected.is_empty() || answer.custom.is_none())))
                })
            }
            _ => false,
        }
    }
}
impl Decisions {
    pub fn len(&self) -> usize {
        self.pending.len()
    }
    #[cfg(test)]
    pub fn add(
        &mut self,
        epoch: u64,
        event_id: RemoteEventId,
        agent: AgentId,
        kind: WaterfallKind,
        value: Value,
    ) -> Result<(), &'static str> {
        let serial = self
            .serial
            .checked_add(1)
            .ok_or("Native decision ticket counter exhausted")?;
        self.add_owned(
            Key {
                epoch,
                event_id,
                serial,
            },
            agent,
            kind,
            value,
        )
    }
    /// Production tickets originate in the worker; UI/fixture counter is not authority.
    pub fn add_owned(
        &mut self,
        key: Key,
        agent: AgentId,
        kind: WaterfallKind,
        value: Value,
    ) -> Result<(), &'static str> {
        if key.serial == 0 || key.serial <= self.serial {
            return Err("Stale native worker decision ticket");
        }
        let event_id = key.event_id.clone();
        // A duplicate cannot reset a submitting request or replace its answer draft.
        if self.pending.contains_key(&event_id) {
            return Err("Duplicate live event id; existing request retained, no new reply sent");
        }
        let payload = decode(kind, &value);
        let bytes = serde_json::to_vec(&value).map_or(MAX_REQUEST, |v| v.len().min(MAX_REQUEST))
            + agent.as_str().len()
            + event_id.as_str().len()
            + 4096
            + match &payload {
                Payload::Question(q) => q.questions.len() * (MAX_CUSTOM + 1024),
                _ => 0,
            };
        if self.pending.len() >= MAX_PENDING
            || self.pending.values().map(|p| p.bytes).sum::<usize>() + bytes > MAX_RETAINED
        {
            return Err("Native pending request capacity reached; no reply or delegation sent");
        }
        self.serial = key.serial;
        let drafts = match &payload {
            Payload::Question(q) => (0..q.questions.len()).map(|_| Draft::default()).collect(),
            _ => vec![],
        };
        self.pending.insert(
            event_id,
            Pending {
                key,
                agent,
                payload,
                drafts,
                phase: Phase::Ready,
                error: None,
                bytes,
            },
        );
        Ok(())
    }
    pub fn cancel(&mut self, event_id: &RemoteEventId) {
        self.pending.remove(event_id);
        if self
            .active
            .as_ref()
            .is_some_and(|key| &key.event_id == event_id)
        {
            self.active = None;
        }
    }
    pub fn disconnect(&mut self) {
        for request in self.pending.values_mut() {
            request.phase = Phase::Disconnected;
            request.error = Some(
                "Connection ended; replies disabled. Any unacknowledged submission may be indeterminate; no automatic retry or delegation."
                    .into(),
            );
        }
    }
    pub fn fail(&mut self, key: &Key, attempt: u64, error: String) {
        if let Some(p) = self.pending.get_mut(&key.event_id) {
            if p.key == *key && matches!(p.phase,Phase::Submitting(a) if a==attempt) {
                p.phase = Phase::Ready;
                p.error = Some(crate::reducer::bounded(&error, 512));
            }
        }
    }
    pub fn acknowledge(&mut self, key: &Key, attempt: u64, result: Result<(), String>) -> bool {
        if !self
            .pending
            .get(&key.event_id)
            .is_some_and(|p| p.key == *key && matches!(p.phase,Phase::Submitting(a) if a==attempt))
        {
            return false;
        }
        match result {
            Err(error) => self.fail(key, attempt, error),
            Ok(()) => self.cancel(&key.event_id),
        }
        true
    }
    pub fn handle(&mut self, action: Action, epoch: u64, enabled: bool) -> Option<Submission> {
        if matches!(action, Action::Close) {
            self.active = None;
            self.list_open = false;
            return None;
        }
        if matches!(action, Action::List) {
            self.list_open = true;
            self.active = None;
            return None;
        }
        if let Action::Open(key) = &action {
            if self
                .pending
                .get(&key.event_id)
                .is_some_and(|p| p.key == *key)
            {
                self.active = Some(key.clone());
                self.list_open = false;
            }
            return None;
        }
        let key = match &action {
            Action::Approval(k, _) | Action::Submit(k) | Action::CancelQuestion(k) => k,
            Action::Toggle { key, .. } | Action::Custom { key, .. } | Action::Skip { key, .. } => {
                key
            }
            _ => return None,
        }
        .clone();
        let pending = self.pending.get_mut(&key.event_id)?;
        if !enabled
            || key.epoch != epoch
            || pending.key != key
            || !matches!(pending.phase, Phase::Ready)
        {
            return None;
        }
        let reply = match action {
            Action::Approval(_, outcome)
                if matches!(pending.payload, Payload::Approval(_))
                    && !matches!(outcome, ApprovalOutcome::Unavailable) =>
            {
                Some(Reply::Approval(outcome))
            }
            Action::CancelQuestion(_) if matches!(pending.payload, Payload::Question(_)) => {
                Some(Reply::CancelQuestion)
            }
            Action::Submit(_) => match answer(pending) {
                Ok(answer) => Some(Reply::Question(answer)),
                Err(error) => {
                    pending.error = Some(error.into());
                    None
                }
            },
            Action::Toggle {
                question, option, ..
            } => {
                if let Payload::Question(q) = &pending.payload {
                    if let (Some(item), Some(draft)) =
                        (q.questions.get(question), pending.drafts.get_mut(question))
                    {
                        if item.options.as_ref().is_some_and(|o| option < o.len()) {
                            if item.multi_select == Some(true) {
                                if !draft.selected.remove(&option) {
                                    draft.selected.insert(option);
                                }
                            } else {
                                draft.selected.clear();
                                draft.selected.insert(option);
                                draft.custom.clear();
                            }
                            draft.skipped = false;
                            pending.error = None;
                        }
                    }
                }
                None
            }
            Action::Custom { question, text, .. } => {
                if let Payload::Question(q) = &pending.payload {
                    if let (Some(item), Some(draft)) =
                        (q.questions.get(question), pending.drafts.get_mut(question))
                    {
                        if text.len() <= MAX_CUSTOM {
                            draft.custom = text;
                            if item.multi_select != Some(true) {
                                draft.selected.clear();
                            }
                            draft.skipped = false;
                            pending.error = None;
                        } else {
                            pending.error = Some(
                                "Custom answer exceeds 2 KiB; last valid input retained".into(),
                            );
                        }
                    }
                }
                None
            }
            Action::Skip { question, .. } => {
                if let Some(draft) = pending.drafts.get_mut(question) {
                    *draft = Draft {
                        skipped: true,
                        ..Default::default()
                    };
                    pending.error = None;
                }
                None
            }
            _ => None,
        }?;
        let Some(attempt) = self.attempt.checked_add(1) else {
            pending.error = Some("Native decision attempt counter exhausted; no reply sent".into());
            return None;
        };
        self.attempt = attempt;
        pending.phase = Phase::Submitting(self.attempt);
        pending.error = None;
        Some(Submission {
            key,
            attempt: self.attempt,
            reply,
        })
    }
    pub fn cards(&self, enabled: bool) -> Element<'_, Action> {
        column![
            button(label(
                format!("Review {} pending human decision(s)", self.pending.len()),
                TEXT
            ))
            .padding([10, 14])
            .style(design::button_style)
            .on_press(Action::List),
            label(
                if enabled {
                    "No automatic answer, delegation or permission change."
                } else {
                    "Replies disabled during smoke or unavailable transport."
                },
                MUTED
            )
        ]
        .spacing(6)
        .into()
    }
    /// Read-only presence check; avoids building a widget tree for layout/motion guards.
    pub fn is_open(&self) -> bool {
        self.list_open
            || self.active.as_ref().is_some_and(|key| {
                self.pending
                    .get(&key.event_id)
                    .is_some_and(|pending| pending.key == *key)
            })
    }
    pub fn dialog(&self, enabled: bool) -> Option<Element<'_, Action>> {
        if self.list_open {
            let header = row![
                container(label("Pending decisions", TEXT).size(26)).width(Length::Fill),
                button("Back")
                    .padding([10, 14])
                    .style(design::button_style)
                    .on_press(Action::Close)
            ]
            .spacing(12);
            let mut list = column![].spacing(12);
            for p in self.pending.values() {
                let name = match &p.payload {
                    Payload::Approval(a) => format!("Approval · {}", a.tool_name),
                    Payload::Question(q) => format!("Questions · {} item(s)", q.questions.len()),
                    Payload::Unsupported { .. } => "Unsupported request".into(),
                };
                list = list.push(
                    button(text(name).font(crate::ui::FONT))
                        .padding(14)
                        .style(design::button_style)
                        .on_press(Action::Open(p.key.clone()))
                        .width(Length::Fill),
                );
            }
            return Some(dialog_shell(
                header.into(),
                list.into(),
                label("Opening a decision never answers it.", MUTED).into(),
            ));
        }

        let key = self.active.as_ref()?;
        let p = self.pending.get(&key.event_id)?;
        if p.key != *key {
            return None;
        }
        let editable = enabled && matches!(p.phase, Phase::Ready);
        let header = row![
            container(label("Your decision", TEXT).size(26)).width(Length::Fill),
            button("Close panel — keep pending")
                .padding([10, 14])
                .style(design::button_style)
                .on_press(Action::Close)
        ]
        .spacing(12);
        let mut footer = column![].spacing(10);
        let mut body = column![
            label(format!("Live agent: {}", p.agent.as_str()), MUTED),
            label(
                "Agent and Session identities are distinct. This panel is global to this Host.",
                MUTED
            )
        ]
        .spacing(12);
        match &p.payload {
            Payload::Approval(a) => {
                body = body.push(label(format!("Tool: {}", a.tool_name), TEXT));
                if let Some(id) = &a.call_id {
                    body = body.push(label(format!("Exact tool call: {}", id.as_str()), MUTED));
                }
                if let Some(reason) = a
                    .display_reason
                    .as_ref()
                    .and_then(|v| v.get("en"))
                    .or(a.reason.as_ref())
                {
                    body = body.push(label(reason, TEXT));
                }
                body=body.push(label("Allow once grants only this operation; it changes no permission preset or approval policy.",MUTED));
                footer =
                    footer.push(
                        row![
                            button("Allow once")
                                .padding([11, 18])
                                .style(design::primary_button)
                                .width(Length::Fill)
                                .on_press_maybe(editable.then(|| Action::Approval(
                                    key.clone(),
                                    ApprovalOutcome::AllowedOnce
                                ))),
                            button("Reject")
                                .padding([11, 18])
                                .style(design::danger_button)
                                .width(Length::Fill)
                                .on_press_maybe(editable.then(|| Action::Approval(
                                    key.clone(),
                                    ApprovalOutcome::Rejected
                                ))),
                            button("Cancel request")
                                .padding([10, 14])
                                .width(Length::Fill)
                                .style(design::button_style)
                                .on_press_maybe(editable.then(|| {
                                    Action::Approval(key.clone(), ApprovalOutcome::Cancelled)
                                }))
                        ]
                        .spacing(10),
                    );
            }
            Payload::Question(q) => {
                if let Some(wait) = &q.wait {
                    body = body.push(label(
                        format!("Question tool call: {}", wait.call_id.as_str()),
                        MUTED,
                    ));
                }
                for (index, item) in q.questions.iter().enumerate() {
                    body = body
                        .push(label(item.header.as_deref().unwrap_or(&item.id), ACCENT))
                        .push(label(&item.question, TEXT));
                    if let Some(detail) = &item.detail {
                        body = body.push(label(detail, TEXT));
                    }
                    let draft = &p.drafts[index];
                    for (option, offered) in item
                        .options
                        .as_deref()
                        .unwrap_or_default()
                        .iter()
                        .enumerate()
                    {
                        let title = format!(
                            "{} {}",
                            if draft.selected.contains(&option) {
                                "[selected]"
                            } else {
                                "[ ]"
                            },
                            offered.label
                        );
                        body = body.push(
                            button(text(title).font(crate::ui::FONT))
                                .padding(14)
                                .style(design::button_style)
                                .on_press_maybe(editable.then(|| Action::Toggle {
                                    key: key.clone(),
                                    question: index,
                                    option,
                                }))
                                .width(Length::Fill),
                        );
                        if let Some(description) = &offered.description {
                            body = body.push(label(description, MUTED));
                        }
                    }
                    let input = text_input("Custom answer (2 KiB maximum)", &draft.custom)
                        .padding(14)
                        .style(design::input_style)
                        .width(Length::Fill);
                    body = body.push(if editable {
                        let key = key.clone();
                        input.on_input(move |text| Action::Custom {
                            key: key.clone(),
                            question: index,
                            text,
                        })
                    } else {
                        input
                    });
                    body = body.push(
                        button(if draft.skipped {
                            "Skipped explicitly"
                        } else {
                            "Skip this question explicitly"
                        })
                        .padding([10, 14])
                        .style(design::button_style)
                        .on_press_maybe(editable.then(|| Action::Skip {
                            key: key.clone(),
                            question: index,
                        })),
                    );
                    body = body.push(label(
                        if item.multi_select == Some(true) {
                            "Multiple offered labels and custom text may be combined."
                        } else {
                            "Single choice: custom text replaces an offered selection."
                        },
                        MUTED,
                    ));
                }
                footer = footer.push(
                    row![
                        button("Submit complete answer")
                            .padding([11, 18])
                            .width(Length::Fill)
                            .style(design::primary_button)
                            .on_press_maybe(editable.then(|| Action::Submit(key.clone()))),
                        button("Cancel request")
                            .padding([10, 14])
                            .style(design::button_style)
                            .on_press_maybe(editable.then(|| Action::CancelQuestion(key.clone())))
                    ]
                    .spacing(10),
                );
            }
            Payload::Unsupported { reason, preview } => {
                body = body.push(label(*reason, DANGER)).push(label(preview, TEXT));
            }
        }
        footer = footer.push(label(
            phase_notice(p),
            if p.error.is_some() { DANGER } else { MUTED },
        ));
        if let Some(error) = &p.error {
            body = body.push(label(error, DANGER));
        }
        Some(dialog_shell(header.into(), body.into(), footer.into()))
    }
}
fn phase_notice(pending: &Pending) -> &'static str {
    match pending.phase {
        Phase::Submitting(_) => "Reply submitted; awaiting RPC acknowledgment. Request retained.",
        Phase::Disconnected => "Transport ended; replies disabled.",
        Phase::Ready if matches!(pending.payload, Payload::Unsupported { .. }) => {
            "Unsupported request; no answer, claim or delegation sent."
        }
        Phase::Ready if pending.error.is_some() => {
            "Reply not confirmed — review the error in the scrollable request."
        }
        Phase::Ready => "Awaiting your explicit action. No automatic response or delegation.",
    }
}
/// Only request content scrolls; dismiss and explicit reply controls retain fixed layout slots.
fn dialog_shell<'a, R: iced::advanced::text::Renderer + 'a>(
    header: Element<'a, Action, iced::Theme, R>,
    content: Element<'a, Action, iced::Theme, R>,
    footer: Element<'a, Action, iced::Theme, R>,
) -> Element<'a, Action, iced::Theme, R> {
    let sheet = container(
        column![
            header,
            scrollable(content).width(Length::Fill).height(Length::Fill),
            footer
        ]
        .width(Length::Fill)
        .spacing(16)
        .height(Length::Fill),
    )
    .padding(20)
    .width(Length::Fill)
    .height(Length::Fill)
    .max_width(760)
    .max_height(720)
    .style(|_| design::card(SURFACE, 20.0));
    container(
        container(sheet)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .padding(12)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn answer(pending: &Pending) -> Result<QuestionAnswer, &'static str> {
    let Payload::Question(request) = &pending.payload else {
        return Err("Not an answerable question request");
    };
    let mut answers = vec![];
    for (question, draft) in request.questions.iter().zip(&pending.drafts) {
        if draft.custom.len() > MAX_CUSTOM {
            return Err("Custom answer too large");
        }
        let options = question.options.as_deref().unwrap_or_default();
        if draft.selected.iter().any(|index| *index >= options.len()) {
            return Err("Selected option is not offered");
        }
        let custom = draft.custom.trim();
        if !draft.skipped && draft.selected.is_empty() && custom.is_empty() {
            return Err("Answer or explicitly skip every question before submitting");
        }
        if question.multi_select != Some(true)
            && (draft.selected.len() > 1 || (!custom.is_empty() && !draft.selected.is_empty()))
        {
            return Err("Single-select answer cannot combine choices and custom text");
        }
        answers.push(QuestionAnswerItem {
            id: question.id.clone(),
            selected: if draft.skipped {
                vec![]
            } else {
                draft
                    .selected
                    .iter()
                    .map(|index| options[*index].label.clone())
                    .collect()
            },
            custom: (!draft.skipped && !custom.is_empty()).then(|| custom.to_owned()),
        });
    }
    let answer = QuestionAnswer { answers };
    if serde_json::to_vec(&answer).map_or(true, |b| b.len() > MAX_REQUEST) {
        return Err("Complete answer exceeds 32 KiB");
    }
    Ok(answer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    pub(super) fn fixture(name: &str) -> Value {
        serde_json::from_str::<Value>(include_str!("../fixtures/interactions.json")).unwrap()[name]
            .clone()
    }
    pub(super) fn add(state: &mut Decisions, kind: WaterfallKind, name: &str) -> Key {
        let event_id = RemoteEventId::new("event-1").unwrap();
        state
            .add(
                1,
                event_id.clone(),
                AgentId::new("agent-NOT-session").unwrap(),
                kind,
                fixture(name),
            )
            .unwrap();
        state.pending[&event_id].key.clone()
    }
    #[test]
    fn receiving_or_viewing_requests_never_answers() {
        let mut state = Decisions::default();
        let key = add(&mut state, WaterfallKind::Approval, "approval");
        assert!(state.handle(Action::Open(key), 1, true).is_none());
        let _ = state.dialog(true);
        assert!(matches!(
            state.pending.values().next().unwrap().phase,
            Phase::Ready
        ));
    }
    #[test]
    fn explicit_approval_outcomes_remain_pending_until_ack_without_own_cancel() {
        for outcome in [
            ApprovalOutcome::AllowedOnce,
            ApprovalOutcome::Rejected,
            ApprovalOutcome::Cancelled,
        ] {
            let mut state = Decisions::default();
            let key = add(&mut state, WaterfallKind::Approval, "approval");
            let sent = state
                .handle(Action::Approval(key.clone(), outcome), 1, true)
                .unwrap();
            assert!(matches!(sent.reply, Reply::Approval(_)));
            assert_eq!(state.len(), 1);
            state.acknowledge(&key, sent.attempt, Ok(()));
            assert_eq!(state.len(), 0);
        }
    }
    #[test]
    fn failure_keeps_request_and_enables_explicit_retry() {
        let mut state = Decisions::default();
        let key = add(&mut state, WaterfallKind::Approval, "approval");
        let sent = state
            .handle(
                Action::Approval(key.clone(), ApprovalOutcome::Rejected),
                1,
                true,
            )
            .unwrap();
        assert!(
            state
                .handle(
                    Action::Approval(key.clone(), ApprovalOutcome::AllowedOnce),
                    1,
                    true
                )
                .is_none()
        );
        state.acknowledge(&key, sent.attempt, Err("Remote code only".into()));
        assert!(state.pending[&key.event_id].error.is_some());
        assert!(
            state
                .handle(Action::Approval(key, ApprovalOutcome::Rejected), 1, true)
                .is_some()
        );
    }
    #[test]
    fn cancel_before_ack_and_reused_id_ignore_old_completion() {
        let mut state = Decisions::default();
        let old = add(&mut state, WaterfallKind::Approval, "approval");
        let sent = state
            .handle(
                Action::Approval(old.clone(), ApprovalOutcome::AllowedOnce),
                1,
                true,
            )
            .unwrap();
        state.cancel(&old.event_id);
        let new = add(&mut state, WaterfallKind::Approval, "approval");
        state.acknowledge(&old, sent.attempt, Ok(()));
        assert_ne!(old, new);
        assert!(matches!(state.pending[&new.event_id].phase, Phase::Ready));
    }
    #[test]
    fn duplicate_live_id_preserves_submitting_request() {
        let mut state = Decisions::default();
        let key = add(&mut state, WaterfallKind::Approval, "approval");
        let sent = state
            .handle(
                Action::Approval(key.clone(), ApprovalOutcome::AllowedOnce),
                1,
                true,
            )
            .unwrap();
        assert!(
            state
                .add(
                    1,
                    key.event_id.clone(),
                    AgentId::new("other-agent").unwrap(),
                    WaterfallKind::Approval,
                    fixture("approval")
                )
                .is_err()
        );
        assert!(
            matches!(state.pending[&key.event_id].phase,Phase::Submitting(a) if a==sent.attempt)
        );
    }
    #[test]
    fn wrong_epoch_smoke_or_disconnect_cannot_reply() {
        let mut state = Decisions::default();
        let key = add(&mut state, WaterfallKind::Approval, "approval");
        assert!(
            state
                .handle(
                    Action::Approval(key.clone(), ApprovalOutcome::AllowedOnce),
                    2,
                    true
                )
                .is_none()
        );
        assert!(
            state
                .handle(
                    Action::Approval(key.clone(), ApprovalOutcome::AllowedOnce),
                    1,
                    false
                )
                .is_none()
        );
        state.disconnect();
        assert_eq!(state.len(), 1);
        assert!(
            state
                .handle(Action::Approval(key, ApprovalOutcome::AllowedOnce), 1, true)
                .is_none()
        );
    }
    #[test]
    fn questions_encode_exact_ids_labels_multi_custom_and_explicit_skip() {
        let mut state = Decisions::default();
        let key = add(&mut state, WaterfallKind::Question, "questions");
        assert!(state.handle(Action::Submit(key.clone()), 1, true).is_none());
        state.handle(
            Action::Toggle {
                key: key.clone(),
                question: 0,
                option: 0,
            },
            1,
            true,
        );
        state.handle(
            Action::Toggle {
                key: key.clone(),
                question: 1,
                option: 0,
            },
            1,
            true,
        );
        state.handle(
            Action::Toggle {
                key: key.clone(),
                question: 1,
                option: 1,
            },
            1,
            true,
        );
        state.handle(
            Action::Custom {
                key: key.clone(),
                question: 1,
                text: " Extra ".into(),
            },
            1,
            true,
        );
        state.handle(
            Action::Skip {
                key: key.clone(),
                question: 2,
            },
            1,
            true,
        );
        let sent = state.handle(Action::Submit(key), 1, true).unwrap();
        let Reply::Question(answer) = sent.reply else {
            panic!("wrong reply type")
        };
        assert_eq!(
            serde_json::to_value(answer).unwrap(),
            json!({"answers":[{"id":"mode","selected":["Safe"]},{"id":"features","selected":["History","Streaming"],"custom":"Extra"},{"id":"notes","selected":[]}]})
        );
    }
    #[test]
    fn single_select_custom_replaces_choice_and_invalid_index_is_ignored() {
        let mut state = Decisions::default();
        let key = add(&mut state, WaterfallKind::Question, "questions");
        state.handle(
            Action::Toggle {
                key: key.clone(),
                question: 0,
                option: 99,
            },
            1,
            true,
        );
        assert!(state.pending[&key.event_id].drafts[0].selected.is_empty());
        state.handle(
            Action::Toggle {
                key: key.clone(),
                question: 0,
                option: 0,
            },
            1,
            true,
        );
        state.handle(
            Action::Custom {
                key: key.clone(),
                question: 0,
                text: "Custom".into(),
            },
            1,
            true,
        );
        assert!(state.pending[&key.event_id].drafts[0].selected.is_empty());
    }
    #[test]
    fn viewing_closing_listing_and_reopening_preserves_partial_question_drafts() {
        let mut state = Decisions::default();
        let key = add(&mut state, WaterfallKind::Question, "questions");
        for action in [
            Action::Toggle {
                key: key.clone(),
                question: 0,
                option: 0,
            },
            Action::Custom {
                key: key.clone(),
                question: 1,
                text: "PUBLIC partial answer".into(),
            },
            Action::Skip {
                key: key.clone(),
                question: 2,
            },
            Action::Open(key.clone()),
            Action::Close,
            Action::List,
            Action::Open(key.clone()),
        ] {
            assert!(state.handle(action, 1, true).is_none());
            let _ = state.dialog(true);
        }
        let pending = &state.pending[&key.event_id];
        assert!(pending.drafts[0].selected.contains(&0));
        assert_eq!(pending.drafts[1].custom, "PUBLIC partial answer");
        assert!(pending.drafts[2].skipped);
        assert!(matches!(pending.phase, Phase::Ready));
        assert_eq!(state.attempt, 0);
    }
    #[test]
    fn compact_footer_never_interpolates_long_multiline_error_details() {
        let mut state = Decisions::default();
        let key = add(&mut state, WaterfallKind::Approval, "approval");
        let submission = state
            .handle(
                Action::Approval(key.clone(), ApprovalOutcome::AllowedOnce),
                1,
                true,
            )
            .unwrap();
        state.fail(
            &key,
            submission.attempt,
            "PUBLIC failure detail\n".repeat(200),
        );
        let pending = &state.pending[&key.event_id];
        assert!(pending.error.as_ref().unwrap().contains('\n'));
        assert!(!phase_notice(pending).contains('\n'));
        assert!(phase_notice(pending).len() < 80);
        assert!(!phase_notice(pending).contains("PUBLIC failure detail"));
        let _ = state.handle(Action::Open(key.clone()), 1, true);
        let _ = state.dialog(true);
        assert_eq!(state.len(), 1);
    }
    #[test]
    fn navigation_stays_local_while_reply_is_submitting_or_transport_disconnected() {
        let mut state = Decisions::default();
        let key = add(&mut state, WaterfallKind::Approval, "approval");
        let submission = state
            .handle(
                Action::Approval(key.clone(), ApprovalOutcome::AllowedOnce),
                1,
                true,
            )
            .unwrap();
        for disconnected in [false, true] {
            if disconnected {
                state.disconnect();
            }
            assert!(state.handle(Action::Close, 1, true).is_none());
            assert!(state.handle(Action::Open(key.clone()), 1, true).is_none());
            assert!(state.dialog(true).is_some());
            assert!(
                state
                    .handle(
                        Action::Approval(key.clone(), ApprovalOutcome::Cancelled),
                        1,
                        true
                    )
                    .is_none()
            );
            assert_eq!(state.attempt, submission.attempt);
        }
    }
    #[cfg(debug_assertions)] // Iced supplies the null renderer only in debug builds.
    #[test]
    fn bounded_dialog_layout_reserves_chrome_independently_of_long_content() {
        use iced::advanced::{layout, widget::Tree};
        fn chrome(width: f32, height: f32, content_height: f32) -> [iced::Rectangle; 3] {
            fn split(node: &layout::Node) -> Option<&layout::Node> {
                if node.children().len() == 3 {
                    Some(node)
                } else {
                    node.children().iter().find_map(split)
                }
            }
            let space = |height| -> Element<'static, Action, iced::Theme, ()> {
                iced::widget::Space::new()
                    .width(Length::Fill)
                    .height(Length::Fixed(height))
                    .into()
            };
            let mut shell = dialog_shell(space(44.0), space(content_height), space(88.0));
            let mut tree = Tree::new(shell.as_widget());
            let node = shell.as_widget_mut().layout(
                &mut tree,
                &(),
                &layout::Limits::new(iced::Size::ZERO, iced::Size::new(width, height)),
            );
            let parts = split(&node).unwrap();
            let children = parts.children();
            assert_eq!(children[0].bounds().height, 44.0);
            assert_eq!(children[2].bounds().height, 88.0);
            assert!(children[1].bounds().height > 0.0);
            assert!(children[2].bounds().y + children[2].bounds().height <= parts.bounds().height);
            assert!(node.bounds().width <= width && node.bounds().height <= height);
            [
                children[0].bounds(),
                children[1].bounds(),
                children[2].bounds(),
            ]
        }
        for (width, height) in [(608.0, 448.0), (760.0, 560.0), (1000.0, 800.0)] {
            assert_eq!(chrome(width, height, 1.0), chrome(width, height, 8192.0));
        }
    }
    #[test]
    fn timed_requests_are_explicitly_unsupported_without_claim_or_reply() {
        let mut state = Decisions::default();
        let key = add(&mut state, WaterfallKind::Question, "timed");
        assert!(matches!(
            state.pending[&key.event_id].payload,
            Payload::Unsupported { .. }
        ));
        assert!(state.handle(Action::Submit(key.clone()), 1, true).is_none());
        assert!(state.handle(Action::CancelQuestion(key), 1, true).is_none());
    }
    #[test]
    fn malformed_duplicate_and_invisible_plan_requests_fail_closed() {
        for value in [
            json!({"questions":[]}),
            json!({"questions":[{"id":"x","question":"a"},{"id":"x","question":"b"}]}),
            json!({"questions":[{"id":"x","question":"a","options":[{"label":"Same"},{"label":"Same"}]}]}),
            json!({"questions":[{"id":"x","question":"a","intent":{"kind":"plan-review","approve":"Missing"}}]}),
        ] {
            assert!(matches!(
                decode(WaterfallKind::Question, &value),
                Payload::Unsupported { .. }
            ));
        }
        assert!(matches!(
            decode(WaterfallKind::Question, &fixture("plan")),
            Payload::Question(_)
        ));
    }
    #[test]
    fn complete_retention_and_multibyte_custom_limits_are_enforced() {
        let mut state = Decisions::default();
        let key = add(&mut state, WaterfallKind::Question, "questions");
        state.handle(
            Action::Custom {
                key: key.clone(),
                question: 0,
                text: "last valid".into(),
            },
            1,
            true,
        );
        state.handle(
            Action::Custom {
                key: key.clone(),
                question: 0,
                text: "你".repeat(1000),
            },
            1,
            true,
        );
        assert_eq!(state.pending[&key.event_id].drafts[0].custom, "last valid");
        for index in 2..200 {
            let id = RemoteEventId::new(format!("event-{index}")).unwrap();
            let _ = state.add(
                1,
                id,
                AgentId::new("agent").unwrap(),
                WaterfallKind::Question,
                fixture("questions"),
            );
        }
        assert!(state.pending.len() <= MAX_PENDING);
        assert!(state.pending.values().map(|p| p.bytes).sum::<usize>() <= MAX_RETAINED);
    }
    #[test]
    fn question_cancel_is_explicit_and_panel_close_preserves_request() {
        let mut state = Decisions::default();
        let key = add(&mut state, WaterfallKind::Question, "questions");
        state.handle(Action::Open(key.clone()), 1, true);
        state.handle(Action::Close, 1, true);
        assert_eq!(state.len(), 1);
        assert!(state.active.is_none());
        assert!(matches!(
            state
                .handle(Action::CancelQuestion(key), 1, true)
                .unwrap()
                .reply,
            Reply::CancelQuestion
        ));
    }
}

#[cfg(test)]
mod gateway_ack_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn self_ack_without_cancel_does_not_exhaust_pending_capacity() {
        let mut state = Decisions::default();
        for serial in 0..130 {
            let id = RemoteEventId::new(format!("self-event-{serial}")).unwrap();
            state
                .add(
                    1,
                    id.clone(),
                    AgentId::new("different-agent-id").unwrap(),
                    WaterfallKind::Approval,
                    json!({"toolName":"bash"}),
                )
                .unwrap();
            let key = state.pending[&id].key.clone();
            let sent = state
                .handle(
                    Action::Approval(key.clone(), ApprovalOutcome::Rejected),
                    1,
                    true,
                )
                .unwrap();
            assert_eq!(state.len(), 1);
            assert!(state.acknowledge(&key, sent.attempt, Ok(())));
            assert_eq!(state.len(), 0);
        }
    }
    #[test]
    fn stale_attempt_ack_does_not_close_explicit_retry() {
        let mut state = Decisions::default();
        let id = RemoteEventId::new("event-1").unwrap();
        state
            .add(
                1,
                id.clone(),
                AgentId::new("agent-1").unwrap(),
                WaterfallKind::Approval,
                json!({"toolName":"bash"}),
            )
            .unwrap();
        let key = state.pending[&id].key.clone();
        let first = state
            .handle(
                Action::Approval(key.clone(), ApprovalOutcome::Rejected),
                1,
                true,
            )
            .unwrap();
        state.acknowledge(&key, first.attempt, Err("indeterminate error".into()));
        let second = state
            .handle(
                Action::Approval(key.clone(), ApprovalOutcome::Rejected),
                1,
                true,
            )
            .unwrap();
        assert!(!state.acknowledge(&key, first.attempt, Ok(())));
        assert_eq!(state.len(), 1);
        assert!(state.acknowledge(&key, second.attempt, Ok(())));
        assert_eq!(state.len(), 0);
    }
    #[test]
    fn question_cancel_ack_also_closes_without_host_cancel() {
        let mut state = Decisions::default();
        let id = RemoteEventId::new("event-question").unwrap();
        state
            .add(
                1,
                id.clone(),
                AgentId::new("agent-1").unwrap(),
                WaterfallKind::Question,
                json!({"questions":[{"id":"q","question":"Pick"}]}),
            )
            .unwrap();
        let key = state.pending[&id].key.clone();
        let sent = state
            .handle(Action::CancelQuestion(key.clone()), 1, true)
            .unwrap();
        assert!(state.acknowledge(&key, sent.attempt, Ok(())));
        assert_eq!(state.len(), 0);
    }
}

#[cfg(test)]
mod object_request_tests {
    use super::tests::{add, fixture};
    use super::*;
    use serde_json::json;
    #[test]
    fn positional_struct_arrays_and_reserved_scope_fields_cannot_grant_answer_or_cancel() {
        let cases = [
            (WaterfallKind::Approval, json!(["bash", null, null, null])),
            (
                WaterfallKind::Approval,
                json!({"toolName":"bash","agent":"other-agent"}),
            ),
            (
                WaterfallKind::Approval,
                json!({"toolName":"bash","signal":null}),
            ),
            (
                WaterfallKind::Question,
                json!([[{"id":"q","question":"Proceed?"}],null]),
            ),
            (
                WaterfallKind::Question,
                json!({"questions":[["q","Proceed?",null,null,null,null,null]]}),
            ),
            (
                WaterfallKind::Question,
                json!({"questions":[{"id":"q","question":"Proceed?","options":[["Yes",null]]}]}),
            ),
            (
                WaterfallKind::Question,
                json!({"questions":[{"id":"q","question":"Proceed?"}],"wait":["call-1",false]}),
            ),
        ];
        for (kind, value) in cases {
            let mut state = Decisions::default();
            let id = RemoteEventId::new("malformed").unwrap();
            state
                .add(
                    1,
                    id.clone(),
                    AgentId::new("real-agent").unwrap(),
                    kind,
                    value,
                )
                .unwrap();
            let key = state.pending[&id].key.clone();
            assert!(matches!(
                state.pending[&id].payload,
                Payload::Unsupported { .. }
            ));
            for action in [
                Action::Approval(key.clone(), ApprovalOutcome::AllowedOnce),
                Action::Approval(key.clone(), ApprovalOutcome::Rejected),
                Action::Approval(key.clone(), ApprovalOutcome::Cancelled),
                Action::Submit(key.clone()),
                Action::CancelQuestion(key.clone()),
            ] {
                assert!(state.handle(action, 1, true).is_none());
            }
        }
    }
    #[test]
    fn worker_owned_ticket_gaps_are_allowed_but_zero_or_replayed_serials_are_not() {
        let mut state = Decisions::default();
        let key = Key {
            epoch: 1,
            event_id: RemoteEventId::new("owned").unwrap(),
            serial: 900,
        };
        state
            .add_owned(
                key.clone(),
                AgentId::new("agent").unwrap(),
                WaterfallKind::Approval,
                fixture("approval"),
            )
            .unwrap();
        assert_eq!(state.pending[&key.event_id].key, key);
        state.cancel(&key.event_id);
        assert!(
            state
                .add_owned(
                    key.clone(),
                    AgentId::new("agent").unwrap(),
                    WaterfallKind::Approval,
                    fixture("approval")
                )
                .is_err()
        );
        let zero = Key {
            serial: 0,
            ..key.clone()
        };
        assert!(
            state
                .add_owned(
                    zero,
                    AgentId::new("agent").unwrap(),
                    WaterfallKind::Approval,
                    fixture("approval")
                )
                .is_err()
        );
        state
            .add_owned(
                Key {
                    serial: 1000,
                    ..key
                },
                AgentId::new("agent").unwrap(),
                WaterfallKind::Approval,
                fixture("approval"),
            )
            .unwrap();
    }
    #[test]
    fn exhausted_local_ticket_or_attempt_counter_never_wraps_or_submits() {
        let mut state = Decisions::default();
        state.serial = u64::MAX;
        assert!(
            state
                .add(
                    1,
                    RemoteEventId::new("one").unwrap(),
                    AgentId::new("agent").unwrap(),
                    WaterfallKind::Approval,
                    fixture("approval")
                )
                .is_err()
        );
        assert_eq!(state.len(), 0);
        let mut state = Decisions::default();
        let key = add(&mut state, WaterfallKind::Approval, "approval");
        state.attempt = u64::MAX;
        assert!(
            state
                .handle(
                    Action::Approval(key.clone(), ApprovalOutcome::AllowedOnce),
                    1,
                    true
                )
                .is_none()
        );
        assert!(matches!(state.pending[&key.event_id].phase, Phase::Ready));
    }
    #[test]
    fn unknown_presentation_intent_and_metadata_remain_generic_questions() {
        let value = json!({"questions":[{"id":"q","question":"Pick","futureMetadata":{"v":1},"intent":{"kind":"future-presentation"}}],"futureRequestMetadata":true});
        assert!(matches!(
            decode(WaterfallKind::Question, &value),
            Payload::Question(_)
        ));
    }
}
