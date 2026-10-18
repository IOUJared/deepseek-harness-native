//! Native MCP server management for the app-owned Home patch layer.
//!
//! Every MCP server is one `@deepseek-ai/dsh-mcp-client` row that this app
//! inserts into `<native-home>/cordis.patch.yml`. That file is the Host's
//! home-level user patch layer: `runProfile` composes it above the profile's own
//! `profiles/desktop/cordis.patch.yml` and below `--patch` overlays. The native
//! Core already exports the native home as `$DSH_HOME`, so the Host reads this
//! file on every start without any Core change and without touching another
//! writer's document — the official ConfigEditor persists managed-Codex settings
//! into the profile's own patch file.
//!
//! This module renders the whole document from validated entries, re-parses the
//! rendered text with its own strict parser before anything is replaced, writes
//! through a pinned private home descriptor, keeps a timestamped backup of the
//! previous file, and refuses every mutation when the existing content is not
//! the canonical form this app produces. It never merges into a shared YAML
//! document and never hand-rolls a rewrite that could drop an unknown row.
//!
//! Its private-file writer is deliberately separate from `draft_store.rs`, which owns the much
//! more frequent draft saves: those hold a cross-process advisory lock, verify the target's inode
//! stamp and 0600/nlink, and compare the decoded map before writing, but keep no backup. This file
//! is instead replaced rarely under an explicit review, so it takes a timestamped backup and
//! refuses any content it did not write. One shared writer would have to carry both taxonomies.
use dsh_native_transport::plugin::FiberPhase;
use rustix::fs::{AtFlags, FileType, Mode, OFlags, open, openat, renameat, statat, unlinkat};
use rustix::io::Errno;
use std::{
    fs::File,
    io::{Read, Write},
    os::unix::fs::PermissionsExt,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use zeroize::Zeroize;

/// Host plugin package every managed row mounts.
pub const PLUGIN_NAME: &str = "@deepseek-ai/dsh-mcp-client";
/// Patch file the Host composes from the native home.
pub const FILE_NAME: &str = "cordis.patch.yml";
/// Deterministic temporary name used for the atomic replacement.
const TEMP_NAME: &str = "cordis.patch.yml.native-tmp";
/// Row-id prefix that marks a row as owned by this app.
pub const ROW_PREFIX: &str = "native-mcp-";
/// Largest existing patch file this app will read or rewrite.
pub const MAX_FILE_BYTES: usize = 1024 * 1024;
/// Largest number of managed servers.
pub const MAX_SERVERS: usize = 32;
/// Largest number of arguments per stdio server.
pub const MAX_ARGS: usize = 64;
/// Largest number of environment variables or headers per server.
pub const MAX_BINDINGS: usize = 64;
/// Largest byte length of one operator-supplied value.
pub const MAX_TEXT: usize = 4096;
/// Largest byte length of one environment-variable or header name.
pub const MAX_KEY: usize = 128;
/// Largest byte length of a `serverName`; the plugin's own pattern is 1..=32.
pub const MAX_SERVER_NAME: usize = 32;
/// Bounds for `toolCallTimeoutMs`; the plugin's own default is 60000.
pub const MIN_TIMEOUT_MS: u64 = 1;
pub const MAX_TIMEOUT_MS: u64 = 3_600_000;
/// Offered for a new row, matching the plugin's own default.
pub const DEFAULT_TIMEOUT_MS: u64 = 60_000;
/// The honest next step after a confirmed write. This build has no hot reload
/// and no in-app Host restart, so the operator must reopen the app.
pub const NEXT_STEP: &str = "The Host composes this layer when it starts. This build loads the profile without --expose-internals, so the Host's config watcher cannot run and the native app has no in-app Host restart: the new rows mount after the native Harness app is restarted.";

/// Which transport one server uses. Selects the config union member.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transport {
    Stdio,
    StreamableHttp,
}

impl Transport {
    fn key(self) -> &'static str {
        match self {
            Self::Stdio => "stdio",
            Self::StreamableHttp => "streamable-http",
        }
    }
    /// Label for the list and the transport choice.
    pub fn label(self) -> &'static str {
        self.key()
    }
}

/// A value the panel never renders. Owned UTF-8 is overwritten on drop.
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    /// Wrap operator text typed into a binding field.
    pub fn new(value: String) -> Self {
        Self(value)
    }
    /// Byte length; the panel reports a length, never the bytes.
    pub fn len(&self) -> usize {
        self.0.len()
    }
    /// Read access for the renderer. The caller must not display the result.
    fn expose(&self) -> &str {
        &self.0
    }
}

impl PartialEq for Secret {
    /// Local configuration comparison only; not a hostile-wire check.
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for Secret {}
impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeMcpSecret([REDACTED])")
    }
}
impl Secret {
    /// Zero the complete allocation, spare capacity included. `Drop` calls this;
    /// the fixture reads it before the drop to observe the guarantee while the
    /// allocation is still live.
    fn wipe(&mut self) {
        self.0.zeroize();
    }
}
impl Drop for Secret {
    fn drop(&mut self) {
        // An environment or header value; the whole allocation goes with it.
        self.wipe();
    }
}

/// One managed `@deepseek-ai/dsh-mcp-client` row, as it will be written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerEntry {
    pub server_name: String,
    pub transport: Transport,
    /// `false` writes `disabled: true` on the row; the entry itself stays.
    pub enabled: bool,
    /// stdio: executable. Empty for the other transport.
    pub command: String,
    pub args: Vec<String>,
    pub env: Vec<(String, Secret)>,
    pub cwd: String,
    /// streamable-http: endpoint. Empty for stdio.
    pub url: String,
    pub headers: Vec<(String, Secret)>,
    pub tool_call_timeout_ms: u64,
    pub fail_on_startup_error: bool,
}

impl ServerEntry {
    /// The row id this entry owns. Stable for a given `serverName`.
    pub fn row_id(&self) -> String {
        format!("{ROW_PREFIX}{}", self.server_name)
    }
    /// The operator-visible target: the command for stdio, the URL otherwise.
    pub fn target(&self) -> &str {
        match self.transport {
            Transport::Stdio => &self.command,
            Transport::StreamableHttp => &self.url,
        }
    }
}

/// Why one entry cannot be written. Each case is a distinct defect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Defect {
    ServerName,
    DuplicateServerName,
    TooManyServers,
    Command,
    TooManyArgs,
    Argument,
    Cwd,
    Url,
    TooManyBindings,
    BindingKey,
    BindingValue,
    Timeout,
    /// The composed file would exceed the patch size this panel may write.
    TooLarge,
}

impl Defect {
    /// The sentence the panel shows. Names the defect, not the symptom.
    pub fn text(self) -> &'static str {
        match self {
            Self::ServerName => {
                "Server name must be 1-32 characters from A-Z, a-z, 0-9, underscore or hyphen."
            }
            Self::DuplicateServerName => {
                "Two servers cannot share a name: the Host reserves one tool namespace per name."
            }
            Self::TooManyServers => "At most 32 MCP servers can be managed from this panel.",
            Self::Command => "A stdio server needs a non-empty command of at most 4096 bytes.",
            Self::TooManyArgs => "At most 64 arguments are accepted for one server.",
            Self::Argument => {
                "Each argument must be at most 4096 bytes with no control characters."
            }
            Self::Cwd => {
                "Working directory must be empty, or an existing absolute directory without '..'."
            }
            Self::Url => "URL must be an absolute http:// or https:// endpoint without a fragment.",
            Self::TooManyBindings => {
                "At most 64 environment variables or headers are accepted for one server."
            }
            Self::BindingKey => {
                "Environment names use A-Z, 0-9 and underscore and cannot start with a digit; header names use HTTP token characters. Both are limited to 128 bytes."
            }
            Self::BindingValue => {
                "Each environment value or header value must be at most 4096 bytes with no control characters."
            }
            Self::Timeout => {
                "Tool-call timeout must be a whole number of milliseconds from 1 to 3600000."
            }
            Self::TooLarge => {
                "The configuration would exceed the 1 MiB patch file this panel writes. Reduce servers, arguments or binding values."
            }
        }
    }
}

