//! Private Linux persistence for local, delivery-unknown composer text, not replay/admission evidence.
//! Version 1 stores a bounded session→text map as plaintext (permissions, not encryption).
//! The caller supplies Core's configured native_home, never user_home or an ambient HOME, and
//! serializes complete snapshots plus UI revision/ACK ownership. Saves are blocking and cannot
//! be cancelled after filesystem admission. Atomic rename protects the previous document before
//! commit; directory-sync failure is indeterminate. Partial private temporary files may remain
//! after an I/O failure. Cooperating writers share a nonblocking advisory lock; this is not
//! containment against an uncooperative same-UID process or root.
//! Store::new retains the native-drafts.v1 namespace and unchecked snapshot saves. Store::sidebar
//! selects fixed native-sidebar.v1.json and native-sidebar.v1.lock names with the same version-1
//! drafts wrapper and limits; its values are caller-validated Entry JSON, never transcript text.
//! save_checked compares the semantic disk map (absent means empty) under the exclusive lock
//! before creating a temporary file; stale snapshots conflict rather than overwrite.
//! The MCP composition file has its own writer in `mcp.rs` and is deliberately not routed through
//! this one: that file is a rare, reviewed, backup-taking replacement of a document the Host also
//! reads, while drafts are frequent, unbacked and CAS-checked under a cross-process lock. Sharing a
//! writer would mean one framework carrying both failure taxonomies; keep the two separate.
use dsh_native_transport::dto::SessionId;
use rustix::fs::{
    AtFlags, FileType, FlockOperation, Mode, OFlags, Stat, flock, fstat, open, openat, renameat,
    statat, unlinkat,
};
use serde::{Deserialize, Deserializer, Serialize, de};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fmt,
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub type Drafts = BTreeMap<SessionId, String>;
pub const MAX_DRAFTS: usize = 128;
pub const MAX_TEXT_BYTES: usize = 32 * 1024;
pub const MAX_TOTAL_TEXT_BYTES: usize = 256 * 1024;
pub const MAX_DOCUMENT_BYTES: usize = 2 * 1024 * 1024;
const DOCUMENT: &str = "native-drafts.v1.json";
const LOCK: &str = "native-drafts.v1.lock";

/// Closed namespaces: callers cannot choose filesystem names.
#[derive(Clone, Copy)]
enum Namespace {
    Drafts,
    Sidebar,
    Layout,
    /// The reference keeps shortcut edits in their own local preference document
    /// (`dsh.keybindings.v1`, `packages/client/shortcuts/src/client/storage.ts:9`), not in a Host
    /// namespace, so they own a document here too rather than sharing the layout map.
    Keybindings,
}
impl Namespace {
    fn document(self) -> &'static str {
        match self {
            Self::Drafts => DOCUMENT,
            Self::Sidebar => "native-sidebar.v1.json",
            Self::Layout => "native-layout.v1.json",
            Self::Keybindings => "native-keybindings.v1.json",
        }
    }
    fn lock(self) -> &'static str {
        match self {
            Self::Drafts => LOCK,
            Self::Sidebar => "native-sidebar.v1.lock",
            Self::Layout => "native-layout.v1.lock",
            Self::Keybindings => "native-keybindings.v1.lock",
        }
    }
    fn temporary_prefix(self) -> &'static str {
        match self {
            Self::Drafts => ".native-drafts",
            Self::Sidebar => ".native-sidebar",
            Self::Layout => ".native-layout",
            Self::Keybindings => ".native-keybindings",
        }
    }
}
const MAX_KEY_BYTES: usize = 128;
static TEMP_SERIAL: AtomicU64 = AtomicU64::new(1);

/// Fixed diagnostics only: never a pathname, JSON parser message, text body, or external cause.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    InvalidHome,
    Unavailable,
    InvalidData,
    Limit,
    Busy,
    Conflict,
    /// The destination was not replaced; a partial private temporary file may remain.
    NotSaved,
    /// Replacement may have succeeded; durable/path ownership ACK failed. Load to reconcile.
    Indeterminate,
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidHome => "Invalid native draft home",
            Self::Unavailable => "Native draft storage unavailable",
            Self::InvalidData => "Native draft document invalid",
            Self::Limit => "Native draft storage limit exceeded",
            Self::Busy => "Native draft storage busy",
            Self::Conflict => "Native draft storage changed",
            Self::NotSaved => "Native drafts not saved; private temporary data may remain",
            Self::Indeterminate => "Native draft save indeterminate; load to reconcile",
        })
    }
}
impl std::error::Error for Failure {}

/// Qualification boundaries in the real save, not simulated or kernel-in-progress syscalls.
/// Every callback runs after the private directory, lock and temporary file are admitted.
#[cfg(any(test, feature = "public-layout-fixture"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveCheckpoint {
    BeforeTempWrite,
    AfterTempWrite,
    BeforeTempSync,
    AfterTempSync,
    BeforeRename,
    AfterRename,
    BeforeDirectorySync,
    AfterDirectorySync,
    BeforeAcknowledgement,
}

/// Metadata-only observation; blocking it leaves the actual Store::save unfinished.
#[cfg(any(test, feature = "public-layout-fixture"))]
pub type SaveObserver = std::sync::Arc<dyn Fn(SaveCheckpoint) -> io::Result<()> + Send + Sync>;

