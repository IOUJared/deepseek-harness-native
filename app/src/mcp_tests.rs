//! Unit tests for the native MCP panel: validation, row rendering, the strict
//! parser, the atomic write with backup, and the consent/fencing state machine.
use super::*;
use std::{
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);

/// A private temporary home removed when the test ends.
struct TempHome {
    path: PathBuf,
}

impl TempHome {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "dsh-native-mcp-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::DirBuilder::new()
            .recursive(false)
            .mode(0o700)
            .create(&path)
            .expect("private temporary home");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
            .expect("private mode");
        Self { path }
    }
    fn file(&self) -> PathBuf {
        self.path.join(FILE_NAME)
    }
    fn read(&self) -> Option<String> {
        std::fs::read_to_string(self.file()).ok()
    }
    /// Plant an existing patch file with the private mode the app requires.
    fn plant(&self, text: &str) {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(self.file())
            .expect("plant patch file");
        file.write_all(text.as_bytes()).expect("plant bytes");
        file.sync_all().expect("plant sync");
    }
    fn backups(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.path)
            .expect("read home")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(&format!("{FILE_NAME}.backup-")))
            .collect();
        names.sort();
        names
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn stdio(name: &str, command: &str) -> ServerEntry {
    ServerEntry {
        server_name: name.to_owned(),
        transport: Transport::Stdio,
        enabled: true,
        command: command.to_owned(),
        args: vec!["--serve".to_owned()],
        env: vec![(
            "API_TOKEN".to_owned(),
            Secret::new("PUBLIC-token".to_owned()),
        )],
        cwd: String::new(),
        url: String::new(),
        headers: Vec::new(),
        tool_call_timeout_ms: DEFAULT_TIMEOUT_MS,
        fail_on_startup_error: true,
    }
}

fn http(name: &str, url: &str) -> ServerEntry {
    ServerEntry {
        server_name: name.to_owned(),
        transport: Transport::StreamableHttp,
        enabled: true,
        command: String::new(),
        args: Vec::new(),
        env: Vec::new(),
        cwd: String::new(),
        url: url.to_owned(),
        headers: vec![(
            "Authorization".to_owned(),
            Secret::new("Bearer PUBLIC".to_owned()),
        )],
        tool_call_timeout_ms: 2500,
        fail_on_startup_error: false,
    }
}

// ---- validation per transport ----

#[test]
fn secret_wipes_its_full_allocation_and_never_prints_its_value() {
    let mut raw = String::with_capacity(64);
    raw.push_str("PUBLIC_FAKE_MCP_TOKEN");
    let mut secret = secret(raw);
    assert_eq!(format!("{secret:?}"), "NativeMcpSecret([REDACTED])");
    secret.wipe();
    // wipe zeroed every allocated byte, so observing the spare capacity is safe now.
    let bytes = unsafe { std::slice::from_raw_parts(secret.0.as_ptr(), secret.0.capacity()) };
    assert!(bytes.iter().all(|byte| *byte == 0));
}

#[test]
fn a_reviewed_set_that_cannot_fit_the_patch_file_is_refused_before_the_write() {
    // Every field is inside its own bound; together they cannot fit the file this panel writes.
    let mut entries = Vec::new();
    for index in 0..5 {
        let mut entry = stdio(&format!("server-{index}"), "/usr/bin/npx");
        entry.env = (0..MAX_BINDINGS)
            .map(|binding| {
                (
                    format!("PUBLIC_KEY_{binding}"),
                    secret("x".repeat(4096 - 1)),
                )
            })
            .collect();
        entries.push(entry);
    }
    assert!(entries.len() <= MAX_SERVERS);
    assert_eq!(
        validate(&entries),
        Err(Defect::TooLarge),
        "the composed file would exceed MAX_FILE_BYTES"
    );
}
#[test]
fn stdio_validation_requires_a_command_and_bounds_every_field() {
    assert_eq!(validate(&[stdio("alpha", "npx")]), Ok(()));

    let mut no_command = stdio("alpha", "npx");
    no_command.command.clear();
    assert_eq!(validate(&[no_command]), Err(Defect::Command));

    let mut bad_name = stdio("a b", "npx");
    assert_eq!(validate(&[bad_name.clone()]), Err(Defect::ServerName));
    bad_name.server_name = "a".repeat(MAX_SERVER_NAME + 1);
    assert_eq!(validate(&[bad_name]), Err(Defect::ServerName));
    assert!(server_name_valid("a_b-C9"));
    assert!(!server_name_valid(""));
    assert!(!server_name_valid("dot.name"));

    let mut many = stdio("alpha", "npx");
    many.args = vec!["a".to_owned(); MAX_ARGS + 1];
    assert_eq!(validate(&[many]), Err(Defect::TooManyArgs));

    let mut huge = stdio("alpha", "npx");
    huge.args = vec!["a".repeat(MAX_TEXT + 1)];
    assert_eq!(validate(&[huge]), Err(Defect::Argument));

    let mut control = stdio("alpha", "npx");
    control.args = vec!["a\nb".to_owned()];
    assert_eq!(validate(&[control]), Err(Defect::Argument));

    let mut relative = stdio("alpha", "npx");
    relative.cwd = "relative/path".to_owned();
    assert_eq!(validate(&[relative.clone()]), Err(Defect::Cwd));
    relative.cwd = "/tmp/../etc".to_owned();
    assert_eq!(validate(&[relative.clone()]), Err(Defect::Cwd));
    relative.cwd = "/definitely/not/a/native/directory".to_owned();
    assert_eq!(validate(&[relative]), Err(Defect::Cwd));

    let mut timeout = stdio("alpha", "npx");
    timeout.tool_call_timeout_ms = 0;
    assert_eq!(validate(&[timeout.clone()]), Err(Defect::Timeout));
    timeout.tool_call_timeout_ms = MAX_TIMEOUT_MS + 1;
    assert_eq!(validate(&[timeout]), Err(Defect::Timeout));

    let mut duplicate = vec![stdio("alpha", "npx"), stdio("alpha", "other")];
    assert_eq!(validate(&duplicate), Err(Defect::DuplicateServerName));
    duplicate[1].server_name = "beta".to_owned();
    assert_eq!(validate(&duplicate), Ok(()));

    let mut too_many = Vec::new();
    for index in 0..=MAX_SERVERS {
        too_many.push(stdio(&format!("s{index}"), "npx"));
    }
    assert_eq!(validate(&too_many), Err(Defect::TooManyServers));
}

