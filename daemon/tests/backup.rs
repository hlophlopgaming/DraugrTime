use draugr_time_core::Storage;
use draugr_time_daemon::backup::{self, Interval, Kind, Settings};
use draugr_time_daemon::Shared;
use serde_json::json;
use std::path::Path;

const DAY: i64 = 86_400_000;
// 2026-06-15T12:00:00Z: far from DST changes in common zones.
const T0: i64 = 1_781_524_800_000;

fn shared(dir: &Path) -> Shared {
    let mut db = Storage::open(dir.join("activity.sqlite")).unwrap();
    db.heartbeat("window", &json!({"app":"Editor"}), 1_000, 3_000)
        .unwrap();
    db.heartbeat("window", &json!({"app":"Editor"}), 2_000, 3_000)
        .unwrap();
    Shared::new(db, "token".into(), 300)
}

fn window_events(path: &Path) -> usize {
    Storage::open(path)
        .unwrap()
        .events(Some("window"), 0, i64::MAX / 2)
        .unwrap()
        .len()
}

#[test]
fn manual_backup_is_complete_listed_and_never_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let shared = shared(dir.path());
    let first = backup::create(&shared, dir.path(), Kind::Manual, T0).unwrap();
    let second = backup::create(&shared, dir.path(), Kind::Manual, T0).unwrap();
    assert_ne!(first.path, second.path);
    assert_eq!(first.kind, Kind::Manual);
    assert!(first.bytes > 0);
    assert_eq!(window_events(&first.path), 1);
    let listed = backup::list(dir.path());
    assert_eq!(listed.len(), 2);
    assert!(listed.iter().all(|b| b.created == T0 / 1000 * 1000));
    // Unrelated and partial files are ignored.
    std::fs::write(backup::directory(dir.path()).join("notes.txt"), "x").unwrap();
    std::fs::write(
        backup::directory(dir.path()).join("draugr-time-auto-2026.sqlite.partial"),
        "x",
    )
    .unwrap();
    assert_eq!(backup::list(dir.path()).len(), 2);
}

#[test]
fn schedule_runs_when_due_and_prunes_only_automatic_backups() {
    let dir = tempfile::tempdir().unwrap();
    let shared = shared(dir.path());
    let settings = Settings {
        keep: 2,
        ..Settings::default()
    };
    settings.save(dir.path()).unwrap();
    assert_eq!(Settings::load(dir.path()).unwrap(), settings);
    backup::create(&shared, dir.path(), Kind::Manual, T0 - 30 * DAY).unwrap();
    assert!(backup::run_if_due(&shared, dir.path(), T0)
        .unwrap()
        .is_some());
    assert!(backup::run_if_due(&shared, dir.path(), T0 + DAY - 1000)
        .unwrap()
        .is_none());
    for day in 1..=3 {
        assert!(backup::run_if_due(&shared, dir.path(), T0 + day * DAY)
            .unwrap()
            .is_some());
    }
    let backups = backup::list(dir.path());
    let auto: Vec<_> = backups.iter().filter(|b| b.kind == Kind::Auto).collect();
    assert_eq!(auto.len(), 2);
    assert_eq!(auto[0].created, T0 + 3 * DAY);
    assert_eq!(backups.iter().filter(|b| b.kind == Kind::Manual).count(), 1);

    let weekly = Settings {
        interval: Interval::Weekly,
        ..settings.clone()
    };
    assert!(!backup::due(&weekly, &backups, T0 + 9 * DAY));
    assert!(backup::due(&weekly, &backups, T0 + 10 * DAY));
    // A clock set backwards does not stop backups.
    assert!(backup::due(&weekly, &backups, T0));
    let off = Settings {
        automatic: false,
        ..settings
    };
    assert!(!backup::due(&off, &[], T0));
}

#[test]
fn settings_default_to_daily_backups_and_tolerate_missing_fields() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(Settings::load(dir.path()).unwrap(), Settings::default());
    assert!(Settings::default().automatic);
    std::fs::write(dir.path().join("backup.json"), br#"{"keep":0}"#).unwrap();
    let loaded = Settings::load(dir.path()).unwrap();
    assert_eq!(loaded.keep, 1);
    assert_eq!(loaded.interval, Interval::Daily);
    std::fs::write(dir.path().join("backup.json"), b"{").unwrap();
    assert!(Settings::load(dir.path()).is_err());
}

#[test]
fn restore_replaces_history_on_next_launch_and_keeps_the_old_one() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let shared = shared(data);
    let saved = backup::create(&shared, data, Kind::Manual, T0).unwrap();
    // More history after the backup, partly still in the WAL.
    shared
        .storage
        .lock()
        .unwrap()
        .heartbeat("window", &json!({"app":"Later"}), 10_000, 3_000)
        .unwrap();
    backup::stage_restore(data, &saved.path).unwrap();
    assert!(backup::restore_pending(data));
    drop(shared);

    assert!(backup::apply_pending_restore(data, T0 + DAY).unwrap());
    assert!(!backup::restore_pending(data));
    assert_eq!(window_events(&data.join("activity.sqlite")), 1);
    let before = backup::list(data)
        .into_iter()
        .find(|b| b.kind == Kind::BeforeRestore)
        .expect("previous history kept");
    assert_eq!(window_events(&before.path), 2);
    assert!(!backup::apply_pending_restore(data, T0 + 2 * DAY).unwrap());
}

#[test]
fn damaged_backups_are_never_staged_or_applied() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    drop(shared(data));
    let junk = data.join("junk.sqlite");
    std::fs::write(&junk, b"not a database").unwrap();
    assert!(backup::stage_restore(data, &junk).is_err());
    assert!(!backup::restore_pending(data));
    std::fs::write(data.join("restore-pending.sqlite"), b"broken").unwrap();
    assert!(backup::apply_pending_restore(data, T0).is_err());
    assert!(!backup::restore_pending(data));
    assert_eq!(window_events(&data.join("activity.sqlite")), 1);

    let ok = data.join("ok.sqlite");
    Storage::open_in_memory().unwrap().backup_to(&ok).unwrap();
    backup::stage_restore(data, &ok).unwrap();
    backup::cancel_restore(data).unwrap();
    assert!(!backup::restore_pending(data));
    backup::cancel_restore(data).unwrap();
}
