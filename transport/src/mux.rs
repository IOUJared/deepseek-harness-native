use crate::{
    Error, Result,
    dto::*,
    http::{Inner, request_args, validate_history},
    validate_json,
};
use futures_util::{SinkExt, StreamExt};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    marker::PhantomData,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore, mpsc, oneshot};
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{Message, client::IntoClientRequest, protocol::WebSocketConfig},
};
use tokio_util::sync::CancellationToken;

const OUTBOUND_BYTES: usize = 256 * 1024;
#[derive(Clone, Copy)]
enum Kind {
    Workspace,
    Session,
    Events,
    Control,
    Question,
}
impl Kind {
    fn endpoint(self) -> &'static str {
        match self {
            Self::Workspace => "workspace/follow",
            Self::Session => "session/follow",
            Self::Events => "$events",
            Self::Control => "session/control",
            Self::Question => "userQuestions/attachWait",
        }
    }
}
struct Delivery {
    value: Value,
    event_delivery: Option<crate::EventDelivery>,
    _bytes: OwnedSemaphorePermit,
}
struct Registration {
    sender: mpsc::Sender<Result<Delivery>>,
    lifetime: CancellationToken,
    kind: Kind,
    event_client: Option<RemoteEventClientId>,
}
struct Command {
    text: String,
    _bytes: OwnedSemaphorePermit,
    registration: Option<(String, Registration)>,
    reply: oneshot::Sender<Result<()>>,
}
struct Handle {
    inner: Arc<Inner>,
    commands: mpsc::Sender<Command>,
    outbound: Arc<Semaphore>,
    lifetime: CancellationToken,
    changed: Arc<Notify>,
    failure: Arc<Mutex<Option<Error>>>,
}
impl Handle {
    async fn send(&self, value: Value, registration: Option<(String, Registration)>) -> Result<()> {
        if self.lifetime.is_cancelled() || self.inner.lifetime.is_cancelled() {
            return Err(Error::Closed);
        }
        let text = serde_json::to_string(&value).map_err(|_| Error::InvalidDto)?;
        if text.len() > OUTBOUND_BYTES {
            return Err(Error::Oversize);
        }
        let permit = self
            .outbound
            .clone()
            .try_acquire_many_owned(text.len() as u32)
            .map_err(|_| Error::QueueFull)?;
        let (reply, rx) = oneshot::channel();
        self.commands
            .try_send(Command {
                text,
                _bytes: permit,
                registration,
                reply,
            })
            .map_err(|_| Error::QueueFull)?;
        tokio::select! {
            biased;
            answer = rx => answer.unwrap_or(Err(Error::Closed)),
            _ = self.lifetime.cancelled() => Err(self.failure.lock().ok().and_then(|e| *e).unwrap_or(Error::Closed)),
            _ = self.inner.lifetime.cancelled() => Err(Error::Closed),
        }
    }
}

