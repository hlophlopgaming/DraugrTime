use draugr_time_core::Storage;
use draugr_time_daemon::{Shared, Tracker};
use draugr_time_watchers::{MockWatcher, WatcherError, WindowInfo};

fn window(app: &str) -> WindowInfo {
    WindowInfo {
        app: app.into(),
        app_name: app.into(),
        title: format!("{app} title"),
        pid: Some(1),
    }
}

#[test]
fn ten_minute_scenario_records_changes_away_and_return() {
    let shared = Shared::new(Storage::open_in_memory().unwrap(), "test-token".into(), 300);
    let mut tracker = Tracker::new(shared.clone());
    let mut watcher = MockWatcher::new(Some(window("editor")), 0);
    for second in 0..=600 {
        watcher.window = Ok(Some(window(if second < 120 {
            "editor"
        } else {
            "browser"
        })));
        watcher.idle = Ok(if (180..540).contains(&second) {
            (second - 180) as u64
        } else {
            0
        });
        tracker
            .poll_once(second * 1_000, &watcher, &watcher)
            .unwrap();
    }
    let storage = shared.storage.lock().unwrap();
    let afk = storage.events(Some("afk"), 0, 601_000).unwrap();
    assert_eq!(afk.len(), 3);
    assert_eq!(afk[0].data["status"], "active");
    assert_eq!((afk[1].start_ts, afk[1].end_ts), (481_000, 540_000));
    assert_eq!(afk[1].data["status"], "away");
    let windows = storage.events(Some("window"), 0, 601_000).unwrap();
    assert_eq!(windows.len(), 3);
    assert_eq!((windows[0].start_ts, windows[0].end_ts), (0, 120_000));
    assert_eq!((windows[1].start_ts, windows[1].end_ts), (120_000, 481_000));
    assert_eq!((windows[2].start_ts, windows[2].end_ts), (540_000, 600_000));
}

#[test]
fn pause_gap_is_not_recorded_and_resume_starts_a_new_interval() {
    let shared = Shared::new(Storage::open_in_memory().unwrap(), "token".into(), 300);
    let mut tracker = Tracker::new(shared.clone());
    let watcher = MockWatcher::new(Some(window("editor")), 0);
    tracker.poll_once(0, &watcher, &watcher).unwrap();
    tracker.poll_once(1_000, &watcher, &watcher).unwrap();
    shared.pause(1_500, None).unwrap();
    tracker.poll_once(2_000, &watcher, &watcher).unwrap();
    shared.resume();
    tracker.poll_once(3_000, &watcher, &watcher).unwrap();
    tracker.poll_once(4_000, &watcher, &watcher).unwrap();
    let events = shared
        .storage
        .lock()
        .unwrap()
        .events(Some("window"), 0, 5_000)
        .unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!((events[0].start_ts, events[0].end_ts), (0, 1_500));
    assert_eq!((events[1].start_ts, events[1].end_ts), (3_000, 4_000));
}

#[test]
fn pause_and_resume_between_ticks_still_breaks_the_interval() {
    let shared = Shared::new(Storage::open_in_memory().unwrap(), "token".into(), 300);
    let mut tracker = Tracker::new(shared.clone());
    let watcher = MockWatcher::new(Some(window("editor")), 0);
    tracker.poll_once(0, &watcher, &watcher).unwrap();
    tracker.poll_once(1_000, &watcher, &watcher).unwrap();
    shared.pause(1_200, None).unwrap();
    shared.resume();
    tracker.poll_once(2_000, &watcher, &watcher).unwrap();
    tracker.poll_once(3_000, &watcher, &watcher).unwrap();
    let events = shared
        .storage
        .lock()
        .unwrap()
        .events(Some("window"), 0, 4_000)
        .unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].end_ts, 1_200);
    assert_eq!(events[1].start_ts, 2_000);
}

