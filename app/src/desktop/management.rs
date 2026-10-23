use super::*;
use chrono::{Datelike, TimeZone};
use draugr_time_core::{Collection, Rule, RuleCoverage, Tag, TagStatistics, TagUndo};
use iced::widget::{column, pick_list, progress_bar};
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Debug)]
pub struct Data {
    pub tags: Vec<Tag>,
    pub usage: HashMap<i64, TagStatistics>,
    pub chart: HashMap<i64, [i64; 7]>,
    pub rules: Vec<Rule>,
    pub coverage: HashMap<i64, Result<RuleCoverage, String>>,
    pub date: NaiveDate,
    pub untagged_ms: i64,
    pub suggestions: Vec<(String, i64)>,
}
#[derive(Clone, Debug, Default)]
pub struct TagDraft {
    pub id: Option<i64>,
    pub name: String,
    pub color: String,
    pub parent: Option<i64>,
    pub archived: bool,
    pub dirty: bool,
    pub merge: bool,
    pub merge_target: Option<i64>,
    pub delete_usage: Option<(usize, i64)>,
}
#[derive(Default)]
pub struct Ui {
    pub data: Option<Data>,
    pub loading: bool,
    pub generation: u64,
    pub error: String,
    pub search: String,
    pub collapsed: HashSet<i64>,
    pub tag: Option<TagDraft>,
    pub dragging: Option<i64>,
    pub custom_color: bool,
    chart_cache: std::cell::RefCell<ChartCache>,
}
#[derive(Clone, Debug)]
pub enum Edit {
    Loaded(u64, Result<Box<Data>, String>),
    Search(String),
    Collapse(i64),
    SelectTag(i64),
    NewTag,
    TagName(String),
    TagColor(String),
    CustomColor,
    TagParent(Option<i64>),
    SaveTag,
    CancelTag,
    ArchiveTag,
    Merge,
    MergeTarget(i64),
    ConfirmMerge,
    DeleteTag,
    ConfirmDelete,
    ToggleRule(i64, bool),
    BeginDrag(i64),
    DragOver(i64),
    EndDrag,
    MoveRule(i64, i8),
    OpenRule(i64),
    Suggestion(String),
    ExportRules,
    Exported(Result<(), String>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Choice {
    id: Option<i64>,
    name: String,
}
impl std::fmt::Display for Choice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.name.fmt(f)
    }
}
fn boundaries(start: NaiveDate) -> [i64; 8] {
    std::array::from_fn(|i| {
        model::day_bounds(start.checked_add_days(Days::new(i as u64)).unwrap_or(start)).start
    })
}
fn descendant(tags: &[Tag], mut child: i64, parent: i64) -> bool {
    for _ in 0..=tags.len() {
        if child == parent {
            return true;
        }
        let Some(id) = tags
            .iter()
            .find(|t| t.id == child)
            .and_then(|t| t.parent_id)
        else {
            return false;
        };
        child = id;
    }
    false
}
fn tree_rows<'a>(tags: &'a [Tag], collapsed: &HashSet<i64>, query: &str) -> Vec<(&'a Tag, usize)> {
    let query = query.trim().to_lowercase();
    let mut rows: Vec<_> = tags
        .iter()
        .filter_map(|t| {
            let mut depth = 0;
            let mut parent = t.parent_id;
            for _ in 0..tags.len() {
                let Some(id) = parent else {
                    break;
                };
                if query.is_empty() && collapsed.contains(&id) {
                    return None;
                }
                depth += 1;
                parent = tags.iter().find(|p| p.id == id).and_then(|p| p.parent_id);
            }
            if !query.is_empty()
                && !model::tag_path(tags, t.id).to_lowercase().contains(&query)
                && !tags.iter().any(|c| {
                    descendant(tags, c.id, t.id)
                        && model::tag_path(tags, c.id).to_lowercase().contains(&query)
                })
            {
                return None;
            }
            Some((t, depth))
        })
        .collect();
    rows.sort_by_cached_key(|(t, _)| model::tag_path(tags, t.id).to_lowercase());
    rows
}
fn load(shared: draugr_time_daemon::Shared, date: NaiveDate) -> Result<Data, String> {
    let db = shared.storage.lock().map_err(|e| e.to_string())?;
    let week = date
        .checked_sub_days(Days::new(u64::from(date.weekday().num_days_from_monday())))
        .unwrap_or(date);
    let last7 = date.checked_sub_days(Days::new(6)).unwrap_or(date);
    let tags = db.tags().map_err(|e| e.to_string())?;
    let usage = db
        .tag_statistics(&boundaries(week))
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|s| (s.id, s))
        .collect();
    let chart = db
        .tag_statistics(&boundaries(last7))
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|s| (s.id, s.days))
        .collect();
    let rules = db.rules().map_err(|e| e.to_string())?;
    let coverage = rules
        .iter()
        .map(|r| {
            (
                r.id,
                if r.enabled {
                    db.rule_coverage(
                        model::day_bounds(last7).start,
                        model::day_bounds(date).end,
                        r,
                    )
                    .map_err(|e| e.to_string())
                } else {
                    Ok(RuleCoverage::default())
                },
            )
        })
        .collect();
    let today = model::day_bounds(date);
    let report = db
        .report(today.start, today.end)
        .map_err(|e| e.to_string())?;
    let untagged_ms = report
        .intervals
        .iter()
        .filter(|i| i.tag_id.is_none())
        .map(|i| i.end_ts - i.start_ts)
        .sum();
    let windows = db
        .events(Some("window"), today.start, today.end)
        .map_err(|e| e.to_string())?;
    let mut apps: BTreeMap<String, i64> = BTreeMap::new();
    let mut cursor = 0;
    for i in report.intervals.iter().filter(|i| i.tag_id.is_none()) {
        while cursor < windows.len() && windows[cursor].end_ts <= i.start_ts {
            cursor += 1;
        }
        if let Some(app) = windows
            .get(cursor)
            .filter(|w| w.start_ts <= i.start_ts && w.end_ts >= i.end_ts)
            .and_then(|w| w.data["app"].as_str())
        {
            *apps.entry(app.to_owned()).or_default() += i.end_ts - i.start_ts;
        }
    }
    let mut suggestions: Vec<_> = apps.into_iter().collect();
    suggestions.sort_by_key(|(_, ms)| std::cmp::Reverse(*ms));
    suggestions.truncate(3);
    Ok(Data {
        tags,
        usage,
        chart,
        rules,
        coverage,
        date,
        untagged_ms,
        suggestions,
    })
}
fn remember(state: &mut State, undo: TagUndo) {
    if state.tag_undo.len() == 20 {
        state.tag_undo.remove(0);
    }
    state.tag_undo.push(undo);
    state.invalidate_load();
}
impl Ui {
    fn release(&mut self) {
        self.data = None;
        self.loading = false;
        self.generation = self.generation.wrapping_add(1);
        self.dragging = None;
        self.chart_cache.get_mut().geometry.clear();
    }
}
impl State {
    pub(super) fn release_management(&mut self) {
        self.management.release();
        if !self.rule_editor.confirm {
            self.rule_editor.preview = Vec::new();
            self.rule_editor.coverage = Default::default();
            self.rule_editor.live_ms = 0;
        }
    }
    pub(super) fn refresh_management(&mut self) -> Task<Message> {
        self.management.generation = self.management.generation.wrapping_add(1);
        let generation = self.management.generation;
        let Some(runtime) = &self.runtime else {
            self.management.error = tr(if self.screen == "Rules" {
                "Start recording to edit rules"
            } else {
                "Start recording to edit tags"
            });
            self.management.loading = false;
            return Task::none();
        };
        let shared = runtime.shared.clone();
        self.finish_motion();
        self.management.loading = true;
        self.management.error.clear();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || load(shared, Local::now().date_naive()))
                    .await
                    .map_err(|e| e.to_string())
                    .and_then(|d| d)
            },
            move |r| Message::Manage(Edit::Loaded(generation, r.map(Box::new))),
        )
    }
    pub(super) fn edit_management(&mut self, event: Edit) -> Task<Message> {
        match event {
            Edit::Loaded(g, result) => {
                if g != self.management.generation || !matches!(self.screen, "Tags" | "Rules") {
                    return Task::none();
                }
                self.management.loading = false;
                match result {
                    Ok(data) => {
                        let data = *data;
                        self.tags = data.tags.clone();
                        if let Some(draft) = &self.management.tag {
                            if draft
                                .id
                                .is_some_and(|id| !data.tags.iter().any(|t| t.id == id))
                            {
                                self.management.tag = None;
                            }
                        }
                        if let Some(r) = data
                            .rules
                            .iter()
                            .find(|r| Some(r.id) == self.rule_editor.id)
                        {
                            self.rule_editor.priority = r.priority.to_string();
                        }
                        if let Some(draft) = self.management.tag.as_mut().filter(|t| !t.dirty) {
                            if let Some(tag) = data.tags.iter().find(|t| Some(t.id) == draft.id) {
                                draft.name = tag.name.clone();
                                draft.color = tag.color.clone().unwrap_or_default();
                                draft.parent = tag.parent_id;
                                draft.archived = tag.archived != 0;
                            }
                        }
                        self.management.data = Some(data);
                        self.management.error.clear();
                        self.management.chart_cache.borrow_mut().geometry.clear();
                        if self.screen == "Rules"
                            && !self.rule_editor.field.is_empty()
                            && !self.rule_editor.confirm
                            && self.rule_editor.error.is_empty()
                        {
                            self.edit_rule(rules::Edit::RefreshPreview);
                        }
                    }
                    Err(e) => self.management.error = e,
                }
            }
            Edit::Search(s) => self.management.search = s,
            Edit::Collapse(id) => {
                let closed = self.management.collapsed.contains(&id);
                self.animate(motion::Key::Tree(id), f32::from(!closed), f32::from(closed));
                if !self.management.collapsed.remove(&id) {
                    self.management.collapsed.insert(id);
                }
            }
            Edit::SelectTag(id) => {
                self.management.custom_color = false;
                if let Some(t) = self
                    .management
                    .data
                    .as_ref()
                    .and_then(|d| d.tags.iter().find(|t| t.id == id))
                {
                    self.management.tag = Some(TagDraft {
                        id: Some(id),
                        name: t.name.clone(),
                        color: t.color.clone().unwrap_or_default(),
                        parent: t.parent_id,
                        archived: t.archived != 0,
                        ..Default::default()
                    });
                }
            }
            Edit::NewTag => {
                self.management.tag = Some(TagDraft {
                    dirty: true,
                    ..Default::default()
                })
            }
            Edit::TagName(s) => {
                if let Some(t) = &mut self.management.tag {
                    t.name = s;
                    t.dirty = true;
                    t.delete_usage = None;
                }
            }
            Edit::CustomColor => self.management.custom_color = !self.management.custom_color,
            Edit::TagColor(s) => {
                if let Some(t) = &mut self.management.tag {
                    t.color = s;
                    t.dirty = true;
                    t.delete_usage = None;
                }
            }
            Edit::TagParent(p) => {
                if let Some(t) = &mut self.management.tag {
                    t.parent = p;
                    t.dirty = true;
                    t.delete_usage = None;
                }
            }
            Edit::CancelTag => {
                let id = self.management.tag.as_ref().and_then(|t| t.id);
                self.management.tag = None;
                if let Some(id) = id {
                    return self.edit_management(Edit::SelectTag(id));
                }
            }
            Edit::SaveTag | Edit::ArchiveTag => {
                let archive = matches!(event, Edit::ArchiveTag);
                let Some(draft) = self.management.tag.clone() else {
                    return Task::none();
                };
                let Some(runtime) = &self.runtime else {
                    return Task::none();
                };
                let result = (|| {
                    let mut db = runtime.shared.storage.lock().map_err(|e| e.to_string())?;
                    if let Some(id) = draft.id {
                        db.update_tag_details(
                            id,
                            &draft.name,
                            (!draft.color.is_empty()).then_some(draft.color.as_str()),
                            if archive {
                                !draft.archived
                            } else {
                                draft.archived
                            },
                            draft.parent,
                        )
                        .map(|u| (id, u))
                        .map_err(|e| e.to_string())
                    } else {
                        db.create_tag_path(&draft.name).map_err(|e| e.to_string())
                    }
                })();
                match result {
                    Ok((id, undo)) => {
                        remember(self, undo);
                        let saved = self
                            .runtime
                            .as_ref()
                            .unwrap()
                            .shared
                            .storage
                            .lock()
                            .unwrap()
                            .tags();
                        match saved {
                            Ok(tags) => {
                                self.management.tag =
                                    tags.into_iter().find(|t| t.id == id).map(|tag| TagDraft {
                                        id: Some(id),
                                        name: tag.name,
                                        color: tag.color.unwrap_or_default(),
                                        parent: tag.parent_id,
                                        archived: tag.archived != 0,
                                        ..Default::default()
                                    })
                            }
                            Err(error) => {
                                self.management.tag = None;
                                self.error = error.to_string();
                            }
                        }
                        return self.refresh_management();
                    }
                    Err(e) => self.management.error = e,
                }
            }
            Edit::Merge => {
                if let Some(t) = &mut self.management.tag {
                    t.merge = !t.merge;
                    t.delete_usage = None;
                }
            }
            Edit::MergeTarget(id) => {
                if let Some(t) = &mut self.management.tag {
                    t.merge_target = Some(id);
                }
            }
            Edit::ConfirmMerge => {
                let Some((source, target)) = self
                    .management
                    .tag
                    .as_ref()
                    .and_then(|t| Some((t.id?, t.merge_target?)))
                else {
                    return Task::none();
                };
                let result = self
                    .runtime
                    .as_ref()
                    .map(|r| r.shared.storage.lock().unwrap().merge_tags(source, target));
                if let Some(result) = result {
                    match result {
                        Ok(undo) => {
                            remember(self, undo);
                            self.management.tag = None;
                            return self.refresh_management();
                        }
                        Err(e) => self.management.error = e.to_string(),
                    }
                }
            }
            Edit::DeleteTag | Edit::ConfirmDelete => {
                let confirm = matches!(event, Edit::ConfirmDelete);
                let Some(id) = self.management.tag.as_ref().and_then(|t| t.id) else {
                    return Task::none();
                };
                let Some(runtime) = &self.runtime else {
                    return Task::none();
                };
                let result = (|| {
                    let mut db = runtime.shared.storage.lock().map_err(|e| e.to_string())?;
                    let usage = db.tag_tree_usage(id).map_err(|e| e.to_string())?;
                    if confirm
                        && self.management.tag.as_ref().and_then(|t| t.delete_usage) == Some(usage)
                    {
                        db.delete_tag_tree(id)
                            .map(|u| (usage, Some(u)))
                            .map_err(|e| e.to_string())
                    } else {
                        Ok((usage, None))
                    }
                })();
                match result {
                    Ok((usage, None)) => {
                        if let Some(t) = &mut self.management.tag {
                            t.delete_usage = Some(usage);
                            t.merge = false;
                        }
                    }
                    Ok((_, Some(undo))) => {
                        remember(self, undo);
                        self.management.tag = None;
                        return self.refresh_management();
                    }
                    Err(e) => self.management.error = e,
                }
            }
            Edit::ToggleRule(id, enabled) => {
                let result = self.runtime.as_ref().map(|r| {
                    let mut db=r.shared.storage.lock().unwrap();
                    let rule=db.rules()?.into_iter().find(|r|r.id==id).ok_or(draugr_time_core::StorageError::NotFound)?;
                    db.put(Collection::Rules,Some(id),serde_json::json!({"field":rule.field,"pattern":rule.pattern,"tag_id":rule.tag_id,"priority":rule.priority,"enabled":enabled}))
                });
                if let Some(Err(e)) = result {
                    self.management.error = e.to_string();
                } else {
                    if self.rule_editor.id == Some(id) {
                        self.rule_editor.enabled = enabled;
                    }
                    return self.refresh_management();
                }
            }
            Edit::BeginDrag(id) => self.management.dragging = Some(id),
            Edit::DragOver(id) => {
                if let (Some(source), Some(data)) =
                    (self.management.dragging, &mut self.management.data)
                {
                    if let (Some(a), Some(b)) = (
                        data.rules.iter().position(|r| r.id == source),
                        data.rules.iter().position(|r| r.id == id),
                    ) {
                        let rule = data.rules.remove(a);
                        data.rules.insert(b, rule);
                    }
                }
            }
            Edit::MoveRule(id, delta) => {
                if let Some(data) = &mut self.management.data {
                    if let Some(index) = data.rules.iter().position(|r| r.id == id) {
                        let next = (index as isize + delta as isize)
                            .clamp(0, data.rules.len().saturating_sub(1) as isize)
                            as usize;
                        data.rules.swap(index, next);
                    }
                }
                return self.edit_management(Edit::EndDrag);
            }
            Edit::EndDrag => {
                self.management.dragging = None;
                if let (Some(data), Some(runtime)) = (&mut self.management.data, &self.runtime) {
                    let ids: Vec<_> = data.rules.iter().map(|r| r.id).collect();
                    let mut db = runtime.shared.storage.lock().unwrap();
                    if let Err(error) = db.reorder_rules(&ids) {
                        self.management.error = error.to_string();
                        if let Ok(rules) = db.rules() {
                            data.rules = rules;
                        }
                        return Task::none();
                    }
                    let count = data.rules.len();
                    for (index, rule) in data.rules.iter_mut().enumerate() {
                        rule.priority = (count - index) as i64;
                        if self.rule_editor.id == Some(rule.id) {
                            self.rule_editor.priority = rule.priority.to_string();
                        }
                    }
                    drop(db);
                    return self.refresh_management();
                }
            }
            Edit::OpenRule(id) => {
                self.set_screen("Rules");
                self.edit_rule(rules::Edit::Select(id));
                return self.refresh_management();
            }
            Edit::Suggestion(app) => {
                self.edit_rule(rules::Edit::New);
                self.rule_editor.field = "app".into();
                self.edit_rule(rules::Edit::Pattern(
                    draugr_time_core::literal_rule_pattern(&app),
                ));
            }
            Edit::ExportRules => {
                let Some(data) = &self.management.data else {
                    return Task::none();
                };
                let bytes = rules_json(data);
                return Task::perform(
                    async move {
                        let Some(file) = rfd::AsyncFileDialog::new()
                            .set_file_name("draugr-rules.json")
                            .save_file()
                            .await
                        else {
                            return Ok(());
                        };
                        reports::write_export(file.path(), &bytes)
                    },
                    |r| Message::Manage(Edit::Exported(r)),
                );
            }
            Edit::Exported(result) => {
                self.management.error = result.err().unwrap_or_else(|| tr("Export complete"))
            }
        }
        Task::none()
    }
}
fn rules_json(data: &Data) -> Vec<u8> {
    serde_json::to_vec_pretty(&serde_json::json!({"format":"draugr-time-rules-v1","tags":data.tags,"rules":data.rules.iter().map(|r|serde_json::json!({"id":r.id,"field":r.field,"pattern":r.pattern,"tag_id":r.tag_id,"priority":r.priority,"enabled":r.enabled})).collect::<Vec<_>>()})).expect("serializable rule configuration")
}

