use crate::{
    Inner, PublicEvent, StopResult,
    protocol::{Event, MAX_LINE},
};
use std::{
    collections::{HashMap, HashSet},
    fs,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    process::Child,
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

#[cfg(test)]
#[path = "process_tests.rs"]
mod traversal_tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Identity {
    pub pid: i32,
    pub group: i32,
    pub start: u64,
}

pub(crate) fn identity(pid: i32) -> Option<Identity> {
    identity_and_parent(pid).map(|(identity, _)| identity)
}
fn identity_and_parent(pid: i32) -> Option<(Identity, i32)> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // comm can include whitespace and parentheses; fields after its final ')' start at field 3.
    let fields: Vec<_> = stat.rsplit_once(')')?.1.split_whitespace().collect();
    Some((
        Identity {
            pid,
            group: fields.get(2)?.parse().ok()?,
            start: fields.get(19)?.parse().ok()?,
        },
        fields.get(1)?.parse().ok()?,
    ))
}

struct OwnedProcess {
    identity: Identity,
    fd: OwnedFd,
}
impl OwnedProcess {
    fn capture(expected: Identity) -> Option<Self> {
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, expected.pid, 0) };
        if fd < 0 {
            return None;
        }
        let fd = unsafe { OwnedFd::from_raw_fd(fd as i32) };
        if identity(expected.pid) != Some(expected) {
            return None;
        }
        Some(Self {
            identity: expected,
            fd,
        })
    }
    fn alive(&self) -> bool {
        let mut poll = libc::pollfd {
            fd: self.fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        unsafe { libc::poll(&mut poll, 1, 0) == 0 }
    }
    fn signal(&self, signal: i32) {
        // The fd, not a reusable numeric PID, authorizes post-root cleanup.
        unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                self.fd.as_raw_fd(),
                signal,
                0usize,
                0,
            );
        }
    }
}

#[cfg(test)]
pub(crate) fn can_capture_identity(expected: Identity) -> bool {
    OwnedProcess::capture(expected).is_some()
}

#[cfg(test)]
pub(crate) fn pin_fixture_cleanup(expected: Identity) -> impl FnOnce() {
    let process = OwnedProcess::capture(expected);
    move || {
        if let Some(process) = process {
            process.signal(libc::SIGKILL);
        }
    }
}

fn snapshot(root: Identity, owned: &mut HashMap<i32, OwnedProcess>) -> bool {
    // Do not enumerate unrelated desktop processes every 25ms. Traverse the kernel's
    // direct-child lists for every thread of the owned root and pinned descendants.
    // PPID + start-time proof prevents a reused numeric child PID becoming authority.
    let mut pending = vec![root];
    for process in owned.values_mut() {
        if let Some(current) = identity(process.identity.pid).filter(|current| {
            current.pid == process.identity.pid && current.start == process.identity.start
        }) {
            if process.alive() {
                // Group movement does not revoke an already proven pidfd owner. This
                // also keeps orphaned owned descendants traversable after reparenting.
                process.identity = current;
                pending.push(current);
            }
        }
    }
    let mut visited = HashSet::new();
    let mut known = true;
    while let Some(parent) = pending.pop() {
        if identity(parent.pid) != Some(parent) || !visited.insert((parent.pid, parent.start)) {
            continue;
        }
        let Ok(tasks) = fs::read_dir(format!("/proc/{}/task", parent.pid)) else {
            known &= identity(parent.pid) != Some(parent);
            continue;
        };
        for task in tasks {
            let Ok(task) = task else {
                known = false;
                continue;
            };
            let children = match fs::read_to_string(task.path().join("children")) {
                Ok(children) => children,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(_) => {
                    known = false;
                    continue;
                }
            };
            for value in children.split_whitespace() {
                let Some(pid) = value.parse::<i32>().ok().filter(|pid| *pid > 0) else {
                    known = false;
                    continue;
                };
                let Some((child, ppid)) = identity_and_parent(pid) else {
                    continue;
                };
                if ppid != parent.pid || identity(parent.pid) != Some(parent) {
                    continue;
                }
                if !owned.get(&pid).is_some_and(|p| p.identity == child) {
                    if let Some(process) = OwnedProcess::capture(child) {
                        owned.insert(pid, process);
                    } else if identity(pid) == Some(child) {
                        known = false;
                        continue;
                    } else {
                        continue;
                    }
                }
                pending.push(child);
            }
        }
    }
    owned.retain(|_, p| p.alive());
    known
}

