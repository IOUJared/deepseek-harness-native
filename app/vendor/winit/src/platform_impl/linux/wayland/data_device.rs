//! Bounded file drops on winit's owned Wayland queue. No borrowed-display FFI or I/O threads.
use std::{
    fs::File,
    hash::Hash,
    io::{self, Read},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use ahash::AHashMap;
use sctk::{
    data_device_manager::{
        data_device::{DataDeviceData, DataDeviceHandler},
        data_offer::{DataOfferData, DataOfferHandler, DragOffer},
        DataDeviceManagerState,
    },
    globals::GlobalData,
    reexports::{
        calloop::{
            generic::Generic,
            timer::{TimeoutAction, Timer},
            Interest, Mode, PostAction, RegistrationToken,
        },
        client::{
            backend::ObjectId,
            protocol::{
                wl_data_device::WlDataDevice,
                wl_data_device_manager::{DndAction, WlDataDeviceManager},
                wl_data_offer::{self, WlDataOffer},
                wl_surface::WlSurface,
            },
            Connection, Dispatch, Proxy, QueueHandle, WEnum,
        },
    },
};

use super::{state::WinitState, WindowId};
use crate::event::WindowEvent;

const MIME: &str = "text/uri-list";
const MAX_URI_BYTES: usize = 64 * 1024;
const MAX_LINES: usize = 64;
const MAX_PATH_BYTES: usize = 4096;
const MAX_TRANSFERS: usize = 8;
const DISPATCH_BYTES: usize = 8 * 1024;
const READ_CHUNK: usize = 4096;
const TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Default)]
pub(super) struct DropState {
    serial: u64,
    sessions: AHashMap<ObjectId, Session>,
    early_actions: EarlyActions<ObjectId>,
}

/// SCTK 0.19.2 discards Action while an offer is Undetermined. Retain only the
/// actual compositor action, never source capability flags. Complete ObjectId
/// keys fence backend object generations; protocol numeric IDs are insufficient.
/// Generic keys let tests execute this same bounded production owner without a
/// second connection/model. Pruning is lazy; no idle timer or clipboard owner.
struct EarlyActions<K> {
    entries: AHashMap<K, CachedAction>,
}
struct CachedAction {
    selected: Result<DndAction, ()>,
    expires: Instant,
}
impl<K> Default for EarlyActions<K> {
    fn default() -> Self {
        Self {
            entries: AHashMap::default(),
        }
    }
}
impl<K: Eq + Hash> EarlyActions<K> {
    fn prune(&mut self, now: Instant) {
        self.entries.retain(|_, entry| now < entry.expires);
    }
    fn capture(&mut self, key: K, action: WEnum<DndAction>, now: Instant) -> bool {
        self.prune(now);
        let selected = confirmed_action(action);
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.selected = selected;
            // Latest action wins, but cannot extend the initial retention lease.
            return true;
        }
        if !can_admit(self.entries.len()) {
            return false;
        }
        self.entries.insert(
            key,
            CachedAction {
                selected,
                expires: now + TIMEOUT,
            },
        );
        true
    }
    fn take(&mut self, key: &K, now: Instant) -> Option<Result<DndAction, ()>> {
        self.prune(now);
        self.entries.remove(key).map(|entry| entry.selected)
    }
}
fn confirmed_action(action: WEnum<DndAction>) -> Result<DndAction, ()> {
    match action {
        WEnum::Value(action) => Ok(action),
        WEnum::Unknown(_) => Err(()),
    }
}
/// A toolkit empty snapshot can mean its pre-enter action was lost. Only retain
/// our latest confirmed action; an explicit post-enter None already clears it.
fn reconcile_action(snapshot: DndAction, confirmed: DndAction) -> DndAction {
    if snapshot.is_empty() {
        confirmed
    } else {
        snapshot
    }
}

struct Session {
    admission: Admission,
    window: WindowId,
    offer: DragOffer,
    deadline: Option<Instant>,
    pipe: Option<RegistrationToken>,
    timer: Option<RegistrationToken>,
}

