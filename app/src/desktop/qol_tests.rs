use super::*;

fn sample() -> State {
    let (mut state, _) = State::boot();
    state.fixture = true;
    state.settings_ui.preferences.onboarded = true;
    state.all = model::fixture(state.day);
    state.rebuild();
    state
}

#[test]
fn filter_trims_outer_whitespace_and_escape_clears_before_selection() {
    let mut state = sample();
    let label = state.rows[0].label.to_string();
    let range = state.all[2].segments[0].range;
    let _ = state.update(Message::Select(Some(range)));
    let _ = state.update(Message::SummaryFilter(format!(" {label} ")));
    assert!(!state.rows.is_empty());
    let _ = state.update(Message::Key(
        keyboard::Key::Named(keyboard::key::Named::Escape),
        keyboard::Modifiers::empty(),
    ));
    assert!(state.summary_search.is_empty());
    assert_eq!(state.selection, Some(range));
}

#[test]
fn day_keys_do_not_change_hidden_dates_or_zoom_on_management_screens() {
    let mut state = sample();
    for screen in ["Settings", "Tags", "Rules", "Reports"] {
        state.screen = screen;
        let date = state.date;
        let view = state.view;
        for key in [
            keyboard::Key::Named(keyboard::key::Named::ArrowLeft),
            keyboard::Key::Character("+".into()),
        ] {
            let _ = state.update(Message::Key(key, keyboard::Modifiers::empty()));
        }
        assert_eq!(state.date, date, "{screen}");
        assert_eq!(state.view, view, "{screen}");
    }
}

#[test]
fn unsaved_rule_can_stay_discard_or_save_and_invalid_save_keeps_draft() {
    let mut state = sample();
    let shared = draugr_time_daemon::Shared::new(
        draugr_time_core::Storage::open_in_memory().unwrap(),
        "test".into(),
        300,
    );
    let tag = shared
        .storage
        .lock()
        .unwrap()
        .create_tag_path("Work")
        .unwrap()
        .0;
    state.runtime = Some(Runtime::for_test(shared.clone()));
    state.edit_rule(rules::Edit::New);
    state.edit_rule(rules::Edit::Pattern("draft".into()));
    state.edit_rule(rules::Edit::New);
    assert_eq!(state.rule_editor.pattern, "draft");
    assert!(state.qol.pending.is_some());
    let _ = state.edit_qol(qol::Edit::Stay);
    assert!(state.qol.pending.is_none());
    state.edit_rule(rules::Edit::New);
    let _ = state.edit_qol(qol::Edit::SaveDraft);
    assert!(
        state.qol.pending.is_some(),
        "missing tag must block the transition"
    );
    assert_eq!(state.rule_editor.pattern, "draft");
    state.edit_rule(rules::Edit::Tag(tag));
    let _ = state.edit_qol(qol::Edit::SaveDraft);
    assert!(state.qol.pending.is_none());
    assert_eq!(
        shared.storage.lock().unwrap().rules().unwrap()[0].pattern,
        "draft"
    );
    state.edit_rule(rules::Edit::Pattern("discard me".into()));
    state.edit_rule(rules::Edit::Select(1));
    let _ = state.edit_qol(qol::Edit::DiscardDraft);
    assert_eq!(state.rule_editor.pattern, "draft");
}

#[test]
fn dirty_tag_and_suggested_rule_are_not_replaced_without_a_decision() {
    let mut state = sample();
    let shared = draugr_time_daemon::Shared::new(
        draugr_time_core::Storage::open_in_memory().unwrap(),
        "test".into(),
        300,
    );
    state.runtime = Some(Runtime::for_test(shared));
    let _ = state.edit_management(management::Edit::NewTag);
    let _ = state.edit_management(management::Edit::TagName("draft tag".into()));
    let _ = state.edit_management(management::Edit::NewTag);
    assert_eq!(state.management.tag.as_ref().unwrap().name, "draft tag");
    let _ = state.edit_qol(qol::Edit::SaveDraft);
    assert!(state.qol.pending.is_none());
    assert!(state
        .runtime
        .as_ref()
        .unwrap()
        .shared
        .storage
        .lock()
        .unwrap()
        .tags()
        .unwrap()
        .iter()
        .any(|t| t.name == "draft tag"));
    state.edit_rule(rules::Edit::New);
    state.edit_rule(rules::Edit::Pattern("keep this".into()));
    let _ = state.edit_management(management::Edit::Suggestion("Browser".into()));
    assert_eq!(state.rule_editor.pattern, "keep this");
    let _ = state.edit_qol(qol::Edit::DiscardDraft);
    assert!(state.rule_editor.pattern.contains("Browser"));
}

