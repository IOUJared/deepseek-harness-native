//! Real native UI/worker/Core saving of a fixed PUBLIC dummy in a new owned home.
//!
//! Port of `scripts/qualify-composed-settings.py`. The containment rules are the
//! point: the root is pinned by pidfd before any poll, every captured parent is
//! fenced against PID reuse back to that root, and cleanup signals only pinned
//! identities with bounded TERM/KILL budgets. No numeric descendant, process
//! group, or post-reap PID is ever signalled.
use super::{environment, pidfd_open, prepare, private_json};
use crate::compositor;
use crate::proc;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const QUALIFICATION_SECONDS: u64 = 40;
const TERM_SECONDS: u64 = 2;
const KILL_SECONDS: u64 = 2;
const POLL: Duration = Duration::from_millis(20);
const FIXTURE_CLASS: &str = "ai.deepseek.harness.native.settings-composed-fixture";

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

extern "C" fn interrupt(_: libc::c_int) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}

type Key = (i32, u64);

fn identity(stat: &proc::Stat) -> Key {
    (stat.pid, stat.start_ticks)
}

fn our_pid() -> i32 {
    std::process::id() as i32
}

/// A child whose reaping is owned by this runner alone.
struct Owned {
    child: Child,
    status: Option<ExitStatus>,
}

impl Owned {
    fn poll(&mut self) -> io::Result<Option<ExitStatus>> {
        if self.status.is_none()
            && let Some(status) = self.child.try_wait()?
        {
            self.status = Some(status);
        }
        Ok(self.status)
    }

    fn reaped(&self) -> bool {
        self.status.is_some()
    }

    fn code(&self) -> i32 {
        self.status.map(crate::exit_code).unwrap_or(-1)
    }
}

fn root_matches(owned: &mut Owned, root: &proc::Stat, owner: i32) -> bool {
    if owned.poll().ok().flatten().is_some() {
        return false;
    }
    proc::stat(Path::new("/proc"), root.pid)
        .is_some_and(|current| identity(&current) == identity(root) && current.ppid == owner)
}

fn pin_root(owned: &mut Owned, owner: i32) -> Result<(proc::Stat, i32), ()> {
    let pid = owned.child.id() as i32;
    let root = proc::stat(Path::new("/proc"), pid).ok_or(())?;
    if owned.poll().ok().flatten().is_some() || root.pid != pid || root.ppid != owner {
        return Err(());
    }
    let fd = pidfd_open(pid).ok_or(())?;
    if !root_matches(owned, &root, owner) {
        // Safe: a descriptor opened for this root and not yet shared.
        unsafe {
            libc::close(fd);
        }
        return Err(());
    }
    Ok((root, fd))
}

/// Fence every captured parent against PID reuse, then recheck the pinned root.
fn parent_chain_matches(
    stat: &proc::Stat,
    snapshot: &BTreeMap<i32, proc::Stat>,
    root: &proc::Stat,
    owner: i32,
) -> bool {
    let mut stat = stat;
    let mut visited = BTreeSet::new();
    while stat.pid != root.pid {
        if !visited.insert(stat.pid) {
            return false;
        }
        let Some(current) = proc::stat(Path::new("/proc"), stat.pid) else {
            return false;
        };
        if identity(&current) != identity(stat) || current.ppid != stat.ppid {
            return false;
        }
        let Some(parent) = snapshot.get(&stat.ppid) else {
            return false;
        };
        stat = parent;
    }
    let current = proc::stat(Path::new("/proc"), root.pid);
    identity(stat) == identity(root)
        && current
            .is_some_and(|current| identity(&current) == identity(root) && current.ppid == owner)
}

