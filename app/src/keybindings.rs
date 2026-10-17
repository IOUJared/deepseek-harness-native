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
    /// The bindings the reference ships for this command — a list, because `move` carries both
    /// arrow keys (`fixed.ts:12`) and native's own list prints "Enter / Ctrl+Enter" for send.
    pub default: Vec<Binding>,
    /// Empty means cleared, which is distinct from absent: the reference can clear a binding
    /// without losing the defaults it resets to.
    pub bindings: Vec<Binding>,
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
    /// Every command back to its default; the confirmation's subject.
    ResetAll,
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
    pub fn new(defaults: impl IntoIterator<Item = (String, Vec<Binding>)>) -> Self {
        Self {
            entries: defaults
                .into_iter()
                .map(|(id, default)| {
                    (
                        id,
                        Entry {
                            bindings: default.clone(),
                            default,
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

    pub fn bindings(&self, id: &str) -> Option<&[Binding]> {
        self.entries.get(id).map(|entry| entry.bindings.as_slice())
    }

    /// Every command the catalog carries, with its effective binding — the list an editor draws.
    pub fn all(&self) -> impl Iterator<Item = (&str, &[Binding])> {
        self.entries
            .iter()
            .map(|(id, entry)| (id.as_str(), entry.bindings.as_slice()))
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
            // The all-command form never reaches the single-command path.
            Edit::ResetAll => return self.reset_all(reviewed_revision),
        };
        let Some(entry) = self.entries.get_mut(&id) else {
            return Outcome::Unknown { id };
        };
        match edit {
            Edit::Set { binding, .. } => {
                entry.bindings = binding.into_iter().collect();
            }
            Edit::Reset { .. } => entry.bindings = entry.default.clone(),
            Edit::ResetAll => unreachable!("handled before the catalog lookup"),
        }
        self.revision += 1;
        Outcome::Saved {
            revision: self.revision,
        }
    }

    /// Restore **every** command to its default — the action behind "Restore all defaults" and
    /// its confirmation, whose own copy promises that "all modified or removed shortcuts will be
    /// restored" (`locales.ts:49-50`). It carries the same revision rule as a single edit: a stale
    /// caller changes nothing and is handed the form to retry.
    pub fn reset_all(&mut self, reviewed_revision: u64) -> Outcome {
        if reviewed_revision != self.revision {
            return Outcome::Stale {
                revision: self.revision,
                retry: Edit::ResetAll,
            };
        }
        for entry in self.entries.values_mut() {
            entry.bindings = entry.default.clone();
        }
        self.revision += 1;
        Outcome::Saved {
            revision: self.revision,
        }
    }

    /// The default a reset would restore, for comparison and for the editor's own display.
    pub fn default_of(&self, id: &str) -> Option<&[Binding]> {
        self.entries.get(id).map(|entry| entry.default.as_slice())
    }

    /// A binding already held by another command, which the reference reports as a conflict.
    pub fn conflict(&self, id: &str, binding: &Binding) -> Option<&str> {
        self.entries
            .iter()
            .find(|(other, entry)| {
                other.as_str() != id && entry.bindings.iter().any(|held| held == binding)
            })
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

/// Whether a press answers a stored binding. The comparison is by **code and modifiers**, and a
/// modifier the binding does not name must not be held: `primary + Slash` must not fire on
/// `primary + shift + Slash`. The reference's own default for the trigger is exactly that pair
/// (`ui-shortcuts/src/client/index.ts:42-52`: `primary + Slash` on every desktop platform).
pub fn matches(
    binding: &Binding,
    key: &iced::keyboard::Key,
    modifiers: iced::keyboard::Modifiers,
) -> bool {
    let Some(pressed) = binding_from(key, modifiers) else {
        return false;
    };
    binding.code == pressed.code && binding.modifiers == pressed.modifiers
}

/// The first command in the store whose binding answers this press — the lookup an input handler
/// performs before deciding whether a key is the app's or the composer's. `None` means the press
/// belongs to whatever has focus.
pub fn resolve<'a>(
    store: &'a Store,
    key: &iced::keyboard::Key,
    modifiers: iced::keyboard::Modifiers,
) -> Option<&'a str> {
    store
        .all()
        .find(|(_, list)| list.iter().any(|binding| matches(binding, key, modifiers)))
        .map(|(id, _)| id)
}

/// This app's own fixed bindings, with the ids and keys its input handling already implements and
/// its Settings row already prints (`Ctrl+B`, `Ctrl+F`, `Enter / Ctrl+Enter`, `Shift+Enter`,
/// `Escape`). They are prefixed `fixed.` like the reference's own fixed rows, so an editor can
/// tell them from catalog commands, and they are **not** the reference's three menu keys — its
/// fixed list is about its own surfaces, and this app's is about its own.
pub fn native_fixed() -> Vec<(String, Vec<Binding>)> {
    fn key(code: &str, modifiers: &[&str]) -> Binding {
        Binding {
            code: code.to_owned(),
            second_code: None,
            modifiers: modifiers.iter().map(|name| (*name).to_owned()).collect(),
        }
    }
    vec![
        (
            "fixed.navigation".to_owned(),
            vec![key("KeyB", &["primary"])],
        ),
        ("fixed.find".to_owned(), vec![key("KeyF", &["primary"])]),
        (
            // Send takes Enter and Ctrl+Enter, and the reference's own editor can only express
            // that now that a command holds a list.
            "fixed.send".to_owned(),
            vec![key("Enter", &[]), key("Enter", &["primary"])],
        ),
        ("fixed.newline".to_owned(), vec![key("Enter", &["shift"])]),
        ("fixed.close".to_owned(), vec![key("Escape", &[])]),
    ]
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
            keys: store.bindings(id).map(describe_all).unwrap_or_default(),
            // A command's group is the catalog's business; without one it reads as application.
            group: "application",
            core: *core,
            modified: store.bindings(id) != store.default_of(id),
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
        .all()
        .filter(|(id, list)| Some(*list) != store.default_of(id))
        .count()
}

/// Render a binding the way a row shows it: modifiers then codes, joined with `+`.
/// Render a command's whole list for a row: the reference prints "Enter / Ctrl+Enter" for a
/// command with a primary and an alternate binding, so the separator is its own.
fn describe_all(bindings: &[Binding]) -> String {
    // `display`, not `describe`: a row shows `Ctrl+B`, and only `display` maps the reference's
    // internal `primary` onto the platform's `Ctrl`.
    bindings.iter().map(display).collect::<Vec<_>>().join(" / ")
}

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
            ("send".to_owned(), vec![binding("Enter")]),
            ("palette".to_owned(), vec![binding("Slash")]),
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
        assert_eq!(
            store.bindings("palette"),
            Some([binding("KeyK")].as_slice())
        );
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
        assert_eq!(store.bindings("palette"), Some(&[] as &[Binding]));
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
        assert_eq!(
            store.bindings("palette"),
            Some([binding("Slash")].as_slice())
        );
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
        assert_eq!(
            store.bindings("palette"),
            Some([binding("Slash")].as_slice())
        );
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
        assert_eq!(before[0].keys, "Ctrl+Slash");
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
        assert_eq!(after[0].keys, "Ctrl+K");
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
        assert!(index.contains(&"Ctrl+Slash".to_owned()));
        assert!(index.contains(&"CtrlSlash".to_owned()));
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
    fn the_apps_fixed_bindings_are_the_five_its_input_handling_implements() {
        use iced::keyboard::{Key, Modifiers};
        let store = Store::new(native_fixed());
        // Every row renders as the Settings disclosure already prints it.
        let catalog: Vec<(String, String, bool)> = native_fixed()
            .into_iter()
            .map(|(id, _)| (id.clone(), id, true))
            .collect();
        let rendered: Vec<String> = rows(&store, &catalog)
            .into_iter()
            .map(|row| row.keys)
            .collect();
        assert!(rendered.contains(&"Ctrl+B".to_owned()));
        assert!(rendered.contains(&"Ctrl+F".to_owned()));
        assert!(rendered.contains(&"Enter / Ctrl+Enter".to_owned()));
        assert!(rendered.contains(&"Shift+Enter".to_owned()));
        assert!(rendered.contains(&"Escape".to_owned()));
        // And they answer presses: the app's own keys, through the same resolver the trigger uses.
        assert_eq!(
            resolve(&store, &Key::Character("b".into()), Modifiers::CTRL),
            Some("fixed.navigation")
        );
        assert_eq!(
            resolve(
                &store,
                &Key::Named(iced::keyboard::key::Named::Enter),
                Modifiers::empty()
            ),
            Some("fixed.send")
        );
        assert_eq!(
            resolve(
                &store,
                &Key::Named(iced::keyboard::key::Named::Enter),
                Modifiers::CTRL
            ),
            Some("fixed.send")
        );
        // Escape is unambiguous because only one command claims it.
        assert_eq!(
            resolve(
                &store,
                &Key::Named(iced::keyboard::key::Named::Escape),
                Modifiers::empty()
            ),
            Some("fixed.close")
        );
    }

    #[test]
    fn a_command_with_a_primary_and_an_alternate_answers_both() {
        use iced::keyboard::{Key, Modifiers};
        // `send` ships with Enter and Ctrl+Enter, and `move` with both arrows: one command, a list
        // of bindings (`fixed.ts:12`; native's own list prints "Enter / Ctrl+Enter").
        let store = Store::new([(
            "send".to_owned(),
            vec![
                Binding {
                    code: "Enter".to_owned(),
                    second_code: None,
                    modifiers: Vec::new(),
                },
                Binding {
                    code: "Enter".to_owned(),
                    second_code: None,
                    modifiers: vec!["primary".to_owned()],
                },
            ],
        )]);
        // Either binding answers the press, which a single-binding store could not express.
        assert_eq!(
            resolve(
                &store,
                &Key::Named(iced::keyboard::key::Named::Enter),
                Modifiers::empty()
            ),
            Some("send")
        );
        assert_eq!(
            resolve(
                &store,
                &Key::Named(iced::keyboard::key::Named::Enter),
                Modifiers::CTRL
            ),
            Some("send")
        );
        // A third form that is not in the list does not.
        assert_eq!(
            resolve(
                &store,
                &Key::Named(iced::keyboard::key::Named::Enter),
                Modifiers::SHIFT
            ),
            None
        );
        // The row renders the whole list, as native's own disclosure already does.
        let catalog = vec![("send".to_owned(), "Send".to_owned(), true)];
        let row = rows(&store, &catalog).remove(0);
        assert_eq!(row.keys, "Enter / Ctrl+Enter");
        // A conflict is found against the *alternate* binding too, not only the first.
        let other = Binding {
            code: "Enter".to_owned(),
            second_code: None,
            modifiers: vec!["primary".to_owned()],
        };
        assert_eq!(store.conflict("send", &other), None);
        let store_two = Store::new([
            ("send".to_owned(), vec![binding("KeyJ")]),
            (
                "palette".to_owned(),
                vec![Binding {
                    code: "Enter".to_owned(),
                    second_code: None,
                    modifiers: vec!["primary".to_owned()],
                }],
            ),
        ]);
        assert_eq!(store_two.conflict("send", &other), Some("palette"));
    }

    #[test]
    fn a_press_resolves_to_the_command_whose_binding_it_answers() {
        use iced::keyboard::{Key, Modifiers};
        let store = store();
        // `palette` ships as `primary + Slash`, which is the reference's trigger default.
        assert_eq!(
            resolve(&store, &Key::Character("/".into()), Modifiers::CTRL),
            Some("palette")
        );
        assert_eq!(
            resolve(&store, &Key::Character("b".into()), Modifiers::CTRL),
            None
        );
        // An unnamed modifier must not be held, or the trigger would fire on Ctrl+Shift+/.
        assert_eq!(
            resolve(
                &store,
                &Key::Character("/".into()),
                Modifiers::CTRL | Modifiers::SHIFT
            ),
            None
        );
    }

    #[test]
    fn the_trigger_default_is_the_reference_pair() {
        // The reference registers `shortcuts.open` as `primary + Slash` on every desktop platform
        // (`ui-shortcuts/src/client/index.ts:42-52`), so the trigger's own binding is that pair.
        let trigger = Binding {
            code: "Slash".to_owned(),
            second_code: None,
            modifiers: vec!["primary".to_owned()],
        };
        assert_eq!(display(&trigger), "Ctrl+Slash");
        let store = Store::new([("shortcuts.open".to_owned(), vec![trigger.clone()])]);
        assert_eq!(
            resolve(
                &store,
                &iced::keyboard::Key::Character("/".into()),
                iced::keyboard::Modifiers::CTRL
            ),
            Some("shortcuts.open")
        );
    }

    #[test]
    fn reset_all_restores_every_default_and_honours_the_reviewed_revision() {
        let mut store = store();
        store.apply(
            Edit::Set {
                id: "palette".to_owned(),
                binding: None,
            },
            0,
        );
        store.apply(
            Edit::Set {
                id: "send".to_owned(),
                binding: Some(binding("KeyJ")),
            },
            1,
        );
        assert_eq!(modified_count(&store), 2);
        // A stale reset-all changes nothing and hands the caller the form to retry.
        assert!(matches!(
            store.reset_all(0),
            Outcome::Stale {
                revision: 2,
                retry: Edit::ResetAll
            }
        ));
        assert_eq!(modified_count(&store), 2);
        // At the reviewed revision it restores both the cleared and the replaced binding.
        assert_eq!(store.reset_all(2), Outcome::Saved { revision: 3 });
        assert_eq!(modified_count(&store), 0);
        assert_eq!(
            store.bindings("palette"),
            Some([binding("Slash")].as_slice())
        );
        assert_eq!(store.bindings("send"), Some([binding("Enter")].as_slice()));
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
