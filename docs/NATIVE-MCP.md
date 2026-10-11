# Native MCP server management

**More → MCP servers** lists, adds, edits and disables the MCP servers this app
composes into its own Host. It is not a new tab: the entry sits in the existing
**More** menu under **APPLICATION**, beside *Work records* and *Terminal*, and
Escape or **Back to conversation** dismisses it like the other auxiliary views.

## What MCP is here

MCP is the Cordis plugin `@deepseek-ai/dsh-mcp-client`. Each plugin instance
connects to one MCP server and publishes that server's tools under
`mcp__<serverName>__<rawName>`. A server is therefore one composition row whose
`config` selects a transport; `toolCallTimeoutMs` (default 60000) bounds one
tool call, `failOnStartupError` decides whether a failed initial connection fails
the row or enters the plugin's reconnect loop, and `serverName` must match
`[A-Za-z0-9_-]{1,32}` and be unique across live instances.

## The two transports and their fields

| Field | `stdio` | `streamable-http` |
| --- | --- | --- |
| target | `command` plus `args[]` (no shell interpolation) | `url`, an absolute `http://`/`https://` endpoint |
| environment | `env{}` merged over the scrubbed ambient environment | `headers{}` attached to MCP requests |
| directory | `cwd`, which may be empty to inherit | — |
| shared | `serverName`, `transport`, `toolCallTimeoutMs`, `failOnStartupError` | same |

Local validation refuses a name outside the plugin's pattern, a duplicate name,
an empty command, a `cwd` that is neither empty nor an existing absolute
directory without `..`, a non-`http(s)` URL (or one with a fragment), an
environment name outside `[A-Za-z_][A-Za-z0-9_]*`, a header name outside the
HTTP token set, values longer than 4096 bytes or containing control characters,
more than 64 arguments, more than 64 bindings, more than 32 servers, and a
timeout outside 1–3600000 ms. A refusal names the rule it broke; nothing is sent.

`env` and header values are treated as secrets. They are parsed so an edit can
restate them byte-for-byte, but they are never rendered: the list and the review
show only the key and a byte count, an unchanged row keeps its loaded value
without a text input, and **Replace value** is an explicit step before the field
accepts typing. Draft values are overwritten in memory when they are dropped.

## Where the composition lives

The Host's patch layers compose in this order (`packages/boot/app-boot`):

1. each bundle in the profile's `dsh.profile.bundles`,
2. the profile's own `profiles/desktop/cordis.patch.yml`,
3. **`<native-home>/cordis.patch.yml`** — the app-owned layer this panel writes,
4. `--patch` overlays, then the telemetry switch.

The native Core exports its private home as `$DSH_HOME`, so the Host already
reads layer 3 on every start: no Core change and no startup argument is
involved. The file is a top-level YAML array of loader patches, so one server is
one entry:

```yaml
- insert:
    - id: native-mcp-alpha
      name: '@deepseek-ai/dsh-mcp-client'
      config:
        transport: stdio
        serverName: "alpha"
        command: "/usr/bin/npx"
        args:
          - "--serve"
        env:
          "API_TOKEN": "…"
        cwd: ""
        toolCallTimeoutMs: 60000
        failOnStartupError: true
```

The app is the only writer of that file. The official ConfigEditor persists
managed-Codex settings into layer 2 instead, and treats an override from a home
patch as a distinct, supported layer, so the two never share a document.

**Add** appends one entry. **Edit** replaces that entry's `config` wholesale —
the Host never deep-merges a config — and restates every field the row needs.
**Remove** sets `disabled: true` on the row and keeps the entry, its arguments
and its bindings; re-enabling is an edit, and nothing another writer put in the
file is deleted.

## Safety rules

- **Private home only.** The resolved home must be an absolute directory with
  mode 0700, opened `O_DIRECTORY | O_NOFOLLOW`; every read, temporary file,
  backup and rename happens relative to that pinned descriptor.
- **Atomic write.** The document is rendered, written to a deterministic
  temporary file in the same directory (`O_CREAT | O_EXCL | O_NOFOLLOW`, mode
  0600), fsynced, renamed over the target, and the directory is fsynced after.
- **Backup first.** When a previous file exists it is copied to
  `cordis.patch.yml.backup-<unix-millis>` (mode 0600) before anything is
  replaced. Backups are never pruned, so a failed or repeated edit can always be
  compared against what was there.
- **Fail closed.** The parser accepts only the canonical form this panel
  produces. Any other row, key, value shape, empty file, comments-only file,
  non-UTF-8 content or oversized file is refused with the offending 1-based line
  and nothing is changed — the app never hand-rolls a YAML rewrite that could
  silently drop an unknown row. The rendered text is re-parsed and compared with
  the entry list it came from before the rename, so the app cannot write a
  document it cannot read back.
- **No optimistic state.** Nothing changes in the panel until the worker's own
  outcome returns. A revision conflict (the file changed since the panel read
  it), a permission or preparation failure, and a queue/lifecycle refusal are
  distinct outcomes; a failure after the rename is reported as **indeterminate**,
  and no outcome is ever retried automatically.
- **One management flight.** An admitted MCP write shares the app's single
  mutation flight with registry changes, rename, fork, terminal create/close and
  session export, so two mutations cannot race. Reads reserve nothing and mutate
  nothing.
- **Explicit consent.** Add, edit and remove each open a review showing the exact
  document that will be written, the resolved file path, and which rows are
  restated or disabled. **Confirm** is the only control that writes; **Cancel**
  and **Cancel review** change nothing.

## What is unqualified

- **No live Host or MCP server has been connected.** Every claim above is
  unit- and App-tested against the real transport types and the real file
  operations; the resolution of `@deepseek-ai/dsh-mcp-client` through the
  profile's runtime interception, the mounting of an inserted row, and a real
  `tools/list` from a real server are **not** verified here.
- **No hot reload.** The Host's config watcher (`@deepseek-ai/dsh-hmr`) watches
  this exact file, but its service constructor requires `--expose-internals`,
  which the native Core does not pass, so the watcher cannot start and the panel
  has no in-app Host restart. A written change therefore takes effect when the
  native Harness app is restarted; the panel says so after every confirmed write.
- **Mount state is advisory.** The inventory reports the Host's own
  `enabled`/`fiberPhase` for `mcp-client` rows, which reflects the session that
  is running now — not the file that was just written.
- **Not covered:** remove only disables (there is no delete), `reconnect`,
  `maxInstructionBytes` and `isolate`/`inject` are left to a hand-edited file,
  the app does not manage rows it did not insert, and no MCP tool call, OAuth
  flow, streamable-HTTP handshake or credential is exercised.
- **Ownership check.** The home is required to exist, be a directory and be mode
  0700; it is opened through a pinned descriptor. The app deliberately does not
  compare the directory's uid, because the mode plus the descriptor already
  decide who can write, and a uid comparison would add no capability.