/// Validate one complete managed set: names unique, every field in bounds.
pub fn validate(entries: &[ServerEntry]) -> Result<(), Defect> {
    if entries.len() > MAX_SERVERS {
        return Err(Defect::TooManyServers);
    }
    for (index, entry) in entries.iter().enumerate() {
        if !server_name_valid(&entry.server_name) {
            return Err(Defect::ServerName);
        }
        if entries[..index]
            .iter()
            .any(|other| other.server_name == entry.server_name)
        {
            return Err(Defect::DuplicateServerName);
        }
        if !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&entry.tool_call_timeout_ms) {
            return Err(Defect::Timeout);
        }
        match entry.transport {
            Transport::Stdio => {
                if entry.command.is_empty() || !text_valid(&entry.command) {
                    return Err(Defect::Command);
                }
                if entry.args.len() > MAX_ARGS {
                    return Err(Defect::TooManyArgs);
                }
                if entry.args.iter().any(|arg| !text_valid(arg)) {
                    return Err(Defect::Argument);
                }
                if !cwd_valid(&entry.cwd) {
                    return Err(Defect::Cwd);
                }
                bindings_valid(&entry.env, true)?;
            }
            Transport::StreamableHttp => {
                if !url_valid(&entry.url) {
                    return Err(Defect::Url);
                }
                bindings_valid(&entry.headers, false)?;
            }
        }
    }
    // The per-field bounds above do not bound the file they compose to: four servers with every
    // binding field at its limit render past `MAX_FILE_BYTES`, which `parse` then refuses at commit
    // time as a filesystem-shaped refusal. Refuse it here, where the user can still edit it.
    if render(entries).len() > MAX_FILE_BYTES {
        return Err(Defect::TooLarge);
    }
    Ok(())
}

fn bindings_valid(bindings: &[(String, Secret)], environment: bool) -> Result<(), Defect> {
    if bindings.len() > MAX_BINDINGS {
        return Err(Defect::TooManyBindings);
    }
    for (key, value) in bindings {
        let key_ok = key.len() <= MAX_KEY
            && if environment {
                environment_key_valid(key)
            } else {
                header_key_valid(key)
            };
        if !key_ok {
            return Err(Defect::BindingKey);
        }
        if !text_valid(value.expose()) {
            return Err(Defect::BindingValue);
        }
    }
    Ok(())
}

/// The plugin's own `SERVER_NAME_PATTERN`.
pub fn server_name_valid(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_SERVER_NAME
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// One operator value: bounded, non-empty here, and free of characters that
/// cannot be emitted as a double-quoted YAML scalar.
fn text_valid(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_TEXT && scalar_safe(value)
}

/// Characters a double-quoted YAML scalar carries literally or with a
/// two-character escape. Everything else is refused rather than escaped.
pub fn scalar_safe(value: &str) -> bool {
    value
        .chars()
        .all(|c| !c.is_control() && c != '\u{2028}' && c != '\u{2029}' && c != '\u{feff}')
}

fn environment_key_valid(key: &str) -> bool {
    let mut bytes = key.bytes();
    match bytes.next() {
        Some(b) if b.is_ascii_alphabetic() || b == b'_' => {}
        _ => return false,
    }
    bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn header_key_valid(key: &str) -> bool {
    !key.is_empty()
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || TCHAR.contains(&b))
}

const TCHAR: &[u8] = b"!#$%&'*+-.^_`|~";

fn cwd_valid(cwd: &str) -> bool {
    if cwd.is_empty() {
        return true;
    }
    if !text_valid(cwd) {
        return false;
    }
    let path = Path::new(cwd);
    path.is_absolute()
        && !path.components().any(|c| matches!(c, Component::ParentDir))
        && path.is_dir()
}

fn url_valid(url: &str) -> bool {
    if !text_valid(url) || url.contains('#') {
        return false;
    }
    let rest = match url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
    {
        Some(rest) => rest,
        None => return false,
    };
    let authority = rest.split(['/', '?']).next().unwrap_or("");
    !authority.is_empty() && !authority.chars().any(char::is_whitespace)
}

/// A change detector for the app-owned file: byte length and FNV-1a. This is
/// not a cryptographic digest and not an authentication token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Revision {
    pub bytes: usize,
    pub hash: u64,
}

impl Revision {
    pub fn of(bytes: &[u8]) -> Self {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Self {
            bytes: bytes.len(),
            hash,
        }
    }
}

/// The first content the parser cannot preserve, with its 1-based line number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Foreign {
    pub line: usize,
    pub text: String,
}

impl Foreign {
    fn at(line: usize, text: &str) -> Self {
        Self {
            line,
            text: excerpt(text),
        }
    }
    /// The sentence the panel and the write refusal both show.
    pub fn reason(&self) -> String {
        format!(
            "line {} is not a patch entry this app can preserve: {}",
            self.line, self.text
        )
    }
}

fn excerpt(line: &str) -> String {
    let cleaned: String = line
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(160)
        .collect();
    if cleaned.is_empty() {
        "(empty)".to_owned()
    } else {
        cleaned
    }
}

const HEADER: &str = concat!(
    "# Native-owned MCP composition for the isolated native Harness home.\n",
    "# Each entry inserts one @deepseek-ai/dsh-mcp-client row into the Host's\n",
    "# home-level patch layer. Managed by the native app: rewritten atomically\n",
    "# with a backup of the previous file. Content this app does not recognise is\n",
    "# never rewritten, and hand edits outside the canonical form make the app\n",
    "# refuse to manage the file.\n",
);
const ROW_INDENT: &str = "      ";
const INSERT_INDENT: &str = "    ";
const CONFIG_INDENT: &str = "        ";
const ITEM_INDENT: &str = "          ";
const ROW_NAME: &str = "name: '";
const ROW_DISABLED: &str = "disabled: true";
const ROW_CONFIG: &str = "config:";
const EMPTY_LIST: &str = "[]\n";

/// Render the complete app-owned document.
pub fn render(entries: &[ServerEntry]) -> String {
    let mut out = String::from(HEADER);
    if entries.is_empty() {
        out.push_str(EMPTY_LIST);
        return out;
    }
    for entry in entries {
        out.push_str("- insert:\n");
        out.push_str(INSERT_INDENT);
        out.push_str("- id: ");
        out.push_str(ROW_PREFIX);
        out.push_str(&entry.server_name);
        out.push('\n');
        out.push_str(ROW_INDENT);
        out.push_str(ROW_NAME);
        out.push_str(PLUGIN_NAME);
        out.push_str("'\n");
        if !entry.enabled {
            out.push_str(ROW_INDENT);
            out.push_str(ROW_DISABLED);
            out.push('\n');
        }
        out.push_str(ROW_INDENT);
        out.push_str(ROW_CONFIG);
        out.push('\n');
        push_scalar(&mut out, "transport", entry.transport.key());
        push_scalar(&mut out, "serverName", &quote(&entry.server_name));
        match entry.transport {
            Transport::Stdio => {
                push_scalar(&mut out, "command", &quote(&entry.command));
                if entry.args.is_empty() {
                    push_scalar(&mut out, "args", "[]");
                } else {
                    push_scalar(&mut out, "args", "");
                    for arg in &entry.args {
                        out.push_str(ITEM_INDENT);
                        out.push_str("- ");
                        out.push_str(&quote(arg));
                        out.push('\n');
                    }
                }
                if entry.env.is_empty() {
                    push_scalar(&mut out, "env", "{}");
                } else {
                    push_scalar(&mut out, "env", "");
                    for (key, value) in &entry.env {
                        push_binding(&mut out, key, value);
                    }
                }
                push_scalar(&mut out, "cwd", &quote(&entry.cwd));
            }
            Transport::StreamableHttp => {
                push_scalar(&mut out, "url", &quote(&entry.url));
                if entry.headers.is_empty() {
                    push_scalar(&mut out, "headers", "{}");
                } else {
                    push_scalar(&mut out, "headers", "");
                    for (key, value) in &entry.headers {
                        push_binding(&mut out, key, value);
                    }
                }
            }
        }
        push_scalar(
            &mut out,
            "toolCallTimeoutMs",
            &entry.tool_call_timeout_ms.to_string(),
        );
        push_scalar(
            &mut out,
            "failOnStartupError",
            if entry.fail_on_startup_error {
                "true"
            } else {
                "false"
            },
        );
    }
    out
}

fn push_binding(out: &mut String, key: &str, value: &Secret) {
    out.push_str(ITEM_INDENT);
    out.push_str(&quote(key));
    out.push_str(": ");
    out.push_str(&quote(value.expose()));
    out.push('\n');
}

fn push_scalar(out: &mut String, key: &str, value: &str) {
    out.push_str(CONFIG_INDENT);
    out.push_str(key);
    out.push(':');
    if value.is_empty() {
        out.push('\n');
    } else {
        out.push(' ');
        out.push_str(value);
        out.push('\n');
    }
}

fn quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

/// Decode one canonical double-quoted scalar. Only `\\` and `\"` are accepted.
fn scalar(text: &str) -> Option<String> {
    let body = text.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next()? {
                '\\' => out.push('\\'),
                '"' => out.push('"'),
                _ => return None,
            }
        } else if c == '"' {
            return None;
        } else {
            out.push(c);
        }
    }
    scalar_safe(&out).then_some(out)
}

/// Split one `key: value` line whose key is either quoted or a bare word.
fn split(line: &str) -> Option<(&str, &str)> {
    if let Some(rest) = line.strip_prefix('"') {
        let mut chars = rest.char_indices();
        while let Some((index, c)) = chars.next() {
            match c {
                '\\' => {
                    chars.next()?;
                }
                '"' => {
                    let key = &line[..index + 2];
                    let tail = &rest[index + 1..];
                    return tail
                        .strip_prefix(':')
                        .map(|value| (key, value.strip_prefix(' ').unwrap_or(value)));
                }
                _ => {}
            }
        }
        return None;
    }
    let (key, value) = line.split_once(':')?;
    Some((key, value.strip_prefix(' ').unwrap_or(value)))
}

