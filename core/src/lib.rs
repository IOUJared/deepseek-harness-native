//! Framework-neutral Linux Host ownership. Public channels contain typed redacted metadata only.
//! Blocking methods belong on application worker threads; Drop queues shutdown without waiting.
#![cfg(target_os = "linux")]

#[cfg(feature = "transport")]
mod agent_discovery;
#[cfg(feature = "transport")]
pub use agent_discovery::AgentDiscovery;
mod browser;
mod codex;
mod options;
mod process;
mod protocol;
mod secret;
#[cfg(test)]
mod tests;

pub use codex::{CodexMetadata, CodexPhase};
pub use options::RustBackendOptions;
pub use secret::{SecretApiKey, SecretCodexCallback};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    os::{
        fd::AsRawFd,
        unix::{fs::PermissionsExt, process::CommandExt},
    },
    path::PathBuf,
    process::{ChildStdin, Command as ProcessCommand, Stdio},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidOptions,
    InvalidCommand,
    Spawn,
    Starting,
    Stopping,
    Exited,
    Protocol,
    RequestFailed,
    WriterBusy,
    ControlClosed,
    WriteTimeout,
    CommandTooLarge,
    RequestTimeout,
    TooManyRequests,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

/// Closed nonsecret native controls. Key and Codex callback input use separate consuming
/// secret operations. DeepSeek account mutation and generic authorization are not implemented.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    CodexStatus,
    CodexStart,
    CodexCancel {
        attempt: u64,
    },
    CodexEnableModels {
        expected_revision: u64,
    },
    Inspect,
    OpenWorkspace(PathBuf),
    OnboardingRead,
    AccountState,
    AccountSubscribe,
    AccountUnsubscribe,
    /// Read a registered Agent's actual ID without creating/resuming it. Requires transport types.
    #[cfg(feature = "transport")]
    DiscoverSessionAgent(dsh_native_transport::dto::SessionId),
    CancelGeneration(String),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inspection {
    pub active_tasks: u32,
    pub scheduled_tasks: u32,
    pub unknown: bool,
}
impl Inspection {
    pub fn is_known_idle(self) -> bool {
        !self.unknown && self.active_tasks == 0 && self.scheduled_tasks == 0
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountStatus {
    SignedOut,
    CredentialStored,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttemptPhase {
    Initializing,
    WaitingBrowser,
    Exchanging,
    Committing,
    Succeeded,
    Cancelled,
    Expired,
    Failed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccountMetadata {
    pub status: AccountStatus,
    pub phase: Option<AttemptPhase>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OnboardingMetadata {
    pub logged_in: bool,
    pub has_api_key: bool,
    pub writable: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicReply {
    Codex(CodexMetadata),
    CodexCallbackAccepted(bool),
    Inspection(Inspection),
    WorkspaceAdded,
    Onboarding(OnboardingMetadata),
    Account(AccountMetadata),
    Unsubscribed,
    #[cfg(feature = "transport")]
    AgentDiscovery(AgentDiscovery),
    GenerationCancelled(bool),
    ApiKeySaved(bool),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadyMetadata {
    pub version: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Warning {
    ProtocolFailure,
    AccountSubscriptionUnavailable,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicEvent {
    Codex(CodexMetadata),
    Ready(ReadyMetadata),
    Account(AccountMetadata),
    Warning(Warning),
    Exited(StopResult),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StopPolicy {
    pub graceful: Duration,
    pub terminate: Duration,
    pub kill: Duration,
}
impl Default for StopPolicy {
    fn default() -> Self {
        Self {
            graceful: Duration::from_secs(3),
            terminate: Duration::from_secs(1),
            kill: Duration::from_secs(1),
        }
    }
}
impl StopPolicy {
    fn valid(self) -> bool {
        [self.graceful, self.terminate, self.kill]
            .iter()
            .all(|t| !t.is_zero() && *t <= Duration::from_secs(10))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StopResult {
    pub exited: bool,
    pub graceful: bool,
    pub containment_unknown: bool,
    pub observed_descendants_remaining: usize,
    pub exit_code: Option<i32>,
}

struct State {
    endpoint: Option<protocol::PrivateEndpoint>,
    account_session: Option<protocol::NativeAccountSessionTokenNativePrivateState>,
    codex_metadata: Option<CodexMetadata>,
    codex_browser: Option<browser::BrowserCapability>,
    codex_opened: Option<u64>,
    codex_known_attempt: Option<u64>,
    codex_cancelled: Option<u64>,
    #[cfg(feature = "transport")]
    transport_client: Option<dsh_native_transport::NativeClient>,
    #[cfg(feature = "transport")]
    transport_attempted: bool,
    #[cfg(feature = "transport")]
    transport_cancel: tokio_util::sync::CancellationToken,
    root_exited: bool,
    stop_result: Option<StopResult>,
}
impl State {
    fn clear_private(&mut self) {
        self.endpoint = None;
        self.account_session = None;
        self.codex_browser = None;
        self.codex_metadata = None;
        self.codex_opened = None;
        self.codex_known_attempt = None;
        self.codex_cancelled = None;
        #[cfg(feature = "transport")]
        self.transport_cancel.cancel();
        #[cfg(feature = "transport")]
        if let Some(client) = self.transport_client.take() {
            client.invalidate();
        }
    }
}

#[cfg(feature = "transport")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectError {
    Backend(Error),
    Transport(dsh_native_transport::Error),
    AlreadyAttempted,
}
#[cfg(feature = "transport")]
impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
#[cfg(feature = "transport")]
impl std::error::Error for ConnectError {}

struct Pending {
    command: &'static str,
    reply: mpsc::Sender<Result<PublicReply, Error>>,
}
struct Inner {
    input: Mutex<ChildStdin>,
    pending: Mutex<HashMap<String, Pending>>,
    next: AtomicU64,
    stopping: AtomicBool,
    poisoned: AtomicBool,
    fault: AtomicBool,
    acknowledged: AtomicBool,
    reader_stop: AtomicBool,
    reader_eof: AtomicBool,
    state: (Mutex<State>, Condvar),
    events: mpsc::SyncSender<PublicEvent>,
    dropped_events: AtomicU64,
    version: String,
    stop: StopPolicy,
}
impl Inner {
    fn begin_stop(&self) {
        // Revocation and browser spawn share this lock: no launch is admitted after stop.
        let mut state = self.state.0.lock().unwrap();
        state.codex_browser = None;
        state.codex_metadata = None;
        #[cfg(feature = "transport")]
        {
            state.transport_cancel.cancel();
            if let Some(client) = &state.transport_client {
                client.invalidate();
            }
            // Publish stopping only after all authenticated authority is revoked.
            self.stopping.store(true, Ordering::Release);
        }
        #[cfg(not(feature = "transport"))]
        self.stopping.store(true, Ordering::Release);
    }
    fn emit(&self, event: PublicEvent) {
        if self.events.try_send(event).is_err() {
            self.dropped_events.fetch_add(1, Ordering::Relaxed);
        }
    }
    fn protocol_fault(&self) {
        self.state.0.lock().unwrap().codex_browser = None;
        if !self.fault.swap(true, Ordering::AcqRel) {
            self.emit(PublicEvent::Warning(Warning::ProtocolFailure));
        }
        self.state.1.notify_all();
    }
    fn send_shutdown(&self) -> Result<(), Error> {
        self.send(&json!({"type":"shutdown"}))
    }
    fn send(&self, value: &Value) -> Result<(), Error> {
        let mut data = serde_json::to_vec(value).map_err(|_| Error::InvalidCommand)?;
        data.push(b'\n');
        self.send_frame(&data)
    }
    fn send_frame(&self, data: &[u8]) -> Result<(), Error> {
        if self.poisoned.load(Ordering::Acquire) {
            return Err(Error::ControlClosed);
        }
        if data.is_empty() || data.last() != Some(&b'\n') {
            return Err(Error::InvalidCommand);
        }
        if data.len() > protocol::MAX_LINE {
            return Err(Error::CommandTooLarge);
        }
        let input = self.input.try_lock().map_err(|_| Error::WriterBusy)?;
        let fd = input.as_raw_fd();
        let deadline = Instant::now() + Duration::from_millis(500);
        let mut offset = 0;
        while offset < data.len() {
            if Instant::now() >= deadline {
                self.poisoned.store(true, Ordering::Release);
                self.protocol_fault();
                return Err(Error::WriteTimeout);
            }
            let written =
                unsafe { libc::write(fd, data[offset..].as_ptr().cast(), data.len() - offset) };
            if written > 0 {
                offset += written as usize;
                continue;
            }
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            if error.kind() != std::io::ErrorKind::WouldBlock {
                self.poisoned.store(true, Ordering::Release);
                self.protocol_fault();
                return Err(Error::ControlClosed);
            }
            let mut poll = libc::pollfd {
                fd,
                events: libc::POLLOUT,
                revents: 0,
            };
            unsafe {
                libc::poll(
                    &mut poll,
                    1,
                    deadline
                        .saturating_duration_since(Instant::now())
                        .as_millis()
                        .clamp(1, 500) as i32,
                );
            }
        }
        Ok(())
    }
}

/// Native-only owner, with no Debug/Display/serde implementation and no raw JSON/event accessors.
/// Keep this handle out of widget properties; give UI code only PublicEvent/PublicReply projections.
///
/// ```compile_fail
/// fn debug<T: std::fmt::Debug>() {}
/// debug::<dsh_native_core::Backend>();
/// ```
///
/// ```compile_fail
/// fn renderer(backend: &dsh_native_core::Backend) {
///     backend.with_private_endpoint(|origin, url| (origin.to_owned(), url.to_owned()));
/// }
/// ```
pub struct Backend {
    inner: Arc<Inner>,
    owner: mpsc::Sender<process::StopRequest>,
    workers: Mutex<Vec<JoinHandle<()>>>,
    startup_timeout: Duration,
}
impl Backend {
    /// Spawn the explicit built fork using a private staged fork entry plus its adjacent helper.
    /// This method and wait_ready are blocking worker operations, not UI event-loop operations.
    pub fn start(
        options: RustBackendOptions,
    ) -> Result<(Self, mpsc::Receiver<PublicEvent>), Error> {
        Self::spawn(options.validate()?, None)
    }
    fn spawn(
        options: options::ValidatedOptions,
        fixture: Option<&str>,
    ) -> Result<(Self, mpsc::Receiver<PublicEvent>), Error> {
        let mut builder = tempfile::Builder::new();
        builder.prefix("dsh-native-core-");
        let directory = if fixture.is_some() {
            builder.tempdir_in(&options.cwd)
        } else {
            builder.tempdir()
        }
        .map_err(|_| Error::Spawn)?;
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
            .map_err(|_| Error::Spawn)?;
        for (name, script) in [
            (
                "fork-supervisor.mjs",
                include_str!("../runtime/fork-supervisor.mjs"),
            ),
            ("supervisor.mjs", include_str!("../runtime/supervisor.mjs")),
            ("codex.mjs", include_str!("../runtime/codex.mjs")),
        ] {
            let path = directory.path().join(name);
            std::fs::write(&path, script).map_err(|_| Error::Spawn)?;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .map_err(|_| Error::Spawn)?;
        }
        let script = if let Some(fixture) = fixture {
            let path = directory.path().join("fixture.mjs");
            std::fs::write(&path, fixture).map_err(|_| Error::Spawn)?;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .map_err(|_| Error::Spawn)?;
            path
        } else {
            directory.path().join("fork-supervisor.mjs")
        };
        let mut command = ProcessCommand::new(&options.node);
        command
            .arg(script)
            .current_dir(&options.cwd)
            .env_clear()
            .env("HOME", &options.user_home)
            .env("PATH", "/usr/bin:/bin")
            .env("DSH_TAURI_HOME", &options.home)
            .env("DSH_TAURI_RUNTIME", &options.runtime)
            .env("DSH_TAURI_EXPECTED_VERSION", &options.version)
            .env("DSH_TELEMETRY_DISABLED", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let parent = std::process::id() as libc::pid_t;
        unsafe {
            command.pre_exec(move || {
                if libc::setpgid(0, 0) != 0
                    || libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0
                {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::getppid() != parent {
                    return Err(std::io::Error::other("Native owner exited"));
                }
                Ok(())
            });
        }
        let (events, receiver) = mpsc::sync_channel(128);
        let (owner, requests) = mpsc::channel();
        let (initialized, initialization) = mpsc::channel();
        let startup_timeout = options.startup_timeout;
        // Linux PDEATHSIG follows the spawning thread. Spawn on the persistent ownership thread,
        // never a caller's temporary startup worker or the GUI thread.
        let monitor = thread::spawn(move || {
            let started = (|| {
                let mut child = command.spawn().map_err(|_| Error::Spawn)?;
                let input = child.stdin.take().expect("piped stdin");
                let output = child.stdout.take().expect("piped stdout");
                if process::nonblocking(input.as_raw_fd()).is_err()
                    || process::nonblocking(output.as_raw_fd()).is_err()
                {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(Error::Spawn);
                }
                let inner = Arc::new(Inner {
                    input: Mutex::new(input),
                    pending: Mutex::new(HashMap::new()),
                    next: AtomicU64::new(1),
                    stopping: AtomicBool::new(false),
                    poisoned: AtomicBool::new(false),
                    fault: AtomicBool::new(false),
                    acknowledged: AtomicBool::new(false),
                    reader_stop: AtomicBool::new(false),
                    reader_eof: AtomicBool::new(false),
                    state: (
                        Mutex::new(State {
                            endpoint: None,
                            account_session: None,
                            codex_metadata: None,
                            codex_browser: None,
                            codex_opened: None,
                            codex_known_attempt: None,
                            codex_cancelled: None,
                            #[cfg(feature = "transport")]
                            transport_client: None,
                            #[cfg(feature = "transport")]
                            transport_attempted: false,
                            #[cfg(feature = "transport")]
                            transport_cancel: tokio_util::sync::CancellationToken::new(),
                            root_exited: false,
                            stop_result: None,
                        }),
                        Condvar::new(),
                    ),
                    events,
                    dropped_events: AtomicU64::new(0),
                    version: options.version,
                    stop: options.stop,
                });
                let reader_inner = inner.clone();
                let reader = thread::spawn(move || process::read_output(output, reader_inner));
                Ok((child, inner, reader))
            })();
            match started {
                Ok((child, inner, reader)) => {
                    let _ = initialized.send(Ok((inner.clone(), reader)));
                    process::monitor(child, directory, inner, requests);
                }
                Err(error) => {
                    let _ = initialized.send(Err(error));
                }
            }
        });
        let (inner, reader) = match initialization.recv().map_err(|_| Error::Spawn)? {
            Ok(started) => started,
            Err(error) => {
                let _ = monitor.join();
                return Err(error);
            }
        };
        Ok((
            Self {
                inner,
                owner,
                workers: Mutex::new(vec![reader, monitor]),
                startup_timeout,
            },
            receiver,
        ))
    }
    pub fn wait_ready(&self) -> Result<ReadyMetadata, Error> {
        let deadline = Instant::now() + self.startup_timeout;
        let mut state = self.inner.state.0.lock().unwrap();
        loop {
            if self.inner.fault.load(Ordering::Acquire) {
                return Err(Error::Protocol);
            }
            if state.root_exited || state.stop_result.is_some() {
                return Err(Error::Exited);
            }
            if self.inner.stopping.load(Ordering::Acquire) {
                return Err(Error::Stopping);
            }
            if state.endpoint.is_some() {
                return Ok(ReadyMetadata {
                    version: self.inner.version.clone(),
                });
            }
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                drop(state);
                let _ = self.stop_async();
                return Err(Error::RequestTimeout);
            };
            state = self.inner.state.1.wait_timeout(state, remaining).unwrap().0;
        }
    }
    /// Initialize the private native HTTP/WS client exactly once for this owned Host.
    /// Call from the application worker, after wait_ready. No launch capability reaches GUI state.
    /// A failed exchange requires a new Host owner; competing calls never duplicate authentication.
    #[cfg(feature = "transport")]
    pub async fn connect_transport(
        &self,
        limits: dsh_native_transport::Limits,
    ) -> Result<dsh_native_transport::NativeClient, ConnectError> {
        let (launch, cancelled) = {
            let mut state = self.inner.state.0.lock().unwrap();
            if state.root_exited || self.inner.stopping.load(Ordering::Acquire) {
                return Err(ConnectError::Backend(Error::Exited));
            }
            if state.transport_attempted {
                return Err(ConnectError::AlreadyAttempted);
            }
            let url = state
                .endpoint
                .as_ref()
                .ok_or(ConnectError::Backend(Error::Starting))?;
            let launch = dsh_native_transport::SecretLaunchUrl::new(url.launch_url.to_owned())
                .map_err(ConnectError::Transport)?;
            state.transport_attempted = true;
            (launch, state.transport_cancel.clone())
        };
        // Stop drops the in-flight exchange and its private launch capability immediately,
        // including monitor-triggered faults. Never hold the state mutex across network I/O.
        let client = tokio::select! {
            biased;
            _ = cancelled.cancelled() => return Err(ConnectError::Backend(Error::Stopping)),
            result = dsh_native_transport::NativeClient::connect(launch, limits) => result.map_err(ConnectError::Transport)?,
        };
        let accepted = {
            let mut state = self.inner.state.0.lock().unwrap();
            if state.root_exited || self.inner.stopping.load(Ordering::Acquire) {
                false
            } else {
                state.transport_client = Some(client.clone());
                true
            }
        };
        if !accepted {
            client.close().await;
            return Err(ConnectError::Backend(Error::Exited));
        }
        Ok(client)
    }

    /// Bounded write (500ms), then bounded reply wait. Only allowlisted reply metadata can return.
    pub fn request(&self, command: Command, timeout: Duration) -> Result<PublicReply, Error> {
        let id = self.request_id(timeout)?;
        let value = command.encode(&id)?;
        if matches!(command, Command::CodexStart) {
            let state = self.inner.state.0.lock().unwrap();
            if state.codex_cancelled.is_some()
                || state.codex_metadata.is_some_and(|m| m.retry_blocked)
            {
                return Err(Error::RequestFailed);
            }
        }
        if let Command::CodexCancel { attempt } = &command {
            // Retire local browser authority before the cancellable Node request/ACK gap.
            let mut state = self.inner.state.0.lock().unwrap();
            // A validated metadata ACK registers its attempt before reaching its caller, even
            // if the first private state/URL packet has not arrived yet. Older cancels cannot
            // revoke a newer known attempt's authority.
            if state.codex_known_attempt == Some(*attempt) {
                state.codex_cancelled = Some(*attempt);
                state.codex_browser = None;
                state.codex_opened = Some(*attempt);
                if let Some(metadata) = state
                    .codex_metadata
                    .as_mut()
                    .filter(|m| m.attempt == Some(*attempt))
                {
                    metadata.phase = CodexPhase::Indeterminate;
                    metadata.browser_available = false;
                    metadata.retry_blocked = true;
                }
            }
        }
        let reply =
            self.control_request(command.name(), id, || self.inner.send(&value), timeout)?;
        #[cfg(feature = "transport")]
        if let (Command::DiscoverSessionAgent(requested), PublicReply::AgentDiscovery(found)) =
            (&command, &reply)
        {
            if &found.session_id != requested {
                return Err(Error::Protocol);
            }
        }
        Ok(reply)
    }

    /// Explicit worker-only persistence through the owned Host's official provider reference.
    /// Consumes/wipes the input and encoded frame; no key is returned, logged or put in an event.
    /// true acknowledges completed persistence, false is provider refusal. A timeout/error may
    /// occur after persistence: re-read onboarding metadata, never retry a write automatically.
    pub fn save_api_key(&self, secret: SecretApiKey, timeout: Duration) -> Result<bool, Error> {
        let id = self.request_id(timeout)?;
        let frame = secret.frame(&id)?;
        drop(secret);
        let reply = self.control_request(
            "save-api-key",
            id,
            move || {
                // The FnOnce drops/wipes its frame on return, before the reply wait begins.
                self.inner.send_frame(frame.as_slice())
            },
            timeout,
        )?;
        match reply {
            PublicReply::ApiKeySaved(saved) => Ok(saved),
            _ => Err(Error::Protocol),
        }
    }

    /// Consume a pasted response for exactly one active native prompt. A true ACK admits the
    /// response only; authorization/persistence success arrives separately as Codex metadata.
    pub fn submit_codex_callback(
        &self,
        attempt: u64,
        prompt: u64,
        secret: SecretCodexCallback,
        timeout: Duration,
    ) -> Result<bool, Error> {
        let id = self.request_id(timeout)?;
        let frame = secret.frame(&id, attempt, prompt)?;
        drop(secret);
        let reply = self.control_request(
            "codex-callback",
            id,
            move || self.inner.send_frame(frame.as_slice()),
            timeout,
        )?;
        match reply {
            PublicReply::CodexCallbackAccepted(accepted) => Ok(accepted),
            _ => Err(Error::Protocol),
        }
    }

    /// Explicit worker-only browser launch. URL authority is private and one-use. Success means
    /// the launcher exited successfully, never that login succeeded. No retry is performed.
    pub fn open_codex_browser(&self, attempt: u64) -> Result<(), Error> {
        if !codex::valid_id(attempt) {
            return Err(Error::InvalidCommand);
        }
        let child = {
            let mut state = self.inner.state.0.lock().unwrap();
            if self.inner.stopping.load(Ordering::Acquire) || state.root_exited {
                return Err(Error::Stopping);
            }
            if self.inner.fault.load(Ordering::Acquire) {
                return Err(Error::Protocol);
            }
            if !state.codex_metadata.is_some_and(|m| {
                m.attempt == Some(attempt)
                    && m.phase == CodexPhase::WaitingBrowser
                    && !m.retry_blocked
            }) {
                return Err(Error::InvalidCommand);
            }
            if state.codex_cancelled == Some(attempt) {
                return Err(Error::InvalidCommand);
            }
            let capability = state.codex_browser.take().ok_or(Error::InvalidCommand)?;
            state.codex_opened = Some(attempt);
            capability.spawn()?
        };
        browser::wait(child)
    }

    fn request_id(&self, timeout: Duration) -> Result<String, Error> {
        if timeout.is_zero() || timeout > Duration::from_secs(30) {
            return Err(Error::InvalidCommand);
        }
        if self.inner.stopping.load(Ordering::Acquire) {
            return Err(Error::Stopping);
        }
        let state = self.inner.state.0.lock().unwrap();
        if state.root_exited {
            return Err(Error::Exited);
        }
        if self.inner.stopping.load(Ordering::Acquire) {
            return Err(Error::Stopping);
        }
        if state.endpoint.is_none() {
            return Err(Error::Starting);
        }
        Ok(self.inner.next.fetch_add(1, Ordering::Relaxed).to_string())
    }

    fn control_request(
        &self,
        command: &'static str,
        id: String,
        send: impl FnOnce() -> Result<(), Error>,
        timeout: Duration,
    ) -> Result<PublicReply, Error> {
        let (reply, receiver) = mpsc::channel();
        {
            let mut pending = self.inner.pending.lock().unwrap();
            if pending.len() >= 128 {
                return Err(Error::TooManyRequests);
            }
            pending.insert(id.clone(), Pending { command, reply });
        }
        if let Err(error) = send() {
            self.inner.pending.lock().unwrap().remove(&id);
            return Err(error);
        }
        let result = receiver.recv_timeout(timeout).map_err(|error| match error {
            mpsc::RecvTimeoutError::Timeout => Error::RequestTimeout,
            mpsc::RecvTimeoutError::Disconnected => Error::Exited,
        });
        self.inner.pending.lock().unwrap().remove(&id);
        result?
    }
    /// Nonblocking app-close entry: worker owns shutdown/ack/exit/escalation. UI may poll this receiver.
    pub fn stop_async(&self) -> mpsc::Receiver<StopResult> {
        let (reply, receiver) = mpsc::channel();
        self.inner.begin_stop();
        let state = self.inner.state.0.lock().unwrap();
        if let Some(result) = state.stop_result {
            let _ = reply.send(result);
        } else {
            let _ = self.owner.send(process::StopRequest { reply: Some(reply) });
        }
        receiver
    }
    /// Blocking worker stop; successful return also joins the output reader and ownership monitor.
    pub fn stop(&self) -> Result<StopResult, Error> {
        let timeout = self.inner.stop.graceful
            + self.inner.stop.terminate
            + self.inner.stop.kill * 2
            + Duration::from_secs(2);
        let result = self
            .stop_async()
            .recv_timeout(timeout)
            .map_err(|_| Error::RequestTimeout)?;
        for worker in self.workers.lock().unwrap().drain(..) {
            let _ = worker.join();
        }
        Ok(result)
    }
    pub fn dropped_event_count(&self) -> u64 {
        self.inner.dropped_events.load(Ordering::Relaxed)
    }

    /// Reserved for a future HTTP implementation INSIDE this crate. Never expose to GUI callbacks.
    /// A borrowed launch URL remains private, and no token-bearing object is cloneable/serializable.
    #[allow(dead_code)]
    pub(crate) fn with_private_endpoint<R>(&self, f: impl FnOnce(&str, &str) -> R) -> Option<R> {
        let state = self.inner.state.0.lock().unwrap();
        state
            .endpoint
            .as_ref()
            .map(|endpoint| f(&endpoint.origin, &endpoint.launch_url))
    }
    #[allow(dead_code)]
    pub(crate) fn with_private_account_session<R>(
        &self,
        f: impl FnOnce(&str, &str, &[(String, String)]) -> R,
    ) -> Option<R> {
        let state = self.inner.state.0.lock().unwrap();
        state.account_session.as_ref().map(|session| {
            f(
                &session.origin,
                &session.token_value,
                &session.request_headers,
            )
        })
    }
}
impl Drop for Backend {
    fn drop(&mut self) {
        self.inner.begin_stop();
        let _ = self.owner.send(process::StopRequest { reply: None });
    }
}