/// The actual production gate, also exercised by lifecycle tests below.
#[derive(Clone, Copy, Debug)]
struct Admission {
    serial: u64,
    dropped: bool,
    reading: bool,
}
impl Admission {
    fn new(serial: u64) -> Self {
        Self {
            serial,
            dropped: false,
            reading: false,
        }
    }
    fn matches(self, serial: u64) -> bool {
        self.serial == serial
    }
    fn drop_requested(&mut self, copy: bool) -> bool {
        if self.dropped {
            return false;
        }
        self.dropped = true;
        self.select_copy(copy)
    }
    fn select_copy(&mut self, copy: bool) -> bool {
        if self.dropped && copy && !self.reading {
            self.reading = true;
            true
        } else {
            false
        }
    }
    fn rejects_action(self, action: DndAction) -> bool {
        // Hover actions may be provisional until our COPY preference is matched.
        self.dropped && !action.is_empty() && action != DndAction::Copy
    }
    fn cancel_on_leave(self) -> bool {
        !self.dropped
    }
    fn notify_cancellation(self, failed: bool, live: bool) -> bool {
        failed && self.dropped && live
    }
    fn can_publish(
        self,
        serial: u64,
        copy: bool,
        deadline: Option<Instant>,
        now: Instant,
        live: bool,
    ) -> bool {
        self.matches(serial)
            && self.dropped
            && self.reading
            && copy
            && live
            && deadline.is_some_and(|deadline| now < deadline)
    }
}
fn can_admit(count: usize) -> bool {
    count < MAX_TRANSFERS
}

#[derive(Debug, PartialEq, Eq)]
enum ReadProgress {
    Pending,
    Complete,
    Rejected,
}

