//! AT-SPI exposure of this app's own controls through AccessKit.
//!
//! What a client hears must be what is on screen, so a node is published only
//! when its widget was found in the last measured layout *and* its state exists
//! in this frame: a control that is not laid out gets no node, and the composer
//! node disappears while a panel covers it. Every published node therefore
//! carries real bounds from the widget tree, never an estimate, and the widget
//! ids below are the ones `ui.rs` tags on the real widgets.
//!
//! Secrets stay out of the tree by construction: the builder reads only the
//! fields of [`Snapshot`], and the API-key draft, MCP values and Codex callbacks
//! live behind panels whose open state already hides the composer. There is no
//! code path that copies a secret into a node.
//!
//! The platform thread never touches app state. AccessKit asks for a tree on
//! activation and delivers actions on its own thread; both become [`Request`]s on
//! a channel that the app drains as a subscription, so an AT activation goes
//! through the same `Message` handlers as a pointer click. Measuring only happens
//! while a client is connected, which is what keeps an unattended app free of the
//! extra traversal.

use accesskit::{
    Action, ActionData, ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, Node,
    NodeId, Rect, Role, TreeId, TreeInfo, TreeUpdate,
};
use accesskit_unix::Adapter;
use iced::advanced::widget::{Id, Operation, operation};
use iced::{Rectangle, Task};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

/// Bounds measured from the widget tree, keyed by published node.
pub(crate) type Bounds = BTreeMap<NodeId, Rectangle>;

// Node identities are fixed so a client's cached tree survives an update.
pub(crate) const ROOT: NodeId = NodeId(1);
pub(crate) const COMPOSER: NodeId = NodeId(2);
pub(crate) const SEND: NodeId = NodeId(3);
pub(crate) const STOP: NodeId = NodeId(4);
pub(crate) const ATTACH: NodeId = NodeId(5);
pub(crate) const SETTINGS: NodeId = NodeId(6);
pub(crate) const MODEL: NodeId = NodeId(7);
pub(crate) const STATUS: NodeId = NodeId(8);
/// The composer's text run: the child that makes the draft readable.
pub(crate) const COMPOSER_TEXT_RUN: NodeId = NodeId(9);

// The widget ids `ui.rs` tags. Reusing the ids the geometry fixtures already use
// keeps one name per widget; `native-status-line` and `native-chat-composer` are
// added for this tree.
pub(crate) const WIDGET_COMPOSER: &str = "native-chat-composer";
pub(crate) const WIDGET_STATUS: &str = "native-status-line";

/// Every control this build exposes: its node, and the widget that measures it.
const EXPOSED: [(NodeId, &str); 7] = [
    (COMPOSER, WIDGET_COMPOSER),
    (SEND, "native-send-control"),
    (STOP, "native-stop-control"),
    (ATTACH, "native-file-open"),
    (SETTINGS, "native-settings-entry"),
    (MODEL, "native-model-controls"),
    (STATUS, WIDGET_STATUS),
];

/// The window name a client announces for the app.
const WINDOW_NAME: &str = "DeepSeek Harness";

/// One accessibility event for the app thread.
#[derive(Clone, Debug)]
pub(crate) enum Request {
    /// A client connected and asked for the tree.
    Activate,
    /// One action on a published node.
    Action(ActionRequest),
}

/// The one accessibility event source. Its hash is constant, so iced keeps the
/// subscription across updates and the receiver is handed out exactly once.
#[derive(Clone)]
pub(crate) struct Source(Arc<Mutex<Option<UnboundedReceiver<Request>>>>);
impl Source {
    pub(crate) fn new(receiver: UnboundedReceiver<Request>) -> Self {
        Self(Arc::new(Mutex::new(Some(receiver))))
    }
    /// A source that never yields: a run that does not expose a tree still has a
    /// subscription slot, so nothing else has to branch on the option.
    pub(crate) fn disabled() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }
    fn take(&self) -> Option<UnboundedReceiver<Request>> {
        self.0.lock().ok()?.take()
    }
    pub(crate) fn stream(&self) -> SourceStream {
        SourceStream(self.take())
    }
}
impl Hash for Source {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // One source per app: the identity is the type, not the channel address.
        std::any::type_name::<Self>().hash(state);
    }
}
impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("accessibility::Source")
    }
}