fn palette(state: &State) -> design::Palette {
    design::Palette::new(state.theme.palette().background.r < 0.5)
}
fn rule_field(field: &str) -> String {
    tr(match field {
        "app" => "App",
        "title" => "Title",
        "url" => "URL",
        _ => "Any",
    })
}
fn divider(p: design::Palette) -> Element<'static, Message> {
    container(space::horizontal())
        .width(Length::Fill)
        .height(design::HAIRLINE)
        .style(move |_| container::Style {
            background: Some(p.line_soft.into()),
            ..Default::default()
        })
        .into()
}
fn dot(color: Color) -> Element<'static, Message> {
    container(space::horizontal())
        .width(design::TAG_SWATCH)
        .height(design::TAG_SWATCH)
        .style(move |_| container::Style {
            background: Some(color.into()),
            ..Default::default()
        })
        .into()
}
fn tag_color(tags: &[Tag], id: i64) -> Color {
    tags.iter()
        .find(|t| t.id == id)
        .and_then(|t| t.color.as_deref())
        .and_then(|s| s.strip_prefix('#'))
        .and_then(|s| u32::from_str_radix(s, 16).ok())
        .map(|rgb| Color::from_rgb8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8))
        .unwrap_or_else(|| {
            let c = model::color(&id.to_string());
            Color::from_rgb(c[0], c[1], c[2])
        })
}
fn progress(ms: i64, total: i64, color: Color, p: design::Palette) -> Element<'static, Message> {
    progress_bar(0.0..=1.0, ms as f32 / total.max(1) as f32)
        .girth(design::PROGRESS)
        .style(move |_| progress_bar::Style {
            background: p.line_soft.into(),
            bar: color.into(),
            border: iced::Border::default(),
        })
        .into()
}
fn caption(value: &str, p: design::Palette) -> Element<'static, Message> {
    text(tr(value).to_uppercase())
        .size(design::CAPTION)
        .color(p.text3)
        .into()
}
fn badge(value: String, p: design::Palette) -> Element<'static, Message> {
    container(text(value).size(design::CAPTION).color(p.text3))
        .padding(design::SUMMARY_GAP as u16)
        .style(move |_| container::Style {
            border: iced::Border {
                color: p.line,
                width: design::HAIRLINE,
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}
fn last_used(ts: Option<i64>, now: i64) -> String {
    let Some(ts) = ts else {
        return "–".into();
    };
    let age = (now - ts).max(0);
    if age < 60_000 {
        tr("now")
    } else if age < 3_600_000 {
        localized("{count} min ago", &[("count", (age / 60_000).to_string())])
    } else if age < 86_400_000 {
        localized("{count} h ago", &[("count", (age / 3_600_000).to_string())])
    } else {
        Local
            .timestamp_millis_opt(ts)
            .single()
            .map(|date| localization::short_date_label(date.date_naive()))
            .unwrap_or_else(|| "–".into())
    }
}
pub(super) fn switch(on: bool, message: Message, p: design::Palette) -> Element<'static, Message> {
    let knob = container(space::horizontal())
        .width(design::SWITCH_KNOB)
        .height(design::SWITCH_KNOB)
        .style(move |_| container::Style {
            background: Some(p.text.into()),
            ..Default::default()
        });
    let content: Element<'static, Message> = if on {
        row![space::horizontal().height(design::SWITCH_KNOB), knob]
            .height(design::SWITCH_KNOB)
            .into()
    } else {
        row![knob, space::horizontal().height(design::SWITCH_KNOB)]
            .height(design::SWITCH_KNOB)
            .into()
    };
    let track = container(content)
        .width(design::SWITCH_WIDTH)
        .height(design::SWITCH_KNOB + 2.0 * design::HAIRLINE)
        .padding(design::HAIRLINE)
        .style(move |_| container::Style {
            background: Some(if on { p.accent } else { p.line }.into()),
            ..Default::default()
        });
    button(
        container(track)
            .center_x(design::CONTROL)
            .center_y(design::DENSE_TARGET),
    )
    .on_press(message)
    .style(design::ghost)
    .padding(0)
    .width(design::CONTROL)
    .height(design::DENSE_TARGET)
    .into()
}
fn wave(p: design::Palette) -> Element<'static, Message> {
    svg(svg::Handle::from_memory(br#"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="4" viewBox="0 0 120 4"><defs><pattern id="w" width="6" height="4" patternUnits="userSpaceOnUse"><path d="M0 2Q1.5 0 3 2T6 2" fill="none" stroke="currentColor"/></pattern></defs><rect width="120" height="4" fill="url(#w)"/></svg>"#.to_vec())).width(Length::Fill).height(design::ERROR_UNDERLINE).style(move |_,_|svg::Style{color:Some(p.danger_text)}).into()
}
impl State {
    pub(super) fn management_view(&self, narrow: bool) -> Element<'_, Message> {
        let p = palette(self);
        let ui = &self.management;
        let tags = self.screen == "Tags";
        let title = text(tr(self.screen))
            .font(design::SEMIBOLD)
            .size(design::TITLE)
            .line_height(1.1);
        let new = button(
            text(tr(if tags { "+ New tag" } else { "+ New rule" }))
                .font(design::SEMIBOLD)
                .size(design::BODY),
        )
        .on_press_maybe(self.runtime.as_ref().map(|_| {
            if tags {
                Message::Manage(Edit::NewTag)
            } else {
                Message::Rule(rules::Edit::New)
            }
        }))
        .style(design::primary);
        let mut actions = row![]
            .spacing(design::CONTROL_GAP)
            .align_y(Alignment::Center);
        if tags {
            actions = actions.push(
                text_input(&tr("Find a tag"), &ui.search)
                    .id("manager-tag-search")
                    .on_input(|s| Message::Manage(Edit::Search(s)))
                    .width(if narrow {
                        Length::Fill
                    } else {
                        Length::Fixed(design::MANAGER_SEARCH)
                    }),
            );
        } else {
            actions = actions
                .push(tooltip(
                    button(text(tr("Import ActivityWatch categories")).size(design::LABEL))
                        .style(design::secondary),
                    container(text(tr("Category import is a later M12 task.")).size(design::LABEL))
                        .padding(design::CONTROL_GAP as u16)
                        .style(design::popover),
                    tooltip::Position::Bottom,
                ))
                .push(
                    button(text(tr("Export"))).on_press_maybe(
                        ui.data.as_ref().map(|_| Message::Manage(Edit::ExportRules)),
                    ),
                );
        }
        actions = actions.push(new);
        let heading: Element<'_, Message> = if narrow {
            column![title, actions.wrap()]
                .spacing(design::CONTROL_GAP)
                .into()
        } else {
            row![title.width(Length::Fill), actions]
                .spacing(design::GAP)
                .align_y(Alignment::Center)
                .into()
        };
        let subtitle = if tags {
            "Client → project → task. Type “Client, Project, Task” to create a branch."
        } else {
            "Tag time automatically. The first matching rule from the top wins."
        };
        let mut body = column![column![
            heading,
            text(tr(subtitle)).size(design::LABEL).color(p.text3)
        ]
        .spacing(design::SMALL_GAP)]
        .spacing(design::GAP);
        if !ui.error.is_empty() {
            body = body.push(
                text(localization::error(&ui.error))
                    .size(design::BODY)
                    .color(p.danger_text),
            );
        }
        if ui.loading && ui.data.is_none() {
            body = body.push(text(tr("Loading…")).size(design::BODY));
        }
        let table = if tags {
            self.tag_table(narrow)
        } else {
            self.rule_table(narrow)
        };
        body = body.push(table);
        if narrow {
            body.into()
        } else {
            body.height(Length::Fill).into()
        }
    }
    fn tag_table(&self, narrow: bool) -> Element<'_, Message> {
        let p = palette(self);
        let ui = &self.management;
        let header = manager_columns(
            caption("Tag name", p),
            caption("This week", p),
            caption("All time", p),
            caption("Rules", p),
            caption("Last used", p),
        );
        let mut rows = column![header, divider(p)];
        if let Some(data) = &ui.data {
            let maximum = data
                .usage
                .values()
                .map(|s| s.days.iter().sum::<i64>())
                .max()
                .unwrap_or(1);
            let collapsed: HashSet<_> = ui
                .collapsed
                .iter()
                .copied()
                .filter(|id| !self.motion.active(motion::Key::Tree(*id)))
                .collect();
            for (tag, depth) in tree_rows(&data.tags, &collapsed, &ui.search) {
                let mut visibility: f32 = 1.0;
                let mut parent = tag.parent_id;
                for _ in 0..data.tags.len() {
                    let Some(id) = parent else {
                        break;
                    };
                    if ui.search.is_empty() {
                        visibility = visibility.min(self.motion.value(
                            motion::Key::Tree(id),
                            f32::from(!ui.collapsed.contains(&id)),
                        ));
                    }
                    parent = data
                        .tags
                        .iter()
                        .find(|tag| tag.id == id)
                        .and_then(|tag| tag.parent_id);
                }
                let id = tag.id;
                let selected = ui.tag.as_ref().and_then(|t| t.id) == Some(id);
                let has_children = data.tags.iter().any(|t| t.parent_id == Some(id));
                let color = tag_color(&data.tags, id);
                let ink = if tag.archived != 0 { p.text3 } else { p.text };
                let mut name = row![space::horizontal().width(depth as f32 * design::TREE_INDENT)]
                    .align_y(Alignment::Center)
                    .spacing(design::CONTROL_GAP);
                name = name
                    .push(
                        button(motion::chevron(
                            self.motion.value(
                                motion::Key::Tree(id),
                                f32::from(!ui.collapsed.contains(&id)),
                            ),
                            if has_children {
                                p.text2
                            } else {
                                iced::Color::TRANSPARENT
                            },
                        ))
                        .on_press_maybe(has_children.then_some(Message::Manage(Edit::Collapse(id))))
                        .style(design::ghost)
                        .padding(0)
                        .width(design::DENSE_TARGET)
                        .height(design::DENSE_TARGET),
                    )
                    .push(dot(color))
                    .push(
                        button(
                            text(tag.name.clone())
                                .size(design::BODY)
                                .font(if depth == 0 {
                                    design::SEMIBOLD
                                } else {
                                    design::UI
                                })
                                .color(ink),
                        )
                        .on_press(Message::Manage(Edit::SelectTag(id)))
                        .style(design::ghost)
                        .padding(0)
                        .width(Length::Fill)
                        .height(Length::Shrink),
                    );
                if tag.archived != 0 {
                    name = name.push(badge(tr("Archived").to_uppercase(), p));
                }
                let stat = data.usage.get(&id);
                let week = stat.map(|s| s.days.iter().sum()).unwrap_or(0);
                let rules = data
                    .rules
                    .iter()
                    .filter(|r| descendant(&data.tags, r.tag_id, id))
                    .count();
                let week_cell = row![
                    progress(week, maximum, color, p),
                    text(if week == 0 {
                        "–".into()
                    } else {
                        model::duration(week)
                    })
                    .size(design::LABEL)
                    .font(design::MONO)
                    .width(design::MANAGER_DURATION)
                ]
                .spacing(design::CONTROL_GAP)
                .align_y(Alignment::Center);
                let line = manager_columns(
                    name.into(),
                    week_cell.into(),
                    text(
                        stat.map(|s| model::duration(s.all_ms))
                            .unwrap_or_else(|| "–".into()),
                    )
                    .size(design::LABEL)
                    .font(design::MONO)
                    .color(p.text2)
                    .into(),
                    text(rules.to_string())
                        .size(design::LABEL)
                        .font(design::MONO)
                        .color(p.text2)
                        .into(),
                    text(last_used(
                        stat.and_then(|s| s.last_used),
                        draugr_time_daemon::now_ms(),
                    ))
                    .size(design::LABEL)
                    .color(p.text3)
                    .into(),
                );
                rows = rows.push(motion::effect(
                    column![
                        container(line).style(move |_| container::Style {
                            background: selected.then_some(p.raised.into()),
                            ..Default::default()
                        }),
                        divider(p)
                    ],
                    motion::Visual {
                        opacity: visibility,
                        height: visibility,
                        ..Default::default()
                    },
                    p.surf,
                ));
            }
            if data.tags.is_empty() {
                rows = rows.push(
                    text(tr("No tags yet. Create a tag or select time in Day.")).size(design::BODY),
                );
            }
        } else if self.runtime.is_none() {
            rows = rows
                .push(text(tr("Start recording to edit tags")))
                .push(button(text(tr("Start recording"))).on_press(Message::Start));
        }
        let content: Element<'_, Message> = if narrow {
            scrollable(rows.width(design::MANAGER_TABLE_MIN))
                .direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::default(),
                ))
                .style(design::scrollbars)
                .into()
        } else {
            scrollable(rows)
                .spacing(design::CONTROL_GAP as f32)
                .style(design::scrollbars)
                .height(Length::Fill)
                .into()
        };
        container(content)
            .padding(design::MANAGER_TABLE_PADDING)
            .style(panel)
            .width(Length::Fill)
            .height(if narrow { Length::Shrink } else { Length::Fill })
            .into()
    }
    fn rule_table(&self, narrow: bool) -> Element<'_, Message> {
        let p = palette(self);
        let ui = &self.management;
        let header = rule_columns(
            text("").into(),
            caption("On", p),
            caption("Field", p),
            caption("Pattern", p),
            caption("Tag name", p),
            caption("Last 7 d", p),
        );
        let mut rows = column![header, divider(p)];
        if let Some(data) = &ui.data {
            for (index, rule) in data.rules.iter().enumerate() {
                let id = rule.id;
                let selected = self.rule_editor.id == Some(id);
                let field = if selected && !self.rule_editor.field.is_empty() {
                    self.rule_editor.field.as_str()
                } else {
                    rule.field.as_str()
                };
                let pattern = if selected && !self.rule_editor.field.is_empty() {
                    self.rule_editor.pattern.clone()
                } else {
                    rule.pattern.clone()
                };
                let invalid = if selected {
                    self.rule_editor.regex_error.is_some()
                } else {
                    data.coverage.get(&id).is_some_and(Result::is_err)
                };
                let color = tag_color(&data.tags, rule.tag_id);
                let ink = if invalid {
                    p.danger_text
                } else if !rule.enabled {
                    p.text3
                } else {
                    p.text
                };
                let number = widget::mouse_area(
                    container(
                        text((index + 1).to_string())
                            .size(design::LABEL)
                            .color(p.text3),
                    )
                    .width(design::RULE_NUMBER)
                    .center_y(design::DENSE_TARGET),
                )
                .on_press(Message::Manage(Edit::BeginDrag(id)))
                .interaction(iced::mouse::Interaction::Grab);
                let mut pattern_content = column![text(pattern)
                    .font(design::MONO)
                    .size(design::META)
                    .color(ink)];
                if invalid {
                    pattern_content = pattern_content.push(wave(p));
                }
                let pattern = button(pattern_content)
                    .on_press(Message::Rule(rules::Edit::Select(id)))
                    .style(design::ghost)
                    .padding(0)
                    .height(design::DENSE_TARGET)
                    .width(Length::Fill);
                let matched = if invalid {
                    tr("error")
                } else {
                    model::duration(
                        data.coverage
                            .get(&id)
                            .and_then(|c| c.as_ref().ok())
                            .map(|c| c.matched_ms)
                            .unwrap_or(0),
                    )
                };
                let line = rule_columns(
                    number.into(),
                    switch(
                        rule.enabled,
                        Message::Manage(Edit::ToggleRule(id, !rule.enabled)),
                        p,
                    ),
                    badge(rule_field(field), p),
                    pattern.into(),
                    button(
                        row![
                            dot(color),
                            text(model::tag_path(&data.tags, rule.tag_id))
                                .size(design::META)
                                .color(if rule.enabled { p.text } else { p.text3 })
                        ]
                        .spacing(design::CONTROL_GAP)
                        .align_y(Alignment::Center),
                    )
                    .on_press(Message::Rule(rules::Edit::Select(id)))
                    .style(design::ghost)
                    .padding(0)
                    .height(design::DENSE_TARGET)
                    .into(),
                    text(matched)
                        .font(design::MONO)
                        .size(design::LABEL)
                        .color(if invalid { p.danger_text } else { p.text2 })
                        .into(),
                );
                rows = rows
                    .push(
                        widget::mouse_area(container(line).style(move |_| container::Style {
                            background:
                                (selected || ui.dragging == Some(id)).then_some(p.raised.into()),
                            ..Default::default()
                        }))
                        .on_enter(Message::Manage(Edit::DragOver(id))),
                    )
                    .push(divider(p));
            }
            if data.rules.is_empty() {
                rows = rows.push(
                    text(tr("No rules yet. Create a rule to preview its matches."))
                        .size(design::BODY),
                );
            }
        } else if self.runtime.is_none() {
            rows = rows
                .push(text(tr("Start recording to edit rules")))
                .push(button(text(tr("Start recording"))).on_press(Message::Start));
        }
        let mut card = rows;
        if let Some(data) = &ui.data {
            let hint = localized(
                "{duration} today is still untagged.",
                &[("duration", model::duration(data.untagged_ms))],
            );
            let mut hints =
                row![text(hint).size(design::LABEL).color(p.text2)].spacing(design::CONTROL_GAP);
            for (app, ms) in &data.suggestions {
                hints = hints.push(
                    button(
                        text(format!("{} · {} →", app, model::duration(*ms))).size(design::LABEL),
                    )
                    .on_press(Message::Manage(Edit::Suggestion(app.clone())))
                    .style(design::link)
                    .padding(0),
                );
            }
            card = card.push(
                container(hints.wrap())
                    .width(Length::Fill)
                    .padding(design::CONTROL_GAP as u16)
                    .style(design::segment_track),
            );
        }
        let list: Element<'_, Message> = if narrow {
            scrollable(card.width(design::MANAGER_TABLE_MIN))
                .direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::default(),
                ))
                .style(design::scrollbars)
                .into()
        } else {
            scrollable(card)
                .spacing(design::CONTROL_GAP as f32)
                .height(Length::Fill)
                .style(design::scrollbars)
                .into()
        };
        container(list)
            .padding(design::MANAGER_TABLE_PADDING)
            .style(panel)
            .width(Length::Fill)
            .height(if narrow { Length::Shrink } else { Length::Fill })
            .into()
    }
}
fn manager_columns<'a>(
    name: Element<'a, Message>,
    week: Element<'a, Message>,
    all: Element<'a, Message>,
    rules: Element<'a, Message>,
    last: Element<'a, Message>,
) -> iced::widget::Row<'a, Message> {
    row![
        container(name).width(Length::FillPortion(4)),
        container(week).width(Length::FillPortion(2)),
        container(all)
            .width(design::MANAGER_ALL)
            .align_x(Alignment::Center),
        container(rules)
            .width(design::MANAGER_RULE_COUNT)
            .align_x(Alignment::Center),
        container(last)
            .width(design::MANAGER_LAST)
            .align_x(Alignment::End)
    ]
    .spacing(design::CONTROL_GAP)
    .align_y(Alignment::Center)
    .padding([design::CONTROL_GAP as u16, 0])
    .height(Length::Shrink)
}
fn rule_columns<'a>(
    number: Element<'a, Message>,
    on: Element<'a, Message>,
    field: Element<'a, Message>,
    pattern: Element<'a, Message>,
    tag: Element<'a, Message>,
    duration: Element<'a, Message>,
) -> iced::widget::Row<'a, Message> {
    row![
        container(number).width(design::RULE_NUMBER),
        container(on).width(design::RULE_SWITCH_CELL),
        container(field).width(design::RULE_FIELD_CELL),
        container(pattern).width(Length::FillPortion(5)),
        container(tag).width(Length::FillPortion(3)),
        container(duration)
            .width(design::MANAGER_DURATION)
            .align_x(Alignment::End)
    ]
    .spacing(design::CONTROL_GAP)
    .align_y(Alignment::Center)
    .padding([design::CONTROL_GAP as u16, 0])
    .height(Length::Shrink)
}

