//! One live instance per native home.
//!
//! The macOS desktop shell takes a single-instance lock and then focuses the
//! window that already owns it. Native now does the same: the owner also listens
//! on a socket beside the lock, and a second launch asks it to come forward
//! instead of standing up a second window on the same profile. The lock remains
//! the arbiter, so a launch still refuses when the owner cannot be reached.
//!
//! `flock` belongs to the open file description rather than to the descriptor,
//! so the winning descriptor is owned by [`Claim`] and must outlive the launch;
//! closing it early would release the profile to the next process.
use rustix::fs::{FileType, FlockOperation, Mode, OFlags, flock, fstat, open};
use std::{
    hash::{Hash, Hasher},
    io::Write,
    os::{
        fd::OwnedFd,
        unix::net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread::JoinHandle,
};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

/// Fixed instance-lock name, versioned like the drafts, sidebar and layout
/// documents so a later format cannot be mistaken for this one.
const LOCK: &str = "native-instance.v1.lock";
/// Fixed focus-request socket, owned by whichever process holds the lock.
const SOCKET: &str = "native-instance.v1.sock";

/// What a launch learned about the instance lock.
#[derive(Debug)]
pub enum Outcome {
    /// This process owns the profile and must hold the claim until it exits.
    Claimed(Claim),
    /// A live process already owns it.
    AlreadyRunning,
    /// The lock could not be created or taken. The caller decides what to do:
    /// unlike [`Outcome::AlreadyRunning`] this is not proof of another instance.
    Unavailable,
}

/// An owned instance lock, released when dropped or when the process exits.
#[derive(Debug)]
pub struct Claim {
    _file: OwnedFd,
    /// Removed on drop so a later launch can bind it again.
    socket: Option<PathBuf>,
    /// Detached: it ends when the process does, which is when the claim ends.
    _listener: Option<JoinHandle<()>>,
    source: Source,
}

/// A focus request delivered to the running window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusRequest;

/// The focus-request stream, shaped like `accessibility::Source` so the app has
/// one subscription pattern rather than two.
#[derive(Clone)]
pub(crate) struct Source(Arc<Mutex<Option<UnboundedReceiver<()>>>>);
impl Source {
    fn new(receiver: UnboundedReceiver<()>) -> Self {
        Self(Arc::new(Mutex::new(Some(receiver))))
    }
    /// A source that never yields, for a process that owns no socket.
    pub(crate) fn disabled() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }
    fn take(&self) -> Option<UnboundedReceiver<()>> {
        self.0.lock().ok()?.take()
    }
    /// Test-only access to the raw receiver; the app uses [`Self::stream`].
    #[cfg(test)]
    fn take_receiver(&self) -> Option<UnboundedReceiver<()>> {
        self.take()
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
        f.write_str("single_instance::Source")
    }
}

/// The receiver as a stream; a second call has nothing left to hand out and ends.
pub(crate) struct SourceStream(Option<UnboundedReceiver<()>>);
impl futures_util::Stream for SourceStream {
    type Item = FocusRequest;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match self.0.as_mut() {
            Some(receiver) => receiver
                .poll_recv(context)
                .map(|ready| ready.map(|()| FocusRequest)),
            None => std::task::Poll::Ready(None),
        }
    }
}