#[test]
fn date_jump_validates_and_cancels_without_changing_the_day() {
    let mut state = sample();
    let original = state.date;
    let _ = state.edit_qol(qol::Edit::OpenDate);
    let _ = state.edit_qol(qol::Edit::Date("2026-02-30".into()));
    let _ = state.edit_qol(qol::Edit::GoDate);
    assert_eq!(state.date, original);
    assert!(!state.qol.date_error.is_empty());
    let _ = state.edit_qol(qol::Edit::CancelDate);
    assert!(state.qol.date.is_none());
    let _ = state.edit_qol(qol::Edit::OpenDate);
    let _ = state.edit_qol(qol::Edit::Date("2026-02-28".into()));
    let _ = state.edit_qol(qol::Edit::GoDate);
    assert_eq!(state.date, NaiveDate::from_ymd_opt(2026, 2, 28).unwrap());
    assert!(state.qol.date.is_none());
}

#[test]
fn time_ticks_leave_room_for_labels_at_narrow_widths_and_all_zoom_levels() {
    for span in [
        120_000, 900_000, 3_600_000, 36_000_000, 86_400_000, 90_000_000,
    ] {
        for width in [60.0, 98.0, 250.0, 420.0, 640.0] {
            let tick = timeline::tick_interval(span, width);
            assert!(width * tick as f32 / span as f32 >= design::TIME_TICK_MIN);
        }
    }
    for width in [250.0, 420.0, 640.0] {
        let step = timeline::hour_label_step(width, 24.0);
        assert!(width / 24.0 * step as f32 >= design::HOUR_TICK_MIN);
    }
}

#[test]
fn window_resize_ignores_hidden_windows_and_zero_minimized_sizes() {
    let mut state = sample();
    let id = window::Id::unique();
    state.window = Some(id);
    let size = iced::Size::new(910.0, 710.0);
    let _ = state.edit_qol(qol::Edit::Resized(id, size));
    let _ = state.edit_qol(qol::Edit::Resized(id, iced::Size::ZERO));
    let _ = state.edit_qol(qol::Edit::Resized(
        window::Id::unique(),
        iced::Size::new(300.0, 200.0),
    ));
    assert_eq!(state.qol.preferences.size, Some(size));
}

#[test]
#[ignore = "writes QoL review captures; run explicitly with --test-threads=1"]
fn render_qol_review_screenshots() {
    use iced::advanced::{
        layout, mouse, renderer, renderer::Headless as _, widget::Tree, Layout, Renderer as _,
    };
    {
        let mut fonts = iced::advanced::graphics::text::font_system()
            .write()
            .unwrap();
        fonts.load_font(std::borrow::Cow::Borrowed(include_bytes!(
            "../../fonts/InterTight.ttf"
        )));
        fonts.load_font(std::borrow::Cow::Borrowed(include_bytes!(
            "../../fonts/JetBrainsMono-Regular.ttf"
        )));
    }
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/screenshots/qol");
    std::fs::create_dir_all(&output).unwrap();
    for language in [
        localization::Language::En,
        localization::Language::Ru,
        localization::Language::De,
    ] {
        for dark in [true, false] {
            for screen in ["Day", "Week", "Settings", "Data", "Date", "Draft", "Help"] {
                let mut state = sample();
                localization::set(language);
                state.theme = native_theme(dark);
                state.screen = match screen {
                    "Day" | "Date" | "Draft" | "Help" => "Day",
                    "Week" => "Week",
                    _ => "Settings",
                };
                if screen == "Week" {
                    let _ = state.edit_report(reports::Edit::Refresh);
                }
                if screen == "Data" {
                    state.settings_ui.section = "Data & import";
                }
                if screen == "Date" {
                    let _ = state.edit_qol(qol::Edit::OpenDate);
                }
                if screen == "Help" {
                    state.shortcuts_open = true;
                }
                if screen == "Draft" {
                    state.qol.pending = Some(qol::Pending::Rule(rules::Edit::New));
                }
                for width in [480, 700, 759, 760, 900, 1280] {
                    let height = if matches!(screen, "Date" | "Draft" | "Help") {
                        600
                    } else {
                        820
                    };
                    let size = iced::Size::new(width as f32, height as f32);
                    let viewport = iced::Rectangle::with_size(size);
                    let mut renderer = iced::Renderer::new(design::UI, 13.0.into());
                    let mut view = state.view_sized(size.width);
                    let mut tree = Tree::new(view.as_widget());
                    let node = view.as_widget_mut().layout(
                        &mut tree,
                        &renderer,
                        &layout::Limits::new(iced::Size::ZERO, size),
                    );
                    assert_eq!(node.size(), size, "{screen} {} {width}", language.code());
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
                    let pixels = renderer.screenshot(
                        iced::Size::new(width, height),
                        1.0,
                        state.theme.palette().background,
                    );
                    assert!(!pixels.is_empty());
                    if language == localization::Language::Ru && [480, 760, 1280].contains(&width) {
                        ::image::save_buffer(
                            output.join(format!(
                                "{}-{}-{width}x{height}.png",
                                screen.to_lowercase(),
                                if dark { "dark" } else { "light" }
                            )),
                            &pixels,
                            width,
                            height,
                            ::image::ColorType::Rgba8,
                        )
                        .unwrap();
                    }
                }
            }
        }
    }
    localization::set(localization::Language::En);
}
