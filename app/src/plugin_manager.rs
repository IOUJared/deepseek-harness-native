//! The read-only plugin manager page.
//!
//! The reference's `PluginManagerPage` lists the profile's installed bundles, lets a reader switch
//! one on or off, and installs or removes packages (`ui-plugin-manager/src/client/PluginManagerPage.tsx`,
//! `manager-store.ts:776-830`). Native's first slice is the listing and its truthful unavailable state:
//! installation and enablement are Host mutations that need their own review/confirm round, so no
//! switch is drawn here yet.
use crate::design;
use crate::ui::{DANGER, MUTED, TEXT, label, short};
use dsh_native_transport::plugin::PluginBundle;
use iced::widget::{button, column, container, scrollable};
use iced::{Element, Length};

/// What the page asks the App to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Open,
    Refresh,
    /// Arm the review for one bundle switch; nothing is sent.
    AskBundle {
        name: String,
        enabled: bool,
    },
    /// Leave the armed review without writing.
    CancelBundle,
    /// Send the armed bundle switch; one explicit attempt.
    ConfirmBundle,
    /// Arm the removal review for one removable bundle; nothing is sent.
    AskRemove {
        name: String,
    },
    /// Leave the armed removal without writing.
    CancelRemove,
    /// Send the armed removal; one explicit destructive attempt.
    ConfirmRemove,
}

#[derive(Default)]
pub struct Manager {
    open: bool,
    loading: bool,
    loaded: bool,
    /// The Host's own answer from `pluginInventory/list`; a deployment without a manageable
    /// profile cannot install or switch plugins here.
    management_available: bool,
    bundles: Vec<PluginBundle>,
    error: Option<String>,
    /// The bundle switch under review: `(name, wanted enabled)`. Nothing is sent until Confirm.
    confirm: Option<(String, bool)>,
    /// The bundle switch in flight.
    pending: Option<(String, bool)>,
    /// The last switch's outcome copy.
    notice: Option<(String, bool)>,
    /// The bundle removal under review; nothing is sent until Confirm.
    remove_confirm: Option<String>,
    /// The removal in flight.
    remove_pending: Option<String>,
}