/// Parse the canonical form this module renders. Anything else is refused with
/// the first offending line, so no unknown row can be silently dropped.
pub fn parse(text: &str) -> Result<Vec<ServerEntry>, Foreign> {
    if text.len() > MAX_FILE_BYTES {
        return Err(Foreign::at(
            0,
            "file is larger than the 1 MiB native management bound",
        ));
    }
    let body = text.strip_suffix('\n').unwrap_or(text);
    let lines: Vec<&str> = body.split('\n').collect();
    let mut entries: Vec<ServerEntry> = Vec::new();
    let mut saw_empty = false;
    let mut saw_entry = false;
    let mut at = 0usize;
    while at < lines.len() {
        let line = lines[at].trim_end();
        if line.is_empty() || line.starts_with('#') {
            at += 1;
            continue;
        }
        if line == "[]" {
            if saw_empty || saw_entry {
                return Err(Foreign::at(at + 1, line));
            }
            saw_empty = true;
            at += 1;
            continue;
        }
        if line != "- insert:" || saw_empty {
            return Err(Foreign::at(at + 1, line));
        }
        saw_entry = true;
        let start = at;
        at += 1;
        while at < lines.len() && lines[at].trim_end() != "- insert:" {
            at += 1;
        }
        entries.push(parse_entry(&lines[start..at], start)?);
        if entries.len() > MAX_SERVERS {
            return Err(Foreign::at(start + 1, "more than 32 managed entries"));
        }
    }
    if !saw_empty && !saw_entry {
        return Err(Foreign::at(
            1,
            "no loader patch list: an empty or comments-only file fails the Host's own parse",
        ));
    }
    Ok(entries)
}

fn parse_entry(block: &[&str], base: usize) -> Result<ServerEntry, Foreign> {
    let bad = |at: usize| Foreign::at(base + at + 1, block.get(at).copied().unwrap_or(""));
    if block.first().map(|line| line.trim_end()) != Some("- insert:") {
        return Err(bad(0));
    }
    let id_line = block.get(1).ok_or_else(|| bad(0))?.trim_end();
    let server_name = id_line
        .strip_prefix(INSERT_INDENT)
        .and_then(|rest| rest.strip_prefix("- id: "))
        .and_then(|rest| rest.strip_prefix(ROW_PREFIX))
        .ok_or_else(|| bad(1))?;
    if !server_name_valid(server_name) {
        return Err(bad(1));
    }
    let name_line = block.get(2).ok_or_else(|| bad(1))?.trim_end();
    let parsed_name = name_line
        .strip_prefix(ROW_INDENT)
        .and_then(|rest| rest.strip_prefix(ROW_NAME))
        .and_then(|rest| rest.strip_suffix('\''));
    if parsed_name != Some(PLUGIN_NAME) {
        return Err(bad(2));
    }
    let mut at = 3usize;
    let mut enabled = true;
    if block.get(at).map(|line| line.trim_end()) == Some(&format!("{ROW_INDENT}{ROW_DISABLED}")[..])
    {
        enabled = false;
        at += 1;
    }
    if block.get(at).map(|line| line.trim_end()) != Some(&format!("{ROW_INDENT}{ROW_CONFIG}")[..]) {
        return Err(bad(at));
    }
    at += 1;

    let mut transport = None;
    let mut server = None;
    let mut command = None;
    let mut cwd = None;
    let mut url = None;
    let mut timeout = None;
    let mut fail = None;
    let mut args: Option<Vec<String>> = None;
    let mut env: Option<Vec<(String, Secret)>> = None;
    let mut headers: Option<Vec<(String, Secret)>> = None;
    while at < block.len() {
        let line = block[at].trim_end();
        at += 1;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(field) = line.strip_prefix(CONFIG_INDENT) else {
            return Err(Foreign::at(base + at, line));
        };
        if field.starts_with(' ') || field.starts_with("- ") {
            return Err(Foreign::at(base + at, line));
        }
        let Some((key, value)) = split(field) else {
            return Err(Foreign::at(base + at, line));
        };
        match key {
            "transport" => {
                if transport.is_some() {
                    return Err(Foreign::at(base + at, line));
                }
                transport = Some(match value {
                    "stdio" => Transport::Stdio,
                    "streamable-http" => Transport::StreamableHttp,
                    _ => return Err(Foreign::at(base + at, line)),
                });
            }
            "serverName" | "command" | "cwd" | "url" => {
                let target = match key {
                    "serverName" => &mut server,
                    "command" => &mut command,
                    "cwd" => &mut cwd,
                    _ => &mut url,
                };
                if target.is_some() {
                    return Err(Foreign::at(base + at, line));
                }
                *target = Some(scalar(value).ok_or_else(|| Foreign::at(base + at, line))?);
            }
            "toolCallTimeoutMs" => {
                if timeout.is_some() {
                    return Err(Foreign::at(base + at, line));
                }
                timeout = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| Foreign::at(base + at, line))?,
                );
            }
            "failOnStartupError" => {
                if fail.is_some() {
                    return Err(Foreign::at(base + at, line));
                }
                fail = Some(match value {
                    "true" => true,
                    "false" => false,
                    _ => return Err(Foreign::at(base + at, line)),
                });
            }
            "args" => {
                if args.is_some() {
                    return Err(Foreign::at(base + at, line));
                }
                let mut list = Vec::new();
                if value != "[]" {
                    if !value.is_empty() {
                        return Err(Foreign::at(base + at, line));
                    }
                    while at < block.len() {
                        let item = block[at].trim_end();
                        if item.is_empty() {
                            at += 1;
                            continue;
                        }
                        let Some(item) = item.strip_prefix(ITEM_INDENT) else {
                            break;
                        };
                        at += 1;
                        let raw = item
                            .strip_prefix("- ")
                            .ok_or_else(|| Foreign::at(base + at, item))?;
                        list.push(scalar(raw).ok_or_else(|| Foreign::at(base + at, item))?);
                    }
                }
                args = Some(list);
            }
            "env" | "headers" => {
                let environment = key == "env";
                let target = if environment { &mut env } else { &mut headers };
                if target.is_some() {
                    return Err(Foreign::at(base + at, line));
                }
                let mut list = Vec::new();
                if value != "{}" {
                    if !value.is_empty() {
                        return Err(Foreign::at(base + at, line));
                    }
                    while at < block.len() {
                        let item = block[at].trim_end();
                        if item.is_empty() {
                            at += 1;
                            continue;
                        }
                        let Some(item) = item.strip_prefix(ITEM_INDENT) else {
                            break;
                        };
                        at += 1;
                        let (raw_key, raw_value) =
                            split(item).ok_or_else(|| Foreign::at(base + at, item))?;
                        list.push((
                            scalar(raw_key).ok_or_else(|| Foreign::at(base + at, item))?,
                            Secret::new(
                                scalar(raw_value).ok_or_else(|| Foreign::at(base + at, item))?,
                            ),
                        ));
                    }
                }
                *target = Some(list);
            }
            _ => return Err(Foreign::at(base + at, line)),
        }
    }
    let entry = match transport {
        Some(Transport::Stdio) => ServerEntry {
            server_name: server.ok_or_else(|| bad(1))?,
            transport: Transport::Stdio,
            enabled,
            command: command.ok_or_else(|| bad(1))?,
            args: args.ok_or_else(|| bad(1))?,
            env: env.ok_or_else(|| bad(1))?,
            cwd: cwd.ok_or_else(|| bad(1))?,
            url: String::new(),
            headers: Vec::new(),
            tool_call_timeout_ms: timeout.ok_or_else(|| bad(1))?,
            fail_on_startup_error: fail.ok_or_else(|| bad(1))?,
        },
        Some(Transport::StreamableHttp) => ServerEntry {
            server_name: server.ok_or_else(|| bad(1))?,
            transport: Transport::StreamableHttp,
            enabled,
            command: String::new(),
            args: Vec::new(),
            env: Vec::new(),
            cwd: String::new(),
            url: url.ok_or_else(|| bad(1))?,
            headers: headers.ok_or_else(|| bad(1))?,
            tool_call_timeout_ms: timeout.ok_or_else(|| bad(1))?,
            fail_on_startup_error: fail.ok_or_else(|| bad(1))?,
        },
        None => return Err(bad(1)),
    };
    if entry.server_name != server_name {
        return Err(bad(1));
    }
    Ok(entry)
}