#[derive(Default)]
struct ChartCache {
    geometry: widget::canvas::Cache,
    theme: Option<Theme>,
    values: [i64; 7],
    color: Option<Color>,
    language: Option<localization::Language>,
}
struct TagChart<'a> {
    values: [i64; 7],
    date: NaiveDate,
    color: Color,
    cache: &'a std::cell::RefCell<ChartCache>,
}
impl widget::canvas::Program<Message> for TagChart<'_> {
    type State = ();
    fn draw(
        &self,
        _: &(),
        renderer: &iced::Renderer,
        theme: &Theme,
        bounds: iced::Rectangle,
        _: iced::mouse::Cursor,
    ) -> Vec<widget::canvas::Geometry> {
        let mut cache = self.cache.borrow_mut();
        let language = localization::current();
        if cache.theme.as_ref() != Some(theme)
            || cache.values != self.values
            || cache.color != Some(self.color)
            || cache.language != Some(language)
        {
            cache.geometry.clear();
            cache.theme = Some(theme.clone());
            cache.values = self.values;
            cache.color = Some(self.color);
            cache.language = Some(language);
        }
        let p = design::Palette::new(theme.palette().background.r < 0.5);
        vec![cache.geometry.draw(renderer, bounds.size(), |frame| {
            let max = self.values.iter().copied().max().unwrap_or(1).max(1) as f32;
            let width = bounds.width / 7.0;
            for (i, value) in self.values.iter().enumerate() {
                let height = (*value as f32 / max * (bounds.height - design::CHART_AXIS))
                    .max(design::HAIRLINE);
                frame.fill_rectangle(
                    iced::Point::new(
                        i as f32 * width,
                        bounds.height - design::CHART_AXIS - height,
                    ),
                    iced::Size::new((width - design::SMALL_GAP as f32).max(0.0), height),
                    if *value == 0 {
                        p.line
                    } else if i == 6 {
                        p.accent
                    } else {
                        self.color
                    },
                );
                let date = self
                    .date
                    .checked_sub_days(Days::new((6 - i) as u64))
                    .unwrap_or(self.date);
                let name = tr([
                    "Monday",
                    "Tuesday",
                    "Wednesday",
                    "Thursday",
                    "Friday",
                    "Saturday",
                    "Sunday",
                ][date.weekday().num_days_from_monday() as usize]);
                frame.fill_text(widget::canvas::Text {
                    content: name.chars().take(1).collect(),
                    position: iced::Point::new(
                        (i as f32 + 0.5) * width,
                        bounds.height - design::CAPTION as f32,
                    ),
                    size: (design::CAPTION as f32).into(),
                    font: design::MONO,
                    color: if i == 6 { p.accent_text } else { p.text3 },
                    align_x: iced::alignment::Horizontal::Center.into(),
                    ..Default::default()
                });
            }
        })]
    }
}
impl State {
    pub(super) fn management_panel(&self) -> Element<'_, Message> {
        if self.screen == "Rules" {
            return self.rule_panel();
        }
        let p = palette(self);
        let ui = &self.management;
        let Some(draft) = &ui.tag else {
            return column![
                text(tr("Select a tag"))
                    .font(design::SEMIBOLD)
                    .size(design::HEADING),
                space::vertical(),
                text(tr("Stored on this computer only"))
                    .size(design::LABEL)
                    .color(p.text3)
            ]
            .height(Length::Fill)
            .into();
        };
        let id = draft.id;
        let tags = ui.data.as_ref().map(|d| d.tags.as_slice()).unwrap_or(&[]);
        let parent = draft
            .parent
            .map(|id| model::tag_path(tags, id))
            .unwrap_or_default();
        let color = id.map(|id| tag_color(tags, id)).unwrap_or(p.accent);
        let mut body = column![
            column![
                text(if parent.is_empty() {
                    tr("Tags")
                } else {
                    format!("{parent} ›")
                })
                .size(design::LABEL)
                .color(p.text3),
                row![
                    dot(color),
                    text(if id.is_some() {
                        draft.name.clone()
                    } else {
                        tr("New tag")
                    })
                    .font(design::SEMIBOLD)
                    .size(design::HEADING)
                    .width(Length::Fill)
                    .wrapping(widget::text::Wrapping::WordOrGlyph)
                ]
                .spacing(design::CONTROL_GAP)
                .align_y(Alignment::Center)
            ]
            .spacing(design::SMALL_GAP),
            text(tr("Name")).size(design::LABEL).color(p.text3),
            text_input(&tr("Client, Project, Task"), &draft.name)
                .id("manager-tag-name")
                .on_input(|s| Message::Manage(Edit::TagName(s)))
                .on_submit(Message::Manage(Edit::SaveTag))
        ]
        .spacing(design::PANEL_GAP);
        if let Some(id) = id {
            let choices: Vec<_> = std::iter::once(Choice {
                id: None,
                name: tr("No parent"),
            })
            .chain(
                tags.iter()
                    .filter(|t| !descendant(tags, t.id, id))
                    .map(|t| Choice {
                        id: Some(t.id),
                        name: model::tag_path(tags, t.id),
                    }),
            )
            .collect();
            let selected = choices.iter().find(|c| c.id == draft.parent).cloned();
            body = body
                .push(text(tr("Parent")).size(design::LABEL).color(p.text3))
                .push(
                    pick_list(choices, selected, |c| {
                        Message::Manage(Edit::TagParent(c.id))
                    })
                    .width(Length::Fill)
                    .text_size(design::BODY)
                    .padding([design::FIELD_VERTICAL, design::FIELD_HORIZONTAL])
                    .style(design::pick_list),
                );
            let mut colors = row![].spacing(design::SMALL_GAP);
            for rgb in design::CATEGORICAL {
                let value = format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]);
                let selected = value.eq_ignore_ascii_case(&draft.color);
                let fill = Color::from_rgb8(rgb[0], rgb[1], rgb[2]);
                colors = colors.push(tooltip(
                    button(space::horizontal())
                        .on_press(Message::Manage(Edit::TagColor(value.clone())))
                        .width(design::DENSE_TARGET)
                        .height(design::DENSE_TARGET)
                        .padding(0)
                        .style(move |_, _| widget::button::Style {
                            background: Some(fill.into()),
                            border: iced::Border {
                                width: if selected { design::FOCUS_WIDTH } else { 0.0 },
                                color: p.text,
                                ..Default::default()
                            },
                            ..Default::default()
                        }),
                    text(value).font(design::MONO).size(design::LABEL),
                    tooltip::Position::Bottom,
                ));
            }
            body = body.push(
                column![
                    row![
                        text(tr("Color")).size(design::LABEL).color(p.text3),
                        space::horizontal(),
                        tooltip(
                            button(text("…"))
                                .on_press(Message::Manage(Edit::CustomColor))
                                .style(design::ghost)
                                .padding(0)
                                .height(Length::Shrink),
                            text(tr("#RRGGBB (optional)")),
                            tooltip::Position::Bottom
                        )
                    ]
                    .align_y(Alignment::Center),
                    colors.wrap()
                ]
                .spacing(0),
            );
            if ui.custom_color {
                body = body.push(
                    text_input(&tr("#RRGGBB (optional)"), &draft.color)
                        .on_input(|s| Message::Manage(Edit::TagColor(s)))
                        .font(design::MONO),
                );
            }
            if let Some(data) = &ui.data {
                let values = data.chart.get(&id).copied().unwrap_or([0; 7]);
                let chart = column![
                    row![
                        caption("Last 7 days", p),
                        space::horizontal(),
                        text(model::duration(values.iter().sum()))
                            .font(design::MONO)
                            .size(design::META)
                    ]
                    .align_y(Alignment::Center),
                    widget::canvas::Canvas::new(TagChart {
                        values,
                        date: data.date,
                        color,
                        cache: &ui.chart_cache
                    })
                    .width(Length::Fill)
                    .height(design::TAG_CHART_HEIGHT)
                ]
                .spacing(design::CONTROL_GAP);
                body = body.push(
                    container(chart)
                        .padding(design::MANAGER_TABLE_PADDING)
                        .style(panel),
                );
                let linked: Vec<_> = data
                    .rules
                    .iter()
                    .filter(|r| descendant(tags, r.tag_id, id))
                    .collect();
                body = body.push(
                    row![text(
                        localized(
                            "Auto-tag rules · {count}",
                            &[("count", linked.len().to_string())]
                        )
                        .to_uppercase()
                    )
                    .size(design::CAPTION)
                    .color(p.text3)]
                    .width(Length::Fill),
                );
                for rule in linked {
                    body = body.push(
                        button(
                            row![
                                text(rule_field(&rule.field))
                                    .size(design::CAPTION)
                                    .color(p.text3),
                                text(rule.pattern.clone())
                                    .font(design::MONO)
                                    .size(design::LABEL)
                                    .color(p.text2)
                                    .width(Length::Fill)
                                    .wrapping(widget::text::Wrapping::WordOrGlyph)
                            ]
                            .spacing(design::CONTROL_GAP),
                        )
                        .on_press(Message::Manage(Edit::OpenRule(rule.id)))
                        .height(Length::Shrink)
                        .width(Length::Fill)
                        .style(design::secondary),
                    );
                }
                if let Some(rule) = data.rules.iter().find(|r| descendant(tags, r.tag_id, id)) {
                    body = body.push(
                        button(text(tr("Edit in Rules →")).size(design::LABEL))
                            .on_press(Message::Manage(Edit::OpenRule(rule.id)))
                            .style(design::link)
                            .padding(0),
                    );
                }
            }
        } else {
            body = body.push(
                text(tr("Use commas to create a tag branch."))
                    .size(design::LABEL)
                    .color(p.text3),
            );
        }
        let mut footer = column![].spacing(design::CONTROL_GAP);
        if !ui.error.is_empty() {
            footer = footer.push(
                text(localization::error(&ui.error))
                    .size(design::LABEL)
                    .color(p.danger_text),
            );
        }
        if draft.dirty {
            footer = footer.push(
                row![
                    button(text(tr("Save")).font(design::SEMIBOLD))
                        .on_press_maybe(
                            (!draft.name.trim().is_empty())
                                .then_some(Message::Manage(Edit::SaveTag))
                        )
                        .style(design::primary)
                        .width(Length::Fill),
                    button(text(tr("Cancel")))
                        .on_press(Message::Manage(Edit::CancelTag))
                        .width(Length::Fill)
                ]
                .spacing(design::CONTROL_GAP),
            );
        }
        if let Some(id) = id {
            footer = footer.push(
                row![
                    button(text(tr("Merge into…")).size(design::META))
                        .on_press(Message::Manage(Edit::Merge))
                        .width(Length::Fill),
                    button(
                        text(tr(if draft.archived {
                            "Unarchive"
                        } else {
                            "Archive"
                        }))
                        .size(design::META)
                    )
                    .on_press(Message::Manage(Edit::ArchiveTag))
                    .width(Length::Fill)
                ]
                .spacing(design::CONTROL_GAP),
            );
            if draft.merge {
                let choices: Vec<_> = tags
                    .iter()
                    .filter(|t| !descendant(tags, t.id, id))
                    .map(|t| Choice {
                        id: Some(t.id),
                        name: model::tag_path(tags, t.id),
                    })
                    .collect();
                let selected = choices.iter().find(|c| c.id == draft.merge_target).cloned();
                footer = footer
                    .push(
                        pick_list(choices, selected, |c| {
                            Message::Manage(Edit::MergeTarget(c.id.unwrap_or_default()))
                        })
                        .placeholder(tr("Choose a tag"))
                        .text_size(design::BODY)
                        .width(Length::Fill)
                        .padding([design::FIELD_VERTICAL, design::FIELD_HORIZONTAL])
                        .style(design::pick_list),
                    )
                    .push(
                        button(text(tr("Confirm merge")))
                            .on_press_maybe(
                                draft
                                    .merge_target
                                    .map(|_| Message::Manage(Edit::ConfirmMerge)),
                            )
                            .width(Length::Fill),
                    );
            }
            let duration = ui
                .data
                .as_ref()
                .and_then(|d| d.usage.get(&id))
                .map(|s| s.all_ms)
                .unwrap_or(0);
            footer = footer.push(
                button(
                    text(localized(
                        "Delete tag · removes {duration} of tagging",
                        &[("duration", model::duration(duration))],
                    ))
                    .size(design::META),
                )
                .on_press(Message::Manage(Edit::DeleteTag))
                .style(design::destructive)
                .height(Length::Shrink)
                .width(Length::Fill),
            );
            if let Some((count, ms)) = draft.delete_usage {
                footer = footer
                    .push(
                        text(localized(
                            "Delete this subtree, {count} ranges ({duration}) and its rules?",
                            &[
                                ("count", count.to_string()),
                                ("duration", model::duration(ms)),
                            ],
                        ))
                        .size(design::LABEL),
                    )
                    .push(
                        row![
                            button(text(tr("Confirm delete")))
                                .on_press(Message::Manage(Edit::ConfirmDelete))
                                .style(design::destructive),
                            button(text(tr("Cancel"))).on_press(Message::Manage(Edit::CancelTag))
                        ]
                        .spacing(design::CONTROL_GAP),
                    );
            }
        }
        footer = footer.push(
            text(tr("Stored on this computer only"))
                .size(design::LABEL)
                .color(p.text3),
        );
        column![
            scrollable(body)
                .spacing(design::CONTROL_GAP as f32)
                .style(design::scrollbars)
                .height(Length::Fill),
            footer
        ]
        .spacing(design::GAP)
        .height(Length::Fill)
        .into()
    }
    fn rule_panel(&self) -> Element<'_, Message> {
        let p = palette(self);
        let e = &self.rule_editor;
        let ui = &self.management;
        if e.field.is_empty() {
            return column![
                text(tr("Rule editor"))
                    .font(design::SEMIBOLD)
                    .size(design::HEADING),
                text(tr("Select a rule or create a new one."))
                    .size(design::BODY)
                    .color(p.text2),
                space::vertical(),
                text(tr("Stored on this computer only"))
                    .size(design::LABEL)
                    .color(p.text3)
            ]
            .spacing(design::GAP)
            .height(Length::Fill)
            .into();
        }
        let number = ui
            .data
            .as_ref()
            .and_then(|d| d.rules.iter().position(|r| Some(r.id) == e.id))
            .map(|i| i + 1);
        let title = number
            .map(|n| localized("Edit rule {number}", &[("number", n.to_string())]))
            .unwrap_or_else(|| tr("New rule"));
        let mut fields = row![]
            .spacing(design::SUMMARY_GAP)
            .align_y(Alignment::Center);
        for (field, label) in [
            ("app", "App"),
            ("title", "Title"),
            ("url", "URL"),
            ("any", "Any"),
        ] {
            fields = fields.push(tooltip(
                button(text(tr(label)).size(design::META))
                    .on_press_maybe(
                        (field != "any").then_some(Message::Rule(rules::Edit::Field(field.into()))),
                    )
                    .style(design::segment(e.field == field))
                    .width(Length::Fill)
                    .height(Length::Shrink)
                    .padding(design::SMALL_GAP as u16),
                text(tr(if field == "any" {
                    "Any-field rules are not available yet."
                } else {
                    "Match in"
                }))
                .size(design::LABEL),
                tooltip::Position::Bottom,
            ));
        }
        let invalid = e.regex_error.is_some();
        let mut body = column![
            row![
                text(title)
                    .font(design::SEMIBOLD)
                    .size(design::HEADING)
                    .width(Length::Fill),
                text(if e.modified {
                    tr("unsaved")
                } else {
                    String::new()
                })
                .size(design::LABEL)
                .color(p.text3)
            ]
            .align_y(Alignment::Center),
            text(tr("Match in")).size(design::LABEL).color(p.text3),
            container(fields)
                .padding(design::SEGMENT_PADDING)
                .style(design::segment_track),
            text(tr("Pattern (regular expression)"))
                .size(design::LABEL)
                .color(p.text3),
            text_input(&tr("Regular expression"), &e.pattern)
                .font(design::MONO)
                .on_input(|s| Message::Rule(rules::Edit::Pattern(s)))
                .style(move |theme, status| {
                    let mut style = design::field(theme, status);
                    if invalid {
                        style.border.color = p.danger_text;
                    }
                    style
                }),
            row![
                switch(
                    e.ignore_case,
                    Message::Rule(rules::Edit::IgnoreCase(!e.ignore_case)),
                    p
                ),
                text(tr("Ignore case")).size(design::META).color(p.text2)
            ]
            .spacing(design::CONTROL_GAP)
            .align_y(Alignment::Center)
        ]
        .spacing(design::PANEL_GAP);
        if invalid {
            body = body.push(wave(p));
        }
        let choices: Vec<_> = ui
            .data
            .as_ref()
            .map(|d| {
                d.tags
                    .iter()
                    .filter(|t| t.archived == 0)
                    .map(|t| Choice {
                        id: Some(t.id),
                        name: model::tag_path(&d.tags, t.id),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let selected = choices.iter().find(|c| c.id == e.tag_id).cloned();
        body = body
            .push(text(tr("Tag name")).size(design::LABEL).color(p.text3))
            .push(
                pick_list(choices, selected, |c| {
                    Message::Rule(rules::Edit::Tag(c.id.unwrap_or_default()))
                })
                .placeholder(tr("Choose a tag"))
                .width(Length::Fill)
                .text_size(design::BODY)
                .padding([design::FIELD_VERTICAL, design::FIELD_HORIZONTAL])
                .style(design::pick_list),
            );
        let mut preview = column![
            row![
                caption("Live preview · 7 days", p),
                space::horizontal(),
                text(model::duration(e.live_ms))
                    .size(design::SUMMARY_VALUE)
                    .font(design::MONO)
                    .color(p.accent_text)
            ]
            .align_y(Alignment::Center),
            text(localized(
                "would be tagged · {duration} already tagged",
                &[("duration", model::duration(e.coverage.covered_ms))]
            ))
            .size(design::LABEL)
            .color(p.text3)
        ]
        .spacing(design::CONTROL_GAP);
        if let Some(error) = &e.regex_error {
            preview = preview.push(
                text(localization::error(error))
                    .size(design::LABEL)
                    .color(p.danger_text),
            );
        }
        for sample in e.coverage.samples.iter().take(3) {
            preview = preview.push(divider(p)).push(
                row![
                    match_text(&e.effective_pattern(), &sample.sample, p),
                    text(model::duration(sample.end_ts - sample.start_ts))
                        .font(design::MONO)
                        .size(design::LABEL)
                        .color(p.text2)
                ]
                .spacing(design::CONTROL_GAP)
                .align_y(Alignment::Center),
            );
        }
        if e.coverage.samples.len() > 3 {
            preview = preview.push(
                text(localized(
                    "+ {count} more samples",
                    &[("count", (e.coverage.samples.len() - 3).to_string())],
                ))
                .size(design::LABEL)
                .color(p.text3),
            );
        }
        body = body.push(
            container(preview)
                .padding(design::MANAGER_TABLE_PADDING)
                .style(panel),
        );
        if !e.error.is_empty() && e.regex_error.is_none() {
            body = body.push(
                text(localization::error(&e.error))
                    .size(design::LABEL)
                    .color(p.danger_text),
            );
        }
        if let Some(id) = e.id {
            body = body.push(
                row![
                    tooltip(
                        button(text("↑"))
                            .on_press(Message::Manage(Edit::MoveRule(id, -1)))
                            .style(design::ghost)
                            .width(design::DENSE_TARGET),
                        text(tr("Move rule up")),
                        tooltip::Position::Bottom
                    ),
                    tooltip(
                        button(text("↓"))
                            .on_press(Message::Manage(Edit::MoveRule(id, 1)))
                            .style(design::ghost)
                            .width(design::DENSE_TARGET),
                        text(tr("Move rule down")),
                        tooltip::Position::Bottom
                    ),
                    space::horizontal(),
                    button(text(tr("Delete rule")).size(design::LABEL))
                        .on_press(Message::Rule(rules::Edit::Delete))
                        .style(design::destructive)
                ]
                .spacing(design::CONTROL_GAP),
            );
        }
        let mut footer = column![].spacing(design::CONTROL_GAP);
        if !ui.error.is_empty() {
            footer = footer.push(
                text(localization::error(&ui.error))
                    .size(design::LABEL)
                    .color(p.danger_text),
            );
        }
        if e.confirm {
            let total = e.preview.iter().map(|r| r.end_ts - r.start_ts).sum();
            footer=footer.push(text(localized("Apply {duration} of automatic tags? Existing tags stay unchanged. You can undo this action.",&[("duration",model::duration(total))])).size(design::LABEL)).push(row![button(text(tr("Cancel"))).on_press(Message::Rule(rules::Edit::Cancel)),button(text(tr("Confirm application"))).on_press(Message::Rule(rules::Edit::Confirm)).style(design::primary)].spacing(design::CONTROL_GAP));
        } else {
            footer = footer
                .push(
                    row![
                        switch(
                            e.apply_recent,
                            Message::Rule(rules::Edit::ApplyRecent(!e.apply_recent)),
                            p
                        ),
                        text(tr("Also apply to the last 30 days (can be undone)"))
                            .size(design::META)
                            .color(p.text2)
                            .width(Length::Fill)
                    ]
                    .spacing(design::CONTROL_GAP)
                    .align_y(Alignment::Center),
                )
                .push(
                    row![
                        button(text(tr("Cancel"))).on_press(Message::Rule(rules::Edit::Discard)),
                        button(text(tr("Save rule")).font(design::SEMIBOLD))
                            .on_press_maybe(
                                (!e.pattern.trim().is_empty() && e.tag_id.is_some() && !invalid)
                                    .then_some(Message::Rule(rules::Edit::Save))
                            )
                            .style(design::primary)
                            .width(Length::Fill)
                    ]
                    .spacing(design::CONTROL_GAP),
                );
        }
        footer = footer.push(
            text(tr("Stored on this computer only"))
                .size(design::LABEL)
                .color(p.text3),
        );
        column![
            scrollable(body)
                .spacing(design::CONTROL_GAP as f32)
                .style(design::scrollbars)
                .height(Length::Fill),
            footer
        ]
        .spacing(design::GAP)
        .height(Length::Fill)
        .into()
    }
}
fn match_text(pattern: &str, value: &str, p: design::Palette) -> Element<'static, Message> {
    let mut spans: Vec<iced::advanced::text::Span<'static, ()>> = Vec::new();
    let mut end = 0;
    if let Ok(matches) = draugr_time_core::rule_match_ranges(pattern, value) {
        for (start, stop) in matches {
            if start > end {
                spans.push(widget::span(value[end..start].to_owned()).color(p.text2));
            }
            spans.push(
                widget::span(value[start..stop].to_owned())
                    .color(p.accent_text)
                    .background(p.accent_dim),
            );
            end = stop;
        }
    }
    if end < value.len() {
        spans.push(widget::span(value[end..].to_owned()).color(p.text2));
    }
    widget::rich_text(spans)
        .font(design::MONO)
        .size(design::LABEL)
        .width(Length::Fill)
        .wrapping(widget::text::Wrapping::WordOrGlyph)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> (State, draugr_time_daemon::Shared) {
        let (mut state, _) = State::boot();
        state.fixture = true;
        state.settings_ui.preferences.onboarded = true;
        state.status.state = "tracking".into();
        state.date = NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();
        state.screen = "Tags";
        let shared = draugr_time_daemon::Shared::new(
            draugr_time_core::Storage::open_in_memory().unwrap(),
            "test".into(),
            300,
        );
        {
            let mut db = shared.storage.lock().unwrap();
            for (path, color) in [
                ("Work", "#69B6D5"),
                ("Work, Research", "#B692D0"),
                ("Work, Planning", "#D3AE60"),
                ("Personal", "#7EB889"),
                ("Personal, Reading", "#CB8A72"),
            ] {
                let (id, _) = db.create_tag_path(path).unwrap();
                db.update_tag(id, path.rsplit(", ").next().unwrap(), Some(color), false)
                    .unwrap();
            }
            for (field, pattern, tag_id) in [
                ("app", "^editor$", 2),
                ("title", "Planning", 3),
                ("app", "^browser$", 5),
            ] {
                db.put(Collection::Rules,None,json!({"field":field,"pattern":pattern,"tag_id":tag_id,"priority":0,"enabled":true})).unwrap();
            }
            for day in (0..7).rev() {
                let start = model::day_bounds(state.date.checked_sub_days(Days::new(day)).unwrap())
                    .start
                    + 9 * 3_600_000;
                let stop = start + (day as i64 + 1) * 600_000;
                db.record_interval(
                    "window",
                    &json!({"app":"editor","title":"Research notes"}),
                    start,
                    stop,
                )
                .unwrap();
                db.record_interval(
                    "computer",
                    &json!({"machine":"awake","activity":"active","recording":"recording"}),
                    start,
                    stop + 600_000,
                )
                .unwrap();
                db.put(
                    Collection::TagRanges,
                    None,
                    json!({"tag_id":2,"start_ts":start,"end_ts":stop-300_000}),
                )
                .unwrap();
                db.record_interval(
                    "window",
                    &json!({"app":"browser","title":"Reading"}),
                    stop,
                    stop + 600_000,
                )
                .unwrap();
            }
        }
        state.runtime = Some(Runtime::for_test(shared.clone()));
        let data = load(shared.clone(), state.date).unwrap();
        state.tags = data.tags.clone();
        state.management.data = Some(data);
        let _ = state.edit_management(Edit::SelectTag(2));
        (state, shared)
    }
    #[test]
    fn tree_search_restores_ancestors_and_ignores_collapse() {
        let (state, _) = fixture();
        let tags = &state.tags;
        let collapsed = HashSet::from([1]);
        assert!(!tree_rows(tags, &collapsed, "")
            .iter()
            .any(|(t, _)| t.id == 2));
        let rows = tree_rows(tags, &collapsed, "research");
        assert_eq!(
            rows.iter().map(|(t, _)| t.id).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert!(descendant(tags, 2, 1));
        assert!(!descendant(tags, 1, 2));
    }
    #[test]
    fn stale_load_cannot_restore_hidden_screen_and_export_preserves_fields() {
        let (mut state, shared) = fixture();
        let data = load(shared, state.date).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&rules_json(&data)).unwrap();
        assert_eq!(json["rules"][0]["pattern"], "^editor$");
        state.set_screen("Day");
        let _ = state.edit_management(Edit::Loaded(0, Ok(Box::new(data))));
        assert!(state.management.data.is_none());
    }
    #[test]
    fn tag_delete_requires_confirmation_of_current_usage() {
        let (mut state, shared) = fixture();
        let _ = state.edit_management(Edit::DeleteTag);
        let before = state.management.tag.as_ref().unwrap().delete_usage;
        shared
            .storage
            .lock()
            .unwrap()
            .put(
                Collection::TagRanges,
                None,
                json!({"tag_id":2,"start_ts":1,"end_ts":100}),
            )
            .unwrap();
        let _ = state.edit_management(Edit::ConfirmDelete);
        assert_ne!(state.management.tag.as_ref().unwrap().delete_usage, before);
        assert!(shared
            .storage
            .lock()
            .unwrap()
            .tags()
            .unwrap()
            .iter()
            .any(|t| t.id == 2));
        let _ = state.edit_management(Edit::ConfirmDelete);
        assert!(!shared
            .storage
            .lock()
            .unwrap()
            .tags()
            .unwrap()
            .iter()
            .any(|t| t.id == 2));
        let _ = state.edit(tagging::Edit::Undo);
        assert!(shared
            .storage
            .lock()
            .unwrap()
            .tags()
            .unwrap()
            .iter()
            .any(|t| t.id == 2));
    }
    #[test]
    fn rule_editor_round_trips_case_discards_drafts_and_reorders() {
        let (mut state, shared) = fixture();
        state.screen = "Rules";
        let _ = state.edit_management(Edit::ToggleRule(1, false));
        assert!(!shared.storage.lock().unwrap().rules().unwrap()[0].enabled);
        let _ = state.edit_management(Edit::ToggleRule(1, true));
        state.edit_rule(rules::Edit::Select(1));
        state.edit_rule(rules::Edit::IgnoreCase(true));
        state.edit_rule(rules::Edit::Save);
        state.edit_rule(rules::Edit::Select(1));
        assert!(state.rule_editor.ignore_case);
        assert_eq!(state.rule_editor.pattern, "^editor$");
        state.edit_rule(rules::Edit::Pattern("[broken".into()));
        state.edit_rule(rules::Edit::Save);
        assert!(state.rule_editor.regex_error.is_some());
        assert_eq!(
            shared.storage.lock().unwrap().rules().unwrap()[0].pattern,
            "(?i:^editor$)"
        );
        state.edit_rule(rules::Edit::Discard);
        assert_eq!(state.rule_editor.pattern, "^editor$");
        let _ = state.edit_management(Edit::MoveRule(3, -1));
        assert_eq!(
            shared
                .storage
                .lock()
                .unwrap()
                .rules()
                .unwrap()
                .iter()
                .map(|r| r.id)
                .collect::<Vec<_>>(),
            vec![1, 3, 2]
        );
        shared
            .storage
            .lock()
            .unwrap()
            .put(
                Collection::Rules,
                Some(1),
                json!({"field":"app","pattern":"(?i:foo)|(?:bar)","tag_id":2,"priority":3,"enabled":true}),
            )
            .unwrap();
        state.edit_rule(rules::Edit::Select(1));
        assert!(!state.rule_editor.ignore_case);
        assert_eq!(state.rule_editor.effective_pattern(), "(?i:foo)|(?:bar)");
    }
    #[test]
    fn closing_drops_management_caches_but_keeps_unsaved_rule_fields() {
        let (mut state, _) = fixture();
        state.screen = "Rules";
        state.edit_rule(rules::Edit::Select(1));
        state.edit_rule(rules::Edit::Pattern("editor.*".into()));
        assert!(!state.rule_editor.coverage.samples.is_empty());
        state.release_day();
        assert!(state.management.data.is_none());
        assert!(state.rule_editor.coverage.samples.is_empty());
        assert!(state.rule_editor.preview.is_empty());
        assert_eq!(state.rule_editor.pattern, "editor.*");
        assert!(state.rule_editor.modified);
    }
    #[test]
    fn new_branch_editor_is_canonical_before_background_refresh() {
        let (mut state, shared) = fixture();
        let _ = state.edit_management(Edit::NewTag);
        let _ = state.edit_management(Edit::TagName("Work, New branch".into()));
        let _ = state.edit_management(Edit::SaveTag);
        let draft = state.management.tag.as_ref().unwrap();
        assert_eq!(draft.name, "New branch");
        assert_eq!(draft.parent, Some(1));
        let id = draft.id.unwrap();
        let _ = state.edit_management(Edit::ArchiveTag);
        let saved = shared
            .storage
            .lock()
            .unwrap()
            .tags()
            .unwrap()
            .into_iter()
            .find(|t| t.id == id)
            .unwrap();
        assert_eq!(saved.parent_id, Some(1));
        assert_eq!(saved.name, "New branch");
        assert_eq!(saved.archived, 1);
    }
    #[test]
    fn background_refresh_preserves_a_failed_history_confirmation() {
        let (mut state, shared) = fixture();
        state.screen = "Rules";
        state.edit_rule(rules::Edit::Select(1));
        state.edit_rule(rules::Edit::ApplyRecent(true));
        state.edit_rule(rules::Edit::Save);
        let matched = state.rule_editor.preview[0].clone();
        shared
            .storage
            .lock()
            .unwrap()
            .tag_interval("Work", "", matched.start_ts, matched.end_ts, false)
            .unwrap();
        state.edit_rule(rules::Edit::Confirm);
        let error = state.rule_editor.error.clone();
        assert!(!error.is_empty());
        let data = load(shared, state.date).unwrap();
        let _ = state.edit_management(Edit::Loaded(
            state.management.generation,
            Ok(Box::new(data)),
        ));
        assert_eq!(state.rule_editor.error, error);
    }
    #[test]
    #[ignore = "writes Tags and Rules review screenshots; run explicitly with --test-threads=1"]
    fn render_management_review_screenshots() {
        use iced::advanced::{
            layout, mouse, renderer, renderer::Headless as _, widget::Tree, Layout, Renderer as _,
        };
        use iced::{Rectangle, Renderer, Size};
        use std::borrow::Cow;
        {
            let mut fonts = iced::advanced::graphics::text::font_system()
                .write()
                .unwrap();
            fonts.load_font(Cow::Borrowed(include_bytes!("../../fonts/InterTight.ttf")));
            fonts.load_font(Cow::Borrowed(include_bytes!(
                "../../fonts/JetBrainsMono-Regular.ttf"
            )));
        }
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/screenshots");
        std::fs::create_dir_all(&path).unwrap();
        let (mut state, _) = fixture();
        for screen in ["Tags", "Rules"] {
            state.screen = screen;
            if screen == "Rules" {
                state.edit_rule(rules::Edit::Select(1));
            }
            for language in [
                localization::Language::En,
                localization::Language::Ru,
                localization::Language::De,
            ] {
                localization::set(language);
                for dark in [true, false] {
                    for (width, height) in [(1280, 820), (700, 820), (700, 1600)] {
                        localization::set(language);
                        state.theme = native_theme(dark);
                        let size = Size::new(width as f32, height as f32);
                        let viewport = Rectangle::with_size(size);
                        let mut renderer = Renderer::new(design::UI, 13.0.into());
                        let mut view = state.view_sized(size.width);
                        let mut tree = Tree::new(view.as_widget());
                        let node = view.as_widget_mut().layout(
                            &mut tree,
                            &renderer,
                            &layout::Limits::new(Size::ZERO, size),
                        );
                        renderer.reset(viewport);
                        view.as_widget().draw(
                            &tree,
                            &mut renderer,
                            &state.theme,
                            &renderer::Style {
                                text_color: state.theme.palette().text,
                            },
                            Layout::new(&node),
                            mouse::Cursor::Unavailable,
                            &viewport,
                        );
                        let rgba = renderer.screenshot(
                            iced::Size::new(width, height),
                            1.0,
                            state.theme.palette().background,
                        );
                        let file = std::fs::File::create(path.join(format!(
                            "m12-{}-{}-{}-{width}x{height}.png",
                            screen.to_lowercase(),
                            language.code(),
                            if dark { "dark" } else { "light" }
                        )))
                        .unwrap();
                        let mut encoder = png::Encoder::new(file, width, height);
                        encoder.set_color(png::ColorType::Rgba);
                        encoder.set_depth(png::BitDepth::Eight);
                        encoder
                            .write_header()
                            .unwrap()
                            .write_image_data(&rgba)
                            .unwrap();
                        eprintln!(
                            "{} {} {width}×{height}: canvas {:.2}ms",
                            language.code(),
                            if dark { "dark" } else { "light" },
                            state.stats.max_build_us.load(Ordering::Relaxed) as f64 / 1000.0
                        );
                        assert!(state.stats.max_build_us.load(Ordering::Relaxed) < 100_000);
                    }
                }
            }
        }
        localization::set(localization::Language::En);
    }
}
