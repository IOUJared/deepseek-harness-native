//! Bounded native directory picker. Selection is local; upload always needs separate consent.
use super::attachment_draft::EditorStamp;
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

const MAX_VISITED: usize = 4096;
const MAX_ENTRIES: usize = 512;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub stamp: EditorStamp,
    pub serial: u64,
}
#[derive(Clone)]
pub enum Action {
    Open(EditorStamp),
    Path(Ticket, String),
    Go(Ticket),
    Up(Ticket),
    Home(Ticket),
    Hidden(Ticket),
    Choose(Ticket, usize),
    Manual(Ticket),
    Close(Ticket),
}
#[derive(Clone)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub directory: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Unavailable,
    TooMany,
    InvalidPath,
    Canceled,
}
impl Failure {
    pub fn text(self) -> &'static str {
        match self {
            Self::Unavailable => "Folder unavailable. Enter another folder or cancel.",
            Self::TooMany => {
                "Folder exceeds the picker limit. Use a smaller folder or enter the file path directly."
            }
            Self::InvalidPath => {
                "Use an absolute UTF-8 folder path, up to 4096 bytes, without control characters."
            }
            Self::Canceled => "Folder read canceled.",
        }
    }
}
#[derive(Clone)]
pub struct Completion {
    pub ticket: Ticket,
    pub result: Result<Vec<Entry>, Failure>,
}
pub struct Request {
    pub ticket: Ticket,
    pub path: PathBuf,
    pub hidden: bool,
    pub canceled: Arc<AtomicBool>,
}
#[derive(Default)]
pub struct Picker {
    visible: Option<Ticket>,
    serial: u64,
    input_floor: u64,
    path: String,
    directory: PathBuf,
    hidden: bool,
    entries: Vec<Entry>,
    notice: Option<Failure>,
    pending: Option<(Ticket, Arc<AtomicBool>)>,
}
impl Picker {
    pub fn ticket(&self) -> Option<Ticket> {
        self.visible
    }
    pub fn is_open(&self) -> bool {
        self.visible.is_some()
    }
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn hidden(&self) -> bool {
        self.hidden
    }
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
    pub fn notice(&self) -> Option<Failure> {
        self.notice
    }
    fn advance(&mut self, stamp: EditorStamp) -> Option<Ticket> {
        let Some(serial) = self.serial.checked_add(1) else {
            self.invalidate();
            return None;
        };
        self.serial = serial;
        let ticket = Ticket {
            stamp,
            serial: self.serial,
        };
        self.visible = Some(ticket);
        Some(ticket)
    }
    pub fn open(&mut self, stamp: EditorStamp, path: PathBuf) -> Option<Request> {
        if self.is_open() || self.busy() {
            return None;
        }
        self.advance(stamp)?;
        self.input_floor = self.serial;
        self.hidden = false;
        self.entries.clear();
        self.path.clear();
        self.directory = PathBuf::new();
        if !valid_path(&path) {
            self.notice = Some(Failure::InvalidPath);
            return None;
        }
        self.path = path.to_str().expect("validated UTF-8 folder").to_owned();
        self.directory = path;
        self.notice = None;
        self.read(self.visible?)
    }
    pub fn edit(&mut self, ticket: Ticket, path: String) {
        if self.visible.is_none_or(|current| {
            current.stamp != ticket.stamp
                || ticket.serial < self.input_floor
                || ticket.serial > current.serial
        }) {
            return;
        }
        if path.len() > 4096 || path.chars().any(char::is_control) {
            self.path.clear();
            self.entries.clear();
            self.advance(ticket.stamp);
            self.notice = Some(Failure::InvalidPath);
            return;
        }
        self.path = path;
        // Editing invalidates any displayed entry action and in-flight adoption.
        self.entries.clear();
        self.advance(ticket.stamp);
    }
    pub fn navigate(&mut self, ticket: Ticket, path: PathBuf) -> Option<Request> {
        if self.visible != Some(ticket) || self.busy() {
            return None;
        }
        let Some(path_text) = path.to_str() else {
            self.notice = Some(Failure::InvalidPath);
            return None;
        };
        if !valid_path(&path) {
            self.notice = Some(Failure::InvalidPath);
            return None;
        }
        self.path = path_text.to_owned();
        self.directory = path;
        self.entries.clear();
        let fresh = self.advance(ticket.stamp)?;
        self.input_floor = fresh.serial;
        self.read(fresh)
    }
    pub fn toggle_hidden(&mut self, ticket: Ticket) -> Option<Request> {
        if self.visible != Some(ticket) || self.busy() {
            return None;
        }
        self.hidden = !self.hidden;
        self.navigate(ticket, self.directory.clone())
    }
    pub fn selected(&self, ticket: Ticket, index: usize) -> Option<Entry> {
        if self.visible != Some(ticket) || self.busy() {
            return None;
        }
        self.entries.get(index).cloned()
    }
    pub fn close(&mut self, ticket: Ticket) {
        if self.visible == Some(ticket) {
            self.invalidate();
        }
    }
    pub fn invalidate(&mut self) {
        self.visible = None;
        self.entries.clear();
        self.path.clear();
        self.notice = None;
        if let Some((_, canceled)) = &self.pending {
            canceled.store(true, Ordering::Relaxed);
        }
    }
    fn read(&mut self, ticket: Ticket) -> Option<Request> {
        if self.busy() {
            return None;
        }
        let path = PathBuf::from(&self.path);
        if !valid_path(&path) {
            self.notice = Some(Failure::InvalidPath);
            return None;
        }
        let canceled = Arc::new(AtomicBool::new(false));
        self.pending = Some((ticket, canceled.clone()));
        self.notice = None;
        Some(Request {
            ticket,
            path,
            hidden: self.hidden,
            canceled,
        })
    }
    pub fn complete(&mut self, result: &Completion) {
        if self.pending.as_ref().map(|(t, _)| *t) != Some(result.ticket) {
            return;
        }
        self.pending = None;
        if self.visible != Some(result.ticket) {
            return;
        }
        match &result.result {
            Ok(entries) => {
                self.entries = entries.clone();
                self.notice = None;
            }
            Err(failure) => {
                self.entries.clear();
                self.notice = Some(*failure);
            }
        }
    }
}
fn valid_path(path: &Path) -> bool {
    path.is_absolute()
        && path
            .to_str()
            .is_some_and(|s| s.len() <= 4096 && !s.chars().any(char::is_control))
}
pub fn scan(path: &Path, hidden: bool, canceled: impl Fn() -> bool) -> Result<Vec<Entry>, Failure> {
    if !valid_path(path) {
        return Err(Failure::InvalidPath);
    }
    if canceled() {
        return Err(Failure::Canceled);
    }
    let folder = std::fs::read_dir(path).map_err(|_| Failure::Unavailable)?;
    let mut entries = Vec::new();
    for (visited, entry) in folder.enumerate() {
        if canceled() {
            return Err(Failure::Canceled);
        }
        if visited >= MAX_VISITED {
            return Err(Failure::TooMany);
        }
        let entry = entry.map_err(|_| Failure::Unavailable)?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if (!hidden && name.starts_with('.')) || name.chars().any(char::is_control) {
            continue;
        }
        let kind = entry.file_type().map_err(|_| Failure::Unavailable)?;
        if !kind.is_file() && !kind.is_dir() {
            continue;
        }
        let path = entry.path();
        if !valid_path(&path) {
            continue;
        }
        if kind.is_file() && crate::worker::file_intake::SelectedFile::new(path.clone()).is_err() {
            continue;
        }
        if entries.len() == MAX_ENTRIES {
            return Err(Failure::TooMany);
        }
        entries.push(Entry {
            name: name.to_owned(),
            path,
            directory: kind.is_dir(),
        });
    }
    if canceled() {
        return Err(Failure::Canceled);
    }
    entries.sort_by(|a, b| {
        b.directory
            .cmp(&a.directory)
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(entries)
}
pub async fn operation(request: Request) -> Completion {
    let ticket = request.ticket;
    let result = tokio::task::spawn_blocking(move || {
        scan(&request.path, request.hidden, || {
            request.canceled.load(Ordering::Relaxed)
        })
    })
    .await
    .unwrap_or(Err(Failure::Unavailable));
    Completion { ticket, result }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::Cell,
        collections::hash_map::RandomState,
        ffi::OsString,
        fs::{self, DirBuilder},
        hash::{BuildHasher, Hasher},
        os::unix::{
            ffi::OsStringExt,
            fs::{DirBuilderExt, symlink},
        },
        sync::atomic::AtomicU64,
    };

    const TEMP_PREFIX: &str = "dsh-native-picker-";
    static SERIAL: AtomicU64 = AtomicU64::new(0);

    struct Temp {
        parent: PathBuf,
        root: PathBuf,
    }
    impl Temp {
        fn new() -> Self {
            let parent = std::env::temp_dir().canonicalize().unwrap();
            assert!(parent.is_absolute());
            loop {
                let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
                let mut hash = RandomState::new().build_hasher();
                hash.write_u64(serial);
                let root = parent.join(format!(
                    "{TEMP_PREFIX}{}-{:x}-{serial}",
                    std::process::id(),
                    hash.finish(),
                ));
                match DirBuilder::new().mode(0o700).create(&root) {
                    Ok(()) => return Self { parent, root },
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("cannot create private picker fixture: {error}"),
                }
            }
        }
        fn path(&self, name: &str) -> PathBuf {
            assert!(!name.is_empty() && !name.contains('/') && name != "." && name != "..");
            self.root.join(name)
        }
        fn file(&self, name: &str) -> PathBuf {
            let path = self.path(name);
            fs::write(&path, b"PUBLIC picker fixture").unwrap();
            path
        }
        fn directory(&self, name: &str) -> PathBuf {
            let path = self.path(name);
            fs::create_dir(&path).unwrap();
            path
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            // Check the exact owned target before deleting the private fixture tree.
            assert!(self.root.is_absolute());
            assert_eq!(self.root.parent(), Some(self.parent.as_path()));
            assert!(
                self.root
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with(TEMP_PREFIX)
            );
            let metadata = fs::symlink_metadata(&self.root).unwrap();
            assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
            assert_eq!(self.root.canonicalize().unwrap(), self.root);
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    fn names(entries: &[Entry]) -> Vec<(&str, bool)> {
        entries
            .iter()
            .map(|entry| (entry.name.as_str(), entry.directory))
            .collect()
    }
    fn stamp() -> EditorStamp {
        let mut draft = super::super::attachment_draft::Draft::default();
        let context = super::super::attachment_draft::Context {
            epoch: 1,
            generation: 2,
            target: Some(dsh_native_transport::dto::SessionId::new("PUBLIC picker").unwrap()),
            allowed: true,
        };
        assert!(draft.open(&context));
        draft.editor().unwrap().0
    }
    #[test]
    fn stale_results_release_only_their_operation_and_never_restore_closed_picker() {
        let mut picker = Picker::default();
        let request = picker.open(stamp(), PathBuf::from("/tmp")).unwrap();
        picker.close(request.ticket);
        assert!(request.canceled.load(Ordering::Relaxed));
        assert!(picker.busy());
        assert!(picker.open(stamp(), PathBuf::from("/tmp")).is_none());
        let wrong = Ticket {
            serial: request.ticket.serial + 1,
            ..request.ticket
        };
        picker.complete(&Completion {
            ticket: wrong,
            result: Ok(vec![]),
        });
        assert!(picker.busy());
        picker.complete(&Completion {
            ticket: request.ticket,
            result: Ok(vec![]),
        });
        assert!(!picker.busy());
        assert!(!picker.is_open());
        let fresh = picker.open(stamp(), PathBuf::from("/tmp")).unwrap();
        assert_ne!(fresh.ticket, request.ticket);
    }
    #[test]
    fn edited_folder_revokes_old_entries_and_late_adoption() {
        let mut picker = Picker::default();
        let request = picker.open(stamp(), PathBuf::from("/tmp")).unwrap();
        picker.edit(request.ticket, "/new".into());
        picker.complete(&Completion {
            ticket: request.ticket,
            result: Ok(vec![Entry {
                name: "old".into(),
                path: PathBuf::from("/tmp/old"),
                directory: false,
            }]),
        });
        assert!(picker.entries().is_empty());
        assert!(picker.selected(request.ticket, 0).is_none());
        assert!(!picker.busy());
        assert!(
            picker
                .navigate(request.ticket, PathBuf::from("/tmp"))
                .is_none()
        );
    }
    #[test]
    fn disk_scan_sorts_directories_first_and_applies_hidden_flag() {
        let temp = Temp::new();
        temp.file("z-file");
        temp.directory("z-directory");
        temp.file("a-file");
        temp.directory("a-directory");
        temp.file(".hidden-file");
        temp.directory(".hidden-directory");
        let visible = scan(&temp.root, false, || false).unwrap();
        assert_eq!(
            names(&visible),
            vec![
                ("a-directory", true),
                ("z-directory", true),
                ("a-file", false),
                ("z-file", false),
            ]
        );
        let all = scan(&temp.root, true, || false).unwrap();
        assert_eq!(
            names(&all),
            vec![
                (".hidden-directory", true),
                ("a-directory", true),
                ("z-directory", true),
                (".hidden-file", false),
                ("a-file", false),
                ("z-file", false),
            ]
        );
        for entry in all {
            assert_eq!(entry.path, temp.path(&entry.name));
        }
    }

    #[test]
    fn disk_scan_skips_symlinks_fifo_non_utf8_and_control_names() {
        let temp = Temp::new();
        let regular = temp.file("regular");
        let directory = temp.directory("directory");
        symlink(&regular, temp.path("file-link")).unwrap();
        symlink(&directory, temp.path("directory-link")).unwrap();
        symlink(temp.path("missing"), temp.path("broken-link")).unwrap();
        rustix::fs::mkfifoat(
            rustix::fs::CWD,
            temp.path("fifo"),
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        )
        .unwrap();
        fs::write(
            temp.root
                .join(OsString::from_vec(b"non-utf8-\xff".to_vec())),
            b"PUBLIC",
        )
        .unwrap();
        fs::create_dir(
            temp.root
                .join(OsString::from_vec(b"non-utf8-dir-\xff".to_vec())),
        )
        .unwrap();
        temp.file("control\nfile");
        temp.directory("control\tdirectory");
        for hidden in [false, true] {
            let entries = scan(&temp.root, hidden, || false).unwrap();
            assert_eq!(
                names(&entries),
                vec![("directory", true), ("regular", false)]
            );
        }
    }

    #[test]
    fn disk_scan_accepts_512_entries_and_rejects_513_without_partial_success() {
        let temp = Temp::new();
        for index in 0..MAX_ENTRIES {
            temp.file(&format!("PUBLIC-{index:04}"));
        }
        assert_eq!(
            scan(&temp.root, false, || false).unwrap().len(),
            MAX_ENTRIES
        );
        temp.file("PUBLIC-one-over");
        assert_eq!(
            scan(&temp.root, false, || false).err(),
            Some(Failure::TooMany)
        );
    }

    #[test]
    fn disk_scan_cancellation_wins_before_open_and_during_iteration() {
        let temp = Temp::new();
        temp.file("PUBLIC file");
        let missing = temp.path("PUBLIC missing");
        assert_eq!(
            scan(&missing, false, || true).err(),
            Some(Failure::Canceled)
        );
        assert!(!missing.exists());
        let calls = Cell::new(0);
        assert_eq!(
            scan(&temp.root, false, || {
                calls.set(calls.get() + 1);
                calls.get() == 2
            })
            .err(),
            Some(Failure::Canceled)
        );
        assert_eq!(calls.get(), 2);
        // Cancellation after an empty iterator also prevents adopting a completed result.
        let empty = temp.directory("PUBLIC empty");
        let calls = Cell::new(0);
        assert_eq!(
            scan(&empty, false, || {
                calls.set(calls.get() + 1);
                calls.get() == 2
            })
            .err(),
            Some(Failure::Canceled)
        );
    }

    #[tokio::test]
    async fn real_operation_keeps_admission_until_owner_complete_and_toggles_hidden() {
        let temp = Temp::new();
        temp.file("PUBLIC file");
        temp.file(".PUBLIC hidden");
        let mut picker = Picker::default();
        let request = picker.open(stamp(), temp.root.clone()).unwrap();
        let initial = request.ticket;
        assert!(picker.busy());
        assert!(!request.hidden);
        let completion = operation(request).await;
        assert_eq!(completion.ticket, initial);
        assert_eq!(
            names(completion.result.as_ref().unwrap()),
            vec![("PUBLIC file", false)]
        );
        assert!(picker.busy());
        assert!(picker.entries().is_empty());
        picker.complete(&completion);
        assert!(!picker.busy());
        assert!(picker.is_open());
        assert_eq!(names(picker.entries()), vec![("PUBLIC file", false)]);
        assert_eq!(picker.notice(), None);

        let request = picker.toggle_hidden(initial).unwrap();
        assert!(request.hidden);
        assert!(picker.hidden());
        assert_ne!(request.ticket, initial);
        assert!(picker.busy());
        let completion = operation(request).await;
        assert!(picker.busy());
        picker.complete(&completion);
        assert!(!picker.busy());
        assert_eq!(
            names(picker.entries()),
            vec![(".PUBLIC hidden", false), ("PUBLIC file", false)]
        );
        assert!(picker.selected(initial, 0).is_none());
    }

    #[tokio::test]
    async fn canceled_real_operation_drains_without_reopening_closed_picker() {
        let temp = Temp::new();
        temp.file("PUBLIC file");
        let mut picker = Picker::default();
        let request = picker.open(stamp(), temp.root.clone()).unwrap();
        picker.close(request.ticket);
        assert!(request.canceled.load(Ordering::Relaxed));
        let completion = operation(request).await;
        assert_eq!(
            completion.result.as_ref().err().copied(),
            Some(Failure::Canceled)
        );
        assert!(picker.busy());
        picker.complete(&completion);
        assert!(!picker.busy());
        assert!(!picker.is_open());
        assert!(picker.entries().is_empty());
        assert_eq!(picker.notice(), None);
    }

    #[test]
    fn scan_rejects_invalid_and_cancelled_paths_without_echoing_private_errors() {
        assert_eq!(
            scan(Path::new("relative"), false, || false).err(),
            Some(Failure::InvalidPath)
        );
        assert_eq!(
            scan(Path::new("/PRIVATE-missing"), false, || true).err(),
            Some(Failure::Canceled)
        );
        assert!(!Failure::Unavailable.text().contains("PRIVATE"));
    }
}
