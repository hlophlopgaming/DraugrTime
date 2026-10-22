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