/// Resolve the private native home as Core does: canonicalize the deepest
/// existing ancestor, then require a private directory.
pub fn resolve_home(native_home: &Path) -> Result<PathBuf, &'static str> {
    if !native_home.is_absolute()
        || native_home
            .to_str()
            .is_none_or(|s| s.bytes().any(|b| b < 32 || b == 127))
        || native_home
            .components()
            .any(|c| matches!(c, Component::ParentDir))
    {
        return Err("The native home path is not an absolute path without '..'.");
    }
    let resolved =
        future_path(native_home).map_err(|_| "The native home could not be resolved.")?;
    let metadata =
        std::fs::symlink_metadata(&resolved).map_err(|_| "The native home does not exist yet.")?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("The native home is not a directory.");
    }
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err("The native home is not private (mode 0700 is required).");
    }
    Ok(resolved)
}

fn future_path(path: &Path) -> std::io::Result<PathBuf> {
    match std::fs::canonicalize(path) {
        Ok(resolved) => Ok(resolved),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))?;
            let name = path
                .file_name()
                .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))?;
            Ok(future_path(parent)?.join(name))
        }
        Err(error) => Err(error),
    }
}

/// Why a write changed nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The private native home could not be used safely.
    Home,
    /// The existing file carries content this app cannot preserve.
    Foreign,
    /// The submitted set failed validation.
    Invalid,
    /// The app-owned file is not private (mode 0600 is required).
    Permissions,
    /// Preparation failed before the target could be replaced.
    Filesystem,
}

impl Refusal {
    pub fn text(self) -> &'static str {
        match self {
            Self::Home => "The private native home is unavailable, so nothing was written.",
            Self::Foreign => {
                "The app-owned patch file content is not recognised, so nothing was written."
            }
            Self::Invalid => "The submitted MCP servers failed validation, so nothing was written.",
            Self::Permissions => {
                "The app-owned patch file is not private (mode 0600), so nothing was written."
            }
            Self::Filesystem => {
                "The app-owned patch file could not be prepared safely, so nothing was written."
            }
        }
    }
}

/// The result of one commit attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteOutcome {
    /// The file was replaced; `revision` describes the new bytes.
    Written {
        revision: Revision,
        backup: Option<String>,
        bytes: usize,
    },
    /// Nothing was written; the reason is a fixed sentence.
    Refused(Refusal),
    /// The file changed since the panel read it, or appeared after an absent read.
    Conflict,
    /// The replacement may have happened; never retried automatically.
    Indeterminate,
    /// The worker, queue or lifecycle refused before any file operation.
    NotSent,
}

/// One loaded panel state: what the file holds and what the Host currently mounts.
#[derive(Clone, Debug, PartialEq)]
pub struct Loaded {
    /// Absolute path the worker resolved and read; empty when the home failed.
    pub path: String,
    /// `None` means the file did not exist when it was read.
    pub revision: Option<Revision>,
    pub entries: Vec<ServerEntry>,
    /// Set when existing content cannot be preserved; mutations are refused.
    pub foreign: Option<String>,
    /// Set when the read itself could not run (the worker refused it, or the private home could
    /// not be opened). Distinct from `foreign`: nothing was inspected, so no file can be blamed.
    pub read_error: Option<String>,
    /// Live `@deepseek-ai/dsh-mcp-client` rows from the plugin inventory.
    pub inventory: Vec<InventoryRow>,
    /// Whether the inventory read succeeded; `false` is not "no rows".
    pub inventory_read: bool,
}

/// The Host's own name for one mount phase.
pub fn phase_label(phase: FiberPhase) -> &'static str {
    match phase {
        FiberPhase::Pending => "pending",
        FiberPhase::Loading => "loading",
        FiberPhase::Active => "active",
        FiberPhase::Failed => "failed",
        FiberPhase::Unloading => "unloading",
        FiberPhase::Absent => "no phase reported",
    }
}

/// One live MCP row the Host reports. Mount state only, never configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventoryRow {
    pub entry_id: String,
    pub enabled: bool,
    pub phase: FiberPhase,
}

impl Loaded {
    /// The Host's mount row for one managed entry, when the inventory read ran.
    pub fn mount(&self, entry: &ServerEntry) -> Option<&InventoryRow> {
        let id = entry.row_id();
        self.inventory.iter().find(|row| row.entry_id == id)
    }
}

impl Loaded {
    /// A read the worker refused before it ran. Nothing was inspected, so the
    /// panel may not offer a mutation against it.
    pub fn refused(reason: &str) -> Self {
        Self {
            path: String::new(),
            revision: None,
            entries: Vec::new(),
            foreign: None,
            read_error: Some(reason.to_owned()),
            inventory: Vec::new(),
            inventory_read: false,
        }
    }
}

/// The live `@deepseek-ai/dsh-mcp-client` rows of one Host plugin inventory.
/// Every other row is ignored: the panel never claims to manage what it did not
/// insert, and it reports the Host's own mount phase for the rows it did.
pub fn inventory(entries: &[dsh_native_transport::plugin::PluginEntry]) -> Vec<InventoryRow> {
    entries
        .iter()
        .filter(|entry| entry.module_name == PLUGIN_NAME)
        .map(|entry| InventoryRow {
            entry_id: entry.entry_id.clone(),
            enabled: entry.enabled,
            phase: entry.fiber_phase,
        })
        .collect()
}

/// Read the app-owned file and project the live MCP rows of the plugin inventory.
pub fn load(native_home: &Path, inventory: Vec<InventoryRow>, inventory_read: bool) -> Loaded {
    let home = match resolve_home(native_home) {
        Ok(home) => home,
        Err(reason) => {
            return Loaded {
                path: String::new(),
                revision: None,
                entries: Vec::new(),
                foreign: None,
                read_error: Some(reason.to_owned()),
                inventory,
                inventory_read,
            };
        }
    };
    let path = home.join(FILE_NAME);
    let displayed = path.to_string_lossy().into_owned();
    let dir = match open(
        &home,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    ) {
        Ok(fd) => File::from(fd),
        Err(_) => {
            return Loaded {
                path: displayed,
                revision: None,
                entries: Vec::new(),
                foreign: None,
                read_error: Some("The private native home could not be opened.".to_owned()),
                inventory,
                inventory_read,
            };
        }
    };
    let (revision, entries, foreign) = match read_prior(&dir) {
        Err(refusal) => (None, Vec::new(), Some(refusal.text().to_owned())),
        Ok(None) => (None, Vec::new(), None),
        Ok(Some(bytes)) => {
            let revision = Revision::of(&bytes);
            let parsed = std::str::from_utf8(&bytes)
                .map_err(|_| "The app-owned patch file is not valid UTF-8.".to_owned())
                .and_then(|text| parse(text).map_err(|foreign| foreign.reason()));
            match parsed {
                Ok(entries) => (Some(revision), entries, None),
                Err(reason) => (Some(revision), Vec::new(), Some(reason)),
            }
        }
    };
    Loaded {
        path: displayed,
        revision,
        entries,
        foreign,
        read_error: None,
        inventory,
        inventory_read,
    }
}

fn read_prior(dir: &File) -> Result<Option<Vec<u8>>, Refusal> {
    let stat = match statat(dir, FILE_NAME, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => stat,
        Err(Errno::NOENT) => return Ok(None),
        Err(_) => return Err(Refusal::Home),
    };
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
        return Err(Refusal::Home);
    }
    if stat.st_mode & 0o077 != 0 {
        return Err(Refusal::Permissions);
    }
    if stat.st_size < 0 || stat.st_size as usize > MAX_FILE_BYTES {
        return Err(Refusal::Foreign);
    }
    let fd = openat(
        dir,
        FILE_NAME,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|_| Refusal::Filesystem)?;
    let file = File::from(fd);
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Refusal::Filesystem)?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(Refusal::Foreign);
    }
    Ok(Some(bytes))
}

/// Replace the app-owned file atomically. `expected` is the revision the panel
/// loaded; `None` requires the file to still be absent.
pub fn commit(
    native_home: &Path,
    expected: Option<Revision>,
    entries: &[ServerEntry],
) -> WriteOutcome {
    if validate(entries).is_err() {
        return WriteOutcome::Refused(Refusal::Invalid);
    }
    let home = match resolve_home(native_home) {
        Ok(home) => home,
        Err(_) => return WriteOutcome::Refused(Refusal::Home),
    };
    let fd = match open(
        &home,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(_) => return WriteOutcome::Refused(Refusal::Home),
    };
    let dir = File::from(fd);
    let metadata = match dir.metadata() {
        Ok(metadata) => metadata,
        Err(_) => return WriteOutcome::Refused(Refusal::Home),
    };
    if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
        return WriteOutcome::Refused(Refusal::Home);
    }
    let prior = match read_prior(&dir) {
        Ok(prior) => prior,
        Err(refusal) => return WriteOutcome::Refused(refusal),
    };
    match (expected, prior.as_ref().map(|bytes| Revision::of(bytes))) {
        (None, None) => {}
        (Some(want), Some(now)) if want == now => {}
        _ => return WriteOutcome::Conflict,
    }
    if let Some(bytes) = prior.as_ref() {
        match std::str::from_utf8(bytes) {
            Ok(text) if parse(text).is_ok() => {}
            _ => return WriteOutcome::Refused(Refusal::Foreign),
        }
    }

    let text = render(entries);
    match parse(&text) {
        Ok(back) if back == entries => {}
        _ => return WriteOutcome::Refused(Refusal::Filesystem),
    }
    let backup = match prior.as_ref() {
        Some(bytes) => match write_backup(&dir, bytes) {
            Ok(name) => Some(name),
            Err(refusal) => return WriteOutcome::Refused(refusal),
        },
        None => None,
    };
    if let Err(refusal) = write_temp(&dir, text.as_bytes()) {
        return WriteOutcome::Refused(refusal);
    }
    if renameat(&dir, TEMP_NAME, &dir, FILE_NAME).is_err() {
        return WriteOutcome::Indeterminate;
    }
    if rustix::fs::fsync(&dir).is_err() {
        return WriteOutcome::Indeterminate;
    }
    WriteOutcome::Written {
        revision: Revision::of(text.as_bytes()),
        backup,
        bytes: text.len(),
    }
}