/// Inert configuration, deliberately without Debug or serialization.
pub struct Store {
    home: PathBuf,
    namespace: Namespace,
    #[cfg(any(test, feature = "public-layout-fixture"))]
    save_observer: Option<SaveObserver>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u8,
    #[serde(deserialize_with = "unique_drafts")]
    drafts: Drafts,
}
fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= MAX_KEY_BYTES && !key.chars().any(char::is_control)
}
fn validate(drafts: &Drafts) -> Result<(), Failure> {
    if drafts.len() > MAX_DRAFTS {
        return Err(Failure::Limit);
    }
    let mut total = 0_usize;
    for (session, text) in drafts {
        if !valid_key(session.as_str()) {
            return Err(Failure::InvalidData);
        }
        if text.len() > MAX_TEXT_BYTES {
            return Err(Failure::Limit);
        }
        total = total.checked_add(text.len()).ok_or(Failure::Limit)?;
        if total > MAX_TOTAL_TEXT_BYTES {
            return Err(Failure::Limit);
        }
    }
    Ok(())
}
fn unique_drafts<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Drafts, D::Error> {
    struct Visitor;
    impl<'de> de::Visitor<'de> for Visitor {
        type Value = Drafts;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a bounded unique session text map")
        }
        fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Drafts, A::Error> {
            let mut drafts = Drafts::new();
            let mut total = 0_usize;
            while let Some((key, text)) = map.next_entry::<String, String>()? {
                if drafts.len() >= MAX_DRAFTS || !valid_key(&key) || text.len() > MAX_TEXT_BYTES {
                    return Err(de::Error::custom("invalid draft map"));
                }
                total = total
                    .checked_add(text.len())
                    .ok_or_else(|| de::Error::custom("invalid draft map"))?;
                if total > MAX_TOTAL_TEXT_BYTES {
                    return Err(de::Error::custom("invalid draft map"));
                }
                let session =
                    SessionId::new(key).map_err(|_| de::Error::custom("invalid draft map"))?;
                if drafts.insert(session, text).is_some() {
                    return Err(de::Error::custom("duplicate draft key"));
                }
            }
            Ok(drafts)
        }
    }
    deserializer.deserialize_map(Visitor)
}
fn decode(bytes: &[u8]) -> Result<Drafts, Failure> {
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(Failure::Limit);
    }
    // Serde's struct visitor also accepts positional arrays; the durable format is an object.
    if bytes
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
        != Some(b'{')
    {
        return Err(Failure::InvalidData);
    }
    let document: Document = serde_json::from_slice(bytes).map_err(|_| Failure::InvalidData)?;
    if document.version != 1 {
        return Err(Failure::InvalidData);
    }
    validate(&document.drafts)?;
    Ok(document.drafts)
}
fn encode(drafts: &Drafts) -> Result<Vec<u8>, Failure> {
    validate(drafts)?;
    #[derive(Serialize)]
    struct Borrowed<'a> {
        version: u8,
        drafts: &'a Drafts,
    }
    let bytes =
        serde_json::to_vec(&Borrowed { version: 1, drafts }).map_err(|_| Failure::InvalidData)?;
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(Failure::Limit);
    }
    Ok(bytes)
}
fn same_inode(a: &Stat, b: &Stat) -> bool {
    a.st_dev == b.st_dev
        && a.st_ino == b.st_ino
        && a.st_uid == b.st_uid
        && a.st_gid == b.st_gid
        && a.st_mode == b.st_mode
        && a.st_nlink == b.st_nlink
}
fn same_stamp(a: &Stat, b: &Stat) -> bool {
    same_inode(a, b)
        && a.st_size == b.st_size
        && a.st_mtime == b.st_mtime
        && a.st_mtime_nsec == b.st_mtime_nsec
        && a.st_ctime == b.st_ctime
        && a.st_ctime_nsec == b.st_ctime_nsec
}
fn private_regular(stat: &Stat, uid: u32) -> bool {
    FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile
        && stat.st_uid == uid
        && stat.st_mode & 0o7777 == 0o600
        && stat.st_nlink == 1
        && stat.st_size >= 0
        && stat.st_size <= MAX_DOCUMENT_BYTES as i64
}
fn private_directory(stat: &Stat, uid: u32) -> bool {
    FileType::from_raw_mode(stat.st_mode) == FileType::Directory
        && stat.st_uid == uid
        && stat.st_mode & 0o7777 == 0o700
}
fn file_flags() -> OFlags {
    OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK
}
struct Directory {
    namespace: Namespace,
    parent: File,
    name: OsString,
    file: File,
    stamp: Stat,
    uid: u32,
}
impl Directory {
    fn open(home: &Path, namespace: Namespace) -> Result<Option<Self>, Failure> {
        // Matches the native Linux configuration owner's fixed procfs UID observation.
        let uid = std::fs::metadata("/proc/self")
            .map_err(|_| Failure::Unavailable)?
            .uid();
        let flags = file_flags() | OFlags::DIRECTORY;
        let mut parent =
            File::from(open("/", flags, Mode::empty()).map_err(|_| Failure::Unavailable)?);
        let components: Vec<_> = home
            .components()
            .filter_map(|component| {
                if let Component::Normal(name) = component {
                    Some(name)
                } else {
                    None
                }
            })
            .collect();
        let (name, ancestors) = components.split_last().ok_or(Failure::InvalidHome)?;
        for ancestor in ancestors {
            match openat(&parent, Path::new(ancestor), flags, Mode::empty()) {
                Ok(fd) => parent = File::from(fd),
                Err(rustix::io::Errno::NOENT) => return Ok(None),
                Err(_) => return Err(Failure::Unavailable),
            }
        }
        let file = match openat(&parent, Path::new(name), flags, Mode::empty()) {
            Ok(fd) => File::from(fd),
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(_) => return Err(Failure::Unavailable),
        };
        let stamp = fstat(&file).map_err(|_| Failure::Unavailable)?;
        if !private_directory(&stamp, uid) {
            return Err(Failure::Unavailable);
        }
        let directory = Self {
            namespace,
            parent,
            name: name.to_os_string(),
            file,
            stamp,
            uid,
        };
        directory.guard()?;
        Ok(Some(directory))
    }
    fn guard(&self) -> Result<(), Failure> {
        let actual = fstat(&self.file).map_err(|_| Failure::Conflict)?;
        let named = statat(
            &self.parent,
            Path::new(&self.name),
            AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(|_| Failure::Conflict)?;
        if private_directory(&actual, self.uid)
            && same_inode(&self.stamp, &actual)
            && same_inode(&actual, &named)
        {
            Ok(())
        } else {
            Err(Failure::Conflict)
        }
    }
    fn lock(&self, create: bool) -> Result<Option<Lock>, Failure> {
        self.guard()?;
        let flags = OFlags::RDWR | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK;
        let file = match openat(&self.file, self.namespace.lock(), flags, Mode::empty()) {
            Ok(fd) => File::from(fd),
            Err(rustix::io::Errno::NOENT) if !create => return Ok(None),
            Err(rustix::io::Errno::NOENT) => {
                let (fd, created) = match openat(
                    &self.file,
                    self.namespace.lock(),
                    flags | OFlags::CREATE | OFlags::EXCL,
                    Mode::RUSR | Mode::WUSR,
                ) {
                    Ok(fd) => (fd, true),
                    // Another cooperating writer won creation. Never truncate or remove its lock.
                    Err(rustix::io::Errno::EXIST) => (
                        openat(&self.file, self.namespace.lock(), flags, Mode::empty())
                            .map_err(|_| Failure::Unavailable)?,
                        false,
                    ),
                    Err(_) => return Err(Failure::Unavailable),
                };
                let file = File::from(fd);
                // Only an EXCL-created lock may be chmodded after a restrictive umask.
                if created {
                    file.set_permissions(std::fs::Permissions::from_mode(0o600))
                        .map_err(|_| Failure::Unavailable)?;
                }
                file
            }
            Err(_) => return Err(Failure::Unavailable),
        };
        let stamp = fstat(&file).map_err(|_| Failure::Unavailable)?;
        if !private_regular(&stamp, self.uid) || stamp.st_size != 0 {
            return Err(Failure::Unavailable);
        }
        let mut lock = Lock {
            name: self.namespace.lock(),
            file,
            stamp,
            locked: false,
        };
        lock.guard(self)?;
        flock(
            &lock.file,
            if create {
                FlockOperation::NonBlockingLockExclusive
            } else {
                FlockOperation::NonBlockingLockShared
            },
        )
        .map_err(|_| Failure::Busy)?;
        lock.locked = true;
        lock.guard(self)?;
        Ok(Some(lock))
    }
    fn read(&self) -> Result<Option<Snapshot>, Failure> {
        self.guard()?;
        let before = match statat(
            &self.file,
            self.namespace.document(),
            AtFlags::SYMLINK_NOFOLLOW,
        ) {
            Ok(stat) => stat,
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(_) => return Err(Failure::Unavailable),
        };
        if !private_regular(&before, self.uid) {
            return Err(Failure::Unavailable);
        }
        let mut file = File::from(
            openat(
                &self.file,
                self.namespace.document(),
                file_flags(),
                Mode::empty(),
            )
            .map_err(|_| Failure::Unavailable)?,
        );
        let opened = fstat(&file).map_err(|_| Failure::Unavailable)?;
        if !same_stamp(&before, &opened) {
            return Err(Failure::Conflict);
        }
        let bytes = bounded_read(&mut file).map_err(|_| Failure::Unavailable)?;
        let after = fstat(&file).map_err(|_| Failure::Unavailable)?;
        let named = statat(
            &self.file,
            self.namespace.document(),
            AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(|_| Failure::Conflict)?;
        if !same_stamp(&opened, &after)
            || !same_stamp(&after, &named)
            || bytes.len() as i64 != after.st_size
        {
            return Err(Failure::Conflict);
        }
        let drafts = decode(&bytes)?;
        self.guard()?;
        Ok(Some(Snapshot {
            stamp: after,
            bytes,
            drafts,
        }))
    }
}
fn bounded_read(file: &mut File) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    file.take(MAX_DOCUMENT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(bytes)
}

struct Lock {
    name: &'static str,
    file: File,
    stamp: Stat,
    locked: bool,
}
impl Drop for Lock {
    fn drop(&mut self) {
        if !self.locked {
            return;
        }
        // An flock lock belongs to the open file description, so closing this descriptor
        // releases it only once every duplicate is closed too. fork(2) duplicates it into a
        // child, where O_CLOEXEC closes the copy at execve: until then the inherited
        // duplicate keeps this critical section alive after the operation already finished,
        // and a cooperating writer observes a spurious Failure::Busy. Unlock explicitly so
        // the lock ends exactly with the operation rather than with the last inherited
        // descriptor. An unlock failure is ignored: closing the descriptor still releases it.
        let _ = flock(&self.file, FlockOperation::Unlock);
    }
}
impl Lock {
    fn guard(&self, directory: &Directory) -> Result<(), Failure> {
        directory.guard()?;
        let opened = fstat(&self.file).map_err(|_| Failure::Conflict)?;
        let named = statat(&directory.file, self.name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| Failure::Conflict)?;
        if private_regular(&opened, directory.uid)
            && opened.st_size == 0
            && same_inode(&self.stamp, &opened)
            && same_inode(&opened, &named)
        {
            Ok(())
        } else {
            Err(Failure::Conflict)
        }
    }
}
struct Snapshot {
    stamp: Stat,
    bytes: Vec<u8>,
    drafts: Drafts,
}
fn same_snapshot(a: Option<&Snapshot>, b: Option<&Snapshot>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => same_stamp(&a.stamp, &b.stamp) && a.bytes == b.bytes,
        _ => false,
    }
}
struct Temporary<'a> {
    directory: &'a Directory,
    name: String,
    file: File,
    renamed: bool,
}
impl<'a> Temporary<'a> {
    fn create(directory: &'a Directory) -> Result<Self, Failure> {
        let serial = TEMP_SERIAL
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |old| {
                old.checked_add(1)
            })
            .map_err(|_| Failure::NotSaved)?;
        let mut random = [0_u8; 16];
        File::open("/dev/urandom")
            .and_then(|mut file| file.read_exact(&mut random))
            .map_err(|_| Failure::NotSaved)?;
        let random: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
        let name = format!(
            "{}-{}-{serial}-{random}.tmp",
            directory.namespace.temporary_prefix(),
            std::process::id()
        );
        let fd = openat(
            &directory.file,
            name.as_str(),
            OFlags::RDWR
                | OFlags::CREATE
                | OFlags::EXCL
                | OFlags::NOFOLLOW
                | OFlags::CLOEXEC
                | OFlags::NONBLOCK,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(|_| Failure::NotSaved)?;
        let temporary = Self {
            directory,
            name,
            file: File::from(fd),
            renamed: false,
        };
        // Only the just-created EXCL descriptor is changed; existing files are never repaired.
        temporary
            .file
            .set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|_| Failure::NotSaved)?;
        temporary.guard()?;
        Ok(temporary)
    }
    fn guard(&self) -> Result<(), Failure> {
        let opened = fstat(&self.file).map_err(|_| Failure::NotSaved)?;
        let named = statat(
            &self.directory.file,
            self.name.as_str(),
            AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(|_| Failure::Conflict)?;
        if private_regular(&opened, self.directory.uid) && same_inode(&opened, &named) {
            Ok(())
        } else {
            Err(Failure::Conflict)
        }
    }
}
impl Drop for Temporary<'_> {
    fn drop(&mut self) {
        if self.renamed {
            return;
        }
        // Resolve the generated name relative to the pinned, verified directory and match
        // its actual EXCL-created descriptor before unlink. Never delete a replacement.
        if self.directory.guard().is_ok() && self.guard().is_ok() {
            let _ = unlinkat(&self.directory.file, self.name.as_str(), AtFlags::empty());
        }
    }
}
/// Reserved key for the conversation content width inside the layout namespace.
/// The web persists the same preference under `dsh.conversation.contentWidth`.
pub const LAYOUT_CONTENT_WIDTH: &str = "layout.conversationContentWidth";
/// Widest stored preference accepted. A value outside `1..=4096` is treated as
/// absent rather than clamped, so a corrupt or hand-edited file cannot silently
/// pin the transcript to an arbitrary width.
const MAX_LAYOUT_WIDTH: f32 = 4096.0;
/// Parse a stored width. Rejects non-numeric, non-finite, non-positive and
/// absurd values; the caller clamps the accepted result against its own column.
pub fn parse_layout_width(raw: &str) -> Option<f32> {
    let value = raw.trim().parse::<f32>().ok()?;
    (value.is_finite() && value > 0.0 && value <= MAX_LAYOUT_WIDTH).then_some(value)
}
/// Format a width for storage, or `None` when it is not storable.
pub fn format_layout_width(width: f32) -> Option<String> {
    (width.is_finite() && width > 0.0 && width <= MAX_LAYOUT_WIDTH).then(|| format!("{width:.1}"))
}
/// Reserved key for the transcript content size inside the layout namespace,
/// beside the width so one file holds both layout preferences.
pub const LAYOUT_CONTENT_SIZE: &str = "layout.contentFontSize";
/// Reserved key for the busy-state Enter preference; absent means Queue.
pub const LAYOUT_BUSY_ENTER: &str = "layout.busyEnter";

