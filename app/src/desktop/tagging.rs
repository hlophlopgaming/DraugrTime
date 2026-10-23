use super::*;
use draugr_time_core::{Storage, TagUndo};
use iced::widget::column;

#[derive(Default)]
pub struct AwayPrompt {
    pub threshold: i64,
    pub seen_end: i64,
}
impl AwayPrompt {
    pub fn observe(&mut self, range: Option<Range>) -> Option<Range> {
        let range = range?;
        if range.end > self.seen_end
            && range.duration()
                >= if self.threshold > 0 {
                    self.threshold
                } else {
                    900_000
                }
        {
            self.seen_end = range.end;
            Some(range)
        } else {
            None
        }
    }
}
#[derive(Default)]
pub struct Editor {
    pub range_id: Option<i64>,
    pub name: String,
    pub note: String,
    pub start: String,
    pub end: String,
    pub color: String,
    pub tag_id: Option<i64>,
    pub archived: bool,
    pub overlap: bool,
    pub confirm_delete: bool,
    pub delete_usage: Option<(usize, i64)>,
}
#[derive(Clone, Debug)]
pub enum Edit {
    #[cfg(test)]
    Tag(i64),
    Open,
    Range(i64),
    Transform(i64, Range),
    Name(String),
    Note(String),
    Start(String),
    End(String),
    Save(bool),
    DeleteRange,
    Cancel,
    Undo,
    Quick(i64),
    Color(String),
    Archive(bool),
    SaveTag,
    Merge(i64),
    DeleteTag(bool),
    AwayTag,
    AwayRecent(String),
    IgnoreAway,
    Threshold(i64),
}

fn timestamp(value: &str) -> Result<i64, String> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|t| t.timestamp_millis())
        .map_err(|_| "Use date/time with timezone, e.g. 2026-10-14T09:30:00+03:00".into())
}
fn local_timestamp(ts: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ts)
        .map(|t| {
            t.with_timezone(&Local)
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, false)
        })
        .unwrap_or_default()
}
impl State {
    pub fn observe_away(&mut self) -> bool {
        if self.away_range.is_some() {
            return false;
        }
        let range = self.runtime.as_ref().and_then(|runtime| {
            match runtime.shared.storage.lock().unwrap().completed_afk() {
                Ok(range) => range.map(|(start, end)| Range::new(start, end)),
                Err(error) => {
                    self.error = error.to_string();
                    None
                }
            }
        });
        self.away_range = self.away_prompt.observe(range);
        self.away_range.is_some()
    }

