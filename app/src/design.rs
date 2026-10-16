//! Shared native Rosé Pine styles and a finite owned sidebar tween; no idle timer.
use iced::widget::{button, container, text_editor, text_input};
use iced::{Border, Color, Degrees, Theme, gradient};
// Dark Rosé Pine, declared once. `ui` re-exports these names because most
// modules spell them `crate::ui::ACCENT` and friends; nothing else may redeclare
// a palette entry.
pub const BASE: Color = Color::from_rgb8(0x19, 0x17, 0x24);
pub const SURFACE: Color = Color::from_rgb8(0x1f, 0x1d, 0x2e);
pub const OVERLAY: Color = Color::from_rgb8(0x26, 0x23, 0x3a);
pub const TEXT: Color = Color::from_rgb8(0xe0, 0xde, 0xf4);
pub const MUTED: Color = Color::from_rgb8(0x90, 0x8c, 0xaa);
pub const ACCENT: Color = Color::from_rgb8(0xc4, 0xa7, 0xe7);
pub const DANGER: Color = Color::from_rgb8(0xeb, 0x6f, 0x92);
pub const FOAM: Color = Color::from_rgb8(0x9c, 0xcf, 0xd8);
pub const GOLD: Color = Color::from_rgb8(0xf6, 0xc1, 0x77);
pub const LINE: Color = Color::from_rgb8(0x40, 0x3d, 0x52);
pub const SIDEBAR: f32 = 260.0;
/// The collapsed rail's width: the reference's rail spec is "36x36 control boxes
/// centered in the 56px rail (10px side padding)" (`SidebarRoot.module.css:34-37`),
/// so 10 + 36 + 10. Native used 68.
pub const RAIL: f32 = 56.0;
/// The rail's own seat, `padding: 18px 10px 6px` with a 12px vertical rhythm and
/// 36px control boxes (`SidebarRoot.module.css:34-37`, `:155-170`).
pub const RAIL_PAD: iced::Padding = iced::Padding {
    top: 18.0,
    right: 10.0,
    bottom: 6.0,
    left: 10.0,
};
pub const RAIL_CONTROL: f32 = 36.0;
pub const RAIL_RHYTHM: f32 = 12.0;
/// The transcript width handles' hover/drag affordance: a `2px` bar held `4px`
/// in from the strip's inner edge, whose visible window is a `16px` solid core
/// centred on the pointer with a fade to transparent by `36px` each way, and
/// whose resting opacity is `0` (`ConversationRoot.module.css:289-317`, with the
/// published `--dsh-width-handle-pointer-y` defaulting to `50%`). Iced has no
/// gradient, so native draws the solid core and centres it.
pub const HANDLE_GLOW_WIDTH: f32 = 2.0;
pub const HANDLE_GLOW_INSET: f32 = 4.0;
pub const HANDLE_GLOW_CORE: f32 = 16.0;
/// Sidebar column metrics, each from the web's own rule
/// (`ui-sidebar/SidebarRoot.module.css`, `ui-workspace/.../Rows.module.css`):
/// `.root { padding: 6px var(--dsh-sidebar-inline-padding) }` with the inline
/// padding at 12px, `.logoRow { margin-bottom: 4px }`,
/// `.newSession { height: 38px; padding: 8px 12px; margin: 0 2px 12px }`,
/// `.panelList { gap: 4px; margin-bottom: 8px }`,
/// `.panelRow { min-height: 36px; padding: 7px 8px }`, and a session cell of
/// `height: 32px; padding: 0 8px` whose title is `14px/20px` with a 4px/6px
/// margin.
pub const SIDEBAR_PAD: [f32; 2] = [6.0, 12.0];
pub const SIDEBAR_LOGO_GAP: f32 = 4.0;
pub const NEW_SESSION_HEIGHT: f32 = 38.0;
pub const NEW_SESSION_PAD: [f32; 2] = [8.0, 12.0];
/// `.newSession { margin: 0 2px 12px }`: the bar is inset 2px inside the column
/// padding, and 12px of clear space follows it.
pub const NEW_SESSION_INSET: f32 = 2.0;
pub const NEW_SESSION_GAP: f32 = 12.0;
pub const PANEL_ROW_HEIGHT: f32 = 36.0;
pub const PANEL_ROW_PAD: [f32; 2] = [7.0, 8.0];
/// `.panelList { gap: 4px }`, reused for the sidebar's own row stack and as the
/// browser region's clearance above the workspace seat (`.panelList`'s
/// `margin-bottom: 8px`).
pub const PANEL_GAP: f32 = 4.0;
pub const PANEL_LIST_GAP: f32 = 8.0;
/// A session row's own box: the web fixes the cell at 32px around a 20px title
/// line, so native keeps 6px of vertical padding there and lets a status line
/// grow the row rather than clipping it inside a fixed height.
pub const SESSION_ROW_PAD: [f32; 2] = [6.0, 8.0];
pub const SESSION_ROW_HEIGHT: f32 = 32.0;
pub const SESSION_TITLE_LINE: f32 = 20.0;
/// The sidebar row title's line box. The web fixes it at `20px`
/// (`Rows.module.css:162-166`) rather than scaling it with
/// `--dsh-content-font-delta`, and it is what makes a one-line session cell
/// exactly `SESSION_ROW_HEIGHT` tall with `SESSION_ROW_PAD`.
pub fn session_title_line() -> iced::advanced::text::LineHeight {
    iced::advanced::text::LineHeight::Absolute(iced::Pixels(SESSION_TITLE_LINE))
}
/// The web's row action cluster is `height: 20px` (`Rows.module.css:258-262`),
/// so the compact trailing control fits inside the 32px session cell.
pub const ROW_ACTION_HEIGHT: f32 = 20.0;
/// The browser region above the session list
/// (`ui-workspace/.../WorkspaceBrowser.module.css`): the expanded search is
/// `height: 30px` with `13px/18px` input text (`:160-171, :214-222`), and
/// `.sectionHeader` is a `height: 36px` box with `gap: 4px` and a 4px trailing
/// margin (`:53-62`), its trailing icon controls `28px` squares (`:17-30`).
pub const SEARCH_HEIGHT: f32 = 30.0;
pub const SEARCH_PAD: [f32; 2] = [6.0, 10.0];
pub const SEARCH_FONT: f32 = 13.0;
pub const SEARCH_LINE: f32 = 18.0;
pub const SECTION_HEADER_HEIGHT: f32 = 36.0;
pub const SECTION_HEADER_GAP: f32 = 4.0;
pub const SECTION_ACTION_HEIGHT: f32 = 28.0;
pub const CHAT: f32 = 800.0;
pub const MIN_CHAT: f32 = 680.0;
pub const MAX_CHAT: f32 = 920.0;
pub const BREAKPOINT: f32 = 1000.0;
/// Horizontal inset for a disclosure's expanded body. The web indents those
/// bodies so they sit under the disclosure *title* rather than under its glyph:
/// `ReasoningRow.module.css` `.thinkBody` uses `calc(22px + delta)`, documented
/// there as the leading `16 + delta` plus a `6` gap. The same 22 already governed
/// this app's search-match offsets, so naming it makes the render agree with them.
pub const CONTENT_INSET: f32 = 22.0;
/// The fixed leading box a process row reserves for its glyph. The web's
/// `DisclosureRow` `.leading` is `calc(16px + delta)`, so the box holds the
/// chevron and the variant icon without the title shifting between them.
pub const LEADING_BOX: f32 = 16.0;
/// The web's transcript line boxes, in absolute pixels rather than a ratio:
/// body text is `calc(24px + var(--dsh-content-font-delta))`
/// (`AssistantMarkdown.module.css:13`, `ToolRow.module.css:42`) and compact or
/// secondary text is `calc(20px + var(--dsh-content-font-delta-secondary))`
/// (`MarkdownText.module.css:334`, `ReasoningRow.module.css:49`). Iced's relative
/// `LineHeight::default()` is `1.3`, which made every native line 19.5px where
/// the web gives 24.
pub const LINE_BODY: f32 = 24.0;
pub const LINE_COMPACT: f32 = 20.0;
/// The transcript body size: the web's `--dsh-content-font-size` default
/// (`gradient-shadow-text.css:57`). Native reserved and drew body text at 15,
/// which was a size the web never uses; with an absolute line box it also made
/// the ratio wrong.
pub const BODY_SIZE: f32 = 14.0;
/// The secondary tier: the web's `--dsh-content-font-size-secondary`
/// (`gradient-shadow-text.css:58`), which is 13 at the default size and steps
/// one under the body's own delta below it, two above.
pub const SECONDARY_SIZE: f32 = 13.0;
/// The gap between assistant blocks. The web's assistant body is a flex column
/// with `gap: 16px` (`AssistantMarkdown.module.css:20`) and its markdown blocks
/// carry `margin: 16px 0` (`MarkdownText.module.css:33`), where native used 12.
pub const BLOCK_GAP: f32 = 16.0;
/// The gap between two conversation flow rows. The web gives every flow sibling
/// `margin-top: var(--dsh-chat-flow-gap, 6px)` (`ChatView.module.css:60-71`), so
/// two process rows sit 6px apart.
pub const FLOW_GAP: f32 = 6.0;
/// The gap around an Assistant response: the response row itself and the row
/// after it take 12px (`ChatView.module.css:73-85`), the live `.running` status
/// included (`:118`). A `turn-process` header would take 16px (`:87-95`), but
/// Chat hides those rows — they are Trajectory-only — so no native row maps to
/// it.
pub const RESPONSE_GAP: f32 = 12.0;
/// The transcript scroller's own vertical inset: the web's `.scroll` padding is
/// `16px calc(clearance + 16px)` (`ChatView.module.css:26-29`), so the first row
/// starts 16px below the header and the last ends 16px above the composer.
pub const FLOW_INSET: f32 = 16.0;
/// The body line box at a content-size delta.
pub fn body_line(delta: f32) -> iced::advanced::text::LineHeight {
    iced::advanced::text::LineHeight::Absolute(iced::Pixels(LINE_BODY + delta))
}
/// The compact line box, at the secondary delta.
pub fn compact_line(delta_secondary: f32) -> iced::advanced::text::LineHeight {
    iced::advanced::text::LineHeight::Absolute(iced::Pixels(LINE_COMPACT + delta_secondary))
}
/// A markdown heading's size. The web's ladder is
/// `--dsw-font-markdown-h1..h3` = 21/19/18 with `h4` and below at the body size
/// (`gradient-shadow-text.css:61-74`), and native's type base sits one pixel
/// above the web's (body 15 against 14), so the ladder is 22/20/19 and the
/// fourth level falls back to the body.
///
/// One ladder for every surface: the transcript used 23/20/17 and the detail
/// panel 25/21/18, so the same document rendered at two different sizes.
pub fn heading_size(level: u8) -> f32 {
    match level {
        1 => 22.0,
        2 => 20.0,
        3 => 19.0,
        _ => 15.0,
    }
}
/// The web's default content size, and the range its settings offer.
pub const CONTENT_FONT_DEFAULT: f32 = 14.0;
pub const CONTENT_FONT_MIN: f32 = 11.0;
pub const CONTENT_FONT_MAX: f32 = 20.0;
/// The secondary line the transcript steps down to at the default size.
const CONTENT_SECONDARY_DEFAULT: f32 = 13.0;
/// A size outside the offered range is treated as the default rather than
/// clamped into a value no control can produce, so a corrupt or hand-edited
/// file cannot pin the UI somewhere the user cannot leave.
pub fn content_size(size: f32) -> f32 {
    if size.is_finite() && (CONTENT_FONT_MIN..=CONTENT_FONT_MAX).contains(&size) {
        size
    } else {
        CONTENT_FONT_DEFAULT
    }
}
/// `--dsh-content-font-delta`: what a size preference adds to every row height
/// and icon box, which the web writes as `calc(<base> + var(…delta, 0px))`.
pub fn content_delta(size: f32) -> f32 {
    content_size(size) - CONTENT_FONT_DEFAULT
}
/// `--dsh-content-font-size-secondary`: one step under the body — the setting
/// minus one at or below the default, minus two above it, as the web documents.
pub fn content_secondary(size: f32) -> f32 {
    let size = content_size(size);
    if size <= CONTENT_FONT_DEFAULT {
        size - 1.0
    } else {
        size - 2.0
    }
}
/// `--dsh-content-font-delta-secondary`, which drives line-heights rather than
/// the row boxes the primary delta drives.
pub fn content_delta_secondary(size: f32) -> f32 {
    content_secondary(size) - CONTENT_SECONDARY_DEFAULT
}
/// The main window's initial size, and the size the App assumes before its first
/// `Resized` event. The web desktop shell creates a 1280x820 window, so that is
/// the reference; these were two independent literals (1200x800 created,
/// 1280x820 assumed) and the lane/compact decisions ran on the wrong one.
pub const WINDOW_WIDTH: f32 = 1280.0;
pub const WINDOW_HEIGHT: f32 = 820.0;
/// Web transcript column: `clamp(680px, 64% of the conversation column, 920px)`
/// (ui-conversation `ConversationRoot`). `column` is the conversation pane
/// width before its horizontal padding. Malformed input falls back to `CHAT`.
///
/// Test-only: production always has a preference slot to fill, so it calls
/// [`chat_width_with`] and this spelling exists to keep the clamp assertions
/// readable.
#[cfg(test)]
pub fn chat_width(column: f32) -> f32 {
    chat_width_with(column, None)
}
/// Floor for a user-set conversation content width. The web calls this
/// `CONTENT_MIN` and matches its layout center-column minimum.
pub const CHAT_CONTENT_MIN: f32 = 640.0;
/// Horizontal room the web reserves for both drag handles and their safe edge
/// zones (`CONTENT_EDGE_BUDGET`); it bounds how wide a set preference may be.
pub const CHAT_EDGE_BUDGET: f32 = 176.0;
/// Conversation content width with the web's own precedence: a stored
/// preference wins and is clamped into `[CHAT_CONTENT_MIN, max(CHAT_CONTENT_MIN,
/// column - CHAT_EDGE_BUDGET)]`; otherwise the adaptive clamp applies. Mirrors
/// `resolveContentWidth` in `ui-conversation/.../ConversationWidthControls.tsx`.
///
/// A malformed column keeps the pre-existing `CHAT` fallback and ignores the
/// preference: without a measured column there is no bound to clamp against,
/// and inventing one would let a stored width exceed the pane.
pub fn chat_width_with(column: f32, preference: Option<f32>) -> f32 {
    if !column.is_finite() || column <= 0.0 {
        return CHAT;
    }
    if let Some(preference) = preference.filter(|value| value.is_finite() && *value > 0.0) {
        let max = CHAT_CONTENT_MIN.max(column - CHAT_EDGE_BUDGET);
        return preference.clamp(CHAT_CONTENT_MIN, max);
    }
    (column * 0.64).clamp(MIN_CHAT, MAX_CHAT)
}
/// Which width handle is being dragged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WidthSide {
    Left,
    Right,
}
/// Content width while a drag is in progress. Mirrors `outwardWidth` in
/// `ui-conversation/.../ConversationWidthControls.tsx`: the outward delta is
/// doubled, because the content is centered, so dragging one handle by `dx`
/// moves both edges and widens the column by `2dx`. The left handle mirrors the
/// sign. The result is deliberately unclamped: the caller resolves it through
/// [`chat_width_with`], so dragging past either bound simply stops growing.
pub fn chat_width_drag(base: f32, origin_x: f32, current_x: f32, side: WidthSide) -> f32 {
    let dx = current_x - origin_x;
    let outward = match side {
        WidthSide::Right => dx,
        WidthSide::Left => -dx,
    };
    base + outward * 2.0
}
/// A message's attachment card: the reference's `.fileCard` is a fixed
/// `240px` box with a `64px` minimum height (`MessageItem.module.css:320-332`),
/// whose name and meta lines (22 and 15) fit inside it at every content size —
/// only `.bubble` follows `--dsh-content-font-size`.
pub const ATTACHMENT_CARD_WIDTH: f32 = 240.0;
pub const ATTACHMENT_CARD_HEIGHT: f32 = 64.0;
/// `.userStack { gap: 8px }`, the gap between the attachment row and the bubble
/// and between the cards themselves (`MessageItem.module.css:10-21`).
pub const ATTACHMENT_CARD_GAP: f32 = 8.0;
/// The composer's attachment rail (`ComposerAttachments.module.css:1-8`,
/// `AttachmentRail.module.css:8-30`): `padding: 2px 10px 0` with a `-6px` bottom
/// margin inside the card's 12px gap, a 10px gap between 64px-tall items, and an
/// 18px round remove control 6px from each card's top-right corner
/// (`FileCard.module.css:77-96`).
pub const ATTACH_RAIL_PAD: iced::Padding = iced::Padding {
    top: 2.0,
    right: 10.0,
    bottom: 0.0,
    left: 10.0,
};
pub const ATTACH_RAIL_GAP: f32 = 10.0;
pub const ATTACH_RAIL_ITEM_HEIGHT: f32 = 64.0;
pub const ATTACH_RAIL_BOTTOM_PULL: f32 = -6.0;
pub const ATTACH_RAIL_REMOVE_SIZE: f32 = 18.0;
pub const ATTACH_RAIL_REMOVE_INSET: f32 = 6.0;
/// The height `count` attachment cards add above a user bubble: one card each,
/// `ATTACHMENT_CARD_GAP` between them, and the same gap before the bubble when
/// one is drawn. The reservation and the view both read this, so a card cannot
/// be drawn without being measured.
pub fn attachment_height(count: usize, has_bubble: bool) -> f32 {
    if count == 0 {
        return 0.0;
    }
    let count = count as f32;
    count * ATTACHMENT_CARD_HEIGHT
        + (count - 1.0) * ATTACHMENT_CARD_GAP
        + if has_bubble { ATTACHMENT_CARD_GAP } else { 0.0 }
}
// Native radius scale, mapped to the web unified scale (docs/NATIVE-UI-PARITY.md):
// sm≈8, md=12, lg=16, xl=20. Native rounds sm to 9 and lg to 18; panel stays a
// tighter 20 than the web 28. Keep these stable — the parity skill forbids
// arbitrary redesigns.
pub const RADIUS_SM: f32 = 9.0;
pub const RADIUS_MD: f32 = 12.0;
pub const RADIUS_LG: f32 = 18.0;
pub const RADIUS_XL: f32 = 20.0;
fn border(radius: f32, color: Color, width: f32) -> Border {
    Border {
        radius: radius.into(),
        color,
        width,
    }
}
pub fn ghost(_: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(OVERLAY.into()),
        text_color: if matches!(status, button::Status::Disabled) {
            MUTED
        } else {
            TEXT
        },
        border: border(RADIUS_SM, Color::TRANSPARENT, 0.0),
        ..Default::default()
    }
}
pub fn primary(_: &Theme, status: button::Status) -> button::Style {
    let disabled = matches!(status, button::Status::Disabled);
    let color = if disabled {
        OVERLAY
    } else if matches!(status, button::Status::Pressed) {
        ACCENT
    } else {
        FOAM
    };
    button::Style {
        background: Some(color.into()),
        text_color: if disabled { MUTED } else { BASE },
        border: border(RADIUS_MD, Color::TRANSPARENT, 0.0),
        ..Default::default()
    }
}
pub fn navigation(theme: &Theme, status: button::Status, selected: bool) -> button::Style {
    let mut style = ghost(theme, status);
    if selected && !matches!(status, button::Status::Disabled) {
        style.background = Some(OVERLAY.into());
        // The web keeps primary ink on a selected row and changes only the
        // surface: its sidebar sets `--dsw-alias-label-primary` on `.panelRow`
        // and re-asserts it on `.panelRow.panelActive`, with no accent state
        // anywhere. Accent here contradicted the callers, which already pin the
        // title to TEXT.
        style.text_color = TEXT;
    }
    style
}
/// [`navigation`] at the web's row radius: `.panelRow` and `.sessionRow` are
/// both `border-radius: var(--dsw-radius-md)` (`SidebarRoot.module.css:493-505`,
/// `Rows.module.css:2-10`), where the shared native row surface rounds to
/// `RADIUS_SM`. Kept a separate style so the other `navigation` surfaces (the
/// settings categories and plugin rows) keep their own radius until their web
/// counterparts are checked.
pub fn navigation_row(theme: &Theme, status: button::Status, selected: bool) -> button::Style {
    let mut style = navigation(theme, status, selected);
    style.border.radius = RADIUS_MD.into();
    style
}
/// The web's New Session bar: `--dsw-alias-button-elevated-fill` over the
/// sidebar fill with a `0.5px solid var(--dsw-alias-border-l3)` edge and
/// `--dsw-radius-md` (`SidebarRoot.module.css:397-409`). Native's elevation
/// ladder puts that fill at `OVERLAY` above the sidebar's `SURFACE`, rounds both
/// l2 and l3 to the `LINE` hairline, and darkens on hover the way
/// `--dsw-alias-button-floating-hover` does.
pub fn elevated(_: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: Some(
            if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                BASE
            } else {
                OVERLAY
            }
            .into(),
        ),
        text_color: TEXT,
        border: border(RADIUS_MD, LINE, 0.5),
        ..Default::default()
    }
}
pub fn danger(_: &Theme, status: button::Status) -> button::Style {
    let disabled = matches!(status, button::Status::Disabled);
    button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(OVERLAY.into()),
        text_color: if disabled { MUTED } else { DANGER },
        border: border(RADIUS_SM, Color::TRANSPARENT, 0.0),
        ..Default::default()
    }
}
pub fn field(_: &Theme, status: text_input::Status) -> text_input::Style {
    text_input::Style {
        background: SURFACE.into(),
        border: border(
            10.0,
            if matches!(status, text_input::Status::Focused { .. }) {
                ACCENT
            } else {
                OVERLAY
            },
            1.0,
        ),
        icon: MUTED,
        placeholder: MUTED,
        value: if matches!(status, text_input::Status::Disabled) {
            MUTED
        } else {
            TEXT
        },
        selection: OVERLAY,
    }
}
/// The web's expanded sidebar search: a transparent fill under a `0.5px`
/// `var(--dsw-alias-border-l4)` hairline at `--dsw-radius-md`
/// (`WorkspaceBrowser.module.css:160-171`), with `13px/18px` input text
/// (`:214-222`). Native rounds l4 to the `LINE` hairline and keeps the accent
/// ring on focus.
pub fn search_field(_: &Theme, status: text_input::Status) -> text_input::Style {
    text_input::Style {
        background: Color::TRANSPARENT.into(),
        border: border(
            RADIUS_MD,
            if matches!(status, text_input::Status::Focused { .. }) {
                ACCENT
            } else {
                LINE
            },
            0.5,
        ),
        icon: MUTED,
        placeholder: MUTED,
        value: if matches!(status, text_input::Status::Disabled) {
            MUTED
        } else {
            TEXT
        },
        selection: OVERLAY,
    }
}
pub fn editor(_: &Theme, status: text_editor::Status) -> text_editor::Style {
    text_editor::Style {
        background: SURFACE.into(),
        // Constant width, transparent on blur and accent on focus: the caret is
        // visible without the box shifting a pixel as focus moves, which is what
        // a 0.0-width ring did (it could never draw). `text_input` above draws
        // its ring the same way.
        border: border(
            RADIUS_MD,
            if matches!(status, text_editor::Status::Focused { .. }) {
                ACCENT
            } else {
                Color::TRANSPARENT
            },
            1.0,
        ),
        placeholder: MUTED,
        value: TEXT,
        selection: OVERLAY,
    }
}
pub fn card(background: Color, radius: f32) -> container::Style {
    container::Style {
        background: Some(background.into()),
        text_color: Some(TEXT),
        border: border(radius, OVERLAY, 1.0),
        ..Default::default()
    }
}
/// The web's elevated input surface, shared by the composer card and a message's
/// attachment card: both fill with `--dsw-specific-input-major` — the same
/// bluish-850 the user bubble uses — and carry the elevation stroke, a `0.5px`
/// `var(--dsw-alias-border-l2)` hairline (`InputBar.module.css:60-100`,
/// `MessageItem.module.css:320-332`, `gradient-shadow-text.css:33-40`). In dark
/// mode that edge resolves to `rgba(255,255,255,0.12)` over the fill, which is
/// this palette's `LINE`; native's elevation ladder puts the fill at `OVERLAY`
/// above `BASE`/`SURFACE`, so the card keeps the reference's visible hairline
/// instead of `card`'s same-as-fill stroke. The web's soft shadow is not drawn.
/// A flat filled strip: the header's 2px tab bar, which the web draws with
/// `border-radius: 2px` in the business-blue accent
/// (`ConversationRoot.module.css:218-231`).
pub fn bar(color: Color) -> container::Style {
    container::Style {
        background: Some(color.into()),
        border: border(2.0, Color::TRANSPARENT, 0.0),
        ..Default::default()
    }
}
pub fn elevated_surface(_: &Theme) -> container::Style {
    container::Style {
        background: Some(OVERLAY.into()),
        text_color: Some(TEXT),
        border: border(RADIUS_XL, LINE, 0.5),
        ..Default::default()
    }
}
/// Height of the fade above the composer. The web fades over its composer seat
/// with `linear-gradient(180deg, … 0px, var(--dsw-alias-bg-base) 36px)`.
pub const COMPOSER_FADE_HEIGHT: f32 = 36.0;
/// The composer card's own inset and the gap between its text area and its
/// control row, from the web's `InputBar.module.css`: `.card { padding-top: 8px;
/// gap: 12px }` (`:60-72`). The card has no side or bottom padding — the text
/// area and the control row carry their own insets below.
pub const COMPOSER_CARD_TOP: f32 = 8.0;
pub const COMPOSER_CARD_GAP: f32 = 12.0;
/// `.input { min-height: 36px; padding: 4px 8px 0 14px; line-height: 24px }`
/// (`InputBar.module.css:169-176`): 14px of left inset for the draft, and the
/// 8px breath the web leaves under a single line inside the 36px floor.
pub const COMPOSER_INPUT_PAD: iced::Padding = iced::Padding {
    top: 4.0,
    right: 8.0,
    bottom: 0.0,
    left: 14.0,
};
pub const COMPOSER_INPUT_MIN: f32 = 36.0;
/// The input's vertical chrome: its 4px top pad plus the 8px breath below a
/// single line. One line plus this chrome is exactly `COMPOSER_INPUT_MIN`, and
/// at the fourteen-line cap the text box is still the web's 336px.
pub const COMPOSER_INPUT_CHROME: f32 = 12.0;
/// `.row { padding: 2px 8px 6px; gap: 12px }` (`InputBar.module.css:251-260`):
/// the controls sit 8px in from the card edge and the row's own 2px downward
/// shift is part of its box.
pub const COMPOSER_ROW_PAD: iced::Padding = iced::Padding {
    top: 2.0,
    right: 8.0,
    bottom: 6.0,
    left: 8.0,
};
pub const COMPOSER_ROW_GAP: f32 = 12.0;
/// The composer's trailing controls: the web's `.select` chip is `height: 28px`
/// with `padding: 0 20px 0 8px` — 8px left, then the text, then a 12px chevron
/// 4px from the trailing edge — and `13px/20px` secondary text
/// (`InputBar.module.css:329-352`); the primary send and the stop control are
/// `34px` boxes (`:369-395`). Native keeps text labels on send and stop (drawn
/// glyphs are blocked and the words are the native affordance), so the boxes
/// take the heights and the chip takes the web's own shape.
pub const COMPOSER_CHIP_HEIGHT: f32 = 28.0;
pub const COMPOSER_CHIP_PAD: iced::Padding = iced::Padding {
    top: 0.0,
    right: 4.0,
    bottom: 0.0,
    left: 8.0,
};
pub const COMPOSER_CHIP_CHEVRON: f32 = 12.0;
pub const COMPOSER_CONTROL_HEIGHT: f32 = 34.0;
/// The transcript's own side inset: the web's `.scroll` pads
/// `calc(var(--dsh-composer-side-clearance) + 16px)` = 32px per side
/// (`ChatView.module.css:26-29`, `ConversationRoot.module.css:389`).
pub const TRANSCRIPT_INSET: f32 = 32.0;
/// The composer seat: the web's `.root { padding: 0
/// var(--dsh-composer-side-clearance) 4px }` (`InputBar.module.css:9-11`) with
/// its dock strip 4px below the card (`:13-20`), and a card capped at
/// `--dsh-chat-content-width + 32px` (`ConversationRoot.module.css:388`) so it
/// overhangs the transcript by 16px per side.
pub const COMPOSER_SEAT_PAD: iced::Padding = iced::Padding {
    top: 0.0,
    right: 16.0,
    bottom: 4.0,
    left: 16.0,
};
pub const COMPOSER_DOCK_GAP: f32 = 4.0;
pub const COMPOSER_CARD_OVERHANG: f32 = 32.0;
/// The empty-state hero: the reference stacks its headline and the composer card
/// with a 12px gap inside a centered shell (`HeroShell.module.css:5-21`,
/// `ConversationContent.tsx:164-171`), and the composer's own `.hero` variant
/// raises the draft floor to `52px` — two 24px lines and the 4px top pad —
/// instead of the docked 36px, and drops the seat's bottom pad
/// (`InputBar.module.css:26-30, 232-236`).
pub const HERO_GAP: f32 = 12.0;
pub const HERO_INPUT_MIN: f32 = 52.0;
/// The hero's headline: `HeroShell.module.css:33-60` draws the fish and the
/// title as one centered row with `column-gap: 10px`, the title at `26/32`
/// weight 500 (figma 34:10411).
/// The hero's workspace chip (`HeroShell.module.css:152-186`): a
/// `min-height: 28px` chip with `padding: 0 8px`, `gap: 4px`, `radius-sm`, a
/// transparent fill that takes the interactive hover, `13px/20px` primary text,
/// a 16px folder glyph and a 12px chevron. Its row carries a `4px` top margin
/// and `padding: 0 16px 0 20px` inside the composer-card width
/// (`ConversationRoot.module.css:520-529`).
pub const HERO_CHIP_PAD: iced::Padding = iced::Padding {
    top: 0.0,
    right: 8.0,
    bottom: 0.0,
    left: 8.0,
};
pub const HERO_CHIP_GAP: f32 = 4.0;
pub const HERO_CHIP_GLYPH: f32 = 16.0;
pub const HERO_CHIP_CHEVRON: f32 = 12.0;
pub const HERO_ROW_TOP: f32 = 4.0;
pub const HERO_ROW_PAD: iced::Padding = iced::Padding {
    top: 0.0,
    right: 16.0,
    bottom: 0.0,
    left: 20.0,
};
pub const HERO_HEADLINE_GAP: f32 = 10.0;
pub const HERO_TITLE_SIZE: f32 = 26.0;
pub const HERO_TITLE_LINE: f32 = 32.0;
pub const COMPOSER_HERO_SEAT_PAD: iced::Padding = iced::Padding {
    top: 0.0,
    right: 16.0,
    bottom: 0.0,
    left: 16.0,
};
/// The conversation header's own rows, from the web's `ConversationRoot`: the
/// title row has `min-height: 30px` (`:72-79`), the tab strip sits `10px` under
/// it with a `36px` gap and an `8px` leading inset (`:193-201`), and each tab is
/// `13px/16px` with a `9px` under-pad and a `2px` bar that ends flush with the
/// header's seam (`:203-231`). Those plus the header's `10px` top inset and
/// `9px` bottom inset are the web's 76px band.
pub const HEADER_TITLE_ROW: f32 = 30.0;
pub const HEADER_TABS_GAP: f32 = 10.0;
pub const HEADER_TABS_INSET: f32 = 8.0;
pub const TAB_GAP: f32 = 36.0;
pub const TAB_LINE: f32 = 16.0;
pub const TAB_PAD_BOTTOM: f32 = 9.0;
pub const TAB_BAR: f32 = 2.0;
/// The edit stat a tool row appends after its summary
/// (`ui-tool/.../ToolRow.module.css:64-77`): `margin-left: 10px`, the code font
/// at `secondary − 2` = 11px, and the `+N -M` counts written with an ASCII
/// hyphen; they take the success/error pair only while the row is hovered or
/// expanded, staying muted at rest.
/// The plan-mode chip the reference puts in the control row's leading `.modes`
/// group beside the attach control: `height: 28px`, `padding: 0 8px`, `gap: 4px`,
/// `radius-sm`, a 14px glyph slot, a `13px/20px` label in the business-primary
/// colour over the business-tertiary fill, and a 6%-mixed fill on hover
/// (`PlanModeControl.module.css:4-40`, `InputBar.tsx:442-450`). Native's palette
/// has no business pair, so the fill is a 14% accent tint under `ACCENT` text.
pub const PLAN_CHIP_PAD: iced::Padding = iced::Padding {
    top: 0.0,
    right: 8.0,
    bottom: 0.0,
    left: 8.0,
};
pub const PLAN_CHIP_GAP: f32 = 4.0;
pub const PLAN_CHIP_GLYPH: f32 = 14.0;
pub const PLAN_CHIP_FILL: Color = Color::from_rgba(0.77, 0.65, 0.91, 0.14);
pub const PLAN_CHIP_HOVER: Color = Color::from_rgba(0.77, 0.65, 0.91, 0.22);
pub const PLAN_CHIP_FILL_DIM: Color = Color::from_rgba(0.77, 0.65, 0.91, 0.07);
/// The reference's jump-to-latest control (`ChatView.module.css:223-273`): a
/// slot 16px above the composer, right-aligned so the button's edge lands on the
/// content column's right edge — or `--dsh-composer-side-clearance + 16px` = 32px
/// from the pane edge once the column is narrower — holding a 34px round
/// floating button with a `100px` radius, the floating fill, the panel elevation
/// shadow and the primary label ink.
/// The header's breadcrumb trail (`ConversationRoot.module.css:102-146`): 4px
/// between segments, each crumb a `4px 8px`-padded `radius-md` box capped at
/// 220px, `14px/20px` in the tertiary colour — primary and weight 500 for the
/// current session — with subagent crumbs at `12px/18px`, and a `14px/20px`
/// caption `/` separator between them.
pub const CRUMB_GAP: f32 = 4.0;
pub const CRUMB_PAD: iced::Padding = iced::Padding {
    top: 4.0,
    right: 8.0,
    bottom: 4.0,
    left: 8.0,
};
pub const CRUMB_MAX: f32 = 220.0;
pub const CRUMB_SIZE: f32 = 14.0;
pub const CRUMB_LINE: f32 = 20.0;
pub const CRUMB_SUBAGENT_SIZE: f32 = 12.0;
pub const CRUMB_SUBAGENT_LINE: f32 = 18.0;
/// A button filled at rest, for the reference's history control:
/// `.older button { border-radius: radius-sm; padding: 4px 12px; font-size: 12px;
/// color: label-secondary; background: interactive-bg-hover-solid }`
/// (`ChatView.module.css:206-218`). Native steps one rung up on hover.
pub fn fill_solid(_: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: Some(
            if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                LINE
            } else {
                OVERLAY
            }
            .into(),
        ),
        text_color: if matches!(status, button::Status::Disabled) {
            MUTED
        } else {
            TEXT
        },
        border: border(RADIUS_SM, Color::TRANSPARENT, 0.0),
        ..Default::default()
    }
}
pub const TO_BOTTOM_SIZE: f32 = 34.0;
pub const TO_BOTTOM_INSET: f32 = 16.0;
pub const TO_BOTTOM_EDGE: f32 = 32.0;
pub const DIFF_STAT_LEFT: f32 = 10.0;
pub const DIFF_STAT_SIZE: f32 = 11.0;
/// The fade itself: transparent at the top, base colour by its bottom, so
/// scrolled content dissolves instead of meeting the composer at a hard edge.
///
/// `Degrees(180.0)` reads downward here because `Radians::to_distance` builds
/// its direction from `angle - FRAC_PI_2`, which yields `(0, 1)` — increasing y
/// — at 180 degrees on a y-down screen.
pub fn composer_fade(_: &Theme) -> container::Style {
    container::Style {
        background: Some(
            gradient::Linear::new(Degrees(180.0))
                .add_stop(0.0, Color { a: 0.0, ..BASE })
                .add_stop(1.0, BASE)
                .into(),
        ),
        ..Default::default()
    }
}
pub fn picker(
    _: &Theme,
    status: iced::widget::pick_list::Status,
) -> iced::widget::pick_list::Style {
    iced::widget::pick_list::Style {
        text_color: TEXT,
        placeholder_color: MUTED,
        handle_color: MUTED,
        background: OVERLAY.into(),
        border: border(
            RADIUS_SM,
            if matches!(
                status,
                iced::widget::pick_list::Status::Hovered
                    | iced::widget::pick_list::Status::Opened { .. }
            ) {
                LINE
            } else {
                Color::TRANSPARENT
            },
            1.0,
        ),
    }
}