#[test]
fn environment_and_header_names_use_their_own_rules() {
    let mut environment = stdio("alpha", "npx");
    environment.env = vec![("9BAD".to_owned(), Secret::new("x".to_owned()))];
    assert_eq!(validate(&[environment.clone()]), Err(Defect::BindingKey));
    environment.env = vec![("_OK9".to_owned(), Secret::new("x".to_owned()))];
    assert_eq!(validate(&[environment.clone()]), Ok(()));
    environment.env = vec![("ok".to_owned(), Secret::new("a\nb".to_owned()))];
    assert_eq!(validate(&[environment]), Err(Defect::BindingValue));

    let mut headers = http("alpha", "https://example.test/mcp");
    headers.headers = vec![("Author ization".to_owned(), Secret::new("x".to_owned()))];
    assert_eq!(validate(&[headers.clone()]), Err(Defect::BindingKey));
    headers.headers = vec![("X-Trace_Id.9".to_owned(), Secret::new("x".to_owned()))];
    assert_eq!(validate(&[headers.clone()]), Ok(()));
    headers.headers = vec![("X".to_owned(), Secret::new(String::new()))];
    assert_eq!(validate(&[headers]), Err(Defect::BindingValue));

    let mut many = stdio("alpha", "npx");
    many.env = (0..=MAX_BINDINGS)
        .map(|index| (format!("K{index}"), Secret::new("x".to_owned())))
        .collect();
    assert_eq!(validate(&[many]), Err(Defect::TooManyBindings));
}

#[test]
fn streamable_http_validation_requires_an_absolute_endpoint() {
    assert_eq!(
        validate(&[http("alpha", "https://example.test/mcp")]),
        Ok(())
    );
    assert_eq!(
        validate(&[http("alpha", "http://127.0.0.1:3000/mcp")]),
        Ok(())
    );
    for url in [
        "",
        "example.test/mcp",
        "ftp://example.test/mcp",
        "https://",
        "https://example.test/mcp#fragment",
        "https://exa mple.test/mcp",
        "https://example.test/mcp\n",
    ] {
        assert_eq!(
            validate(&[http("alpha", url)]),
            Err(Defect::Url),
            "{url:?} must be refused"
        );
    }
}

// ---- row generation ----

#[test]
fn rendered_rows_carry_the_loader_dialect_and_no_other_transport_keys() {
    let text = render(&[stdio("alpha", "/usr/bin/npx")]);
    assert!(text.contains("- insert:\n"));
    assert!(text.contains("    - id: native-mcp-alpha\n"));
    assert!(text.contains("      name: '@deepseek-ai/dsh-mcp-client'\n"));
    assert!(text.contains("      config:\n"));
    assert!(text.contains("        transport: stdio\n"));
    assert!(text.contains("        serverName: \"alpha\"\n"));
    assert!(text.contains("        command: \"/usr/bin/npx\"\n"));
    assert!(text.contains("        args:\n          - \"--serve\"\n"));
    assert!(text.contains("        env:\n          \"API_TOKEN\": \"PUBLIC-token\"\n"));
    assert!(text.contains("        cwd: \"\"\n"));
    assert!(text.contains("        toolCallTimeoutMs: 60000\n"));
    assert!(text.contains("        failOnStartupError: true\n"));
    assert!(!text.contains("url:"));
    assert!(!text.contains("headers:"));

    let http_text = render(&[http("beta", "https://example.test/mcp")]);
    assert!(http_text.contains("        transport: streamable-http\n"));
    assert!(http_text.contains("        url: \"https://example.test/mcp\"\n"));
    assert!(
        http_text.contains("        headers:\n          \"Authorization\": \"Bearer PUBLIC\"\n")
    );
    assert!(!http_text.contains("command:"));
    assert!(!http_text.contains("args:"));
    assert!(!http_text.contains("env:"));
    assert!(!http_text.contains("cwd:"));
}

#[test]
fn disabled_rows_keep_their_entry_and_configuration() {
    let mut disabled = stdio("alpha", "/usr/bin/npx");
    disabled.enabled = false;
    let text = render(&[disabled.clone()]);
    assert!(text.contains("      name: '@deepseek-ai/dsh-mcp-client'\n      disabled: true\n"));
    let parsed = parse(&text).expect("canonical round trip");
    assert_eq!(parsed, vec![disabled]);

    let enabled_text = render(&[stdio("alpha", "/usr/bin/npx")]);
    assert!(!enabled_text.contains("disabled: true"));
}

#[test]
fn an_empty_managed_set_is_an_empty_patch_list() {
    let text = render(&[]);
    assert!(text.ends_with("[]\n"));
    assert_eq!(parse(&text), Ok(Vec::new()));
}

