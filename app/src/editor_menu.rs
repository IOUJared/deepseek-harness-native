//! The shell's editable-field context menu (`apps/desktop/src/main.ts:276-300`).
//!
//! The DMG shell installs a right-click menu of Electron *roles* for editable fields:
//! Undo, Redo, a separator, Cut, Copy, Paste, a separator, Select all — each enabled
//! from the edit flags Chromium reports, with `accelerator: ''` so no shortcut label is
//! drawn. Its `application` popup path (`:1018-1034`) is a different menu with a Delete
//! item; this module is the editable one.
use iced::widget::{Column, Space, button, container};
use iced::{Element, Length};

use super::{Message, label};
use crate::design;

/// One command the menu can send.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
}

/// A rendered entry: an item, or one of the shell's group separators.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    Separator,
    Item {
        label: &'static str,
        action: Action,
        enabled: bool,
    },
}

/// The shell's eight entries in order.
pub const ENTRIES: usize = 8;

/// The menu the shell shows on an editable field, with each item's enabled state.
/// Paste and Select all are always offered here: the shell enables them from Chromium's
/// edit flags, and Iced exposes no cheap equivalent for the clipboard half.
pub fn entries(can_undo: bool, can_redo: bool, has_selection: bool) -> [Entry; ENTRIES] {
    use Entry::{Item, Separator};
    [
        Item {
            label: "Undo",
            action: Action::Undo,
            enabled: can_undo,
        },
        Item {
            label: "Redo",
            action: Action::Redo,
            enabled: can_redo,
        },
        Separator,
        Item {
            label: "Cut",
            action: Action::Cut,
            enabled: has_selection,
        },
        Item {
            label: "Copy",
            action: Action::Copy,
            enabled: has_selection,
        },
        Item {
            label: "Paste",
            action: Action::Paste,
            enabled: true,
        },
        Separator,
        Item {
            label: "Select all",
            action: Action::SelectAll,
            enabled: true,
        },
    ]
}

/// The popup surface, styled like the app's other menus.
pub fn view(can_undo: bool, can_redo: bool, has_selection: bool) -> Element<'static, Message> {
    let mut items = Column::new().spacing(2);
    for entry in entries(can_undo, can_redo, has_selection) {
        let item: Element<'static, Message> = match entry {
            Entry::Separator => container(Space::new().height(1))
                .width(Length::Fill)
                .style(|_| container::Style {
                    background: Some(design::LINE.into()),
                    ..Default::default()
                })
                .into(),
            Entry::Item {
                label: text,
                action,
                enabled,
            } => button(label(text, if enabled { design::TEXT } else { design::MUTED }).size(13))
                .on_press_maybe(enabled.then_some(Message::EditorMenuAction(action)))
                .style(design::ghost)
                .padding([4, 8])
                .width(Length::Fill)
                .into(),
        };
        items = items.push(item);
    }
    container(items)
        .id("native-editor-menu")
        .width(180)
        .padding(6)
        .style(|_| design::card(design::SURFACE, 10.0))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_menu_matches_the_shell_entries_and_enablement() {
        let full = entries(true, true, true);
        let labels: Vec<_> = full
            .iter()
            .map(|entry| match entry {
                Entry::Separator => "--",
                Entry::Item { label, .. } => label,
            })
            .collect();
        assert_eq!(
            labels,
            [
                "Undo",
                "Redo",
                "--",
                "Cut",
                "Copy",
                "Paste",
                "--",
                "Select all"
            ]
        );
        // An editable field has no Delete entry, unlike the shell's application popup.
        assert!(!labels.contains(&"Delete"));
        for entry in full {
            if let Entry::Item { enabled, .. } = entry {
                assert!(enabled, "{entry:?}");
            }
        }
        // Undo/Redo follow the editor's own history; Cut/Copy need a selection.
        let empty = entries(false, false, false);
        let enabled: Vec<_> = empty
            .iter()
            .map(|entry| match entry {
                Entry::Separator => None,
                Entry::Item { label, enabled, .. } => Some((*label, *enabled)),
            })
            .flatten()
            .collect();
        assert_eq!(
            enabled,
            [
                ("Undo", false),
                ("Redo", false),
                ("Cut", false),
                ("Copy", false),
                ("Paste", true),
                ("Select all", true)
            ]
        );
        // And the popup itself builds at the popup id the fixture can name.
        let _ = view(true, true, true);
    }
}