fn observe_descendants(
    owned: &mut Owned,
    root: &proc::Stat,
    pidfds: &mut BTreeMap<Key, i32>,
    observed: &mut BTreeSet<Key>,
    owner: i32,
) -> u64 {
    let snapshot: BTreeMap<i32, proc::Stat> =
        proc::process_tree(Path::new("/proc"), root.pid, root.start_ticks)
            .into_iter()
            .map(|stat| (stat.pid, stat))
            .collect();
    let mut failures = 0;
    for stat in snapshot.values() {
        let key = identity(stat);
        if pidfds.contains_key(&key) {
            continue;
        }
        if !root_matches(owned, root, owner) || !parent_chain_matches(stat, &snapshot, root, owner)
        {
            failures += 1;
            continue;
        }
        let Some(fd) = pidfd_open(stat.pid) else {
            failures += 1;
            continue;
        };
        if !root_matches(owned, root, owner) || !parent_chain_matches(stat, &snapshot, root, owner)
        {
            // Safe: this descriptor was opened for this identity and is not shared.
            unsafe {
                libc::close(fd);
            }
            failures += 1;
            continue;
        }
        pidfds.insert(key, fd);
        observed.insert(key);
    }
    failures
}

/// A pidfd becomes readable when its process exits; an unreadable probe is
/// "unknown", which is treated as still-live and reported as a failure.
fn live_pidfds(pidfds: &BTreeMap<Key, i32>) -> (Vec<Key>, bool) {
    let mut live = Vec::new();
    let mut failed = false;
    for (key, fd) in pidfds {
        let mut probe = libc::pollfd {
            fd: *fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // Safe: one pollfd for zero milliseconds.
        let result = unsafe { libc::poll(&mut probe, 1, 0) };
        if result < 0 {
            // Unknown is not exited; still report the probe failure.
            live.push(*key);
            failed = true;
        } else if result == 0 {
            live.push(*key);
        }
    }
    (live, failed)
}

fn signal_pinned(pidfds: &BTreeMap<Key, i32>, keys: &[Key], signal: i32) -> bool {
    let mut failed = false;
    for key in keys {
        if let Some(fd) = pidfds.get(key) {
            // Safe: sending to a descriptor this run opened for that identity.
            let result = unsafe {
                libc::syscall(
                    libc::SYS_pidfd_send_signal,
                    *fd,
                    signal,
                    std::ptr::null::<libc::siginfo_t>(),
                    0,
                )
            };
            if result != 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    failed = true;
                }
            }
        }
    }
    failed
}

