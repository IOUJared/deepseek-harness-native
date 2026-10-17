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

    /// A binding already held by another command, which the reference reports as a conflict.
    pub fn conflict(&self, id: &str, binding: &Binding) -> Option<&str> {
        self.entries
            .iter()
            .find(|(other, entry)| other.as_str() != id && entry.binding.as_ref() == Some(binding))
            .map(|(other, _)| other.as_str())
    }
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
    fn a_binding_held_by_another_command_is_reported_as_a_conflict() {
        let store = store();
        assert_eq!(store.conflict("send", &binding("Slash")), Some("palette"));
        assert_eq!(store.conflict("send", &binding("Enter")), None);
    }
}
