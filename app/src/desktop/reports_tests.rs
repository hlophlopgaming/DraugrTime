use super::*;
use draugr_time_core::{GroupBy, Report};
#[test]
fn overview_and_selection_equal_day_active_totals() {
	let (mut state, _) = State::boot();
	state.all = model::fixture(state.day);
	state.rebuild();
	let report = reports::from_tracks(&state.all, &state.tags, state.day).unwrap();
	assert_eq!(report.active_ms, state.metrics[0]);
	assert_eq!(
		report
			.rows(GroupBy::Tag, None)
			.iter()
			.map(|r| r.duration_ms)
			.sum::<i64>(),
		state.metrics[0]
	);
	let range = Range::new(
		state.day.start + 9 * 3_600_000,
		state.day.start + 14 * 3_600_000,
	);
	let selection = reports::from_tracks(&state.all, &state.tags, range).unwrap();
	let active: Vec<_> = state.all[1]
		.segments
		.iter()
		.filter(|s| s.key.as_ref() == "active")
		.map(|s| s.range)
		.collect();
	assert_eq!(selection.active_ms, model::union_duration(&active, range));
}
#[test]
fn overview_other_expands_and_highlighting_retains_editing_state() {
	let (mut state, _) = State::boot();
	state.all = model::fixture(state.day);
	state.rebuild();
	let all = state.all.clone();
	let selection = Some(Range::new(state.day.start, state.day.start + 100));
	state.selection = selection;
	let r = reports::from_tracks(&state.all, &state.tags, state.day).unwrap();
	let slices = reports::slices(&r, false);
	assert_eq!(slices.iter().map(|s| s.duration).sum::<i64>(), r.active_ms);
	let _ = state.update(Message::Report(reports::Edit::Highlight(Some((
		None, None,
	)))));
	assert_eq!(state.all[2].segments, all[2].segments);
	assert_eq!(state.selection, selection);
	let _ = state.update(Message::Report(reports::Edit::Expand));
	assert!(state.report_ui.expanded);
	state.release_day();
	assert!(state.report_ui.data.is_none());
}
#[test]
fn tiny_slices_preserve_total_and_expand_with_exact_members() {
	let mut r = Report {
		from: 0,
		to: 1000,
		active_ms: 1000,
		..Default::default()
	};
	for (app, start, end) in [("Main", 0, 990), ("tiny", 990, 995), ("tiny2", 995, 1000)] {
		r.intervals.push(draugr_time_core::ReportInterval {
			start_ts: start,
			end_ts: end,
			tag_id: None,
			tag: "Untagged".into(),
			application: app.into(),
			document: "".into(),
		});
	}
	let s = reports::slices(&r, false);
	let other = s.iter().find(|s| s.application == "Other").unwrap();
	assert_eq!(other.duration, 10);
	assert_eq!(other.members.len(), 2);
	assert_eq!(reports::slices(&r, true).len(), 3);
}
#[test]
fn overview_keyboard_highlights_without_opening_tag_dialog() {
	let (mut state, _) = State::boot();
	state.all = model::fixture(state.day);
	state.rebuild();
	state.screen = "Overview";
	let _ = state.update(Message::Key(
		keyboard::Key::Named(keyboard::key::Named::ArrowDown),
		keyboard::Modifiers::default(),
	));
	assert!(state.report_ui.highlight.is_some());
	assert!(state.editor.is_none());
	let _ = state.update(Message::Key(
		keyboard::Key::Named(keyboard::key::Named::Enter),
		keyboard::Modifiers::default(),
	));
	assert!(state.editor.is_none());
	let _ = state.update(Message::Key(
		keyboard::Key::Named(keyboard::key::Named::Escape),
		keyboard::Modifiers::default(),
	));
	assert!(state.report_ui.highlight.is_none());
}
#[test]
fn report_load_generations_drop_stale_results() {
	let (mut state, _) = State::boot();
	state.fixture = true;
	state.screen = "Reports";
	let _ = state.edit_report(reports::Edit::Preset(0));
	let data = state.report_ui.data.clone().unwrap();
	let old = state.report_ui.generation;
	state.release_day();
	let _ = state.edit_report(reports::Edit::Loaded(old, Ok(data)));
	assert!(state.report_ui.data.is_none());
}
#[test]
fn month_preset_covers_short_long_leap_and_year_end_months() {
	for (year, month, days) in [
		(2026, 2, 28),
		(2024, 2, 29),
		(2026, 4, 30),
		(2026, 10, 31),
		(2026, 12, 31),
	] {
		let (mut state, _) = State::boot();
		state.screen = "Reports";
		state.runtime = None;
		state.date = NaiveDate::from_ymd_opt(year, month, 14).unwrap();
		let _ = state.edit_report(reports::Edit::Preset(2));
		assert_eq!(state.report_ui.from, format!("{year}-{month:02}-01"));
		assert_eq!(state.report_ui.to, format!("{year}-{month:02}-{days}"));
	}
}

#[test]
fn leaving_report_screens_releases_data_and_rejects_late_loads() {
	for source in ["Week", "Reports"] {
		for destination in ["Day", "Overview", "Tags", "Rules", "Settings"] {
			let (mut state, _) = State::boot();
			state.fixture = true;
			state.screen = source;
			let _ = state.edit_report(reports::Edit::Refresh);
			let old = state.report_ui.generation;
			let data = state.report_ui.data.clone().unwrap();
			let _ = state.update(Message::Screen(destination));
			assert!(state.report_ui.data.is_none(), "{source} -> {destination}");
			let _ = state.edit_report(reports::Edit::Loaded(old, Ok(data)));
			assert!(state.report_ui.data.is_none());
		}
	}
}

#[test]
fn clicking_week_day_releases_report_data() {
	let (mut state, _) = State::boot();
	state.fixture = true;
	state.screen = "Week";
	let _ = state.edit_report(reports::Edit::Refresh);
	let _ = state.edit_report(reports::Edit::OpenDay(state.date));
	assert_eq!(state.screen, "Day");
	assert!(state.report_ui.data.is_none());
}

#[test]
fn tray_and_away_navigation_release_reports() {
	for event in [
		Message::Edit(tagging::Edit::Quick(5)),
		Message::Edit(tagging::Edit::AwayTag),
		Message::Tray(tray::Event::Menu("settings".into())),
	] {
		let (mut state, _) = State::boot();
		state.fixture = true;
		state.screen = "Week";
		state.away_range = Some(state.day);
		let _ = state.edit_report(reports::Edit::Refresh);
		let _ = state.update(event);
		assert!(state.report_ui.data.is_none());
	}
}

#[test]
fn week_refresh_preserves_custom_report_dates() {
	let (mut state, _) = State::boot();
	state.fixture = true;
	state.screen = "Reports";
	state.report_ui.from = "2026-09-01".into();
	state.report_ui.to = "2026-09-30".into();
	state.screen = "Week";
	let _ = state.edit_report(reports::Edit::Refresh);
	assert_eq!(state.report_ui.from, "2026-09-01");
	assert_eq!(state.report_ui.to, "2026-09-30");
	assert_eq!(state.report_ui.data.as_ref().unwrap().days.len(), 7);
	let _ = state.update(Message::Screen("Reports"));
	assert_eq!(state.report_ui.data.as_ref().unwrap().days.len(), 30);
}
