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
use std::os::unix::fs::OpenOptionsExt;
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

pub(super) static INTERRUPTED: AtomicBool = AtomicBool::new(false);

pub(super) extern "C" fn interrupt(_: libc::c_int) {
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
pub(super) struct Owned {
    pub child: Child,
    pub status: Option<ExitStatus>,
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

    pub(super) fn code(&self) -> i32 {
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

/// The outcome of one guarded owned run: identity-pinned facts and cleanup.
pub(super) struct GuardedRun {
    pub owned: Owned,
    pub facts: Option<compositor::Facts>,
    pub forced: bool,
    pub failure: Option<&'static str>,
    pub observation_failures: u64,
    pub observed: usize,
    pub cleanup: Cleanup,
    pub interrupted: bool,
}

/// Pin, observe and clean up one spawned process tree. The caller installs the
/// signal handlers before spawning and restores them after this returns.
pub(super) fn guard(owned: Owned) -> GuardedRun {
    guard_with(owned, QUALIFICATION_SECONDS)
}

/// The same guard with an explicit budget, for multi-phase qualifiers.
pub(super) fn guard_with(mut owned: Owned, seconds: u64) -> GuardedRun {
    let owner = our_pid();
    let mut pidfds: BTreeMap<Key, i32> = BTreeMap::new();
    let mut observed: BTreeSet<Key> = BTreeSet::new();
    let mut facts: Option<compositor::Facts> = None;
    let mut forced = false;
    let mut failure: Option<&'static str> = None;
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
    if let Some(pinned) = root.as_ref() {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        loop {
            if owned.poll().ok().flatten().is_some() {
                break;
            }
            observation_failures +=
                observe_descendants(&mut owned, pinned, &mut pidfds, &mut observed, owner);
            if facts.is_none()
                && let Some(candidate) = compositor::window_facts(pinned.pid)
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
    GuardedRun {
        owned,
        facts,
        forced,
        failure,
        observation_failures,
        observed: observed.len(),
        cleanup,
        interrupted: INTERRUPTED.load(Ordering::SeqCst),
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
    let run = guard(Owned {
        child,
        status: None,
    });
    // Safe: restore the handlers this run installed.
    unsafe {
        libc::signal(libc::SIGINT, previous_int);
        libc::signal(libc::SIGTERM, previous_term);
    }
    let forced = run.forced || run.cleanup.root_was_running_at_cleanup;
    let app = read_app(&output);
    let exit_code = run.owned.code();
    let binary_unchanged = crate::sha256_file(&binary)
        .map(|hash| hash == before_hash)
        .unwrap_or(false);
    let passed = composed_passed(
        exit_code,
        forced,
        run.failure,
        &run.cleanup,
        run.observation_failures,
        &app,
        run.facts.as_ref(),
        binary_unchanged,
    );
    let report = json!({
        "status": if passed { "passed" } else { "failed" },
        "scope": "Real native UI/worker/Core saving of a fixed PUBLIC dummy in a new owned home only.",
        "binarySha256": before_hash,
        "exitCode": exit_code,
        "forced": forced,
        "interrupted": run.interrupted,
        "failureCode": run.failure,
        "ownedWindow": run.facts,
        "observedOwnedProcessCount": run.observed,
        "ownershipObservationFailures": run.observation_failures,
        "qualificationBudgetSeconds": QUALIFICATION_SECONDS,
        "cleanup": &run.cleanup,
        "remainingBeforeExternalCleanup": &run.cleanup.remaining_before_external_cleanup,
        "remainingAfterExternalCleanup": &run.cleanup.remaining_after_external_cleanup,
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

fn stage_number(stages: &serde_json::Map<String, Value>, stage: &str, field: &str) -> Option<i64> {
    stages.get(stage)?.get(field)?.as_i64()
}

fn non_negative(group: &serde_json::Map<String, Value>) -> bool {
    group
        .values()
        .all(|value| value.as_i64().is_some_and(|number| number >= 0))
}

/// The account observation's own report must describe exactly the expected
/// subscription lifecycle; any missing or extra stage is a failure.
pub fn valid_account(value: &Value) -> bool {
    let (Some(counts), Some(events), Some(stages)) = (
        value["counts"].as_object(),
        value["events"].as_object(),
        value["snapshots"].as_object(),
    ) else {
        return false;
    };
    if !non_negative(counts)
        || !non_negative(events)
        || !stages
            .values()
            .all(|stage| stage.as_object().is_some_and(non_negative))
    {
        return false;
    }
    let ready = stages.get("ready").and_then(Value::as_object);
    let ready_ok = ready.is_some_and(|ready| {
        [
            "watch",
            "state",
            "session",
            "live",
            "maxLive",
            "ended",
            "accountEvents",
            "platformEvents",
        ]
        .iter()
        .all(|field| ready.get(*field).and_then(Value::as_i64) == Some(0))
    });
    let stopped_watch = stage_number(stages, "stopped", "watch");
    let stopped_live = stage_number(stages, "stopped", "live");
    value["status"].as_str() == Some("passed")
        && value["phase"].as_str() == Some("complete")
        && value["fatal"].as_bool() == Some(false)
        && value["processWideNetworkTrace"].as_bool() == Some(false)
        && value["providerHotReplacementQualified"].as_bool() == Some(false)
        && [
            "scriptedSignIn",
            "scriptedModelOrCatalog",
            "scriptedCredentialSave",
        ]
        .iter()
        .all(|name| value[*name].as_bool() == Some(false))
        && ready_ok
        && stage_number(stages, "metadata", "state").is_some_and(|state| state >= 2)
        && stage_number(stages, "metadata", "watch") == Some(0)
        && stage_number(stages, "metadata", "session") == Some(0)
        && stage_number(stages, "metadata", "platformEvents") == Some(0)
        && stage_number(stages, "subscribed", "watch") == Some(1)
        && stage_number(stages, "subscribed", "session") == Some(1)
        && stage_number(stages, "subscribed", "live") == Some(1)
        && stage_number(stages, "subscribed", "accountEvents") == Some(1)
        && stage_number(stages, "unsubscribed", "live") == Some(0)
        && stage_number(stages, "unsubscribed", "ended") == Some(1)
        && counts.get("watch").and_then(Value::as_i64) == Some(2)
        && counts.get("session").and_then(Value::as_i64) == Some(2)
        && counts.get("ended").and_then(Value::as_i64) == Some(2)
        && counts.get("live").and_then(Value::as_i64) == Some(0)
        && counts.get("maxLive").and_then(Value::as_i64) == Some(1)
        && events.get("nonnullPlatform").and_then(Value::as_i64) == Some(0)
        && events.get("observationErrors").and_then(Value::as_i64) == Some(0)
        && stopped_watch == counts.get("watch").and_then(Value::as_i64)
        && stopped_live == Some(0)
}

/// The shared private ownership runner used by the account-style qualifiers.
/// A command built after the private output directory exists.
pub type CommandFactory<'a> = Box<dyn Fn(&Path) -> Vec<String> + 'a>;

pub struct OwnedRun<'a> {
    pub script: &'a Path,
    pub report_name: &'a str,
    pub report_key: &'a str,
    pub evidence_env: &'a str,
    pub scope: Option<&'a str>,
    pub command: CommandFactory<'a>,
    pub capture_stdout: bool,
    pub timeout: Duration,
}

/// Run one public Node observation under the same pin/fence/cleanup rules.
pub fn run_owned(
    root: &Path,
    output: &Path,
    spec: &OwnedRun<'_>,
    validator: impl Fn(&Value) -> bool,
) -> Result<Value, String> {
    let output = prepare(root, output)?;
    let environment_paths = [
        spec.script.to_path_buf(),
        root.join("core/runtime/fork-supervisor.mjs"),
        root.join("core/runtime/supervisor.mjs"),
        root.join("core/runtime/codex.mjs"),
    ];
    let mut digests = BTreeMap::new();
    for path in &environment_paths {
        let relative = path
            .strip_prefix(root)
            .map(|relative| relative.to_string_lossy().into_owned())
            .unwrap_or_else(|_| path.to_string_lossy().into_owned());
        digests.insert(
            relative,
            crate::sha256_file(path).map_err(|error| error.to_string())?,
        );
    }
    let mut env = environment(&output);
    env.insert(
        "DSH_TAURI_RUNTIME".into(),
        root.parent()
            .map(|parent| parent.join("deepseek-harness-linux/apps/cli"))
            .unwrap_or_default()
            .into_os_string(),
    );
    env.insert("DSH_TAURI_EXPECTED_VERSION".into(), "0.2.1-alpha.1".into());
    env.insert(
        "DSH_TAURI_HOME".into(),
        output.join("harness").into_os_string(),
    );
    env.insert(spec.evidence_env.into(), output.clone().into_os_string());
    let report_path = output.join(spec.report_name);
    let stdout = if spec.capture_stdout {
        Stdio::from(
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&report_path)
                .map_err(|error| error.to_string())?,
        )
    } else {
        Stdio::null()
    };
    let argv = (spec.command)(&output);
    let mut command = Command::new(&argv[0]);
    command
        .args(&argv[1..])
        .current_dir(output.join("workspace"))
        .env_clear()
        .envs(&env)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(Stdio::null())
        .process_group(0);
    let child = command.spawn().map_err(|error| error.to_string())?;
    let mut owned = Owned {
        child,
        status: None,
    };
    let owner = our_pid();
    let previous_int =
        unsafe { libc::signal(libc::SIGINT, interrupt as *const () as libc::sighandler_t) };
    let previous_term =
        unsafe { libc::signal(libc::SIGTERM, interrupt as *const () as libc::sighandler_t) };
    INTERRUPTED.store(false, Ordering::SeqCst);
    let mut pidfds: BTreeMap<Key, i32> = BTreeMap::new();
    let mut observed: BTreeSet<Key> = BTreeSet::new();
    let mut pin_failures = 0u64;
    let mut timed_out = false;
    let mut cancelled = false;
    let mut pinned = false;
    let mut root_stat: Option<proc::Stat> = None;
    match pin_root(&mut owned, owner) {
        Ok((pinned_root, fd)) => {
            pidfds.insert(identity(&pinned_root), fd);
            observed.insert(identity(&pinned_root));
            pinned = true;
            root_stat = Some(pinned_root);
        }
        Err(()) => {
            pin_failures += 1;
        }
    }
    if let Some(pinned_root) = root_stat.as_ref() {
        let deadline = Instant::now() + spec.timeout;
        loop {
            if owned.poll().ok().flatten().is_some() {
                break;
            }
            pin_failures +=
                observe_descendants(&mut owned, pinned_root, &mut pidfds, &mut observed, owner);
            if owned.poll().is_err() {
                pin_failures += 1;
                break;
            }
            if INTERRUPTED.load(Ordering::SeqCst) {
                cancelled = true;
                break;
            }
            if Instant::now() >= deadline {
                timed_out = true;
                break;
            }
            std::thread::sleep(POLL);
        }
    }
    let cleanup = cleanup_owned(&mut owned, &pidfds, pinned);
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
    let account = fs::read_to_string(&report_path)
        .ok()
        .filter(|_| {
            fs::symlink_metadata(&report_path)
                .map(|meta| {
                    use std::os::unix::fs::MetadataExt;
                    meta.mode() & 0o7777 == 0o600 && meta.size() <= 65536
                })
                .unwrap_or(false)
        })
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .unwrap_or_else(|| json!({"status": "invalid-or-missing-private-report"}));
    let unchanged = digests.iter().all(|(name, digest)| {
        crate::sha256_file(&root.join(name))
            .map(|current| current == *digest)
            .unwrap_or(false)
    });
    let exit_code = owned.code();
    let valid = exit_code == 0
        && pinned
        && !timed_out
        && !cancelled
        && pin_failures == 0
        && cleanup.remaining_before_external_cleanup.is_empty()
        && !cleanup.root_was_running_at_cleanup
        && cleanup.observed_cleanup_complete
        && unchanged
        && validator(&account);
    let mut result = json!({
        "status": if valid { "passed" } else { "failed" },
        "scope": spec.scope.unwrap_or("owned actual public alpha Loader/native supervisor observation; not Rust worker/GUI or global credential/network trace"),
        "sourceSha256": digests,
        "sourceUnchanged": unchanged,
        "exitCode": exit_code,
        "timedOut": timed_out,
        "cancelled": cancelled,
        "identityPinFailures": pin_failures,
        "observedProcessCount": observed.len(),
        "cleanup": &cleanup,
    });
    result[spec.report_key] = account;
    private_json(&output.join("result.json"), &result).map_err(|error| error.to_string())?;
    println!(
        "{}",
        json!({"status": result["status"], "report": output.join("result.json").to_string_lossy()})
    );
    Ok(result)
}

/// `xtask qualify lazy-account --output NEW_DIR`.
pub fn lazy_account(root: &Path, output: &Path) -> i32 {
    let spec = OwnedRun {
        script: &root.join("scripts/qualify-lazy-account.mjs"),
        report_name: "account.json",
        report_key: "account",
        evidence_env: "DSH_NATIVE_ACCOUNT_EVIDENCE",
        scope: None,
        command: {
            let script = root
                .join("scripts/qualify-lazy-account.mjs")
                .to_string_lossy()
                .into_owned();
            Box::new(move |_| vec!["/usr/bin/node".to_string(), script.clone()])
        },
        capture_stdout: false,
        timeout: Duration::from_secs(35),
    };
    match run_owned(root, output, &spec, valid_account) {
        Ok(result) => {
            if result["status"].as_str() == Some("passed") {
                0
            } else {
                1
            }
        }
        Err(message) => {
            eprintln!("lazy-account qualification failed: {message}");
            1
        }
    }
}

#[cfg(test)]
mod account_tests {
    use super::*;

    fn stage(values: &[(&str, i64)]) -> Value {
        Value::Object(
            values
                .iter()
                .map(|(name, value)| ((*name).to_string(), json!(value)))
                .collect(),
        )
    }

    pub(super) fn account() -> Value {
        json!({
            "status": "passed", "phase": "complete", "fatal": false,
            "processWideNetworkTrace": false, "providerHotReplacementQualified": false,
            "scriptedSignIn": false, "scriptedModelOrCatalog": false, "scriptedCredentialSave": false,
            "counts": {"watch": 2, "session": 2, "ended": 2, "live": 0, "maxLive": 1},
            "events": {"nonnullPlatform": 0, "observationErrors": 0},
            "snapshots": {
                "ready": stage(&[("watch", 0), ("state", 0), ("session", 0), ("live", 0),
                                 ("maxLive", 0), ("ended", 0), ("accountEvents", 0), ("platformEvents", 0)]),
                "metadata": stage(&[("watch", 0), ("state", 2), ("session", 0), ("platformEvents", 0)]),
                "subscribed": stage(&[("watch", 1), ("session", 1), ("live", 1), ("accountEvents", 1)]),
                "unsubscribed": stage(&[("live", 0), ("ended", 1)]),
                "stopped": stage(&[("watch", 2), ("live", 0)]),
            },
            "resource": {"loaded": 1},
        })
    }

    #[test]
    fn a_complete_account_lifecycle_is_valid_and_any_extra_live_subscription_is_not() {
        assert!(valid_account(&account()));
        let mut live = account();
        live["counts"]["live"] = json!(1);
        assert!(!valid_account(&live));
        let mut missing_stage = account();
        missing_stage["snapshots"]
            .as_object_mut()
            .unwrap()
            .remove("stopped");
        assert!(!valid_account(&missing_stage));
        let mut scripted = account();
        scripted["scriptedSignIn"] = json!(true);
        assert!(!valid_account(&scripted));
        let mut negative = account();
        negative["counts"]["watch"] = json!(-1);
        assert!(!valid_account(&negative));
        assert!(
            !valid_account(&json!({"status": "passed"})),
            "a truncated report is not valid"
        );
    }
}

#[cfg(test)]
mod account_guard_tests {
    use super::account_tests::account;
    use super::*;
    use serde_json::json;

    #[test]
    fn eager_reads_unretired_watches_and_overclaims_never_qualify() {
        for (section, key, value) in [
            ("snapshots.ready", "watch", 1),
            ("snapshots.ready", "state", 1),
            ("snapshots.metadata", "session", 1),
            ("events", "nonnullPlatform", 1),
            ("events", "observationErrors", 1),
        ] {
            let mut data = account();
            let mut target = &mut data;
            for part in section.split('.') {
                target = &mut target[part];
            }
            target[key] = json!(value);
            assert!(
                !valid_account(&data),
                "{section}.{key} = {value} was accepted"
            );
        }
        for (key, value) in [
            ("maxLive", 2),
            ("live", 1),
            ("ended", 1),
            ("watch", 3),
            ("session", 3),
        ] {
            let mut data = account();
            data["counts"][key] = json!(value);
            assert!(!valid_account(&data), "counts.{key} = {value} was accepted");
        }
        for key in [
            "processWideNetworkTrace",
            "providerHotReplacementQualified",
            "scriptedCredentialSave",
            "scriptedSignIn",
            "scriptedModelOrCatalog",
            "fatal",
        ] {
            let mut data = account();
            data[key] = json!(true);
            assert!(!valid_account(&data), "{key} true was accepted");
        }
        for value in [json!(true), json!(-1), json!(1.0), json!("1"), json!(null)] {
            let mut data = account();
            data["counts"]["maxLive"] = value.clone();
            assert!(
                !valid_account(&data),
                "counts.maxLive = {value} was accepted"
            );
        }
    }
}