/// Owns one authenticated physical socket. Drop cancels its actor; close also awaits it.
/// No Debug/Serialize or socket/origin/cookie accessors.
pub struct Mux {
    handle: Arc<Handle>,
    next: AtomicU64,
    done: Option<oneshot::Receiver<()>>,
}
impl Mux {
    pub(crate) async fn connect(inner: Arc<Inner>) -> Result<Self> {
        // Keep close()'s quiescence barrier occupied until the handshake actor is registered.
        // Cancellation cannot race an empty TaskTracker followed by a newly spawned socket owner.
        let handshake_owner = inner.clone();
        let _handshake = tokio::select! {
            biased;
            _ = handshake_owner.lifetime.cancelled() => return Err(Error::Closed),
            permit = handshake_owner.rpc_slots.acquire() => permit.map_err(|_| Error::Closed)?,
        };
        let mux_permit = inner
            .mux_slot
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::StreamLimit)?;
        let auth = inner.authority()?;
        let mut ws_url = auth
            .origin
            .join("api/remote.mux")
            .map_err(|_| Error::InvalidLaunch)?;
        ws_url.set_scheme("ws").map_err(|_| Error::InvalidLaunch)?;
        let mut request = ws_url
            .as_str()
            .into_client_request()
            .map_err(|_| Error::InvalidLaunch)?;
        request.headers_mut().insert(
            "origin",
            auth.origin
                .origin()
                .ascii_serialization()
                .parse()
                .map_err(|_| Error::InvalidLaunch)?,
        );
        request.headers_mut().insert("cookie", auth.cookie);
        let config = WebSocketConfig::default()
            .max_message_size(Some(inner.limits.max_result_bytes))
            .max_frame_size(Some(inner.limits.max_result_bytes))
            .write_buffer_size(0)
            .max_write_buffer_size(OUTBOUND_BYTES + 1024);
        let connected = tokio::select! {
            biased;
            _ = inner.lifetime.cancelled() => return Err(Error::Closed),
            result = tokio::time::timeout(inner.limits.timeout, connect_async_with_config(request, Some(config), true)) => result,
        };
        let (mut socket, _) = connected
            .map_err(|_| Error::Timeout)?
            .map_err(|_| Error::Network)?;
        let (commands, mut incoming) = mpsc::channel::<Command>(64);
        let (finished, done) = oneshot::channel();
        let lifetime = inner.lifetime.child_token();
        let changed = Arc::new(Notify::new());
        let failure = Arc::new(Mutex::new(None));
        let handle = Arc::new(Handle {
            inner: inner.clone(),
            commands,
            outbound: Arc::new(Semaphore::new(OUTBOUND_BYTES)),
            lifetime: lifetime.clone(),
            changed: changed.clone(),
            failure: failure.clone(),
        });
        let inbound = Arc::new(Semaphore::new(inner.limits.queue_bytes));
        let tasks = inner.tasks.clone();
        tasks.spawn(async move {
            let _mux_permit = mux_permit;
            let mut streams: HashMap<String, Registration> = HashMap::new();
            let mut retired = std::collections::VecDeque::<String>::new();
            let result: Result<()> = async {
                loop {
                    tokio::select! {
                        biased;
                        _ = lifetime.cancelled() => break,
                        _ = changed.notified() => {
                            let cancelled: Vec<String> = streams.iter().filter(|(_, s)| s.lifetime.is_cancelled()).map(|(id, _)| id.clone()).collect();
                            for id in cancelled {
                                if let Some(reg) = streams.remove(&id) { clear_events(&inner, &reg); }
                                if retired.len() == 256 { retired.pop_front(); }
                                retired.push_back(id.clone());
                                send(&mut socket, Message::text(json!({"type":"cancel","streamId":id}).to_string()), &lifetime, inner.limits.timeout).await?;
                            }
                        },
                        command = incoming.recv() => {
                            let Some(command) = command else { break };
                            if let Some((id, reg)) = command.registration {
                                if command.reply.is_closed() || reg.lifetime.is_cancelled() {
                                    reg.lifetime.cancel();
                                    continue;
                                }
                                if streams.len() >= inner.limits.max_streams || matches!(reg.kind, Kind::Events) && streams.values().any(|s| matches!(s.kind, Kind::Events)) {
                                    let _ = command.reply.send(Err(Error::StreamLimit));
                                    continue;
                                }
                                streams.insert(id, reg);
                            }
                            let sent = send(&mut socket, Message::text(command.text), &lifetime, inner.limits.timeout).await;
                            let _ = command.reply.send(sent);
                            sent?;
                        },
                        next = socket.next() => {
                            let Some(message) = next else { return Err(Error::Closed); };
                            match message.map_err(|error| match error { tokio_tungstenite::tungstenite::Error::Capacity(_) => Error::Oversize, _ => Error::Network })? {
                                Message::Ping(_) => {
                                    tokio::select! {
                                        biased;
                                        _ = lifetime.cancelled() => return Err(Error::Closed),
                                        result = tokio::time::timeout(inner.limits.timeout, socket.flush()) => result.map_err(|_| Error::Timeout)?.map_err(|_| Error::Network)?,
                                    }
                                },
                                Message::Pong(_) => {},
                                Message::Close(_) => return Err(Error::Closed),
                                Message::Text(text) => {
                                    if text.len() > inner.limits.max_result_bytes { return Err(Error::Oversize); }
                                    let frame: Value = serde_json::from_str(&text).map_err(|_| Error::InvalidFrame)?;
                                    validate_json(&frame, inner.limits.max_json_items)?;
                                    let object = frame.as_object().ok_or(Error::InvalidFrame)?;
                                    let tag = object.get("type").and_then(Value::as_str).ok_or(Error::InvalidFrame)?;
                                    let id = object.get("streamId").and_then(Value::as_str).ok_or(Error::InvalidFrame)?;
                                    if streams.get(id).is_some_and(|reg| reg.lifetime.is_cancelled() || reg.sender.is_closed()) {
                                        if let Some(reg) = streams.remove(id) { clear_events(&inner, &reg); }
                                        if retired.len() == 256 { retired.pop_front(); }
                                        retired.push_back(id.to_owned());
                                        send(&mut socket, Message::text(json!({"type":"cancel","streamId":id}).to_string()), &lifetime, inner.limits.timeout).await?;
                                        continue;
                                    }
                                    if !streams.contains_key(id) {
                                        if retired.iter().any(|known| known == id) && ["item", "end", "error"].contains(&tag) { continue; }
                                        return Err(Error::Correlation);
                                    }
                                    match tag {
                                        "item" if object.keys().all(|k| ["type","streamId","value"].contains(&k.as_str())) => {
                                            let reg = streams.get_mut(id).ok_or(Error::Correlation)?;
                                            let value = object.get("value").cloned().ok_or(Error::InvalidFrame)?;
                                            let event_delivery = if matches!(reg.kind, Kind::Events) { register_event(&inner, reg, &value)? } else { None };
                                            let bytes = inbound.clone().try_acquire_many_owned(text.len() as u32).map_err(|_| Error::QueueFull)?;
                                            reg.sender.try_send(Ok(Delivery { value, event_delivery, _bytes: bytes })).map_err(|_| Error::QueueFull)?;
                                        },
                                        "end" if object.len() == 2 => {
                                            let reg = streams.remove(id).ok_or(Error::Correlation)?;
                                            clear_events(&inner, &reg);
                                        },
                                        "error" if object.len() == 3 => {
                                            let error = object.get("error").and_then(Value::as_object).ok_or(Error::InvalidFrame)?;
                                            if !error.get("code").is_some_and(Value::is_string) || !error.get("message").is_some_and(Value::is_string) || !error.get("details").is_some_and(Value::is_object) { return Err(Error::InvalidFrame); }
                                            let reg = streams.remove(id).ok_or(Error::Correlation)?;
                                            clear_events(&inner, &reg);
                                            reg.sender.try_send(Err(Error::RemoteFailure)).map_err(|_| Error::QueueFull)?;
                                        },
                                        _ => return Err(Error::InvalidFrame),
                                    }
                                },
                                _ => return Err(Error::InvalidFrame),
                            }
                        },
                    }
                }
                Ok(())
            }.await;
            if let Err(error) = result { if let Ok(mut stored) = failure.lock() { *stored = Some(error); } }
            for reg in streams.values() { clear_events(&inner, reg); }
            lifetime.cancel();
            streams.clear();
            // Bounded carrier cleanup; TCP ownership drops even if the peer never acknowledges.
            let _ = tokio::time::timeout(std::time::Duration::from_secs(1), socket.close(None)).await;
            let _ = finished.send(());
        });
        Ok(Self {
            handle,
            next: AtomicU64::new(1),
            done: Some(done),
        })
    }
    async fn open<T: DeserializeOwned>(&self, kind: Kind, args: Value) -> Result<NativeStream<T>> {
        let id = format!(
            "g{}-s{}",
            self.handle.inner.generation,
            self.next.fetch_add(1, Ordering::Relaxed)
        );
        let (sender, receiver) = mpsc::channel(64);
        let lifetime = self.handle.lifetime.child_token();
        // Construct the stream owner before the first await: dropping an in-progress open
        // drops this owner and cancels any registration already queued to the actor.
        let stream = NativeStream {
            handle: self.handle.clone(),
            lifetime: lifetime.clone(),
            receiver,
            kind,
            started: false,
            cursor: None,
            ended: false,
            _type: PhantomData,
        };
        self.handle.send(json!({"type":"open","streamId":id,"endpoint":kind.endpoint(),"payload":{"args":args}}),
            Some((id.clone(), Registration { sender, lifetime, kind, event_client: None }))).await?;
        Ok(stream)
    }
    pub async fn workspace_follow(&self) -> Result<NativeStream<WorkspaceFollowFrame>> {
        self.open(Kind::Workspace, json!({})).await
    }
    pub async fn session_follow(
        &self,
        request: SessionFollowRequest,
    ) -> Result<NativeStream<SessionFollowFrame>> {
        validate_history(request.max_messages, request.turn_window.as_ref())?;
        if request.assistant_stream == Some(false) {
            return Err(Error::InvalidDto);
        }
        self.open(Kind::Session, request_args(request)?).await
    }
    pub async fn events(&self) -> Result<NativeStream<RemoteEventFrame>> {
        self.open(Kind::Events, json!({})).await
    }
    pub async fn control(&self) -> Result<NativeStream<ControlFrame>> {
        self.open(Kind::Control, json!({})).await
    }
    /// Low-level timed-wait stream: sending open is not an acquired Host claim.
    /// Requires current explicit live Agent/call authority, never a derived Session id.
    /// Prefer `hold_question` to consume and validate the acquisition item first.
    pub async fn question_wait(
        &self,
        agent_id: &AgentId,
        call_id: &ToolCallId,
    ) -> Result<NativeStream<QuestionWait>> {
        self.open(Kind::Question, json!({"agentId":agent_id,"callId":call_id}))
            .await
    }
    /// Acquire a nonexclusive timed-question claim only after its validated first Host item.
    ///
    /// The queue-inclusive opening uses the configured transport timeout. Abandoning this
    /// future drops/cancels any registered stream; no answer, rejection or delegation occurs.
    /// Normal end before the first item is `QuestionUnavailable`, not proof of timeout.
    /// Caller must bind the explicit live Agent/call to its current timed waterfall and
    /// preserve that delivery/generation/lifetime separately; this API does not discover it.
    /// The first item records acquisition at the Host, not guaranteed continued liveness:
    /// immediately monitor `wait_closed` and fence answers against delivery/claim termination.
    pub async fn hold_question(
        &self,
        agent_id: &AgentId,
        call_id: &ToolCallId,
    ) -> Result<crate::QuestionClaim> {
        let acquire = async {
            let mut stream = self.question_wait(agent_id, call_id).await?;
            match stream.next().await {
                Some(Ok(opening)) => {
                    Ok(crate::QuestionClaim::acquired(stream, opening.remaining_ms))
                }
                Some(Err(error)) => Err(error),
                None => Err(Error::QuestionUnavailable),
            }
        };
        tokio::time::timeout(self.handle.inner.limits.timeout, acquire)
            .await
            .map_err(|_| Error::Timeout)?
    }
    pub async fn close(mut self) {
        self.handle.lifetime.cancel();
        if let Some(done) = self.done.take() {
            let _ = done.await;
        }
    }
}
impl Drop for Mux {
    fn drop(&mut self) {
        self.handle.lifetime.cancel();
    }
}

