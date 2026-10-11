---
name: native-ui-parity
description: >-
  Port DeepSeek Harness web UI surfaces into the native Rust/Iced app with
  faithful hierarchy, density, alignment, sizing, scrolling and progressive
  disclosure. Extract the web UI's real tokens, dimensions, states and
  behavior, translate them into reusable Iced components, and document
  intentional differences — never an arbitrary redesign and never HTML/CSS.
  Use when changing or reviewing native frontend layout, components, or
  styling against the web reference.
---

# Native UI parity

Adapts the web reference's UI/UX judgment ([`dsh-client-ui-ux`](../../../deepseek-harness-linux/.agents/skills/dsh-client-ui-ux/SKILL.md)) and token rules ([`docs/web-styling.md`](../../../deepseek-harness-linux/docs/web-styling.md)) to the Rust/Iced frontend. Web-specific instructions are translated into Iced terms; this skill never produces HTML, CSS, or Tailwind for the native app.

## Pinned reference

- Web reference: `IOUJared/deepseek-harness-linux`, branch `native-linux`, commit `c6044cc50bc4db0a2b57d1e02d0b53bfeda4590c` ("Release 2026.10.10-r1"). Re-pin before a new comparison and record the new commit in [`docs/NATIVE-UI-PARITY.md`](../../docs/NATIVE-UI-PARITY.md) so results stay reproducible.
- Extracted measurements, the web→native token mapping, palette differences and remaining gaps live in [`docs/NATIVE-UI-PARITY.md`](../../docs/NATIVE-UI-PARITY.md). Build on it; do not re-derive measurements by eye.
- Native token source of truth: [`app/src/design.rs`](../../app/src/design.rs). Web token source of truth: `packages/client/ui-theme/src/styles/` (design-platform.css, base.css, gradient-shadow-text.css).

## Workflow

1. **Extract, never infer.** Read the web reference's actual values from source — `ui-theme` tokens, `ui-primitives` catalog, and the owning feature package's CSS Modules (`ui-sidebar`, `ui-chat`, `ui-conversation`, `ui-layout`). Record exact spacing, typography (size + line height + weight), radii, icon sizes, component states (hover/pressed/focused/disabled/selected), and interaction behavior (dismissal, viewport-fitting, keyboard). Note the value's owning token, not a rounded guess.
2. **Map to a native token before writing a literal.** Every Iced literal must trace to a named constant in `design.rs` that records its web counterpart and any deliberate delta. A value with no web counterpart is either a documented intentional difference or a candidate to align — state which.
3. **Translate into a reusable Iced component or style.** Match the web's hierarchy and density, not its exact pixels where the native palette, font, or window chrome already differ. Reuse `design.rs` styles and existing `ui.rs` components before adding one; extend a component rather than stacking a parallel control.
4. **Verify state, not just the happy path.** Check hover/pressed/disabled/selected, empty and long content, running/completed/failed tools, loading and error states, keyboard focus, menus/dialogs (dismissable, viewport-fitting, unclipped), resizing and display scaling.
5. **Document the difference, then move on.** Palette, font stack (DejaVu Sans vs the web stack), radius rounding and platform chrome are first-pass intentional differences, listed in [`docs/NATIVE-UI-PARITY.md`](../../docs/NATIVE-UI-PARITY.md). Do not silently "fix" them into a new design.

## Translation rules (web → Iced)

- **Tokens:** web `--dsw-*` semantic aliases map to the named Rosé Pine constants in `design.rs` (`BASE`, `SURFACE`, `OVERLAY`, `TEXT`, `MUTED`, `ACCENT`, `DANGER`, `FOAM`, `LINE`). Keep the native palette; record palette differences separately.
- **Hairline seams and borders** become `container` borders or `design::card` strokes. Prefer the shared `panel`/`card`/`seam` helpers over ad-hoc inline styles.
- **Radii** follow the native scale named in `design.rs`; the web's unified scale (xs 4 / sm 8 / md 12 / lg 16 / xl 20 / panel 28) is the reference the native scale approximates — record the mapping, keep native values stable.
- **Font sizes** map to Iced `.size()`. The web scale (11/12/13/14/16/20/24 with paired line heights) informs roles; the native 14/15 px DejaVu base and 13 px code remain unless a parity task explicitly changes typography.
- **Icons** map to the small text glyphs already in use (`↳`, `>_`, `▤`, `▸`, `⋯`, `+`, `‹`, `›`). Do not introduce an icon font or asset without a named need; keep glyph size proportionate to the adjacent text.
- **Menus and dialogs** map to the native `settings_overlay`/`conversation_menu_view`/panel pattern: centered over a dimmed inactive background, Escape-dismissable, never a second OS window.

## Redesign guardrails

- Do not change spacing, sizing, or radius values that already have a documented native choice unless the parity task names that surface. "Closer to web" is not a reason to regress the native palette or geometry owners (`chat_geometry`, `layout_fixture`).
- Any new component gets its geometry and bounds tested through the existing production owners (unit tests, `layout_smoke` fixture); do not add a second model of the same layout.
- Icons, text, and neighbours stay proportionate and aligned along a deliberate axis. Font weight tops out at 500 in feature styling (Rosé Pine native styling), matching the web's weight discipline.

## Evidence

Visual parity claims require inspecting a rendered result: capture the web reference with browser tooling and the native app with real window capture at matching window sizes, conversation data and states. Source-only comparison is never "visual parity". Record screenshots under `app/evidence/` (local, ignored) and distinguish passed checks from unrun ones in the handoff.
