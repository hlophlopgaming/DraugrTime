//! Settings → Data & import → Backups: back up now, automatic backups and
//! restoring a backup on the next launch. File work lives in
//! `draugr_time_daemon::backup`.

use super::*;
use draugr_time_daemon::backup::{self, Backup, Interval, Settings, KEEP_CHOICES};
use iced::widget::{column, row, text};
use iced::{Alignment, Length};
use std::path::PathBuf;

/// Backups shown in the list; older ones stay on disk.
const SHOWN: usize = 8;

#[derive(Clone, Debug)]
pub enum Edit {
    Refresh,
    Now,
    Done(Result<String, String>),
    Automatic(bool),
    Interval(Interval),
    Keep(usize),
    Restore(PathBuf),
    ConfirmRestore,
    CancelRestore,
}

#[derive(Default)]
pub struct Ui {
    pub settings: Settings,
    pub list: Vec<Backup>,
    /// Backup the user picked and has not confirmed yet.
    pub confirm: Option<PathBuf>,
    /// A restore is staged for the next launch.
    pub staged: bool,
    pub busy: bool,
    pub message: String,
}

fn data_dir() -> Result<PathBuf, String> {
    draugr_time_daemon::paths::data_dir().map_err(|e| e.to_string())
}

fn size(bytes: u64) -> String {
    if bytes >= 1 << 20 {
        format!("{:.1} MB", bytes as f64 / f64::from(1 << 20))
    } else {
        format!("{} KB", bytes.div_ceil(1024))
    }
}

