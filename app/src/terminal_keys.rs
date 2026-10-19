//! The bytes one key stands for, so a native terminal can send what the reference's key handler
//! sends (`ui-sidebar-terminal/src/client/terminal.tsx:101`: `xterm.onData` → the Host).
//!
//! Native has no emulator, so the mapping is explicit rather than inferred: named keys become their
//! escape sequences, Ctrl combinations become control bytes, and a printable character is itself.
//! Anything this table does not name is not sent, which is the honest answer — guessing a sequence
//! would put bytes in the shell that the user never pressed.

/// The bytes one key press sends, or `None` when this app does not translate that key.
pub fn bytes(key: &iced::keyboard::Key, modifiers: iced::keyboard::Modifiers) -> Option<String> {
    use iced::keyboard::key::{Key, Named};
    match key {
        Key::Character(text) => {
            // Ctrl turns a letter into its control byte (Ctrl-C is 0x03); otherwise the character is
            // sent as typed.
            if modifiers.control() {
                let mut chars = text.chars();
                let (Some(ch), None) = (chars.next(), chars.next()) else {
                    return None;
                };
                let upper = ch.to_ascii_uppercase();
                if upper.is_ascii_alphabetic() {
                    // The control byte itself, not its decimal spelling: Ctrl-C is 0x03, and a
                    // `to_string()` on the integer would have sent the two characters "3".
                    return Some(char::from((upper as u8) & 0x1f).to_string());
                }
                return None;
            }
            Some(text.to_string())
        }
        Key::Named(named) => Some(
            match named {
                Named::Enter => "\r",
                Named::Tab => "\t",
                Named::Escape => "\x1b",
                Named::Backspace => "\x7f",
                Named::Delete => "\x1b[3~",
                Named::ArrowUp => "\x1b[A",
                Named::ArrowDown => "\x1b[B",
                Named::ArrowRight => "\x1b[C",
                Named::ArrowLeft => "\x1b[D",
                Named::Home => "\x1b[H",
                Named::End => "\x1b[F",
                Named::PageUp => "\x1b[5~",
                Named::PageDown => "\x1b[6~",
                _ => return None,
            }
            .to_owned(),
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    //! The mapping the panel sends, keystroke by keystroke.
    use super::*;
    use iced::keyboard::Modifiers;
    use iced::keyboard::key::{Key, Named};

    fn named(named: Named) -> Key {
        Key::Named(named)
    }

    #[test]
    fn named_keys_become_their_escape_sequences() {
        for (key, expected) in [
            (Named::Enter, "\r"),
            (Named::Tab, "\t"),
            (Named::Escape, "\x1b"),
            (Named::Backspace, "\x7f"),
            (Named::ArrowUp, "\x1b[A"),
            (Named::ArrowLeft, "\x1b[D"),
            (Named::Delete, "\x1b[3~"),
        ] {
            assert_eq!(
                bytes(&named(key), Modifiers::default()).as_deref(),
                Some(expected),
                "{key:?}"
            );
        }
    }

    #[test]
    fn a_character_is_sent_as_typed_and_ctrl_makes_a_control_byte() {
        assert_eq!(
            bytes(&Key::Character("a".into()), Modifiers::default()).as_deref(),
            Some("a")
        );
        assert_eq!(
            bytes(&Key::Character("c".into()), Modifiers::CTRL).as_deref(),
            Some("\u{3}"),
            "Ctrl-C interrupts a shell"
        );
        assert_eq!(
            bytes(&Key::Character("d".into()), Modifiers::CTRL).as_deref(),
            Some("\u{4}"),
            "Ctrl-D ends input"
        );
        // A key this app does not translate sends nothing rather than a guess.
        assert!(bytes(&named(Named::F5), Modifiers::default()).is_none());
    }
}