impl Claim {
    /// The stream the app subscribes to. One consumer, as with the accessibility
    /// source: a second call has nothing left and ends.
    pub(crate) fn focus_source(&self) -> Source {
        Source(Arc::clone(&self.source.0))
    }
}
impl Drop for Claim {
    fn drop(&mut self) {
        if let Some(path) = &self.socket {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// The fixed lock pathname inside `native_home`.
pub fn lock_path(native_home: &Path) -> PathBuf {
    native_home.join(LOCK)
}

/// The fixed focus-socket pathname inside `native_home`.
pub fn socket_path(native_home: &Path) -> PathBuf {
    native_home.join(SOCKET)
}

/// Try to claim `native_home` for this process.
///
/// A held lock and an unusable one are reported separately, because only the
/// first is evidence that another instance is running.
pub fn claim(native_home: &Path) -> Outcome {
    let Ok(file) = open(
        lock_path(native_home),
        OFlags::CREATE | OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::RUSR | Mode::WUSR,
    ) else {
        return Outcome::Unavailable;
    };
    // Refuse anything that is not a plain private file: a symlink (already
    // refused by `NOFOLLOW`), a device, or a file another account could write is
    // not a lock this process should honour. Ownership is established by the
    // 0700 home the other stores also require.
    let Ok(stamp) = fstat(&file) else {
        return Outcome::Unavailable;
    };
    if FileType::from_raw_mode(stamp.st_mode) != FileType::RegularFile
        || stamp.st_mode & 0o7777 != 0o600
        || stamp.st_nlink != 1
    {
        return Outcome::Unavailable;
    }
    match flock(&file, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => {
            // The lock is held; now offer the socket a second launch knocks on.
            // A socket that cannot be bound is not a reason to lose the profile:
            // the launch still owns it and simply cannot be asked to come forward.
            let (source, listener, socket) = match listen(native_home) {
                Some((source, listener)) => {
                    (source, Some(listener), Some(socket_path(native_home)))
                }
                None => (Source::disabled(), None, None),
            };
            Outcome::Claimed(Claim {
                _file: file,
                socket,
                _listener: listener,
                source,
            })
        }
        Err(_) => Outcome::AlreadyRunning,
    }
}

/// Bind the focus socket and hand each connection to the app as one request.
fn listen(native_home: &Path) -> Option<(Source, JoinHandle<()>)> {
    let path = socket_path(native_home);
    // A socket left by a crashed owner would refuse the bind; the lock already
    // proved no live owner holds this profile, so removing it is safe.
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).ok()?;
    let (sender, receiver) = unbounded_channel();
    let handle = std::thread::spawn(move || {
        for stream in listener.incoming() {
            if stream.is_err() {
                break;
            }
            // One request per connection; the sender is synchronous, so the app
            // needs no runtime for the listener thread to reach it.
            if sender.send(()).is_err() {
                break;
            }
        }
    });
    Some((Source::new(receiver), handle))
}

/// Ask the process that owns `native_home` to come forward.
///
/// Returns `false` when nothing is listening, which is the caller's cue to fall
/// back to refusing the launch rather than silently doing nothing.
pub fn notify_running(native_home: &Path) -> bool {
    let Ok(mut stream) = UnixStream::connect(socket_path(native_home)) else {
        return false;
    };
    stream.write_all(b"focus\n").is_ok()
}

/// How long a launch that lost the lock waits for the owner's listener to accept it.
///
/// The owner takes the lock and *then* binds its socket, so a launch that starts in that window
/// sees the lock held and no socket yet. Retrying briefly turns a false "nothing is listening"
/// into the focus request the owner can already serve; a profile whose owner really is gone still
/// fails after this bound, so the caller keeps its refusal.
pub const OWNER_STARTING_WAIT: std::time::Duration = std::time::Duration::from_millis(500);

/// Ask the owner that holds the lock to come forward, tolerating the moment before its listener
/// binds. Only a *held* lock reaches this path, so waiting cannot mask an absent owner.
pub fn notify_owner(native_home: &Path) -> bool {
    let started = std::time::Instant::now();
    loop {
        if notify_running(native_home) {
            return true;
        }
        if started.elapsed() >= OWNER_STARTING_WAIT {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::{DirBuilderExt, MetadataExt},
        sync::atomic::{AtomicU64, Ordering},
    };

    static SERIAL: AtomicU64 = AtomicU64::new(1);

    /// A fresh private home per test, never the user's real one.
    fn home(tag: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "dsh-instance-{tag}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .expect("private temp home");
        path
    }

    #[test]
    fn a_second_claim_refuses_while_the_first_is_held() {
        let home = home("second");
        let first = claim(&home);
        assert!(matches!(first, Outcome::Claimed(_)), "first claim wins");
        assert!(
            matches!(claim(&home), Outcome::AlreadyRunning),
            "a held lock refuses the next launch"
        );
        drop(first);
        assert!(
            matches!(claim(&home), Outcome::Claimed(_)),
            "releasing the claim frees the profile"
        );
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn separate_homes_do_not_contend() {
        let (a, b) = (home("a"), home("b"));
        let first = claim(&a);
        let second = claim(&b);
        assert!(matches!(first, Outcome::Claimed(_)));
        assert!(matches!(second, Outcome::Claimed(_)));
        let _ = fs::remove_dir_all(&a);
        let _ = fs::remove_dir_all(&b);
    }

    #[test]
    fn an_unusable_home_is_unavailable_rather_than_already_running() {
        let absent = home("absent");
        let _ = fs::remove_dir_all(&absent);
        assert!(matches!(claim(&absent), Outcome::Unavailable));
    }

    #[test]
    fn the_lock_is_a_private_regular_file() {
        let home = home("mode");
        let _ = claim(&home);
        let stamp = fs::metadata(lock_path(&home)).expect("lock created");
        assert!(stamp.is_file());
        assert_eq!(stamp.mode() & 0o777, 0o600, "lock is owner-only");
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn a_second_launch_reaches_the_owner_that_holds_the_lock() {
        let home = home("notify");
        let held = match claim(&home) {
            Outcome::Claimed(claim) => claim,
            other => panic!("first claim should win, got {other:?}"),
        };
        // This is the second launch's view: it cannot take the lock, so it asks.
        let mut receiver = held
            .focus_source()
            .take_receiver()
            .expect("the owner holds the receiver");
        assert!(matches!(claim(&home), Outcome::AlreadyRunning));
        assert!(notify_running(&home), "the owner answers on its socket");
        let mut received = false;
        for _ in 0..200 {
            if receiver.try_recv().is_ok() {
                received = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(received, "the owner received the focus request");
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn a_launch_waits_through_the_owners_startup_window_and_then_gives_up() {
        // The owner takes the lock and then binds, so a launching process can ask before the
        // socket exists. A listener that appears during the window must still be reached.
        let home = home("starting");
        let path = socket_path(&home);
        let binder = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(60));
            let listener = UnixListener::bind(&path).expect("late bind");
            // The launch makes exactly one request, so one accept is the whole conversation.
            let _ = listener.accept();
        });
        assert!(
            notify_owner(&home),
            "the launch waits through the owner's startup window"
        );
        let _ = binder.join();
        // With nothing listening the same call still fails, bounded, rather than hanging.
        let gone = super::tests::home("no-listener");
        let started = std::time::Instant::now();
        assert!(!notify_owner(&gone), "nothing listening means false");
        assert!(
            started.elapsed() <= OWNER_STARTING_WAIT + std::time::Duration::from_millis(400),
            "the wait is bounded"
        );
        let _ = fs::remove_dir_all(&home);
        let _ = fs::remove_dir_all(&gone);
    }

    #[test]
    fn notifying_with_no_live_owner_reports_failure() {
        let home = home("unowned");
        // No claim was ever taken here, so there is no socket to answer.
        assert!(!notify_running(&home), "nothing listening means false");
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn the_socket_is_removed_when_the_claim_ends() {
        let home = home("cleanup");
        let held = match claim(&home) {
            Outcome::Claimed(claim) => claim,
            other => panic!("first claim should win, got {other:?}"),
        };
        assert!(socket_path(&home).exists(), "the owner binds its socket");
        drop(held);
        assert!(
            !socket_path(&home).exists(),
            "a later launch can bind the same path"
        );
        let _ = fs::remove_dir_all(&home);
    }
}