impl State {
    pub(super) fn edit_backup(&mut self, edit: Edit) -> Task<Message> {
        let ui = &mut self.backup_ui;
        let dir = match data_dir() {
            Ok(dir) => dir,
            Err(error) => {
                ui.message = error;
                return Task::none();
            }
        };
        match edit {
            Edit::Refresh => {
                match Settings::load(&dir) {
                    Ok(settings) => ui.settings = settings,
                    Err(error) => ui.message = error.to_string(),
                }
                ui.list = backup::list(&dir);
                ui.staged = backup::restore_pending(&dir);
            }
            Edit::Now if !ui.busy => {
                let Some(runtime) = &self.runtime else {
                    ui.message = tr("Start recording before managing stored data.");
                    return Task::none();
                };
                ui.busy = true;
                ui.message = tr("Backing up…");
                let shared = runtime.shared.clone();
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            backup::create(
                                &shared,
                                &dir,
                                backup::Kind::Manual,
                                draugr_time_daemon::now_ms(),
                            )
                            .map(|backup| {
                                backup
                                    .path
                                    .file_name()
                                    .map(|name| name.to_string_lossy().into_owned())
                                    .unwrap_or_default()
                            })
                        })
                        .await
                        .map_err(|e| e.to_string())
                        .and_then(|result| result)
                    },
                    |result| Message::Backup(Edit::Done(result)),
                );
            }
            Edit::Done(result) => {
                ui.busy = false;
                ui.message = match result {
                    Ok(name) => localized("Backup saved: {name}", &[("name", name)]),
                    Err(error) => localized("Backup failed: {error}", &[("error", error)]),
                };
                return self.edit_backup(Edit::Refresh);
            }
            Edit::Automatic(on) => {
                ui.settings.automatic = on;
                self.save_backup_settings();
            }
            Edit::Interval(interval) => {
                ui.settings.interval = interval;
                self.save_backup_settings();
            }
            Edit::Keep(keep) => {
                ui.settings.keep = keep;
                self.save_backup_settings();
            }
            Edit::Restore(path) => ui.confirm = Some(path),
            Edit::ConfirmRestore => {
                if let Some(path) = ui.confirm.take() {
                    ui.message = match backup::stage_restore(&dir, &path) {
                        Ok(()) => {
                            ui.staged = true;
                            tr("Restart Draugr Time to finish restoring. Current history will be kept in the backups folder.")
                        }
                        Err(error) => localized("Backup failed: {error}", &[("error", error)]),
                    };
                }
            }
            Edit::CancelRestore => {
                if ui.confirm.take().is_none() {
                    match backup::cancel_restore(&dir) {
                        Ok(()) => {
                            ui.staged = false;
                            ui.message = tr("Restore cancelled.");
                        }
                        Err(error) => ui.message = error.to_string(),
                    }
                }
            }
            Edit::Now => {}
        }
        Task::none()
    }

    /// Persist changed backup settings; called after each settings edit.
    fn save_backup_settings(&mut self) {
        let ui = &mut self.backup_ui;
        match data_dir().and_then(|dir| ui.settings.save(&dir).map_err(|e| e.to_string())) {
            Ok(()) => ui.message = tr("Backup settings saved."),
            Err(error) => ui.message = error,
        }
    }

    pub(super) fn backups_view(&self) -> Element<'_, Message> {
        let ui = &self.backup_ui;
        let p = design::Palette::new(self.theme.palette().background.r < 0.5);
        let send = |edit| Message::Backup(edit);
        let choice = |label: String, selected: bool, message: Message| {
            button(text(label).size(design::LABEL))
                .style(if selected {
                    design::primary
                } else {
                    design::secondary
                })
                .on_press(message)
        };
        let folder = data_dir()
            .map(|dir| backup::directory(&dir).display().to_string())
            .unwrap_or_else(|error| error);
        let mut body = column![
            text(tr("Backups")).size(design::BODY),
            text(folder).size(design::META).color(p.text2),
            button(text(tr(if ui.busy {
                "Backing up…"
            } else {
                "Back up now"
            })))
            .style(design::primary)
            .on_press_maybe((!ui.busy).then(|| send(Edit::Now))),
            row![
                management::switch(
                    ui.settings.automatic,
                    send(Edit::Automatic(!ui.settings.automatic)),
                    p
                ),
                text(tr("Automatic backups")).size(design::BODY),
            ]
            .spacing(design::CONTROL_GAP)
            .align_y(Alignment::Center),
        ]
        .spacing(design::CONTROL_GAP);
        if ui.settings.automatic {
            body = body.push(
                row![
                    choice(
                        tr("Daily"),
                        ui.settings.interval == Interval::Daily,
                        send(Edit::Interval(Interval::Daily))
                    ),
                    choice(
                        tr("Weekly"),
                        ui.settings.interval == Interval::Weekly,
                        send(Edit::Interval(Interval::Weekly))
                    ),
                ]
                .spacing(design::CONTROL_GAP)
                .wrap(),
            );
            let mut keep = row![text(tr("Keep")).size(design::LABEL)]
                .spacing(design::CONTROL_GAP)
                .align_y(Alignment::Center);
            for count in KEEP_CHOICES {
                keep = keep.push(choice(
                    count.to_string(),
                    ui.settings.keep == count,
                    send(Edit::Keep(count)),
                ));
            }
            body = body.push(keep.wrap()).push(
                text(tr(
                    "Older automatic backups are deleted. Manual backups are kept.",
                ))
                .size(design::META)
                .color(p.text2),
            );
        }
        if ui.staged {
            body = body.push(
                row![
                    text(tr("A restore is waiting for the next launch."))
                        .size(design::META)
                        .color(p.accent_text),
                    button(text(tr("Cancel restore")).size(design::LABEL))
                        .on_press(send(Edit::CancelRestore)),
                ]
                .spacing(design::CONTROL_GAP)
                .align_y(Alignment::Center)
                .wrap(),
            );
        }
        if ui.list.is_empty() {
            body = body.push(
                text(tr("No backups yet."))
                    .size(design::META)
                    .color(p.text2),
            );
        }
        for item in ui.list.iter().take(SHOWN) {
            let kind = tr(match item.kind {
                backup::Kind::Manual => "Manual",
                backup::Kind::Auto => "Automatic",
                backup::Kind::BeforeRestore => "Before restore",
            });
            body = body.push(
                row![
                    text(format!("{} · {kind} · {}", item.label, size(item.bytes)))
                        .size(design::META)
                        .width(Length::Fill),
                    button(text(tr("Restore…")).size(design::LABEL)).on_press_maybe(
                        (!ui.staged).then(|| send(Edit::Restore(item.path.clone())))
                    ),
                ]
                .spacing(design::CONTROL_GAP)
                .align_y(Alignment::Center),
            );
            if ui.confirm.as_ref() == Some(&item.path) {
                body = body
                    .push(
                        text(tr("Replace all history with this backup on the next launch? Current history will be kept in the backups folder."))
                            .size(design::META),
                    )
                    .push(
                        row![
                            button(text(tr("Restore on next launch")))
                                .style(design::destructive)
                                .on_press(send(Edit::ConfirmRestore)),
                            button(text(tr("Cancel"))).on_press(send(Edit::CancelRestore)),
                        ]
                        .spacing(design::CONTROL_GAP)
                        .wrap(),
                    );
            }
        }
        body.push(text(&ui.message).size(design::META)).into()
    }
}