#[test]
fn missing_backend_is_visible_and_does_not_record_false_activity() {
    let shared = Shared::new(Storage::open_in_memory().unwrap(), "token".into(), 300);
    let mut tracker = Tracker::new(shared.clone());
    let mut watcher = MockWatcher::new(None, 0);
    watcher.window = Err(WatcherError::Unsupported("install compositor extension"));
    watcher.idle = Err(WatcherError::Unsupported("idle protocol unavailable"));
    tracker.poll_once(0, &watcher, &watcher).unwrap();
    assert_eq!(shared.status().state, "error");
    assert!(shared
        .status()
        .backends
        .iter()
        .all(|backend| backend.state == "unsupported"));
    assert!(shared
        .storage
        .lock()
        .unwrap()
        .events(None, 0, 1_000)
        .unwrap()
        .is_empty());
}

#[test]
fn sleep_gap_is_not_counted_as_work_and_timed_pause_expires() {
    let shared = Shared::new(Storage::open_in_memory().unwrap(), "token".into(), 5);
    let mut tracker = Tracker::new(shared.clone());
    let watcher = MockWatcher::new(Some(window("editor")), 0);
    tracker.poll_once(0, &watcher, &watcher).unwrap();
    tracker.poll_once(1_000, &watcher, &watcher).unwrap();
    tracker.poll_once(60_000, &watcher, &watcher).unwrap();
    tracker.poll_once(61_000, &watcher, &watcher).unwrap();
    shared.pause(61_000, Some(1_000)).unwrap();
    tracker.poll_once(63_000, &watcher, &watcher).unwrap();
    assert_eq!(shared.status().state, "tracking");
    let events = shared
        .storage
        .lock()
        .unwrap()
        .events(Some("window"), 0, 64_000)
        .unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].end_ts, 1_000);
    assert_eq!(events[1].start_ts, 60_000);
}

