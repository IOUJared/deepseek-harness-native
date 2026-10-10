//! Presentation follows Host status; sending/ACKs/animation never infer agent activity.
use super::*;
impl App {
    pub(super) fn running_status_live(&self) -> bool {
        self.ready && self.root_ready && !self.roster_pending && !self.stopped
    }
    pub(super) fn working_label(&self, session: &SessionSummary) -> Option<&'static str> {
        session.running.then_some(if self.running_status_live() {
            "Working"
        } else {
            "Status unavailable"
        })
    }
    pub(super) fn working_animation_active(&self) -> bool {
        // Focus is a conservative power gate, not a claim of Wayland visibility.
        if !self.window_active
            || !self.running_status_live()
            || !self.layout_controls_enabled()
            || self.capabilities
            || self.conversation_menu
        {
            return false;
        }
        let selected = !self.sidebar_sheet
            && self
                .selected
                .as_ref()
                .and_then(|id| self.sessions.get(id))
                .is_some_and(|session| session.running);
        let sidebar =
            self.sidebar_sheet || (!self.compact() && self.sidebar_motion.value() >= 185.0);
        selected || (sidebar && self.sidebar_working_presented())
    }
    fn sidebar_working_presented(&self) -> bool {
        // Find the first running row without allocating/sorting the whole navigation list.
        if !self.sessions.values().any(|session| session.running) { return false; }
        let Some(first) = self.navigation_candidates().filter(|session| session.running)
            .min_by(|a, b| self.navigation_order(a, b)) else { return false; };
        self.navigation_candidates().filter(|session| self.navigation_order(session, first).is_lt()).take(200).count() < 200
    }
    pub(super) fn navigation_order(&self, a: &SessionSummary, b: &SessionSummary) -> std::cmp::Ordering {
        self.management.registry.pin_rank(&a.session_id).unwrap_or(usize::MAX)
            .cmp(&self.management.registry.pin_rank(&b.session_id).unwrap_or(usize::MAX))
            .then(b.updated_at.cmp(&a.updated_at)).then(a.session_id.cmp(&b.session_id))
    }
    pub(super) fn working_badge(&self, session: &SessionSummary) -> Element<'_, Message> {
        let status = self.working_label(session).unwrap_or("");
        let mut badge = row![].spacing(6).align_y(iced::Alignment::Center);
        if session.running && self.running_status_live() {
            badge = badge.push(working_indicator::view(self.working_phase));
        }
        badge
            .push(
                label(
                    status,
                    if self.running_status_live() {
                        design::FOAM
                    } else {
                        MUTED
                    },
                )
                .size(11),
            )
            .into()
    }
}