impl Manager {
    pub fn is_open(&self) -> bool {
        self.open
    }
    /// Whether a read has ever settled, so opening again does not queue a second read.
    pub fn loaded(&self) -> bool {
        self.loaded
    }
    /// Start a read, unless one is already in flight. Returns whether this call started one.
    pub fn begin(&mut self) -> bool {
        if self.loading {
            return false;
        }
        self.open = true;
        self.loading = true;
        self.error = None;
        true
    }
    pub fn close(&mut self) {
        self.open = false;
    }
    pub fn ask(&mut self, name: String, enabled: bool) {
        if self.pending.is_none() {
            self.confirm = Some((name, enabled));
            self.notice = None;
        }
    }
    pub fn cancel(&mut self) {
        self.confirm = None;
    }
    /// Take the armed switch into flight. The caller sends it; a second press cannot duplicate it.
    pub fn confirm_now(&mut self) -> Option<(String, bool)> {
        if self.pending.is_some() {
            return None;
        }
        let (name, enabled) = self.confirm.take()?;
        self.pending = Some((name.clone(), enabled));
        self.notice = None;
        Some((name, enabled))
    }
    /// Adopt one switch's settlement and say whether the list should be re-read.
    pub fn changed(
        &mut self,
        name: String,
        enabled: bool,
        result: Result<dsh_native_transport::plugin::PluginChange, String>,
    ) -> bool {
        self.pending = None;
        match result {
            Ok(change) => {
                let verb = if enabled { "Enabled" } else { "Disabled" };
                self.notice = Some(match change.application {
                    dsh_native_transport::plugin::PluginApplication::Applied => {
                        (format!("{verb} {name}."), false)
                    }
                    dsh_native_transport::plugin::PluginApplication::RestartRequired => (
                        format!("Change saved. It takes effect at the next start."),
                        false,
                    ),
                    dsh_native_transport::plugin::PluginApplication::Cancelled => {
                        (format!("The change to {name} was cancelled."), false)
                    }
                    dsh_native_transport::plugin::PluginApplication::Overridden
                    | dsh_native_transport::plugin::PluginApplication::Failed => {
                        let code = change
                            .error
                            .as_deref()
                            .map(|code| format!(" ({code})"))
                            .unwrap_or_default();
                        (format!("The Host did not change {name}{code}."), true)
                    }
                });
                true
            }
            Err(error) => {
                self.notice = Some((
                    format!("Could not change {name}: {}", short(&error, 80)),
                    true,
                ));
                true
            }
        }
    }
    pub fn ask_remove(&mut self, name: String) {
        if self.remove_pending.is_none() {
            self.remove_confirm = Some(name);
            self.notice = None;
        }
    }
    pub fn cancel_remove(&mut self) {
        self.remove_confirm = None;
    }
    /// Take the armed removal into flight. The caller sends it; a second press cannot duplicate it.
    pub fn confirm_remove(&mut self) -> Option<String> {
        if self.remove_pending.is_some() {
            return None;
        }
        let name = self.remove_confirm.take()?;
        self.remove_pending = Some(name.clone());
        self.notice = None;
        Some(name)
    }
    /// Adopt one removal's settlement and say whether the list should be re-read.
    pub fn removed(
        &mut self,
        name: String,
        result: Result<dsh_native_transport::plugin::PluginChange, String>,
    ) -> bool {
        self.remove_pending = None;
        match result {
            Ok(change) => {
                self.notice = Some(match change.application {
                    dsh_native_transport::plugin::PluginApplication::Applied => {
                        (format!("Uninstalled {name}."), false)
                    }
                    dsh_native_transport::plugin::PluginApplication::RestartRequired => (
                        "Removal saved. It takes effect at the next start.".to_owned(),
                        false,
                    ),
                    dsh_native_transport::plugin::PluginApplication::Cancelled => {
                        (format!("The removal of {name} was cancelled."), false)
                    }
                    dsh_native_transport::plugin::PluginApplication::Overridden
                    | dsh_native_transport::plugin::PluginApplication::Failed => {
                        let code = change
                            .error
                            .as_deref()
                            .map(|code| format!(" ({code})"))
                            .unwrap_or_default();
                        (format!("The Host did not uninstall {name}{code}."), true)
                    }
                });
                true
            }
            Err(error) => {
                self.notice = Some((
                    format!("Could not uninstall {name}: {}", short(&error, 80)),
                    true,
                ));
                true
            }
        }
    }
    /// The armed removal, for the fixture and its pinning test.
    #[cfg_attr(not(any(test, feature = "public-layout-fixture")), allow(dead_code))]
    pub fn remove_armed(&self) -> Option<&str> {
        self.remove_confirm.as_deref()
    }
    /// Seed the armed removal for a public fixture: no Host ran and nothing is sent.
    #[cfg_attr(not(any(test, feature = "public-layout-fixture")), allow(dead_code))]
    pub fn seed_remove(&mut self, name: &str) {
        self.remove_confirm = Some(name.to_owned());
    }
    /// The armed review, for the fixture and its pinning test.
    #[cfg_attr(not(any(test, feature = "public-layout-fixture")), allow(dead_code))]
    pub fn review_armed(&self) -> Option<(&str, bool)> {
        self.confirm
            .as_ref()
            .map(|(name, enabled)| (name.as_str(), *enabled))
    }
    /// Seed the armed review for a public fixture: no Host ran and nothing is sent.
    #[cfg_attr(not(any(test, feature = "public-layout-fixture")), allow(dead_code))]
    pub fn seed_confirm(&mut self, name: &str, enabled: bool) {
        self.confirm = Some((name.to_owned(), enabled));
    }
    /// Adopt one settled read: the Host's profile availability and, when manageable, its bundles.
    pub fn settled(&mut self, result: Result<(bool, Vec<PluginBundle>), String>) {
        self.loading = false;
        self.loaded = true;
        match result {
            Ok((available, bundles)) => {
                self.management_available = available;
                self.bundles = bundles;
                self.error = None;
            }
            Err(error) => {
                self.error = Some(error);
                self.bundles.clear();
            }
        }
    }
    /// Seed the page for a public fixture: no Host ran.
    #[cfg_attr(not(any(test, feature = "public-layout-fixture")), allow(dead_code))]
    pub fn seed(&mut self, available: bool, bundles: Vec<PluginBundle>) {
        self.open = true;
        self.loading = false;
        self.loaded = true;
        self.management_available = available;
        self.bundles = bundles;
        self.error = None;
    }
    pub fn len(&self) -> usize {
        self.bundles.len()
    }
    pub fn view(&self) -> Element<'_, crate::ui::Message> {
        use crate::ui::Message;
        let mut body = column![
            iced::widget::row![
                label("Plugins", TEXT).size(24).width(Length::Fill),
                button(label("Close", MUTED).size(12))
                    .padding([5, 10])
                    .style(design::ghost)
                    .on_press(Message::DismissPanel),
            ]
            .align_y(iced::Alignment::Center),
            label(
                "Installed bundles and their profile state. Switching or uninstalling a bundle \
                 takes its own explicit confirmation; installing a package is not offered here.",
                MUTED,
            )
            .size(12),
            button(if self.loading {
                "Reading…"
            } else {
                "Refresh"
            })
            .padding([8, 12])
            .style(design::ghost)
            .on_press_maybe((!self.loading).then_some(Message::PluginManager(Action::Refresh))),
        ]
        .spacing(12);
        if let Some((notice, danger)) = &self.notice {
            body = body
                .push(label(notice.clone(), if *danger { DANGER } else { design::FOAM }).size(12));
        }
        if let Some(error) = &self.error {
            body = body.push(label(short(error, 120), DANGER).size(12));
            body = body.push(
                button("Retry")
                    .padding([8, 12])
                    .style(design::ghost)
                    .on_press(Message::PluginManager(Action::Refresh)),
            );
        } else if self.loaded && !self.management_available {
            // `manager-store.ts:769-777` settles an unmanageable deployment as unavailable, not empty.
            body = body.push(label(
                "This deployment runs without a manageable profile, so plugins cannot be installed or switched here.",
                MUTED,
            ).size(12));
        } else if self.loading && !self.loaded {
            body = body.push(label("Reading plugins…", MUTED).size(12));
        } else if self.bundles.is_empty() {
            body = body.push(label("No plugins are installed yet.", MUTED).size(12));
        } else {
            let mut rows =
                column![label(format!("Bundles · {}", self.bundles.len()), MUTED,).size(11)]
                    .spacing(8);
            for bundle in &self.bundles {
                let version = bundle
                    .version
                    .as_deref()
                    .map(|version| format!(" · {version}"))
                    .unwrap_or_default();
                let state = format!(
                    "{}{}{}",
                    if bundle.enabled {
                        "enabled"
                    } else {
                        "disabled"
                    },
                    if bundle.installed {
                        " · installed"
                    } else {
                        " · shipped"
                    },
                    if bundle.optional { " · optional" } else { "" }
                );
                let mut row = column![
                    label(
                        format!("{}{version} · {state}", short(&bundle.name, 80)),
                        TEXT,
                    )
                    .size(13),
                ]
                .spacing(2);
                if let Some(description) = &bundle.description {
                    row = row.push(label(short(description, 160), MUTED).size(11));
                }
                let detail = format!(
                    "{} rows{}",
                    bundle.rows,
                    if bundle.overrides > 0 {
                        format!(" · {} overrides", bundle.overrides)
                    } else {
                        String::new()
                    }
                );
                row = row.push(
                    label(
                        format!(
                            "{detail}{}",
                            bundle
                                .read_only_reason
                                .as_deref()
                                .map(|reason| format!(" · read-only: {reason}"))
                                .unwrap_or_default()
                        ),
                        if bundle.error.is_some() {
                            DANGER
                        } else {
                            MUTED
                        },
                    )
                    .size(10),
                );
                // Enable/disable is a Host write: the control arms an explicit review and sends
                // only Confirm. A bundle the Host marks read-only offers no control.
                if self.management_available && bundle.read_only_reason.is_none() {
                    let wanted = !bundle.enabled;
                    let name = bundle.name.clone();
                    let pending = self
                        .pending
                        .as_ref()
                        .is_some_and(|(pending, _)| pending == &bundle.name);
                    let asking = self
                        .confirm
                        .as_ref()
                        .is_some_and(|(asking, _)| asking == &bundle.name);
                    if pending {
                        row = row.push(label("Changing…", MUTED).size(10));
                    } else if asking {
                        row = row.push(
                            iced::widget::row![
                                label(
                                    format!(
                                        "{} {} for the next start?",
                                        if wanted { "Enable" } else { "Disable" },
                                        short(&bundle.name, 40)
                                    ),
                                    TEXT,
                                )
                                .size(11)
                                .width(Length::Fill),
                                button(label("Cancel", MUTED).size(11))
                                    .padding([3, 8])
                                    .style(design::ghost)
                                    .on_press(Message::PluginManager(Action::CancelBundle)),
                                button(label("Confirm", design::ACCENT).size(11))
                                    .padding([3, 8])
                                    .style(design::ghost)
                                    .on_press(Message::PluginManager(Action::ConfirmBundle)),
                            ]
                            .spacing(6)
                            .align_y(iced::Alignment::Center),
                        );
                    } else {
                        row = row.push(
                            button(label(if wanted { "Enable" } else { "Disable" }, TEXT).size(11))
                                .padding([3, 8])
                                .style(design::ghost)
                                .on_press(Message::PluginManager(Action::AskBundle {
                                    name,
                                    enabled: wanted,
                                })),
                        );
                    }
                }
                // Removal is destructive, so it always carries the reference's own question
                // (`confirmUninstallTitle`); a shipped or Host-protected bundle offers nothing.
                if self.management_available
                    && bundle.removable
                    && bundle.read_only_reason.is_none()
                {
                    let name = bundle.name.clone();
                    if self
                        .remove_pending
                        .as_deref()
                        .is_some_and(|pending| pending == bundle.name)
                    {
                        row = row.push(label("Removing…", MUTED).size(10));
                    } else if self
                        .remove_confirm
                        .as_deref()
                        .is_some_and(|asking| asking == bundle.name)
                    {
                        row = row.push(
                            iced::widget::column![
                                label(format!("Uninstall \"{}\"?", short(&bundle.name, 48)), TEXT)
                                    .size(11),
                                label("What it provides goes away once it is uninstalled.", MUTED)
                                    .size(10),
                                iced::widget::row![
                                    button(label("Cancel", MUTED).size(11))
                                        .padding([3, 8])
                                        .style(design::ghost)
                                        .on_press(Message::PluginManager(Action::CancelRemove)),
                                    button(label("Uninstall", DANGER).size(11))
                                        .padding([3, 8])
                                        .style(design::ghost)
                                        .on_press(Message::PluginManager(Action::ConfirmRemove)),
                                ]
                                .spacing(6),
                            ]
                            .spacing(2),
                        );
                    } else {
                        row = row.push(
                            button(label("Uninstall…", MUTED).size(11))
                                .padding([3, 8])
                                .style(design::ghost)
                                .on_press(Message::PluginManager(Action::AskRemove { name })),
                        );
                    }
                }
                rows = rows.push(
                    container(row)
                        .padding([8, 10])
                        .width(Length::Fill)
                        .style(|_| design::card(design::OVERLAY, design::RADIUS_SM)),
                );
            }
            body = body.push(rows);
        }
        container(scrollable(body.width(Length::Fill)).height(Length::Fill))
            .padding([20, 24])
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
