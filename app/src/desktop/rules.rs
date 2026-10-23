use super::*;
use draugr_time_core::{Collection, Rule, RuleMatch};
use serde_json::json;

#[derive(Default)]
pub struct Editor {
    pub id: Option<i64>,
    pub field: String,
    pub pattern: String,
    pub tag_id: Option<i64>,
    pub priority: String,
    pub enabled: bool,
    pub from: String,
    pub to: String,
    pub preview: Vec<RuleMatch>,
    pub error: String,
    pub confirm: bool,
    pub modified: bool,
    pub ignore_case: bool,
    pub apply_recent: bool,
    pub regex_error: Option<String>,
    pub coverage: draugr_time_core::RuleCoverage,
    pub live_ms: i64,
}
#[derive(Clone, Debug)]
pub enum Edit {
    New,
    Select(i64),
    Field(String),
    Pattern(String),
    Tag(i64),
    Save,
    Delete,
    Apply,
    Confirm,
    Cancel,
    Discard,
    IgnoreCase(bool),
    ApplyRecent(bool),
    RefreshPreview,
}
impl Editor {
    pub(super) fn effective_pattern(&self) -> String {
        if self.ignore_case {
            format!("(?i:{})", self.pattern)
        } else {
            self.pattern.clone()
        }
    }
    fn draft(&self) -> Result<Rule, String> {
        Ok(Rule {
            id: self.id.unwrap_or(0),
            field: self.field.clone(),
            pattern: self.effective_pattern(),
            tag_id: self.tag_id.ok_or_else(|| tr("Choose a tag"))?,
            priority: self
                .priority
                .parse()
                .map_err(|_| tr("Priority must be an integer"))?,
            enabled: self.enabled,
        })
    }
    fn bounds(&self) -> Result<Range, String> {
        let parse = |s: &str| {
            NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| tr("Use YYYY-MM-DD dates"))
        };
        let range = Range::new(
            model::day_bounds(parse(&self.from)?).start,
            model::day_bounds(parse(&self.to)?).end,
        );
        if range.start < 0 || range.end <= range.start {
            return Err(tr("Choose a valid date range"));
        }
        Ok(range)
    }
}
impl State {
    pub fn edit_rule(&mut self, event: Edit) {
        let Some(runtime) = &self.runtime else {
            self.error = tr("Start recording to edit rules");
            return;
        };
        let date = self
            .management
            .data
            .as_ref()
            .map(|d| d.date)
            .unwrap_or(self.date);
        let mut db = runtime.shared.storage.lock().unwrap();
        let e = &mut self.rule_editor;
        let confirmed = e.confirm;
        e.confirm = false;
        if matches!(
            event,
            Edit::Field(_) | Edit::Pattern(_) | Edit::Tag(_) | Edit::IgnoreCase(_)
        ) {
            e.modified = true;
        }
        match event {
            Edit::New => {
                *e = Editor {
                    field: "app".into(),
                    priority: db
                        .rules()
                        .unwrap_or_default()
                        .iter()
                        .map(|r| r.priority)
                        .max()
                        .unwrap_or(-1)
                        .saturating_add(1)
                        .to_string(),
                    modified: true,
                    enabled: true,
                    from: date
                        .checked_sub_days(Days::new(6))
                        .unwrap_or(date)
                        .to_string(),
                    to: date.to_string(),
                    ..Default::default()
                };
            }
            Edit::Select(id) => match db.rules() {
                Ok(rules) => {
                    if let Some(r) = rules.into_iter().find(|r| r.id == id) {
                        let (pattern, ignore_case) = if let Some(pattern) = r
                            .pattern
                            .strip_prefix("(?i:")
                            .and_then(|p| p.strip_suffix(')'))
                            .filter(|p| draugr_time_core::validate_rule_pattern(p).is_ok())
                        {
                            (pattern.to_owned(), true)
                        } else {
                            (r.pattern.clone(), false)
                        };
                        *e = Editor {
                            id: Some(id),
                            ignore_case,
                            field: r.field,
                            pattern,
                            tag_id: Some(r.tag_id),
                            priority: r.priority.to_string(),
                            enabled: r.enabled,
                            from: date
                                .checked_sub_days(Days::new(6))
                                .unwrap_or(date)
                                .to_string(),
                            to: date.to_string(),
                            ..Default::default()
                        };
                    }
                }
                Err(error) => e.error = error.to_string(),
            },
            Edit::IgnoreCase(value) => e.ignore_case = value,
            Edit::ApplyRecent(value) => {
                e.apply_recent = value;
                return;
            }
            Edit::Discard => {
                let id = e.id;
                *e = Editor::default();
                drop(db);
                if let Some(id) = id {
                    self.edit_rule(Edit::Select(id));
                }
                return;
            }
            Edit::Field(value) => e.field = value,
            Edit::Pattern(value) => e.pattern = value,
            Edit::Tag(id) => e.tag_id = Some(id),
            Edit::Save => {
                let result=e.draft().and_then(|r|db.put(Collection::Rules,e.id,json!({"field":r.field,"pattern":r.pattern,"tag_id":r.tag_id,"priority":r.priority,"enabled":r.enabled})).map_err(|e|e.to_string()));
                match result {
                    Ok(value) => {
                        e.id = value["id"].as_i64();
                        self.error.clear();
                        e.modified = false;
                        if e.apply_recent {
                            e.from = date
                                .checked_sub_days(Days::new(29))
                                .unwrap_or(date)
                                .to_string();
                            e.to = date.to_string();
                            drop(db);
                            self.edit_rule(Edit::Apply);
                            return;
                        }
                    }
                    Err(error) => {
                        e.error = error;
                        return;
                    }
                }
            }
            Edit::Delete => {
                if let Some(id) = e.id {
                    match db.delete(Collection::Rules, id) {
                        Ok(_) => *e = Editor::default(),
                        Err(error) => e.error = error.to_string(),
                    }
                }
                return;
            }
            Edit::Apply => {
                match e.bounds().and_then(|range| {
                    db.preview_rules(range.start, range.end, None)
                        .map_err(|e| e.to_string())
                }) {
                    Ok(preview) => {
                        e.preview = preview
                            .into_iter()
                            .filter(|r| Some(r.rule_id) == e.id)
                            .collect();
                        e.confirm = e.id.is_some();
                        e.error.clear();
                    }
                    Err(error) => e.error = error,
                }
                return;
            }
            Edit::Confirm => {
                if !confirmed {
                    return;
                }
                let result = e.bounds().and_then(|range| {
                    db.apply_rule_checked(range.start, range.end, e.id.unwrap_or(0), &e.preview)
                        .map_err(|e| e.to_string())
                });
                match result {
                    Ok((_, undo)) => {
                        if self.tag_undo.len() == 20 {
                            self.tag_undo.remove(0);
                        }
                        self.tag_undo.push(undo);
                        e.preview.clear();
                        e.error.clear();
                    }
                    Err(error) => e.error = error,
                }
                drop(db);
                self.invalidate_load();
                return;
            }
            Edit::Cancel => return,
            Edit::RefreshPreview => {}
        }
        e.regex_error = draugr_time_core::validate_rule_pattern(&e.effective_pattern())
            .err()
            .map(|e| e.to_string());
        let live_range = Range::new(
            model::day_bounds(date.checked_sub_days(Days::new(6)).unwrap_or(date)).start,
            model::day_bounds(date).end,
        );
        e.coverage = e
            .draft()
            .and_then(|draft| {
                db.rule_coverage(live_range.start, live_range.end, &draft)
                    .map_err(|e| e.to_string())
            })
            .unwrap_or_default();
        match e.draft().and_then(|draft| {
            db.preview_rules(live_range.start, live_range.end, Some(&draft))
                .map_err(|e| e.to_string())
        }) {
            Ok(preview) => {
                e.live_ms = preview
                    .iter()
                    .filter(|r| r.rule_id == e.id.unwrap_or(0))
                    .map(|r| r.end_ts - r.start_ts)
                    .sum();
                e.preview = preview;
                e.error.clear();
            }
            Err(error) => {
                e.preview.clear();
                e.live_ms = 0;
                e.error = error;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn draft_validates_priority_tag_and_dates() {
        let mut e = Editor {
            field: "app".into(),
            pattern: "editor".into(),
            priority: "x".into(),
            from: "2026-10-01".into(),
            to: "2026-10-07".into(),
            ..Default::default()
        };
        assert!(e.draft().is_err());
        e.tag_id = Some(1);
        e.priority = "2".into();
        assert_eq!(e.draft().unwrap().priority, 2);
        assert_eq!(e.bounds().unwrap().duration(), 7 * 86_400_000);
        e.to = "bad".into();
        assert!(e.bounds().is_err());
    }
}

#[cfg(test)]
mod workflow_tests {
    use super::*;
    #[test]
    fn editor_live_preview_confirm_apply_and_undo_match() {
        let (mut state, _) = State::boot();
        let shared = draugr_time_daemon::Shared::new(
            draugr_time_core::Storage::open_in_memory().unwrap(),
            "test".into(),
            300,
        );
        let start = state.day.start;
        {
            let mut db = shared.storage.lock().unwrap();
            db.put(Collection::Tags, None, json!({"name":"Work"}))
                .unwrap();
            db.record_interval(
                "window",
                &json!({"app":"editor","title":"Work"}),
                start,
                start + 120_000,
            )
            .unwrap();
            db.record_interval(
                "computer",
                &json!({"machine":"awake","activity":"active","recording":"recording"}),
                start,
                start + 120_000,
            )
            .unwrap();
        }
        state.runtime = Some(Runtime::for_test(shared.clone()));
        state.edit_rule(Edit::New);
        state.edit_rule(Edit::Tag(1));
        state.edit_rule(Edit::Pattern("^editor$".into()));
        assert_eq!(
            state
                .rule_editor
                .preview
                .iter()
                .map(|r| r.end_ts - r.start_ts)
                .sum::<i64>(),
            120_000
        );
        assert!(shared
            .storage
            .lock()
            .unwrap()
            .list(Collection::Rules)
            .unwrap()
            .is_empty());
        state.edit_rule(Edit::Save);
        state.edit_rule(Edit::Apply);
        assert!(state.rule_editor.confirm);
        assert!(shared
            .storage
            .lock()
            .unwrap()
            .tag_ranges(start, start + 200_000)
            .unwrap()
            .is_empty());
        state.edit_rule(Edit::Confirm);
        assert_eq!(
            shared
                .storage
                .lock()
                .unwrap()
                .tag_ranges(start, start + 200_000)
                .unwrap()[0]
                .end_ts,
            start + 120_000
        );
        let _ = state.edit(tagging::Edit::Undo);
        assert!(shared
            .storage
            .lock()
            .unwrap()
            .tag_ranges(start, start + 200_000)
            .unwrap()
            .is_empty());
    }
}
