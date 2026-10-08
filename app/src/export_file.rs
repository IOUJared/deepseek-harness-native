//! Explicit native export destinations. No overwrite, extraction, or remote filenames.
//! Directory descriptors anchor creation; completed or partial files are never auto-removed.
use dsh_native_transport::export::SessionArchive;
use rustix::fs::{Mode, OFlags, open, openat};
use std::{
    ffi::OsString,
    fs::File,
    io,
    os::unix::fs::PermissionsExt,
    path::{Component, Path, PathBuf},
};

const MAX_PATH_BYTES: usize = 4096;
const MAX_ARCHIVE_BYTES: usize = 32 * 1024 * 1024;

/// Owned operator input, deliberately without Debug or serialization.
pub struct Destination {
    path: PathBuf,
}
/// A directory pinned after admission, not an assertion that its pathname stays unchanged.
pub struct PreparedDestination {
    directory: File,
    filename: OsString,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    InvalidPath,
    NotCreated,
    /// Creation succeeded; a complete or partial private file can remain. Never auto-retry.
    MayRemain,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Saved {
    pub bytes: usize,
}

impl Destination {
    /// Require an explicit absolute new .zip pathname; do not trim, append, or normalize it.
    pub fn parse(input: String) -> Result<Self, Failure> {
        if input.is_empty()
            || input.len() > MAX_PATH_BYTES
            || input.chars().any(char::is_control)
            || !input.starts_with('/')
            || input.ends_with('/')
            || input
                .split('/')
                .skip(1)
                .any(|part| part.is_empty() || part == "." || part == "..")
        {
            return Err(Failure::InvalidPath);
        }
        let path = PathBuf::from(input);
        if !path.is_absolute()
            || path.extension().is_none_or(|extension| extension != "zip")
            || path.file_stem().is_none_or(|stem| stem.is_empty())
            || path
                .components()
                .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
        {
            return Err(Failure::InvalidPath);
        }
        Ok(Self { path })
    }
    /// Open existing directory components without following any symlink.
    /// No directory or destination file is created by preparation.
    pub fn prepare(self) -> Result<PreparedDestination, Failure> {
        let parent = self.path.parent().ok_or(Failure::InvalidPath)?;
        let filename = self
            .path
            .file_name()
            .ok_or(Failure::InvalidPath)?
            .to_owned();
        let flags = OFlags::RDONLY
            | OFlags::DIRECTORY
            | OFlags::NOFOLLOW
            | OFlags::CLOEXEC
            | OFlags::NONBLOCK;
        let mut directory =
            File::from(open("/", flags, Mode::empty()).map_err(|_| Failure::NotCreated)?);
        for component in parent.components() {
            if let Component::Normal(name) = component {
                directory = File::from(
                    openat(&directory, Path::new(name), flags, Mode::empty())
                        .map_err(|_| Failure::NotCreated)?,
                );
            }
        }
        Ok(PreparedDestination {
            directory,
            filename,
        })
    }
}
impl PreparedDestination {
    /// Consume a bounded archive into a new mode-0600 file relative to the pinned parent.
    /// A success requires exact byte accounting and file/directory sync. No rollback is promised.
    pub fn save(self, archive: SessionArchive) -> Result<Saved, Failure> {
        let expected = archive.len();
        if expected == 0 || expected > MAX_ARCHIVE_BYTES {
            return Err(Failure::NotCreated);
        }
        self.save_with(expected, |file| archive.write_to(file))
    }
    fn save_with(
        self,
        expected: usize,
        write: impl FnOnce(&mut File) -> io::Result<usize>,
    ) -> Result<Saved, Failure> {
        let flags = OFlags::WRONLY
            | OFlags::CREATE
            | OFlags::EXCL
            | OFlags::NOFOLLOW
            | OFlags::CLOEXEC
            | OFlags::NONBLOCK;
        let fd = openat(
            &self.directory,
            Path::new(&self.filename),
            flags,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(|_| Failure::NotCreated)?;
        let mut file = File::from(fd);
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|_| Failure::MayRemain)?;
        let written = write(&mut file).map_err(|_| Failure::MayRemain)?;
        if written != expected
            || file.metadata().map_err(|_| Failure::MayRemain)?.len() != expected as u64
        {
            return Err(Failure::MayRemain);
        }
        file.sync_all().map_err(|_| Failure::MayRemain)?;
        self.directory.sync_all().map_err(|_| Failure::MayRemain)?;
        Ok(Saved { bytes: written })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        os::unix::fs::{DirBuilderExt, symlink},
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(1);
    fn owned_directory() -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "dsh-export-file-PUBLIC-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::DirBuilder::new()
            .recursive(false)
            .mode(0o700)
            .create(&directory)
            .unwrap();
        directory
    }
    fn prepared(path: &Path) -> PreparedDestination {
        Destination::parse(path.to_str().unwrap().to_owned())
            .unwrap()
            .prepare()
            .unwrap()
    }
    fn write_public(file: &mut File) -> io::Result<usize> {
        file.write_all(b"PUBLIC")?;
        Ok(6)
    }
    #[test]
    fn explicit_zip_paths_do_not_trim_normalize_or_guess() {
        for input in [
            "",
            "relative.zip",
            "/a/../b.zip",
            "/a/./b.zip",
            "/a//b.zip",
            "/a.zip/",
            "/.zip",
            "/b.ZIP",
            "/a.txt",
            "/b.zip\n",
            "/b\0.zip",
        ] {
            assert!(
                matches!(Destination::parse(input.into()), Err(Failure::InvalidPath)),
                "{input:?}"
            );
        }
        assert!(Destination::parse("/a/Unicode-日本語.zip".into()).is_ok());
        assert!(Destination::parse(format!("/{}.zip", "a".repeat(4096))).is_err());
    }
    #[test]
    fn preparation_creates_nothing_and_symlink_ancestors_refuse() {
        let directory = owned_directory();
        let missing = directory.join("missing").join("file.zip");
        assert!(matches!(
            Destination::parse(missing.to_str().unwrap().into())
                .unwrap()
                .prepare(),
            Err(Failure::NotCreated)
        ));
        assert!(!directory.join("missing").exists());
        let real = directory.join("real");
        std::fs::create_dir(&real).unwrap();
        symlink(&real, directory.join("linked")).unwrap();
        assert!(matches!(
            Destination::parse(directory.join("linked/file.zip").to_str().unwrap().into())
                .unwrap()
                .prepare(),
            Err(Failure::NotCreated)
        ));
        let _prepared = prepared(&real.join("file.zip"));
        assert!(!real.join("file.zip").exists());
    }
    #[test]
    fn creates_private_complete_file_and_never_overwrites() {
        let directory = owned_directory();
        let path = directory.join("file.zip");
        assert_eq!(
            prepared(&path).save_with(6, write_public),
            Ok(Saved { bytes: 6 })
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"PUBLIC");
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            prepared(&path).save_with(6, write_public),
            Err(Failure::NotCreated)
        );
        assert_eq!(std::fs::read(path).unwrap(), b"PUBLIC");
    }
    #[test]
    fn final_symlink_or_directory_is_not_followed_or_written() {
        let directory = owned_directory();
        let existing = directory.join("existing");
        std::fs::write(&existing, b"KEEP").unwrap();
        let destination = directory.join("link.zip");
        symlink(&existing, &destination).unwrap();
        assert_eq!(
            prepared(&destination).save_with(6, write_public),
            Err(Failure::NotCreated)
        );
        assert_eq!(std::fs::read(existing).unwrap(), b"KEEP");
        let folder = directory.join("folder.zip");
        std::fs::create_dir(&folder).unwrap();
        assert_eq!(
            prepared(&folder).save_with(6, write_public),
            Err(Failure::NotCreated)
        );
    }
    #[test]
    fn write_error_or_accounting_mismatch_is_indeterminate_and_retained() {
        let directory = owned_directory();
        let partial = directory.join("partial.zip");
        assert_eq!(
            prepared(&partial).save_with(6, |file| {
                file.write_all(b"P")?;
                Err(io::ErrorKind::Other.into())
            }),
            Err(Failure::MayRemain)
        );
        assert_eq!(std::fs::read(partial).unwrap(), b"P");
        let wrong = directory.join("wrong.zip");
        assert_eq!(
            prepared(&wrong).save_with(7, write_public),
            Err(Failure::MayRemain)
        );
        assert_eq!(std::fs::read(wrong).unwrap(), b"PUBLIC");
    }
    #[test]
    fn late_file_creation_is_refused_before_writer_runs() {
        let directory = owned_directory();
        let path = directory.join("late.zip");
        let destination = prepared(&path);
        std::fs::write(&path, b"KEEP").unwrap();
        assert_eq!(
            destination.save_with(6, |_| panic!("must not write existing file")),
            Err(Failure::NotCreated)
        );
        assert_eq!(std::fs::read(path).unwrap(), b"KEEP");
    }
    #[test]
    fn held_parent_descriptor_survives_path_replacement_without_redirecting() {
        let directory = owned_directory();
        let original = directory.join("original");
        std::fs::create_dir(&original).unwrap();
        let destination = prepared(&original.join("file.zip"));
        let moved = directory.join("moved");
        // Both absolute owned fixture paths are established here, not computed from empty variables.
        let resolved_parent = directory.canonicalize().unwrap();
        assert_eq!(
            original.canonicalize().unwrap(),
            resolved_parent.join("original")
        );
        assert_eq!(
            moved.parent().unwrap().canonicalize().unwrap(),
            resolved_parent
        );
        assert_eq!(moved.file_name().unwrap(), "moved");
        assert!(!moved.exists());
        std::fs::rename(&original, &moved).unwrap();
        std::fs::create_dir(&original).unwrap();
        assert_eq!(
            destination.save_with(6, write_public),
            Ok(Saved { bytes: 6 })
        );
        assert_eq!(std::fs::read(moved.join("file.zip")).unwrap(), b"PUBLIC");
        assert!(!original.join("file.zip").exists());
    }
}