    fn tag_write(
        &mut self,
        change: impl FnOnce(&mut Storage) -> Result<TagUndo, draugr_time_core::StorageError>,
    ) {
        let result = self
            .runtime
            .as_ref()
            .ok_or_else(|| "Start recording to edit tags".to_string())
            .and_then(|r| change(&mut r.shared.storage.lock().unwrap()).map_err(|e| e.to_string()));
        match result {
            Ok(undo) => {
                if self.tag_undo.len() == 20 {
                    self.tag_undo.remove(0);
                }
                self.tag_undo.push(undo);
                self.editor = None;
                self.error.clear();
                self.invalidate_load();
            }
            Err(error) => self.error = error,
        }
    }
    pub fn edit(&mut self, event: Edit) -> Task<Message> {
        match event {
            Edit::Open => {
                if let Some(range) = self.selection {
                    self.editor = Some(Editor {
                        start: local_timestamp(range.start),
                        end: local_timestamp(range.end),
                        ..Editor::default()
                    });
                    return widget::operation::focus("tag-name");
                }
            }
            Edit::Quick(minutes) => {
                let end = draugr_time_daemon::now_ms();
                self.change_date(Local::now().date_naive());
                self.set_screen("Day");
                self.selection = Some(Range::new((end - minutes * 60_000).max(0), end));
                let focus = self.edit(Edit::Open);
                return Task::batch([self.open(true), focus]);
            }
            Edit::Transform(id, range) => {
                let _ = self.edit(Edit::Range(id));
                if let Some(e) = &mut self.editor {
                    e.start = local_timestamp(range.start);
                    e.end = local_timestamp(range.end);
                }
                return self.edit(Edit::Save(false));
            }
            Edit::Range(id) => {
                if let Some(range) = self.tag_ranges.iter().find(|r| r.id == id) {
                    self.selection = Some(Range::new(range.start_ts, range.end_ts));
                    self.editor = Some(Editor {
                        range_id: Some(id),
                        name: model::tag_path(&self.tags, range.tag_id),
                        note: range.note.as_deref().unwrap_or("").into(),
                        start: local_timestamp(self.selection.unwrap().start),
                        end: local_timestamp(self.selection.unwrap().end),
                        ..Editor::default()
                    });
                }
            }
            Edit::Name(value) => {
                if let Some(e) = &mut self.editor {
                    e.name = value;
                    e.overlap = false;
                }
            }
            Edit::Note(value) => {
                if let Some(e) = &mut self.editor {
                    e.note = value;
                }
            }
            Edit::Start(value) => {
                if let Some(e) = &mut self.editor {
                    e.start = value;
                    e.overlap = false;
                }
            }
            Edit::End(value) => {
                if let Some(e) = &mut self.editor {
                    e.end = value;
                    e.overlap = false;
                }
            }
            Edit::Save(replace) => {
                if let Some(e) = &mut self.editor {
                    let bounds = timestamp(&e.start)
                        .and_then(|start| timestamp(&e.end).map(|end| (start, end)));
                    match bounds {
                        Ok((start, end)) => {
                            if !e.overlap {
                                let overlaps = self.runtime.as_ref().map(|runtime| {
                                    runtime
                                        .shared
                                        .storage
                                        .lock()
                                        .unwrap()
                                        .tag_ranges(start, end)
                                });
                                match overlaps {
                                    Some(Ok(ranges))
                                        if ranges.iter().any(|r| Some(r.id) != e.range_id) =>
                                    {
                                        e.overlap = true;
                                        return Task::none();
                                    }
                                    Some(Err(error)) => {
                                        self.error = error.to_string();
                                        return Task::none();
                                    }
                                    _ => {}
                                }
                            }
                            let (id, name, note) = (e.range_id, e.name.clone(), e.note.clone());
                            self.tag_write(|db| {
                                db.edit_tag_interval(id, &name, &note, start, end, replace)
                            });
                            if self.editor.is_none() {
                                self.away_range = None;
                                let started = Instant::now();
                                self.tag_toast = Some((Range::new(start, end), started));
                                return Task::perform(
                                    async move {
                                        tokio::time::sleep(Duration::from_secs(6)).await;
                                        started
                                    },
                                    Message::ToastExpired,
                                );
                            }
                        }
                        Err(error) => self.error = error,
                    }
                }
            }
            Edit::DeleteRange => {
                if let Some(id) = self.editor.as_ref().and_then(|e| e.range_id) {
                    self.tag_write(|db| db.delete_tag_range(id));
                }
            }
            Edit::Cancel => self.editor = None,
            Edit::Undo => {
                self.tag_toast = None;
                if let (Some(runtime), Some(undo)) = (&self.runtime, self.tag_undo.last().cloned())
                {
                    let result = runtime.shared.storage.lock().unwrap().undo_tags(undo);
                    match result {
                        Ok(()) => {
                            self.tag_undo.pop();
                            self.editor = None;
                            self.invalidate_load();
                        }
                        Err(e) => self.error = e.to_string(),
                    }
                }
            }
            #[cfg(test)]
            Edit::Tag(id) => {
                if let Some(t) = self.tags.iter().find(|t| t.id == id) {
                    self.editor = Some(Editor {
                        tag_id: Some(id),
                        name: t.name.clone(),
                        color: t.color.clone().unwrap_or_default(),
                        archived: t.archived == 1,
                        ..Editor::default()
                    });
                }
            }
            Edit::Color(value) => {
                if let Some(e) = &mut self.editor {
                    e.color = value;
                }
            }
            Edit::Archive(value) => {
                if let Some(e) = &mut self.editor {
                    e.archived = value;
                }
            }
            Edit::SaveTag => {
                if let Some(e) = &self.editor {
                    if let Some(id) = e.tag_id {
                        let (name, color, archive) = (e.name.clone(), e.color.clone(), e.archived);
                        self.tag_write(|db| {
                            db.update_tag(
                                id,
                                &name,
                                (!color.is_empty()).then_some(color.as_str()),
                                archive,
                            )
                        });
                    }
                }
            }
            Edit::Merge(target) => {
                if let Some(id) = self.editor.as_ref().and_then(|e| e.tag_id) {
                    self.tag_write(|db| db.merge_tags(id, target));
                }
            }
            Edit::DeleteTag(confirm) => {
                if let Some(e) = &mut self.editor {
                    if confirm && e.confirm_delete {
                        if let Some(id) = e.tag_id {
                            self.tag_write(|db| db.delete_tag_tree(id));
                        }
                    } else {
                        if let (Some(runtime), Some(id)) = (&self.runtime, e.tag_id) {
                            match runtime.shared.storage.lock().unwrap().tag_tree_usage(id) {
                                Ok(usage) => {
                                    e.delete_usage = Some(usage);
                                    e.confirm_delete = true;
                                }
                                Err(error) => self.error = error.to_string(),
                            }
                        }
                    }
                }
            }
            Edit::AwayRecent(name) => {
                let _ = self.edit(Edit::AwayTag);
                if let Some(e) = &mut self.editor {
                    e.name = name;
                }
                return self.edit(Edit::Save(false));
            }
            Edit::AwayTag => {
                if let Some(range) = self.away_range {
                    self.selection = Some(range);
                    self.set_screen("Day");
                    return self.edit(Edit::Open);
                }
            }
            Edit::IgnoreAway => self.away_range = None,
            Edit::Threshold(minutes) => {
                let threshold = minutes.saturating_mul(60_000);
                match save_threshold(threshold) {
                    Ok(()) => {
                        self.away_prompt.threshold = threshold;
                        self.away_range = None;
                    }
                    Err(error) => self.error = error,
                }
            }
        }
        Task::none()
    }
    pub fn tag_editor(&self) -> Element<'_, Message> {
        let Some(e) = &self.editor else {
            return space::vertical().height(0).into();
        };
        let edit = |e| Message::Edit(e);
        let mut body = column![
            text(tr(if e.tag_id.is_some() {
                "Edit tag"
            } else {
                "Tag selection"
            }))
            .size(18),
            text_input(&tr("Client, Project, Task"), &e.name)
                .id("tag-name")
                .on_input(move |v| edit(Edit::Name(v)))
        ]
        .spacing(8);
        if e.tag_id.is_some() {
            body = body
                .push(
                    text_input(&tr("#RRGGBB (optional)"), &e.color)
                        .on_input(move |v| edit(Edit::Color(v))),
                )
                .push(
                    checkbox(e.archived)
                        .label(tr("Archived (history retained)"))
                        .on_toggle(move |v| edit(Edit::Archive(v))),
                );
            body = body.push(
                row![
                    button(text(tr("Save")))
                        .on_press(edit(Edit::SaveTag))
                        .style(design::primary),
                    button(text(tr("Delete…"))).on_press(edit(Edit::DeleteTag(false))),
                    button(text(tr("Cancel"))).on_press(edit(Edit::Cancel))
                ]
                .spacing(8),
            );
            if e.confirm_delete {
                let (count, duration) = e.delete_usage.unwrap_or_default();
                body = body
                    .push(text(localized(
                        "Delete this subtree, {count} ranges ({duration}) and its rules?",
                        &[
                            ("count", count.to_string()),
                            ("duration", model::duration(duration)),
                        ],
                    )))
                    .push(
                        button(text(tr("Confirm delete")))
                            .on_press(edit(Edit::DeleteTag(true)))
                            .style(design::destructive),
                    );
            }
            body = body.push(text(tr("Merge into (moves history, children and rules):")));
            for t in &self.tags {
                let id = t.id;
                if Some(id) != e.tag_id {
                    body = body.push(
                        button(text(model::tag_path(&self.tags, id)))
                            .on_press(edit(Edit::Merge(id))),
                    );
                }
            }
        } else {
            let query = e.name.to_lowercase();
            let mut suggestions = self.recent_tags.clone();
            for t in &self.tags {
                let id = t.id;
                if !suggestions.contains(&id) {
                    suggestions.push(id);
                }
            }
            for id in suggestions
                .into_iter()
                .filter(|id| self.tags.iter().any(|t| t.id == *id && t.archived != 1))
                .filter(|id| {
                    model::tag_path(&self.tags, *id)
                        .to_lowercase()
                        .contains(&query)
                })
                .take(6)
            {
                let name = model::tag_path(&self.tags, id);
                body = body.push(button(text(name.clone())).on_press(edit(Edit::Name(name))));
            }
            body = body
                .push(
                    text_input(&tr("Note (optional)"), &e.note)
                        .on_input(move |v| edit(Edit::Note(v))),
                )
                .push(text(tr(
                    "Local date/time with timezone; changing bounds moves or resizes the block",
                )))
                .push(
                    row![
                        text_input(&tr("Start"), &e.start).on_input(move |v| edit(Edit::Start(v))),
                        text_input(&tr("End"), &e.end).on_input(move |v| edit(Edit::End(v)))
                    ]
                    .spacing(8),
                );
            let mut actions = column![
                button(text(tr(if e.overlap {
                    "Split existing blocks around selection"
                } else {
                    "Save"
                })))
                .on_press(edit(Edit::Save(false)))
                .style(design::primary),
                button(text(tr("Cancel"))).on_press(edit(Edit::Cancel))
            ]
            .spacing(8);
            if e.overlap {
                body = body.push(text(tr(
                    "Selection overlaps existing tags. Choose how to handle them.",
                )));
                actions = actions.push(
                    button(text(tr("Replace intersecting blocks")))
                        .on_press(edit(Edit::Save(true))),
                );
            }
            if e.range_id.is_some() {
                actions = actions
                    .push(button(text(tr("Delete block"))).on_press(edit(Edit::DeleteRange)));
            }
            body = body.push(actions);
        }
        container(body).padding(12).style(panel).into()
    }
}

