#[cfg(any(target_os = "linux", target_os = "macos"))]
use super::setup::SetupProvider;
use super::*;
use draugr_time_daemon::preferences::{Preferences, RecordingSchedule};
use iced::widget::column;
use std::path::{Path, PathBuf};

pub const SECTIONS: [&str; 6] = [
    "General",
    "Tracking",
    "Privacy",
    "Data & import",
    "Status & permissions",
    "About",
];

#[derive(Clone, Debug)]
pub enum Edit {
    Next,
    Section(&'static str),
    AutostartAnswer(bool),
    RemoveApp(bool, String),
    #[cfg(target_os = "linux")]
    InstallBridge(setup::Bridge),
    #[cfg(target_os = "linux")]
    InstallDesktop,
    #[cfg(target_os = "macos")]
    Permissions,
    Excluded(String),
    Masked(String),
    Schedule(bool),
    Weekdays(bool),
    Start(String),
    End(String),
    Save,
    Delete,
    ConfirmDelete,
    CancelDelete,
    Deleted(Result<(), String>),
}
pub struct Ui {
    pub preferences: Preferences,
    pub step: usize,
    pub section: &'static str,
    excluded: String,
    masked: String,
    schedule: bool,
    weekdays: bool,
    start: String,
    end: String,
    message: String,
    deleting: Option<Range>,
    busy: bool,
}
impl Default for Ui {
    fn default() -> Self {
        Self {
            preferences: Preferences::default(),
            step: 0,
            section: "General",
            excluded: String::new(),
            masked: String::new(),
            schedule: false,
            weekdays: true,
            start: "09:00".into(),
            end: "19:00".into(),
            message: String::new(),
            deleting: None,
            busy: false,
        }
    }
}
fn path() -> Result<PathBuf, String> {
    draugr_time_daemon::paths::data_dir()
        .map(|p| p.join("settings.json"))
        .map_err(|e| e.to_string())
}
pub fn app_list(text: &str) -> Vec<String> {
    let mut apps = Vec::new();
    for app in text
        .split(['\n', ';'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if !apps
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(app))
        {
            apps.push(app.to_owned());
        }
    }
    apps
}
pub fn minute(value: &str) -> Result<u32, String> {
    let (hours, minutes) = value
        .split_once(':')
        .ok_or_else(|| tr("Use HH:MM times; the end must follow the start."))?;
    let hours = hours
        .parse::<u32>()
        .map_err(|_| tr("Use HH:MM times; the end must follow the start."))?;
    let minutes = minutes
        .parse::<u32>()
        .map_err(|_| tr("Use HH:MM times; the end must follow the start."))?;
    if hours > 24 || minutes > 59 || (hours == 24 && minutes != 0) {
        return Err(tr("Use HH:MM times; the end must follow the start."));
    }
    Ok(hours * 60 + minutes)
}
impl Ui {
    pub fn load() -> Self {
        if cfg!(test) {
            return Self::default();
        }
        match path().and_then(|path| Preferences::load(&path).map_err(|e| e.to_string())) {
            Ok(preferences) => {
                let mut ui = Self {
                    excluded: preferences.excluded_apps.join("; "),
                    masked: preferences.masked_apps.join("; "),
                    ..Self::default()
                };
                if let Some(schedule) = &preferences.schedule {
                    ui.schedule = true;
                    ui.weekdays = schedule.weekdays == vec![1, 2, 3, 4, 5];
                    ui.start = format!(
                        "{:02}:{:02}",
                        schedule.start_minute / 60,
                        schedule.start_minute % 60
                    );
                    ui.end = format!(
                        "{:02}:{:02}",
                        schedule.end_minute / 60,
                        schedule.end_minute % 60
                    );
                }
                ui.preferences = preferences;
                ui
            }
            Err(message) => Self {
                message,
                ..Self::default()
            },
        }
    }
    #[cfg(test)]
    pub fn edit_lists(&mut self, excluded: &str, masked: &str) {
        self.excluded = excluded.to_owned();
        self.masked = masked.to_owned();
    }
    /// Preferences as they would be saved from the Privacy page's fields.
    pub fn privacy_preferences(&self) -> Result<Preferences, String> {
        let mut preferences = self.preferences.clone();
        preferences.excluded_apps = app_list(&self.excluded);
        preferences.masked_apps = app_list(&self.masked);
        preferences.schedule = if self.schedule {
            let (start, end) = (minute(&self.start)?, minute(&self.end)?);
            if end <= start {
                return Err(tr("Use HH:MM times; the end must follow the start."));
            }
            Some(RecordingSchedule {
                weekdays: if self.weekdays {
                    vec![1, 2, 3, 4, 5]
                } else {
                    vec![1, 2, 3, 4, 5, 6, 7]
                },
                start_minute: start,
                end_minute: end,
            })
        } else {
            None
        };
        Ok(preferences)
    }
    pub fn answer_autostart(&mut self, path: &Path) -> Result<(), String> {
        let mut preferences = self.preferences.clone();
        preferences.autostart_prompt_pending = false;
        preferences.save(path).map_err(|e| e.to_string())?;
        self.preferences = preferences;
        Ok(())
    }
    pub fn advance(&mut self, path: &Path) -> Result<(), String> {
        if self.step == 0 {
            self.step = 1;
        } else {
            let mut preferences = self.preferences.clone();
            preferences.onboarded = true;
            preferences.autostart_prompt_pending = true;
            preferences.save(path).map_err(|e| e.to_string())?;
            self.preferences = preferences;
        }
        Ok(())
    }
}
impl State {
    pub(super) fn edit_settings(&mut self, edit: Edit) -> Task<Message> {
        let ui = &mut self.settings_ui;
        match edit {
            Edit::Section(section) if SECTIONS.contains(&section) => ui.section = section,
            Edit::RemoveApp(masked, app) => {
                let list = if masked { &mut ui.masked } else { &mut ui.excluded };
                *list = app_list(list).into_iter().filter(|value| !value.eq_ignore_ascii_case(&app)).collect::<Vec<_>>().join("; ");
            }
            Edit::AutostartAnswer(enabled) => {
                if enabled {
                    match startup::SystemStartup::new().and_then(|mut provider| startup::set_enabled(&mut provider, true)) {
                        Ok(value) => { self.autostart_enabled = value; self.autostart_error.clear(); }
                        Err(error) => { self.autostart_error = error; return Task::none(); }
                    }
                }
                match path().and_then(|path| ui.answer_autostart(&path)) {
                    Ok(()) => {
                        self.autostart_error.clear();
                        if let Some(runtime) = &self.runtime {
                            runtime.shared.preferences.lock().unwrap().autostart_prompt_pending = false;
                        }
                    }
                    Err(error) => self.autostart_error = error,
                }
                if !ui.preferences.autostart_prompt_pending { self.animate(motion::Key::Dialog, 1.0, 0.0); }
            }
            #[cfg(target_os="linux")]
            Edit::InstallBridge(bridge) => ui.message=setup::SystemSetup.install_bridge(bridge).map(|()|tr("Bridge installed. GNOME: log out and back in, then enable Draugr Time Watcher in Extensions. KDE: enable Draugr Time Watcher in System Settings → Window Management → KWin Scripts.")).unwrap_or_else(|e|e),
            #[cfg(target_os="linux")]
            Edit::InstallDesktop => ui.message=setup::SystemSetup.install_desktop().map(|()|tr("Application menu entry installed.")).unwrap_or_else(|e|e),
            #[cfg(target_os="macos")]
            Edit::Permissions => { if let Err(error)=setup::SystemSetup.open_permissions(){ui.message=error;} }
            Edit::Next => match path().and_then(|path|ui.advance(&path)) {
                Ok(()) => { if ui.preferences.onboarded { self.start(); if self.autostart_question() { self.animate(motion::Key::Dialog, 0.0, 1.0); } } }
                Err(error) => ui.message=error,
            },
            Edit::Excluded(value) => ui.excluded=value,
            Edit::Masked(value) => ui.masked=value,
            Edit::Schedule(value) => ui.schedule=value,
            Edit::Weekdays(value) => ui.weekdays=value,
            Edit::Start(value) => ui.start=value,
            Edit::End(value) => ui.end=value,
            Edit::Save => {
                match ui.privacy_preferences().and_then(|preferences| path().and_then(|path|preferences.save(&path).map_err(|e|e.to_string())).map(|()|preferences)) {
                    Ok(preferences) => {
                        ui.preferences=preferences.clone();
                        if let Some(runtime)=&self.runtime {
                            *runtime.shared.preferences.lock().unwrap()=preferences;
                            runtime.shared.control.lock().unwrap().status.current_window=None;
                        }
                        ui.message=tr("Settings saved. Exclusions and masking apply to new activity.");
                    }
                    Err(error) => ui.message=error,
                }
            }
            Edit::Delete if !ui.busy => ui.deleting=Some(self.selection.unwrap_or(self.day)),
            Edit::CancelDelete => ui.deleting=None,
            Edit::ConfirmDelete if !ui.busy => {
                let (Some(range),Some(runtime))=(ui.deleting,&self.runtime) else {ui.message=tr("Start recording before managing stored data.");return Task::none();};
                ui.deleting=None;ui.busy=true;
                let shared=runtime.shared.clone();
                return Task::perform(async move {
                    tokio::task::spawn_blocking(move || {
                        let mut control=shared.control.lock().unwrap();
                        shared.storage.lock().unwrap().delete_history(range.start,range.end).map_err(|e|e.to_string())?;
                        control.pause_generation=control.pause_generation.wrapping_add(1);
                        Ok(())
                    }).await.map_err(|e|e.to_string()).and_then(|r|r)
                },|result|Message::Settings(Edit::Deleted(result)));
            }
            Edit::Deleted(result) => {
                ui.busy=false;
                match result {
                    Ok(()) => {
                        ui.message=tr("History deleted.");self.tag_undo.clear();self.rule_editor=rules::Editor::default();self.editor=None;self.away_range=None;self.release_day();self.rebuild();
                    }
                    Err(error) => ui.message=error,
                }
            }
            _ => {}
        }
        Task::none()
    }
    pub(super) fn setup_controls(&self) -> Element<'_, Message> {
        let body=column![text(tr("macOS: allow Accessibility access. GNOME/KDE Wayland: install and enable the bridge. X11, Sway and Hyprland need no bridge."))].spacing(10);
        #[cfg(target_os = "linux")]
        let body = body
            .push(
                button(text(tr("Install GNOME bridge")))
                    .on_press(Message::Settings(Edit::InstallBridge(setup::Bridge::Gnome))),
            )
            .push(
                button(text(tr("Install KDE bridge")))
                    .on_press(Message::Settings(Edit::InstallBridge(setup::Bridge::Kde))),
            )
            .push(
                button(text(tr("Add to application menu")))
                    .on_press(Message::Settings(Edit::InstallDesktop)),
            );
        #[cfg(target_os = "macos")]
        let body = body.push(
            button(text(tr("Open Accessibility settings")))
                .on_press(Message::Settings(Edit::Permissions)),
        );
        body.into()
    }
    pub(super) fn onboarding(&self) -> Element<'_, Message> {
        let ui = &self.settings_ui;
        let mut body=column![text(tr("Welcome to Draugr Time")).size(30),text(tr("Applications, window titles and active/away time are stored only on this computer. Nothing leaves the machine; no telemetry.")),text(draugr_time_daemon::paths::data_dir().map(|p|p.display().to_string()).unwrap_or_else(|e|e.to_string()))].spacing(18).max_width(700);
        if ui.step == 1 {
            body = body.push(self.setup_controls());
        }
        body = body.push(text(&ui.message)).push(
            button(text(tr(if ui.step == 0 {
                "Continue"
            } else {
                "Start recording"
            })))
            .on_press(Message::Settings(Edit::Next)),
        );
        container(scrollable(body))
            .padding(36)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|theme: &Theme| container::Style {
                background: Some(theme.palette().background.into()),
                ..Default::default()
            })
            .into()
    }
}

