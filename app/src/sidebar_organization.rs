//! Local sidebar metadata only: never backend cwd/account changes, cancellation or log erasure.
//! The parent admits live context/group membership, executes one joined Store::sidebar operation
//! at a time, and returns its ticket. A save proposal is private until its successful ACK. Reload
//! is explicit after any error/uncertainty; absent roster IDs are deliberately never pruned.
use super::draft_store::{Drafts, Failure, MAX_DRAFTS};
use dsh_native_transport::dto::{SessionId, WorkspaceId};
use serde::{Deserialize, Deserializer, Serialize, de};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

pub type Data = BTreeMap<SessionId, Entry>;
const MAX_ID_BYTES: usize = 128;
// A bounded identifier and fixed fields fit comfortably; reject oversized input before parsing.
const MAX_ENTRY_BYTES: usize = 2048;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum Location {
    #[default]
    Original,
    Workspace(WorkspaceId),
    Ungrouped,
}

// A map visitor rejects positional arrays, duplicate fields, unknown fields and extra content
// on unit variants. DTO identifier parsing alone does not enforce our smaller persistence bound.
impl<'de> Deserialize<'de> for Location {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Location;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a closed sidebar location object")
            }
            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Location, A::Error> {
                let mut kind = None;
                let mut id = None;
                while let Some(field) = map.next_key::<String>()? {
                    match field.as_str() {
                        "kind" if kind.is_none() => kind = Some(map.next_value::<String>()?),
                        "id" if id.is_none() => id = Some(map.next_value::<WorkspaceId>()?),
                        _ => return Err(de::Error::custom("invalid location field")),
                    }
                }
                match (kind.as_deref(), id) {
                    (Some("original"), None) => Ok(Location::Original),
                    (Some("ungrouped"), None) => Ok(Location::Ungrouped),
                    (Some("workspace"), Some(id)) if valid_id(id.as_str()) => {
                        Ok(Location::Workspace(id))
                    }
                    _ => Err(de::Error::custom("invalid location")),
                }
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub location: Location,
    // null is valid, omission is not: every persisted entry has the complete closed schema.
    #[serde(deserialize_with = "required_rank")]
    pub rank: Option<u64>,
    pub trashed: bool,
}
fn required_rank<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u64>, D::Error> {
    Option::<u64>::deserialize(deserializer)
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_ID_BYTES && !id.chars().any(char::is_control)
}
fn validate(data: &Data) -> Result<(), Failure> {
    if data.len() > MAX_DRAFTS {
        return Err(Failure::Limit);
    }
    for (id, entry) in data {
        if !valid_id(id.as_str())
            || matches!(&entry.location, Location::Workspace(id) if !valid_id(id.as_str()))
        {
            return Err(Failure::InvalidData);
        }
    }
    Ok(())
}

/// Store owns the closed, version-1 outer document; each map value is one closed Entry object.
/// No names, paths, transcript/composer text or backend mutations are encoded here.
pub fn encode(data: &Data) -> Result<Drafts, Failure> {
    validate(data)?;
    data.iter()
        .map(|(id, entry)| {
            let value = serde_json::to_string(entry).map_err(|_| Failure::InvalidData)?;
            if value.len() > MAX_ENTRY_BYTES {
                return Err(Failure::Limit);
            }
            Ok((id.clone(), value))
        })
        .collect()
}

