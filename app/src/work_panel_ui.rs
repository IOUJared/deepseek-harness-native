//! Read-only Work records panel controls. Opening and closing are presentation only: no
//! variant can write, cancel, retry or reschedule. (An earlier comment also named "section
//! expansion"; the panel draws every section expanded and has no collapse control.)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Opened from the existing More menu.
    Open,
    Close,
}
