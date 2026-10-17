//! The rebinding store's logic, read from the reference rather than invented.
//!
//! `ui-shortcuts/src/client/Editor.tsx` defines every part this needs:
//! - an edit is `{ type: 'set', id, binding }` or `{ type: 'reset', id }`, and `set` with a null
//!   binding is the **clear** (`:165-167`, where reset and clear are separate actions);
//! - a save carries the **revision it reviewed** (`:48`) and a **stale** result re-arms the exact
//!   binding for a retry (`:57`, `:69-71`, `:179`), so two editors cannot silently overwrite each
//!   other;
//! - the result's success case is a saved status; every other case is reported against the
//!   command **catalog** and the **fixed** built-ins (`:57`);
//! - `readonly` is first-class (`:165`), so a locked environment still shows its bindings.
//!
//! Nothing here touches input handling yet: this is the store the editor will write through.

use std::collections::BTreeMap;

/// One captured binding: a code, an optional second code for a chord, and the modifiers held.
/// The shape is the editor's own `pending` value (`Editor.tsx:119`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub code: String,
    pub second_code: Option<String>,
    pub modifiers: Vec<String>,
}

/// A command's binding: a default the reference ships, and whatever the user has set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub default: Option<Binding>,
    /// `None` means cleared, which is distinct from absent: the reference can clear a binding
    /// without losing the default it resets to.
    pub binding: Option<Binding>,
}

/// What an editor asks the store to do (`ShortcutEdit`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edit {
    Set {
        id: String,
        binding: Option<Binding>,
    },
    Reset {
        id: String,
    },
}

/// The store's answer. `Saved` carries the new revision; `Stale` re-arms the edit so the editor
/// can retry it (`:57`, `:69-71`); `Unknown` names a command the catalog does not carry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Saved { revision: u64 },
    Stale { revision: u64, retry: Edit },
    Unknown { id: String },
}

/// The bindings of one profile, keyed by command id, with the revision that guards writes.
#[derive(Clone, Debug, Default)]
pub struct Store {
    entries: BTreeMap<String, Entry>,
    revision: u64,
}