/// Create one new private file in the app-owned directory and sync it. `EXCL`
/// refuses an existing name instead of reusing it, `NOFOLLOW` refuses a
/// substituted symlink, and mode 0600 keeps the content private from creation;
/// the umask cannot widen it. The caller names the file.
fn create_private(dir: &File, name: &str, bytes: &[u8]) -> Result<(), Refusal> {
    let fd = openat(
        dir,
        name,
        OFlags::WRONLY
            | OFlags::CREATE
            | OFlags::EXCL
            | OFlags::NOFOLLOW
            | OFlags::CLOEXEC
            | OFlags::NONBLOCK,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|_| Refusal::Filesystem)?;
    let mut file = File::from(fd);
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| Refusal::Filesystem)
}

/// The prior document, kept beside the replacement under a timestamped name. A
/// reused name is impossible (millisecond stamp plus `EXCL`); an unwritable
/// backup refuses the whole write rather than replacing the file unprotected.
fn write_backup(dir: &File, previous: &[u8]) -> Result<String, Refusal> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or(0);
    let name = format!("{FILE_NAME}.backup-{millis}");
    create_private(dir, &name, previous)?;
    Ok(name)
}

/// The replacement is staged beside the target and renamed over it. A leftover
/// temp from an interrupted write is replaced only when it is a regular file:
/// anything else there refuses, because this app did not leave it.
fn write_temp(dir: &File, text: &[u8]) -> Result<(), Refusal> {
    if let Ok(stat) = statat(dir, TEMP_NAME, AtFlags::SYMLINK_NOFOLLOW) {
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
            return Err(Refusal::Filesystem);
        }
        unlinkat(dir, TEMP_NAME, AtFlags::empty()).map_err(|_| Refusal::Filesystem)?;
    }
    create_private(dir, TEMP_NAME, text)
}

// ---- Panel state machine ----

/// One panel read identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadTicket {
    pub epoch: u64,
    pub panel: u64,
    pub serial: u64,
}

/// One open draft or review identity. `draft` names the draft, `editor` its
/// structure, `revision` its content, so a stale text input or a replayed
/// confirmation cannot address a newer document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub epoch: u64,
    pub panel: u64,
    pub draft: u64,
    pub editor: u64,
    pub revision: u64,
}

/// One admitted write, fenced to the panel and the loaded document revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WriteTicket {
    pub epoch: u64,
    pub panel: u64,
    pub serial: u64,
    /// The revision the panel loaded; `None` requires the file to be absent.
    pub expected: Option<Revision>,
}

/// Which operator action one reviewed write performs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Add,
    Edit { index: usize },
    Disable { index: usize },
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Add => "Add server",
            Self::Edit { .. } => "Replace configuration",
            Self::Disable { .. } => "Disable row",
        }
    }
    pub fn text(self) -> &'static str {
        match self {
            Self::Add => "Insert one new @deepseek-ai/dsh-mcp-client row.",
            Self::Edit { .. } => {
                "Replace this row's config wholesale. Every field is restated; the Host never deep-merges a config."
            }
            Self::Disable { .. } => {
                "Set disabled: true on this row. The entry and its configuration stay in the file; nothing is deleted."
            }
        }
    }
    /// The reviewed row inside the reviewed document.
    pub fn row(self, entries: &[ServerEntry]) -> Option<&ServerEntry> {
        match self {
            Self::Add => entries.last(),
            Self::Edit { index } | Self::Disable { index } => entries.get(index),
        }
    }
}

/// One reviewed, submitted write.
#[derive(Clone, Debug, PartialEq)]
pub struct Submission {
    pub ticket: WriteTicket,
    pub kind: Kind,
    /// The complete new document: every managed entry, in order.
    pub entries: Vec<ServerEntry>,
}

/// Pure panel context, assembled by the app.
#[derive(Clone, Copy)]
pub struct Context {
    pub epoch: u64,
    pub visible: bool,
    pub enabled: bool,
}

/// One single-line non-secret draft field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    ServerName,
    Command,
    Cwd,
    Url,
    Timeout,
}

/// Which binding list a row belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingKind {
    Environment,
    Header,
}

/// One panel action.
#[derive(Clone, Debug)]
pub enum Action {
    /// Open from the More menu. The app enters the panel and starts the read;
    /// the controller itself never opens from a closed state.
    Open,
    Dismiss(ReadTicket),
    Refresh(ReadTicket),
    BeginAdd(ReadTicket),
    BeginEdit {
        ticket: ReadTicket,
        index: usize,
    },
    BeginDisable {
        ticket: ReadTicket,
        index: usize,
    },
    SetTransport {
        token: Token,
        transport: Transport,
    },
    SetEnabled {
        token: Token,
        enabled: bool,
    },
    SetFailOnStartup {
        token: Token,
        value: bool,
    },
    Field {
        token: Token,
        field: Field,
        value: String,
    },
    AddArgument(Token),
    SetArgument {
        token: Token,
        index: usize,
        value: String,
    },
    RemoveArgument {
        token: Token,
        index: usize,
    },
    AddBinding {
        token: Token,
        binding: BindingKind,
    },
    SetBindingKey {
        token: Token,
        binding: BindingKind,
        index: usize,
        value: String,
    },
    /// Turn one loaded binding row into a typed replacement without showing it.
    ReplaceBindingValue {
        token: Token,
        binding: BindingKind,
        index: usize,
    },
    SetBindingSecret {
        token: Token,
        binding: BindingKind,
        index: usize,
        value: Secret,
    },
    RemoveBinding {
        token: Token,
        binding: BindingKind,
        index: usize,
    },
    Review(Token),
    CancelReview(Token),
    Confirm(Token),
    Cancel(Token),
}

/// One effect the app turns into a worker command.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    Read(ReadTicket),
    Write(Submission),
}

/// What the panel says after the last event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notice {
    Idle,
    Loading,
    Loaded,
    ReadFailed,
    /// The read never ran (worker refusal or an unopenable home); no file was inspected.
    ReadRefused,
    Foreign,
    Invalid,
    Reviewing,
    Pending,
    Confirmed,
    Conflict,
    Refused,
    Indeterminate,
    NotSent,
    Earlier,
    Exhausted,
}

impl Notice {
    pub fn text(self) -> &'static str {
        match self {
            Self::Idle => {
                "Opening this panel alone writes nothing and reads local state only. Refresh explicitly to read the app-owned patch file and the live Host inventory."
            }
            Self::Loading => "Reading the app-owned patch file and the live Host inventory…",
            Self::Loaded => {
                "Loaded. Mount state comes from the Host's plugin inventory and reflects the Host session that is running now."
            }
            Self::ReadFailed => {
                "The app-owned patch file or the Host inventory could not be read. Refresh explicitly; no automatic retry."
            }
            Self::ReadRefused => {
                "The MCP configuration could not be read in this run; nothing was inspected and nothing was changed. Refresh explicitly."
            }
            Self::Foreign => {
                "The app-owned patch file holds content this app cannot preserve, so every mutation is refused and nothing was changed. An operator must move or remove that content."
            }
            Self::Invalid => "Invalid or unavailable entry. No write was sent.",
            Self::Reviewing => {
                "Review the exact write below. Confirm is the only control that writes; Cancel changes nothing."
            }
            Self::Pending => {
                "Write submitted; awaiting the actual outcome. Closing the panel cannot undo it."
            }
            Self::Confirmed => {
                "The app-owned patch file was replaced and the previous file was backed up."
            }
            Self::Conflict => {
                "The app-owned patch file changed since this panel read it. Nothing was written. Refresh explicitly; no automatic retry."
            }
            Self::Refused => {
                "The write was refused before the file could be replaced. Nothing was written; refresh explicitly."
            }
            Self::Indeterminate => {
                "Write outcome indeterminate: the file may already have been replaced. Never retry automatically; refresh explicitly to see the file as it is now."
            }
            Self::NotSent => {
                "The worker, queue or lifecycle refused the write before any file operation. Nothing was written."
            }
            Self::Earlier => {
                "An earlier panel's write returned. It does not apply to this panel. Refresh explicitly; no automatic retry."
            }
            Self::Exhausted => "Native MCP panel authority exhausted. Restart the app.",
        }
    }
}