/// The receiver as a stream; a second call has nothing left to hand out and ends.
pub(crate) struct SourceStream(Option<UnboundedReceiver<Request>>);
impl futures_util::Stream for SourceStream {
    type Item = Request;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match self.0.as_mut() {
            Some(receiver) => receiver.poll_recv(context),
            None => std::task::Poll::Ready(None),
        }
    }
}

/// The AccessKit adapter plus the flag that says whether anyone is listening.
pub(crate) struct Bridge {
    adapter: Adapter,
    live: Arc<AtomicBool>,
}
impl Bridge {
    /// Create the adapter. The default `async-io` executor is deliberate: the
    /// adapter then owns its thread and cannot fail for want of a tokio context
    /// on the thread that boots the app.
    pub(crate) fn new(sender: UnboundedSender<Request>) -> Self {
        let live = Arc::new(AtomicBool::new(false));
        let adapter = Adapter::new(
            Activate {
                sender: sender.clone(),
                live: Arc::clone(&live),
            },
            Actions { sender },
            Deactivate {
                live: Arc::clone(&live),
            },
        );
        Self { adapter, live }
    }
    /// True while a client is connected; measuring is skipped otherwise.
    pub(crate) fn live(&self) -> bool {
        self.live.load(Ordering::Relaxed)
    }
    /// Publish a full tree. A no-op while no client is connected.
    pub(crate) fn publish(&mut self, update: TreeUpdate) {
        self.adapter.update_if_active(|| update);
    }
    /// Tell the adapter whether the window is focused, so it reports activation
    /// and deactivation rather than leaving a stale focused state.
    pub(crate) fn window_focus(&mut self, focused: bool) {
        self.adapter.update_window_focus_state(focused);
    }
}

struct Activate {
    sender: UnboundedSender<Request>,
    live: Arc<AtomicBool>,
}
impl ActivationHandler for Activate {
    /// Answering `None` is deliberate: the tree needs measured geometry, which
    /// only the app thread can collect, so the platform adapter keeps its
    /// placeholder until this app publishes the real tree.
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        self.live.store(true, Ordering::Relaxed);
        let _ = self.sender.send(Request::Activate);
        None
    }
}

struct Actions {
    sender: UnboundedSender<Request>,
}
impl ActionHandler for Actions {
    fn do_action(&mut self, request: ActionRequest) {
        // The platform thread only forwards; the app decides and answers.
        let _ = self.sender.send(Request::Action(request));
    }
}

struct Deactivate {
    live: Arc<AtomicBool>,
}
impl DeactivationHandler for Deactivate {
    /// Clearing the flag is all this needs to do: the poll then stops, and a
    /// later activation asks for a fresh tree.
    fn deactivate_accessibility(&mut self) {
        self.live.store(false, Ordering::Relaxed);
    }
}

/// What the app should do for one action request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Intent {
    /// Replace the composer draft with this text.
    SetComposer(String),
    /// Put keyboard focus in the composer.
    FocusComposer,
    /// The message this app already uses for the same control.
    Send,
    Stop,
    Attach,
    Settings,
    Model,
}

/// Map one request to an intent. An action this build does not support, a node it
/// does not publish, or a request for another tree does nothing at all, which is
/// what AccessKit requires of an unsupported action.
pub(crate) fn intent(request: &ActionRequest) -> Option<Intent> {
    if request.target_tree != TreeId::ROOT {
        return None;
    }
    match (request.target_node, request.action) {
        (SEND, Action::Click) => Some(Intent::Send),
        (STOP, Action::Click) => Some(Intent::Stop),
        (ATTACH, Action::Click) => Some(Intent::Attach),
        (SETTINGS, Action::Click) => Some(Intent::Settings),
        (MODEL, Action::Click) => Some(Intent::Model),
        // A click on a text field means "put the caret in it", which is what
        // clicking this editor does, and AccessKit only advertises the action
        // interface for a node that offers a click.
        (COMPOSER, Action::Click | Action::Focus) => Some(Intent::FocusComposer),
        (COMPOSER, Action::SetValue) => match request.data.as_ref() {
            Some(ActionData::Value(value)) => Some(Intent::SetComposer(value.to_string())),
            _ => None,
        },
        _ => None,
    }
}

