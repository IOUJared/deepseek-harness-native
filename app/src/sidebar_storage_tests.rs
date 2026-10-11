//! Tests of the shared production persistence primitive, not sidebar Entry validation/UI.
use super::*;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, symlink};
use std::sync::{Arc, atomic::AtomicBool};

static NEXT: AtomicU64 = AtomicU64::new(1);
struct Fixture {
    path: PathBuf,
    dev: u64,
    ino: u64,
}
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "dsh-sidebar-PUBLIC-{}-{}",
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
        Store::sidebar(self.path.clone()).unwrap()
    }
    fn document(&self) -> PathBuf {
        self.path.join(Namespace::Sidebar.document())
    }
    fn put(&self, bytes: &[u8]) {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(self.document())
            .unwrap();
        file.write_all(bytes).unwrap();
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Only this EXCL-created absolute synthetic root with unchanged identity is removed.
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
fn entries(label: &str) -> Drafts {
    BTreeMap::from([(
        SessionId::new("PUBLIC-session").unwrap(),
        serde_json::json!({ "label": label, "pinned": true }).to_string(),
    )])
}

#[test]
fn sidebar_restart_and_draft_namespace_isolation() {
    let fixture = Fixture::new();
    let mut drafts = Store::new(fixture.path.clone()).unwrap();
    let transcript = BTreeMap::from([(
        SessionId::new("PUBLIC-session").unwrap(),
        "PUBLIC unsent transcript".into(),
    )]);
    drafts.save(&transcript).unwrap();
    let draft_bytes = std::fs::read(fixture.path.join(DOCUMENT)).unwrap();
    assert!(fixture.store().load().unwrap().is_empty());
    assert!(!fixture.path.join(Namespace::Sidebar.lock()).exists());
    let custom = entries("PUBLIC 日本語");
    fixture
        .store()
        .save_checked(&custom, &Drafts::new())
        .unwrap();
    assert!(fixture.store().load().unwrap() == custom);
    assert!(drafts.load().unwrap() == transcript);
    assert_eq!(
        std::fs::read(fixture.path.join(DOCUMENT)).unwrap(),
        draft_bytes
    );
    let sidebar_bytes = std::fs::read(fixture.document()).unwrap();
    assert!(
        !String::from_utf8(sidebar_bytes.clone())
            .unwrap()
            .contains("transcript")
    );
    drafts.save(&Drafts::new()).unwrap();
    assert_eq!(std::fs::read(fixture.document()).unwrap(), sidebar_bytes);
    for path in [
        fixture.document(),
        fixture.path.join(Namespace::Sidebar.lock()),
    ] {
        assert_eq!(std::fs::metadata(path).unwrap().mode() & 0o7777, 0o600);
    }
}

#[test]
fn two_writers_stale_expected_conflicts_before_temp_and_unchecked_save_still_replaces() {
    let fixture = Fixture::new();
    let mut first = fixture.store();
    let mut second = fixture.store();
    first
        .save_checked(&entries("PUBLIC initial"), &Drafts::new())
        .unwrap();
    let expected = second.load().unwrap();
    first
        .save_checked(&entries("PUBLIC first"), &expected)
        .unwrap();
    let committed = std::fs::read(fixture.document()).unwrap();
    let observed = Arc::new(AtomicBool::new(false));
    let record = observed.clone();
    second.set_save_observer(Some(Arc::new(move |_| {
        record.store(true, Ordering::Relaxed);
        Ok(())
    })));
    assert_eq!(
        second.save_checked(&entries("PUBLIC stale"), &expected),
        Err(Failure::Conflict)
    );
    assert!(!observed.load(Ordering::Relaxed));
    assert_eq!(std::fs::read(fixture.document()).unwrap(), committed);
    assert_eq!(std::fs::read_dir(&fixture.path).unwrap().count(), 2);
    second.save(&entries("PUBLIC unchecked")).unwrap();
    assert!(first.load().unwrap() == entries("PUBLIC unchecked"));
}

#[test]
fn absent_and_present_empty_are_semantically_equal_and_nonempty_absent_conflicts() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    assert_eq!(
        store.save_checked(&entries("PUBLIC new"), &entries("PUBLIC old")),
        Err(Failure::Conflict)
    );
    assert!(!fixture.document().exists());
    store.save_checked(&Drafts::new(), &Drafts::new()).unwrap();
    store
        .save_checked(&entries("PUBLIC first"), &Drafts::new())
        .unwrap();
    assert!(store.load().unwrap() == entries("PUBLIC first"));
}

