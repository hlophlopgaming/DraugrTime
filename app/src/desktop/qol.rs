//! Small desktop conveniences and their local, history-independent preferences.
use super::*;
use iced::widget::column;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub enum Edit {
    OpenDate,
    Date(String),
    GoDate,
    CancelDate,
    Help,
    ClearSearch,
    OpenFolder,
    FolderOpened(Result<(), String>),
    CopyFolder,
    SaveDraft,
    DiscardDraft,
    Stay,
    Resized(window::Id, iced::Size),
    FitWindow(window::Id, Option<iced::Size>),
}

#[derive(Clone, Debug)]
pub(super) enum Pending {
    Tag(management::Edit),
    Rule(rules::Edit),
    Suggestion(String),
}

#[derive(Clone, Debug)]
pub(super) struct Preferences {
    pub theme: appearance::Preference,
    pub size: Option<iced::Size>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: appearance::Preference::System,
            size: None,
        }
    }
}

impl Preferences {
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default())
            }
            Err(error) => return Err(error.to_string()),
        };
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        let theme = match value["theme"].as_str() {
            Some("Light") => appearance::Preference::Light,
            Some("Dark") => appearance::Preference::Dark,
            _ => appearance::Preference::System,
        };
        let size = value["width"]
            .as_f64()
            .zip(value["height"].as_f64())
            .map(|(w, h)| iced::Size::new(w as f32, h as f32))
            .filter(|s| valid_size(*s));
        Ok(Self { theme, size })
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let parent = path.parent().ok_or("Invalid preferences path")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let size = self.size.filter(|s| valid_size(*s));
        let bytes = serde_json::to_vec_pretty(&serde_json::json!({
            "theme": self.theme.label(),
            "width": size.map(|s| s.width),
            "height": size.map(|s| s.height),
        }))
        .map_err(|e| e.to_string())?;
        use std::io::Write;
        let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        file.as_file().sync_all().map_err(|e| e.to_string())?;
        file.persist(path).map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[derive(Default)]
pub(super) struct Ui {
    pub preferences: Preferences,
    pub date: Option<String>,
    pub date_error: String,
    pub pending: Option<Pending>,
    pub notice: String,
}

pub(super) fn path() -> Result<PathBuf, String> {
    draugr_time_daemon::paths::default_dir()
        .map(|p| p.join("ui.json"))
        .map_err(|e| e.to_string())
}

fn valid_size(size: iced::Size) -> bool {
    size.width.is_finite() && size.height.is_finite() && size.width > 0.0 && size.height > 0.0
}

pub(super) fn fit_size(size: iced::Size, monitor: iced::Size) -> iced::Size {
    let available = iced::Size::new(
        (monitor.width - design::PADDING as f32 * 2.0).max(1.0),
        (monitor.height - design::PADDING as f32 * 2.0).max(1.0),
    );
    iced::Size::new(
        size.width.clamp(
            design::WINDOW_MIN.width.min(available.width),
            available.width,
        ),
        size.height.clamp(
            design::WINDOW_MIN.height.min(available.height),
            available.height,
        ),
    )
}

pub(super) fn search_target(screen: &str) -> Option<&'static str> {
    match screen {
        "Day" => Some("day-filter"),
        "Tags" => Some("manager-tag-search"),
        "Rules" => Some("rule-pattern"),
        "Overview" | "Reports" => Some("activity-search"),
        _ => None,
    }
}

/// Arguments are kept separate so spaces and non-ASCII paths are not shell code.
pub(super) fn folder_command(path: &Path) -> (&'static str, Vec<std::ffi::OsString>) {
    #[cfg(target_os = "windows")]
    let program = "explorer.exe";
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let program = "xdg-open";
    (program, vec![path.as_os_str().to_owned()])
}

impl State {
    pub(super) fn persist_ui(&mut self) {
        if cfg!(test) || self.fixture || self.smoke || self.runtime_test || self.benchmark {
            return;
        }
        self.qol.preferences.theme = self.theme_preference;
        if let Err(error) = path().and_then(|path| self.qol.preferences.save(&path)) {
            self.error = localized(
                "Could not save interface preferences: {error}",
                &[("error", error)],
            );
        }
    }

    pub(super) fn focus_search(&mut self) -> Task<Message> {
        let Some(id) = search_target(self.screen) else {
            return Task::none();
        };
        if self.screen == "Rules" && self.rule_editor.field.is_empty() {
            return Task::none();
        }
        self.search_open = matches!(self.screen, "Overview" | "Reports");
        widget::operation::focus(id)
    }

