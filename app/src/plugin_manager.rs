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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Open,
    Refresh,
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
        let mut body =
            column![
            iced::widget::row![
                label("Plugins", TEXT).size(24).width(Length::Fill),
                button(label("Close", MUTED).size(12))
                    .padding([5, 10])
                    .style(design::ghost)
                    .on_press(Message::DismissPanel),
            ]
            .align_y(iced::Alignment::Center),
            label(
                "Installed bundles and their profile state. Installing, switching or removing a \
                 plugin is a Host write and is not offered here yet.",
                MUTED,
            )
            .size(12),
            button(if self.loading { "Reading…" } else { "Refresh" })
                .padding([8, 12])
                .style(design::ghost)
                .on_press_maybe(
                    (!self.loading).then_some(Message::PluginManager(Action::Refresh))
                ),
        ]
            .spacing(12);
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