/// The General page's Work details mode; absent means the reference's `Standard`.
pub const LAYOUT_WORK_DETAILS: &str = "layout.workDetails";
/// The General page's "Show coding view" preference; absent means on.
pub const LAYOUT_CODING_VIEW: &str = "layout.codingView";
/// The shell the terminal panel last created with, by path: the reference keeps the same fact in its
/// own preference key (`api/terminal-controller/src/client/shell-preference.ts:2`), so a reload does
/// not send the user back to the first discovered shell.
pub const LAYOUT_TERMINAL_SHELL: &str = "layout.terminalShell";
/// The application the workspace opener last launched, by catalog id: the reference keeps the same
/// fact in `dsh.open-in-app.choice` (`ui-open-in-app/src/client/controller.ts:30-34`).
pub const LAYOUT_OPEN_IN_APP_CHOICE: &str = "layout.openInAppChoice";
/// A stored app choice is accepted only when it is a catalog-shaped id; anything else is ignored,
/// leaving the opener on the Host's first nameable app.
pub fn parse_open_in_app_choice(raw: &str) -> Option<String> {
    let id = raw.trim();
    (!id.is_empty()
        && id.len() <= 32
        && id
            .chars()
            .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit()))
    .then(|| id.to_owned())
}
/// A stored shell path is accepted only when it looks like one: absolute, bounded, and free of
/// control characters. Anything else is ignored, leaving the panel on the Host's first shell.
pub fn parse_terminal_shell(raw: &str) -> Option<String> {
    let path = raw.trim();
    (!path.is_empty()
        && path.len() <= 4096
        && path.starts_with('/')
        && !path.chars().any(char::is_control))
    .then(|| path.to_owned())
}
/// Parse a stored content size. The offered range belongs to `design::content_size`,
/// which maps anything it does not offer back to the default, so this rejects only
/// what is not a usable number at all.
pub fn parse_layout_size(raw: &str) -> Option<f32> {
    let value = raw.trim().parse::<f32>().ok()?;
    (value.is_finite() && value > 0.0).then_some(value)
}
/// Format a content size for storage, or `None` when it is not storable.
pub fn format_layout_size(size: f32) -> Option<String> {
    (size.is_finite() && size > 0.0).then(|| format!("{size:.1}"))
}
impl Store {
    /// Validate an explicit native-home pathname without opening or creating anything.
    /// Save requires Core to have already established this current-UID, exact-mode-0700 home.
    pub fn new(native_home: PathBuf) -> Result<Self, Failure> {
        Self::with_namespace(native_home, Namespace::Drafts)
    }
    /// Separate bounded sidebar customization storage; Entry validation belongs to the caller.
    pub fn sidebar(native_home: PathBuf) -> Result<Self, Failure> {
        Self::with_namespace(native_home, Namespace::Sidebar)
    }
    /// Separate bounded app-local layout preferences; value validation belongs to the caller.
    pub fn layout(native_home: PathBuf) -> Result<Self, Failure> {
        Self::with_namespace(native_home, Namespace::Layout)
    }
    /// Separate bounded shortcut overrides; value validation belongs to the caller.
    pub fn keybindings(native_home: PathBuf) -> Result<Self, Failure> {
        Self::with_namespace(native_home, Namespace::Keybindings)
    }
    fn with_namespace(native_home: PathBuf, namespace: Namespace) -> Result<Self, Failure> {
        let text = native_home.to_str().ok_or(Failure::InvalidHome)?;
        if !native_home.is_absolute()
            || text.len() > 4096
            || text == "/"
            || text.chars().any(char::is_control)
            || text.ends_with('/')
            || text
                .split('/')
                .skip(1)
                .any(|part| part.is_empty() || part == "." || part == "..")
            || native_home
                .components()
                .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
        {
            return Err(Failure::InvalidHome);
        }
        Ok(Self {
            home: native_home,
            namespace,
            #[cfg(any(test, feature = "public-layout-fixture"))]
            save_observer: None,
        })
    }
    /// Install qualification-only instrumentation. No environment or ordinary-app activation.
    /// The caller owns bounded observation, release and join; never acquire this Store's mutex
    /// from the GUI while its save callback is blocked. Errors retain the save's commit boundary.
    #[cfg(any(test, feature = "public-layout-fixture"))]
    pub fn set_save_observer(&mut self, observer: Option<SaveObserver>) {
        self.save_observer = observer;
    }
    #[cfg(any(test, feature = "public-layout-fixture"))]
    fn checkpoint(&self, step: SaveCheckpoint) -> Result<(), Failure> {
        let Some(observer) = &self.save_observer else {
            return Ok(());
        };
        observer(step).map_err(|_| match step {
            SaveCheckpoint::BeforeTempWrite
            | SaveCheckpoint::AfterTempWrite
            | SaveCheckpoint::BeforeTempSync
            | SaveCheckpoint::AfterTempSync
            | SaveCheckpoint::BeforeRename => Failure::NotSaved,
            SaveCheckpoint::AfterRename
            | SaveCheckpoint::BeforeDirectorySync
            | SaveCheckpoint::AfterDirectorySync
            | SaveCheckpoint::BeforeAcknowledgement => Failure::Indeterminate,
        })
    }
    /// Read a validated bounded complete map. A missing home/document is empty, not an ACK.
    /// No directories or lock files are created; malformed existing files are not overwritten.
    pub fn load(&mut self) -> Result<Drafts, Failure> {
        let Some(directory) = Directory::open(&self.home, self.namespace)? else {
            return Ok(Drafts::new());
        };
        let lock = directory.lock(false)?;
        let snapshot = directory.read()?;
        if let Some(lock) = &lock {
            lock.guard(&directory)?;
        }
        directory.guard()?;
        Ok(snapshot.map_or_else(Drafts::new, |snapshot| snapshot.drafts))
    }
    /// Persist a complete local text snapshot; delivery is unknown. No request/receipt/account state.
    /// Caller-owned sequencing is mandatory; this operation does not merge or authorize Send.
    /// Never blindly retry an Indeterminate result. Blocking I/O has no cancellation guarantee.
    pub fn save(&mut self, drafts: &Drafts) -> Result<(), Failure> {
        self.save_with(drafts, |file, bytes| file.write_all(bytes))
    }
    /// Compare semantic current data under the exclusive lock before any temporary creation.
    /// A missing document matches an empty expected map. Conflict never replaces the document;
    /// intra-operation inode/byte guards and Indeterminate reconciliation still apply.
    pub fn save_checked(&mut self, data: &Drafts, expected: &Drafts) -> Result<(), Failure> {
        self.save_operation(data, Some(expected), |file, bytes| file.write_all(bytes))
    }
    fn save_with(
        &mut self,
        drafts: &Drafts,
        write: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
    ) -> Result<(), Failure> {
        self.save_operation(drafts, None, write)
    }
    fn save_operation(
        &mut self,
        drafts: &Drafts,
        expected: Option<&Drafts>,
        write: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
    ) -> Result<(), Failure> {
        let bytes = encode(drafts)?;
        let directory = Directory::open(&self.home, self.namespace)?.ok_or(Failure::Unavailable)?;
        let lock = directory.lock(true)?.ok_or(Failure::Unavailable)?;
        let original = directory.read()?;
        if let Some(expected) = expected {
            let matches = original.as_ref().map_or_else(
                || expected.is_empty(),
                |snapshot| &snapshot.drafts == expected,
            );
            if !matches {
                return Err(Failure::Conflict);
            }
        }
        let mut temporary = Temporary::create(&directory)?;
        #[cfg(any(test, feature = "public-layout-fixture"))]
        self.checkpoint(SaveCheckpoint::BeforeTempWrite)?;
        write(&mut temporary.file, &bytes).map_err(|_| Failure::NotSaved)?;
        #[cfg(any(test, feature = "public-layout-fixture"))]
        self.checkpoint(SaveCheckpoint::AfterTempWrite)?;
        temporary.guard()?;
        if fstat(&temporary.file)
            .map_err(|_| Failure::NotSaved)?
            .st_size
            != bytes.len() as i64
        {
            return Err(Failure::NotSaved);
        }
        #[cfg(any(test, feature = "public-layout-fixture"))]
        self.checkpoint(SaveCheckpoint::BeforeTempSync)?;
        temporary.file.sync_all().map_err(|_| Failure::NotSaved)?;
        #[cfg(any(test, feature = "public-layout-fixture"))]
        self.checkpoint(SaveCheckpoint::AfterTempSync)?;
        temporary
            .file
            .seek(SeekFrom::Start(0))
            .map_err(|_| Failure::NotSaved)?;
        if bounded_read(&mut temporary.file).map_err(|_| Failure::NotSaved)? != bytes {
            return Err(Failure::NotSaved);
        }
        lock.guard(&directory)?;
        let current = directory.read()?;
        if !same_snapshot(original.as_ref(), current.as_ref()) {
            return Err(Failure::Conflict);
        }
        temporary.guard()?;
        directory.guard()?;
        // Both resolved names belong to the verified pinned home: the source matches its
        // owned EXCL descriptor; the fixed destination is still the validated original.
        // Once replacement is invoked, retain uncertainty even for an I/O error. Do not
        // unlink a temporary whose rename outcome may be unknown or blindly repeat the write.
        #[cfg(any(test, feature = "public-layout-fixture"))]
        self.checkpoint(SaveCheckpoint::BeforeRename)?;
        temporary.renamed = true;
        renameat(
            &directory.file,
            temporary.name.as_str(),
            &directory.file,
            directory.namespace.document(),
        )
        .map_err(|_| Failure::Indeterminate)?;
        #[cfg(any(test, feature = "public-layout-fixture"))]
        self.checkpoint(SaveCheckpoint::AfterRename)?;
        #[cfg(any(test, feature = "public-layout-fixture"))]
        self.checkpoint(SaveCheckpoint::BeforeDirectorySync)?;
        directory
            .file
            .sync_all()
            .map_err(|_| Failure::Indeterminate)?;
        #[cfg(any(test, feature = "public-layout-fixture"))]
        self.checkpoint(SaveCheckpoint::AfterDirectorySync)?;
        lock.guard(&directory).map_err(|_| Failure::Indeterminate)?;
        let committed = directory
            .read()
            .map_err(|_| Failure::Indeterminate)?
            .ok_or(Failure::Indeterminate)?;
        if committed.bytes != bytes {
            return Err(Failure::Indeterminate);
        }
        #[cfg(any(test, feature = "public-layout-fixture"))]
        self.checkpoint(SaveCheckpoint::BeforeAcknowledgement)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "sidebar_storage_tests.rs"]
mod sidebar_storage_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{DirBuilderExt, symlink};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct Fixture {
        path: PathBuf,
        dev: u64,
        ino: u64,
    }
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "dsh-drafts-PUBLIC-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&path)
                .unwrap();
            let path = path.canonicalize().unwrap();
            let stat = std::fs::symlink_metadata(&path).unwrap();
            Self {
                path,
                dev: stat.dev(),
                ino: stat.ino(),
            }
        }
        fn store(&self) -> Store {
            Store::new(self.path.clone()).unwrap()
        }
        fn put(&self, bytes: &[u8]) {
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(self.path.join(DOCUMENT))
                .unwrap();
            file.write_all(bytes).unwrap();
            file.set_permissions(std::fs::Permissions::from_mode(0o600))
                .unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            // Only this EXCL-created absolute synthetic root is eligible for recursive cleanup.
            let Ok(stat) = std::fs::symlink_metadata(&self.path) else {
                return;
            };
            if stat.is_dir()
                && stat.dev() == self.dev
                && stat.ino() == self.ino
                && self.path.canonicalize().ok().as_ref() == Some(&self.path)
            {
                let _ = std::fs::remove_dir_all(&self.path);
            }
        }
    }
    fn drafts(text: &str) -> Drafts {
        BTreeMap::from([(SessionId::new("PUBLIC-session").unwrap(), text.into())])
    }
    const SAVE_CHECKPOINTS: [SaveCheckpoint; 9] = [
        SaveCheckpoint::BeforeTempWrite,
        SaveCheckpoint::AfterTempWrite,
        SaveCheckpoint::BeforeTempSync,
        SaveCheckpoint::AfterTempSync,
        SaveCheckpoint::BeforeRename,
        SaveCheckpoint::AfterRename,
        SaveCheckpoint::BeforeDirectorySync,
        SaveCheckpoint::AfterDirectorySync,
        SaveCheckpoint::BeforeAcknowledgement,
    ];

    #[test]
    fn real_save_observes_all_checkpoints_in_order_without_changing_private_format() {
        use std::sync::{Arc, Mutex};
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let record = observed.clone();
        store.set_save_observer(Some(Arc::new(move |step| {
            record.lock().unwrap().push(step);
            Ok(())
        })));
        let expected = drafts("PUBLIC delivery unknown\nΚαλημέρα · 日本語 · 🦀 · مرحبا · café");
        store.save(&expected).unwrap();
        assert_eq!(*observed.lock().unwrap(), SAVE_CHECKPOINTS);
        assert!(store.load().unwrap() == expected);
        assert_eq!(
            std::fs::read(fixture.path.join(DOCUMENT)).unwrap(),
            encode(&expected).unwrap()
        );
        for (path, mode) in [
            (fixture.path.clone(), 0o700),
            (fixture.path.join(DOCUMENT), 0o600),
            (fixture.path.join(LOCK), 0o600),
        ] {
            assert_eq!(std::fs::metadata(path).unwrap().mode() & 0o7777, mode);
        }
        store.set_save_observer(None);
        store.save(&Drafts::new()).unwrap();
        assert_eq!(*observed.lock().unwrap(), SAVE_CHECKPOINTS);
    }

    #[test]
    fn checkpoint_failures_preserve_commit_boundary_and_do_not_acknowledge_save() {
        use std::sync::{Arc, Mutex};
        for (index, fail_at) in SAVE_CHECKPOINTS.into_iter().enumerate() {
            let fixture = Fixture::new();
            let mut store = fixture.store();
            let old = drafts("PUBLIC old delivery unknown\n日本語 · café");
            let new = drafts("PUBLIC new delivery unknown\nΚαλημέρα · 🦀 · مرحبا");
            store.save(&old).unwrap();
            let observed = Arc::new(Mutex::new(Vec::new()));
            let record = observed.clone();
            store.set_save_observer(Some(Arc::new(move |step| {
                record.lock().unwrap().push(step);
                if step == fail_at {
                    Err(io::Error::other("PUBLIC injected checkpoint failure"))
                } else {
                    Ok(())
                }
            })));
            let replaced = index >= 5;
            assert_eq!(
                store.save(&new),
                Err(if replaced {
                    Failure::Indeterminate
                } else {
                    Failure::NotSaved
                })
            );
            assert_eq!(
                observed.lock().unwrap().as_slice(),
                &SAVE_CHECKPOINTS[..=index]
            );
            let expected = if replaced { &new } else { &old };
            assert!(fixture.store().load().unwrap() == *expected);
            assert_eq!(
                std::fs::read(fixture.path.join(DOCUMENT)).unwrap(),
                encode(expected).unwrap()
            );
            assert!(std::fs::read_dir(&fixture.path).unwrap().all(|entry| {
                !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".native-drafts-")
            }));
        }
    }

    #[test]
    fn admitted_save_is_unfinished_and_lock_busy_until_callback_release_and_join() {
        use std::sync::{Arc, Mutex, mpsc};
        use std::time::Duration;
        let fixture = Fixture::new();
        let mut store = fixture.store();
        store.save(&drafts("PUBLIC old 日本語")).unwrap();
        let (reached_tx, reached_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let release_rx = Mutex::new(release_rx);
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        store.set_save_observer(Some(Arc::new(move |step| {
            if step == SaveCheckpoint::BeforeTempWrite {
                reached_tx
                    .send(())
                    .map_err(|_| io::Error::other("PUBLIC observer disconnected"))?;
                release_rx
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_millis(3000))
                    .map_err(|_| io::Error::other("PUBLIC release deadline"))?;
            }
            Ok(())
        })));
        let writer = std::thread::spawn(move || {
            let result = store.save(&drafts("PUBLIC new 🦀 · مرحبا"));
            let _ = done_tx.send(result);
            result
        });
        // Collect observations before asserting: even a failing assertion must not strand the gate.
        let reached = reached_rx.recv_timeout(Duration::from_millis(1000));
        let unfinished = done_rx.try_recv();
        let save_busy = fixture.store().save(&drafts("PUBLIC competing writer"));
        let load_busy = fixture.store().load();
        let release = release_tx.send(());
        let completed = done_rx.recv_timeout(Duration::from_millis(3000));
        let joined = writer.join();
        assert!(reached.is_ok());
        assert!(matches!(unfinished, Err(mpsc::TryRecvError::Empty)));
        assert_eq!(save_busy, Err(Failure::Busy));
        assert!(matches!(load_busy, Err(Failure::Busy)));
        assert!(release.is_ok());
        assert_eq!(completed.unwrap(), Ok(()));
        assert_eq!(joined.unwrap(), Ok(()));
        assert!(fixture.store().load().unwrap() == drafts("PUBLIC new 🦀 · مرحبا"));
    }

    #[test]
    fn inert_validation_and_missing_home_do_not_create_paths() {
        let fixture = Fixture::new();
        let missing = fixture.path.join("missing");
        let mut store = Store::new(missing.clone()).unwrap();
        assert!(!missing.exists());
        assert!(store.load().unwrap().is_empty());
        assert_eq!(store.save(&drafts("PUBLIC")), Err(Failure::Unavailable));
        assert!(!missing.exists());
        for path in ["relative", "/", "/a/../b", "/a/./b", "/a//b", "/a/", "/a\n"] {
            assert!(matches!(Store::new(path.into()), Err(Failure::InvalidHome)));
        }
    }
    #[test]
    fn complete_snapshots_round_trip_private_plaintext_without_replay_fields() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        assert!(store.load().unwrap().is_empty());
        assert!(!fixture.path.join(LOCK).exists());
        let first = drafts("PUBLIC unsent\n日本語");
        store.save(&first).unwrap();
        assert!(store.load().unwrap() == first);
        assert_eq!(
            std::fs::metadata(fixture.path.join(DOCUMENT))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(fixture.path.join(LOCK))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777,
            0o600
        );
        let body: serde_json::Value =
            serde_json::from_slice(&std::fs::read(fixture.path.join(DOCUMENT)).unwrap()).unwrap();
        assert_eq!(body.as_object().unwrap().len(), 2);
        assert_eq!(body["version"], 1);
        store.save(&Drafts::new()).unwrap();
        assert!(store.load().unwrap().is_empty());
    }
    #[test]
    fn malformed_unknown_duplicate_and_newer_version_documents_are_preserved() {
        for bytes in [
            b"{".as_slice(),
            b"null",
            b"[1,{}]",
            b"{\"version\":1,\"drafts\":{},\"version\":1}",
            b"{\"version\":2,\"drafts\":{}}",
            b"{\"version\":1,\"drafts\":{},\"requestId\":\"PUBLIC\"}",
            b"{\"version\":1,\"drafts\":{\"PUBLIC\":\"a\",\"PUBLIC\":\"b\"}}",
            b"{\"version\":1,\"drafts\":{\"PUBLIC\":null}}",
        ] {
            let fixture = Fixture::new();
            fixture.put(bytes);
            let mut store = fixture.store();
            assert!(store.load().is_err());
            assert!(store.save(&drafts("NEW PUBLIC")).is_err());
            assert_eq!(std::fs::read(fixture.path.join(DOCUMENT)).unwrap(), bytes);
        }
    }
    #[test]
    fn input_and_encoded_size_limits_refuse_before_replacing() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        store.save(&drafts("KEEP PUBLIC")).unwrap();
        let original = std::fs::read(fixture.path.join(DOCUMENT)).unwrap();
        assert_eq!(
            store.save(&drafts(&"x".repeat(MAX_TEXT_BYTES + 1))),
            Err(Failure::Limit)
        );
        let too_many: Drafts = (0..=MAX_DRAFTS)
            .map(|i| (SessionId::new(format!("PUBLIC-{i}")).unwrap(), "x".into()))
            .collect();
        assert_eq!(store.save(&too_many), Err(Failure::Limit));
        let too_large: Drafts = (0..9)
            .map(|i| {
                (
                    SessionId::new(format!("PUBLIC-{i}")).unwrap(),
                    "x".repeat(MAX_TEXT_BYTES),
                )
            })
            .collect();
        assert_eq!(store.save(&too_large), Err(Failure::Limit));
        assert_eq!(
            std::fs::read(fixture.path.join(DOCUMENT)).unwrap(),
            original
        );
        assert!(decode(&vec![b' '; MAX_DOCUMENT_BYTES + 1]).is_err());
    }
    #[test]
    fn symlink_parent_target_lock_nonregular_and_hardlinked_documents_refuse() {
        let fixture = Fixture::new();
        let private = fixture.path.join("home");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&private)
            .unwrap();
        symlink(&private, fixture.path.join("linked")).unwrap();
        let mut linked = Store::new(fixture.path.join("linked")).unwrap();
        assert!(linked.load().is_err());
        assert!(linked.save(&drafts("PUBLIC")).is_err());
        let mut nested = Store::new(fixture.path.join("linked/nested")).unwrap();
        assert!(nested.load().is_err());
        let keep = fixture.path.join("keep");
        std::fs::write(&keep, b"KEEP PUBLIC").unwrap();
        symlink(&keep, private.join(DOCUMENT)).unwrap();
        let mut store = Store::new(private.clone()).unwrap();
        assert!(store.load().is_err());
        assert!(store.save(&drafts("PUBLIC")).is_err());
        assert_eq!(std::fs::read(&keep).unwrap(), b"KEEP PUBLIC");
        let lock_fixture = Fixture::new();
        symlink(&keep, lock_fixture.path.join(LOCK)).unwrap();
        assert!(lock_fixture.store().save(&drafts("PUBLIC")).is_err());
        let folder_fixture = Fixture::new();
        std::fs::create_dir(folder_fixture.path.join(DOCUMENT)).unwrap();
        assert!(folder_fixture.store().save(&drafts("PUBLIC")).is_err());
        let hard_fixture = Fixture::new();
        hard_fixture.put(&encode(&drafts("KEEP PUBLIC")).unwrap());
        std::fs::hard_link(
            hard_fixture.path.join(DOCUMENT),
            hard_fixture.path.join("hardlink"),
        )
        .unwrap();
        assert!(hard_fixture.store().load().is_err());
        assert!(hard_fixture.store().save(&drafts("PUBLIC")).is_err());
    }
    #[test]
    fn wrong_permissions_and_simulated_owner_check_refuse_without_chmod() {
        let fixture = Fixture::new();
        fixture.put(&encode(&drafts("KEEP PUBLIC")).unwrap());
        let path = fixture.path.join(DOCUMENT);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(fixture.store().load().is_err());
        assert!(fixture.store().save(&drafts("PUBLIC")).is_err());
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o7777,
            0o644
        );
        std::fs::set_permissions(&fixture.path, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(fixture.store().load().is_err());
        assert!(fixture.store().save(&drafts("PUBLIC")).is_err());
        let owned = Fixture::new();
        owned.put(&encode(&drafts("PUBLIC")).unwrap());
        let stat = fstat(File::open(owned.path.join(DOCUMENT)).unwrap()).unwrap();
        assert!(private_regular(&stat, stat.st_uid));
        assert!(!private_regular(&stat, stat.st_uid.wrapping_add(1)));
        let directory = Directory::open(&owned.path, Namespace::Drafts)
            .unwrap()
            .unwrap();
        assert!(!private_directory(
            &directory.stamp,
            directory.uid.wrapping_add(1)
        ));
    }
    #[test]
    fn partial_wrong_and_failed_writes_preserve_the_exact_previous_document() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        store.save(&drafts("KEEP PUBLIC")).unwrap();
        let original = std::fs::read(fixture.path.join(DOCUMENT)).unwrap();
        assert_eq!(
            store.save_with(&drafts("NEW PUBLIC"), |file, _| {
                file.write_all(b"{")?;
                Err(io::ErrorKind::WriteZero.into())
            }),
            Err(Failure::NotSaved)
        );
        assert_eq!(
            std::fs::read(fixture.path.join(DOCUMENT)).unwrap(),
            original
        );
        assert_eq!(
            store.save_with(&drafts("NEW PUBLIC"), |file, _| file.write_all(b"{")),
            Err(Failure::NotSaved)
        );
        assert_eq!(
            std::fs::read(fixture.path.join(DOCUMENT)).unwrap(),
            original
        );
        assert_eq!(
            store.save_with(&drafts("NEW PUBLIC"), |file, bytes| file
                .write_all(&vec![b'x'; bytes.len()])),
            Err(Failure::NotSaved)
        );
        assert_eq!(
            std::fs::read(fixture.path.join(DOCUMENT)).unwrap(),
            original
        );
    }
    #[test]
    fn maximum_text_and_map_boundaries_accept_and_oversized_disk_file_refuses() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let maximum: Drafts = (0..8)
            .map(|i| {
                (
                    SessionId::new(format!("PUBLIC-{i}")).unwrap(),
                    "x".repeat(MAX_TEXT_BYTES),
                )
            })
            .collect();
        store.save(&maximum).unwrap();
        assert!(store.load().unwrap() == maximum);
        let count: Drafts = (0..MAX_DRAFTS)
            .map(|i| {
                (
                    SessionId::new(format!("PUBLIC-{i}")).unwrap(),
                    "PUBLIC".into(),
                )
            })
            .collect();
        store.save(&count).unwrap();
        assert!(store.load().unwrap() == count);
        let invalid = BTreeMap::from([(
            SessionId::new("x".repeat(MAX_KEY_BYTES + 1)).unwrap(),
            "PUBLIC".into(),
        )]);
        assert_eq!(store.save(&invalid), Err(Failure::InvalidData));
        let oversized = Fixture::new();
        oversized.put(&vec![b' '; MAX_DOCUMENT_BYTES + 1]);
        let original = std::fs::metadata(oversized.path.join(DOCUMENT)).unwrap();
        assert!(oversized.store().load().is_err());
        assert!(oversized.store().save(&drafts("PUBLIC")).is_err());
        let retained = std::fs::metadata(oversized.path.join(DOCUMENT)).unwrap();
        assert_eq!(
            (retained.ino(), retained.len()),
            (original.ino(), original.len())
        );
    }
    #[test]
    fn cooperating_writer_is_busy_and_late_target_change_is_not_overwritten() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        store.save(&drafts("KEEP PUBLIC")).unwrap();
        let directory = Directory::open(&fixture.path, Namespace::Drafts)
            .unwrap()
            .unwrap();
        let held = directory.lock(true).unwrap().unwrap();
        assert_eq!(store.save(&drafts("NEW PUBLIC")), Err(Failure::Busy));
        assert_eq!(store.load().err(), Some(Failure::Busy));
        drop(held);
        let changed = encode(&drafts("OTHER PUBLIC")).unwrap();
        let path = fixture.path.join(DOCUMENT);
        let result = store.save_with(&drafts("NEW PUBLIC"), |file, bytes| {
            file.write_all(bytes)?;
            std::fs::write(&path, &changed)
        });
        assert_eq!(result, Err(Failure::Conflict));
        assert_eq!(std::fs::read(path).unwrap(), changed);
    }
    #[test]
    fn layout_width_preference_round_trips_and_rejects_junk() {
        for width in [640.0_f32, 748.5, 920.0, 4096.0] {
            let raw = format_layout_width(width).expect("storable");
            assert_eq!(parse_layout_width(&raw), Some(width));
        }
        assert_eq!(parse_layout_width("  812.5 "), Some(812.5));
        for raw in ["", "x", "NaN", "inf", "0", "-20", "5000", "1e9"] {
            assert_eq!(parse_layout_width(raw), None, "value {raw:?}");
        }
        for width in [f32::NAN, f32::INFINITY, 0.0, -1.0, 4097.0] {
            assert_eq!(format_layout_width(width), None, "width {width}");
        }
    }
}