/// Production nonblocking read step: fixed body, fixed per-dispatch work, one-byte ceiling probe.
fn read_uri_chunk(mut reader: impl Read, buffer: &mut [u8], used: &mut usize) -> ReadProgress {
    if buffer.len() != MAX_URI_BYTES || *used > MAX_URI_BYTES {
        return ReadProgress::Rejected;
    }
    let mut budget = DISPATCH_BYTES;
    for _ in 0..16 {
        if budget == 0 {
            return ReadProgress::Pending;
        }
        let mut probe = [0; 1];
        let at_limit = *used == MAX_URI_BYTES;
        let end = (*used + READ_CHUNK.min(budget)).min(MAX_URI_BYTES);
        let result = if at_limit {
            reader.read(&mut probe)
        } else {
            reader.read(&mut buffer[*used..end])
        };
        match result {
            Ok(0) => return ReadProgress::Complete,
            Ok(_) if at_limit => return ReadProgress::Rejected,
            Ok(count) => {
                *used += count;
                budget -= count;
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                return ReadProgress::Pending
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return ReadProgress::Rejected,
        }
    }
    ReadProgress::Pending
}
fn destroy(offer: &DragOffer) {
    // SCTK already destroys an undropped offer on leave/replacement.
    if offer.inner().is_alive() {
        offer.destroy();
    }
}
fn device_seat(device: &WlDataDevice) -> Option<ObjectId> {
    device.data::<DataDeviceData>().map(|data| data.seat().id())
}
fn device_offer(device: &WlDataDevice) -> Option<DragOffer> {
    device
        .data::<DataDeviceData>()
        .and_then(|data| data.drag_offer())
}
fn copy_selected(offer: &DragOffer) -> bool {
    offer.inner().version() < 3 || offer.selected_action == DndAction::Copy
}

impl WinitState {
    pub(super) fn cancel_drop_seat(&mut self, seat: &ObjectId) {
        self.retire_drop_seat(seat, true);
    }
    fn retire_drop_seat(&mut self, seat: &ObjectId, notify_failure: bool) {
        if let Some(session) = self.file_drops.sessions.remove(seat) {
            if session.admission.notify_cancellation(
                notify_failure,
                self.windows.borrow().contains_key(&session.window),
            ) {
                self.events_sink
                    .push_window_event(WindowEvent::HoveredFileCancelled, session.window);
                self.dispatched_events = true;
            }
            if let Some(token) = session.pipe {
                self.loop_handle.remove(token);
            }
            if let Some(token) = session.timer {
                self.loop_handle.remove(token);
            }
            destroy(&session.offer);
        }
    }
    pub(super) fn cancel_drop_window(&mut self, window: WindowId) {
        let seats: Vec<_> = self
            .file_drops
            .sessions
            .iter()
            .filter(|(_, session)| session.window == window)
            .map(|(seat, _)| seat.clone())
            .collect();
        for seat in seats {
            self.cancel_drop_seat(&seat);
        }
    }
    fn current_drop(&self, seat: &ObjectId, serial: u64) -> bool {
        self.file_drops
            .sessions
            .get(seat)
            .is_some_and(|session| session.admission.matches(serial))
    }
    fn arm_drop_deadline(&mut self, seat: &ObjectId) -> bool {
        let Some(session) = self.file_drops.sessions.get_mut(seat) else {
            return false;
        };
        if session.deadline.is_some() {
            return true;
        }
        let deadline = Instant::now() + TIMEOUT;
        session.deadline = Some(deadline);
        let serial = session.admission.serial;
        let seat = seat.clone();
        let key = seat.clone();
        match self
            .loop_handle
            .insert_source(Timer::from_deadline(deadline), move |_, _, state| {
                if state.current_drop(&key, serial) {
                    // This callback owns its timer removal; cancel removes the other source.
                    state.file_drops.sessions.get_mut(&key).unwrap().timer = None;
                    state.cancel_drop_seat(&key);
                }
                TimeoutAction::Drop
            }) {
            Ok(token) => {
                self.file_drops.sessions.get_mut(&seat).unwrap().timer = Some(token);
                true
            }
            Err(_) => {
                self.cancel_drop_seat(&seat);
                false
            }
        }
    }
    fn start_drop_read(&mut self, seat: &ObjectId) {
        let Some(session) = self.file_drops.sessions.get(seat) else {
            return;
        };
        let serial = session.admission.serial;
        let Some(deadline) = session.deadline else {
            self.cancel_drop_seat(seat);
            return;
        };
        let pipe = match session.offer.receive(MIME.to_owned()) {
            Ok(pipe) => pipe,
            Err(_) => {
                self.cancel_drop_seat(seat);
                return;
            }
        };
        let nonblock = rustix::fs::fcntl_getfl(&pipe)
            .and_then(|flags| rustix::fs::fcntl_setfl(&pipe, flags | rustix::fs::OFlags::NONBLOCK));
        if nonblock.is_err() {
            self.cancel_drop_seat(seat);
            return;
        }
        let file = File::from(std::os::fd::OwnedFd::from(pipe));
        let source = Generic::new(file, Interest::READ, Mode::Level);
        let key = seat.clone();
        // Fixed allocation; the one-byte overflow probe never grows this body.
        let mut buffer = vec![0; MAX_URI_BYTES];
        let mut used = 0;
        match self
            .loop_handle
            .insert_source(source, move |_, file, state| {
                if !state.current_drop(&key, serial) {
                    return Ok(PostAction::Remove);
                }
                let window = state.file_drops.sessions.get(&key).unwrap().window;
                if Instant::now() >= deadline || !state.windows.borrow().contains_key(&window) {
                    state.file_drops.sessions.get_mut(&key).unwrap().pipe = None;
                    state.cancel_drop_seat(&key);
                    return Ok(PostAction::Remove);
                }
                match read_uri_chunk(&**file, &mut buffer, &mut used) {
                    ReadProgress::Pending => Ok(PostAction::Continue),
                    ReadProgress::Complete => {
                        state.file_drops.sessions.get_mut(&key).unwrap().pipe = None;
                        state.complete_file_drop(&key, serial, &buffer[..used]);
                        Ok(PostAction::Remove)
                    }
                    ReadProgress::Rejected => {
                        state.file_drops.sessions.get_mut(&key).unwrap().pipe = None;
                        state.cancel_drop_seat(&key);
                        Ok(PostAction::Remove)
                    }
                }
            }) {
            Ok(token) => self.file_drops.sessions.get_mut(seat).unwrap().pipe = Some(token),
            Err(_) => self.cancel_drop_seat(seat),
        }
    }
    fn complete_file_drop(&mut self, seat: &ObjectId, serial: u64, bytes: &[u8]) {
        if !self.current_drop(seat, serial) {
            return;
        }
        let session = self.file_drops.sessions.get(seat).unwrap();
        let paths = parse_uri_list(bytes);
        let valid = session.admission.can_publish(
            serial,
            copy_selected(&session.offer),
            session.deadline,
            Instant::now(),
            self.windows.borrow().contains_key(&session.window),
        );
        let mut published = false;
        if valid {
            if let Ok(paths) = paths {
                published = true;
                let window = session.window;
                // Finish only a valid complete COPY. v1/v2 have no finish request.
                if session.offer.inner().version() >= 3 {
                    session.offer.finish();
                }
                for path in paths {
                    self.events_sink
                        .push_window_event(WindowEvent::DroppedFile(path), window);
                }
                self.dispatched_events = true;
            }
        }
        self.retire_drop_seat(seat, !published);
    }
}

impl DataDeviceHandler for WinitState {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        device: &WlDataDevice,
        _: f64,
        _: f64,
        surface: &WlSurface,
    ) {
        let Some(seat) = device_seat(device) else {
            if let Some(offer) = device_offer(device) {
                self.file_drops
                    .early_actions
                    .take(&offer.inner().id(), Instant::now());
            }
            return;
        };
        // Every enter replaces the previous seat session, including a null offer.
        self.cancel_drop_seat(&seat);
        let Some(mut offer) = device_offer(device) else {
            return;
        };
        // Consume even when this enter cannot be admitted; no offer may replay
        // another enter's saved action. SCTK has already released its mutex here.
        let early = self
            .file_drops
            .early_actions
            .take(&offer.inner().id(), Instant::now());
        if let Some(selected) = early {
            let Ok(action) = selected else {
                offer.accept_mime_type(offer.serial, None);
                destroy(&offer);
                return;
            };
            offer.selected_action = reconcile_action(offer.selected_action, action);
        }
        // A pre-enter MOVE/ASK can be provisional. Retain it while advertising
        // only COPY below, but never read/finish before a dropped confirmed COPY.
        let window = super::make_wid(surface);
        let mime = offer.with_mime_types(|types| types.iter().any(|mime| mime == MIME));
        if !mime
            || !self.windows.borrow().contains_key(&window)
            || !can_admit(self.file_drops.sessions.len())
        {
            offer.accept_mime_type(offer.serial, None);
            destroy(&offer);
            return;
        }
        let Some(serial) = self.file_drops.serial.checked_add(1) else {
            destroy(&offer);
            return;
        };
        self.file_drops.serial = serial;
        offer.set_actions(DndAction::Copy, DndAction::Copy);
        offer.accept_mime_type(offer.serial, Some(MIME.to_owned()));
        self.file_drops.sessions.insert(
            seat,
            Session {
                admission: Admission::new(serial),
                window,
                offer,
                deadline: None,
                pipe: None,
                timer: None,
            },
        );
    }
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, device: &WlDataDevice) {
        if let Some(seat) = device_seat(device) {
            if self
                .file_drops
                .sessions
                .get(&seat)
                .is_some_and(|session| session.admission.cancel_on_leave())
            {
                self.cancel_drop_seat(&seat);
            }
        }
    }
    fn motion(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice, _: f64, _: f64) {}
    // Never change clipboard selection. SCTK owns its ordinary selection-offer lifetime.
    fn selection(&mut self, _: &Connection, _: &QueueHandle<Self>, device: &WlDataDevice) {
        let now = Instant::now();
        self.file_drops.early_actions.prune(now);
        if let Some(offer) = device
            .data::<DataDeviceData>()
            .and_then(|data| data.selection_offer())
        {
            self.file_drops.early_actions.take(&offer.inner().id(), now);
        }
    }
    fn drop_performed(&mut self, _: &Connection, _: &QueueHandle<Self>, device: &WlDataDevice) {
        let Some(seat) = device_seat(device) else {
            return;
        };
        let Some(offer) = device_offer(device) else {
            self.cancel_drop_seat(&seat);
            return;
        };
        let Some(session) = self.file_drops.sessions.get_mut(&seat) else {
            destroy(&offer);
            return;
        };
        if session.offer.inner().id() != offer.inner().id() {
            self.cancel_drop_seat(&seat);
            return;
        }
        let selected = reconcile_action(offer.selected_action, session.offer.selected_action);
        session.offer = offer;
        session.offer.selected_action = selected;
        if session.offer.inner().version() >= 3
            && !selected.is_empty()
            && selected != DndAction::Copy
        {
            // A real drop arrived: mark admission before retirement so rejected
            // MOVE/ASK produces the existing cancellation feedback, never finish.
            session.admission.drop_requested(false);
            session.offer.accept_mime_type(session.offer.serial, None);
            self.cancel_drop_seat(&seat);
            return;
        }
        let start = session
            .admission
            .drop_requested(copy_selected(&session.offer));
        if !self.arm_drop_deadline(&seat) {
            return;
        }
        if start {
            self.start_drop_read(&seat);
        }
    }
}
impl DataOfferHandler for WinitState {
    fn source_actions(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        offer: &mut DragOffer,
        actions: DndAction,
    ) {
        // SCTK invokes this with its offer-data mutex HELD: never call drag_offer or
        // with_mime_types here. Proxy IDs/requests do not lock that data.
        let seat = self
            .file_drops
            .sessions
            .iter()
            .find(|(_, session)| session.offer.inner().id() == offer.inner().id())
            .map(|(seat, _)| seat.clone());
        if let Some(seat) = seat {
            if !actions.contains(DndAction::Copy) {
                self.cancel_drop_seat(&seat);
            }
        }
    }
    fn selected_action(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        offer: &mut DragOffer,
        action: DndAction,
    ) {
        let seat = self
            .file_drops
            .sessions
            .iter()
            .find(|(_, session)| session.offer.inner().id() == offer.inner().id())
            .map(|(seat, _)| seat.clone());
        let Some(seat) = seat else {
            return;
        };
        let session = self.file_drops.sessions.get_mut(&seat).unwrap();
        session.offer.selected_action = action;
        if session.admission.rejects_action(action) {
            session.offer.accept_mime_type(session.offer.serial, None);
            self.cancel_drop_seat(&seat);
            return;
        }
        if session.admission.select_copy(action == DndAction::Copy) {
            self.start_drop_read(&seat);
        }
    }
}
// Only incoming data devices/offers; deliberately no data source or clipboard mutation.
sctk::reexports::client::delegate_dispatch!(WinitState: [WlDataDeviceManager: GlobalData] => DataDeviceManagerState);
sctk::reexports::client::delegate_dispatch!(WinitState: [WlDataDevice: DataDeviceData] => DataDeviceManagerState);
// Preserve SCTK's manager, data, queue and every ordinary offer event. Only
// interpose before its Action handler can discard an Undetermined offer action.
impl Dispatch<WlDataOffer, DataOfferData> for WinitState {
    fn event(
        state: &mut Self,
        offer: &WlDataOffer,
        event: wl_data_offer::Event,
        data: &DataOfferData,
        conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let now = Instant::now();
        state.file_drops.early_actions.prune(now);
        if let wl_data_offer::Event::Action { dnd_action } = &event {
            let seat = state
                .file_drops
                .sessions
                .iter()
                .find(|(_, session)| session.offer.inner().id() == offer.id())
                .map(|(seat, _)| seat.clone());
            if let Some(seat) = seat {
                // SCTK ignores unknown values even after enter. Retire our
                // authority first; forwarding still occurs exactly once below.
                if confirmed_action(*dnd_action).is_err() {
                    state.cancel_drop_seat(&seat);
                }
            } else if offer.is_alive() {
                state
                    .file_drops
                    .early_actions
                    .capture(offer.id(), *dnd_action, now);
            }
        }
        <DataDeviceManagerState as Dispatch<WlDataOffer, DataOfferData, WinitState>>::event(
            state, offer, event, data, conn, qh,
        );
    }
}