pub fn drag_range(range: Range, delta: i64, edge: i8) -> Range {
    match edge {
        -1 => Range::new(
            range.start.saturating_add(delta).clamp(0, range.end - 1),
            range.end,
        ),
        1 => Range::new(
            range.start,
            range.end.saturating_add(delta).max(range.start + 1),
        ),
        _ => {
            let start = range
                .start
                .saturating_add(delta)
                .max(0)
                .min(i64::MAX - range.duration());
            Range::new(start, start + range.duration())
        }
    }
}

pub fn load_threshold() -> i64 {
    if cfg!(test) {
        return 900_000;
    }
    draugr_time_daemon::paths::data_dir()
        .ok()
        .and_then(|dir| std::fs::read(dir.join("away-prompt.json")).ok())
        .and_then(|bytes| serde_json::from_slice::<i64>(&bytes).ok())
        .filter(|v| *v > 0)
        .unwrap_or(900_000)
}
fn save_threshold(threshold: i64) -> Result<(), String> {
    let dir = draugr_time_daemon::paths::data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("away-prompt.json");
    let temporary = dir.join("away-prompt.json.tmp");
    std::fs::write(&temporary, threshold.to_string()).map_err(|e| e.to_string())?;
    std::fs::rename(temporary, path).map_err(|e| e.to_string())
}
