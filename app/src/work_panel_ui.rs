//! Read-only Work records panel controls. Opening, closing and section expansion
//! are presentation only: no variant can write, cancel, retry or reschedule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Opened from the existing More menu.
    Open,
    Close,
}
