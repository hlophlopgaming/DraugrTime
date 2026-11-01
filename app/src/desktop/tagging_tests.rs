use super::*;
use serde_json::json;

#[test]
fn away_prompt_threshold_and_duplicate_suppression() {
    let mut prompt = tagging::AwayPrompt::default();
    assert_eq!(prompt.observe(Some(Range::new(0, 899_999))), None);
    assert_eq!(
        prompt.observe(Some(Range::new(0, 900_000))),
        Some(Range::new(0, 900_000))
    );
    assert_eq!(prompt.observe(Some(Range::new(0, 900_000))), None);
    assert_eq!(prompt.observe(None), None);
    prompt.threshold = 1_800_000;
    assert_eq!(prompt.observe(Some(Range::new(900_000, 1_800_000))), None);
}

#[test]
fn tag_totals_clip_at_day_boundaries_and_union_overlaps() {
    let tracks = model::tracks(
        &[],
        &[serde_json::from_value(json!({"id":1,"name":"Work"})).unwrap()],
        &[
            serde_json::from_value(json!({"id":1,"tag_id":1,"start_ts":0,"end_ts":200})).unwrap(),
            serde_json::from_value(json!({"id":2,"tag_id":1,"start_ts":150,"end_ts":350})).unwrap(),
        ],
        Range::new(100, 300),
    );
    let summary = model::summarize(&tracks[0].segments, Range::new(100, 300));
    assert_eq!(summary[0].duration, 200);
}

#[test]
fn tag_drag_moves_or_resizes_without_changing_duration_or_inverting_edges() {
    let range = Range::new(100, 200);
    assert_eq!(tagging::drag_range(range, 50, 0), Range::new(150, 250));
    assert_eq!(tagging::drag_range(range, -200, 0), Range::new(0, 100));
    assert_eq!(tagging::drag_range(range, 20, -1), Range::new(120, 200));
    assert_eq!(tagging::drag_range(range, -20, 1), Range::new(100, 180));
    assert_eq!(tagging::drag_range(range, 200, -1), Range::new(199, 200));
}

#[test]
fn mock_afk_return_prompt_creates_exact_tag_range_and_undo_removes_it() {
    use draugr_time_core::{Collection, Storage};
    use draugr_time_daemon::{Shared, Tracker};
    use draugr_time_watchers::MockWatcher;
    let shared = Shared::new(Storage::open_in_memory().unwrap(), "test".into(), 300);
    let mut tracker = Tracker::new(shared.clone());
    let active = MockWatcher::new(None, 0);
    let away = MockWatcher::new(None, 301);
    tracker.poll_once(1000, &active, &active).unwrap();
    for ts in (2000..=902_000).step_by(1000) {
        tracker.poll_once(ts, &away, &away).unwrap();
    }
    tracker.poll_once(903_000, &active, &active).unwrap();
    let snapshot =
        Runtime::load(shared.clone(), Range::new(0, 1_000_000), Default::default()).unwrap();
    let (mut state, _) = State::boot();
    state.runtime = Some(Runtime::for_test(shared.clone()));
    state.away_prompt = Default::default();
    let _ = state.update(Message::Loaded(state.load_generation, Ok(snapshot)));
    assert_eq!(state.away_range, Some(Range::new(2000, 903_000)));
    let _ = state.edit(tagging::Edit::AwayTag);
    let _ = state.edit(tagging::Edit::Name("Meetings, Planning".into()));
    let _ = state.edit(tagging::Edit::Save(false));
    let ranges = shared
        .storage
        .lock()
        .unwrap()
        .list(Collection::TagRanges)
        .unwrap();
    assert_eq!(ranges[0]["start_ts"], 2000);
    assert_eq!(ranges[0]["end_ts"], 903_000);
    assert_eq!(state.away_range, None);
    let _ = state.edit(tagging::Edit::Undo);
    assert!(shared
        .storage
        .lock()
        .unwrap()
        .list(Collection::TagRanges)
        .unwrap()
        .is_empty());
}

#[test]
fn crossing_day_tag_keeps_original_bounds_and_dialog_checks_overlaps() {
    use draugr_time_core::{Collection, Storage};
    use draugr_time_daemon::Shared;
    let mut db = Storage::open_in_memory().unwrap();
    db.tag_interval("Overnight", "", 100, 300, false).unwrap();
    db.tag_interval("Next", "", 350, 450, false).unwrap();
    let shared = Shared::new(db, "test".into(), 300);
    let (mut state, _) = State::boot();
    state.day = Range::new(200, 400);
    state.runtime = Some(Runtime::for_test(shared.clone()));
    let snapshot = Runtime::load(shared.clone(), state.day, Default::default()).unwrap();
    let _ = state.update(Message::Loaded(state.load_generation, Ok(snapshot)));
    let segment = &state.all[0].segments[0];
    assert_eq!(segment.range, Range::new(200, 300));
    let (id, original) = segment.tag_record.unwrap();
    assert_eq!(original, Range::new(100, 300));
    let _ = state.edit(tagging::Edit::Transform(id, Range::new(200, 400)));
    assert!(state.editor.as_ref().unwrap().overlap);
    assert_eq!(
        shared
            .storage
            .lock()
            .unwrap()
            .get(Collection::TagRanges, id)
            .unwrap()["start_ts"],
        100
    );
    let _ = state.edit(tagging::Edit::Save(false));
    let ranges = shared
        .storage
        .lock()
        .unwrap()
        .list(Collection::TagRanges)
        .unwrap();
    assert!(ranges
        .iter()
        .any(|r| r["start_ts"] == 400 && r["end_ts"] == 450));
    assert!(ranges
        .iter()
        .any(|r| r["start_ts"] == 200 && r["end_ts"] == 400));
    let _ = state.edit(tagging::Edit::Undo);
    assert_eq!(
        shared
            .storage
            .lock()
            .unwrap()
            .get(Collection::TagRanges, id)
            .unwrap()["start_ts"],
        100
    );
}

