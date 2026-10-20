//! The composer's slot rule, pinned with Iced's own widgets.
//!
//! A rendered composer needs a real GPU renderer, so the production view cannot be
//! laid out in a unit test (`NATIVE-COMPOSER-CONTROLS.md` records that limit). What
//! this file can pin is the widget-tree behavior [`composer_shell`] exists for, using
//! a stand-in field: Iced reconciles a column positionally and drops the state of a
//! child whose slot is matched with a different widget, so a block appearing above
//! the card takes the text editor's focus with it — unless the card keeps one slot.
use super::*;
use iced::Size;
use iced::advanced::widget::{Id, Operation, Tree, operation};
use iced::advanced::{Layout, layout};
use iced::widget::text_editor::{self, TextEditor};

const FIELD: &str = "PUBLIC-composer-focus";

/// Puts focus in the named field.
struct Focus(Option<&'static str>);

impl Operation<()> for Focus {
    fn focusable(
        &mut self,
        id: Option<&Id>,
        _bounds: iced::Rectangle,
        state: &mut dyn operation::Focusable,
    ) {
        if id == self.0.map(Id::new).as_ref() {
            state.focus();
        }
    }
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<()>)) {
        operate(self);
    }
}

/// Reads whether the named field currently holds focus.
struct ReadsFocus(&'static str, bool);

impl Operation<()> for ReadsFocus {
    fn focusable(
        &mut self,
        id: Option<&Id>,
        _bounds: iced::Rectangle,
        state: &mut dyn operation::Focusable,
    ) {
        if id == Some(&Id::new(self.0)) {
            self.1 = state.is_focused();
        }
    }
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<()>)) {
        operate(self);
    }
}

type Standin<'a> = Element<'a, (), iced::Theme, ()>;

/// The composer card with a real text editor at the id the rule is about.
fn card(content: &text_editor::Content<()>) -> Standin<'_> {
    let editor: Standin<'_> = Element::new(TextEditor::new(content).id(FIELD));
    Element::new(FocusKeeper {
        content: container(editor).into(),
        on_press: (),
    })
}

fn metadata<'a>() -> Standin<'a> {
    Column::with_children(vec![Space::new().height(4.0).into()]).into()
}

fn menu_row<'a>() -> Standin<'a> {
    text("Commands").into()
}

/// The shape the composer used to build: each block is its own sibling of the card.
fn unstable(content: &text_editor::Content<()>, open: bool) -> Standin<'_> {
    let mut column = Column::new();
    if open {
        column = column.push(menu_row());
    }
    column
        .push(card(content))
        .push(Space::new().height(6.0))
        .push(metadata())
        .into()
}

/// The shape [`composer_shell`] builds: the blocks share one slot above the card.
fn stable(content: &text_editor::Content<()>, open: bool) -> Standin<'_> {
    let above: Standin<'_> = if open {
        Column::with_children(vec![menu_row()]).into()
    } else {
        Column::new().into()
    };
    composer_shell(above, card(content), metadata(), 6.0)
}

/// Focuses the field in a closed composer, opens the menu, and reports whether the
/// field still has the focus after the tree is reconciled and laid out again.
fn focus_survives_an_opening_menu(
    view: for<'a> fn(&'a text_editor::Content<()>, bool) -> Standin<'a>,
) -> bool {
    let content = text_editor::Content::<()>::with_text("PUBLIC draft");
    let limits = layout::Limits::new(Size::ZERO, Size::new(420.0, 320.0));

    let mut closed = view(&content, false);
    let mut tree = Tree::new(closed.as_widget());
    let node = closed.as_widget_mut().layout(&mut tree, &(), &limits);
    closed
        .as_widget_mut()
        .operate(&mut tree, Layout::new(&node), &(), &mut Focus(Some(FIELD)));

    let mut opened = view(&content, true);
    tree.diff(&opened);
    let node = opened.as_widget_mut().layout(&mut tree, &(), &limits);
    let mut probe = ReadsFocus(FIELD, false);
    opened
        .as_widget_mut()
        .operate(&mut tree, Layout::new(&node), &(), &mut probe);
    probe.1
}

/// The reported bug: typing `/` opens the completion menu, and the field it was typed
/// into stops receiving keys. The menu is a block above the card, so pushing it as a
/// sibling rebuilt the card's subtree and dropped the editor's own focus state.
#[test]
fn a_block_inserted_as_a_sibling_rebuilds_the_card_and_loses_the_caret() {
    assert!(
        !focus_survives_an_opening_menu(unstable),
        "the stand-in must reproduce the sibling-insertion focus loss, or the rule below proves nothing"
    );
}

/// The fix: one always-present slot for every block above the card keeps the card's
/// tree index, so opening the menu is a layout change and nothing else.
#[test]
fn the_composer_shell_keeps_the_caret_when_a_menu_appears_above_the_card() {
    assert!(
        focus_survives_an_opening_menu(stable),
        "the completion menu must not take the editor's focus"
    );
}