/// The last confirmed write, shown with the file path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub path: String,
    pub backup: Option<String>,
    pub bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    List,
    Form,
    Review,
}

/// Why a review did or did not begin. Only counter exhaustion disables the panel.
enum ReviewStart {
    Started,
    Invalid,
    Exhausted,
}

struct Binding {
    key: String,
    /// `Unchanged` restates the loaded bytes without ever showing them.
    value: Value,
}

enum Value {
    Unchanged(Secret),
    Set(Secret),
}

impl Value {
    fn secret(&self) -> &Secret {
        match self {
            Self::Unchanged(secret) | Self::Set(secret) => secret,
        }
    }
    fn label(&self) -> String {
        match self {
            Self::Unchanged(secret) => format!("unchanged · {} bytes", secret.len()),
            Self::Set(secret) => format!("replace · {} bytes", secret.len()),
        }
    }
    fn replaced(&self) -> bool {
        matches!(self, Self::Set(_))
    }
}

struct Draft {
    kind: Kind,
    transport: Transport,
    enabled: bool,
    server_name: String,
    command: String,
    cwd: String,
    url: String,
    timeout: String,
    fail_on_startup_error: bool,
    args: Vec<String>,
    env: Vec<Binding>,
    headers: Vec<Binding>,
}

impl Draft {
    fn add() -> Self {
        Self {
            kind: Kind::Add,
            transport: Transport::Stdio,
            enabled: true,
            server_name: String::new(),
            command: String::new(),
            cwd: String::new(),
            url: String::new(),
            timeout: DEFAULT_TIMEOUT_MS.to_string(),
            fail_on_startup_error: false,
            args: Vec::new(),
            env: Vec::new(),
            headers: Vec::new(),
        }
    }
    fn edit(index: usize, entry: &ServerEntry) -> Self {
        let bindings = |source: &[(String, Secret)]| {
            source
                .iter()
                .map(|(key, value)| Binding {
                    key: key.clone(),
                    value: Value::Unchanged(value.clone()),
                })
                .collect()
        };
        Self {
            kind: Kind::Edit { index },
            transport: entry.transport,
            enabled: entry.enabled,
            server_name: entry.server_name.clone(),
            command: entry.command.clone(),
            cwd: entry.cwd.clone(),
            url: entry.url.clone(),
            timeout: entry.tool_call_timeout_ms.to_string(),
            fail_on_startup_error: entry.fail_on_startup_error,
            args: entry.args.clone(),
            env: bindings(&entry.env),
            headers: bindings(&entry.headers),
        }
    }
    /// The reviewed row. Only the selected transport's fields are carried, so a
    /// config is never restated with the other union member's keys.
    fn entry(&self) -> ServerEntry {
        // An unparsable or out-of-range timeout becomes 0, which `validate`
        // refuses, instead of silently substituting a default.
        let timeout = self.timeout.parse::<u64>().unwrap_or(0);
        let bindings = |source: &[Binding]| {
            source
                .iter()
                .map(|binding| (binding.key.clone(), binding.value.secret().clone()))
                .collect()
        };
        match self.transport {
            Transport::Stdio => ServerEntry {
                server_name: self.server_name.clone(),
                transport: Transport::Stdio,
                enabled: self.enabled,
                command: self.command.clone(),
                args: self.args.clone(),
                env: bindings(&self.env),
                cwd: self.cwd.clone(),
                url: String::new(),
                headers: Vec::new(),
                tool_call_timeout_ms: timeout,
                fail_on_startup_error: self.fail_on_startup_error,
            },
            Transport::StreamableHttp => ServerEntry {
                server_name: self.server_name.clone(),
                transport: Transport::StreamableHttp,
                enabled: self.enabled,
                command: String::new(),
                args: Vec::new(),
                env: Vec::new(),
                cwd: String::new(),
                url: self.url.clone(),
                headers: bindings(&self.headers),
                tool_call_timeout_ms: timeout,
                fail_on_startup_error: self.fail_on_startup_error,
            },
        }
    }
    fn bindings(&self, kind: BindingKind) -> &[Binding] {
        match kind {
            BindingKind::Environment => &self.env,
            BindingKind::Header => &self.headers,
        }
    }
    fn bindings_mut(&mut self, kind: BindingKind) -> &mut Vec<Binding> {
        match kind {
            BindingKind::Environment => &mut self.env,
            BindingKind::Header => &mut self.headers,
        }
    }
}

struct Review {
    token: Token,
    kind: Kind,
    entries: Vec<ServerEntry>,
}

/// The panel's whole state. No state is predicted before a receipt.
pub struct Mcp {
    open: bool,
    epoch: u64,
    panel: u64,
    read_serial: u64,
    draft_serial: u64,
    editor: u64,
    revision: u64,
    configured: usize,
    restart_required: bool,
    exhausted: bool,
    loaded: Option<Loaded>,
    reading: Option<ReadTicket>,
    mode: Stage,
    draft: Option<Draft>,
    review: Option<Review>,
    pending: Option<WriteTicket>,
    notice: Notice,
    defect: Option<Defect>,
    report: Option<Report>,
}

impl Default for Mcp {
    fn default() -> Self {
        Self {
            open: false,
            epoch: 0,
            panel: 0,
            read_serial: 0,
            draft_serial: 0,
            editor: 0,
            revision: 0,
            configured: 0,
            restart_required: false,
            exhausted: false,
            loaded: None,
            reading: None,
            mode: Stage::List,
            draft: None,
            review: None,
            pending: None,
            notice: Notice::Idle,
            defect: None,
            report: None,
        }
    }
}

fn bump(counter: &mut u64) -> bool {
    match counter.checked_add(1) {
        Some(next) => {
            *counter = next;
            true
        }
        None => false,
    }
}

