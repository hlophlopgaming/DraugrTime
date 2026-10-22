#[cfg(any(target_os = "linux", target_os = "macos"))]
use super::setup::SetupProvider;
use super::*;
use draugr_time_daemon::preferences::{Preferences, RecordingSchedule};
use iced::widget::column;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub enum Edit {
    Next,
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
    pub fn advance(&mut self, path: &Path) -> Result<(), String> {
        if self.step == 0 {
            self.step = 1;
        } else {
            let mut preferences = self.preferences.clone();
            preferences.onboarded = true;
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
            #[cfg(target_os="linux")]
            Edit::InstallBridge(bridge) => ui.message=setup::SystemSetup.install_bridge(bridge).map(|()|tr("Bridge installed. GNOME: log out and back in, then enable Draugr Time Watcher in Extensions. KDE: enable Draugr Time Watcher in System Settings → Window Management → KWin Scripts.")).unwrap_or_else(|e|e),
            #[cfg(target_os="linux")]
            Edit::InstallDesktop => ui.message=setup::SystemSetup.install_desktop().map(|()|tr("Application menu entry installed.")).unwrap_or_else(|e|e),
            #[cfg(target_os="macos")]
            Edit::Permissions => { if let Err(error)=setup::SystemSetup.open_permissions(){ui.message=error;} }
            Edit::Next => match path().and_then(|path|ui.advance(&path)) {
                Ok(()) => { if ui.preferences.onboarded { self.start(); } }
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
    pub(super) fn privacy_settings(&self) -> Element<'_, Message> {
        let ui = &self.settings_ui;
        let mut body=column![text(tr("Privacy")).size(22),text(tr("Excluded application IDs or executable paths, separated by semicolons. Excluded applications and titles are never recorded; active/away totals remain.")),text_input(&tr("Excluded applications"),&ui.excluded).on_input(|s|Message::Settings(Edit::Excluded(s))),text(tr("Masked application IDs or executable paths, separated by semicolons. Their time is recorded, but window titles are stored as [masked]. Exclusion wins if an app is in both lists.")),text_input(&tr("Masked applications"),&ui.masked).on_input(|s|Message::Settings(Edit::Masked(s))),checkbox(ui.schedule).label(tr("Recording schedule (local time)")).on_toggle(|v|Message::Settings(Edit::Schedule(v))),checkbox(ui.weekdays).label(tr("Weekdays only (otherwise every day)")).on_toggle(|v|Message::Settings(Edit::Weekdays(v))),row![text_input("09:00",&ui.start).on_input(|s|Message::Settings(Edit::Start(s))),text("–"),text_input("19:00",&ui.end).on_input(|s|Message::Settings(Edit::End(s)))].spacing(10),text(tr("Outside the schedule, recording pauses. Disable the schedule to resume outside its hours.")),button(text(tr("Save privacy settings"))).on_press(Message::Settings(Edit::Save)),text(tr("Delete the Day selection, or the displayed day if nothing is selected. History outside the range is preserved.")),button(text(tr("Delete selected history…"))).on_press_maybe((!ui.busy).then_some(Message::Settings(Edit::Delete)))].spacing(10);
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
                            .on_press(Message::Settings(Edit::ConfirmDelete)),
                        button(text(tr("Cancel"))).on_press(Message::Settings(Edit::CancelDelete))
                    ]
                    .spacing(10),
                );
        }
        body.push(self.setup_controls())
            .push(text(&ui.message))
            .into()
    }
}
