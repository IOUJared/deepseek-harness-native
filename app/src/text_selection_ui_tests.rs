//! Transcript text selection: the copy buffer, the highlight runs and the block
//! numbering. The drag itself needs a rendered window, so the pointer-to-offset half
//! is verified by the real-window pass recorded in `docs/NATIVE-CHAT-FEATURES.md`.
use super::text_selection::{self, Piece, Selection};
use super::*;

fn selection(piece: Piece, text: &str, start: usize, end: usize) -> Selection {
    Selection {
        piece,
        start,
        end,
        text: text[start..end].to_owned(),
    }
}

fn standard() -> (App, tokio::sync::mpsc::Receiver<Command>) {
    super::conversation_actions_ui_tests::app()
}

#[test]
fn a_transcript_drag_is_stored_and_reaching_for_the_composer_clears_it() {
    let (mut app, mut queue) = standard();
    let piece = Piece { row: 9, block: 2 };
    let stored = selection(piece, "PUBLIC selected words", 0, 6);
    drop(app.update(Message::TextSelect(Some(stored.clone()))));
    assert_eq!(app.text_selection.as_ref(), Some(&stored));
    // Reaching into the composer is what ends a transcript selection, the way a press
    // in a browser's text field ends the page's.
    let task = app.update(Message::ComposerFocus);
    assert!(app.text_selection.is_none());
    assert!(task.units() >= 1, "the composer still takes the caret");
    // The selection itself is local state: it never becomes a Host command.
    assert!(queue.try_recv().is_err());
}

#[test]
fn ctrl_c_answers_for_a_transcript_selection_and_only_then() {
    let (mut app, _) = standard();
    let c = keyboard::Key::Character("c".into());
    // No selection: the key stays with the field and binding chain, untouched.
    assert!(app.transcript_copy(&c, keyboard::Modifiers::CTRL).is_none());
    let stored = selection(Piece { row: 4, block: 0 }, "copy this sentence", 5, 16);
    drop(app.update(Message::TextSelect(Some(stored))));
    assert!(
        app.transcript_copy(&c, keyboard::Modifiers::CTRL).is_some(),
        "a live transcript selection must reach the clipboard"
    );
    // A bare "c", Shift+C and Alt+C are not the copy chord.
    assert!(
        app.transcript_copy(&c, keyboard::Modifiers::empty())
            .is_none()
    );
    assert!(
        app.transcript_copy(&c, keyboard::Modifiers::CTRL | keyboard::Modifiers::SHIFT)
            .is_none()
    );
    assert!(
        app.transcript_copy(&c, keyboard::Modifiers::CTRL | keyboard::Modifiers::ALT)
            .is_none()
    );
    // A cleared selection stops answering, so the field's own copy resumes.
    drop(app.update(Message::TextSelect(None)));
    assert!(app.transcript_copy(&c, keyboard::Modifiers::CTRL).is_none());
}

#[test]
fn a_new_drag_replaces_the_previous_selection_and_an_empty_one_clears_it() {
    let (mut app, _) = standard();
    let first = selection(Piece { row: 1, block: 0 }, "first block", 0, 5);
    drop(app.update(Message::TextSelect(Some(first))));
    let second = selection(Piece { row: 2, block: 3 }, "second block", 7, 12);
    drop(app.update(Message::TextSelect(Some(second.clone()))));
    assert_eq!(app.text_selection.as_ref(), Some(&second));
    drop(app.update(Message::TextSelect(None)));
    assert!(app.text_selection.is_none());
}

#[test]
fn a_reversed_drag_range_reads_in_text_order() {
    let reversed = Selection {
        piece: Piece { row: 1, block: 0 },
        start: 12,
        end: 3,
        text: String::new(),
    };
    assert_eq!(reversed.range(), 3..12);
}

#[test]
fn the_highlight_splits_only_the_selected_bytes_and_only_in_its_own_block() {
    let runs = || vec![iced::widget::span("hello "), iced::widget::span("world")];
    let piece = Piece { row: 3, block: 1 };
    let selected =
        text_selection::highlighted(runs(), Some(&selection(piece, "hello world", 3, 8)), piece);
    // The runs stay split by their own styles: "lo " keeps the body run, "wo" the bold
    // one, and only those two carry the tint.
    let texts: Vec<&str> = selected.iter().map(|run| run.text.as_ref()).collect();
    assert_eq!(texts, ["hel", "lo ", "wo", "rld"]);
    assert!(selected[0].highlight.is_none() && selected[3].highlight.is_none());
    assert!(selected[1].highlight.is_some() && selected[2].highlight.is_some());
    assert_eq!(
        selected
            .iter()
            .map(|run| run.text.as_ref())
            .collect::<String>(),
        "hello world"
    );
    // The same range in any other block is not this block's selection.
    let other = text_selection::highlighted(
        runs(),
        Some(&selection(piece, "hello world", 3, 8)),
        Piece { row: 3, block: 2 },
    );
    assert_eq!(
        other
            .iter()
            .map(|run| run.text.as_ref())
            .collect::<String>(),
        "hello world"
    );
    assert!(other.iter().all(|run| run.highlight.is_none()));
}

#[test]
fn a_block_number_is_the_rows_own_and_survives_a_hidden_row_above_it() {
    let top = super::conversation_content::Fields::for_row(41, None);
    let bottom = super::conversation_content::Fields::for_row(42, None);
    assert_eq!(top.claim(), 0);
    assert_eq!(top.claim(), 1);
    assert_eq!(bottom.claim(), 0);
    assert_eq!(top.piece_of(1), Piece { row: 41, block: 1 });
    assert_ne!(top.piece_of(1), bottom.piece_of(1));
}