#[test]
fn semantic_cas_accepts_noncanonical_outer_json() {
    let fixture = Fixture::new();
    let expected = entries("PUBLIC same");
    fixture.put(
        &serde_json::to_vec_pretty(&serde_json::json!({
            "drafts": expected, "version": 1,
        }))
        .unwrap(),
    );
    fixture
        .store()
        .save_checked(&entries("PUBLIC next"), &expected)
        .unwrap();
    assert!(fixture.store().load().unwrap() == entries("PUBLIC next"));
}

#[test]
fn sidebar_lock_is_busy_but_draft_lock_is_independent() {
    let fixture = Fixture::new();
    let directory = Directory::open(&fixture.path, Namespace::Sidebar)
        .unwrap()
        .unwrap();
    let held = directory.lock(true).unwrap().unwrap();
    assert_eq!(
        fixture
            .store()
            .save_checked(&entries("PUBLIC"), &Drafts::new()),
        Err(Failure::Busy)
    );
    assert!(matches!(fixture.store().load(), Err(Failure::Busy)));
    Store::new(fixture.path.clone())
        .unwrap()
        .save(&Drafts::new())
        .unwrap();
    drop(held);
    fixture
        .store()
        .save_checked(&entries("PUBLIC"), &Drafts::new())
        .unwrap();
}

#[test]
fn sidebar_malformed_unknown_duplicate_or_newer_documents_are_not_repaired() {
    for bytes in [
        b"{".as_slice(),
        b"null",
        b"[1,{}]",
        b"{\"version\":2,\"drafts\":{}}",
        b"{\"version\":1,\"drafts\":{},\"unknown\":true}",
        b"{\"version\":1,\"drafts\":{\"PUBLIC\":\"a\",\"PUBLIC\":\"b\"}}",
    ] {
        let fixture = Fixture::new();
        fixture.put(bytes);
        assert!(matches!(fixture.store().load(), Err(Failure::InvalidData)));
        assert_eq!(
            fixture
                .store()
                .save_checked(&entries("PUBLIC"), &Drafts::new()),
            Err(Failure::InvalidData)
        );
        assert_eq!(std::fs::read(fixture.document()).unwrap(), bytes);
    }
}

#[test]
fn sidebar_symlink_and_wrong_mode_document_or_lock_refuse_without_repair() {
    for lock in [false, true] {
        let fixture = Fixture::new();
        let keep = fixture.path.join("PUBLIC-keep");
        std::fs::write(&keep, b"PUBLIC keep").unwrap();
        let target = if lock {
            fixture.path.join(Namespace::Sidebar.lock())
        } else {
            fixture.document()
        };
        symlink(&keep, &target).unwrap();
        assert!(matches!(fixture.store().load(), Err(Failure::Unavailable)));
        assert_eq!(
            fixture
                .store()
                .save_checked(&entries("PUBLIC"), &Drafts::new()),
            Err(Failure::Unavailable)
        );
        assert_eq!(std::fs::read(&keep).unwrap(), b"PUBLIC keep");
        assert!(
            std::fs::symlink_metadata(target)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
    for lock in [false, true] {
        let fixture = Fixture::new();
        let target = if lock {
            let target = fixture.path.join(Namespace::Sidebar.lock());
            std::fs::write(&target, b"").unwrap();
            target
        } else {
            fixture.put(&encode(&Drafts::new()).unwrap());
            fixture.document()
        };
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(fixture.store().load(), Err(Failure::Unavailable)));
        assert_eq!(
            fixture
                .store()
                .save_checked(&entries("PUBLIC"), &Drafts::new()),
            Err(Failure::Unavailable)
        );
        assert_eq!(std::fs::metadata(target).unwrap().mode() & 0o7777, 0o644);
    }
}

#[test]
fn checked_save_retains_intra_operation_bytes_guard() {
    let fixture = Fixture::new();
    let expected = entries("PUBLIC old");
    let mut store = fixture.store();
    store.save(&expected).unwrap();
    let changed = encode(&entries("PUBLIC concurrent")).unwrap();
    let replacement = changed.clone();
    let path = fixture.document();
    store.set_save_observer(Some(Arc::new(move |step| {
        if step == SaveCheckpoint::AfterTempWrite {
            std::fs::write(&path, &replacement)?;
        }
        Ok(())
    })));
    assert_eq!(
        store.save_checked(&entries("PUBLIC next"), &expected),
        Err(Failure::Conflict)
    );
    assert_eq!(std::fs::read(fixture.document()).unwrap(), changed);
}

// Regression for the intermittent Failure::Busy in this module. An flock lock belongs to the
// open file description, so it survives closing the descriptor that took it while any
// duplicate remains. This process forks this same test binary in
// `cross_process_sidebar_writer_invalidates_loaded_expected_snapshot`; the child inherits
// every open lock descriptor until O_CLOEXEC closes the copies at execve, which is exactly
// what dup(2) reproduces here without racing a real fork (std's fork path returns from
// `Command::spawn` only once the child has exec'd, so no deterministic pre-exec window is
// observable from the parent). The duplicate below must not extend the lock past the
// operation that took it.
#[test]
fn completed_operation_releases_exclusive_lock_while_a_duplicate_descriptor_remains() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    store.save(&entries("PUBLIC seeded")).unwrap();
    let directory = Directory::open(&fixture.path, Namespace::Sidebar)
        .unwrap()
        .unwrap();
    let held = directory.lock(true).unwrap().unwrap();
    // Same open file description as `held`, hence the same exclusive lock.
    let inherited = held.file.try_clone().unwrap();
    drop(held);
    let observed = store.load();
    drop(inherited);
    assert!(
        observed.is_ok(),
        "a completed operation must release its exclusive lock: {observed:?}"
    );
    assert!(store.load().is_ok());
}

