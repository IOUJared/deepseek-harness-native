# Model option panels

| Click | Opens |
|---|---|
| Model | Provider-grouped catalog; selected model checked |
| Effort | Default plus the model’s advertised efforts |
| Speed | Advertised service tiers; premium warning stays visible |
| Output detail | Advertised verbosity choices |
| Back | Summary menu |
| Escape / outside click | Close without activating the conversation |

- Click an active option again to close its panel. The model pill toggles the whole popup.
- Wide window: options left, summary right. Narrow window: one panel with Back.
- Startup resolves the active model; opening refreshes the catalog. No generic model-default label or Load button. Cached choices remain clickable during refresh.
- Click a model to switch this chat. From the welcome screen, the click creates a blank chat and applies that model. No prompt is sent; acknowledgment owns the change.
- Session, transport and opening tickets reject stale clicks; changing panels retires queued choices. Existing pending/consent/archive guards remain.
- Unadvertised controls stay unavailable. No invented provider settings or bypassed billing.

Implementation: [menu renderer](<../app/src/model_menu_view.rs>), [selection guards/tests](<../app/src/model_controls.rs>), [PUBLIC fixtures](<../app/src/layout_fixture.rs>).

Checks: 534 app / 231 fixture-UI tests and a real keyless GUI/Core startup smoke passed. The desktop build is active, verified by executable SHA/inode, mapped native Wayland window and owned backend. Tests cover active-model names, welcome-screen clicks, failure recovery, toggling and acknowledged Speed/Output choices. Earlier nine-mode paint qualification remains separate.

Qualification excludes physical input, live model execution and full parity. Evidence is local-only; older frozen packages are unchanged.