    pub(super) fn clear_search(&mut self) -> bool {
        match self.screen {
            "Day" if !self.summary_search.is_empty() => {
                self.summary_search.clear();
                self.summary();
            }
            "Tags" if !self.management.search.is_empty() => self.management.search.clear(),
            "Overview" | "Reports" if self.search_open => {
                self.search.clear();
                self.search_open = false;
                self.rebuild();
            }
            _ => return false,
        }
        true
    }

    pub(super) fn guard_rule(&mut self, edit: &rules::Edit) -> bool {
        if matches!(edit, rules::Edit::New | rules::Edit::Select(_)) && self.rule_editor.modified {
            self.qol.pending = Some(Pending::Rule(edit.clone()));
            true
        } else {
            false
        }
    }

    pub(super) fn guard_tag(&mut self, edit: &management::Edit) -> bool {
        if matches!(
            edit,
            management::Edit::NewTag | management::Edit::SelectTag(_)
        ) && self.management.tag.as_ref().is_some_and(|t| t.dirty)
        {
            self.qol.pending = Some(Pending::Tag(edit.clone()));
            true
        } else if let management::Edit::Suggestion(app) = edit {
            if self.rule_editor.modified {
                self.qol.pending = Some(Pending::Suggestion(app.clone()));
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    pub(super) fn edit_qol(&mut self, edit: Edit) -> Task<Message> {
        match edit {
            Edit::OpenDate => {
                self.qol.date = Some(self.date.to_string());
                self.qol.date_error.clear();
                return widget::operation::focus("jump-date");
            }
            Edit::Date(value) => {
                self.qol.date = Some(value);
                self.qol.date_error.clear();
            }
            Edit::CancelDate => {
                self.qol.date = None;
                self.qol.date_error.clear();
            }
            Edit::GoDate => {
                let value = self.qol.date.as_deref().unwrap_or("").trim();
                match NaiveDate::parse_from_str(value, "%Y-%m-%d")
                    .ok()
                    .filter(|date| {
                        date.to_string() == value
                            && (1970..=9999).contains(&chrono::Datelike::year(date))
                            && model::day_bounds(*date).start >= 0
                    }) {
                    Some(date) => {
                        self.qol.date = None;
                        self.qol.date_error.clear();
                        self.change_date(date);
                        if self.screen == "Week" {
                            return self.edit_report(reports::Edit::Refresh);
                        }
                    }
                    None => {
                        self.qol.date_error = tr("Enter a valid date as YYYY-MM-DD (1970–9999).")
                    }
                }
            }
            Edit::Help => self.shortcuts_open = true,
            Edit::ClearSearch => {
                self.clear_search();
            }
            Edit::CopyFolder => match draugr_time_daemon::paths::data_dir() {
                Ok(path) => {
                    self.qol.notice = tr("Path copied");
                    return iced::clipboard::write(path.display().to_string());
                }
                Err(error) => self.error = error.to_string(),
            },
            Edit::OpenFolder => {
                return Task::perform(
                    async {
                        tokio::task::spawn_blocking(|| {
                            let path =
                                draugr_time_daemon::paths::data_dir().map_err(|e| e.to_string())?;
                            let (command, args) = folder_command(&path);
                            let status = std::process::Command::new(command)
                                .args(args)
                                .status()
                                .map_err(|e| e.to_string())?;
                            if status.success() {
                                Ok(())
                            } else {
                                Err(status.to_string())
                            }
                        })
                        .await
                        .map_err(|e| e.to_string())
                        .and_then(|result| result)
                    },
                    |result| Message::Qol(Edit::FolderOpened(result)),
                );
            }
            Edit::FolderOpened(result) => {
                if let Err(error) = result {
                    self.error = localized(
                        "Could not open the data folder: {error}",
                        &[("error", error)],
                    );
                }
            }
            Edit::Stay => self.qol.pending = None,
            Edit::SaveDraft | Edit::DiscardDraft => {
                let Some(pending) = self.qol.pending.clone() else {
                    return Task::none();
                };
                let mut tasks = Vec::new();
                if matches!(pending, Pending::Tag(_)) {
                    if matches!(edit, Edit::SaveDraft) {
                        tasks.push(self.edit_management(management::Edit::SaveTag));
                        if self.management.tag.as_ref().is_none_or(|t| t.dirty) {
                            return Task::batch(tasks);
                        }
                    } else if let Some(tag) = &mut self.management.tag {
                        tag.dirty = false;
                    }
                } else if matches!(edit, Edit::SaveDraft) {
                    self.edit_rule(rules::Edit::Save);
                    if self.rule_editor.modified {
                        return Task::none();
                    }
                    if self.rule_editor.confirm {
                        self.qol.pending = None;
                        return Task::none();
                    }
                } else {
                    self.rule_editor.modified = false;
                }
                self.qol.pending = None;
                match pending {
                    Pending::Tag(edit) => tasks.push(self.edit_management(edit)),
                    Pending::Rule(edit) => self.edit_rule(edit),
                    Pending::Suggestion(app) => {
                        tasks.push(self.edit_management(management::Edit::Suggestion(app)))
                    }
                }
                return Task::batch(tasks);
            }
            Edit::Resized(id, size) => {
                if self.window == Some(id) && valid_size(size) {
                    self.qol.preferences.size = Some(size);
                }
            }
            Edit::FitWindow(id, Some(monitor))
                if self.window == Some(id)
                    && valid_size(monitor)
                    && !self.smoke
                    && !self.runtime_test
                    && !self.benchmark =>
            {
                let size = fit_size(
                    self.qol.preferences.size.unwrap_or(design::WINDOW_DEFAULT),
                    monitor,
                );
                self.qol.preferences.size = Some(size);
                return Task::batch([
                    window::set_min_size(
                        id,
                        Some(iced::Size::new(
                            design::WINDOW_MIN.width.min(size.width),
                            design::WINDOW_MIN.height.min(size.height),
                        )),
                    ),
                    window::resize(id, size),
                ]);
            }
            Edit::FitWindow(_, _) => {}
        }
        Task::none()
    }

    pub(super) fn qol_dialog(&self) -> Option<Element<'_, Message>> {
        let p = design::Palette::new(self.theme.palette().background.r < 0.5);
        let content: Element<'_, Message> = if let Some(date) = &self.qol.date {
            column![
                text(tr("Go to date")).size(design::HEADING),
                text_input("YYYY-MM-DD", date)
                    .id("jump-date")
                    .on_input(|s| Message::Qol(Edit::Date(s)))
                    .on_submit(Message::Qol(Edit::GoDate)),
                text(&self.qol.date_error)
                    .size(design::META)
                    .color(p.danger_text),
                row![
                    button(text(tr("Cancel"))).on_press(Message::Qol(Edit::CancelDate)),
                    button(text(tr("Go to date")))
                        .style(design::primary)
                        .on_press(Message::Qol(Edit::GoDate))
                ]
                .spacing(design::CONTROL_GAP)
                .wrap(),
            ]
            .spacing(design::GAP)
            .into()
        } else if let Some(pending) = &self.qol.pending {
            let error = if matches!(pending, Pending::Tag(_)) {
                &self.management.error
            } else {
                &self.rule_editor.error
            };
            column![
                text(tr("Unsaved changes")).size(design::HEADING),
                text(tr("Save your changes before continuing?")).size(design::BODY),
                text(localization::error(error))
                    .size(design::META)
                    .color(p.danger_text),
                row![
                    button(text(tr("Stay here"))).on_press(Message::Qol(Edit::Stay)),
                    button(text(tr("Discard changes")))
                        .style(design::destructive)
                        .on_press(Message::Qol(Edit::DiscardDraft)),
                    button(text(tr("Save and continue")))
                        .style(design::primary)
                        .on_press(Message::Qol(Edit::SaveDraft)),
                ]
                .spacing(design::CONTROL_GAP)
                .wrap(),
            ]
            .spacing(design::GAP)
            .into()
        } else {
            return None;
        };
        Some(
            container(content)
                .padding(design::PADDING)
                .max_width(design::SETTINGS_DIALOG_WIDTH)
                .style(panel)
                .into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferences_round_trip_and_small_monitor_bounds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ui.json");
        assert_eq!(
            Preferences::load(&path).unwrap().theme,
            appearance::Preference::System
        );
        Preferences {
            theme: appearance::Preference::Dark,
            size: Some(iced::Size::new(1200.0, 780.0)),
        }
        .save(&path)
        .unwrap();
        let restored = Preferences::load(&path).unwrap();
        assert_eq!(restored.theme, appearance::Preference::Dark);
        assert_eq!(restored.size, Some(iced::Size::new(1200.0, 780.0)));
        let fitted = fit_size(restored.size.unwrap(), iced::Size::new(800.0, 600.0));
        assert!(fitted.width <= 800.0 && fitted.height <= 600.0);
        std::fs::write(&path, "{\"theme\":\"unknown\",\"width\":-1,\"height\":20}").unwrap();
        assert!(Preferences::load(&path).unwrap().size.is_none());
        std::fs::write(&path, "broken").unwrap();
        assert!(Preferences::load(&path).is_err());
    }
    #[test]
    fn search_uses_real_fields_and_folder_paths_are_single_arguments() {
        assert_eq!(search_target("Rules"), Some("rule-pattern"));
        assert_eq!(search_target("Week"), None);
        assert_eq!(search_target("Settings"), None);
        let path = Path::new("/home/example/Папка с пробелами & test");
        let (_, args) = folder_command(path);
        assert_eq!(args, [path.as_os_str()]);
    }
}
