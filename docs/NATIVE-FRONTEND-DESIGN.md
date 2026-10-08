# Native frontend modernization

This page records the earlier modernization qualification. The later [control-layout and Harness-style conversation](<NATIVE-CONTROL-LAYOUT.md>) pass supersedes default tool-card placement and fixed-height tool rows; its execution evidence is separate.

## Design intent

Use ChatGPT desktop's familiar navigation/conversation/composer hierarchy as inspiration, not a pixel-for-pixel copy or OpenAI-branded app. Keep the primary UI **native Iced/wgpu**, standard dark Rosé Pine and existing font. No Electron, webview, browser engine, new UI dependency or desktop configuration change.

## Changes

- Quiet rounded navigation with selected-session styling, real session titles, search and a collapsible sidebar.
- Narrow-window navigation rail and a scrollable navigation/workspace sheet; centered conversation width rather than edge-to-edge debug panels.
- Role-aware conversation previews: right-aligned user bubbles, restrained assistant text and distinct tool/configuration cards.
- Metadata-only sessions open to an honest welcome state. An explicit disclosure retains access to the original records; no synthetic chat history is inserted.
- Rounded composer, clearer model/reasoning controls, explicit model-catalog loading and Send/Stop actions.
- Matching centered settings and decision sheets with quiet, primary and danger treatments. Credential confirmation and approval wording stay explicit.
- Finite sidebar-width transition; no perpetual animation timer or backend activity. Hover/focus feedback is event-driven.
- Focused composer Ctrl+Enter sends only when normal Send and panel/lifecycle guards allow it. Enter remains a newline; navigation shortcuts do not send messages.

## Preserved invariants

The backend, transport, credential carrier, approval/question authority and persistence contracts are unchanged. Catalog loading remains explicit; no provider request or prompt happens merely from opening a pane or animating navigation. Keyless smoke independently disables prompt/catalog/settings writes and decision replies.

History remains bounded to fixed 160 px virtual rows with existing overscan and stable sequence/detail keys. This redesign does **not** add full Markdown, images, attachments or complete long-message rendering. Excerpts and full bounded plaintext details remain distinct.

Credential input remains masked, one-use/fenced and explicitly reviewed/confirmed. Confirmation is not validation/sign-in; errors can remain indeterminate. Decision replies retain exact Agent/Remote Event/epoch/attempt fences and never imply tool execution success.

## Validation record

Final two-job, offline/locked release build passes formatting, all-target checks and **83 app tests**. [Qualification summary](<../app/evidence/frontend-modernization-qppgwtsc/qualification.json>) pins the matching release binary and reports two real isolated-alpha keyless Wayland runs at application scales 1.0/1.25, plus ten public local component rendering/reducer fixtures. Both real runs folded the genuine three bootstrap records, made zero model prompts/catalog requests and stopped gracefully with no observed descendants left.

The parent inspected both final real app frames and selected approval/question/masked-confirmation component frames: readable standard Rosé Pine, rounded controls, clean spacing and a readable disabled Send label. [Actual app at 125%](<../app/evidence/frontend-modernization-qppgwtsc/final-native-scale125/own-window.png>) is an own-window render, not a desktop capture or fabricated chat. Short UI+Node medians were approximately RSS 323/329 MiB, PSS 247/252 MiB and CPU 0% at sampling resolution; these are not matched long-chat performance claims.

A screenshot of the real keyless app is not evidence of actual model generation, live approvals, native keyboard/IME input, arbitrary resizing/minimum-window paint, accessibility or full desktop parity. These remain separately unqualified.

Existing release records and source backups are retained under [modernization evidence](../app/evidence/frontend-modernization-qppgwtsc/). A prior short component-style module was preserved from its full read with reconstruction noted, not represented as a byte-for-byte pre-edit snapshot.