pub(crate) struct StopRequest {
    pub reply: Option<mpsc::Sender<StopResult>>,
}

pub(crate) fn monitor(
    mut child: Child,
    directory: tempfile::TempDir,
    inner: Arc<Inner>,
    requests: mpsc::Receiver<StopRequest>,
) {
    let root = identity(child.id() as i32);
    let root_fd = root.and_then(OwnedProcess::capture);
    let mut owned = HashMap::new();
    let mut unknown = root.is_none() || root_fd.is_none();
    let mut waiters = Vec::new();
    let mut shutdown = None;
    let mut phase = 0u8;
    let mut forced = false;
    let mut root_exit = None;
    let mut scanned = Instant::now() - Duration::from_secs(1);
    let mut exited_at = None;
    loop {
        loop {
            match requests.try_recv() {
                Ok(request) => {
                    if let Some(reply) = request.reply {
                        waiters.push(reply);
                    }
                    if shutdown.is_none() {
                        inner.begin_stop();
                        let _ = inner.send_shutdown();
                        shutdown = Some(Instant::now() + inner.stop.graceful);
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    if shutdown.is_none() {
                        inner.begin_stop();
                        let _ = inner.send_shutdown();
                        shutdown = Some(Instant::now() + inner.stop.graceful);
                    }
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
            }
        }
        if inner.fault.load(std::sync::atomic::Ordering::Acquire) && shutdown.is_none() {
            inner.begin_stop();
            shutdown = Some(Instant::now());
        }
        if root_exit.is_none() {
            if scanned.elapsed() >= Duration::from_millis(25) {
                if let Some(root) = root {
                    unknown |= !snapshot(root, &mut owned);
                }
                scanned = Instant::now();
            }
            match child.try_wait() {
                Ok(Some(status)) => {
                    root_exit = Some(status);
                    exited_at = Some(Instant::now());
                    let mut state = inner.state.0.lock().unwrap();
                    state.root_exited = true;
                    state.clear_private();
                    inner.pending.lock().unwrap().clear();
                    inner.state.1.notify_all();
                    // Snapshot races and descendants changing groups cannot be certified after root exit.
                    unknown |= shutdown.is_none();
                    if shutdown.is_none() {
                        shutdown = Some(Instant::now());
                    }
                }
                Ok(None) => {}
                Err(_) => {
                    unknown = true;
                    if let Some(root_fd) = &root_fd {
                        root_fd.signal(libc::SIGKILL);
                    }
                }
            }
        }
        if root_exit.is_none()
            && inner.reader_eof.load(std::sync::atomic::Ordering::Acquire)
            && !inner.stopping.load(std::sync::atomic::Ordering::Acquire)
        {
            inner.protocol_fault();
        }
        owned.retain(|_, p| p.alive());
        if root_exit.is_some() && !owned.is_empty() && phase == 0 {
            forced = true;
            for process in owned.values() {
                process.signal(libc::SIGTERM);
            }
            phase = 1;
            shutdown = Some(Instant::now() + inner.stop.terminate);
        }
        if let Some(deadline) = shutdown {
            if Instant::now() >= deadline && (root_exit.is_none() || !owned.is_empty()) {
                forced = true;
                let signal = if phase == 0 {
                    libc::SIGTERM
                } else {
                    libc::SIGKILL
                };
                if root_exit.is_none() {
                    // Child has not been reaped, so its PID cannot be reused. Recheck before group use.
                    match child.try_wait() {
                        Ok(None) => {
                            if root.is_some_and(|root| {
                                identity(root.pid) == Some(root) && root.group == root.pid
                            }) {
                                unsafe {
                                    libc::kill(-(child.id() as i32), signal);
                                }
                            } else {
                                unknown = true;
                                if let Some(root_fd) = &root_fd {
                                    root_fd.signal(signal);
                                }
                            }
                        }
                        // Reap and publish on the next iteration; never signal its former group.
                        Ok(Some(status)) => {
                            root_exit = Some(status);
                            exited_at = Some(Instant::now());
                            let mut state = inner.state.0.lock().unwrap();
                            state.root_exited = true;
                            state.clear_private();
                            inner.pending.lock().unwrap().clear();
                            inner.state.1.notify_all();
                        }
                        Err(_) => {
                            unknown = true;
                        }
                    }
                }
                for process in owned.values() {
                    process.signal(signal);
                }
                phase = phase.saturating_add(1);
                shutdown = Some(
                    Instant::now()
                        + if phase == 1 {
                            inner.stop.terminate
                        } else {
                            inner.stop.kill
                        },
                );
                if phase >= 3 {
                    unknown = true;
                    break;
                }
            }
        }
        if root_exit.is_some() && owned.is_empty() {
            // Reader acknowledgement may lag wait(); bounded wait cannot depend on descendant EOF.
            if inner
                .acknowledged
                .load(std::sync::atomic::Ordering::Acquire)
                || exited_at.is_some_and(|at| at.elapsed() >= Duration::from_millis(500))
            {
                break;
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
    let result = StopResult {
        exited: root_exit.is_some(),
        graceful: !forced
            && root_exit.is_some_and(|status| status.success())
            && inner
                .acknowledged
                .load(std::sync::atomic::Ordering::Acquire),
        containment_unknown: unknown
            || forced
            || !inner
                .acknowledged
                .load(std::sync::atomic::Ordering::Acquire),
        observed_descendants_remaining: owned.len(),
        exit_code: root_exit.and_then(|status| status.code()),
    };
    // If the bounded kill wait failed, keep a dedicated reaper owning Child and staged files.
    // It cannot target a new group and never blocks the GUI thread.
    if root_exit.is_none() || !owned.is_empty() {
        thread::spawn(move || {
            if root_exit.is_none() {
                if let Some(root_fd) = &root_fd {
                    root_fd.signal(libc::SIGKILL);
                }
                let _ = child.wait();
            }
            // Retain pidfd ownership even after a bounded stop report. An uninterruptible owned
            // process is not silently abandoned or mistaken for successful containment.
            while !owned.is_empty() {
                owned.retain(|_, process| process.alive());
                for process in owned.values() {
                    process.signal(libc::SIGKILL);
                }
                if !owned.is_empty() {
                    thread::sleep(Duration::from_millis(50));
                }
            }
            drop(directory);
        });
    } else {
        drop(directory);
    }
    inner
        .reader_stop
        .store(true, std::sync::atomic::Ordering::Release);
    {
        let mut state = inner.state.0.lock().unwrap();
        state.clear_private();
        state.stop_result = Some(result);
        inner.state.1.notify_all();
    }
    inner.emit(PublicEvent::Exited(result));
    // stop_async holds the state mutex while enqueueing. Publishing the cached result before
    // this drain closes the exit/last-stop-request race without blocking the UI sender.
    while let Ok(request) = requests.try_recv() {
        if let Some(reply) = request.reply {
            waiters.push(reply);
        }
    }
    for waiter in waiters {
        let _ = waiter.send(result);
    }
}

pub(crate) fn nonblocking(fd: i32) -> Result<(), crate::Error> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(crate::Error::Spawn);
    }
    Ok(())
}

pub(crate) fn read_output(mut output: std::process::ChildStdout, inner: Arc<Inner>) {
    use std::io::Read;
    let mut pending = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        if inner.reader_stop.load(std::sync::atomic::Ordering::Acquire) {
            break;
        }
        match output.read(&mut chunk) {
            Ok(0) => {
                inner
                    .reader_eof
                    .store(true, std::sync::atomic::Ordering::Release);
                if !pending.is_empty() {
                    inner.protocol_fault();
                }
                break;
            }
            Ok(n) => {
                for byte in &chunk[..n] {
                    if pending.len() >= MAX_LINE {
                        inner.protocol_fault();
                        return;
                    }
                    pending.push(*byte);
                    if *byte == b'\n' {
                        match crate::protocol::parse(&pending[..pending.len() - 1], &inner.version)
                        {
                            Ok(event) => handle_event(&inner, event),
                            Err(_) => {
                                inner.protocol_fault();
                                return;
                            }
                        }
                        pending.clear();
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                let mut poll = libc::pollfd {
                    fd: output.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                };
                unsafe {
                    libc::poll(&mut poll, 1, 50);
                }
            }
            Err(_) => {
                inner.protocol_fault();
                break;
            }
        }
    }
}

fn handle_event(inner: &Inner, event: Event) {
    use std::sync::atomic::Ordering;
    match event {
        Event::Ready(endpoint) => {
            let mut state = inner.state.0.lock().unwrap();
            if state.root_exited || inner.stopping.load(Ordering::Acquire) {
                return;
            }
            if state.endpoint.is_some() {
                drop(state);
                inner.protocol_fault();
                return;
            }
            state.endpoint = Some(endpoint);
            inner.state.1.notify_all();
            drop(state);
            inner.emit(PublicEvent::Ready(crate::ReadyMetadata {
                version: inner.version.clone(),
            }));
        }
        Event::Auth(session) => {
            let mut state = inner.state.0.lock().unwrap();
            if state.root_exited {
                return;
            }
            state.account_session = session;
            // No raw callback/value, origin, token, or request headers leaves private state.
        }
        Event::Codex(mut metadata, browser) => {
            let mut state = inner.state.0.lock().unwrap();
            if state.root_exited
                || inner.stopping.load(Ordering::Acquire)
                || inner.fault.load(Ordering::Acquire)
            {
                return;
            }
            if matches!((state.codex_known_attempt, metadata.attempt), (Some(old), Some(new)) if new < old)
            {
                return;
            }
            if let Some(attempt) = metadata.attempt {
                state.codex_known_attempt = Some(attempt);
            }
            if state.codex_cancelled == metadata.attempt && metadata.attempt.is_some() {
                metadata.retry_blocked = true;
                metadata.browser_available = false;
                metadata.prompt = None;
                if metadata.phase == crate::CodexPhase::WaitingBrowser {
                    metadata.phase = crate::CodexPhase::Indeterminate;
                }
            }
            if state.codex_metadata.is_some_and(|previous| {
                previous.attempt == metadata.attempt
                    && previous.phase != crate::CodexPhase::WaitingBrowser
                    && previous.phase != crate::CodexPhase::Idle
                    && metadata.phase == crate::CodexPhase::WaitingBrowser
            }) {
                return;
            }
            if state.codex_metadata.is_some_and(|previous| {
                previous.attempt == metadata.attempt && previous.retry_blocked
            }) {
                metadata.retry_blocked = true;
                metadata.browser_available = false;
            }
            if state.codex_opened == metadata.attempt && metadata.attempt.is_some() {
                metadata.browser_available = false;
                state.codex_browser = None;
            } else {
                state.codex_browser = browser;
            }
            state.codex_metadata = Some(metadata);
            drop(state);
            inner.emit(PublicEvent::Codex(metadata));
        }
        Event::Account(account) => inner.emit(PublicEvent::Account(account)),
        Event::Reply { id, command, reply } => {
            // Release request-map lock before taking private state (monitor uses state→map).
            let pending = inner.pending.lock().unwrap().remove(&id);
            if let Some(pending) = pending {
                let reply = reply.and_then(|reply| {
                    if command
                        .as_deref()
                        .is_some_and(|command| command != pending.command)
                        || !crate::protocol::matches_reply(pending.command, &reply)
                    {
                        Err(crate::Error::Protocol)
                    } else {
                        if let crate::PublicReply::Codex(mut metadata) = reply {
                            let mut state = inner.state.0.lock().unwrap();
                            if let Some(attempt) = metadata.attempt {
                                state.codex_known_attempt = Some(
                                    state
                                        .codex_known_attempt
                                        .map_or(attempt, |known| known.max(attempt)),
                                );
                            }
                            if state.codex_cancelled == metadata.attempt
                                && metadata.attempt.is_some()
                            {
                                metadata.retry_blocked = true;
                                metadata.browser_available = false;
                                metadata.prompt = None;
                                if metadata.phase == crate::CodexPhase::WaitingBrowser {
                                    metadata.phase = crate::CodexPhase::Indeterminate;
                                }
                            }
                            Ok(crate::PublicReply::Codex(metadata))
                        } else {
                            Ok(reply)
                        }
                    }
                });
                let _ = pending.reply.send(reply);
            }
        }
        Event::Acknowledged => {
            if !inner.stopping.load(Ordering::Acquire) {
                inner.protocol_fault();
                return;
            }
            inner.acknowledged.store(true, Ordering::Release);
            inner.state.1.notify_all();
        }
        Event::Fatal => inner.protocol_fault(),
        Event::AccountSubscriptionError => inner.emit(PublicEvent::Warning(
            crate::Warning::AccountSubscriptionUnavailable,
        )),
        Event::Ignored => {}
    }
}