/// Reject the entire list on any invalid URI: never publish partially validated paths.
fn parse_uri_list(bytes: &[u8]) -> Result<Vec<PathBuf>, ()> {
    if bytes.len() > MAX_URI_BYTES {
        return Err(());
    }
    let mut paths = Vec::new();
    for (visited, line) in bytes.split(|byte| *byte == b'\n').enumerate() {
        if visited == MAX_LINES {
            return Err(());
        }
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() || line.starts_with(b"#") {
            continue;
        }
        // Native composer owns one attachment: multiple paths reject the entire list.
        if !paths.is_empty() {
            return Err(());
        }
        let raw = line.strip_prefix(b"file://").ok_or(())?;
        if !raw.starts_with(b"/")
            || raw.iter().any(|byte| {
                byte.is_ascii_control()
                    || byte.is_ascii_whitespace()
                    || *byte == b'?'
                    || *byte == b'#'
            })
        {
            return Err(());
        }
        let mut decoded = Vec::with_capacity(raw.len().min(MAX_PATH_BYTES));
        let mut index = 0;
        while index < raw.len() {
            let byte = if raw[index] == b'%' {
                if index + 2 >= raw.len() {
                    return Err(());
                }
                let high = hex(raw[index + 1]).ok_or(())?;
                let low = hex(raw[index + 2]).ok_or(())?;
                index += 3;
                (high << 4) | low
            } else {
                let byte = raw[index];
                index += 1;
                byte
            };
            if decoded.len() == MAX_PATH_BYTES || byte == 0 {
                return Err(());
            }
            decoded.push(byte);
        }
        let path = String::from_utf8(decoded).map_err(|_| ())?;
        if path.chars().any(char::is_control) || !Path::new(&path).is_absolute() {
            return Err(());
        }
        paths.push(PathBuf::from(path));
    }
    if paths.is_empty() {
        Err(())
    } else {
        Ok(paths)
    }
}
fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_provisional_move_hover_renegotiates_copy_but_move_drop_notifies() {
        let now = Instant::now();
        let mut gate = Admission::new(1);
        let mut selected = reconcile_action(DndAction::empty(), DndAction::Move);
        assert!(!gate.rejects_action(selected)); // preference has not been negotiated yet
        assert!(!gate.select_copy(selected == DndAction::Copy));
        assert!(!gate.reading);
        // A real post-enter compositor COPY callback confirms renegotiation.
        selected = confirmed_action(WEnum::Value(DndAction::Copy)).unwrap();
        assert!(!gate.rejects_action(selected));
        assert!(!gate.select_copy(selected == DndAction::Copy)); // still hovering
        assert!(gate.drop_requested(selected == DndAction::Copy));
        assert!(gate.can_publish(1, true, Some(now + TIMEOUT), now, true));
        for action in [DndAction::Move, DndAction::Ask] {
            let mut rejected = Admission::new(2);
            assert!(!rejected.rejects_action(action));
            assert!(!rejected.drop_requested(false)); // real non-COPY drop, not a hover
            assert!(rejected.rejects_action(action));
            assert!(!rejected.reading);
            assert!(rejected.notify_cancellation(true, true));
            assert!(!rejected.can_publish(2, false, Some(now + TIMEOUT), now, true));
        }
    }

    #[test]
    fn production_early_action_capture_consumption_and_drop_snapshot_link() {
        let now = Instant::now();
        let mut cache = EarlyActions::default();
        let key = (17_u32, 1_u64);
        assert!(cache.capture(key, WEnum::Value(DndAction::Copy), now));
        // The actual raw Action survives SCTK's empty Undetermined -> Drag snapshot.
        let confirmed = cache.take(&key, now).unwrap().unwrap();
        let mut local = reconcile_action(DndAction::empty(), confirmed);
        assert_eq!(local, DndAction::Copy);
        // The later dropped toolkit snapshot is empty too; confirmed COPY remains.
        local = reconcile_action(DndAction::empty(), local);
        let mut gate = Admission::new(1);
        assert!(gate.drop_requested(local == DndAction::Copy));
        assert!(gate.can_publish(1, local == DndAction::Copy, Some(now + TIMEOUT), now, true));
        assert_eq!(cache.take(&key, now), None); // no replay of a consumed offer
                                                 // Actual post-enter None overwrites local authority, unlike a lost snapshot.
        local = confirmed_action(WEnum::Value(DndAction::empty())).unwrap();
        local = reconcile_action(DndAction::empty(), local);
        assert!(local.is_empty());
        assert!(!gate.can_publish(1, local == DndAction::Copy, Some(now + TIMEOUT), now, true));
        assert_eq!(
            reconcile_action(DndAction::Move, DndAction::Copy),
            DndAction::Move
        );
        assert_eq!(
            reconcile_action(DndAction::Ask, DndAction::Copy),
            DndAction::Ask
        );
    }

    #[test]
    fn production_early_actions_latest_none_move_ask_and_unknown_fail_closed() {
        let now = Instant::now();
        let mut cache = EarlyActions::default();
        for latest in [
            WEnum::Value(DndAction::empty()),
            WEnum::Value(DndAction::Move),
            WEnum::Value(DndAction::Ask),
            WEnum::Unknown(128),
        ] {
            assert!(cache.capture(1_u64, WEnum::Value(DndAction::Copy), now));
            assert!(cache.capture(1_u64, latest, now));
            let selected = cache.take(&1, now).unwrap();
            assert_eq!(selected, confirmed_action(latest));
            assert_ne!(selected, Ok(DndAction::Copy));
            assert!(!Admission::new(1).drop_requested(selected == Ok(DndAction::Copy)));
        }
        assert!(confirmed_action(WEnum::Unknown(128)).is_err());
        // A later real compositor COPY is confirmation, not source-flag inference.
        assert!(cache.capture(1_u64, WEnum::Unknown(128), now));
        assert!(cache.capture(1_u64, WEnum::Value(DndAction::Copy), now));
        assert_eq!(cache.take(&1, now), Some(Ok(DndAction::Copy)));
    }

    #[test]
    fn production_early_action_capacity_expiry_and_no_lease_extension() {
        let now = Instant::now();
        let mut cache = EarlyActions::default();
        for key in 0..MAX_TRANSFERS {
            assert!(cache.capture(key, WEnum::Value(DndAction::Copy), now));
        }
        assert!(!cache.capture(MAX_TRANSFERS, WEnum::Value(DndAction::Copy), now));
        assert_eq!(cache.entries.len(), MAX_TRANSFERS);
        assert!(cache.capture(0, WEnum::Value(DndAction::Move), now + TIMEOUT / 2));
        assert_eq!(cache.take(&0, now + TIMEOUT), None); // exact deadline, no extension
        assert!(cache.entries.is_empty());
        assert!(cache.capture(MAX_TRANSFERS, WEnum::Value(DndAction::Copy), now + TIMEOUT));
        assert_eq!(
            cache.take(&MAX_TRANSFERS, now + TIMEOUT),
            Some(Ok(DndAction::Copy))
        );
    }

    #[test]
    fn production_early_action_generation_keys_and_selection_retirement() {
        let now = Instant::now();
        let mut cache = EarlyActions::default();
        // Production uses complete backend ObjectId, not just protocol_id().
        let old_generation = (42_u32, 1_u64);
        let replacement = (42_u32, 2_u64);
        assert!(cache.capture(old_generation, WEnum::Value(DndAction::Copy), now));
        assert_eq!(cache.take(&replacement, now), None);
        assert_eq!(cache.take(&old_generation, now), Some(Ok(DndAction::Copy)));
        assert!(cache.capture(replacement, WEnum::Value(DndAction::Copy), now));
        // Same take path is used by selection/nonadmitted enter without requests
        // or changing SCTK's clipboard/ordinary selection-offer lifetime.
        cache.take(&replacement, now);
        assert_eq!(cache.take(&replacement, now), None);
    }
    #[test]
    fn local_uri_list_decodes_exact_utf8_comments_and_crlf() {
        assert_eq!(
            parse_uri_list(b"# PUBLIC comment\r\n\r\nfile:///tmp/PUBLIC%20image.png\r\n").unwrap(),
            vec![PathBuf::from("/tmp/PUBLIC image.png")]
        );
        assert_eq!(
            parse_uri_list(b"file:///tmp/%E4%B8%96%E7%95%8C%25.txt\n").unwrap(),
            vec![PathBuf::from("/tmp/世界%.txt")]
        );
        assert_eq!(
            parse_uri_list(b"file:///tmp/%3F%23%40").unwrap(),
            vec![PathBuf::from("/tmp/?#@")]
        );
    }
    #[test]
    fn malformed_remote_and_unsupported_uris_reject_whole_list() {
        for bad in [
            b"https://example.test/file".as_slice(),
            b"file://remote/tmp/file",
            b"file://user@host/tmp/file",
            b"file://localhost/tmp/file",
            b"file:relative",
            b"file:///tmp/file?query",
            b"file:///tmp/file#fragment",
            b"file:///tmp/%",
            b"file:///tmp/%0",
            b"file:///tmp/%GG",
            b"file:///tmp/%00",
            b"file:///tmp/%0a",
            b"file:///tmp/%C2%85",
            b"file:///tmp/%FF",
            b"file:///tmp/raw space",
            b"file:///tmp/raw\t",
            b"",
        ] {
            assert!(parse_uri_list(bad).is_err());
            let mut list = b"file:///tmp/PUBLIC-valid\n".to_vec();
            list.extend_from_slice(bad);
            if !bad.is_empty() {
                assert!(parse_uri_list(&list).is_err());
            }
        }
    }
    #[test]
    fn exact_byte_path_and_item_limits() {
        let path = format!("file:///{}", "a".repeat(MAX_PATH_BYTES - 1));
        assert_eq!(
            parse_uri_list(path.as_bytes()).unwrap()[0]
                .as_os_str()
                .len(),
            MAX_PATH_BYTES
        );
        assert!(parse_uri_list(format!("{path}a").as_bytes()).is_err());
        let uri = b"file:///tmp/PUBLIC";
        assert!(parse_uri_list(&b"file:///tmp/PUBLIC\n".repeat(2)).is_err());
        let mut lines = b"# PUBLIC comment\n".repeat(MAX_LINES - 1);
        lines.extend_from_slice(uri);
        assert_eq!(parse_uri_list(&lines).unwrap().len(), 1);
        lines.extend_from_slice(b"\n# one over");
        assert!(parse_uri_list(&lines).is_err());
        let mut bytes = vec![b'#'; MAX_URI_BYTES];
        assert!(parse_uri_list(&bytes).is_err()); // only a comment is not a drop
        bytes.push(b'#');
        assert!(parse_uri_list(&bytes).is_err());
    }
    #[test]
    fn production_reader_budget_ceiling_probe_and_io_errors() {
        let mut bytes = vec![0; MAX_URI_BYTES];
        let mut used = 0;
        let mut source = io::Cursor::new(vec![b'x'; MAX_URI_BYTES]);
        assert_eq!(
            read_uri_chunk(&mut source, &mut bytes, &mut used),
            ReadProgress::Pending
        );
        assert_eq!(used, DISPATCH_BYTES);
        for _ in 1..MAX_URI_BYTES / DISPATCH_BYTES {
            assert_eq!(
                read_uri_chunk(&mut source, &mut bytes, &mut used),
                ReadProgress::Pending
            );
        }
        assert_eq!(used, MAX_URI_BYTES);
        assert_eq!(
            read_uri_chunk(&mut source, &mut bytes, &mut used),
            ReadProgress::Complete
        );
        let mut used = MAX_URI_BYTES;
        assert_eq!(
            read_uri_chunk(&b"x"[..], &mut bytes, &mut used),
            ReadProgress::Rejected
        );
        assert_eq!(used, MAX_URI_BYTES);
        struct ErrorReader {
            kind: io::ErrorKind,
            calls: usize,
        }
        impl Read for ErrorReader {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                self.calls += 1;
                Err(io::Error::from(self.kind))
            }
        }
        for (kind, expected, calls) in [
            (io::ErrorKind::WouldBlock, ReadProgress::Pending, 1),
            (io::ErrorKind::Interrupted, ReadProgress::Pending, 16),
            (io::ErrorKind::BrokenPipe, ReadProgress::Rejected, 1),
        ] {
            let mut reader = ErrorReader { kind, calls: 0 };
            let mut used = 0;
            assert_eq!(read_uri_chunk(&mut reader, &mut bytes, &mut used), expected);
            assert_eq!(reader.calls, calls);
            assert_eq!(used, 0);
        }
    }
    #[test]
    fn production_reader_handles_real_nonblocking_io_then_eof() {
        use std::{io::Write, os::unix::net::UnixStream};
        let (mut writer, reader) = UnixStream::pair().unwrap();
        reader.set_nonblocking(true).unwrap();
        let mut bytes = vec![0; MAX_URI_BYTES];
        let mut used = 0;
        assert_eq!(
            read_uri_chunk(&reader, &mut bytes, &mut used),
            ReadProgress::Pending
        );
        writer.write_all(b"file:///tmp/PUBLIC").unwrap();
        assert_eq!(
            read_uri_chunk(&reader, &mut bytes, &mut used),
            ReadProgress::Pending
        );
        drop(writer);
        assert_eq!(
            read_uri_chunk(&reader, &mut bytes, &mut used),
            ReadProgress::Complete
        );
        assert_eq!(
            parse_uri_list(&bytes[..used]).unwrap(),
            vec![PathBuf::from("/tmp/PUBLIC")]
        );
    }
    #[test]
    fn failed_drop_notification_is_not_success_or_uncommitted_hover() {
        let mut gate = Admission::new(1);
        assert!(!gate.notify_cancellation(true, true));
        assert!(gate.drop_requested(true));
        assert!(gate.notify_cancellation(true, true));
        assert!(!gate.notify_cancellation(false, true));
        assert!(!gate.notify_cancellation(true, false));
    }
    #[test]
    fn production_publication_gate_rejects_stale_closed_expired_or_wrong_action() {
        let now = Instant::now();
        let deadline = Some(now + TIMEOUT);
        let mut admission = Admission::new(12);
        assert!(!admission.can_publish(12, true, deadline, now, true));
        assert!(admission.drop_requested(true));
        assert!(admission.can_publish(12, true, deadline, now, true));
        assert!(!admission.can_publish(11, true, deadline, now, true));
        assert!(!admission.can_publish(12, true, deadline, now, false));
        assert!(!admission.can_publish(12, false, deadline, now, true));
        assert!(!admission.can_publish(12, true, deadline, now + TIMEOUT, true));
        assert!(!admission.can_publish(12, true, None, now, true));
    }
    #[test]
    fn production_admission_orders_copy_and_drop_once() {
        let mut gate = Admission::new(7);
        assert!(!gate.select_copy(true));
        assert!(gate.cancel_on_leave());
        assert!(gate.drop_requested(true));
        assert!(!gate.cancel_on_leave());
        assert!(!gate.drop_requested(true));
        assert!(!gate.select_copy(true));
        assert!(gate.matches(7));
        assert!(!gate.matches(6));
        let mut delayed = Admission::new(8);
        assert!(!delayed.drop_requested(false));
        assert!(!delayed.cancel_on_leave());
        assert!(delayed.select_copy(true));
        assert!(!delayed.select_copy(true));
        assert!(can_admit(MAX_TRANSFERS - 1));
        assert!(!can_admit(MAX_TRANSFERS));
        assert!(!can_admit(MAX_TRANSFERS + 1));
    }
}