impl Store {
    /// A store holding a catalog's defaults at revision zero.
    pub fn new(defaults: impl IntoIterator<Item = (String, Option<Binding>)>) -> Self {
        Self {
            entries: defaults
                .into_iter()
                .map(|(id, default)| {
                    (
                        id,
                        Entry {
                            default: default.clone(),
                            binding: default,
                        },
                    )
                })
                .collect(),
            revision: 0,
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn binding(&self, id: &str) -> Option<Option<&Binding>> {
        self.entries.get(id).map(|entry| entry.binding.as_ref())
    }

    /// Every command the catalog carries, with its effective binding — the list an editor draws.
    pub fn bindings(&self) -> impl Iterator<Item = (&str, Option<&Binding>)> {
        self.entries
            .iter()
            .map(|(id, entry)| (id.as_str(), entry.binding.as_ref()))
    }

    /// Apply an edit against the revision the editor reviewed. A mismatch changes nothing and
    /// returns the edit to retry: the same staleness rule the reference's editor implements.
    pub fn apply(&mut self, edit: Edit, reviewed_revision: u64) -> Outcome {
        if reviewed_revision != self.revision {
            return Outcome::Stale {
                revision: self.revision,
                retry: edit,
            };
        }
        let id = match &edit {
            Edit::Set { id, .. } | Edit::Reset { id } => id.clone(),
        };
        let Some(entry) = self.entries.get_mut(&id) else {
            return Outcome::Unknown { id };
        };
        match edit {
            Edit::Set { binding, .. } => entry.binding = binding,
            Edit::Reset { .. } => entry.binding = entry.default.clone(),
        }
        self.revision += 1;
        Outcome::Saved {
            revision: self.revision,
        }
    }

    /// The default a reset would restore, for comparison and for the editor's own display.
    pub fn default_of(&self, id: &str) -> Option<&Binding> {
        self.entries
            .get(id)
            .and_then(|entry| entry.default.as_ref())
    }

    /// A binding already held by another command, which the reference reports as a conflict.
    pub fn conflict(&self, id: &str, binding: &Binding) -> Option<&str> {
        self.entries
            .iter()
            .find(|(other, entry)| other.as_str() != id && entry.binding.as_ref() == Some(binding))
            .map(|(other, _)| other.as_str())
    }
}

/// The DOM-style codes the reference captures and displays (`Editor.tsx:119`, and the codes in
/// `fixed.ts`: `ArrowUp`, `Enter`, `Escape`). Iced reports a character or a named key, so this
/// maps between them — including the punctuation names, which are the ones a binding for `/`
/// depends on (`shortcuts.open` is `primary + Slash`).
pub fn code_for(key: &iced::keyboard::Key) -> Option<String> {
    use iced::keyboard::key::Named;
    let named = |name: &str| Some(name.to_owned());
    match key {
        iced::keyboard::Key::Named(Named::Enter) => named("Enter"),
        iced::keyboard::Key::Named(Named::Escape) => named("Escape"),
        iced::keyboard::Key::Named(Named::Space) => named("Space"),
        iced::keyboard::Key::Named(Named::Tab) => named("Tab"),
        iced::keyboard::Key::Named(Named::Backspace) => named("Backspace"),
        iced::keyboard::Key::Named(Named::Delete) => named("Delete"),
        iced::keyboard::Key::Named(Named::ArrowUp) => named("ArrowUp"),
        iced::keyboard::Key::Named(Named::ArrowDown) => named("ArrowDown"),
        iced::keyboard::Key::Named(Named::ArrowLeft) => named("ArrowLeft"),
        iced::keyboard::Key::Named(Named::ArrowRight) => named("ArrowRight"),
        iced::keyboard::Key::Named(Named::Home) => named("Home"),
        iced::keyboard::Key::Named(Named::End) => named("End"),
        iced::keyboard::Key::Named(Named::PageUp) => named("PageUp"),
        iced::keyboard::Key::Named(Named::PageDown) => named("PageDown"),
        iced::keyboard::Key::Character(text) => match text.as_str() {
            "/" => named("Slash"),
            "." => named("Period"),
            "," => named("Comma"),
            ";" => named("Semicolon"),
            "'" => named("Quote"),
            "[" => named("BracketLeft"),
            "]" => named("BracketRight"),
            "-" => named("Minus"),
            "=" => named("Equal"),
            "`" => named("Backquote"),
            "\\" => named("Backslash"),
            other if other.chars().count() == 1 => {
                let letter = other.to_uppercase();
                Some(format!("Key{letter}"))
            }
            _ => None,
        },
        _ => None,
    }
}

/// A binding from a key press: the code plus the modifiers the reference records. `primary` is
/// the reference's own name for the platform's main modifier (`Editor.tsx:119`), which is what a
/// default such as `primary + Slash` needs.
pub fn binding_from(
    key: &iced::keyboard::Key,
    modifiers: iced::keyboard::Modifiers,
) -> Option<Binding> {
    let code = code_for(key)?;
    let mut held = Vec::new();
    if modifiers.control() || modifiers.command() {
        held.push("primary".to_owned());
    }
    if modifiers.alt() {
        held.push("alt".to_owned());
    }
    if modifiers.shift() {
        held.push("shift".to_owned());
    }
    Some(Binding {
        code,
        second_code: None,
        modifiers: held,
    })
}

/// Render a binding for display: modifier glyphs then the code, joined with `+` — the reference's
/// own "Ctrl+B" (`locales.ts:64` and the row's `keys`). A code loses its `Key` prefix for display,
/// as a DOM code does.
pub fn display(binding: &Binding) -> String {
    let mut parts: Vec<String> = binding
        .modifiers
        .iter()
        .map(|modifier| match modifier.as_str() {
            "primary" => "Ctrl".to_owned(),
            "alt" => "Alt".to_owned(),
            "shift" => "Shift".to_owned(),
            other => other.to_owned(),
        })
        .collect();
    parts.push(display_code(&binding.code));
    if let Some(second) = &binding.second_code {
        parts.push(display_code(second));
    }
    parts.join("+")
}

fn display_code(code: &str) -> String {
    code.strip_prefix("Key").unwrap_or(code).to_owned()
}

/// The four groups in the reference's **product order**
/// (`ui-shortcuts/src/client/Reference.tsx:179`).
pub const GROUPS: [&str; 4] = ["application", "input", "menus", "approval"];

/// The reference's own English copy, quoted rather than paraphrased
/// (`ui-shortcuts/src/client/locales.ts:47-64`).
pub const COPY_RESET: &str = "Restore default";
pub const COPY_CLEAR: &str = "Remove";
pub const COPY_MODIFIED: &str = "Modified";
pub const COPY_RETRY_SAVE: &str = "Retry save";
pub const COPY_MODIFIED_COUNT: &str = "{count} customized";
pub const COPY_RESET_ALL: &str = "Restore all defaults";
pub const COPY_RESET_TITLE: &str = "Restore all default shortcuts?";
pub const COPY_RESET_DESCRIPTION: &str = "Restore the default shortcuts for this platform. All modified or removed shortcuts will be restored. Other platforms are unaffected.";
pub const COPY_RESET_FAILED: &str =
    "Could not restore defaults. Your shortcuts are unchanged. Please retry.";
pub const COPY_CANCEL: &str = "Cancel";
pub const COPY_OPEN_FROM_ANYWHERE: &str = "Open from anywhere";

/// One drawable row: a command's id and label, the keys it answers to, its group, whether the
/// reference marks it core, and whether the user has changed it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: String,
    pub label: String,
    pub keys: String,
    pub group: &'static str,
    pub core: bool,
    pub modified: bool,
}

/// Build the rows for a catalog: **core actions first, then other commands by id within each
/// group** (`Reference.tsx:67`), groups in product order.
pub fn rows(store: &Store, catalog: &[(String, String, bool)]) -> Vec<Row> {
    let mut rows: Vec<Row> = catalog
        .iter()
        .map(|(id, label, core)| Row {
            id: id.clone(),
            label: label.clone(),
            keys: store
                .binding(id)
                .flatten()
                .map(describe)
                .unwrap_or_default(),
            // A command's group is the catalog's business; without one it reads as application.
            group: "application",
            core: *core,
            modified: store.binding(id).flatten() != store.default_of(id),
        })
        .collect();
    rows.sort_by(|left, right| {
        let group = group_rank(left.group).cmp(&group_rank(right.group));
        group
            .then(right.core.cmp(&left.core))
            .then(left.id.cmp(&right.id))
    });
    rows
}

/// The reference's search index for a row, as far as its labels are readable from here: the keys
/// joined with `+` and joined without (`Reference.tsx:137`). The aliases and the `aria` variant
/// need the catalog's own strings, which this app does not carry yet.
pub fn search_index(row: &Row) -> Vec<String> {
    vec![
        row.keys.replace(' ', "+"),
        // The reference's second form drops the separator as well as the spaces
        // (`Reference.tsx:137`: `keys.filter(key => key !== '+').join('')`).
        row.keys.replace([' ', '+'], ""),
        row.id.clone(),
    ]
}

/// How many commands the user has changed — the reference's "{{count}} customized" input.
pub fn modified_count(store: &Store) -> usize {
    store
        .bindings()
        .filter(|(id, binding)| *binding != store.default_of(id))
        .count()
}

/// Render a binding the way a row shows it: modifiers then codes, joined with `+`.
fn describe(binding: &Binding) -> String {
    let mut parts = binding.modifiers.clone();
    parts.push(binding.code.clone());
    if let Some(second) = &binding.second_code {
        parts.push(second.clone());
    }
    parts.join("+")
}

fn group_rank(group: &str) -> usize {
    GROUPS.iter().position(|name| *name == group).unwrap_or(0)
}

/// The built-ins the reference fixes rather than reading from the catalog
/// (`ui-shortcuts/src/client/fixed.ts:10-16`): three menu keys, each with a display glyph and a
/// code, in the `menus` group. Their ids are prefixed `fixed.` in the reference so an editor can
/// tell them from catalog commands, and they cannot be rebound.
pub fn fixed() -> [(&'static str, &'static str, &'static str); 3] {
    [
        ("fixed.move", "↑/↓", "ArrowUp/ArrowDown"),
        ("fixed.select", "Enter", "Enter"),
        ("fixed.dismiss", "Esc", "Escape"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(code: &str) -> Binding {
        Binding {
            code: code.to_owned(),
            second_code: None,
            modifiers: vec!["primary".to_owned()],
        }
    }

    fn store() -> Store {
        Store::new([
            ("send".to_owned(), Some(binding("Enter"))),
            ("palette".to_owned(), Some(binding("Slash"))),
        ])
    }

    #[test]
    fn a_set_at_the_reviewed_revision_saves_and_bumps_the_revision() {
        let mut store = store();
        let outcome = store.apply(
            Edit::Set {
                id: "palette".to_owned(),
                binding: Some(binding("KeyK")),
            },
            0,
        );
        assert_eq!(outcome, Outcome::Saved { revision: 1 });
        assert_eq!(store.binding("palette"), Some(Some(&binding("KeyK"))));
    }

    #[test]
    fn a_set_with_a_null_binding_clears_without_losing_the_default() {
        let mut store = store();
        assert!(matches!(
            store.apply(
                Edit::Set {
                    id: "palette".to_owned(),
                    binding: None
                },
                0
            ),
            Outcome::Saved { .. }
        ));
        assert_eq!(store.binding("palette"), Some(None));
        // The clear is not the reset: the default is still there to return to.
        assert!(matches!(
            store.apply(
                Edit::Reset {
                    id: "palette".to_owned()
                },
                1
            ),
            Outcome::Saved { .. }
        ));
        assert_eq!(store.binding("palette"), Some(Some(&binding("Slash"))));
    }

    #[test]
    fn a_stale_edit_changes_nothing_and_returns_itself_to_retry() {
        let mut store = store();
        let edit = Edit::Set {
            id: "palette".to_owned(),
            binding: Some(binding("KeyK")),
        };
        let outcome = store.apply(edit.clone(), 7);
        assert_eq!(
            outcome,
            Outcome::Stale {
                revision: 0,
                retry: edit
            }
        );
        assert_eq!(store.binding("palette"), Some(Some(&binding("Slash"))));
        assert_eq!(store.revision(), 0);
    }

    #[test]
    fn an_edit_for_a_command_outside_the_catalog_is_named_not_ignored() {
        let mut store = store();
        let outcome = store.apply(
            Edit::Reset {
                id: "missing".to_owned(),
            },
            0,
        );
        assert_eq!(
            outcome,
            Outcome::Unknown {
                id: "missing".to_owned()
            }
        );
        assert_eq!(store.revision(), 0);
    }

    #[test]
    fn rows_put_core_first_then_id_within_a_group_in_product_order() {
        let store = store();
        let catalog = vec![
            ("zebra".to_owned(), "Zebra".to_owned(), false),
            ("send".to_owned(), "Send".to_owned(), true),
            ("alpha".to_owned(), "Alpha".to_owned(), false),
        ];
        let rows = rows(&store, &catalog);
        let ids: Vec<&str> = rows.iter().map(|row| row.id.as_str()).collect();
        // Core first, then the rest by id — not catalog order.
        assert_eq!(ids, vec!["send", "alpha", "zebra"]);
        assert_eq!(GROUPS.len(), 4);
    }

    #[test]
    fn a_row_describes_its_binding_and_counts_as_modified_only_when_it_differs() {
        let mut store = store();
        let catalog = vec![("palette".to_owned(), "Palette".to_owned(), true)];
        let before = rows(&store, &catalog);
        assert_eq!(before[0].keys, "primary+Slash");
        assert!(!before[0].modified);
        assert_eq!(modified_count(&store), 0);
        store.apply(
            Edit::Set {
                id: "palette".to_owned(),
                binding: Some(binding("KeyK")),
            },
            0,
        );
        let after = rows(&store, &catalog);
        assert_eq!(after[0].keys, "primary+KeyK");
        assert!(after[0].modified);
        assert_eq!(modified_count(&store), 1);
        // A clear counts as modified too: absent is not the default.
        store.apply(
            Edit::Set {
                id: "palette".to_owned(),
                binding: None,
            },
            1,
        );
        assert_eq!(modified_count(&store), 1);
    }

    #[test]
    fn the_search_index_carries_both_joinings_and_the_id() {
        let store = store();
        let catalog = vec![("palette".to_owned(), "Palette".to_owned(), true)];
        let row = rows(&store, &catalog).remove(0);
        let index = search_index(&row);
        assert!(index.contains(&"primary+Slash".to_owned()));
        assert!(index.contains(&"primarySlash".to_owned()));
        assert!(index.contains(&"palette".to_owned()));
    }

    #[test]
    fn the_copy_is_the_reference_strings_verbatim() {
        assert_eq!(COPY_RESET, "Restore default");
        assert_eq!(COPY_CLEAR, "Remove");
        assert_eq!(COPY_MODIFIED, "Modified");
        assert_eq!(COPY_RETRY_SAVE, "Retry save");
        assert_eq!(COPY_MODIFIED_COUNT, "{count} customized");
        assert_eq!(COPY_RESET_ALL, "Restore all defaults");
        assert_eq!(COPY_CANCEL, "Cancel");
        assert_eq!(COPY_OPEN_FROM_ANYWHERE, "Open from anywhere");
        assert!(
            COPY_RESET_DESCRIPTION.starts_with("Restore the default shortcuts for this platform.")
        );
        assert!(COPY_RESET_FAILED.ends_with("Please retry."));
    }

    #[test]
    fn a_key_press_becomes_the_reference_code_and_modifiers() {
        use iced::keyboard::key::Named;
        use iced::keyboard::{Key, Modifiers};
        let ctrl = Modifiers::CTRL;
        let binding = binding_from(&Key::Character("b".into()), ctrl).unwrap();
        assert_eq!(binding.code, "KeyB");
        assert_eq!(binding.modifiers, vec!["primary".to_owned()]);
        assert_eq!(display(&binding), "Ctrl+B");
        // The slash case the trigger depends on.
        let slash = binding_from(&Key::Character("/".into()), ctrl).unwrap();
        assert_eq!(slash.code, "Slash");
        assert_eq!(display(&slash), "Ctrl+Slash");
        // Named keys keep their own names, and shift is recorded rather than folded into the code.
        let enter = binding_from(&Key::Named(Named::Enter), Modifiers::SHIFT).unwrap();
        assert_eq!(enter.code, "Enter");
        assert_eq!(display(&enter), "Shift+Enter");
        let escape = binding_from(&Key::Named(Named::Escape), Modifiers::empty()).unwrap();
        assert_eq!(display(&escape), "Escape");
        let up = binding_from(&Key::Named(Named::ArrowUp), Modifiers::empty()).unwrap();
        assert_eq!(display(&up), "ArrowUp");
    }

    #[test]
    fn the_fixed_built_ins_are_the_reference_three() {
        let fixed = fixed();
        assert_eq!(fixed.len(), 3);
        assert_eq!(fixed[0].0, "fixed.move");
        assert_eq!(fixed[1].2, "Enter");
        assert_eq!(fixed[2].2, "Escape");
        // A fixed id is outside any catalog, so the store refuses an edit that names one.
        let mut store = store();
        assert!(matches!(
            store.apply(
                Edit::Reset {
                    id: "fixed.select".to_owned()
                },
                0
            ),
            Outcome::Unknown { .. }
        ));
    }

    #[test]
    fn a_binding_held_by_another_command_is_reported_as_a_conflict() {
        let store = store();
        assert_eq!(store.conflict("send", &binding("Slash")), Some("palette"));
        assert_eq!(store.conflict("send", &binding("Enter")), None);
    }
}
