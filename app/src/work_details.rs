//! The reference's work-details modes and the presentation policy each enables
//! (`presentation-policy.ts:11-56`, `settings/TranscriptViewRow.tsx`, `locale.ts:295-301`).
//!
//! One persisted preference selects how much of a turn's process is shown: whether a settled turn
//! folds its steps away, whether groups exist at all, whether a live group's title carries the
//! running command or path, and whether a settled reasoning row previews its first line. Renderers
//! read a single policy field; none compares the mode directly, so adding a mode changes one table.

/// The four work-details modes, in the reference's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Mode {
    Compact,
    Standard,
    Detailed,
    Verbose,
}

impl Default for Mode {
    fn default() -> Self {
        // `standard` is the reference's default row state.
        Self::Standard
    }
}

impl Mode {
    /// Every mode, in the Settings row's order.
    pub const ALL: [Mode; 4] = [Mode::Compact, Mode::Standard, Mode::Detailed, Mode::Verbose];
    /// The reference's `settings.transcript.*` label for this mode.
    pub fn label(self) -> &'static str {
        match self {
            Mode::Compact => "Compact",
            Mode::Standard => "Standard",
            Mode::Detailed => "Detailed",
            Mode::Verbose => "Verbose",
        }
    }
    /// The settings key this mode persists as.
    pub fn key(self) -> &'static str {
        match self {
            Mode::Compact => "compact",
            Mode::Standard => "standard",
            Mode::Detailed => "detailed",
            Mode::Verbose => "verbose",
        }
    }
    /// Parse a persisted key; anything unrecognized keeps the default.
    pub fn parse(key: &str) -> Self {
        Mode::ALL
            .into_iter()
            .find(|mode| mode.key() == key)
            .unwrap_or_default()
    }
    /// The policy this mode enables.
    pub fn policy(self) -> Policy {
        match self {
            Mode::Compact => Policy {
                fold_completed_turns: true,
                step_grouping: StepGrouping::Collapsed,
                live_process_detail: false,
                settled_reasoning_preview: false,
            },
            Mode::Standard => Policy {
                fold_completed_turns: true,
                step_grouping: StepGrouping::Collapsed,
                live_process_detail: true,
                settled_reasoning_preview: true,
            },
            Mode::Detailed => Policy {
                fold_completed_turns: true,
                step_grouping: StepGrouping::History,
                live_process_detail: true,
                settled_reasoning_preview: true,
            },
            Mode::Verbose => Policy {
                fold_completed_turns: false,
                step_grouping: StepGrouping::None,
                live_process_detail: false,
                settled_reasoning_preview: true,
            },
        }
    }
}

/// Collapsible group headers for all turns, historical turns only, or no turns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepGrouping {
    Collapsed,
    History,
    None,
}

/// Presentation capabilities one work-details mode enables.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    /// Whether a normally completed turn folds its process rows behind the turn control.
    pub fold_completed_turns: bool,
    /// Which turns get collapsible group headers.
    pub step_grouping: StepGrouping,
    /// Whether a live group's title carries the running command, path or query.
    pub live_process_detail: bool,
    /// Whether a settled reasoning row previews its first line beside the Think title.
    pub settled_reasoning_preview: bool,
}

impl Policy {
    /// Whether a group's seat is drawn at all for one turn state.
    pub fn groups(&self, settled: bool) -> bool {
        match self.step_grouping {
            StepGrouping::Collapsed => true,
            StepGrouping::History => settled,
            StepGrouping::None => false,
        }
    }
    /// Whether a group starts folded. The reference's seat owns an *initially collapsed*
    /// disclosure, whatever its turn's state (`use-disclosure.ts`: "Own one initially
    /// collapsed disclosure"; `ChatGroupSeat.tsx:162` hides the body while `grouped && !open`),
    /// so every seat a mode draws starts folded and a click opens it. `fold_completed_turns`
    /// is the mode's folding capability; the reference's separate *outer* turn fold
    /// (`ChatGroupSeat.tsx:152-153`) is not built in native.
    pub fn starts_folded(&self, settled: bool) -> bool {
        self.fold_completed_turns && self.groups(settled)
    }
}

/// The Settings row's copy (`locale.ts:296-301`).
pub const SETTINGS_TITLE: &str = "Work details";
pub const SETTINGS_DESCRIPTION: &str = "Choose how much detail to show for tool calls";

#[cfg(test)]
mod tests {
    //! The four modes and what each enables, exactly as the reference's table reads.
    use super::*;

    #[test]
    fn the_default_is_standard_and_round_trips_through_its_key() {
        assert_eq!(Mode::default(), Mode::Standard);
        for mode in Mode::ALL {
            assert_eq!(Mode::parse(mode.key()), mode);
        }
        assert_eq!(Mode::parse("unknown"), Mode::Standard);
        assert_eq!(Mode::parse(""), Mode::Standard);
    }

    #[test]
    fn each_mode_enables_the_reference_policy() {
        let compact = Mode::Compact.policy();
        assert!(compact.fold_completed_turns);
        assert_eq!(compact.step_grouping, StepGrouping::Collapsed);
        assert!(!compact.live_process_detail);
        assert!(!compact.settled_reasoning_preview);

        let standard = Mode::Standard.policy();
        assert!(standard.live_process_detail);
        assert!(standard.settled_reasoning_preview);

        let detailed = Mode::Detailed.policy();
        assert_eq!(detailed.step_grouping, StepGrouping::History);
        assert!(detailed.fold_completed_turns);

        let verbose = Mode::Verbose.policy();
        assert!(!verbose.fold_completed_turns);
        assert_eq!(verbose.step_grouping, StepGrouping::None);
    }

    #[test]
    fn the_labels_are_the_reference_copy() {
        assert_eq!(
            Mode::ALL.map(Mode::label),
            ["Compact", "Standard", "Detailed", "Verbose"]
        );
    }

    #[test]
    fn grouping_and_folding_follow_from_the_policy() {
        let standard = Mode::Standard.policy();
        assert!(standard.groups(false) && standard.groups(true));
        // The reference's disclosure starts collapsed for a live group too, so Standard
        // shows one running line until the reader opens it.
        assert!(standard.starts_folded(false));
        assert!(standard.starts_folded(true));

        let detailed = Mode::Detailed.policy();
        assert!(!detailed.groups(false), "a live turn has no header");
        assert!(detailed.groups(true), "a historical turn does");
        assert!(detailed.starts_folded(true));

        let verbose = Mode::Verbose.policy();
        assert!(!verbose.groups(false) && !verbose.groups(true));
        assert!(
            !verbose.starts_folded(false) && !verbose.starts_folded(true),
            "verbose folds nothing"
        );
    }
}
