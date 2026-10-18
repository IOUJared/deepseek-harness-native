# Native Host terminal panel

**More → Terminal** is a client-only view of the Host's `terminal/*` service. The app never
spawns a shell or child process; the Host owns every PTY and its process range.

## Flow

1. **Open** (More → *Terminal · Host shell*) issues one read: `terminal/list` for the selected
   Session's retained terminals, then best-effort `terminal/shells` and `terminal/environment`
   for the registered Agent. Opening produces no shell, terminal or other Host mutation.
2. **Select** one listed terminal attaches `terminal/follow` with a fresh attachment id and
   exclusive input control (an older attachment becomes read-only). The first frame is a
   complete bounded screen, then ordered output and metadata changes.
3. **New terminal…** reviews a discovered shell profile and the fixed `80×24` request, then
   sends one `terminal/create` for a caller-generated `[\w-]{1,128}` identity.
4. **Close…** reviews one exact terminal, then sends one `terminal/close`.
5. **Write** sends the typed input line to the selected attachment as-is.

Left out of this slice: `terminal/rename`, `terminal/retain` and in-panel `terminal/resize`.
Resize exists in the transport but the panel always requests `80×24`, clamped to the limits the
Host advertised for the Session.

## Authority and consent

- Copy in the panel states plainly that **the shell runs on the Host with the Host's authority**,
  that this app is a client, and that input is interpreted by the Host shell, not normalized here.
- Create, close and write are mutating. Each is an explicit **Review → Confirm** (write is the
  explicit **Write** action) and the app holds no optimistic state:
  - A create adds nothing until the Host ACK returns the **exact requested identity**.
  - A close removes nothing until the Host confirms it.
  - A write clears the typed line only after the Host accepts it.
- Create and close use the same owned-ticket/epoch fencing as rename/fork: epoch, session
  generation, target and panel serial are checked on every admission. They share the app's
  single management flight, so a create/close cannot race a rename, fork, export or registry
  mutation (and those cannot race it).
- No automatic retry. A refusal, timeout or disconnect is surfaced as an indeterminate outcome
  (`CreatingIndeterminate`, `ClosingIndeterminate`, `WriteIndeterminate`) and requires a new
  explicit action.

## Bounds

- Output is sanitized to plain text (CSI/OSC sequences and control bytes removed) and retained
  in the panel only: at most **400 chunks / 128 KiB**, newest kept, and the panel renders only
  the newest **32 KiB**. Every discarded chunk and every byte an oversized chunk loses is
  counted and shown.
- One input line is bounded by the smaller of the Host's `environment.maxInputBytes` and the
  transport's 256 KiB outbound cap; a create's dimensions are `80×24` bounded by the host
  environment's `maxCols`/`maxRows` (round 343).
- The panel is an in-shell body (like Work records): it hides the composer and Chat/Trajectory
  tabs while open and never touches the chat transcript, scroll anchor or virtualisation.

## Not verified

No live Host terminal was created, attached, written to or closed. Shell discovery, real PTY
stream framing, actual `WebTerminalInfo` values and Host refusal codes were exercised only
against a local fake BFF fixture. The alpha `desktop` profile is expected to mount the service
(`core/runtime/fork-supervisor.mjs` layers `@deepseek-ai/dsh-web-app`, whose
`cordis.patch.yml` mounts `terminal-controller` unconditionally), but that is a source reading,
not a composed-profile or runtime observation.
