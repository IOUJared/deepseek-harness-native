//! A private directory with the semantics `tempfile.mkdtemp(mode=0o700)` gave the
//! evidence directories: a fresh name that cannot collide with a concurrent run.
use std::fs::{DirBuilder, File};
use std::io::{self, ErrorKind, Read};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

const ATTEMPTS: usize = 32;

/// Create `parent/<prefix><random>` with mode 0700 and return it.
pub fn make_temp_dir(parent: &Path, prefix: &str) -> io::Result<PathBuf> {
    let mut random = File::open("/dev/urandom")?;
    for _ in 0..ATTEMPTS {
        let mut bytes = [0u8; 6];
        random.read_exact(&mut bytes)?;
        let suffix: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        let candidate = parent.join(format!("{prefix}{suffix}"));
        match DirBuilder::new().mode(0o700).create(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::other(
        "could not create a unique private directory",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn a_created_directory_is_private_and_named_with_its_prefix() {
        let parent = std::env::temp_dir();
        let created = make_temp_dir(&parent, "xtask-test-").expect("directory");
        let name = created.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("xtask-test-"), "{name}");
        assert_eq!(
            created.metadata().unwrap().permissions().mode() & 0o777,
            0o700
        );
        std::fs::remove_dir(&created).unwrap();
    }

    #[test]
    fn two_calls_never_return_the_same_directory() {
        let parent = std::env::temp_dir();
        let first = make_temp_dir(&parent, "xtask-unique-").expect("directory");
        let second = make_temp_dir(&parent, "xtask-unique-").expect("directory");
        assert_ne!(first, second);
        std::fs::remove_dir(&first).unwrap();
        std::fs::remove_dir(&second).unwrap();
    }
}
