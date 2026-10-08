//! One-use local selection; blocking intake belongs on an owned, bounded worker, never the UI.
use dsh_native_transport::upload::{FileUpload, MAX_UPLOAD_BYTES};
use rustix::fs::{self, FileType, Mode, OFlags};
use std::{
    fmt,
    fs::File,
    io::{self, Read},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

const MAX_PATH_BYTES: usize = 4096;
const READ_CHUNK_BYTES: usize = 64 * 1024;

/// A consumed local capability, not a serializable path or a reusable upload body.
pub struct SelectedFile {
    path: PathBuf,
}

/// Closed, path-free failures. No OS error, file name or file body is retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    InvalidSelection,
    Unavailable,
    NotRegular,
    TooLarge,
    Canceled,
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidSelection => "Invalid local file selection",
            Self::Unavailable => "Selected file is unavailable",
            Self::NotRegular => "Selected file is not a regular file",
            Self::TooLarge => "Selected file exceeds the intake limit",
            Self::Canceled => "Local file intake canceled",
        })
    }
}
impl std::error::Error for Failure {}

impl fmt::Debug for SelectedFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SelectedFile { .. }")
    }
}
impl SelectedFile {
    /// Validate only the absolute Linux path and display leaf; no open, stat or read.
    pub fn new(path: PathBuf) -> Result<Self, Failure> {
        let raw = path.as_os_str().as_bytes();
        if !path.is_absolute() || raw.len() > MAX_PATH_BYTES || raw.contains(&0) {
            return Err(Failure::InvalidSelection);
        }
        FileUpload::new(Vec::new(), Some(leaf(&path)?.to_owned()))
            .map_err(|_| Failure::InvalidSelection)?;
        Ok(Self { path })
    }

    /// Synchronous WORKER-ONLY read; the caller owns worker admission, cancellation and draining.
    /// NONBLOCK prevents FIFO open waits, not blocking regular-file kernel I/O on a bad filesystem.
    /// Cancellation is cooperative, not kernel-I/O cancellation, confinement or an atomic snapshot.
    /// NOFOLLOW rejects the final symlink only; parent symlinks and ordinary path traversal remain.
    pub fn read(self, canceled: impl Fn() -> bool) -> Result<FileUpload, Failure> {
        if canceled() {
            return Err(Failure::Canceled);
        }
        let name = leaf(&self.path)?.to_owned();
        let fd = fs::open(
            &self.path,
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| Failure::Unavailable)?;
        if canceled() {
            return Err(Failure::Canceled);
        }
        let stat = fs::fstat(&fd).map_err(|_| Failure::Unavailable)?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
            return Err(Failure::NotRegular);
        }
        let size = u64::try_from(stat.st_size).map_err(|_| Failure::Unavailable)?;
        read_upload(File::from(fd), size, name, canceled)
    }
}

fn leaf(path: &Path) -> Result<&str, Failure> {
    // Use the literal last segment: Path::file_name normalizes away trailing slash/dot.
    let bytes = path.as_os_str().as_bytes();
    std::str::from_utf8(
        bytes
            .rsplit(|byte| *byte == b'/')
            .next()
            .unwrap_or_default(),
    )
    .map_err(|_| Failure::InvalidSelection)
}