#[cfg(test)]
mod open_in_app_choice_tests {
    //! The remembered app: accepted only when it is a catalog-shaped id.
    use super::*;

    #[test]
    fn only_a_catalog_shaped_choice_is_stored() {
        assert_eq!(
            parse_open_in_app_choice("vscode").as_deref(),
            Some("vscode")
        );
        assert_eq!(
            parse_open_in_app_choice("  gnometerminal ").as_deref(),
            Some("gnometerminal")
        );
        let long = "x".repeat(33);
        for rejected in ["", "   ", "VS Code", "vs-code", "vscode.exe", long.as_str()] {
            assert!(parse_open_in_app_choice(rejected).is_none(), "{rejected:?}");
        }
    }
}

#[cfg(test)]
mod terminal_shell_preference_tests {
    //! The stored shell path: accepted only when it is one.
    use super::*;

    #[test]
    fn only_an_absolute_bounded_path_is_stored() {
        assert_eq!(
            parse_terminal_shell("/bin/zsh").as_deref(),
            Some("/bin/zsh")
        );
        assert_eq!(
            parse_terminal_shell("  /usr/bin/fish  ").as_deref(),
            Some("/usr/bin/fish")
        );
        for rejected in ["", "   ", "bin/zsh", "./zsh", "/bin/z\x07sh"] {
            assert!(parse_terminal_shell(rejected).is_none(), "{rejected:?}");
        }
        // The bound matches the layout namespace's own text limit.
        let long = format!("/{}", "a".repeat(4_096));
        assert!(parse_terminal_shell(&long).is_none());
    }
}