#[test]
fn render_and_parse_round_trip_every_field_including_escapes() {
    let entries = vec![
        stdio("alpha", "/usr/bin/npx"),
        http("beta", "http://127.0.0.1:3000/mcp?x=1"),
    ];
    assert_eq!(parse(&render(&entries)), Ok(entries.clone()));

    let mut escaped = stdio("gamma", "cmd \"quoted\" \\ backslash");
    escaped.args = vec![
        "a \"b\"".to_owned(),
        "c\\d".to_owned(),
        "日本語 — ünïcode".to_owned(),
    ];
    escaped.env = vec![("K".to_owned(), Secret::new("v \"1\" \\ 2".to_owned()))];
    let parsed = parse(&render(&[escaped.clone()])).expect("escape round trip");
    assert_eq!(parsed, vec![escaped]);
}

// ---- preserving existing content, or refusing it ----

#[test]
fn unrecognised_content_is_refused_with_the_offending_line() {
    let cases: Vec<(&str, usize)> = vec![
        ("- id: operator-row\n  disabled: true\n", 1),
        (
            "- insert:\n    - id: demo-mcp\n      name: '@deepseek-ai/dsh-mcp-client'\n",
            2,
        ),
        ("[]\n- insert:\n", 2),
        ("# only comments\n", 1),
        ("{}\n", 1),
        ("not-yaml\n", 1),
        (
            "- insert:\n    - id: native-mcp-alpha\n      name: '@deepseek-ai/dsh-mcp-client'\n      config:\n        transport: stdio\n        serverName: \"alpha\"\n        mystery: 1\n",
            7,
        ),
    ];
    for (text, line) in cases {
        let foreign = parse(text).expect_err("non-canonical content is refused");
        assert_eq!(foreign.line, line, "{text:?}");
        assert!(
            foreign
                .reason()
                .contains("not a patch entry this app can preserve")
        );
    }
}

#[test]
fn duplicated_or_missing_canonical_fields_are_refused() {
    let duplicated = "- insert:\n    - id: native-mcp-alpha\n      name: '@deepseek-ai/dsh-mcp-client'\n      config:\n        transport: stdio\n        transport: stdio\n";
    assert!(parse(duplicated).is_err());
    let missing = "- insert:\n    - id: native-mcp-alpha\n      name: '@deepseek-ai/dsh-mcp-client'\n      config:\n        transport: stdio\n        serverName: \"alpha\"\n";
    assert!(parse(missing).is_err());
    let mismatched = "- insert:\n    - id: native-mcp-beta\n      name: '@deepseek-ai/dsh-mcp-client'\n      config:\n        transport: stdio\n        serverName: \"alpha\"\n        command: \"x\"\n        args: []\n        env: {}\n        cwd: \"\"\n        toolCallTimeoutMs: 1\n        failOnStartupError: false\n";
    assert!(parse(mismatched).is_err());
    let foreign_plugin =
        "- insert:\n    - id: native-mcp-alpha\n      name: '@deepseek-ai/dsh-mcp-server'\n";
    assert!(parse(foreign_plugin).is_err());
    let oversized = format!("# pad\n{}", "x".repeat(MAX_FILE_BYTES));
    assert!(parse(&oversized).is_err());
}

#[test]
fn parsed_entries_are_validated_before_they_are_written() {
    // A hand-authored canonical row with an out-of-range timeout parses and is
    // then refused by validation, so the panel never writes it back.
    let text = "- insert:\n    - id: native-mcp-alpha\n      name: '@deepseek-ai/dsh-mcp-client'\n      config:\n        transport: stdio\n        serverName: \"alpha\"\n        command: \"npx\"\n        args: []\n        env: {}\n        cwd: \"\"\n        toolCallTimeoutMs: 999999999\n        failOnStartupError: false\n";
    let entries = parse(text).expect("canonical row");
    assert_eq!(validate(&entries), Err(Defect::Timeout));
}

// ---- home resolution and the atomic write ----