/// Fail the complete map on any malformed entry, rather than silently dropping metadata.
pub fn decode(drafts: &Drafts) -> Result<Data, Failure> {
    if drafts.len() > MAX_DRAFTS {
        return Err(Failure::Limit);
    }
    let mut data = Data::new();
    for (id, value) in drafts {
        if value.len() > MAX_ENTRY_BYTES {
            return Err(Failure::Limit);
        }
        // Serde's struct visitor also accepts positional arrays; the format is object-only.
        if value.bytes().find(|byte| !byte.is_ascii_whitespace()) != Some(b'{') {
            return Err(Failure::InvalidData);
        }
        let entry = serde_json::from_str(value).map_err(|_| Failure::InvalidData)?;
        data.insert(id.clone(), entry);
    }
    validate(&data)?;
    Ok(data)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stamp {
    pub epoch: u64,
    pub generation: u64,
    pub serial: u64,
    pub revision: u64,
    pub target: SessionId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Mutation {
    Trash,
    Restore,
    Move(Location),
    /// Exact current group, validated against the live roster by the parent. Positions start at 0.
    Reorder(Vec<SessionId>),
}

/// Only these snapshots need to cross the blocking I/O boundary, never the proposal's Data.
pub struct Write {
    pub ticket: u64,
    pub expected: Drafts,
    pub proposed: Drafts,
}

struct Panel {
    stamp: Stamp,
    review_trash: bool,
}
enum Pending {
    Load {
        ticket: u64,
    },
    Save {
        ticket: u64,
        data: Data,
        encoded: Drafts,
        revision: u64,
    },
}

#[derive(Default)]
pub struct State {
    /// Committed, validated metadata. Parent must treat this as read-only.
    pub data: Data,
    // Keep exact decoded disk strings: normalizing JSON before CAS would create false conflicts.
    encoded: Drafts,
    loaded: bool,
    started: bool,
    failure: Option<Failure>,
    exhausted: bool,
    serial: u64,
    ticket: u64,
    revision: u64,
    panel: Option<Panel>,
    pending: Option<Pending>,
}
impl State {
    pub fn ready(&self) -> bool {
        self.loaded
    }
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
    pub fn failure(&self) -> Option<Failure> {
        self.failure
    }
    pub fn is_open(&self) -> bool {
        self.panel.is_some()
    }
    pub fn stamp(&self) -> Option<Stamp> {
        self.panel.as_ref().map(|panel| panel.stamp.clone())
    }
    pub fn review_trash(&self) -> bool {
        self.panel.as_ref().is_some_and(|panel| panel.review_trash)
    }
    pub fn trashed(&self, id: &SessionId) -> bool {
        self.data.get(id).is_some_and(|entry| entry.trashed)
    }
    pub fn location(&self, id: &SessionId) -> Location {
        self.data
            .get(id)
            .map_or(Location::Original, |entry| entry.location.clone())
    }
    pub fn rank(&self, id: &SessionId) -> Option<u64> {
        self.data.get(id).and_then(|entry| entry.rank)
    }

    /// Automatic initial load once; later reads require an explicit Reload click.
    pub fn start_load(&mut self, explicit: bool) -> Option<u64> {
        if self.busy() || self.exhausted || (self.started && !explicit) {
            return None;
        }
        let ticket = self.next_ticket()?;
        self.started = true;
        self.loaded = false;
        self.pending = Some(Pending::Load { ticket });
        Some(ticket)
    }

    pub fn loaded(&mut self, ticket: u64, result: Result<Drafts, Failure>) {
        if !matches!(self.pending, Some(Pending::Load { ticket: current }) if current == ticket) {
            return;
        }
        self.pending = None;
        if self.exhausted {
            return;
        }
        match result.and_then(|encoded| decode(&encoded).map(|data| (encoded, data))) {
            Ok((encoded, data)) => {
                let Some(revision) = self.next_revision() else {
                    return;
                };
                self.data = data;
                self.encoded = encoded;
                self.loaded = true;
                self.failure = None;
                self.publish_revision(revision);
            }
            Err(failure) => self.failure = Some(failure),
        }
    }

    /// Only the parent admits target/context. Storage availability does not block inspection.
    pub fn open(&mut self, epoch: u64, generation: u64, target: SessionId) {
        let Some(serial) = self.next_serial() else {
            return;
        };
        self.panel = Some(Panel {
            stamp: Stamp {
                epoch,
                generation,
                serial,
                revision: self.revision,
                target,
            },
            review_trash: false,
        });
    }
    pub fn close(&mut self, stamp: &Stamp) {
        if self.matches(stamp) {
            self.dismiss();
        }
    }
    /// Closing is not rollback or cancellation; an admitted operation still requires its join.
    pub fn dismiss(&mut self) {
        self.panel = None;
    }

    /// Enter review and invalidate the original menu action; confirm uses the new current stamp.
    pub fn review(&mut self, stamp: &Stamp) {
        if !self.admits(stamp) || self.trashed(&stamp.target) {
            return;
        }
        let Some(serial) = self.next_serial() else {
            return;
        };
        if let Some(panel) = &mut self.panel {
            panel.stamp.serial = serial;
            panel.review_trash = true;
        }
    }

    pub fn propose(&mut self, stamp: &Stamp, mutation: Mutation) -> Option<Write> {
        if !self.admits(stamp) {
            return None;
        }
        let mut proposed = self.data.clone();
        match mutation {
            Mutation::Trash => {
                if !self.review_trash() || self.trashed(&stamp.target) {
                    return None;
                }
                proposed.entry(stamp.target.clone()).or_default().trashed = true;
            }
            Mutation::Restore => {
                if !self.trashed(&stamp.target) {
                    return None;
                }
                proposed.get_mut(&stamp.target)?.trashed = false;
            }
            Mutation::Move(location) => {
                if self.trashed(&stamp.target) {
                    return None;
                }
                let entry = proposed.entry(stamp.target.clone()).or_default();
                if entry.location == location {
                    return None;
                }
                entry.location = location;
                // Ordering belongs to the previous group, not the destination group.
                entry.rank = None;
            }
            Mutation::Reorder(ids) => {
                if ids.len() > MAX_DRAFTS {
                    self.fail(Failure::Limit);
                    return None;
                }
                let unique: BTreeSet<_> = ids.iter().collect();
                if ids.is_empty() || unique.len() != ids.len() {
                    return None;
                }
                for (position, id) in ids.into_iter().enumerate() {
                    // A current visible group cannot include Trash; live group membership itself
                    // remains the parent's responsibility (Original needs backend cwd metadata).
                    if self.trashed(&id) {
                        return None;
                    }
                    proposed.entry(id).or_default().rank = Some(position as u64);
                }
            }
        }
        // Resetting this target to defaults reclaims its slot. Never evict untouched
        // metadata (even explicit defaults), and never prune based on roster visibility.
        if proposed.get(&stamp.target) == Some(&Entry::default()) {
            proposed.remove(&stamp.target);
        }
        if proposed == self.data {
            return None;
        }
        let encoded = match encode(&proposed) {
            Ok(encoded) => encoded,
            Err(failure) => {
                self.fail(failure);
                return None;
            }
        };
        // Reserve the completion revision before disk admission; overflow must not admit a write.
        let revision = self.next_revision()?;
        let serial = self.next_serial()?;
        let ticket = self.next_ticket()?;
        if let Some(panel) = &mut self.panel {
            panel.stamp.serial = serial;
            panel.review_trash = false;
        }
        let write = Write {
            ticket,
            expected: self.encoded.clone(),
            proposed: encoded.clone(),
        };
        self.pending = Some(Pending::Save {
            ticket,
            data: proposed,
            encoded,
            revision,
        });
        Some(write)
    }

    pub fn saved(&mut self, ticket: u64, result: Result<(), Failure>) {
        if !matches!(self.pending, Some(Pending::Save { ticket: current, .. }) if current == ticket)
        {
            return;
        }
        let Some(Pending::Save {
            data,
            encoded,
            revision,
            ..
        }) = self.pending.take()
        else {
            return;
        };
        match result {
            Ok(()) => {
                self.data = data;
                self.encoded = encoded;
                self.publish_revision(revision);
            }
            Err(failure) => {
                // Preserve counter exhaustion if it occurred while this operation was pending.
                if !self.exhausted {
                    self.fail(failure);
                }
            }
        }
    }

    fn matches(&self, stamp: &Stamp) -> bool {
        self.panel
            .as_ref()
            .is_some_and(|panel| panel.stamp == *stamp)
    }
    fn admits(&self, stamp: &Stamp) -> bool {
        self.matches(stamp)
            && self.loaded
            && !self.busy()
            && self.failure.is_none()
            && !self.exhausted
    }
    fn fail(&mut self, failure: Failure) {
        self.failure = Some(failure);
    }
    fn limit(&mut self) {
        self.exhausted = true;
        self.fail(Failure::Limit);
    }
    fn next_serial(&mut self) -> Option<u64> {
        let Some(serial) = self.serial.checked_add(1) else {
            self.limit();
            return None;
        };
        self.serial = serial;
        Some(serial)
    }
    fn next_ticket(&mut self) -> Option<u64> {
        let Some(ticket) = self.ticket.checked_add(1) else {
            self.limit();
            return None;
        };
        self.ticket = ticket;
        Some(ticket)
    }
    fn next_revision(&mut self) -> Option<u64> {
        let Some(revision) = self.revision.checked_add(1) else {
            self.limit();
            return None;
        };
        Some(revision)
    }
    fn publish_revision(&mut self, revision: u64) {
        self.revision = revision;
        if let Some(panel) = &mut self.panel {
            panel.stamp.revision = revision;
            panel.review_trash = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id(value: &str) -> SessionId {
        SessionId::new(value).unwrap()
    }
    fn workspace() -> Location {
        Location::Workspace(WorkspaceId::new("PUBLIC-workspace").unwrap())
    }
    fn data() -> Data {
        Data::from([(
            id("PUBLIC-missing-from-roster"),
            Entry {
                location: workspace(),
                rank: Some(17),
                trashed: false,
            },
        )])
    }
    fn state(data: Data) -> State {
        let mut state = State::default();
        let ticket = state.start_load(false).unwrap();
        state.loaded(ticket, Ok(encode(&data).unwrap()));
        assert!(state.ready());
        state.open(2, 7, id("PUBLIC-missing-from-roster"));
        state
    }
    fn propose(state: &mut State, mutation: Mutation) -> Write {
        state.propose(&state.stamp().unwrap(), mutation).unwrap()
    }

    #[test]
    fn resetting_customization_reclaims_a_slot_without_pruning_other_missing_sessions() {
        let mut owner = state(data());
        owner.open(2, 7, id("PUBLIC-extra"));
        let write = propose(&mut owner, Mutation::Move(workspace()));
        owner.saved(write.ticket, Ok(()));
        let write = propose(&mut owner, Mutation::Move(Location::Original));
        owner.saved(write.ticket, Ok(()));
        assert!(!owner.data.contains_key(&id("PUBLIC-extra")));
        assert!(owner.data.contains_key(&id("PUBLIC-missing-from-roster")));
    }

    #[test]
    fn codec_round_trip_all_locations_ranks_and_restart_retains_missing_ids() {
        let mut expected = data();
        expected.insert(id("PUBLIC-original"), Entry::default());
        expected.insert(
            id("PUBLIC-ungrouped"),
            Entry {
                location: Location::Ungrouped,
                rank: Some(u64::MAX),
                trashed: true,
            },
        );
        let disk = encode(&expected).unwrap();
        assert_eq!(decode(&disk).unwrap(), expected);
        let mut restarted = State::default();
        let ticket = restarted.start_load(false).unwrap();
        restarted.loaded(ticket, Ok(disk));
        assert_eq!(restarted.data, expected);
        assert_eq!(
            restarted.location(&id("PUBLIC-never-customized")),
            Location::Original
        );
        assert_eq!(restarted.rank(&id("PUBLIC-never-customized")), None);
        assert!(!restarted.trashed(&id("PUBLIC-never-customized")));
    }

    #[test]
    fn codec_rejects_complete_document_on_malformed_nonobject_unknown_or_version_fields() {
        for bad in [
            "{",
            "null",
            "[]",
            "[{},null,false]",
            "true",
            r#"{"location":{"kind":"original"},"rank":null,"trashed":false,"version":2}"#,
            r#"{"location":{"kind":"newer"},"rank":null,"trashed":false}"#,
            r#"{"location":{"kind":"original","id":"PUBLIC"},"rank":null,"trashed":false}"#,
            r#"{"location":{"kind":"workspace"},"rank":null,"trashed":false}"#,
            r#"{"location":{"kind":"ungrouped","path":"/PUBLIC"},"rank":null,"trashed":false}"#,
            r#"{"location":["original"],"rank":null,"trashed":false}"#,
            r#"{"location":{"kind":"original","kind":"original"},"rank":null,"trashed":false}"#,
            r#"{"location":{"kind":"workspace","id":"PUBLIC","id":"PUBLIC"},"rank":null,"trashed":false}"#,
            r#"{"location":{"kind":"original"},"rank":-1,"trashed":false}"#,
            r#"{"location":{"kind":"original"},"rank":18446744073709551616,"trashed":false}"#,
            r#"{"location":{"kind":"original"},"rank":1.5,"trashed":false}"#,
            r#"{"location":{"kind":"original"},"rank":null,"trashed":false,"trashed":true}"#,
            r#"{"location":{"kind":"original"},"rank":null}"#,
            r#"{"location":{"kind":"original"},"trashed":false}"#,
            r#"{"location":{"kind":"workspace","id":"PUBLIC\n"},"rank":null,"trashed":false}"#,
        ] {
            let mut disk = encode(&data()).unwrap();
            disk.insert(id("PUBLIC-bad"), bad.into());
            assert_eq!(decode(&disk), Err(Failure::InvalidData), "{bad}");
        }
    }

    #[test]
    fn codec_count_byte_and_identifier_limits() {
        let maximum: Data = (0..MAX_DRAFTS)
            .map(|n| (id(&format!("PUBLIC-{n}")), Entry::default()))
            .collect();
        assert_eq!(decode(&encode(&maximum).unwrap()).unwrap(), maximum);
        let mut too_many = maximum;
        too_many.insert(id("PUBLIC-extra"), Entry::default());
        assert_eq!(encode(&too_many), Err(Failure::Limit));
        let raw: Drafts = too_many
            .keys()
            .map(|id| (id.clone(), "{}".into()))
            .collect();
        assert_eq!(decode(&raw), Err(Failure::Limit));
        assert_eq!(
            decode(&Drafts::from([(
                id("PUBLIC"),
                " ".repeat(MAX_ENTRY_BYTES + 1)
            )])),
            Err(Failure::Limit)
        );
        for invalid in ["x".repeat(MAX_ID_BYTES + 1), "PUBLIC\n".into()] {
            assert_eq!(
                encode(&Data::from([(id(&invalid), Entry::default())])),
                Err(Failure::InvalidData)
            );
            let invalid_location = Data::from([(
                id("PUBLIC"),
                Entry {
                    location: Location::Workspace(WorkspaceId::new(invalid).unwrap()),
                    ..Entry::default()
                },
            )]);
            assert_eq!(encode(&invalid_location), Err(Failure::InvalidData));
        }
    }

    #[test]
    fn trash_needs_fresh_review_then_restore_keeps_exact_location_and_rank() {
        let original = data();
        let mut state = state(original.clone());
        let before_review = state.stamp().unwrap();
        assert!(state.propose(&before_review, Mutation::Trash).is_none());
        state.review(&before_review);
        assert!(state.review_trash());
        let confirmation = state.stamp().unwrap();
        assert_ne!(before_review, confirmation);
        assert!(state.propose(&before_review, Mutation::Trash).is_none());
        let write = state.propose(&confirmation, Mutation::Trash).unwrap();
        assert_eq!(state.data, original);
        assert!(state.busy());
        assert!(state.propose(&confirmation, Mutation::Trash).is_none());
        assert!(state.start_load(true).is_none());
        state.saved(write.ticket + 1, Ok(()));
        assert!(state.busy());
        state.saved(write.ticket, Ok(()));
        let target = id("PUBLIC-missing-from-roster");
        assert!(state.trashed(&target));
        assert_eq!(state.location(&target), workspace());
        assert_eq!(state.rank(&target), Some(17));
        assert!(state.is_open());
        assert!(state.propose(&confirmation, Mutation::Restore).is_none());
        let restore = propose(&mut state, Mutation::Restore);
        state.saved(restore.ticket, Ok(()));
        assert_eq!(state.data, original);
        state.saved(write.ticket, Err(Failure::Conflict));
        assert_eq!(state.failure(), None);
    }

    #[test]
    fn pending_close_late_ack_cannot_reopen_and_wrong_kind_cannot_complete() {
        let mut state = state(Data::new());
        let write = propose(&mut state, Mutation::Move(Location::Ungrouped));
        state.loaded(write.ticket, Ok(Drafts::new()));
        assert!(state.busy());
        let current = state.stamp().unwrap();
        state.close(&current);
        assert!(!state.is_open());
        assert!(state.busy());
        state.saved(write.ticket, Ok(()));
        assert!(!state.is_open());
        assert_eq!(state.location(&current.target), Location::Ungrouped);
        state.open(2, 7, current.target.clone());
        state.close(&current);
        assert!(state.is_open());
        state.dismiss();
        assert!(!state.is_open());
    }

    #[test]
    fn full_stamp_context_target_serial_revision_and_review_lifetime_are_enforced() {
        let mut state = state(data());
        let current = state.stamp().unwrap();
        let mut forgeries = Vec::new();
        for field in 0..5 {
            let mut stale = current.clone();
            match field {
                0 => stale.epoch += 1,
                1 => stale.generation += 1,
                2 => stale.serial += 1,
                3 => stale.revision += 1,
                _ => stale.target = id("PUBLIC-other"),
            }
            forgeries.push(stale);
        }
        for stale in forgeries {
            state.review(&stale);
            assert!(!state.review_trash());
            state.close(&stale);
            assert!(state.is_open());
            assert!(
                state
                    .propose(&stale, Mutation::Move(Location::Ungrouped))
                    .is_none()
            );
        }
        state.review(&current);
        state.open(2, 7, current.target);
        assert!(!state.review_trash());
    }

    #[test]
    fn all_save_failures_retain_committed_data_no_replay_explicit_reload_only() {
        for failure in [
            Failure::InvalidHome,
            Failure::Unavailable,
            Failure::InvalidData,
            Failure::Limit,
            Failure::Busy,
            Failure::Conflict,
            Failure::NotSaved,
            Failure::Indeterminate,
        ] {
            let original = data();
            let mut state = state(original.clone());
            let write = propose(&mut state, Mutation::Move(Location::Ungrouped));
            state.saved(write.ticket, Err(failure));
            assert_eq!(state.data, original);
            assert_eq!(state.failure(), Some(failure));
            assert!(!state.busy());
            assert!(
                state
                    .propose(&state.stamp().unwrap(), Mutation::Move(Location::Ungrouped))
                    .is_none()
            );
            assert!(state.start_load(false).is_none());
            let read = state.start_load(true).unwrap();
            state.saved(read, Ok(()));
            assert!(state.busy());
            state.loaded(read + 1, Ok(write.proposed.clone()));
            assert!(state.busy());
            // Uncertainty can mean the intended write is visible on disk; only validated Reload
            // publishes that reality, never replaying the original operation.
            state.loaded(read, Ok(write.proposed.clone()));
            assert!(state.ready());
            assert_eq!(state.failure(), None);
            assert_eq!(state.data, decode(&write.proposed).unwrap());
            assert!(!state.busy());
        }
    }

    #[test]
    fn load_failure_and_unavailable_panel_never_authorize_mutation_or_publish_bad_map() {
        let mut state = State::default();
        state.open(1, 1, id("PUBLIC"));
        assert!(state.is_open());
        assert!(
            state
                .propose(&state.stamp().unwrap(), Mutation::Move(Location::Ungrouped))
                .is_none()
        );
        let ticket = state.start_load(false).unwrap();
        state.loaded(ticket, Err(Failure::Unavailable));
        assert!(!state.ready());
        assert!(state.start_load(false).is_none());
        let ticket = state.start_load(true).unwrap();
        state.loaded(ticket, Ok(encode(&data()).unwrap()));
        let committed = state.data.clone();
        let ticket = state.start_load(true).unwrap();
        state.loaded(ticket, Ok(Drafts::from([(id("PUBLIC"), "null".into())])));
        assert_eq!(state.data, committed);
        assert!(!state.ready());
        assert_eq!(state.failure(), Some(Failure::InvalidData));
    }

    #[test]
    fn reorder_creates_entries_unique_bounded_preserves_others_and_move_clears_old_rank() {
        let mut state = state(data());
        let untouched = state.data.clone();
        assert!(
            state
                .propose(
                    &state.stamp().unwrap(),
                    Mutation::Reorder(vec![id("PUBLIC-a"), id("PUBLIC-a")])
                )
                .is_none()
        );
        assert!(!state.busy());
        let write = propose(
            &mut state,
            Mutation::Reorder(vec![id("PUBLIC-b"), id("PUBLIC-a")]),
        );
        assert_eq!(state.data, untouched);
        state.saved(write.ticket, Ok(()));
        assert_eq!(state.rank(&id("PUBLIC-b")), Some(0));
        assert_eq!(state.rank(&id("PUBLIC-a")), Some(1));
        assert_eq!(
            state.data.get(&id("PUBLIC-missing-from-roster")),
            untouched.get(&id("PUBLIC-missing-from-roster"))
        );
        state.open(2, 7, id("PUBLIC-a"));
        let write = propose(&mut state, Mutation::Move(workspace()));
        state.saved(write.ticket, Ok(()));
        assert_eq!(state.rank(&id("PUBLIC-a")), None);
        assert_eq!(state.location(&id("PUBLIC-a")), workspace());
        let ids = (0..=MAX_DRAFTS)
            .map(|n| id(&format!("PUBLIC-{n}")))
            .collect();
        assert!(
            state
                .propose(&state.stamp().unwrap(), Mutation::Reorder(ids))
                .is_none()
        );
        assert_eq!(state.failure(), Some(Failure::Limit));
    }

    #[test]
    fn union_capacity_refuses_without_eviction_or_pending_io() {
        let maximum: Data = (0..MAX_DRAFTS)
            .map(|n| (id(&format!("PUBLIC-{n}")), Entry::default()))
            .collect();
        let mut state = state(maximum.clone());
        state.open(2, 7, id("PUBLIC-extra"));
        state.review(&state.stamp().unwrap());
        assert!(
            state
                .propose(&state.stamp().unwrap(), Mutation::Trash)
                .is_none()
        );
        assert_eq!(state.failure(), Some(Failure::Limit));
        assert!(!state.busy());
        assert_eq!(state.data, maximum);
    }

    #[test]
    fn explicit_reload_resets_review_and_late_ack_retains_new_panel_not_old_target() {
        let mut state = state(data());
        state.review(&state.stamp().unwrap());
        let stale = state.stamp().unwrap();
        let read = state.start_load(true).unwrap();
        state.loaded(read, Ok(encode(&data()).unwrap()));
        assert!(!state.review_trash());
        assert!(state.propose(&stale, Mutation::Trash).is_none());
        let write = propose(&mut state, Mutation::Move(Location::Ungrouped));
        state.dismiss();
        state.open(9, 10, id("PUBLIC-other"));
        let before_ack = state.stamp().unwrap();
        state.saved(write.ticket, Ok(()));
        let after_ack = state.stamp().unwrap();
        assert_eq!(after_ack.target, id("PUBLIC-other"));
        assert_eq!(after_ack.epoch, 9);
        assert_eq!(after_ack.generation, 10);
        assert_eq!(after_ack.serial, before_ack.serial);
        assert!(after_ack.revision > before_ack.revision);
        state.close(&before_ack);
        assert!(state.is_open());
    }

    #[test]
    fn exact_loaded_strings_are_cas_expected_until_successful_ack() {
        let mut state = State::default();
        let raw = Drafts::from([(
            id("PUBLIC"),
            " {\"trashed\":false, \"rank\":null, \"location\":{\"kind\":\"original\"}} ".into(),
        )]);
        let ticket = state.start_load(false).unwrap();
        state.loaded(ticket, Ok(raw.clone()));
        state.open(1, 1, id("PUBLIC"));
        let write = propose(&mut state, Mutation::Move(Location::Ungrouped));
        assert_eq!(write.expected, raw);
        let saved = write.proposed.clone();
        state.saved(write.ticket, Ok(()));
        let write = propose(&mut state, Mutation::Move(workspace()));
        assert_eq!(write.expected, saved);
    }

    #[test]
    fn counter_exhaustion_never_wraps_or_admits_io_and_reload_cannot_reset_it() {
        for counter in 0..3 {
            let mut state = state(data());
            match counter {
                0 => state.serial = u64::MAX,
                1 => state.ticket = u64::MAX,
                _ => state.revision = u64::MAX,
            }
            assert!(
                state
                    .propose(&state.stamp().unwrap(), Mutation::Move(Location::Ungrouped))
                    .is_none()
            );
            assert_eq!(state.failure(), Some(Failure::Limit));
            assert!(!state.busy());
            assert!(state.start_load(true).is_none());
        }
        let mut state = state(data());
        state.serial = u64::MAX;
        state.review(&state.stamp().unwrap());
        assert_eq!(state.failure(), Some(Failure::Limit));
        assert!(!state.review_trash());
    }
}
