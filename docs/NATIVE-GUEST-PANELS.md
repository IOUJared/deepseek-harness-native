# Native guest and management panels

What the backend can already serve, what the native app would have to add, and the honest order to build
it in. No dependency is approved yet; everything below that needs one says so.

## How this was established

Every backend package ships a generated remote-client manifest
(`packages/**/lib/typert.remote-client.d.ts`). Enumerating all of them on
2026-10-11 gave **143 distinct RPC methods**; the capability table below is that enumeration,
not inference. Availability at runtime is separate: a manifest can exist in the repository while
the isolated alpha profile does not mount that package, so each panel must confirm membership
(plugin inventory / profile composition) before showing controls.

## Capability table

| Capability | Backend RPCs found | Native cost | New dependency |
| --- | --- | --- | --- |
| Terminal | `terminal/{create,follow,write,resize,close,list,shells,environment,rename,retain}` | Medium: a bounded output view, input line, resize; the Host owns the shell | **None** |
| Goals control | `goals/{create,edit,get,pause,resume,complete,clear}` | Small: extend the existing read-only Work panel with consent-gated actions | **None** |
| Schedules control | `schedule/{catalog,list,history,update,delete}` | Small–medium: same panel, explicit destructive confirm | **None** |
| Subagents | `subagents/{prompt,interruptByParent}` | Medium: needs agent identity/authority rules, not just a list | **None** |
| Plugin management | `pluginManager/{listPlugins,listBundles,inspect,setPluginEnabled,setBundleEnabled,installBundle,removeBundle,registries,waitForInstall,cancelInstall,listVersionExemptions,setVersionExemption}` | Medium–large: install/remove surfaces and version-exemption consent | **None** |
| Office / documents | `officeToPdf/{generation,render}` | Medium: page/image fetch and a scrollable page view | **None** (app already decodes PNG/JPEG) |
| Todos | none — only the `todo/write` event | Read-only in the Work panel (already built) | — |
| Browser / computer use | no matching RPC found | Not feasible as a native panel today: these are agent tools, not a renderable surface | — |

This **corrects an earlier assumption** in this project that the Host offered only
`session/*`, `workspace/*`, `settings/*` and `account/*`: goal and schedule *mutations* do exist,
and so do a full terminal service and a plugin manager. Todo mutation genuinely does not.

## Recommended order

1. **Terminal panel** — the highest value per unit of risk: complete RPCs, no new dependency, no webview,
   and a visibly missing capability in the native app. The Host already owns the shell process; the app must
   stay a *client* of it and never spawn the shell itself. Consent copy must say the terminal runs on the
   Host with the Host's authority, and output must stay bounded like other streams.
2. **Goals and schedules control** on the existing Work panel — reuses the panel's layout and the app's
   consent/ownership patterns; schedules need an explicit destructive confirmation for delete.
3. **Plugin manager panel** — mostly list/toggle plus install/remove; needs careful consent because
   installing a bundle executes allowed build scripts.
4. **Office/document view** — server-side conversion plus a page view; only after the image path is
   proven for large pages.
5. **Browser / computer use** — leave as agent tools; no native panel unless the Host later exposes a
   stream to render.

## Risks and rules carried from the rest of the project

- The app currently spawns **no child processes**; every panel above keeps the shell/browser/guest under
  Host ownership, so nothing here adds app-side process ownership or teardown obligations.
- Read-only first: a panel that can mutate must use the same explicit Review → Confirm, epoch/ticket
  fencing and unknown-outcome handling as rename/fork, and must never retry a committed write.
- Terminal and plugin install are privileged surfaces: keep the Host's authority explicit in the UI, never
  imply sandboxing the Host does not provide, and keep every control that can execute or install visible
  rather than hidden behind a routine toggle.
- Nothing on this page is implemented or verified yet; it is the grounded plan, not a status claim.