fn read_upload(
    mut reader: impl Read,
    initial_size: u64,
    name: String,
    canceled: impl Fn() -> bool,
) -> Result<FileUpload, Failure> {
    if canceled() {
        return Err(Failure::Canceled);
    }
    if initial_size > MAX_UPLOAD_BYTES as u64 {
        return Err(Failure::TooLarge);
    }
    // Fixed ceiling: neither stale metadata nor Vec growth can allocate an oversized body.
    let mut bytes = vec![0; MAX_UPLOAD_BYTES];
    let mut used = 0;
    let mut probe = [0; 1];
    loop {
        if canceled() {
            return Err(Failure::Canceled);
        }
        let at_limit = used == MAX_UPLOAD_BYTES;
        let result = if at_limit {
            reader.read(&mut probe)
        } else {
            let end = (used + READ_CHUNK_BYTES).min(MAX_UPLOAD_BYTES);
            reader.read(&mut bytes[used..end])
        };
        match result {
            Ok(0) => break,
            Ok(_) if at_limit => return Err(Failure::TooLarge),
            Ok(count) => used += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(Failure::Unavailable),
        }
    }
    bytes.truncate(used);
    if canceled() {
        return Err(Failure::Canceled);
    }
    FileUpload::new(bytes, Some(name)).map_err(|_| Failure::InvalidSelection)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::Cell,
        collections::hash_map::RandomState,
        ffi::OsString,
        fs::{self as std_fs, DirBuilder},
        hash::{BuildHasher, Hasher},
        os::unix::{
            ffi::OsStringExt,
            fs::{DirBuilderExt, symlink},
            net::UnixListener,
        },
        sync::atomic::{AtomicU64, Ordering},
    };

    const TEMP_PREFIX: &str = "dsh-native-file-intake-";
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
                let nonce = hash.finish();
                let root = parent.join(format!(
                    "{TEMP_PREFIX}{}-{nonce:x}-{serial}",
                    std::process::id()
                ));
                match DirBuilder::new().mode(0o700).create(&root) {
                    Ok(()) => return Self { parent, root },
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("cannot create private fixture: {error}"),
                }
            }
        }
        fn path(&self, name: &str) -> PathBuf {
            assert!(!name.is_empty() && !name.contains('/') && name != "." && name != "..");
            self.root.join(name)
        }
        fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.path(name);
            std_fs::write(&path, bytes).unwrap();
            path
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            // Delete only this exact privately-created root after checking its resolved identity.
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
            let metadata = std_fs::symlink_metadata(&self.root).unwrap();
            assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
            assert_eq!(self.root.canonicalize().unwrap(), self.root);
            std_fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[derive(Default)]
    struct CountingReader {
        remaining: usize,
        total: usize,
        requests: Vec<usize>,
        interruptions: usize,
        probe_interruptions: usize,
        fail: bool,
        probe_fail: bool,
    }
    impl CountingReader {
        fn new(remaining: usize) -> Self {
            Self {
                remaining,
                ..Self::default()
            }
        }
    }
    impl Read for CountingReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.requests.push(buf.len());
            if self.interruptions != 0 {
                self.interruptions -= 1;
                return Err(io::Error::from(io::ErrorKind::Interrupted));
            }
            if buf.len() == 1 && self.probe_interruptions != 0 {
                self.probe_interruptions -= 1;
                return Err(io::Error::from(io::ErrorKind::Interrupted));
            }
            if self.fail || (buf.len() == 1 && self.probe_fail) {
                return Err(io::Error::other("PRIVATE_IO_ERROR"));
            }
            let count = self.remaining.min(buf.len());
            buf[..count].fill(0xff);
            self.remaining -= count;
            self.total += count;
            Ok(count)
        }
    }
    fn synthetic(reader: impl Read, size: u64) -> Result<FileUpload, Failure> {
        read_upload(reader, size, "fixture.bin".into(), || false)
    }

    #[test]
    fn constructor_does_not_require_an_existing_file() {
        let temp = Temp::new();
        assert!(SelectedFile::new(temp.path("absent.bin")).is_ok());
    }
    #[test]
    fn relative_and_missing_literal_leaves_are_invalid() {
        for path in [
            "",
            "relative.bin",
            "/",
            "/parent/",
            "/parent/.",
            "/parent/..",
        ] {
            assert_eq!(
                SelectedFile::new(path.into()).unwrap_err(),
                Failure::InvalidSelection
            );
        }
    }
    #[test]
    fn invalid_names_follow_transport_constructor_policy() {
        for name in ["bad\\leaf", "bad\nleaf", "bad\0leaf", "bad\u{0085}leaf"] {
            assert_eq!(
                SelectedFile::new(PathBuf::from(format!("/parent/{name}"))).unwrap_err(),
                Failure::InvalidSelection
            );
        }
        assert_eq!(
            SelectedFile::new(PathBuf::from(format!("/parent/{}", "é".repeat(513)))).unwrap_err(),
            Failure::InvalidSelection
        );
        assert!(SelectedFile::new(PathBuf::from(format!("/parent/{}", "é".repeat(512)))).is_ok());
    }
    #[test]
    fn non_utf8_leaf_is_invalid_but_non_utf8_parent_is_allowed() {
        let bad_leaf = PathBuf::from(OsString::from_vec(b"/parent/\xff".to_vec()));
        assert_eq!(
            SelectedFile::new(bad_leaf).unwrap_err(),
            Failure::InvalidSelection
        );
        let parent = PathBuf::from(OsString::from_vec(b"/\xff/leaf.bin".to_vec()));
        assert!(SelectedFile::new(parent).is_ok());
    }
    #[test]
    fn nul_in_parent_is_invalid() {
        assert_eq!(
            SelectedFile::new(PathBuf::from(OsString::from_vec(
                b"/bad\0parent/leaf".to_vec()
            )))
            .unwrap_err(),
            Failure::InvalidSelection
        );
    }
    #[test]
    fn absolute_path_limit_counts_multibyte_linux_bytes() {
        let exact = format!("/{}/a", "é".repeat(2046) + "x");
        assert_eq!(exact.len(), MAX_PATH_BYTES);
        assert!(SelectedFile::new(exact.into()).is_ok());
        let over = format!("/{}/a", "é".repeat(2047));
        assert_eq!(over.len(), MAX_PATH_BYTES + 1);
        assert_eq!(
            SelectedFile::new(over.into()).unwrap_err(),
            Failure::InvalidSelection
        );
    }
    #[test]
    fn debug_and_failures_are_redacted() {
        let selected = SelectedFile::new("/PRIVATE_PARENT/PRIVATE_LEAF".into()).unwrap();
        assert_eq!(format!("{selected:?}"), "SelectedFile { .. }");
        for failure in [
            Failure::InvalidSelection,
            Failure::Unavailable,
            Failure::NotRegular,
            Failure::TooLarge,
            Failure::Canceled,
        ] {
            for text in [format!("{failure:?}"), failure.to_string()] {
                assert!(!text.contains("PRIVATE_"));
            }
            assert!(std::error::Error::source(&failure).is_none());
        }
    }
    #[test]
    fn real_empty_and_arbitrary_bytes_are_allowed() {
        let temp = Temp::new();
        let empty = SelectedFile::new(temp.file("empty.bin", &[]))
            .unwrap()
            .read(|| false)
            .unwrap();
        assert!(empty.is_empty());
        let binary = SelectedFile::new(temp.file("binary.bin", &[0, 255, 128, 10]))
            .unwrap()
            .read(|| false)
            .unwrap();
        assert_eq!(binary.len(), 4);
    }
    #[test]
    fn real_exact_limit_succeeds_and_limit_plus_one_is_rejected() {
        let temp = Temp::new();
        let exact = temp.path("exact.bin");
        File::create(&exact)
            .unwrap()
            .set_len(MAX_UPLOAD_BYTES as u64)
            .unwrap();
        assert_eq!(
            SelectedFile::new(exact)
                .unwrap()
                .read(|| false)
                .unwrap()
                .len(),
            MAX_UPLOAD_BYTES
        );
        let over = temp.path("over.bin");
        File::create(&over)
            .unwrap()
            .set_len(MAX_UPLOAD_BYTES as u64 + 1)
            .unwrap();
        assert_eq!(
            SelectedFile::new(over).unwrap().read(|| false).unwrap_err(),
            Failure::TooLarge
        );
    }
    #[test]
    fn missing_file_is_unavailable() {
        let temp = Temp::new();
        assert_eq!(
            SelectedFile::new(temp.path("missing"))
                .unwrap()
                .read(|| false)
                .unwrap_err(),
            Failure::Unavailable
        );
    }
    #[test]
    fn cancellation_precedes_open_even_for_missing_file() {
        let temp = Temp::new();
        assert_eq!(
            SelectedFile::new(temp.path("missing"))
                .unwrap()
                .read(|| true)
                .unwrap_err(),
            Failure::Canceled
        );
    }
    #[test]
    fn directory_is_not_regular() {
        let temp = Temp::new();
        assert_eq!(
            SelectedFile::new(temp.root.clone())
                .unwrap()
                .read(|| false)
                .unwrap_err(),
            Failure::NotRegular
        );
    }
    #[test]
    fn symlink_leaf_is_not_followed() {
        let temp = Temp::new();
        let target = temp.file("target", b"private bytes");
        for (name, target) in [
            ("file-link", target),
            ("directory-link", temp.root.clone()),
            ("broken-link", temp.path("missing")),
        ] {
            let link = temp.path(name);
            symlink(target, &link).unwrap();
            assert_eq!(
                SelectedFile::new(link).unwrap().read(|| false).unwrap_err(),
                Failure::Unavailable
            );
        }
    }
    #[test]
    fn fifo_without_writer_is_opened_nonblocking_then_rejected() {
        let temp = Temp::new();
        let fifo = temp.path("fifo");
        fs::mkfifoat(fs::CWD, &fifo, Mode::RUSR | Mode::WUSR).unwrap();
        assert_eq!(
            SelectedFile::new(fifo).unwrap().read(|| false).unwrap_err(),
            Failure::NotRegular
        );
    }
    #[test]
    fn socket_leaf_is_rejected() {
        let temp = Temp::new();
        let socket = temp.path("socket");
        let _listener = UnixListener::bind(&socket).unwrap();
        assert_eq!(
            SelectedFile::new(socket)
                .unwrap()
                .read(|| false)
                .unwrap_err(),
            Failure::Unavailable
        );
    }
    #[test]
    fn oversized_preflight_never_reads() {
        let mut reader = CountingReader::new(1);
        assert_eq!(
            synthetic(&mut reader, MAX_UPLOAD_BYTES as u64 + 1).unwrap_err(),
            Failure::TooLarge
        );
        assert!(reader.requests.is_empty());
    }
    #[test]
    fn wrong_zero_size_cannot_bypass_read_bound_and_one_byte_probe() {
        let mut reader = CountingReader::new(MAX_UPLOAD_BYTES + READ_CHUNK_BYTES);
        assert_eq!(synthetic(&mut reader, 0).unwrap_err(), Failure::TooLarge);
        assert_eq!(reader.total, MAX_UPLOAD_BYTES + 1);
        assert_eq!(
            reader.requests.len(),
            MAX_UPLOAD_BYTES / READ_CHUNK_BYTES + 1
        );
        assert!(
            reader.requests[..reader.requests.len() - 1]
                .iter()
                .all(|size| *size == READ_CHUNK_BYTES)
        );
        assert_eq!(reader.requests.last(), Some(&1));
    }
    #[test]
    fn synthetic_exact_limit_requires_one_byte_eof_probe() {
        let mut reader = CountingReader::new(MAX_UPLOAD_BYTES);
        assert_eq!(synthetic(&mut reader, 1).unwrap().len(), MAX_UPLOAD_BYTES);
        assert_eq!(reader.total, MAX_UPLOAD_BYTES);
        assert_eq!(reader.requests.last(), Some(&1));
    }
    #[test]
    fn shrinking_size_and_short_chunks_use_actual_bytes() {
        let mut reader = CountingReader::new(READ_CHUNK_BYTES + 7);
        assert_eq!(
            synthetic(&mut reader, MAX_UPLOAD_BYTES as u64)
                .unwrap()
                .len(),
            READ_CHUNK_BYTES + 7
        );
        assert_eq!(reader.requests.len(), 3);
        assert!(reader.requests.iter().all(|size| *size <= READ_CHUNK_BYTES));
    }
    #[test]
    fn interrupted_reads_retry_without_losing_bound() {
        let mut reader = CountingReader::new(MAX_UPLOAD_BYTES + 1);
        reader.interruptions = 3;
        assert_eq!(synthetic(&mut reader, 0).unwrap_err(), Failure::TooLarge);
        assert_eq!(reader.total, MAX_UPLOAD_BYTES + 1);
        assert_eq!(
            reader.requests.len(),
            MAX_UPLOAD_BYTES / READ_CHUNK_BYTES + 4
        );
        assert_eq!(reader.requests.last(), Some(&1));
    }
    #[test]
    fn io_error_is_closed_and_stops_reads() {
        let mut reader = CountingReader::new(1);
        reader.fail = true;
        let failure = synthetic(&mut reader, 0).unwrap_err();
        assert_eq!(failure, Failure::Unavailable);
        assert_eq!(reader.requests.len(), 1);
        assert!(!format!("{failure:?} {failure}").contains("PRIVATE_IO_ERROR"));
    }
    #[test]
    fn initial_cancellation_never_reads() {
        let mut reader = CountingReader::new(1);
        assert_eq!(
            read_upload(&mut reader, 0, "leaf".into(), || true).unwrap_err(),
            Failure::Canceled
        );
        assert!(reader.requests.is_empty());
    }
    #[test]
    fn cancellation_between_chunks_prevents_further_reads() {
        let mut reader = CountingReader::new(MAX_UPLOAD_BYTES);
        let checks = Cell::new(0);
        assert_eq!(
            read_upload(&mut reader, 0, "leaf".into(), || {
                checks.set(checks.get() + 1);
                checks.get() >= 3
            })
            .unwrap_err(),
            Failure::Canceled
        );
        assert_eq!(reader.requests.len(), 1);
        assert_eq!(reader.total, READ_CHUNK_BYTES);
    }
    #[test]
    fn cancellation_after_interruption_prevents_retry_read() {
        let mut reader = CountingReader::new(1);
        reader.interruptions = usize::MAX;
        let checks = Cell::new(0);
        assert_eq!(
            read_upload(&mut reader, 0, "leaf".into(), || {
                checks.set(checks.get() + 1);
                checks.get() >= 3
            })
            .unwrap_err(),
            Failure::Canceled
        );
        assert_eq!(reader.requests.len(), 1);
        assert_eq!(reader.total, 0);
    }
    #[test]
    fn final_cancellation_precedes_upload_constructor() {
        let mut reader = CountingReader::new(0);
        let checks = Cell::new(0);
        assert_eq!(
            read_upload(&mut reader, 0, "leaf".into(), || {
                checks.set(checks.get() + 1);
                checks.get() >= 3
            })
            .unwrap_err(),
            Failure::Canceled
        );
        assert_eq!(reader.requests.len(), 1);
    }
    #[test]
    fn interrupted_probe_retries_only_one_byte() {
        let mut reader = CountingReader::new(MAX_UPLOAD_BYTES + 1);
        reader.probe_interruptions = 2;
        assert_eq!(synthetic(&mut reader, 0).unwrap_err(), Failure::TooLarge);
        assert_eq!(reader.total, MAX_UPLOAD_BYTES + 1);
        let chunks = MAX_UPLOAD_BYTES / READ_CHUNK_BYTES;
        assert_eq!(&reader.requests[chunks..], &[1, 1, 1]);
    }
    #[test]
    fn probe_io_error_stops_without_retry_or_constructor() {
        let mut reader = CountingReader::new(MAX_UPLOAD_BYTES);
        reader.probe_fail = true;
        assert_eq!(synthetic(&mut reader, 0).unwrap_err(), Failure::Unavailable);
        assert_eq!(reader.total, MAX_UPLOAD_BYTES);
        assert_eq!(
            reader.requests.len(),
            MAX_UPLOAD_BYTES / READ_CHUNK_BYTES + 1
        );
        assert_eq!(reader.requests.last(), Some(&1));
    }
    #[test]
    fn cancellation_at_limit_prevents_growth_probe() {
        let mut reader = CountingReader::new(MAX_UPLOAD_BYTES + 1);
        let checks = Cell::new(0);
        assert_eq!(
            read_upload(&mut reader, 0, "leaf".into(), || {
                checks.set(checks.get() + 1);
                checks.get() > MAX_UPLOAD_BYTES / READ_CHUNK_BYTES + 1
            })
            .unwrap_err(),
            Failure::Canceled
        );
        assert_eq!(reader.total, MAX_UPLOAD_BYTES);
        assert_eq!(reader.requests.len(), MAX_UPLOAD_BYTES / READ_CHUNK_BYTES);
        assert_eq!(reader.requests.last(), Some(&READ_CHUNK_BYTES));
    }
    #[test]
    fn final_transport_constructor_still_rejects_invalid_leaf() {
        let mut reader = CountingReader::new(0);
        assert_eq!(
            read_upload(&mut reader, 0, "bad\\leaf".into(), || false).unwrap_err(),
            Failure::InvalidSelection
        );
        assert_eq!(reader.requests.len(), 1);
    }
}
