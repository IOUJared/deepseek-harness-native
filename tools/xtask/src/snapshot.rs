//! Source snapshot used to detect a change made while checks were running.
//!
//! The snapshot lists tracked and non-ignored untracked files, hashes each one
//! (a symlink as its target string, matching `os.readlink`), and hashes the sorted
//! map. Bodies never reach evidence: only names and digests.
use crate::json::dump_sorted_object;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Serialize)]
pub struct Snapshot {
    pub sha256: String,
    pub files: BTreeMap<String, String>,
}

/// Hash every tracked or non-ignored untracked entry under `root`.
pub fn source_snapshot(root: &Path) -> io::Result<Snapshot> {
    let listing = Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .current_dir(root)
        .output()?;
    if !listing.status.success() {
        return Err(io::Error::other("git ls-files failed"));
    }
    // Python sorted the raw NUL-separated byte strings and dropped duplicates.
    let mut names: Vec<&[u8]> = listing
        .stdout
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
        .collect();
    names.sort_unstable();
    names.dedup();

    let mut files = BTreeMap::new();
    for name in names {
        let relative = Path::new(OsStr::from_bytes(name));
        let path = root.join(relative);
        let digest = match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let target = fs::read_link(&path)?;
                hash_bytes(target.as_os_str().as_bytes())
            }
            Ok(metadata) if metadata.is_file() => hash_bytes(&fs::read(&path)?),
            // A directory entry listed by git (a submodule), or an entry that
            // vanished between listing and hashing, is recorded, not skipped.
            Ok(_) | Err(_) => "missing".to_string(),
        };
        files.insert(String::from_utf8_lossy(name).into_owned(), digest);
    }
    Ok(Snapshot {
        sha256: hash_bytes(dump_sorted_object(&files).as_bytes()),
        files,
    })
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tempdir::make_temp_dir;

    fn git(directory: &Path, arguments: &[&str]) {
        let status = Command::new("git")
            .args([
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "user.name=Test",
            ])
            .args(arguments)
            .current_dir(directory)
            .status()
            .expect("git runs");
        assert!(status.success(), "git {arguments:?}");
    }

    #[test]
    fn a_snapshot_hashes_tracked_untracked_files_and_a_symlink_target() {
        let base = make_temp_dir(&std::env::temp_dir(), "snapshot-").expect("temp dir");
        git(&base, &["init", "--quiet"]);
        fs::write(base.join("tracked.txt"), "PUBLIC tracked\n").unwrap();
        git(&base, &["add", "tracked.txt"]);
        fs::write(base.join("untracked.txt"), "PUBLIC untracked\n").unwrap();
        fs::write(base.join(".gitignore"), "ignored.txt\n").unwrap();
        fs::write(base.join("ignored.txt"), "PUBLIC ignored\n").unwrap();
        std::os::unix::fs::symlink("tracked.txt", base.join("link")).unwrap();

        let snapshot = source_snapshot(&base).expect("snapshot");
        let names: Vec<&str> = snapshot.files.keys().map(String::as_str).collect();
        assert_eq!(
            names,
            [".gitignore", "link", "tracked.txt", "untracked.txt"]
        );
        assert_eq!(
            snapshot.files["tracked.txt"],
            hash_bytes(b"PUBLIC tracked\n"),
            "a file is hashed by its body"
        );
        assert_eq!(
            snapshot.files["link"],
            hash_bytes(b"tracked.txt"),
            "a symlink is hashed by its target string, not its contents"
        );
        assert_eq!(
            snapshot.sha256,
            hash_bytes(dump_sorted_object(&snapshot.files).as_bytes()),
            "the aggregate digest covers the sorted per-file map"
        );
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn an_empty_repository_snapshots_to_the_empty_object_digest() {
        let base = make_temp_dir(&std::env::temp_dir(), "snapshot-empty-").expect("temp dir");
        git(&base, &["init", "--quiet"]);
        let snapshot = source_snapshot(&base).expect("snapshot");
        assert!(snapshot.files.is_empty());
        assert_eq!(snapshot.sha256, hash_bytes(b"{}"));
        fs::remove_dir_all(base).unwrap();
    }
}