impl State {
    pub(super) fn settings_navigation(&self, narrow: bool) -> Element<'_, Message> {
        let mut items = Vec::new();
        for section in SECTIONS {
            let active = section == self.settings_ui.section;
            items.push(
                button(text(tr(section)).size(design::BODY))
                    .width(if narrow { Length::Shrink } else { Length::Fill })
                    .height(Length::Shrink)
                    .style(design::segment(active))
                    .on_press(Message::Settings(Edit::Section(section)))
                    .into(),
            );
        }
        let items: Element<'_, Message> = if narrow {
            row(items).spacing(design::CONTROL_GAP).wrap().into()
        } else {
            column(items).spacing(design::CONTROL_GAP).into()
        };
        column![
            text(tr("Settings")).size(design::HEADING),
            items,
            text(tr("Activity stays on this computer.")).size(design::LABEL)
        ]
        .spacing(design::GAP)
        .into()
    }

    pub(super) fn autostart_question(&self) -> bool {
        self.settings_ui.preferences.onboarded
            && self.settings_ui.preferences.autostart_prompt_pending
            && (!self.autostart_enabled || !self.autostart_error.is_empty())
            && !self.fixture
    }

    pub(super) fn autostart_dialog(&self) -> Element<'_, Message> {
        let p = design::Palette::new(self.theme.palette().background.r < 0.5);
        container(column![
            image(image::Handle::from_bytes(crate::branding::logo(self.theme.palette().background.r < 0.5))).width(design::TARGET).height(design::TARGET),
            text(tr(if self.runtime.is_some() { "Recording already started" } else { "Not recording" })).size(design::LABEL).color(p.accent_text),
            text(tr("Start Draugr Time with your computer?")).size(design::HEADING),
            text(tr("Starts activity tracking in the tray. Turn this off here to remove it from system startup.")).size(design::BODY),
            text(localization::error(if self.autostart_error.is_empty() { &self.error } else { &self.autostart_error })).size(design::BODY).color(p.danger_text),
            row![
                button(text(tr("No, thanks"))).on_press(Message::Settings(Edit::AutostartAnswer(false))),
                button(text(tr("Yes, start with system"))).style(design::primary).on_press(Message::Settings(Edit::AutostartAnswer(true))),
            ].spacing(design::CONTROL_GAP).wrap(),
            text(tr("Asked only once")).size(design::LABEL).color(p.text3),
        ].spacing(design::GAP).align_x(Alignment::Center))
        .padding(design::PADDING)
        .max_width(design::SETTINGS_DIALOG_WIDTH)
        .style(panel)
        .into()
    }

    pub(super) fn settings(&self, narrow: bool) -> Element<'_, Message> {
        let ui = &self.settings_ui;
        let p = design::Palette::new(self.theme.palette().background.r < 0.5);
        let setting = |label: &'static str,
                       explanation: &'static str,
                       control: Element<'static, Message>|
         -> Element<'static, Message> {
            let labels = column![
                text(tr(label)).size(design::BODY),
                text(tr(explanation)).size(design::META).color(p.text2)
            ]
            .spacing(design::SMALL_GAP);
            let body: Element<'_, Message> = if narrow {
                column![labels, control].spacing(design::CONTROL_GAP).into()
            } else {
                row![container(labels).width(Length::Fill), control]
                    .spacing(design::GAP)
                    .align_y(Alignment::Center)
                    .into()
            };
            container(body)
                .padding(design::RAIL_PADDING)
                .width(Length::Fill)
                .into()
        };
        let mut body = column![text(tr(ui.section)).size(design::HEADING)].spacing(design::GAP);
        match ui.section {
            "General" => {
                body = body.push(setting(
                    "Theme",
                    "System default",
                    button(text(tr(self.theme_preference.label())))
                        .on_press(Message::Theme)
                        .into(),
                ));
                let mut languages =
                    row![button(text(tr("System default"))).on_press(Message::Language(None))]
                        .spacing(design::CONTROL_GAP);
                for language in localization::Language::ALL {
                    languages = languages.push(
                        button(text(language.name())).on_press(Message::Language(Some(language))),
                    );
                }
                body = body
                    .push(
                        text(localized(
                            "Language: {language}",
                            &[("language", localization::current().name().into())],
                        ))
                        .size(design::BODY),
                    )
                    .push(languages.wrap());
            }
            "Tracking" => {
                body = body.push(setting("Start Draugr Time when I log in", "Starts activity tracking in the tray. Turn this off here to remove it from system startup.", management::switch(self.autostart_enabled, Message::AutoStart(!self.autostart_enabled), p)));
                body = body
                    .push(text(localization::error(&self.autostart_error)).color(p.danger_text));
                body = body
                    .push(text(tr("While you were away")).size(design::BODY))
                    .push(
                        text(localized(
                            "Prompt threshold: {duration}",
                            &[(
                                "duration",
                                if self.away_prompt.threshold > 86_400_000 {
                                    tr("disabled")
                                } else {
                                    model::duration(self.away_prompt.threshold)
                                },
                            )],
                        ))
                        .size(design::META),
                    );
                let mut thresholds = row![].spacing(design::CONTROL_GAP);
                for minutes in [15, 30, 60] {
                    thresholds = thresholds.push(
                        button(text(model::duration(minutes * 60_000)))
                            .on_press(Message::Edit(tagging::Edit::Threshold(minutes))),
                    );
                }
                body =
                    body.push(
                        thresholds
                            .push(button(text(tr("Off"))).on_press(Message::Edit(
                                tagging::Edit::Threshold(i64::MAX / 60_000),
                            )))
                            .wrap(),
                    );
                let mut pauses = row![].spacing(design::CONTROL_GAP);
                for minutes in [5, 10, 15, 30, 60] {
                    pauses = pauses.push(
                        button(text(model::duration(minutes * 60_000))).on_press_maybe(
                            self.runtime
                                .as_ref()
                                .map(|_| Message::Pause(Some(minutes * 60_000))),
                        ),
                    );
                }
                body = body.push(text(tr("Don't track for")).size(design::BODY)).push(pauses.wrap())
                    .push(setting("Recording schedule (local time)", "Outside the schedule, recording pauses. Disable the schedule to resume outside its hours.", management::switch(ui.schedule, Message::Settings(Edit::Schedule(!ui.schedule)), p)));
                if ui.schedule {
                    body = body
                        .push(setting(
                            "Weekdays only (otherwise every day)",
                            "Recording schedule (local time)",
                            management::switch(
                                ui.weekdays,
                                Message::Settings(Edit::Weekdays(!ui.weekdays)),
                                p,
                            ),
                        ))
                        .push(
                            row![
                                text_input("09:00", &ui.start)
                                    .on_input(|s| Message::Settings(Edit::Start(s))),
                                text("–"),
                                text_input("19:00", &ui.end)
                                    .on_input(|s| Message::Settings(Edit::End(s)))
                            ]
                            .spacing(design::CONTROL_GAP),
                        );
                }
                body = body
                    .push(
                        button(text(tr("Save")))
                            .style(design::primary)
                            .on_press(Message::Settings(Edit::Save)),
                    )
                    .push(text(&ui.message).size(design::META));
            }
            "Privacy" => {
                for (masked, label, value, explanation) in [
                    (false, "Excluded applications", &ui.excluded, "Excluded application IDs or executable paths, separated by semicolons. Excluded applications and titles are never recorded; active/away totals remain."),
                    (true, "Masked applications", &ui.masked, "Masked application IDs or executable paths, separated by semicolons. Their time is recorded, but window titles are stored as [masked]. Exclusion wins if an app is in both lists."),
                ] {
                    body = body.push(text(tr(label)).size(design::BODY)).push(text(tr(explanation)).size(design::META).color(p.text2));
                    let mut chips = row![].spacing(design::CONTROL_GAP);
                    for app in app_list(value) {
                        chips = chips.push(button(text(format!("{app} ×")).size(design::META)).height(design::DENSE_TARGET).style(design::chip).on_press(Message::Settings(Edit::RemoveApp(masked, app))));
                    }
                    body = body.push(chips.wrap()).push(text_input(&tr(label), value).on_input(move |value| Message::Settings(if masked { Edit::Masked(value) } else { Edit::Excluded(value) })));
                }
                body = body.push(button(text(tr("Save privacy settings"))).style(design::primary).on_press(Message::Settings(Edit::Save)))
                    .push(text(tr("Delete the Day selection, or the displayed day if nothing is selected. History outside the range is preserved.")).size(design::META))
                    .push(button(text(tr("Delete selected history…"))).style(design::destructive).on_press_maybe((!ui.busy).then_some(Message::Settings(Edit::Delete))));
                if let Some(range) = ui.deleting {
                    let label = |ts| {
                        chrono::DateTime::from_timestamp_millis(ts)
                            .map(|date| {
                                date.with_timezone(&Local)
                                    .format("%Y-%m-%d %H:%M:%S %:z")
                                    .to_string()
                            })
                            .unwrap_or_default()
                    };
                    body = body
                        .push(text(format!(
                            "{} {} – {}",
                            tr("Permanently delete history:"),
                            label(range.start),
                            label(range.end)
                        )))
                        .push(
                            row![
                                button(text(tr("Confirm permanent deletion")))
                                    .style(design::destructive)
                                    .on_press(Message::Settings(Edit::ConfirmDelete)),
                                button(text(tr("Cancel")))
                                    .on_press(Message::Settings(Edit::CancelDelete))
                            ]
                            .spacing(design::CONTROL_GAP)
                            .wrap(),
                        );
                }
                body = body.push(text(&ui.message).size(design::META));
            }
            "Data & import" => {
                body = body.push(text(tr("Data folder")).size(design::BODY)).push(text(draugr_time_daemon::paths::data_dir().map(|p| p.display().to_string()).unwrap_or_else(|e| e.to_string())).size(design::META))
                    .push(button(text(tr("Choose folder"))).on_press(Message::Folder))
                    .push(text(self.pending_dir.as_ref().map(|path| localized("Next launch: {path}. Existing history stays in the old folder.", &[("path", path.clone())])).unwrap_or_else(|| tr("The folder change takes effect after restarting Draugr Time."))).size(design::META))
                    .push(button(text(tr("Import ActivityWatch / Draugr JSON"))).on_press(Message::Report(reports::Edit::Import)))
                    .push(text(tr("Import adds history atomically; overlapping event buckets are rejected. JSON export is available in Reports.")).size(design::META))
                    .push(button(text(tr("Open full report →"))).on_press(Message::Screen("Reports")));
            }
            "Status & permissions" => {
                if self.runtime.is_some() {
                    let computer = &self.status.computer;
                    body = body.push(
                        text(localized(
                            "Computer: {machine} · Activity: {activity} · Recording: {recording}",
                            &[
                                ("machine", localization::state(&computer.machine)),
                                ("activity", localization::state(&computer.activity)),
                                ("recording", localization::state(&computer.recording)),
                            ],
                        ))
                        .size(design::META),
                    );
                    if let Some(reason) = &computer.reason {
                        body = body.push(text(localization::error(reason)).size(design::META));
                    }
                }
                if self.runtime.is_none() {
                    body = body.push(text(tr(
                        "Tracking is stopped. Press Start recording to collect activity.",
                    )));
                }
                for backend in &self.status.backends {
                    body = body.push(
                        text(format!(
                            "● {} · {}",
                            tr(&backend.name),
                            localization::state(&backend.state)
                        ))
                        .size(design::BODY),
                    );
                    if let Some(error) = &backend.error {
                        body = body.push(
                            text(localization::error(error))
                                .size(design::META)
                                .color(p.danger_text),
                        );
                    }
                }
                body = body
                    .push(
                        text(localization::error(&self.status.computer_capabilities))
                            .size(design::META),
                    )
                    .push(self.setup_controls())
                    .push(text(&ui.message).size(design::META));
            }
            "About" => {
                body = body
                    .push(
                        image(image::Handle::from_bytes(crate::branding::logo(
                            self.theme.palette().background.r < 0.5,
                        )))
                        .width(design::LOGO)
                        .height(design::LOGO),
                    )
                    .push(text(tr("Draugr Time")).size(design::HEADING))
                    .push(
                        text(concat!(
                            "Version ",
                            env!("CARGO_PKG_VERSION"),
                            " · GPL-3.0-only"
                        ))
                        .size(design::META),
                    )
                    .push(
                        button(text("github.com/hlophlopgaming/DraugrTime")).on_press(
                            Message::OpenUrl("https://github.com/hlophlopgaming/DraugrTime"),
                        ),
                    )
                    .push(text(tr("Created by")).size(design::BODY))
                    .push(
                        button(text("hlophlopgaming"))
                            .on_press(Message::OpenUrl("https://github.com/hlophlopgaming")),
                    );
            }
            _ => {}
        }
        scrollable(
            container(body)
                .padding(design::CARD_PADDING)
                .width(Length::Fill)
                .style(panel),
        )
        .style(design::scrollbars)
        .into()
    }
}