/// Typed logical stream. No cursor request/replay invention; each follow starts with a replacement baseline.
pub struct NativeStream<T> {
    handle: Arc<Handle>,
    lifetime: CancellationToken,
    receiver: mpsc::Receiver<Result<Delivery>>,
    kind: Kind,
    started: bool,
    cursor: Option<i64>,
    ended: bool,
    _type: PhantomData<T>,
}
impl<T: DeserializeOwned> NativeStream<T> {
    pub async fn next(&mut self) -> Option<Result<T>> {
        self.next_owned()
            .await
            .map(|result| result.map(|(value, _)| value))
    }
    async fn next_owned(&mut self) -> Option<Result<(T, Option<crate::EventDelivery>)>> {
        if self.ended {
            return None;
        }
        if self.lifetime.is_cancelled() || self.handle.inner.lifetime.is_cancelled() {
            self.finish();
            return Some(Err(self
                .handle
                .failure
                .lock()
                .ok()
                .and_then(|e| *e)
                .unwrap_or(Error::Closed)));
        }
        let delivery = tokio::select! {
            biased;
            _ = self.lifetime.cancelled() => {
                self.finish();
                return Some(Err(self.handle.failure.lock().ok().and_then(|e| *e).unwrap_or(Error::Closed)));
            },
            _ = self.handle.inner.lifetime.cancelled() => {
                self.finish();
                return Some(Err(Error::Closed));
            },
            delivery = self.receiver.recv() => delivery,
        };
        match delivery {
            Some(Ok(delivery)) => {
                let checked = self.check(&delivery.value).and_then(|_| {
                    serde_json::from_value(delivery.value)
                        .map(|value| (value, delivery.event_delivery))
                        .map_err(|_| Error::InvalidDto)
                });
                if checked.is_err() {
                    self.lifetime.cancel();
                    self.handle.changed.notify_one();
                    self.finish();
                }
                Some(checked)
            }
            Some(Err(error)) => {
                self.finish();
                Some(Err(error))
            }
            None => {
                self.finish();
                let error = self.handle.failure.lock().ok().and_then(|e| *e);
                if let Some(error) = error {
                    Some(Err(error))
                } else if self.handle.inner.lifetime.is_cancelled() {
                    Some(Err(Error::Closed))
                } else {
                    None
                }
            }
        }
    }
    fn check(&mut self, value: &Value) -> Result<()> {
        let tag = value.get("type").and_then(Value::as_str);
        let opening = match self.kind {
            Kind::Workspace | Kind::Control => Some("baseline"),
            Kind::Session => Some("snapshot"),
            Kind::Events => Some("ready"),
            Kind::Question => None,
        };
        if let Some(opening) = opening {
            if !self.started && tag != Some(opening) || self.started && tag == Some(opening) {
                return Err(Error::Sequence);
            }
        }
        if matches!(self.kind, Kind::Question) {
            if self.started {
                return Err(Error::Sequence);
            }
            let object = value.as_object().ok_or(Error::InvalidDto)?;
            if object.len() != 1 || object.get("remainingMs").and_then(Value::as_u64).is_none() {
                return Err(Error::InvalidDto);
            }
        }
        if matches!(self.kind, Kind::Session) {
            match tag {
                Some("snapshot") => {
                    let cursor = value
                        .get("cursor")
                        .and_then(Value::as_i64)
                        .ok_or(Error::InvalidDto)?;
                    if cursor < -1 {
                        return Err(Error::Sequence);
                    }
                    self.cursor = Some(cursor);
                }
                Some("event") => {
                    let seq = value
                        .get("event")
                        .and_then(|e| e.get("seq"))
                        .and_then(Value::as_i64)
                        .ok_or(Error::InvalidDto)?;
                    if self.cursor.and_then(|c| c.checked_add(1)) != Some(seq) {
                        return Err(Error::Sequence);
                    }
                    self.cursor = Some(seq);
                }
                Some("assistant-stream") => {}
                _ => return Err(Error::InvalidFrame),
            }
        }
        self.started = true;
        Ok(())
    }
    /// Local unsubscribe; no Host acknowledgment and never a generation-cancel command.
    pub async fn cancel(&mut self) {
        self.finish();
    }
    fn finish(&mut self) {
        self.lifetime.cancel();
        self.handle.changed.notify_one();
        self.receiver.close();
        // Unsubscribed stream objects may remain alive in GUI/reconnection state.
        // Release all retained shared byte permits immediately, not only on Drop.
        while self.receiver.try_recv().is_ok() {}
        self.ended = true;
    }
}
impl NativeStream<RemoteEventFrame> {
    /// Receive a frame with exact actor-intake delivery authority. Buffered old Waterfalls
    /// retain their revoked capability even if the same event ID was subsequently reused.
    pub async fn next_event(&mut self) -> Option<Result<crate::OwnedRemoteEvent>> {
        self.next_owned().await.map(|result| {
            result.map(|(frame, delivery)| crate::OwnedRemoteEvent { frame, delivery })
        })
    }
}
impl<T> Drop for NativeStream<T> {
    fn drop(&mut self) {
        self.lifetime.cancel();
        self.handle.changed.notify_one();
    }
}
fn register_event(
    inner: &Inner,
    reg: &mut Registration,
    value: &Value,
) -> Result<Option<crate::EventDelivery>> {
    let frame: RemoteEventFrame =
        serde_json::from_value(value.clone()).map_err(|_| Error::InvalidDto)?;
    match frame {
        RemoteEventFrame::Ready { client_id, .. } => {
            if reg.event_client.is_some() {
                return Err(Error::Sequence);
            }
            reg.event_client = Some(client_id);
        }
        RemoteEventFrame::Waterfall {
            event_id,
            event,
            request,
            ..
        } => {
            // Match alpha's forwarded-event parser, not serde's positional-struct
            // convenience. Agent/signal are out-of-process authority, never payload fields.
            let request = request.as_object().ok_or(Error::InvalidDto)?;
            if request.contains_key("agent") || request.contains_key("signal") {
                return Err(Error::InvalidDto);
            }
            let client_id = reg.event_client.clone().ok_or(Error::Sequence)?;
            let mut pending = inner.pending.lock().map_err(|_| Error::Closed)?;
            if pending.contains_key(&event_id) {
                return Err(Error::Sequence);
            }
            if pending.len() >= inner.limits.max_pending_events {
                return Err(Error::QueueFull);
            }
            let delivery = Arc::new(crate::event::PendingEvent {
                event_id: event_id.clone(),
                client_id,
                kind: event,
                lifetime: reg.lifetime.child_token(),
            });
            pending.insert(event_id, delivery.clone());
            return Ok(Some(crate::EventDelivery::new(delivery)));
        }
        RemoteEventFrame::Cancel { event_id } => {
            if let Some(delivery) = inner
                .pending
                .lock()
                .map_err(|_| Error::Closed)?
                .remove(&event_id)
            {
                delivery.lifetime.cancel();
            }
        }
        RemoteEventFrame::Emit { .. } => {
            if reg.event_client.is_none() {
                return Err(Error::Sequence);
            }
        }
    }
    Ok(None)
}
async fn send(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    message: Message,
    lifetime: &CancellationToken,
    timeout: std::time::Duration,
) -> Result<()> {
    tokio::select! {
        biased;
        _ = lifetime.cancelled() => Err(Error::Closed),
        result = tokio::time::timeout(timeout, socket.send(message)) => {
            result.map_err(|_| Error::Timeout)?.map_err(|_| Error::Network)
        },
    }
}

fn clear_events(inner: &Inner, reg: &Registration) {
    if let Some(client_id) = &reg.event_client {
        if let Ok(mut pending) = inner.pending.lock() {
            pending.retain(|_, delivery| {
                if &delivery.client_id == client_id {
                    delivery.lifetime.cancel();
                    false
                } else {
                    true
                }
            });
        }
    }
}