impl Mcp {
    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn loaded(&self) -> Option<&Loaded> {
        self.loaded.as_ref()
    }
    pub fn entries(&self) -> &[ServerEntry] {
        self.loaded.as_ref().map_or(&[], |loaded| &loaded.entries)
    }
    pub fn mode(&self) -> &'static str {
        match self.mode {
            Stage::List => "list",
            Stage::Form => "form",
            Stage::Review => "review",
        }
    }
    pub fn draft(&self) -> Option<DraftView<'_>> {
        self.draft.as_ref().map(|draft| DraftView { draft })
    }
    pub fn review_kind(&self) -> Option<Kind> {
        self.review.as_ref().map(|review| review.kind)
    }
    pub fn review_entries(&self) -> Option<&[ServerEntry]> {
        self.review.as_ref().map(|review| review.entries.as_slice())
    }
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }
    pub fn report(&self) -> Option<&Report> {
        self.report.as_ref()
    }
    pub fn notice(&self) -> Notice {
        self.notice
    }
    /// The validation defect that refused the last review, when there was one.
    pub fn defect(&self) -> Option<Defect> {
        self.defect
    }
    pub fn read_ticket(&self) -> ReadTicket {
        ReadTicket {
            epoch: self.epoch,
            panel: self.panel,
            serial: self.read_serial,
        }
    }
    pub fn token(&self) -> Token {
        Token {
            epoch: self.epoch,
            panel: self.panel,
            draft: self.draft_serial,
            editor: self.editor,
            revision: self.revision,
        }
    }
    /// The sentence the panel shows for content it cannot preserve.
    pub fn foreign_reason(&self) -> Option<&str> {
        self.loaded.as_ref()?.foreign.as_deref()
    }
    /// True when the panel may currently accept a mutation.
    pub fn writable(&self) -> bool {
        self.loaded
            .as_ref()
            .is_some_and(|loaded| loaded.foreign.is_none() && loaded.read_error.is_none())
            && !self.pending()
            && !self.exhausted
    }
    /// The number of managed servers the last successful read or write reported. Survives the
    /// panel closing, so the More-menu entry does not claim zero servers once it is populated.
    pub fn configured(&self) -> usize {
        self.configured
    }
    /// A write replaced the composition file and the running Host has not re-read it.
    pub fn restart_required(&self) -> bool {
        self.restart_required
    }
    /// Whether a successful read found no managed server. Loading, a refused read and a foreign
    /// file are not "empty": each has its own notice, and none may carry the empty-list sentence.
    pub fn config_empty(&self) -> bool {
        self.loaded.as_ref().is_some_and(|loaded| {
            loaded.read_error.is_none() && loaded.foreign.is_none() && loaded.entries.is_empty()
        })
    }
    fn allowed(&self, context: Context) -> bool {
        self.open
            && context.visible
            && context.enabled
            && context.epoch == self.epoch
            && self.epoch != 0
            && !self.exhausted
    }
    fn exhaust(&mut self) {
        self.exhausted = true;
        self.draft = None;
        self.review = None;
        self.mode = Stage::List;
        self.notice = Notice::Exhausted;
    }
    fn bump_editor(&mut self) -> bool {
        self.review = None;
        self.revision = self.revision.saturating_add(1);
        bump(&mut self.editor)
    }
    fn bump_revision(&mut self) -> bool {
        self.review = None;
        bump(&mut self.revision)
    }
    fn stale(&self, token: Token) -> bool {
        token.epoch != self.epoch
            || token.panel != self.panel
            || token.draft != self.draft_serial
            || token.editor != self.editor
    }
    /// Enter the panel and start its read.
    pub fn enter(&mut self, epoch: u64) -> Effect {
        self.leave();
        self.open = true;
        self.epoch = epoch;
        self.exhausted = false;
        self.refresh()
    }
    /// Hide the panel and retire local drafts. Never cancels an admitted write.
    pub fn dismiss(&mut self) {
        self.leave();
        self.open = false;
    }
    /// Fence late receipts after the transport generation is gone.
    pub fn disconnect(&mut self) {
        let admitted = self.pending.take().is_some();
        self.dismiss();
        self.epoch = 0;
        self.notice = if admitted {
            Notice::Indeterminate
        } else {
            Notice::ReadFailed
        };
    }
    fn leave(&mut self) {
        self.loaded = None;
        self.reading = None;
        self.draft = None;
        self.review = None;
        self.mode = Stage::List;
        if !bump(&mut self.panel)
            || !bump(&mut self.draft_serial)
            || !bump(&mut self.editor)
            || !bump(&mut self.revision)
            || !bump(&mut self.read_serial)
        {
            self.exhaust();
            return;
        }
        self.defect = None;
        self.notice = Notice::Idle;
        // Closing is not cancellation: an admitted write can still complete.
    }
    fn refresh(&mut self) -> Effect {
        self.defect = None;
        if !bump(&mut self.read_serial) {
            self.exhaust();
            return Effect::Read(self.read_ticket());
        }
        let ticket = self.read_ticket();
        self.reading = Some(ticket);
        self.loaded = None;
        self.draft = None;
        self.review = None;
        self.mode = Stage::List;
        self.notice = Notice::Loading;
        Effect::Read(ticket)
    }
    /// A loaded snapshot from the worker. Only the exact current read applies.
    pub fn loaded_snapshot(&mut self, ticket: ReadTicket, loaded: Loaded) {
        if self.reading != Some(ticket) || ticket.epoch != self.epoch {
            return;
        }
        self.reading = None;
        if !bump(&mut self.draft_serial) || !bump(&mut self.editor) || !bump(&mut self.revision) {
            self.exhaust();
            return;
        }
        self.report = None;
        self.notice = if loaded.read_error.is_some() {
            Notice::ReadRefused
        } else if loaded.foreign.is_some() {
            Notice::Foreign
        } else if loaded.inventory_read {
            Notice::Loaded
        } else {
            Notice::ReadFailed
        };
        // The count survives the panel closing (the menu entry draws it), but never a secret.
        if self.notice != Notice::ReadRefused && self.notice != Notice::Foreign {
            self.configured = loaded.entries.len();
        }
        self.loaded = Some(loaded);
    }
    /// A write outcome from the worker. Only the exact admitted ticket settles.
    pub fn written(&mut self, submission: &Submission, outcome: WriteOutcome) {
        if self.pending.as_ref() != Some(&submission.ticket) {
            return;
        }
        self.pending = None;
        self.defect = None;
        if !self.open
            || self.epoch != submission.ticket.epoch
            || self.panel != submission.ticket.panel
        {
            // The panel closed after the write was admitted. A confirmed replacement still
            // happened, so it must be reported where the user can see it: the menu entry keeps the
            // count and says the running Host needs a restart, instead of a notice that claims the
            // write does not apply here.
            if let WriteOutcome::Written { .. } = outcome {
                self.configured = submission.entries.len();
                self.restart_required = true;
            }
            self.notice = Notice::Earlier;
            return;
        }
        match outcome {
            WriteOutcome::Written {
                revision,
                backup,
                bytes,
            } => {
                self.notice = Notice::Confirmed;
                self.configured = submission.entries.len();
                // The Host composed its patch layers at start; this file now describes a
                // composition the running Host has not read.
                self.restart_required = true;
                self.report = self.loaded.as_ref().map(|loaded| Report {
                    path: loaded.path.clone(),
                    backup,
                    bytes,
                });
                // Installed from the acknowledged write only; a later refresh
                // replaces it from the file on disk.
                if let Some(loaded) = self.loaded.as_mut() {
                    loaded.foreign = None;
                    loaded.entries = submission.entries.clone();
                    loaded.revision = Some(revision);
                }
            }
            WriteOutcome::Refused(refusal) => {
                self.report = None;
                self.notice = match refusal {
                    Refusal::Foreign => Notice::Foreign,
                    Refusal::Invalid => Notice::Invalid,
                    _ => Notice::Refused,
                };
            }
            WriteOutcome::Conflict => {
                self.report = None;
                self.notice = Notice::Conflict;
            }
            WriteOutcome::Indeterminate => {
                self.report = None;
                self.notice = Notice::Indeterminate;
            }
            WriteOutcome::NotSent => {
                self.report = None;
                self.notice = Notice::NotSent;
            }
        }
        // No optimistic state and no automatic retry: the panel never resends.
    }

    pub fn apply(&mut self, action: Action, context: Context) -> Option<Effect> {
        if let Action::Dismiss(ticket) = action
            && self.open
            && self.read_ticket() == ticket
        {
            self.dismiss();
            return None;
        }
        if !self.allowed(context) {
            return None;
        }
        match action {
            Action::Dismiss(_) => {}
            Action::Refresh(ticket)
                if ticket == self.read_ticket() && !self.pending() && self.reading.is_none() =>
            {
                self.defect = None;
                return Some(self.refresh());
            }
            Action::BeginAdd(ticket) if ticket == self.read_ticket() && self.writable() => {
                if !bump(&mut self.draft_serial)
                    || !bump(&mut self.editor)
                    || !bump(&mut self.revision)
                {
                    self.exhaust();
                    return None;
                }
                self.draft = Some(Draft::add());
                self.review = None;
                self.mode = Stage::Form;
                self.notice = Notice::Idle;
            }
            Action::BeginEdit { ticket, index }
                if ticket == self.read_ticket() && self.writable() =>
            {
                let Some(entry) = self.entries().get(index) else {
                    self.defect = None;
                    self.notice = Notice::Invalid;
                    return None;
                };
                let draft = Draft::edit(index, entry);
                if !bump(&mut self.draft_serial)
                    || !bump(&mut self.editor)
                    || !bump(&mut self.revision)
                {
                    self.exhaust();
                    return None;
                }
                self.draft = Some(draft);
                self.review = None;
                self.mode = Stage::Form;
                self.notice = Notice::Idle;
            }
            Action::BeginDisable { ticket, index }
                if ticket == self.read_ticket() && self.writable() =>
            {
                let mut entries = self.entries().to_vec();
                let Some(entry) = entries.get_mut(index) else {
                    self.notice = Notice::Invalid;
                    return None;
                };
                if !entry.enabled {
                    self.defect = None;
                    self.notice = Notice::Invalid;
                    return None;
                }
                entry.enabled = false;
                if matches!(
                    self.begin_review(Kind::Disable { index }, entries),
                    ReviewStart::Exhausted
                ) {
                    self.exhaust();
                }
            }
            Action::SetTransport { token, transport }
                if !self.stale(token) && self.writable() && self.draft.is_some() =>
            {
                if !self.bump_editor() {
                    self.exhaust();
                    return None;
                }
                if let Some(draft) = self.draft.as_mut()
                    && draft.transport != transport
                {
                    // The config union replaces wholesale: switching transport
                    // starts from that transport's own defaults.
                    let mut fresh = Draft::add();
                    fresh.kind = draft.kind;
                    fresh.enabled = draft.enabled;
                    fresh.transport = transport;
                    fresh.server_name = draft.server_name.clone();
                    fresh.timeout = draft.timeout.clone();
                    fresh.fail_on_startup_error = draft.fail_on_startup_error;
                    *draft = fresh;
                }
                self.notice = Notice::Idle;
            }
            Action::SetEnabled { token, enabled }
                if !self.stale(token) && self.writable() && self.draft.is_some() =>
            {
                if self.bump_editor()
                    && let Some(draft) = self.draft.as_mut()
                {
                    draft.enabled = enabled;
                }
            }
            Action::SetFailOnStartup { token, value }
                if !self.stale(token) && self.writable() && self.draft.is_some() =>
            {
                if self.bump_editor()
                    && let Some(draft) = self.draft.as_mut()
                {
                    draft.fail_on_startup_error = value;
                }
            }
            Action::Field {
                token,
                field,
                value,
            } if !self.stale(token) && self.writable() => {
                if value.len() > MAX_TEXT || !scalar_safe(&value) {
                    self.defect = None;
                    self.notice = Notice::Invalid;
                    return None;
                }
                let draft = self.draft.as_mut()?;
                match field {
                    Field::ServerName => draft.server_name = value,
                    Field::Command => draft.command = value,
                    Field::Cwd => draft.cwd = value,
                    Field::Url => draft.url = value,
                    Field::Timeout => draft.timeout = value,
                }
                if !self.bump_revision() {
                    self.exhaust();
                }
            }
            Action::AddArgument(token)
                if !self.stale(token) && self.writable() && self.argument_room() =>
            {
                if !self.bump_editor() {
                    self.exhaust();
                    return None;
                }
                if let Some(draft) = self.draft.as_mut() {
                    draft.args.push(String::new());
                }
            }
            Action::SetArgument {
                token,
                index,
                value,
            } if !self.stale(token) && self.writable() => {
                if value.len() > MAX_TEXT || !scalar_safe(&value) {
                    self.defect = None;
                    self.notice = Notice::Invalid;
                    return None;
                }
                let draft = self.draft.as_mut()?;
                let arg = draft.args.get_mut(index)?;
                *arg = value;
                if !self.bump_revision() {
                    self.exhaust();
                }
            }
            Action::RemoveArgument { token, index } if !self.stale(token) && self.writable() => {
                if !self.bump_editor() {
                    self.exhaust();
                    return None;
                }
                if let Some(draft) = self.draft.as_mut()
                    && index < draft.args.len()
                {
                    draft.args.remove(index);
                }
            }
            Action::AddBinding { token, binding }
                if !self.stale(token) && self.writable() && self.binding_room(binding) =>
            {
                if !self.bump_editor() {
                    self.exhaust();
                    return None;
                }
                if let Some(draft) = self.draft.as_mut() {
                    draft.bindings_mut(binding).push(Binding {
                        key: String::new(),
                        value: Value::Set(Secret::new(String::new())),
                    });
                }
            }
            Action::SetBindingKey {
                token,
                binding,
                index,
                value,
            } if !self.stale(token) && self.writable() => {
                if value.len() > MAX_KEY || !scalar_safe(&value) {
                    self.defect = None;
                    self.notice = Notice::Invalid;
                    return None;
                }
                let draft = self.draft.as_mut()?;
                let row = draft.bindings_mut(binding).get_mut(index)?;
                row.key = value;
                if !self.bump_revision() {
                    self.exhaust();
                }
            }
            // Replacing a value never reveals the previous one.
            Action::ReplaceBindingValue {
                token,
                binding,
                index,
            } if !self.stale(token) && self.writable() => {
                if !self.bump_editor() {
                    self.exhaust();
                    return None;
                }
                if let Some(draft) = self.draft.as_mut()
                    && let Some(row) = draft.bindings_mut(binding).get_mut(index)
                {
                    row.value = Value::Set(Secret::new(String::new()));
                }
            }
            Action::SetBindingSecret {
                token,
                binding,
                index,
                value,
            } if !self.stale(token) && self.writable() => {
                if value.len() > MAX_TEXT || !scalar_safe(value.expose()) {
                    self.defect = None;
                    self.notice = Notice::Invalid;
                    return None;
                }
                let draft = self.draft.as_mut()?;
                let row = draft.bindings_mut(binding).get_mut(index)?;
                if !row.value.replaced() {
                    return None;
                }
                row.value = Value::Set(value);
                if !self.bump_revision() {
                    self.exhaust();
                }
            }
            Action::RemoveBinding {
                token,
                binding,
                index,
            } if !self.stale(token) && self.writable() => {
                if !self.bump_editor() {
                    self.exhaust();
                    return None;
                }
                if let Some(draft) = self.draft.as_mut() {
                    let rows = draft.bindings_mut(binding);
                    if index < rows.len() {
                        rows.remove(index);
                    }
                }
            }
            Action::Review(token) if token == self.token() && self.writable() => {
                let draft = self.draft.as_ref()?;
                let kind = draft.kind;
                let candidate = draft.entry();
                let mut entries = self.entries().to_vec();
                match kind {
                    Kind::Add => entries.push(candidate),
                    Kind::Edit { index } | Kind::Disable { index } => {
                        let Some(slot) = entries.get_mut(index) else {
                            self.defect = None;
                            self.notice = Notice::Invalid;
                            return None;
                        };
                        *slot = candidate;
                    }
                }
                if matches!(self.begin_review(kind, entries), ReviewStart::Exhausted) {
                    self.exhaust();
                }
            }
            Action::CancelReview(token)
                if self.review.as_ref().is_some_and(|r| r.token == token) =>
            {
                if !bump(&mut self.revision) {
                    self.exhaust();
                    return None;
                }
                self.review = None;
                self.defect = None;
                self.mode = if self.draft.is_some() {
                    Stage::Form
                } else {
                    Stage::List
                };
                self.notice = Notice::Idle;
            }
            Action::Confirm(token)
                if self.review.as_ref().is_some_and(|r| r.token == token) && self.writable() =>
            {
                let Some(expected) = self.loaded.as_ref().map(|loaded| loaded.revision) else {
                    self.defect = None;
                    self.notice = Notice::Invalid;
                    return None;
                };
                let review = self.review.take().expect("matching review");
                if !bump(&mut self.revision) {
                    self.exhaust();
                    return None;
                }
                let submission = Submission {
                    ticket: WriteTicket {
                        epoch: self.epoch,
                        panel: self.panel,
                        serial: review.token.revision,
                        expected,
                    },
                    kind: review.kind,
                    entries: review.entries,
                };
                self.pending = Some(submission.ticket);
                self.mode = Stage::List;
                self.draft = None;
                self.notice = Notice::Pending;
                return Some(Effect::Write(submission));
            }
            Action::Cancel(token) if token == self.token() => {
                if !bump(&mut self.draft_serial)
                    || !bump(&mut self.editor)
                    || !bump(&mut self.revision)
                {
                    self.exhaust();
                    return None;
                }
                self.draft = None;
                self.review = None;
                self.defect = None;
                self.mode = Stage::List;
                self.notice = Notice::Idle;
            }
            _ => {}
        }
        None
    }

    fn begin_review(&mut self, kind: Kind, entries: Vec<ServerEntry>) -> ReviewStart {
        if let Err(defect) = validate(&entries) {
            self.defect = Some(defect);
            self.notice = Notice::Invalid;
            return ReviewStart::Invalid;
        }
        self.defect = None;
        if !bump(&mut self.revision) {
            return ReviewStart::Exhausted;
        }
        self.review = Some(Review {
            token: self.token(),
            kind,
            entries,
        });
        self.mode = Stage::Review;
        self.notice = Notice::Reviewing;
        ReviewStart::Started
    }

    fn argument_room(&self) -> bool {
        self.draft
            .as_ref()
            .is_some_and(|draft| draft.args.len() < MAX_ARGS)
    }
    fn binding_room(&self, kind: BindingKind) -> bool {
        self.draft
            .as_ref()
            .is_some_and(|draft| draft.bindings(kind).len() < MAX_BINDINGS)
    }
}

