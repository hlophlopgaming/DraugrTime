use super::*;

#[test]
fn onboarding_completes_only_after_the_last_step_and_persists() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    let mut ui = settings::Ui::default();
    ui.advance(&path).unwrap();
    assert_eq!(ui.step, 1);
    assert!(!ui.preferences.onboarded);
    ui.advance(&path).unwrap();
    assert!(ui.preferences.onboarded);
    assert!(
        draugr_time_daemon::preferences::Preferences::load(&path)
            .unwrap()
            .onboarded
    );
}

#[test]
fn privacy_lists_trim_deduplicate_and_preserve_paths_with_commas() {
    assert_eq!(
        settings::app_list(" editor \n\neditor\n/path/with,comma\n"),
        vec!["editor", "/path/with,comma"]
    );
}

#[test]
fn schedule_times_reject_invalid_values() {
    assert_eq!(settings::minute("09:30").unwrap(), 570);
    assert_eq!(settings::minute("24:00").unwrap(), 1440);
    for value in ["24:01", "09:60", "noon", "9:", "-1:00"] {
        assert!(settings::minute(value).is_err());
    }
}

#[test]
fn day_overview_preserves_tag_precedence_after_history_deletion() {
    use draugr_time_core::{Collection, Storage};
    use serde_json::json;
    let mut db = Storage::open_in_memory().unwrap();
    db.record_interval("afk", &json!({"status":"active"}), 100, 500)
        .unwrap();
    db.record_interval("window", &json!({"app":"editor","title":"work"}), 100, 500)
        .unwrap();
    for (id, name, start) in [(1, "Older", 100), (2, "Newer", 350)] {
        db.put(Collection::Tags, None, json!({"name":name}))
            .unwrap();
        db.put(
            Collection::TagRanges,
            None,
            json!({"tag_id":id,"start_ts":start,"end_ts":500}),
        )
        .unwrap();
    }
    db.delete_history(200, 300).unwrap();
    let range = Range::new(350, 500);
    let tags = db.tags().unwrap();
    let tracks = model::tracks(
        &db.events(None, 0, 1000).unwrap(),
        &tags,
        &db.tag_ranges(0, 1000).unwrap(),
        Range::new(0, 1000),
    );
    let actual = reports::from_tracks(&tracks, &tags, range).unwrap();
    assert_eq!(
        serde_json::to_value(actual).unwrap(),
        serde_json::to_value(db.report(350, 500).unwrap()).unwrap()
    );
}

#[test]
fn privacy_settings_save_masked_apps_next_to_exclusions() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    let mut ui = settings::Ui::default();
    ui.edit_lists("secret", " Messenger ; messenger\n/opt/chat,app\n");
    let preferences = ui.privacy_preferences().unwrap();
    assert_eq!(preferences.excluded_apps, vec!["secret"]);
    assert_eq!(preferences.masked_apps, vec!["Messenger", "/opt/chat,app"]);
    preferences.save(&path).unwrap();
    let loaded = draugr_time_daemon::preferences::Preferences::load(&path).unwrap();
    assert_eq!(loaded.masked_apps, vec!["Messenger", "/opt/chat,app"]);
    let window = draugr_time_watchers::WindowInfo {
        app: "messenger".into(),
        app_name: "Messenger".into(),
        title: "Private chat".into(),
        pid: None,
    };
    assert_eq!(
        loaded.filter(Some(window)).unwrap().title,
        draugr_time_daemon::preferences::MASKED_TITLE
    );
}

#[test]
fn autostart_prompt_is_only_armed_by_new_onboarding() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.json");
    std::fs::write(&path, r#"{"onboarded":true}"#).unwrap();
    let old = draugr_time_daemon::preferences::Preferences::load(&path).unwrap();
    assert!(!old.autostart_prompt_pending);
    let mut ui = settings::Ui::default();
    ui.advance(&path).unwrap();
    ui.advance(&path).unwrap();
    assert!(ui.preferences.autostart_prompt_pending);
    ui.answer_autostart(&path).unwrap();
    assert!(
        !draugr_time_daemon::preferences::Preferences::load(&path)
            .unwrap()
            .autostart_prompt_pending
    );
}

#[test]
fn failed_prompt_persistence_keeps_the_question_pending() {
    let folder = tempfile::tempdir().unwrap();
    let mut ui = settings::Ui::default();
    ui.preferences.autostart_prompt_pending = true;
    assert!(ui.answer_autostart(folder.path()).is_err());
    assert!(ui.preferences.autostart_prompt_pending);
}

