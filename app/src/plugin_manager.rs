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
use iced::widget::{button, column, container, scrollable, text_input};
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
    /// Edit the typed install spec. Never sends.
    EditSpec(String),
    /// Inspect the typed spec; nothing is installed.
    Inspect,
    /// Choose the registry the inspect asks first; `None` is the Host's configured default.
    SelectRegistry(Option<String>),
    /// Arm the review for one live plugin row; nothing is sent.
    AskRow {
        entry_id: String,
        enabled: bool,
    },
    /// Leave the armed row review without writing.
    CancelRow,
    /// Send the armed row switch; one explicit attempt.
    ConfirmRow,
    /// Arm the install review for the inspected spec; nothing is installed.
    AskInstall,
    /// Leave the armed install without writing.
    CancelInstall,
    /// Send the armed install; one explicit attempt.
    ConfirmInstall,
    /// Ask the Host to stop the installation in flight.
    CancelRunningInstall,
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
    /// The install spec being typed, and its last inspection.
    spec: String,
    inspecting: bool,
    inspection: Option<dsh_native_transport::plugin::PluginInspection>,
    /// The Host's registry order, read once for the Add-plugin form.
    registries: Option<dsh_native_transport::plugin::PluginRegistries>,
    registries_loading: bool,
    registries_loaded: bool,
    registry: Option<String>,
    /// The live profile entries, joined to the rows each bundle declares.
    rows: Vec<dsh_native_transport::plugin::PluginRow>,
    rows_loaded: bool,
    row_confirm: Option<(String, bool)>,
    row_pending: Option<(String, bool)>,
    /// The install review armed by the accepted preview, and the install in flight.
    install_confirm: bool,
    installing: bool,
    /// The request id this install was started with, kept until its result is known.
    install_request: Option<String>,
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
    pub fn begin_rows(&mut self) -> bool {
        if self.rows_loaded {
            return false;
        }
        self.rows_loaded = true;
        true
    }
    pub fn rows_settled(
        &mut self,
        result: Result<Vec<dsh_native_transport::plugin::PluginRow>, String>,
    ) {
        match result {
            Ok(rows) => self.rows = rows,
            Err(error) => {
                self.rows.clear();
                self.notice = Some((
                    format!("Could not read the plugin rows: {}", short(&error, 80)),
                    true,
                ));
            }
        }
    }
    pub fn ask_install(&mut self) {
        if !self.installing
            && matches!(
                self.inspection,
                Some(dsh_native_transport::plugin::PluginInspection::Accepted { .. })
            )
        {
            self.install_confirm = true;
            self.notice = None;
        }
    }
    pub fn cancel_install(&mut self) {
        self.install_confirm = false;
    }
    /// Take the armed install into flight; the caller sends it once.
    pub fn confirm_install(&mut self) -> Option<(String, Option<String>, String)> {
        if self.installing || !self.install_confirm {
            return None;
        }
        let spec = self.spec.trim().to_owned();
        if spec.is_empty() {
            return None;
        }
        self.install_confirm = false;
        self.installing = true;
        self.install_request = Some(next_install_request());
        self.notice = None;
        Some((spec, self.registry(), self.install_request.clone()?))
    }
    #[cfg_attr(not(any(test, feature = "public-layout-fixture")), allow(dead_code))]
    pub fn is_installing(&self) -> bool {
        self.installing
    }
    /// Seed the installing state for a public fixture: no Host run exists behind it.
    #[cfg_attr(not(any(test, feature = "public-layout-fixture")), allow(dead_code))]
    pub fn seed_installing(&mut self, request_id: &str) {
        self.installing = true;
        self.install_request = Some(request_id.to_owned());
    }
    /// The request id whose result is still unknown, for a recovery read.
    pub fn install_request(&self) -> Option<String> {
        self.install_request.clone()
    }
    /// Adopt a recovery read: an active result settles the install; `None` is an unknown outcome.
    pub fn wait_settled(
        &mut self,
        result: Result<Option<dsh_native_transport::plugin::PluginChange>, String>,
    ) -> bool {
        match result {
            Ok(Some(change)) => self.install_settled(Ok(change)),
            Ok(None) => {
                self.installing = false;
                self.install_request = None;
                self.notice = Some((
                    "Installation result unavailable; check the plugin list.".to_owned(),
                    true,
                ));
                true
            }
            Err(error) => {
                self.installing = false;
                self.install_request = None;
                self.notice = Some((
                    format!(
                        "Installation result unavailable ({}); check the plugin list.",
                        short(&error, 80)
                    ),
                    true,
                ));
                true
            }
        }
    }
    /// Adopt a cancellation answer. Returns whether the page should ask for the run's result.
    pub fn cancel_settled(
        &mut self,
        result: Result<dsh_native_transport::plugin::PluginCancellation, String>,
    ) -> bool {
        match result {
            Ok(dsh_native_transport::plugin::PluginCancellation::Cancelled) => {
                self.notice = Some(("Stopping the installation…".to_owned(), false));
                true
            }
            Ok(dsh_native_transport::plugin::PluginCancellation::TooLate) => {
                self.notice = Some((
                    "Cancellation was not confirmed; waiting for the installation's result."
                        .to_owned(),
                    true,
                ));
                true
            }
            Ok(dsh_native_transport::plugin::PluginCancellation::NotRunning) => {
                self.installing = false;
                self.install_request = None;
                self.notice = Some((
                    "The installation is no longer running; check the plugin list.".to_owned(),
                    true,
                ));
                true
            }
            Err(error) => {
                self.notice = Some((
                    format!(
                        "Cancellation was not confirmed ({}); waiting for the installation's result.",
                        short(&error, 80)
                    ),
                    true,
                ));
                true
            }
        }
    }
    /// Adopt one installation's settlement and say whether the list should be re-read.
    pub fn install_settled(
        &mut self,
        result: Result<dsh_native_transport::plugin::PluginChange, String>,
    ) -> bool {
        self.installing = false;
        match result {
            Ok(change)
                if change.application
                    == dsh_native_transport::plugin::PluginApplication::Applied
                    || change.application
                        == dsh_native_transport::plugin::PluginApplication::RestartRequired =>
            {
                self.install_request = None;
                let installed = change.bundle.as_deref().unwrap_or(change.target.as_str());
                let version = change
                    .version
                    .as_deref()
                    .map(|version| format!(" {version}"))
                    .unwrap_or_default();
                let restart = change.application
                    == dsh_native_transport::plugin::PluginApplication::RestartRequired;
                self.notice = Some((
                    format!(
                        "Installed {installed}{version}.{}",
                        if restart {
                            " It takes effect at the next start."
                        } else {
                            ""
                        }
                    ),
                    false,
                ));
                true
            }
            Ok(change) => {
                self.install_request = None;
                let mut notice = format!(
                    "The Host did not install the spec{}.",
                    change
                        .error
                        .as_deref()
                        .map(|code| format!(" ({code})"))
                        .unwrap_or_default()
                );
                if !change.pending_builds.is_empty() {
                    // The reference offers per-package script approval; this page does not yet.
                    notice.push_str(&format!(
                        " Awaiting script approval for {}; approving is not offered here.",
                        short(&change.pending_builds.join(", "), 120)
                    ));
                }
                self.notice = Some((notice, true));
                true
            }
            Err(error) => {
                // Keep the request id: the caller asks the Host for the active result first.
                self.installing = true;
                self.notice = Some((
                    format!(
                        "Installation result unavailable ({}); asking the Host for it.",
                        short(&error, 80)
                    ),
                    true,
                ));
                false
            }
        }
    }
    #[cfg_attr(not(any(test, feature = "public-layout-fixture")), allow(dead_code))]
    pub fn install_armed(&self) -> bool {
        self.install_confirm
    }
    #[cfg_attr(not(any(test, feature = "public-layout-fixture")), allow(dead_code))]
    pub fn seed_install_confirm(&mut self) {
        self.install_confirm = true;
    }
    pub fn ask_row(&mut self, entry_id: String, enabled: bool) {
        if self.row_pending.is_none() {
            self.row_confirm = Some((entry_id, enabled));
            self.notice = None;
        }
    }
    pub fn cancel_row(&mut self) {
        self.row_confirm = None;
    }
    pub fn confirm_row(&mut self) -> Option<(String, bool)> {
        if self.row_pending.is_some() {
            return None;
        }
        let (entry_id, enabled) = self.row_confirm.take()?;
        self.row_pending = Some((entry_id.clone(), enabled));
        self.notice = None;
        Some((entry_id, enabled))
    }
    /// Adopt one row switch's settlement and say whether the rows should be re-read.
    pub fn row_changed(
        &mut self,
        entry_id: String,
        enabled: bool,
        result: Result<dsh_native_transport::plugin::PluginChange, String>,
    ) -> bool {
        self.row_pending = None;
        self.row_confirm = None;
        match result {
            Ok(change) => {
                self.notice = Some(match change.application {
                    dsh_native_transport::plugin::PluginApplication::Applied => (
                        format!(
                            "{} {entry_id}.",
                            if enabled { "Enabled" } else { "Disabled" }
                        ),
                        false,
                    ),
                    dsh_native_transport::plugin::PluginApplication::RestartRequired => (
                        "Change saved. It takes effect at the next start.".to_owned(),
                        false,
                    ),
                    dsh_native_transport::plugin::PluginApplication::Cancelled => {
                        (format!("The change to {entry_id} was cancelled."), false)
                    }
                    dsh_native_transport::plugin::PluginApplication::Overridden
                    | dsh_native_transport::plugin::PluginApplication::Failed => {
                        let code = change
                            .error
                            .as_deref()
                            .map(|code| format!(" ({code})"))
                            .unwrap_or_default();
                        (format!("The Host did not change {entry_id}{code}."), true)
                    }
                });
                true
            }
            Err(error) => {
                self.notice = Some((
                    format!("Could not change {entry_id}: {}", short(&error, 80)),
                    true,
                ));
                true
            }
        }
    }
    #[cfg_attr(not(any(test, feature = "public-layout-fixture")), allow(dead_code))]
    pub fn rows(&self) -> &[dsh_native_transport::plugin::PluginRow] {
        &self.rows
    }
    #[cfg_attr(not(any(test, feature = "public-layout-fixture")), allow(dead_code))]
    pub fn seed_rows(&mut self, rows: Vec<dsh_native_transport::plugin::PluginRow>) {
        self.rows = rows;
        self.rows_loaded = true;
    }
    /// The armed row review, for the fixture and its pinning test.
    #[cfg_attr(not(any(test, feature = "public-layout-fixture")), allow(dead_code))]
    pub fn row_armed(&self) -> Option<(&str, bool)> {
        self.row_confirm
            .as_ref()
            .map(|(entry, enabled)| (entry.as_str(), *enabled))
    }
    #[cfg_attr(not(any(test, feature = "public-layout-fixture")), allow(dead_code))]
    pub fn seed_row_confirm(&mut self, entry_id: &str, enabled: bool) {
        self.row_confirm = Some((entry_id.to_owned(), enabled));
    }
    pub fn select_registry(&mut self, registry: Option<String>) {
        self.registry = registry;
        self.notice = None;
    }
    /// The registry the next inspect asks first; `None` is the Host's default.
    pub fn registry(&self) -> Option<String> {
        self.registry.clone()
    }
    /// Start the one registry read the Add-plugin form needs.
    pub fn begin_registries(&mut self) -> bool {
        if self.registries_loading || self.registries_loaded {
            return false;
        }
        self.registries_loading = true;
        true
    }
    pub fn registries_settled(
        &mut self,
        result: Result<dsh_native_transport::plugin::PluginRegistries, String>,
    ) {
        self.registries_loading = false;
        match result {
            Ok(registries) => {
                self.registries = Some(registries);
                self.registries_loaded = true;
            }
            Err(error) => {
                // The form stays usable with the Host's default registry; the failure is stated.
                self.notice = Some((
                    format!("Could not read the registries: {}", short(&error, 80)),
                    true,
                ));
            }
        }
    }
    #[cfg_attr(not(any(test, feature = "public-layout-fixture")), allow(dead_code))]
    pub fn registries(&self) -> Option<&dsh_native_transport::plugin::PluginRegistries> {
        self.registries.as_ref()
    }
    /// Seed the registry order for a public fixture: no Host was asked.
    #[cfg_attr(not(any(test, feature = "public-layout-fixture")), allow(dead_code))]
    pub fn seed_registries(&mut self, registries: dsh_native_transport::plugin::PluginRegistries) {
        self.registries = Some(registries);
        self.registries_loaded = true;
        self.registries_loading = false;
    }
    pub fn edit_spec(&mut self, text: String) {
        self.spec = text;
        self.notice = None;
    }
    /// Start one inspect read for the typed spec. Nothing is installed and a blank spec sends nothing.
    pub fn begin_inspect(&mut self) -> Option<String> {
        let spec = self.spec.trim().to_owned();
        if spec.is_empty() || self.inspecting {
            return None;
        }
        self.inspecting = true;
        self.inspection = None;
        self.notice = None;
        Some(spec)
    }
    pub fn inspected(
        &mut self,
        result: Result<dsh_native_transport::plugin::PluginInspection, String>,
    ) {
        self.inspecting = false;
        match result {
            Ok(inspection) => self.inspection = Some(inspection),
            Err(error) => {
                self.inspection = None;
                self.notice = Some((
                    format!("Could not inspect the spec: {}", short(&error, 80)),
                    true,
                ));
            }
        }
    }
    #[cfg_attr(not(any(test, feature = "public-layout-fixture")), allow(dead_code))]
    pub fn spec(&self) -> &str {
        &self.spec
    }
    #[cfg_attr(not(any(test, feature = "public-layout-fixture")), allow(dead_code))]
    pub fn inspection(&self) -> Option<&dsh_native_transport::plugin::PluginInspection> {
        self.inspection.as_ref()
    }
    /// Seed a settled inspection for a public fixture: no Host was asked.
    #[cfg_attr(not(any(test, feature = "public-layout-fixture")), allow(dead_code))]
    pub fn seed_inspection(
        &mut self,
        spec: &str,
        inspection: dsh_native_transport::plugin::PluginInspection,
    ) {
        self.spec = spec.to_owned();
        self.inspecting = false;
        self.inspection = Some(inspection);
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
                "Installed bundles and their profile state. Switching, uninstalling or installing \
                 a package takes its own explicit confirmation.",
                MUTED,
            )
            .size(12)
            .width(Length::Fill),
            button(if self.loading {
                "Reading…"
            } else {
                "Refresh"
            })
            .padding([8, 12])
            .style(design::ghost)
            .on_press_maybe((!self.loading).then_some(Message::PluginManager(Action::Refresh))),
            label("Add plugin", TEXT).size(13),
            {
                let mut options: Vec<Option<String>> = vec![None];
                if let Some(registries) = &self.registries {
                    for url in registries
                        .registry
                        .iter()
                        .chain(registries.fallback.iter())
                        .chain(registries.resolved.iter())
                    {
                        if !url.is_empty()
                            && !options.iter().any(|option| option.as_deref() == Some(url))
                        {
                            options.push(Some(url.clone()));
                        }
                    }
                }
                let mut row = iced::widget::row![
                    label("Registry", MUTED).size(11),
                    button(
                        label(
                            "Default registry",
                            if self.registry.is_none() { TEXT } else { MUTED }
                        )
                        .size(11)
                    )
                    .padding([3, 8])
                    .style(design::ghost)
                    .on_press(Message::PluginManager(Action::SelectRegistry(None))),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center);
                for option in options.into_iter().skip(1).flatten() {
                    let selected = self.registry.as_deref() == Some(option.as_str());
                    row = row.push(
                        button(
                            label(
                                if selected {
                                    format!("● {}", short(&option, 42))
                                } else {
                                    short(&option, 42)
                                },
                                if selected { TEXT } else { MUTED },
                            )
                            .size(11),
                        )
                        .padding([3, 8])
                        .style(design::ghost)
                        .on_press(Message::PluginManager(Action::SelectRegistry(Some(option)))),
                    );
                }
                row
            },
            iced::widget::row![
                text_input(
                    "Package name, local path, git URL or tarball",
                    self.spec.as_str()
                )
                .padding(8)
                .size(12)
                .style(design::field)
                .width(Length::Fill)
                .on_input(move |text| Message::PluginManager(Action::EditSpec(text))),
                button(if self.inspecting {
                    "Reading…"
                } else {
                    "Inspect"
                })
                .padding([8, 12])
                .style(design::ghost)
                .on_press_maybe(
                    (!self.inspecting).then_some(Message::PluginManager(Action::Inspect))
                ),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(12);
        if let Some(inspection) = &self.inspection {
            match inspection {
                dsh_native_transport::plugin::PluginInspection::Accepted {
                    kind,
                    name,
                    version,
                    description,
                    bundle,
                    registry,
                    host,
                } => {
                    let identity = match (name.as_deref(), version.as_deref()) {
                        (Some(name), Some(version)) => format!("{name} · {version}"),
                        (Some(name), None) => name.to_owned(),
                        _ => "Unnamed spec".to_owned(),
                    };
                    let mut card = column![
                        label(short(&identity, 100), TEXT).size(13),
                        label(
                            format!(
                                "{kind}{}",
                                match bundle {
                                    Some(true) => " · declares a bundle patch",
                                    Some(false) => " · no bundle patch",
                                    None => " · bundle unknown",
                                }
                            ),
                            MUTED,
                        )
                        .size(11),
                    ]
                    .spacing(2);
                    if let Some(description) = description {
                        card = card.push(label(short(description, 200), MUTED).size(11));
                    }
                    if let Some(registry) = registry {
                        card = card.push(label(format!("Registry {registry}"), MUTED).size(10));
                    }
                    if let Some(host) = host {
                        card = card.push(label(format!("Host {host}"), MUTED).size(10));
                    }
                    if self.installing {
                        card = card.push(
                            iced::widget::row![
                                label("Installing…", MUTED).size(10).width(Length::Fill),
                                button(label("Cancel installation", MUTED).size(10))
                                    .padding([2, 6])
                                    .style(design::ghost)
                                    .on_press(Message::PluginManager(Action::CancelRunningInstall)),
                            ]
                            .spacing(6)
                            .align_y(iced::Alignment::Center),
                        );
                    } else if self.install_confirm {
                        card = card.push(
                            iced::widget::column![
                                label(
                                    "Install this spec? The Host runs pnpm in the profile; \
                                     scripts run only with the profile's approval.",
                                    MUTED,
                                )
                                .size(10)
                                .width(Length::Fill),
                                iced::widget::row![
                                    button(label("Cancel", MUTED).size(11))
                                        .padding([3, 8])
                                        .style(design::ghost)
                                        .on_press(Message::PluginManager(Action::CancelInstall)),
                                    button(label("Install", design::ACCENT).size(11))
                                        .padding([3, 8])
                                        .style(design::ghost)
                                        .on_press(Message::PluginManager(Action::ConfirmInstall)),
                                ]
                                .spacing(6),
                            ]
                            .spacing(2),
                        );
                    } else {
                        card = card.push(
                            button(label("Install…", TEXT).size(11))
                                .padding([3, 8])
                                .style(design::ghost)
                                .on_press(Message::PluginManager(Action::AskInstall)),
                        );
                    }
                    body = body.push(
                        container(card)
                            .padding([8, 10])
                            .width(Length::Fill)
                            .style(|_| design::card(design::OVERLAY, design::RADIUS_SM)),
                    );
                }
                dsh_native_transport::plugin::PluginInspection::Refused {
                    problem,
                    reason,
                    registries,
                } => {
                    body = body.push(
                        label(
                            format!(
                                "Could not inspect the spec ({problem}): {}{}",
                                short(reason, 200),
                                if *registries > 1 {
                                    format!(" · {registries} registries asked")
                                } else {
                                    String::new()
                                }
                            ),
                            DANGER,
                        )
                        .size(12),
                    );
                }
            }
        } else if self.inspecting {
            body = body.push(label("Reading the spec…", MUTED).size(11));
        }
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
                    bundle.rows.len(),
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
                // The rows the bundle declares, each joined to its live entry so its own switch
                // can be offered (`BundleRowInfo`, `types.d.ts:35-42`).
                if !bundle.rows.is_empty() {
                    let mut list = column![].spacing(2);
                    for declared in bundle.rows.iter().take(6) {
                        let live = declared
                            .entry_id
                            .as_deref()
                            .and_then(|entry| self.rows.iter().find(|row| row.entry_id == entry));
                        let mut line = iced::widget::row![
                            label(
                                format!(
                                    "{} · {}",
                                    short(&declared.module_name, 28),
                                    short(&declared.row_id, 28)
                                ),
                                MUTED,
                            )
                            .size(10)
                            .width(Length::Fill),
                            label(
                                match live {
                                    Some(live) => format!(
                                        "{} · {:?}",
                                        if live.enabled { "enabled" } else { "disabled" },
                                        live.fiber_phase
                                    ),
                                    None => "no live entry".to_owned(),
                                },
                                MUTED,
                            )
                            .size(10),
                        ]
                        .spacing(6)
                        .align_y(iced::Alignment::Center);
                        if let Some(live) = live.filter(|live| {
                            live.patch_id.is_some() && live.read_only_reason.is_none()
                        }) {
                            let wanted = !live.enabled;
                            let entry = live.entry_id.clone();
                            let pending = self
                                .row_pending
                                .as_ref()
                                .is_some_and(|(pending, _)| pending == &entry);
                            let asking = self
                                .row_confirm
                                .as_ref()
                                .is_some_and(|(asking, _)| asking == &entry);
                            if pending {
                                line = line.push(label("Changing…", MUTED).size(10));
                            } else if asking {
                                line = line.push(
                                    iced::widget::row![
                                        label(
                                            format!(
                                                "{} for the next start?",
                                                if wanted { "Enable" } else { "Disable" }
                                            ),
                                            TEXT
                                        )
                                        .size(10),
                                        button(label("Cancel", MUTED).size(10))
                                            .padding([2, 6])
                                            .style(design::ghost)
                                            .on_press(Message::PluginManager(Action::CancelRow)),
                                        button(label("Confirm", design::ACCENT).size(10))
                                            .padding([2, 6])
                                            .style(design::ghost)
                                            .on_press(Message::PluginManager(Action::ConfirmRow)),
                                    ]
                                    .spacing(4)
                                    .align_y(iced::Alignment::Center),
                                );
                            } else {
                                line = line.push(
                                    button(
                                        label(if wanted { "Enable" } else { "Disable" }, TEXT)
                                            .size(10),
                                    )
                                    .padding([2, 6])
                                    .style(design::ghost)
                                    .on_press(
                                        Message::PluginManager(Action::AskRow {
                                            entry_id: entry,
                                            enabled: wanted,
                                        }),
                                    ),
                                );
                            }
                        }
                        list = list.push(line);
                    }
                    if bundle.rows.len() > 6 {
                        list = list.push(
                            label(format!("… {} more rows", bundle.rows.len() - 6), MUTED).size(10),
                        );
                    }
                    row = row.push(list);
                }
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

/// An opaque request id for one installation. The Host treats it as a key, not a UUID, and the app
/// spends no dependency on one; only the process, time and a local sequence are encoded.
fn next_install_request() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    format!(
        "native-{}-{nanos}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}
