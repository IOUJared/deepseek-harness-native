use super::*;
use std::{os::unix::process::CommandExt, path::Path, process::Command};

struct Cleanup {
    child: Child,
    root: OwnedProcess,
    owned: HashMap<i32, OwnedProcess>,
    directory: tempfile::TempDir,
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        // The fork receipt is private fixture data. Authorize C only while its proven,
        // pinned parent B remains live; a failed assertion must not abandon fixture C.
        if let Some(pid) = fixture_pid(self.directory.path(), "c") {
            if let Some((child, parent)) = identity_and_parent(pid) {
                if self.owned.get(&parent).is_some_and(|p| {
                    p.alive() && p.identity.start == identity(parent).map_or(0, |i| i.start)
                }) {
                    if let Some(pin) = OwnedProcess::capture(child) {
                        self.owned.insert(pid, pin);
                    }
                }
            }
        }
        for process in self.owned.values() {
            process.signal(libc::SIGKILL);
        }
        self.root.signal(libc::SIGKILL);
        let _ = self.child.wait();
    }
}
fn fixture_pid(directory: &Path, name: &str) -> Option<i32> {
    fs::read_to_string(directory.join(name)).ok()?.parse().ok()
}
fn until(mut test: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !test() {
        assert!(
            Instant::now() < deadline,
            "owned fixture condition timed out"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn orphaned_group_changed_pinned_parent_still_discovers_new_child() {
    let directory = tempfile::tempdir().unwrap();
    let script = r#"
import os,sys,time,pathlib
p=pathlib.Path(sys.argv[1])
a=os.fork()
if a==0:
    original=os.getpid()
    b=os.fork()
    if b==0:
        (p/'b').write_text(str(os.getpid()))
        while not (p/'exit-a').exists(): time.sleep(.005)
        while os.getppid()==original: time.sleep(.005)
        os.setsid()
        c=os.fork()
        if c==0: (p/'c').write_text(str(os.getpid()))
        while True: time.sleep(1)
    else:
        (p/'a').write_text(str(os.getpid()))
        while not (p/'exit-a').exists(): time.sleep(.005)
        os._exit(0)
else:
    while True: time.sleep(1)
"#;
    let child = Command::new("/usr/bin/python3")
        .args(["-c", script])
        .arg(directory.path())
        .process_group(0)
        .spawn()
        .unwrap();
    let root = identity(child.id() as i32).unwrap();
    let mut cleanup = Cleanup {
        child,
        root: OwnedProcess::capture(root).unwrap(),
        owned: HashMap::new(),
        directory,
    };
    until(|| fixture_pid(cleanup.directory.path(), "b").is_some());
    let b = fixture_pid(cleanup.directory.path(), "b").unwrap();
    assert!(snapshot(root, &mut cleanup.owned));
    assert!(cleanup.owned.contains_key(&b));
    let before = cleanup.owned[&b].identity;
    fs::write(cleanup.directory.path().join("exit-a"), "go").unwrap();
    until(|| fixture_pid(cleanup.directory.path(), "c").is_some());
    let c = fixture_pid(cleanup.directory.path(), "c").unwrap();
    let moved = identity(b).unwrap();
    assert_eq!(moved.start, before.start);
    assert_ne!(moved.group, before.group);
    assert!(snapshot(root, &mut cleanup.owned));
    assert!(cleanup.owned.contains_key(&c));
    assert_eq!(cleanup.owned[&b].identity, moved);
}