#[test]
fn pause_and_shutdown_persist_the_overlay_and_stop_further_sampling() {
    let path = std::env::temp_dir().join(format!("draugr-flush-{}.sqlite", uuid::Uuid::new_v4()));
    let shared = Shared::new(Storage::open(&path).unwrap(), "test".into(), 300);
    let mut tracker = Tracker::new(shared.clone());
    let watcher = MockWatcher::new(Some(window("editor")), 0);
    for now in [1000, 2000] {
        tracker.poll_once(now, &watcher, &watcher).unwrap();
    }
    shared.pause(2000, None).unwrap();
    assert_eq!(
        Storage::open(&path)
            .unwrap()
            .events(Some("window"), 0, 10_000)
            .unwrap()[0]
            .end_ts,
        2000
    );
    tracker.poll_once(2000, &watcher, &watcher).unwrap();
    shared.resume();
    for now in [3000, 4000] {
        tracker.poll_once(now, &watcher, &watcher).unwrap();
    }
    shared.shutdown().unwrap();
    tracker.poll_once(5000, &watcher, &watcher).unwrap();
    let storage = Storage::open(&path).unwrap();
    let events = storage.events(Some("window"), 0, 10_000).unwrap();
    assert_eq!(events.last().unwrap().end_ts, 4000);
    drop(storage);
    drop(tracker);
    drop(shared);
    std::fs::remove_file(path).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn shutdown_notification_handles_a_signal_before_the_waiter_starts() {
    let shared = Shared::new(Storage::open_in_memory().unwrap(), "test".into(), 300);
    shared.shutdown().unwrap();
    tokio::time::timeout(
        std::time::Duration::from_millis(100),
        shared.wait_for_shutdown(),
    )
    .await
    .unwrap();
}

#[test]
fn rules_tag_observed_active_time_but_never_away_pause_or_gaps() {
    use draugr_time_core::Collection;
    use serde_json::json;
    let shared = Shared::new(Storage::open_in_memory().unwrap(), "token".into(), 300);
    {
        let mut db = shared.storage.lock().unwrap();
        db.put(Collection::Tags, None, json!({"name":"Work"}))
            .unwrap();
        db.put(
            Collection::Rules,
            None,
            json!({"field":"app","pattern":"editor","tag_id":1}),
        )
        .unwrap();
    }
    let mut tracker = Tracker::new(shared.clone());
    let watcher = MockWatcher::new(Some(window("editor")), 0);
    for ts in [0, 1000, 2000] {
        tracker.poll_once(ts, &watcher, &watcher).unwrap();
    }
    shared.pause(2000, None).unwrap();
    tracker.poll_once(3000, &watcher, &watcher).unwrap();
    shared.resume();
    for ts in [4000, 5000, 9000, 10000] {
        tracker.poll_once(ts, &watcher, &watcher).unwrap();
    }
    let db = shared.storage.lock().unwrap();
    let ranges = db.tag_ranges(0, 11000).unwrap();
    assert_eq!(
        ranges.iter().map(|r| r.end_ts - r.start_ts).sum::<i64>(),
        4000
    );
    assert!(ranges
        .iter()
        .all(|r| r.end_ts <= 2000 || r.start_ts >= 4000 && r.end_ts <= 5000 || r.start_ts >= 9000));
    assert_eq!(ranges.len(), 3, "continuous auto tags merge");
}

fn restarted(dir: &std::path::Path, now: i64) -> (Shared, bool) {
    let shared = Shared::new(Storage::open_in_memory().unwrap(), "token".into(), 300);
    shared.set_pause_file(dir.join("pause.json"));
    let restored = shared.restore_pause(now).unwrap();
    (shared, restored)
}

#[test]
fn timed_pause_continues_after_restart_and_ends_at_the_original_time() {
    let dir = tempfile::tempdir().unwrap();
    let (first, _) = restarted(dir.path(), 0);
    first.pause(10_000, Some(60_000)).unwrap();
    drop(first);
    let (second, restored) = restarted(dir.path(), 20_000);
    assert!(restored);
    let status = second.status();
    assert_eq!(status.state, "paused");
    assert_eq!(status.paused_until, Some(70_000));
    let mut tracker = Tracker::new(second.clone());
    let watcher = MockWatcher::new(Some(window("editor")), 0);
    tracker.poll_once(30_000, &watcher, &watcher).unwrap();
    assert_eq!(second.status().state, "paused");
    let windows = |shared: &Shared| {
        shared
            .storage
            .lock()
            .unwrap()
            .events(Some("window"), 0, 200_000)
            .unwrap()
    };
    assert!(windows(&second).is_empty());
    tracker.poll_once(70_000, &watcher, &watcher).unwrap();
    assert_eq!(second.status().state, "tracking");
    assert!(!dir.path().join("pause.json").exists());
    let (third, restored) = restarted(dir.path(), 71_000);
    assert!(!restored);
    assert_ne!(third.status().state, "paused");
}

#[test]
fn expired_resumed_and_untimed_pauses_do_not_survive_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let (shared, _) = restarted(dir.path(), 0);
    shared.pause(0, Some(5_000)).unwrap();
    let (later, restored) = restarted(dir.path(), 5_000);
    assert!(!restored);
    assert_eq!(later.status().paused_until, None);
    assert!(!dir.path().join("pause.json").exists());

    shared.pause(6_000, Some(60_000)).unwrap();
    shared.resume();
    assert!(!restarted(dir.path(), 7_000).1);

    shared.pause(8_000, Some(60_000)).unwrap();
    shared.pause(9_000, None).unwrap();
    assert!(!restarted(dir.path(), 10_000).1);
}

#[test]
fn extending_a_pause_moves_the_saved_end() {
    let dir = tempfile::tempdir().unwrap();
    let (shared, _) = restarted(dir.path(), 0);
    shared.pause(0, Some(300_000)).unwrap();
    shared.pause(1_000, Some(299_000 + 900_000)).unwrap();
    let (later, restored) = restarted(dir.path(), 400_000);
    assert!(restored);
    assert_eq!(later.status().paused_until, Some(1_200_000));
}

#[test]
fn unreadable_pause_file_keeps_recording() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("pause.json"), b"not json").unwrap();
    assert!(!restarted(dir.path(), 0).1);
}
