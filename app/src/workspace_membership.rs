//! Borrowed workspace membership; index only after repeated linear scans justify it.
use std::{collections::HashSet, hash::Hash};

pub(super) enum Membership<'a, T> {
    All,
    Small(&'a [T]),
    Adaptive {
        ids: &'a [T],
        comparisons: usize,
        index: Option<HashSet<&'a T>>,
    },
}
impl<'a, T: Eq + Hash> Membership<'a, T> {
    #[inline]
    pub(super) fn new(ids: Option<&'a [T]>) -> Self {
        match ids {
            None => Self::All,
            Some(ids) if ids.len() < 32 => Self::Small(ids),
            Some(ids) => Self::Adaptive {
                ids,
                comparisons: 0,
                index: None,
            },
        }
    }
    #[inline]
    pub(super) fn contains(&mut self, key: &T) -> bool {
        match self {
            Self::All => true,
            Self::Small(ids) => ids.contains(key),
            Self::Adaptive {
                ids,
                comparisons,
                index,
            } => {
                if index.is_none() && *comparisons >= ids.len().saturating_mul(2) {
                    *index = Some(ids.iter().collect());
                }
                if let Some(index) = index {
                    return index.contains(key);
                }
                let position = ids.iter().position(|id| id == key);
                *comparisons = comparisons.saturating_add(position.map_or(ids.len(), |i| i + 1));
                position.is_some()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_membership_matches_original_including_duplicates_and_unicode() {
        for count in [0, 1, 12, 31, 32, 64, 512, 4096] {
            let mut ids: Vec<_> = (0..count).map(|i| format!("PUBLIC-🌹-{i}")).collect();
            if let Some(first) = ids.first().cloned() {
                ids.push(first);
            }
            for scope in [None, Some(ids.as_slice())] {
                let mut membership = Membership::new(scope);
                for round in 0..3 {
                    for i in 0..count + 2 {
                        let key = format!(
                            "{}-{i}",
                            if round == 1 {
                                "PUBLIC-🌹"
                            } else {
                                "public-🌹"
                            }
                        );
                        assert_eq!(
                            membership.contains(&key),
                            scope.is_none_or(|ids| ids.contains(&key))
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn short_predicates_and_early_matches_do_not_build_an_index() {
        let ids: Vec<_> = (0..4096).collect();
        let mut membership = Membership::new(Some(&ids));
        for _ in 0..32 {
            assert!(membership.contains(&0));
        }
        assert!(matches!(
            membership,
            Membership::Adaptive { index: None, .. }
        ));
        assert!(!membership.contains(&5000));
        assert!(!membership.contains(&5000));
        assert!(membership.contains(&4095));
        assert!(matches!(
            membership,
            Membership::Adaptive { index: Some(_), .. }
        ));
    }
    #[test]
    fn each_iterator_uses_current_workspace_membership() {
        let first = ["PUBLIC-a", "PUBLIC-b"];
        let second = ["PUBLIC-c"];
        assert!(Membership::new(Some(&first)).contains(&"PUBLIC-a"));
        assert!(!Membership::new(Some(&second)).contains(&"PUBLIC-a"));
        assert!(Membership::new(Some(&second)).contains(&"PUBLIC-c"));
        assert!(!Membership::<&str>::new(Some(&[])).contains(&"PUBLIC-c"));
        assert!(Membership::<&str>::new(None).contains(&"PUBLIC-c"));
    }
}