/// Finite, retargetable logical-width animation; no subscription when settled.
#[derive(Clone, Debug)]
pub struct SidebarMotion {
    value: f32,
    from: f32,
    target: f32,
    started: Option<std::time::Instant>,
}
impl SidebarMotion {
    const DURATION: std::time::Duration = std::time::Duration::from_millis(180);
    pub fn new(width: f32) -> Self {
        let width = width.clamp(RAIL, SIDEBAR);
        Self {
            value: width,
            from: width,
            target: width,
            started: None,
        }
    }
    pub fn value(&self) -> f32 {
        self.value
    }
    pub fn running(&self, visible: bool) -> bool {
        visible && self.started.is_some()
    }
    pub fn snap(&mut self, target: f32) {
        self.value = target.clamp(RAIL, SIDEBAR);
        self.from = self.value;
        self.target = self.value;
        self.started = None;
    }
    pub fn retarget(&mut self, target: f32, now: std::time::Instant, visible: bool) {
        self.advance(now);
        let target = target.clamp(RAIL, SIDEBAR);
        if !visible || (self.value - target).abs() < 0.01 {
            self.snap(target);
            return;
        }
        self.from = self.value;
        self.target = target;
        self.started = Some(now);
    }
    pub fn advance(&mut self, now: std::time::Instant) {
        let Some(started) = self.started else {
            return;
        };
        let elapsed = now.saturating_duration_since(started);
        if elapsed >= Self::DURATION {
            self.snap(self.target);
            return;
        }
        let t = elapsed.as_secs_f32() / Self::DURATION.as_secs_f32();
        self.value = (self.from + (self.target - self.from) * (1.0 - (1.0 - t).powi(3)))
            .clamp(RAIL, SIDEBAR);
    }
}
#[cfg(test)]
mod motion_tests {
    use super::*;
    use std::time::{Duration, Instant};
    #[test]
    fn idle_hidden_and_settled_motion_has_no_subscription() {
        let now = Instant::now();
        let mut m = SidebarMotion::new(SIDEBAR);
        assert!(!m.running(true));
        m.retarget(RAIL, now, true);
        assert!(m.running(true));
        assert!(!m.running(false));
        m.advance(now + Duration::from_millis(181));
        assert_eq!(m.value(), RAIL);
        assert!(!m.running(true));
        m.retarget(SIDEBAR, now, false);
        assert_eq!(m.value(), SIDEBAR);
        assert!(!m.running(true));
    }
    #[test]
    fn interrupted_motion_is_continuous_and_clamped() {
        let now = Instant::now();
        let mut m = SidebarMotion::new(SIDEBAR);
        m.retarget(RAIL, now, true);
        m.advance(now + Duration::from_millis(60));
        let middle = m.value();
        assert!(middle > RAIL && middle < SIDEBAR);
        m.retarget(SIDEBAR, now + Duration::from_millis(60), true);
        assert_eq!(m.value(), middle);
        m.advance(now + Duration::from_secs(1));
        assert_eq!(m.value(), SIDEBAR);
        m.snap(-100.0);
        assert_eq!(m.value(), RAIL);
        m.snap(1000.0);
        assert_eq!(m.value(), SIDEBAR);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn luminance(color: Color) -> f32 {
        fn linear(v: f32) -> f32 {
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
    }
    #[test]
    fn primary_enabled_label_has_accessible_contrast() {
        let style = primary(&Theme::Dark, button::Status::Active);
        let Some(iced::Background::Color(fill)) = style.background else {
            panic!("primary fill required");
        };
        let contrast = (luminance(fill) + 0.05) / (luminance(style.text_color) + 0.05);
        assert!(contrast >= 4.5);
    }
    #[test]
    fn attachment_cards_reserve_their_own_box_and_the_stack_gap() {
        // No cards, no height, and a file-only message has no gap after the last
        // card because `.userStack` gaps sit between children.
        assert_eq!(attachment_height(0, true), 0.0);
        assert_eq!(attachment_height(1, false), ATTACHMENT_CARD_HEIGHT);
        // One card above a bubble is the card plus one 8px gap: 72px.
        assert_eq!(attachment_height(1, true), 72.0);
        // Two cards add one more card and one more gap: 64 + 8 + 64 + 8.
        assert_eq!(attachment_height(2, true), 144.0);
        assert_eq!(attachment_height(2, false), 136.0);
    }
    #[test]
    fn ghost_does_not_fill_idle_secondary_actions() {
        assert!(
            ghost(&Theme::Dark, button::Status::Active)
                .background
                .is_none()
        );
        assert!(
            ghost(&Theme::Dark, button::Status::Hovered)
                .background
                .is_some()
        );
        assert_eq!(
            primary(&Theme::Dark, button::Status::Disabled).text_color,
            MUTED
        );
    }
    #[test]
    fn chat_width_matches_the_web_clamp_with_a_bounded_fallback() {
        // Narrow columns sit on the 680 floor, wide ones on the 920 cap.
        assert_eq!(chat_width(780.0), MIN_CHAT);
        assert_eq!(chat_width(2300.0), MAX_CHAT);
        // 64% governs the middle band.
        let middle = chat_width(1180.0);
        assert!(middle > MIN_CHAT && middle < MAX_CHAT);
        assert!((middle - 755.2).abs() < 0.05, "mid-band width {middle}");
        // Boundaries: the floor ends at 680/0.64 and the cap at 920/0.64.
        assert_eq!(chat_width(1062.0), MIN_CHAT);
        assert_eq!(chat_width(1438.0), MAX_CHAT);
        // Malformed or non-positive columns fall back to the shipped default.
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, -1.0] {
            assert_eq!(chat_width(value), CHAT, "column {value}");
        }
    }
    #[test]
    fn a_set_width_preference_wins_and_is_clamped_like_the_web() {
        // Mirrors resolveContentWidth: the bound is max(CONTENT_MIN, column - EDGE_BUDGET).
        assert_eq!(chat_width_with(1200.0, Some(800.0)), 800.0);
        // Below the floor clamps up; beyond the pane budget clamps down.
        assert_eq!(chat_width_with(1200.0, Some(100.0)), CHAT_CONTENT_MIN);
        assert_eq!(
            chat_width_with(1200.0, Some(5000.0)),
            1200.0 - CHAT_EDGE_BUDGET
        );
        // A pane too narrow for the budget keeps the floor as both bounds.
        assert_eq!(chat_width_with(700.0, Some(900.0)), CHAT_CONTENT_MIN);
        // Unusable values are treated as absent, so the adaptive clamp still applies.
        for value in [f32::NAN, f32::INFINITY, 0.0, -4.0] {
            assert_eq!(
                chat_width_with(1200.0, Some(value)),
                chat_width(1200.0),
                "preference {value}"
            );
        }
        assert_eq!(chat_width_with(1200.0, None), chat_width(1200.0));
        // A malformed column ignores a set preference rather than inventing a bound.
        assert_eq!(chat_width_with(f32::NAN, Some(800.0)), CHAT);
    }
    #[test]
    fn the_composer_fade_runs_transparent_to_base_over_its_height() {
        let style = composer_fade(&Theme::Dark);
        let Some(iced::Background::Gradient(gradient::Gradient::Linear(linear))) = style.background
        else {
            panic!("the composer fade must be a linear gradient");
        };
        let stops: Vec<_> = linear.stops.iter().flatten().collect();
        assert_eq!(stops.len(), 2, "exactly two stops");
        assert_eq!(stops[0].offset, 0.0);
        assert_eq!(stops[0].color.a, 0.0, "fully transparent at the top");
        assert_eq!(stops[1].offset, 1.0);
        assert_eq!(stops[1].color, BASE, "opaque base at the bottom");
        // 180 degrees is downward, so the transparent stop is the upper one.
        assert_eq!(linear.angle, iced::Radians::from(Degrees(180.0)));
    }
    #[test]
    fn the_content_size_preference_derives_the_web_deltas() {
        // The default is the identity: nothing about the shipped layout moves
        // until a preference is actually set.
        assert_eq!(content_delta(CONTENT_FONT_DEFAULT), 0.0);
        assert_eq!(content_delta_secondary(CONTENT_FONT_DEFAULT), 0.0);
        assert_eq!(content_secondary(CONTENT_FONT_DEFAULT), 13.0);
        // The delta is the setting minus the default, at both ends.
        assert_eq!(content_delta(CONTENT_FONT_MIN), -3.0);
        assert_eq!(content_delta(CONTENT_FONT_MAX), 6.0);
        // Secondary steps down one at or below the default, two above it.
        assert_eq!(content_secondary(11.0), 10.0);
        assert_eq!(content_secondary(14.0), 13.0);
        assert_eq!(content_secondary(15.0), 13.0);
        assert_eq!(content_secondary(16.0), 14.0);
        assert_eq!(content_secondary(20.0), 18.0);
        assert_eq!(content_delta_secondary(20.0), 5.0);
        // Out-of-range and unusable values behave as the default, not as a clamp
        // to a size the controls cannot produce.
        for value in [f32::NAN, f32::INFINITY, 0.0, -4.0, 10.0, 21.0, 100.0] {
            assert_eq!(content_delta(value), 0.0, "size {value}");
            assert_eq!(content_secondary(value), 13.0, "size {value}");
        }
    }
    #[test]
    fn the_heading_ladder_matches_the_web_tokens_at_natives_type_step() {
        // The web publishes 21/19/18 and the body size from h4 down; native's type
        // base is one pixel above it, and one ladder serves every surface.
        assert_eq!(heading_size(1), 22.0);
        assert_eq!(heading_size(2), 20.0);
        assert_eq!(heading_size(3), 19.0);
        assert_eq!(heading_size(4), 15.0);
        assert_eq!(heading_size(6), 15.0);
    }
    #[test]
    fn dragging_outward_widens_by_twice_the_pointer_delta() {
        // The right handle widens when the pointer moves right, and each pointer
        // pixel counts twice because both centered edges move.
        assert_eq!(
            chat_width_drag(700.0, 100.0, 110.0, WidthSide::Right),
            720.0
        );
        // The left handle mirrors the sign, so dragging left is still outward.
        assert_eq!(chat_width_drag(700.0, 100.0, 90.0, WidthSide::Left), 720.0);
        // Inward shrinks, and a press that never moves changes nothing.
        assert_eq!(chat_width_drag(700.0, 100.0, 90.0, WidthSide::Right), 680.0);
        assert_eq!(
            chat_width_drag(700.0, 100.0, 100.0, WidthSide::Right),
            700.0
        );
        // The raw result is unclamped; resolving it applies the floor and budget.
        let dragged = chat_width_drag(900.0, 0.0, 400.0, WidthSide::Right);
        assert_eq!(dragged, 1700.0);
        assert_eq!(
            chat_width_with(1200.0, Some(dragged)),
            1200.0 - CHAT_EDGE_BUDGET
        );
    }
}