#[test]
fn return_status_prompts_while_window_is_hidden_on_a_historical_day() {
    use draugr_time_core::Storage;
    use draugr_time_daemon::Shared;
    let mut db = Storage::open_in_memory().unwrap();
    db.record_interval(
        "computer",
        &json!({"machine":"awake","activity":"away","recording":"recording"}),
        1000,
        901_000,
    )
    .unwrap();
    db.record_interval(
        "computer",
        &json!({"machine":"awake","activity":"active","recording":"recording"}),
        901_000,
        902_000,
    )
    .unwrap();
    let shared = Shared::new(db, "test".into(), 300);
    let (mut state, _) = State::boot();
    state.runtime = Some(Runtime::for_test(shared));
    state.window = None;
    state.date = state.date.pred_opt().unwrap();
    state.away_prompt = Default::default();
    state.status.computer.activity = "away".into();
    let mut status = state.status.clone();
    status.computer.activity = "active".into();
    let _ = state.update(Message::Tray(tray::Event::Status(status.into())));
    assert_eq!(state.away_range, Some(Range::new(1000, 901_000)));
    assert!(state.window.is_some());
}

#[test]
fn scoped_metadata_preserves_global_delete_confirmation_and_outside_day_overlap() {
    use draugr_time_core::Storage;
    use draugr_time_daemon::Shared;
    let mut storage = Storage::open_in_memory().unwrap();
    storage
        .tag_interval("Project", "", 100, 200, false)
        .unwrap();
    storage
        .tag_interval("Project", "", 1000, 1100, false)
        .unwrap();
    let shared = Shared::new(storage, "test".into(), 300);
    let (mut state, _) = State::boot();
    state.day = Range::new(100, 300);
    state.runtime = Some(Runtime::for_test(shared.clone()));
    let snapshot = Runtime::load(shared, state.day, Default::default()).unwrap();
    let _ = state.update(Message::Loaded(state.load_generation, Ok(snapshot)));
    assert_eq!(state.tag_ranges.len(), 1);
    let tag = state.tags[0].id;
    let _ = state.edit(tagging::Edit::Tag(tag));
    let _ = state.edit(tagging::Edit::DeleteTag(false));
    assert_eq!(state.editor.as_ref().unwrap().delete_usage, Some((2, 200)));
    state.selection = Some(Range::new(1000, 1100));
    let _ = state.edit(tagging::Edit::Open);
    let _ = state.edit(tagging::Edit::Name("Other".into()));
    let _ = state.edit(tagging::Edit::Save(false));
    assert!(state.editor.as_ref().unwrap().overlap);
}

#[test]
fn quick_tag_minutes_field_moves_the_start_and_rejects_invalid_lengths() {
    let (mut state, _) = State::boot();
    state.fixture = true;
    let _ = state.update(Message::Edit(tagging::Edit::Quick(15)));
    let editor = state.editor.as_ref().unwrap();
    assert_eq!(editor.minutes, "15");
    let end = state.selection.unwrap().end;
    assert_eq!(state.selection.unwrap().duration(), 15 * 60_000);

    let _ = state.update(Message::Edit(tagging::Edit::Minutes("40".into())));
    assert_eq!(state.selection.unwrap(), Range::new(end - 40 * 60_000, end));
    assert_eq!(state.editor.as_ref().unwrap().minutes, "40");

    for invalid in ["0", "", "1441", "abc"] {
        let _ = state.update(Message::Edit(tagging::Edit::Minutes(invalid.into())));
        assert_eq!(state.selection.unwrap(), Range::new(end - 40 * 60_000, end));
    }
    let _ = state.update(Message::Edit(tagging::Edit::Minutes("9x0".into())));
    assert_eq!(state.editor.as_ref().unwrap().minutes, "90");
    assert_eq!(state.selection.unwrap(), Range::new(end - 90 * 60_000, end));
}

#[test]
fn tray_custom_quick_tag_opens_an_empty_minutes_field() {
    let (mut state, _) = State::boot();
    state.fixture = true;
    state.screen = "Week";
    let _ = state.update(Message::Tray(tray::Event::Menu("quick_custom".into())));
    assert_eq!(state.screen, "Day");
    let editor = state.editor.as_ref().unwrap();
    assert!(editor.minutes.is_empty());
    assert!(editor.range_id.is_none());
}