fn wait_quiescent(
    owned: &mut Owned,
    pidfds: &BTreeMap<Key, i32>,
    seconds: u64,
) -> (Vec<Key>, bool) {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut failed = false;
    loop {
        failed |= owned.poll().is_err();
        let (live, probe_failed) = live_pidfds(pidfds);
        failed |= probe_failed;
        if owned.reaped() && live.is_empty() {
            return (live, failed);
        }
        if Instant::now() >= deadline {
            return (live, failed);
        }
        std::thread::sleep(POLL);
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cleanup {
    pub root_pidfd_pinned: bool,
    pub root_was_running_at_cleanup: bool,
    pub owned_child_fallback_used: bool,
    pub remaining_before_external_cleanup: Vec<Key>,
    pub remaining_after_external_cleanup: Vec<Key>,
    pub kill_escalated: bool,
    pub root_reaped: bool,
    pub cleanup_timed_out: bool,
    pub cleanup_probe_or_signal_failed: bool,
    pub observed_cleanup_complete: bool,
    pub term_budget_seconds: u64,
    pub kill_budget_seconds: u64,
}

/// Bounded TERM/KILL and reap for every pinned identity, even after the GUI root exits.
fn cleanup_owned(owned: &mut Owned, pidfds: &BTreeMap<Key, i32>, root_pinned: bool) -> Cleanup {
    let mut failed = owned.poll().is_err();
    let (before, probe_failed) = live_pidfds(pidfds);
    failed |= probe_failed;
    let root_was_running = !owned.reaped();
    let fallback = !root_pinned && root_was_running;
    failed |= signal_pinned(pidfds, &before, libc::SIGTERM);
    if fallback {
        // The directly owned, unreaped child is the only fallback authority.
        let _ = owned.child.kill();
    }
    let (after_term, wait_failed) = wait_quiescent(owned, pidfds, TERM_SECONDS);
    failed |= wait_failed;
    let escalated = !after_term.is_empty() || !owned.reaped();
    failed |= signal_pinned(pidfds, &after_term, libc::SIGKILL);
    if fallback && !owned.reaped() {
        let _ = owned.child.kill();
    }
    let (after, wait_failed) = wait_quiescent(owned, pidfds, KILL_SECONDS);
    failed |= wait_failed;
    Cleanup {
        root_pidfd_pinned: root_pinned,
        root_was_running_at_cleanup: root_was_running,
        owned_child_fallback_used: fallback,
        remaining_before_external_cleanup: before,
        remaining_after_external_cleanup: after.clone(),
        kill_escalated: escalated,
        root_reaped: owned.reaped(),
        cleanup_timed_out: !owned.reaped() || !after.is_empty(),
        cleanup_probe_or_signal_failed: failed,
        observed_cleanup_complete: root_pinned && owned.reaped() && after.is_empty() && !failed,
        term_budget_seconds: TERM_SECONDS,
        kill_budget_seconds: KILL_SECONDS,
    }
}

/// The verdict, separated so it can be reasoned about without a run.
#[allow(clippy::too_many_arguments)]
pub fn composed_passed(
    exit_code: i32,
    forced: bool,
    failure: Option<&str>,
    cleanup: &Cleanup,
    observation_failures: u64,
    app: &Value,
    facts: Option<&compositor::Facts>,
    binary_unchanged: bool,
) -> bool {
    exit_code == 0
        && !forced
        && failure.is_none()
        && cleanup.observed_cleanup_complete
        && cleanup.remaining_before_external_cleanup.is_empty()
        && observation_failures == 0
        && app.get("status").and_then(Value::as_str) == Some("passed")
        && binary_unchanged
        && facts.is_some_and(|facts| {
            facts["class"].as_str() == Some(FIXTURE_CLASS)
                && facts["xwayland"].as_bool() == Some(false)
                && facts["mapped"].as_bool() == Some(true)
        })
}

fn read_app(output: &Path) -> Value {
    match fs::read_to_string(output.join("app.json")) {
        Ok(text) => {
            serde_json::from_str(&text).unwrap_or_else(|_| json!({"status": "invalid-app-report"}))
        }
        Err(_) => json!({"status": "missing-app-report"}),
    }
}

/// Run the composed settings fixture and return its report.
pub fn measure(script_root: &Path, output: &Path, scale: f64) -> Result<Value, String> {
    let output = prepare(script_root, output)?;
    let binary = script_root
        .join("app/target/release/examples/settings_composed_smoke")
        .canonicalize()
        .map_err(|_| "release composed fixture not ready".to_string())?;
    if !binary.is_file() {
        return Err("release composed fixture not ready".into());
    }
    let before_hash = crate::sha256_file(&binary).map_err(|error| error.to_string())?;
    // Install before spawning so an interrupt during acquisition also cleans up.
    // Safe: constant signals and a handler that only stores an atomic flag.
    let previous_int =
        unsafe { libc::signal(libc::SIGINT, interrupt as *const () as libc::sighandler_t) };
    let previous_term =
        unsafe { libc::signal(libc::SIGTERM, interrupt as *const () as libc::sighandler_t) };
    INTERRUPTED.store(false, Ordering::SeqCst);
    let child = Command::new(&binary)
        .arg("--output")
        .arg(&output)
        .arg("--scale")
        .arg(format!("{scale}"))
        .env_clear()
        .envs(environment(&output))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|error| error.to_string())?;
    let mut owned = Owned {
        child,
        status: None,
    };
    let owner = our_pid();
    let mut pidfds: BTreeMap<Key, i32> = BTreeMap::new();
    let mut observed: BTreeSet<Key> = BTreeSet::new();
    let mut facts: Option<compositor::Facts> = None;
    let mut forced = false;
    let mut failure: Option<&str> = None;
    let mut observation_failures = 0u64;
    let mut root_pinned = false;
    let mut root: Option<proc::Stat> = None;

    match pin_root(&mut owned, owner) {
        Err(()) => {
            failure = Some("root-pidfd-acquisition-failed");
            forced = true;
        }
        Ok((pinned, fd)) => {
            root_pinned = true;
            pidfds.insert(identity(&pinned), fd);
            observed.insert(identity(&pinned));
            root = Some(pinned);
        }
    }
    if let Some(root) = root.as_ref() {
        let deadline = Instant::now() + Duration::from_secs(QUALIFICATION_SECONDS);
        loop {
            if owned.poll().ok().flatten().is_some() {
                break;
            }
            observation_failures +=
                observe_descendants(&mut owned, root, &mut pidfds, &mut observed, owner);
            if facts.is_none()
                && let Some(candidate) = compositor::window_facts(root.pid)
                && candidate["mapped"].as_bool() == Some(true)
            {
                facts = Some(candidate);
            }
            if INTERRUPTED.load(Ordering::SeqCst) || Instant::now() >= deadline {
                forced = true;
                break;
            }
            std::thread::sleep(POLL * 5);
        }
    }
    let cleanup = cleanup_owned(&mut owned, &pidfds, root_pinned);
    for fd in pidfds.values() {
        // Safe: each descriptor was opened for one identity this run and closed once.
        unsafe {
            libc::close(*fd);
        }
    }
    // Safe: restore the handlers this run installed.
    unsafe {
        libc::signal(libc::SIGINT, previous_int);
        libc::signal(libc::SIGTERM, previous_term);
    }
    if cleanup.root_was_running_at_cleanup {
        forced = true;
    }
    let app = read_app(&output);
    let exit_code = owned.code();
    let binary_unchanged = crate::sha256_file(&binary)
        .map(|hash| hash == before_hash)
        .unwrap_or(false);
    let passed = composed_passed(
        exit_code,
        forced,
        failure,
        &cleanup,
        observation_failures,
        &app,
        facts.as_ref(),
        binary_unchanged,
    );
    let report = json!({
        "status": if passed { "passed" } else { "failed" },
        "scope": "Real native UI/worker/Core saving of a fixed PUBLIC dummy in a new owned home only.",
        "binarySha256": before_hash,
        "exitCode": exit_code,
        "forced": forced,
        "interrupted": INTERRUPTED.load(Ordering::SeqCst),
        "failureCode": failure,
        "ownedWindow": facts,
        "observedOwnedProcessCount": observed.len(),
        "ownershipObservationFailures": observation_failures,
        "qualificationBudgetSeconds": QUALIFICATION_SECONDS,
        "cleanup": &cleanup,
        "remainingBeforeExternalCleanup": &cleanup.remaining_before_external_cleanup,
        "remainingAfterExternalCleanup": &cleanup.remaining_after_external_cleanup,
        "app": app,
        "paintInspected": false,
        "nativeKeyboardPointerInputQualified": false,
        "liveHostDecisionQualified": false,
        "desktopSettingsChanged": false,
        "installedRuntimeChanged": false,
        "realCredentialsUsed": false,
    });
    private_json(&output.join("result.json"), &report).map_err(|error| error.to_string())?;
    println!(
        "{}",
        json!({"status": report["status"], "report": output.join("result.json").to_string_lossy()})
    );
    Ok(report)
}

/// `xtask qualify composed-settings --output NEW_DIR [--scale 1.0|1.25]`.
pub fn run(script_root: &Path, output: &Path, scale: f64) -> i32 {
    match measure(script_root, output, scale) {
        Ok(report) => {
            if report["status"].as_str() == Some("passed") {
                0
            } else {
                1
            }
        }
        Err(message) => {
            eprintln!("composed settings qualification failed: {message}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cleanup() -> Cleanup {
        Cleanup {
            root_pidfd_pinned: true,
            root_was_running_at_cleanup: false,
            owned_child_fallback_used: false,
            remaining_before_external_cleanup: Vec::new(),
            remaining_after_external_cleanup: Vec::new(),
            kill_escalated: false,
            root_reaped: true,
            cleanup_timed_out: false,
            cleanup_probe_or_signal_failed: false,
            observed_cleanup_complete: true,
            term_budget_seconds: TERM_SECONDS,
            kill_budget_seconds: KILL_SECONDS,
        }
    }

    fn app() -> Value {
        json!({"status": "passed"})
    }

    fn facts() -> compositor::Facts {
        let mut facts = compositor::Facts::new();
        facts.insert("class".into(), json!(FIXTURE_CLASS));
        facts.insert("xwayland".into(), json!(false));
        facts.insert("mapped".into(), json!(true));
        facts
    }

    #[test]
    fn a_composed_qualification_requires_complete_containment_and_a_native_window() {
        assert!(composed_passed(
            0,
            false,
            None,
            &cleanup(),
            0,
            &app(),
            Some(&facts()),
            true
        ));
        // Unknown cleanup is not containment.
        let mut unknown = cleanup();
        unknown.observed_cleanup_complete = false;
        assert!(!composed_passed(
            0,
            false,
            None,
            &unknown,
            0,
            &app(),
            Some(&facts()),
            true
        ));
        // A retained process before external cleanup disqualifies the run.
        let mut retained = cleanup();
        retained.remaining_before_external_cleanup = vec![(7, 9)];
        assert!(!composed_passed(
            0,
            false,
            None,
            &retained,
            0,
            &app(),
            Some(&facts()),
            true
        ));
        // A changed binary invalidates the pinned observation.
        assert!(!composed_passed(
            0,
            false,
            None,
            &cleanup(),
            0,
            &app(),
            Some(&facts()),
            false
        ));
        // An observation failure, a forced run or a missing proof all fail.
        assert!(!composed_passed(
            0,
            false,
            None,
            &cleanup(),
            1,
            &app(),
            Some(&facts()),
            true
        ));
        assert!(!composed_passed(
            0,
            true,
            None,
            &cleanup(),
            0,
            &app(),
            Some(&facts()),
            true
        ));
        assert!(!composed_passed(
            0,
            false,
            None,
            &cleanup(),
            0,
            &app(),
            None,
            true
        ));
        assert!(!composed_passed(
            1,
            false,
            None,
            &cleanup(),
            0,
            &app(),
            Some(&facts()),
            true
        ));
    }

    #[test]
    fn a_foreign_window_class_is_not_the_settings_fixture() {
        let mut foreign = facts();
        foreign.insert("class".into(), json!("another-class"));
        assert!(!composed_passed(
            0,
            false,
            None,
            &cleanup(),
            0,
            &app(),
            Some(&foreign),
            true
        ));
    }

    #[test]
    fn cleanup_pins_and_reaps_a_live_child() {
        let mut owned = Owned {
            child: Command::new("/bin/sleep").arg("30").spawn().unwrap(),
            status: None,
        };
        let owner = our_pid();
        let (root, fd) = pin_root(&mut owned, owner).expect("the direct child pins");
        let mut pidfds = BTreeMap::new();
        pidfds.insert(identity(&root), fd);
        let cleanup = cleanup_owned(&mut owned, &pidfds, true);
        assert!(
            cleanup.root_was_running_at_cleanup,
            "the sleep was live before cleanup"
        );
        assert!(cleanup.observed_cleanup_complete, "{cleanup:?}");
        assert!(cleanup.root_reaped);
        assert!(cleanup.remaining_after_external_cleanup.is_empty());
        unsafe {
            libc::close(fd);
        }
    }
}