#[test]
fn settings_sections_do_not_change_privacy_drafts() {
    let mut state = State::boot().0;
    state.settings_ui.edit_lists("private", "chat");
    for section in settings::SECTIONS {
        let _ = state.edit_settings(settings::Edit::Section(section));
        assert_eq!(state.settings_ui.section, section);
        let draft = state.settings_ui.privacy_preferences().unwrap();
        assert_eq!(draft.excluded_apps, vec!["private"]);
        assert_eq!(draft.masked_apps, vec!["chat"]);
    }
}

#[test]
#[ignore = "writes Settings and autostart review screenshots; run explicitly with --test-threads=1"]
fn render_settings_review_screenshots() {
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
    let (mut state, _) = State::boot();
    state.settings_ui.preferences.onboarded = true;
    let shared = draugr_time_daemon::Shared::new(
        draugr_time_core::Storage::open_in_memory().unwrap(),
        "test".into(),
        300,
    );
    shared
        .begin_recording(draugr_time_daemon::now_ms())
        .unwrap();
    state.runtime = Some(runtime::Runtime::for_test(shared));
    state.status = state.runtime.as_ref().unwrap().status().unwrap();
    state.settings_ui.edit_lists(
        "private-editor; password-vault",
        "messenger; /opt/local-chat",
    );
    for section in settings::SECTIONS.into_iter().chain(["Autostart"]) {
        state.fixture = section != "Autostart";
        state.autostart_enabled = false;
        state.settings_ui.preferences.autostart_prompt_pending = section == "Autostart";
        state.screen = "Settings";
        state.settings_ui.section = section;
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
                        "m12-settings-{}-{}-{}-{width}x{height}.png",
                        section.split_whitespace().next().unwrap().to_lowercase(),
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

#[test]
fn removing_privacy_chips_changes_only_the_chosen_draft() {
    let mut state = State::boot().0;
    state
        .settings_ui
        .edit_lists("Secret; editor", "CHAT; editor");
    let _ = state.edit_settings(settings::Edit::RemoveApp(true, "chat".into()));
    let draft = state.settings_ui.privacy_preferences().unwrap();
    assert_eq!(draft.excluded_apps, vec!["Secret", "editor"]);
    assert_eq!(draft.masked_apps, vec!["editor"]);
    assert!(state.settings_ui.preferences.masked_apps.is_empty());
}

#[test]
fn autostart_question_respects_existing_startup_and_keeps_failed_answers_visible() {
    let mut state = State::boot().0;
    state.fixture = false;
    state.settings_ui.preferences.onboarded = true;
    state.settings_ui.preferences.autostart_prompt_pending = true;
    state.autostart_enabled = false;
    assert!(state.autostart_question());
    let date = state.date;
    let _ = state.update(Message::Key(
        iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowLeft),
        iced::keyboard::Modifiers::empty(),
    ));
    assert_eq!(state.date, date);
    state.autostart_enabled = true;
    assert!(!state.autostart_question());
    state.autostart_error = "settings could not be saved".into();
    assert!(state.autostart_question());
    state.settings_ui.preferences.autostart_prompt_pending = false;
    assert!(!state.autostart_question());
}

#[test]
fn tray_extensions_and_resume_use_the_same_pause_as_the_window() {
    let shared = draugr_time_daemon::Shared::new(
        draugr_time_core::Storage::open_in_memory().unwrap(),
        "test".into(),
        300,
    );
    shared
        .begin_recording(draugr_time_daemon::now_ms())
        .unwrap();
    let runtime = runtime::Runtime::for_test(shared);
    runtime.pause(Some(600_000)).unwrap();
    let mut state = State::boot().0;
    state.status = runtime.status().unwrap();
    state.runtime = Some(runtime);
    for (event, extension) in [("extend5", 300_000), ("extend15", 900_000)] {
        let before = state.status.paused_until.unwrap();
        let _ = state.update(Message::Tray(tray::Event::Menu(event.into())));
        let after = state.status.paused_until.unwrap();
        assert!((after - before - extension).abs() < 1000);
        assert_eq!(state.status.state, "paused");
    }
    let _ = state.update(Message::Tray(tray::Event::Menu("resume".into())));
    assert!(state.status.paused_until.is_none());
    assert_ne!(state.status.state, "paused");
}