/// The state one published tree is built from. Only these fields can reach a
/// node, which is what keeps secrets out of the tree.
pub(crate) struct Snapshot<'a> {
    /// The composer draft, or `None` while the composer is not on screen.
    pub composer: Option<String>,
    /// Whether Send is admitted now.
    pub send: bool,
    /// Whether Stop is admitted now.
    pub stop: bool,
    /// Whether the attach action is admitted now.
    pub attach: bool,
    /// The model selector's caption, when the selector is on screen.
    pub model: Option<String>,
    /// The status sentence, when the app is showing one.
    pub status: Option<String>,
    /// Bounds measured from the widget tree in the last pass.
    pub bounds: &'a Bounds,
}

/// Build the full tree. A control with no measured bounds is left out, so the
/// tree only ever claims what the frame really contains.
pub(crate) fn tree(snapshot: &Snapshot<'_>) -> TreeUpdate {
    let mut nodes = Vec::with_capacity(EXPOSED.len() + 2);
    let mut children = Vec::new();
    // The composer comes first because its text run is built by hand: a client
    // reads the draft through that child, not through the field's own value.
    if let (Some(draft), Some(rect)) = (snapshot.composer.as_deref(), bounds(snapshot, COMPOSER)) {
        let mut node = Node::new(Role::MultilineTextInput);
        node.set_label("Message DeepSeek Harness");
        node.set_value(draft);
        node.set_bounds(rect);
        node.add_action(Action::Click);
        node.add_action(Action::Focus);
        node.add_action(Action::SetValue);
        if let Some(run) = text_run(draft) {
            node.set_children([COMPOSER_TEXT_RUN]);
            nodes.push((COMPOSER_TEXT_RUN, run));
        }
        children.push(COMPOSER);
        nodes.push((COMPOSER, node));
    }
    let mut push = |id: NodeId, node: Node| {
        children.push(id);
        nodes.push((id, node));
    };
    for (id, label, enabled) in [
        (SEND, "Send", snapshot.send),
        (STOP, "Stop", snapshot.stop),
        (ATTACH, "Attach files", snapshot.attach),
    ] {
        let Some(rect) = bounds(snapshot, id) else {
            continue;
        };
        let mut node = Node::new(Role::Button);
        node.set_label(label);
        node.set_bounds(rect);
        // A refused control is announced as disabled and offers no click, so a
        // client cannot ask for an action this frame would refuse anyway.
        if enabled {
            node.add_action(Action::Click);
        } else {
            node.set_disabled();
        }
        push(id, node);
    }
    if let Some(rect) = bounds(snapshot, SETTINGS) {
        let mut node = Node::new(Role::Button);
        node.set_label("Settings");
        node.set_bounds(rect);
        node.add_action(Action::Click);
        push(SETTINGS, node);
    }
    if let (Some(caption), Some(rect)) = (snapshot.model.as_deref(), bounds(snapshot, MODEL)) {
        let mut node = Node::new(Role::Button);
        node.set_label("Model");
        node.set_value(caption);
        node.set_bounds(rect);
        node.add_action(Action::Click);
        push(MODEL, node);
    }
    if let (Some(status), Some(rect)) = (snapshot.status.as_deref(), bounds(snapshot, STATUS)) {
        let mut node = Node::new(Role::Status);
        node.set_label(status);
        node.set_bounds(rect);
        push(STATUS, node);
    }
    let mut root = Node::new(Role::Window);
    root.set_label(WINDOW_NAME);
    root.set_children(children);
    let mut nodes_with_root = vec![(ROOT, root)];
    nodes_with_root.extend(nodes);
    TreeUpdate {
        nodes: nodes_with_root,
        tree: Some(TreeInfo {
            root: ROOT,
            toolkit_name: Some("Iced".to_owned()),
            toolkit_version: None,
        }),
        tree_id: TreeId::ROOT,
        // No node claims keyboard focus yet: this build does not track which
        // widget holds it, and the root is the honest answer for "no specific
        // node is focused".
        focus: ROOT,
    }
}