#[test]
fn sidebar_after_rename_fault_is_indeterminate_and_restart_reconciles() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    store.set_save_observer(Some(Arc::new(|step| {
        if step == SaveCheckpoint::AfterRename {
            Err(io::Error::other("PUBLIC injected after-rename fault"))
        } else {
            Ok(())
        }
    })));
    let new = entries("PUBLIC committed");
    assert_eq!(
        store.save_checked(&new, &Drafts::new()),
        Err(Failure::Indeterminate)
    );
    assert!(fixture.store().load().unwrap() == new);
    assert_eq!(std::fs::read_dir(&fixture.path).unwrap().count(), 2);
}

#[test]
fn sidebar_capacity_uses_shared_limits() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let maximum: Drafts = (0..MAX_DRAFTS)
        .map(|i| (SessionId::new(format!("PUBLIC-{i}")).unwrap(), "{}".into()))
        .collect();
    store.save_checked(&maximum, &Drafts::new()).unwrap();
    let mut excessive = maximum.clone();
    excessive.insert(SessionId::new("PUBLIC-extra").unwrap(), "{}".into());
    assert_eq!(
        store.save_checked(&excessive, &maximum),
        Err(Failure::Limit)
    );
    assert_eq!(
        store.save_checked(
            &BTreeMap::from([(
                SessionId::new("PUBLIC-big").unwrap(),
                "x".repeat(MAX_TEXT_BYTES + 1),
            )]),
            &maximum
        ),
        Err(Failure::Limit)
    );
    assert!(store.load().unwrap() == maximum);
}

// A separate process writes after this process loaded its whole-snapshot expectation.
#[test]
fn cross_process_sidebar_writer_invalidates_loaded_expected_snapshot() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let expected = store.load().unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "ui::draft_store::sidebar_storage_tests::subprocess_sidebar_writer",
            "--nocapture",
        ])
        .env("DSH_PUBLIC_SIDEBAR_STORAGE_TEST_HOME", &fixture.path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "PUBLIC child writer failed: {output:?}"
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("running 1 test"),
        "child must execute the exact storage writer"
    );
    assert!(store.load().unwrap() == entries("PUBLIC child"));
    assert_eq!(
        store.save_checked(&entries("PUBLIC stale parent"), &expected),
        Err(Failure::Conflict)
    );
    assert!(fixture.store().load().unwrap() == entries("PUBLIC child"));
}

#[test]
fn subprocess_sidebar_writer() {
    let Some(home) = std::env::var_os("DSH_PUBLIC_SIDEBAR_STORAGE_TEST_HOME") else {
        return;
    };
    Store::sidebar(home.into())
        .unwrap()
        .save_checked(&entries("PUBLIC child"), &Drafts::new())
        .unwrap();
}