/// Read-only view of the open draft, for the panel.
pub struct DraftView<'a> {
    draft: &'a Draft,
}
impl DraftView<'_> {
    pub fn kind(&self) -> Kind {
        self.draft.kind
    }
    pub fn transport(&self) -> Transport {
        self.draft.transport
    }
    pub fn enabled(&self) -> bool {
        self.draft.enabled
    }
    pub fn fail_on_startup_error(&self) -> bool {
        self.draft.fail_on_startup_error
    }
    pub fn server_name(&self) -> &str {
        &self.draft.server_name
    }
    pub fn command(&self) -> &str {
        &self.draft.command
    }
    pub fn cwd(&self) -> &str {
        &self.draft.cwd
    }
    pub fn url(&self) -> &str {
        &self.draft.url
    }
    pub fn timeout(&self) -> &str {
        &self.draft.timeout
    }
    pub fn args(&self) -> &[String] {
        &self.draft.args
    }
    /// One binding row: its key and a label that never contains the value.
    pub fn bindings(&self, kind: BindingKind) -> Vec<(&str, String)> {
        self.draft
            .bindings(kind)
            .iter()
            .map(|row| (row.key.as_str(), row.value.label()))
            .collect()
    }
    /// The operator's own replacement text for one binding row, or `None` while
    /// the row still restates the loaded value. A loaded value is never
    /// returned here, so it can never be rendered.
    pub fn binding_input(&self, kind: BindingKind, index: usize) -> Option<&str> {
        match &self.draft.bindings(kind).get(index)?.value {
            Value::Set(secret) => Some(secret.expose()),
            Value::Unchanged(_) => None,
        }
    }
}

/// Wrap operator text typed into a binding field.
pub fn secret(value: String) -> Secret {
    Secret::new(value)
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