/// The composer's text as the run node a client needs.
///
/// AccessKit serves `Text` and `EditableText` only for a text node with a
/// `Role::TextRun` child, and each entry of `character_lengths` is one
/// character's length in UTF-8 bytes, summing to the value's byte length. A
/// draft past the cap publishes no run rather than one entry per character of
/// an essay: the field keeps its name, its value and its actions, and only the
/// character-level reads are absent.
fn text_run(draft: &str) -> Option<Node> {
    const MAX_CHARACTERS: usize = 4096;
    if draft.is_empty() || draft.chars().take(MAX_CHARACTERS + 1).count() > MAX_CHARACTERS {
        return None;
    }
    let mut run = Node::new(Role::TextRun);
    run.set_value(draft);
    run.set_character_lengths(
        draft
            .chars()
            .map(|c| c.len_utf8() as u8)
            .collect::<Vec<u8>>(),
    );
    Some(run)
}

fn bounds(snapshot: &Snapshot<'_>, id: NodeId) -> Option<Rect> {
    let rectangle = *snapshot.bounds.get(&id)?;
    if !rectangle.x.is_finite()
        || !rectangle.y.is_finite()
        || !rectangle.width.is_finite()
        || !rectangle.height.is_finite()
        || rectangle.width <= 0.0
        || rectangle.height <= 0.0
    {
        return None;
    }
    Some(Rect {
        x0: f64::from(rectangle.x),
        y0: f64::from(rectangle.y),
        x1: f64::from(rectangle.x + rectangle.width),
        y1: f64::from(rectangle.y + rectangle.height),
    })
}

/// Measure the exposed widgets in the live layout. The operation visits every
/// descendant that reports an id; `finish` hands the collected bounds back once.
pub(crate) fn measure<M: Send + 'static>(
    on_measured: impl FnOnce(Bounds) -> M + Send + 'static,
) -> Task<M> {
    iced::advanced::widget::operate(Collect {
        found: BTreeMap::new(),
        on_measured: Mutex::new(Some(on_measured)),
    })
}