#[test]
fn home_resolution_refuses_relative_parent_and_non_private_paths() {
    assert!(resolve_home(Path::new("relative")).is_err());
    assert!(resolve_home(Path::new("/tmp/../etc")).is_err());
    let home = TempHome::new();
    assert_eq!(resolve_home(&home.path), Ok(home.path.clone()));
    std::fs::set_permissions(&home.path, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(resolve_home(&home.path).is_err());
}

#[test]
fn commit_writes_privately_backs_up_the_previous_file_and_advances_the_revision() {
    let home = TempHome::new();
    let first = vec![stdio("alpha", "/usr/bin/npx")];

    let outcome = commit(&home.path, None, &first);
    let WriteOutcome::Written {
        revision,
        backup,
        bytes,
    } = outcome
    else {
        panic!("first write must succeed: {outcome:?}");
    };
    assert_eq!(backup, None, "no previous file means no backup");
    let text = home.read().expect("file exists");
    assert_eq!(bytes, text.len());
    assert_eq!(revision, Revision::of(text.as_bytes()));
    assert_eq!(
        std::fs::metadata(home.file()).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(parse(&text), Ok(first.clone()));

    // The deterministic temporary file never survives a successful write.
    assert!(!home.path.join(TEMP_NAME).exists());

    let second = vec![
        stdio("alpha", "/usr/bin/npx"),
        http("beta", "https://example.test/mcp"),
    ];
    let outcome = commit(&home.path, Some(revision), &second);
    let WriteOutcome::Written {
        revision: next,
        backup,
        ..
    } = outcome
    else {
        panic!("second write must succeed: {outcome:?}");
    };
    let backup = backup.expect("previous file is backed up");
    let backups = home.backups();
    assert_eq!(backups, vec![backup.clone()]);
    assert_eq!(
        std::fs::read_to_string(home.path.join(&backup)).unwrap(),
        text
    );
    assert!(
        std::fs::metadata(home.path.join(&backup))
            .unwrap()
            .permissions()
            .mode()
            & 0o777
            == 0o600
    );
    assert_ne!(next, revision);
    assert_eq!(parse(&home.read().unwrap()), Ok(second));
}

#[test]
fn commit_refuses_a_stale_revision_a_new_file_and_unrecognised_content() {
    let home = TempHome::new();
    let entries = vec![stdio("alpha", "/usr/bin/npx")];
    let stale = Revision { bytes: 1, hash: 2 };
    assert_eq!(
        commit(&home.path, Some(stale), &entries),
        WriteOutcome::Conflict
    );
    assert_eq!(home.read(), None, "a refused write creates no file");

    home.plant("# operator file\n");
    assert_eq!(commit(&home.path, None, &entries), WriteOutcome::Conflict);
    assert_eq!(
        commit(
            &home.path,
            Some(Revision::of(b"# operator file\n")),
            &entries
        ),
        WriteOutcome::Refused(Refusal::Foreign)
    );
    assert_eq!(home.read().as_deref(), Some("# operator file\n"));
    assert!(home.backups().is_empty(), "a refused write takes no backup");
}

#[test]
fn commit_writes_nothing_when_validation_or_the_home_is_unusable() {
    let home = TempHome::new();
    let mut invalid = stdio("alpha", "/usr/bin/npx");
    invalid.server_name = "has space".to_owned();
    assert_eq!(
        commit(&home.path, None, &[invalid]),
        WriteOutcome::Refused(Refusal::Invalid)
    );
    assert_eq!(home.read(), None);

    let missing = home.path.join("missing-home");
    assert_eq!(
        commit(&missing, None, &[stdio("alpha", "/usr/bin/npx")]),
        WriteOutcome::Refused(Refusal::Home)
    );

    let plain = TempHome::new();
    std::fs::set_permissions(&plain.path, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        commit(&plain.path, None, &[stdio("alpha", "/usr/bin/npx")]),
        WriteOutcome::Refused(Refusal::Home)
    );
    assert_eq!(plain.read(), None);

    let private = TempHome::new();
    private.plant(&render(&[stdio("alpha", "/usr/bin/npx")]));
    std::fs::set_permissions(private.file(), std::fs::Permissions::from_mode(0o644)).unwrap();
    let previous = private.read().unwrap();
    assert_eq!(
        commit(
            &private.path,
            Some(Revision::of(previous.as_bytes())),
            &[stdio("beta", "/usr/bin/npx")]
        ),
        WriteOutcome::Refused(Refusal::Permissions)
    );
    assert_eq!(private.read().as_deref(), Some(previous.as_str()));
}

#[test]
fn left_over_temporary_files_are_reused_not_accumulated() {
    let home = TempHome::new();
    let entries = vec![stdio("alpha", "/usr/bin/npx")];
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(home.path.join(TEMP_NAME))
        .unwrap();
    let outcome = commit(&home.path, None, &entries);
    assert!(matches!(outcome, WriteOutcome::Written { .. }));
    assert!(!home.path.join(TEMP_NAME).exists());
    assert_eq!(parse(&home.read().unwrap()), Ok(entries));
}

#[test]
fn loading_reports_the_revision_entries_and_a_foreign_reason() {
    let home = TempHome::new();
    let entries = vec![stdio("alpha", "/usr/bin/npx")];
    let loaded = load(&home.path, Vec::new(), true);
    assert_eq!(loaded.revision, None);
    assert_eq!(loaded.entries, Vec::new());
    assert_eq!(loaded.foreign, None);
    assert_eq!(loaded.path, home.file().to_string_lossy());

    home.plant(&render(&entries));
    let text = home.read().unwrap();
    let loaded = load(&home.path, Vec::new(), true);
    assert_eq!(loaded.revision, Some(Revision::of(text.as_bytes())));
    assert_eq!(loaded.entries, entries);
    assert!(loaded.foreign.is_none());

    home.plant("# operator file\n");
    let loaded = load(&home.path, Vec::new(), false);
    assert!(loaded.foreign.is_some());
    assert!(!loaded.inventory_read);

    let missing = home.path.join("missing-home");
    let loaded = load(&missing, Vec::new(), true);
    // The home could not be resolved: the read never inspected a file, so this is a read error
    // and no file content may be blamed for it.
    assert!(loaded.read_error.is_some());
    assert!(loaded.foreign.is_none());
    assert!(loaded.path.is_empty());

    let plain = TempHome::new();
    std::fs::set_permissions(&plain.path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let loaded = load(&plain.path, Vec::new(), true);
    assert!(loaded.read_error.is_some());
    assert!(loaded.foreign.is_none());
}

// ---- panel state machine ----

const CONTEXT: Context = Context {
    epoch: 1,
    visible: true,
    enabled: true,
};

fn opened() -> (Mcp, ReadTicket) {
    let mut panel = Mcp::default();
    let Effect::Read(ticket) = panel.enter(CONTEXT.epoch) else {
        panic!("opening reads");
    };
    (panel, ticket)
}

fn with_entries(entries: Vec<ServerEntry>) -> (Mcp, ReadTicket) {
    let (mut panel, ticket) = opened();
    let loaded = Loaded {
        path: "/home/native/cordis.patch.yml".to_owned(),
        revision: Some(Revision {
            bytes: 10,
            hash: 20,
        }),
        entries,
        foreign: None,
        read_error: None,
        inventory: Vec::new(),
        inventory_read: true,
    };
    panel.loaded_snapshot(ticket, loaded);
    (panel, ticket)
}

/// Confirm the write the panel currently has under review.
fn confirm(panel: &mut Mcp) -> Submission {
    let token = panel.token();
    let effect = panel
        .apply(Action::Confirm(token), CONTEXT)
        .expect("confirm submits");
    let Effect::Write(submission) = effect else {
        panic!("confirm is the only write");
    };
    submission
}

/// Review the open draft and then confirm it: the only path that writes.
fn review_and_confirm(panel: &mut Mcp) -> Submission {
    let token = panel.token();
    assert!(panel.apply(Action::Review(token), CONTEXT).is_none());
    assert_eq!(panel.mode(), "review");
    confirm(panel)
}

#[test]
fn panel_opens_reads_and_dismisses_without_writing() {
    let (mut panel, ticket) = opened();
    assert!(panel.is_open());
    assert_eq!(panel.mode(), "list");
    assert_eq!(panel.read_ticket(), ticket);
    assert!(!panel.pending());

    // A second read is refused while the first is still outstanding.
    assert!(panel.apply(Action::Refresh(ticket), CONTEXT).is_none());
    panel.loaded_snapshot(
        ticket,
        Loaded {
            path: "/home/native/cordis.patch.yml".to_owned(),
            revision: None,
            entries: Vec::new(),
            foreign: None,
            read_error: None,
            inventory: Vec::new(),
            inventory_read: true,
        },
    );
    assert_eq!(
        panel.loaded().map(|l| l.path.as_str()),
        Some("/home/native/cordis.patch.yml")
    );

    // A refresh is a new read identity, not a write.
    let Effect::Read(next) = panel
        .apply(Action::Refresh(ticket), CONTEXT)
        .expect("refresh reads")
    else {
        panic!("refresh must be a read");
    };
    assert_ne!(next.serial, ticket.serial);
    // A stale snapshot cannot replace the current read.
    panel.loaded_snapshot(
        ticket,
        Loaded {
            path: "stale".to_owned(),
            revision: None,
            entries: Vec::new(),
            foreign: None,
            read_error: None,
            inventory: Vec::new(),
            inventory_read: true,
        },
    );
    assert!(panel.loaded().is_none());
    panel.loaded_snapshot(
        next,
        Loaded {
            path: "/home/native/cordis.patch.yml".to_owned(),
            revision: None,
            entries: Vec::new(),
            foreign: None,
            read_error: None,
            inventory: Vec::new(),
            inventory_read: true,
        },
    );
    assert_eq!(
        panel.loaded().map(|l| l.path.as_str()),
        Some("/home/native/cordis.patch.yml")
    );

    panel.apply(Action::Dismiss(panel.read_ticket()), CONTEXT);
    assert!(!panel.is_open());
    // A dismissed panel refuses a stale dismissal and any later action.
    assert!(panel.apply(Action::Dismiss(ticket), CONTEXT).is_none());
    assert!(
        panel
            .apply(Action::BeginAdd(panel.read_ticket()), CONTEXT)
            .is_none()
    );
}

#[test]
fn add_requires_the_loaded_document_review_and_confirm() {
    let (mut panel, _) = with_entries(Vec::new());
    assert!(
        panel
            .apply(Action::BeginAdd(panel.read_ticket()), CONTEXT)
            .is_none()
    );
    assert_eq!(panel.mode(), "form");

    // Confirm without a review is refused.
    let sketch = panel.entries().to_vec();
    assert!(
        panel
            .apply(Action::Confirm(panel.token()), CONTEXT)
            .is_none()
    );
    assert!(!panel.pending());

    let token = panel.token();
    for (field, value) in [
        (Field::ServerName, "alpha"),
        (Field::Command, "/usr/bin/npx"),
    ] {
        panel.apply(
            Action::Field {
                token: panel.token(),
                field,
                value: value.to_owned(),
            },
            CONTEXT,
        );
    }
    // A field input bound to a superseded editor generation is dropped.
    panel.apply(
        Action::SetFailOnStartup {
            token: panel.token(),
            value: true,
        },
        CONTEXT,
    );
    assert!(
        panel
            .apply(
                Action::Field {
                    token,
                    field: Field::Command,
                    value: "ignored".to_owned(),
                },
                CONTEXT,
            )
            .is_none()
    );
    assert_eq!(panel.draft().unwrap().command(), "/usr/bin/npx");

    let submission = review_and_confirm(&mut panel);
    assert_eq!(submission.kind, Kind::Add);
    assert_eq!(submission.entries.len(), 1);
    assert_eq!(submission.entries[0].server_name, "alpha");
    assert!(panel.pending());
    assert!(
        panel
            .apply(Action::Review(panel.token()), CONTEXT)
            .is_none()
    );
    assert!(
        panel
            .apply(Action::Confirm(panel.token()), CONTEXT)
            .is_none()
    );
    assert_eq!(
        panel.entries(),
        sketch.as_slice(),
        "no optimistic state before the receipt"
    );
}

#[test]
fn cancel_and_cancel_review_write_nothing() {
    let entries = vec![stdio("alpha", "/usr/bin/npx")];
    let (mut panel, _) = with_entries(entries.clone());
    assert!(
        panel
            .apply(Action::BeginAdd(panel.read_ticket()), CONTEXT)
            .is_none()
    );
    panel.apply(
        Action::Field {
            token: panel.token(),
            field: Field::ServerName,
            value: "beta".to_owned(),
        },
        CONTEXT,
    );
    let token = panel.token();
    assert!(panel.apply(Action::Review(token), CONTEXT).is_none());
    assert!(
        panel
            .apply(Action::CancelReview(panel.token()), CONTEXT)
            .is_none()
    );
    assert_eq!(panel.mode(), "form");
    assert!(panel.review_kind().is_none());
    // The canceled review cannot be confirmed by replay.
    assert!(
        panel
            .apply(Action::Confirm(panel.token()), CONTEXT)
            .is_none()
    );
    assert!(!panel.pending());

    assert!(
        panel
            .apply(Action::Cancel(panel.token()), CONTEXT)
            .is_none()
    );
    assert_eq!(panel.mode(), "list");
    assert_eq!(panel.entries(), entries.as_slice());
    assert!(!panel.pending());
}

#[test]
fn edit_restates_every_field_and_preserves_the_other_rows() {
    let entries = vec![
        stdio("alpha", "/usr/bin/npx"),
        http("beta", "https://a.test/mcp"),
    ];
    let (mut panel, _) = with_entries(entries.clone());
    assert!(
        panel
            .apply(
                Action::BeginEdit {
                    ticket: panel.read_ticket(),
                    index: 0
                },
                CONTEXT
            )
            .is_none()
    );
    let draft = panel.draft().expect("edit draft");
    assert_eq!(draft.server_name(), "alpha");
    assert_eq!(draft.command(), "/usr/bin/npx");
    assert_eq!(draft.args(), &["--serve".to_owned()]);
    assert_eq!(
        draft.bindings(BindingKind::Environment),
        vec![("API_TOKEN", "unchanged · 12 bytes".to_owned())]
    );
    assert!(draft.binding_input(BindingKind::Environment, 0).is_none());
    // The other transport's fields are not carried into this draft.
    assert_eq!(draft.url(), "");
    assert!(draft.bindings(BindingKind::Header).is_empty());

    panel.apply(
        Action::Field {
            token: panel.token(),
            field: Field::Command,
            value: "/usr/bin/other".to_owned(),
        },
        CONTEXT,
    );
    let submission = review_and_confirm(&mut panel);
    assert_eq!(submission.kind, Kind::Edit { index: 0 });
    assert_eq!(submission.entries.len(), 2);
    let edited = &submission.entries[0];
    assert_eq!(edited.command, "/usr/bin/other");
    assert_eq!(edited.args, vec!["--serve".to_owned()]);
    assert_eq!(
        edited.env,
        vec![(
            "API_TOKEN".to_owned(),
            Secret::new("PUBLIC-token".to_owned())
        )],
        "the loaded secret is restated without being displayed"
    );
    assert_eq!(edited.tool_call_timeout_ms, DEFAULT_TIMEOUT_MS);
    assert!(edited.fail_on_startup_error);
    assert_eq!(
        submission.entries[1], entries[1],
        "other rows are preserved"
    );
}

#[test]
fn replace_value_never_displayed_and_removal_is_explicit() {
    let (mut panel, _) = with_entries(vec![stdio("alpha", "/usr/bin/npx")]);
    assert!(
        panel
            .apply(
                Action::BeginEdit {
                    ticket: panel.read_ticket(),
                    index: 0
                },
                CONTEXT
            )
            .is_none()
    );
    let view = panel.draft().unwrap();
    assert_eq!(
        view.bindings(BindingKind::Environment),
        vec![("API_TOKEN", "unchanged · 12 bytes".to_owned())]
    );
    assert!(!format!("{:?}", Secret::new("PUBLIC-token".to_owned())).contains("PUBLIC"));

    // A secret arriving for a row that is not in replacement mode is dropped.
    assert!(
        panel
            .apply(
                Action::SetBindingSecret {
                    token: panel.token(),
                    binding: BindingKind::Environment,
                    index: 0,
                    value: Secret::new("PUBLIC-new".to_owned()),
                },
                CONTEXT
            )
            .is_none()
    );
    assert!(
        panel
            .apply(
                Action::ReplaceBindingValue {
                    token: panel.token(),
                    binding: BindingKind::Environment,
                    index: 0,
                },
                CONTEXT
            )
            .is_none()
    );
    assert!(
        panel
            .draft()
            .unwrap()
            .binding_input(BindingKind::Environment, 0)
            .is_some()
    );
    panel.apply(
        Action::SetBindingSecret {
            token: panel.token(),
            binding: BindingKind::Environment,
            index: 0,
            value: Secret::new("PUBLIC-new".to_owned()),
        },
        CONTEXT,
    );
    let submission = review_and_confirm(&mut panel);
    assert_eq!(
        submission.entries[0].env,
        vec![("API_TOKEN".to_owned(), Secret::new("PUBLIC-new".to_owned()))]
    );
}

#[test]
fn remove_disables_the_row_and_keeps_its_configuration() {
    let entry = stdio("alpha", "/usr/bin/npx");
    let (mut panel, _) = with_entries(vec![entry.clone()]);
    let ticket = panel.read_ticket();
    assert!(
        panel
            .apply(Action::BeginDisable { ticket, index: 0 }, CONTEXT)
            .is_none()
    );
    assert_eq!(panel.mode(), "review");
    let submission = confirm(&mut panel);
    assert_eq!(submission.kind, Kind::Disable { index: 0 });
    assert_eq!(submission.entries.len(), 1);
    assert!(!submission.entries[0].enabled);
    assert_eq!(submission.entries[0].command, entry.command);
    assert_eq!(submission.entries[0].args, entry.args);
    assert_eq!(submission.entries[0].env.len(), 1);

    // A disable of an already disabled row is refused.
    panel.written(
        &submission,
        WriteOutcome::Written {
            revision: Revision { bytes: 1, hash: 1 },
            backup: None,
            bytes: 1,
        },
    );
    assert!(
        panel
            .apply(
                Action::BeginDisable {
                    ticket: panel.read_ticket(),
                    index: 0
                },
                CONTEXT
            )
            .is_none()
    );
}

#[test]
fn the_write_ticket_carries_the_loaded_revision_and_follows_a_confirmed_write() {
    let (mut panel, _) = with_entries(vec![stdio("alpha", "/usr/bin/npx")]);
    panel.apply(
        Action::BeginEdit {
            ticket: panel.read_ticket(),
            index: 0,
        },
        CONTEXT,
    );
    panel.apply(
        Action::Field {
            token: panel.token(),
            field: Field::Command,
            value: "/usr/bin/other".to_owned(),
        },
        CONTEXT,
    );
    let submission = review_and_confirm(&mut panel);
    assert_eq!(
        submission.ticket.expected,
        Some(Revision {
            bytes: 10,
            hash: 20
        })
    );
    assert_eq!(submission.ticket.epoch, CONTEXT.epoch);
    assert_eq!(submission.ticket.panel, panel.read_ticket().panel);
    assert_eq!(submission.ticket.serial, submission.ticket.serial);

    let next = Revision {
        bytes: 99,
        hash: 77,
    };
    panel.written(
        &submission,
        WriteOutcome::Written {
            revision: next,
            backup: Some("cordis.patch.yml.backup-1".to_owned()),
            bytes: 99,
        },
    );
    assert_eq!(panel.notice(), Notice::Confirmed);
    assert_eq!(
        panel.loaded().and_then(|loaded| loaded.revision),
        Some(next)
    );
    let report = panel.report().expect("confirmed report");
    assert_eq!(report.path, "/home/native/cordis.patch.yml");
    assert_eq!(report.bytes, 99);

    panel.apply(
        Action::BeginEdit {
            ticket: panel.read_ticket(),
            index: 0,
        },
        CONTEXT,
    );
    panel.apply(
        Action::Field {
            token: panel.token(),
            field: Field::Command,
            value: "/usr/bin/third".to_owned(),
        },
        CONTEXT,
    );
    let submission = review_and_confirm(&mut panel);
    assert_eq!(submission.ticket.expected, Some(next));
}

#[test]
fn every_unconfirmed_outcome_is_reported_and_never_retried() {
    for (outcome, notice) in [
        (WriteOutcome::Refused(Refusal::Foreign), Notice::Foreign),
        (WriteOutcome::Refused(Refusal::Permissions), Notice::Refused),
        (WriteOutcome::Conflict, Notice::Conflict),
        (WriteOutcome::Indeterminate, Notice::Indeterminate),
        (WriteOutcome::NotSent, Notice::NotSent),
    ] {
        let (mut panel, _) = with_entries(vec![stdio("alpha", "/usr/bin/npx")]);
        panel.apply(
            Action::BeginEdit {
                ticket: panel.read_ticket(),
                index: 0,
            },
            CONTEXT,
        );
        let submission = review_and_confirm(&mut panel);
        panel.written(&submission, outcome.clone());
        assert_eq!(panel.notice(), notice, "{outcome:?}");
        assert!(!panel.pending());
        assert!(panel.report().is_none());
        // A duplicate late receipt for the settled ticket changes nothing.
        panel.written(
            &submission,
            WriteOutcome::Written {
                revision: Revision { bytes: 1, hash: 1 },
                backup: None,
                bytes: 1,
            },
        );
        assert_eq!(panel.notice(), notice);
        assert!(!panel.pending());
    }
    // An outcome for a ticket this panel never admitted is ignored.
    let (mut panel, _) = with_entries(vec![stdio("alpha", "/usr/bin/npx")]);
    let ghost = Submission {
        ticket: WriteTicket {
            epoch: CONTEXT.epoch,
            panel: panel.read_ticket().panel,
            serial: 9,
            expected: None,
        },
        kind: Kind::Add,
        entries: Vec::new(),
    };
    panel.written(&ghost, WriteOutcome::Indeterminate);
    assert_eq!(panel.notice(), Notice::Loaded);
}

#[test]
fn an_unrecognised_file_refuses_every_mutation() {
    let (mut panel, ticket) = opened();
    panel.loaded_snapshot(
        ticket,
        Loaded {
            path: "/home/native/cordis.patch.yml".to_owned(),
            revision: Some(Revision { bytes: 4, hash: 4 }),
            entries: Vec::new(),
            foreign: Some("line 1 is not a patch entry this app can preserve: - id: x".to_owned()),
            read_error: None,
            inventory: Vec::new(),
            inventory_read: true,
        },
    );
    assert_eq!(panel.notice(), Notice::Foreign);
    assert!(panel.foreign_reason().is_some());
    assert!(!panel.writable());
    assert_eq!(
        panel.foreign_reason(),
        Some("line 1 is not a patch entry this app can preserve: - id: x")
    );
    // Only an explicit refresh re-reads; the panel never retries on its own.
    let Effect::Read(next) = panel
        .apply(Action::Refresh(ticket), CONTEXT)
        .expect("explicit refresh reads")
    else {
        panic!("refresh must be a read");
    };
    assert_ne!(next.serial, ticket.serial);
    assert!(
        panel
            .apply(Action::BeginAdd(panel.read_ticket()), CONTEXT)
            .is_none()
    );
    assert!(
        panel
            .apply(
                Action::BeginEdit {
                    ticket: panel.read_ticket(),
                    index: 0
                },
                CONTEXT
            )
            .is_none()
    );
    assert!(
        panel
            .apply(
                Action::BeginDisable {
                    ticket: panel.read_ticket(),
                    index: 0
                },
                CONTEXT
            )
            .is_none()
    );
    assert_eq!(panel.mode(), "list");
    assert!(!panel.pending(), "no mutation was ever admitted");
}

#[test]
fn context_epoch_visibility_and_shutdown_fence_the_panel() {
    let (mut panel, _) = with_entries(vec![stdio("alpha", "/usr/bin/npx")]);
    let disable = Action::BeginDisable {
        ticket: panel.read_ticket(),
        index: 0,
    };
    assert!(
        panel
            .apply(
                disable.clone(),
                Context {
                    visible: false,
                    ..CONTEXT
                }
            )
            .is_none()
    );
    assert!(
        panel
            .apply(
                disable.clone(),
                Context {
                    enabled: false,
                    ..CONTEXT
                }
            )
            .is_none()
    );
    assert!(
        panel
            .apply(
                disable.clone(),
                Context {
                    epoch: 2,
                    ..CONTEXT
                }
            )
            .is_none()
    );
    assert_eq!(panel.mode(), "list");

    // Disconnecting with an admitted write keeps the outcome unknown.
    panel.apply(disable, CONTEXT);
    let submission = review_and_confirm(&mut panel);
    panel.disconnect();
    assert!(!panel.pending());
    assert_eq!(panel.notice(), Notice::Indeterminate);
    assert!(!panel.is_open());
    let late = panel.apply(Action::Dismiss(panel.read_ticket()), CONTEXT);
    assert!(late.is_none());
    panel.written(
        &submission,
        WriteOutcome::Written {
            revision: Revision { bytes: 1, hash: 1 },
            backup: None,
            bytes: 1,
        },
    );
    assert_eq!(panel.notice(), Notice::Indeterminate);
}

#[test]
fn an_absent_file_is_a_valid_expected_revision_only_after_a_successful_read() {
    let (mut panel, ticket) = opened();
    // Before any snapshot there is no loaded document and no review is possible.
    assert!(
        panel
            .apply(Action::BeginAdd(panel.read_ticket()), CONTEXT)
            .is_none()
    );

    panel.loaded_snapshot(
        ticket,
        Loaded {
            path: "/home/native/cordis.patch.yml".to_owned(),
            revision: None,
            entries: Vec::new(),
            foreign: None,
            read_error: None,
            inventory: Vec::new(),
            inventory_read: true,
        },
    );
    panel.apply(Action::BeginAdd(panel.read_ticket()), CONTEXT);
    panel.apply(
        Action::Field {
            token: panel.token(),
            field: Field::ServerName,
            value: "alpha".to_owned(),
        },
        CONTEXT,
    );
    panel.apply(
        Action::Field {
            token: panel.token(),
            field: Field::Command,
            value: "/usr/bin/npx".to_owned(),
        },
        CONTEXT,
    );
    let submission = review_and_confirm(&mut panel);
    assert_eq!(submission.ticket.expected, None);
}

#[test]
fn bound_reached_is_refused_without_touching_the_draft() {
    let mut entry = stdio("alpha", "/usr/bin/npx");
    entry.args = vec!["a".to_owned(); MAX_ARGS];
    entry.env = (0..MAX_BINDINGS)
        .map(|index| (format!("K{index}"), Secret::new("x".to_owned())))
        .collect();
    let (mut panel, _) = with_entries(vec![entry]);
    panel.apply(
        Action::BeginEdit {
            ticket: panel.read_ticket(),
            index: 0,
        },
        CONTEXT,
    );
    let before = panel.token();
    assert!(
        panel
            .apply(Action::AddArgument(panel.token()), CONTEXT)
            .is_none()
    );
    assert!(
        panel
            .apply(
                Action::AddBinding {
                    token: panel.token(),
                    binding: BindingKind::Environment
                },
                CONTEXT
            )
            .is_none()
    );
    assert_eq!(panel.draft().unwrap().args().len(), MAX_ARGS);
    assert_eq!(
        panel
            .draft()
            .unwrap()
            .bindings(BindingKind::Environment)
            .len(),
        MAX_BINDINGS
    );
    assert_eq!(panel.token().editor, before.editor);
}

#[test]
fn transport_switch_starts_from_that_transports_own_defaults() {
    let (mut panel, _) = with_entries(Vec::new());
    panel.apply(Action::BeginAdd(panel.read_ticket()), CONTEXT);
    panel.apply(
        Action::Field {
            token: panel.token(),
            field: Field::ServerName,
            value: "alpha".to_owned(),
        },
        CONTEXT,
    );
    panel.apply(
        Action::Field {
            token: panel.token(),
            field: Field::Command,
            value: "/usr/bin/npx".to_owned(),
        },
        CONTEXT,
    );
    panel.apply(
        Action::SetFailOnStartup {
            token: panel.token(),
            value: true,
        },
        CONTEXT,
    );
    panel.apply(
        Action::SetTransport {
            token: panel.token(),
            transport: Transport::StreamableHttp,
        },
        CONTEXT,
    );
    let draft = panel.draft().unwrap();
    assert_eq!(draft.transport(), Transport::StreamableHttp);
    assert_eq!(draft.server_name(), "alpha");
    assert_eq!(draft.command(), "");
    assert_eq!(draft.url(), "");
    assert!(draft.fail_on_startup_error());

    // An out-of-range timeout is refused at review, never defaulted silently.
    panel.apply(
        Action::Field {
            token: panel.token(),
            field: Field::Timeout,
            value: "not-a-number".to_owned(),
        },
        CONTEXT,
    );
    assert!(
        panel
            .apply(Action::Review(panel.token()), CONTEXT)
            .is_none()
    );
    assert_eq!(panel.notice(), Notice::Invalid);
    assert!(panel.review_kind().is_none());
}