struct Collect<F> {
    found: Bounds,
    // `finish` takes `&self`; consume this one-use callback without cloning it.
    on_measured: Mutex<Option<F>>,
}
impl<F> Collect<F> {
    fn record(&mut self, id: Option<&Id>, bounds: Rectangle) {
        let Some(node) = id.and_then(|id| {
            EXPOSED
                .iter()
                .find(|(_, widget)| id == &Id::new(*widget))
                .map(|(node, _)| *node)
        }) else {
            return;
        };
        // A later duplicate of the same id never replaces the first match, the
        // same rule the scroll restore uses.
        self.found.entry(node).or_insert(bounds);
    }
}
impl<M, F: FnOnce(Bounds) -> M + Send> Operation<M> for Collect<F> {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<M>)) {
        operate(self);
    }
    fn container(&mut self, id: Option<&Id>, bounds: Rectangle) {
        self.record(id, bounds);
    }
    fn focusable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        _state: &mut dyn operation::Focusable,
    ) {
        self.record(id, bounds);
    }
    fn text_input(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        _state: &mut dyn operation::TextInput,
    ) {
        self.record(id, bounds);
    }
    fn finish(&self) -> operation::Outcome<M> {
        let callback = self
            .on_measured
            .lock()
            .ok()
            .and_then(|mut callback| callback.take());
        match callback {
            Some(callback) => operation::Outcome::Some(callback(self.found.clone())),
            None => operation::Outcome::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use accesskit::NodeIdContent;
    use tokio::sync::mpsc::unbounded_channel;

    fn request(node: NodeId, action: Action) -> ActionRequest {
        ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: node,
            data: None,
        }
    }
    fn bounds_map(entries: &[(NodeId, f32)]) -> Bounds {
        entries
            .iter()
            .map(|(id, y)| {
                (
                    *id,
                    Rectangle {
                        x: 10.0,
                        y: *y,
                        width: 40.0,
                        height: 20.0,
                    },
                )
            })
            .collect()
    }
    fn roles(tree: &TreeUpdate) -> Vec<(NodeIdContent, Role)> {
        tree.nodes
            .iter()
            .map(|(id, node)| (id.0, node.role()))
            .collect()
    }

    #[test]
    fn clicks_map_to_the_messages_the_same_controls_already_send() {
        assert_eq!(intent(&request(SEND, Action::Click)), Some(Intent::Send));
        assert_eq!(intent(&request(STOP, Action::Click)), Some(Intent::Stop));
        assert_eq!(
            intent(&request(ATTACH, Action::Click)),
            Some(Intent::Attach)
        );
        assert_eq!(
            intent(&request(SETTINGS, Action::Click)),
            Some(Intent::Settings)
        );
        assert_eq!(intent(&request(MODEL, Action::Click)), Some(Intent::Model));
        assert_eq!(
            intent(&request(COMPOSER, Action::Focus)),
            Some(Intent::FocusComposer)
        );
        assert_eq!(
            intent(&request(COMPOSER, Action::Click)),
            Some(Intent::FocusComposer),
            "clicking a text field focuses it"
        );
        let mut set = request(COMPOSER, Action::SetValue);
        set.data = Some(ActionData::Value("PUBLIC draft".into()));
        assert_eq!(
            intent(&set),
            Some(Intent::SetComposer("PUBLIC draft".to_owned()))
        );
    }

    #[test]
    fn unsupported_actions_nodes_and_other_trees_do_nothing() {
        // SetValue on a button, a value-less SetValue, an unexposed node and an
        // action this build never published all no-op. A request for another tree
        // is refused too, though this app only ever publishes the root tree.
        let mut value = request(SEND, Action::SetValue);
        value.data = Some(ActionData::Value("PUBLIC".into()));
        assert_eq!(intent(&value), None);
        assert_eq!(intent(&request(COMPOSER, Action::SetValue)), None);
        assert_eq!(intent(&request(ROOT, Action::Click)), None);
        assert_eq!(intent(&request(SEND, Action::Blur)), None);
    }

    #[test]
    fn tree_claims_only_what_the_frame_contains() {
        let bounds = bounds_map(&[(COMPOSER, 10.0), (SEND, 40.0), (SETTINGS, 400.0)]);
        let snapshot = Snapshot {
            composer: Some("PUBLIC draft".to_owned()),
            send: true,
            stop: false,
            attach: false,
            model: Some("PUBLIC model".to_owned()),
            status: Some("PUBLIC status".to_owned()),
            bounds: &bounds,
        };
        let tree = tree(&snapshot);
        // Composer, Send and Settings were measured; Stop, Attach, model and
        // status were not, so no node claims them.
        assert_eq!(
            roles(&tree),
            vec![
                (ROOT.0, Role::Window),
                // The draft's text run is built before the field that owns it.
                (COMPOSER_TEXT_RUN.0, Role::TextRun),
                (COMPOSER.0, Role::MultilineTextInput),
                (SEND.0, Role::Button),
                (SETTINGS.0, Role::Button),
            ]
        );
        let (_, root) = &tree.nodes[0];
        assert_eq!(root.label(), Some(WINDOW_NAME));
        assert_eq!(
            root.children(),
            vec![COMPOSER, SEND, SETTINGS],
            "the root lists exactly the published children"
        );
        assert_eq!(tree.focus, ROOT);
        assert_eq!(tree.tree_id, TreeId::ROOT);
        let (_, composer) = tree
            .nodes
            .iter()
            .find(|(id, _)| *id == COMPOSER)
            .expect("the composer is published");
        assert_eq!(composer.value(), Some("PUBLIC draft"));
        assert_eq!(
            composer.children(),
            vec![COMPOSER_TEXT_RUN],
            "the field points at its text run"
        );
    }

    #[test]
    fn a_hidden_or_disabled_control_is_announced_truthfully() {
        let bounds = bounds_map(&[(COMPOSER, 10.0), (SEND, 40.0), (STOP, 70.0), (MODEL, 90.0)]);
        let snapshot = Snapshot {
            composer: Some(String::new()),
            send: false,
            stop: true,
            attach: false,
            model: Some("Choose model".to_owned()),
            status: None,
            bounds: &bounds,
        };
        let tree = tree(&snapshot);
        let send = &tree.nodes.iter().find(|(id, _)| *id == SEND).unwrap().1;
        assert!(
            send.is_disabled(),
            "a refused Send is disabled, not clickable"
        );
        let stop = &tree.nodes.iter().find(|(id, _)| *id == STOP).unwrap().1;
        assert!(!stop.is_disabled());
        // The composer keeps the empty draft it really has.
        let composer = &tree.nodes.iter().find(|(id, _)| *id == COMPOSER).unwrap().1;
        assert_eq!(composer.value(), Some(""));
    }

    #[test]
    fn the_draft_is_readable_through_its_text_run() {
        let bounds = bounds_map(&[(COMPOSER, 10.0)]);
        let draft = "Hi é😀"; // five characters, nine bytes
        let snapshot = Snapshot {
            composer: Some(draft.to_owned()),
            send: false,
            stop: false,
            attach: false,
            model: None,
            status: None,
            bounds: &bounds,
        };
        let tree = tree(&snapshot);
        let (_, run) = tree
            .nodes
            .iter()
            .find(|(id, _)| *id == COMPOSER_TEXT_RUN)
            .expect("a non-empty draft publishes a text run");
        assert_eq!(run.role(), Role::TextRun);
        assert_eq!(run.value(), Some(draft));
        assert_eq!(
            run.character_lengths(),
            vec![1, 1, 1, 2, 4],
            "one UTF-8 byte length per character"
        );
        assert_eq!(
            run.character_lengths()
                .iter()
                .map(|len| *len as usize)
                .sum::<usize>(),
            draft.len(),
            "the lengths must sum to the value's byte length"
        );
    }

    #[test]
    fn an_empty_or_oversized_draft_publishes_no_text_run() {
        let bounds = bounds_map(&[(COMPOSER, 10.0)]);
        for draft in [String::new(), "x".repeat(4097)] {
            let snapshot = Snapshot {
                composer: Some(draft.clone()),
                send: true,
                stop: false,
                attach: false,
                model: None,
                status: None,
                bounds: &bounds,
            };
            let tree = tree(&snapshot);
            assert!(
                !tree.nodes.iter().any(|(id, _)| *id == COMPOSER_TEXT_RUN),
                "no run for a draft of {} characters",
                draft.chars().count()
            );
            // The field itself is still there, with the value it really has.
            let (_, composer) = tree
                .nodes
                .iter()
                .find(|(id, _)| *id == COMPOSER)
                .expect("the composer stays published");
            assert!(composer.children().is_empty());
            assert_eq!(composer.value().map(str::len), Some(draft.len()));
        }
    }

    #[test]
    fn degenerate_measurements_are_never_published() {
        // A widget that reported a zero, negative or non-finite box is treated as
        // not measured, so a client never gets an unusable node.
        for rectangle in [
            Rectangle {
                width: 0.0,
                ..Rectangle::default()
            },
            Rectangle {
                height: -1.0,
                ..Rectangle::default()
            },
            Rectangle {
                x: f32::NAN,
                width: 10.0,
                height: 10.0,
                y: 0.0,
            },
        ] {
            let mut bounds = BTreeMap::new();
            bounds.insert(SEND, rectangle);
            let snapshot = Snapshot {
                composer: None,
                send: true,
                stop: false,
                attach: false,
                model: None,
                status: None,
                bounds: &bounds,
            };
            let tree = tree(&snapshot);
            assert_eq!(tree.nodes.len(), 1, "only the root remains");
        }
    }

    #[test]
    fn the_source_hands_its_receiver_out_once_and_then_ends() {
        let (_sender, receiver) = unbounded_channel::<Request>();
        let source = Source::new(receiver);
        assert!(source.take().is_some());
        assert!(source.take().is_none(), "a second subscriber gets nothing");

        fn hash_of(source: &Source) -> u64 {
            use std::hash::Hasher;
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            source.hash(&mut hasher);
            hasher.finish()
        }
        let (_sender, receiver) = unbounded_channel::<Request>();
        // The identity is the type, so re-hashing a rebuilt source keeps the
        // subscription alive across updates.
        assert_eq!(hash_of(&Source::new(receiver)), hash_of(&source));
    }
}
