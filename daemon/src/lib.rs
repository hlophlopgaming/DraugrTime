pub mod api;
mod computer;
mod history;
pub mod preferences;
pub use computer::ComputerState;
pub mod paths;
pub mod service;

use draugr_time_core::{Storage, StorageError};
use draugr_time_watchers::{IdleWatcher, WatcherError, WindowInfo, WindowWatcher};
use serde::Serialize;
use serde_json::json;
use std::sync::{Arc, Mutex};

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct BackendStatus {
    pub name: String,
    pub state: String,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Status {
    pub computer: ComputerState,
    pub computer_capabilities: String,
    pub state: String,
    pub started_ts: i64,
    pub updated_ts: i64,
    pub paused_until: Option<i64>,
    pub backends: Vec<BackendStatus>,
    pub current_window: Option<WindowInfo>,
    pub idle_seconds: Option<u64>,
    pub error: Option<String>,
}

pub struct Control {
    pub session_active: bool,
    pub status: Status,
    pub paused: bool,
    pub pause_started: i64,
    pub shutdown: bool,
    pub pause_generation: u64,
}

#[derive(Clone)]
pub struct Shared {
    pub storage: Arc<Mutex<Storage>>,
    pub control: Arc<Mutex<Control>>,
    pub token: Arc<String>,
    pub idle_threshold: u64,
    pub preferences: Arc<Mutex<preferences::Preferences>>,
    /// Where the end of a timed pause is kept so it survives a restart.
    /// `None` (tests, fixtures) keeps the pause in memory only.
    pause_file: Arc<Mutex<Option<std::path::PathBuf>>>,
    shutdown_signal: Arc<tokio::sync::Notify>,
}

impl Shared {
    pub fn new(storage: Storage, token: String, idle_threshold: u64) -> Self {
        let now = now_ms();
        Self {
            storage: Arc::new(Mutex::new(storage)),
            control: Arc::new(Mutex::new(Control {
                session_active: false,
                status: Status {
                    computer: ComputerState::default(),
                    computer_capabilities:
                        "Lifecycle backend not initialized; off/history reconstruction unavailable"
                            .into(),
                    state: "starting".into(),
                    started_ts: now,
                    updated_ts: now,
                    paused_until: None,
                    backends: vec![],
                    current_window: None,
                    idle_seconds: None,
                    error: None,
                },
                paused: false,
                pause_started: 0,
                shutdown: false,
                pause_generation: 0,
            })),
            token: Arc::new(token),
            idle_threshold,
            preferences: Arc::new(Mutex::new(preferences::Preferences::default())),
            pause_file: Arc::new(Mutex::new(None)),
            shutdown_signal: Arc::new(tokio::sync::Notify::new()),
        }
    }
    /// Keep timed pauses in `path` from now on. Call [`Shared::restore_pause`]
    /// afterwards to continue a pause that was running when the process stopped.
    pub fn set_pause_file(&self, path: std::path::PathBuf) {
        *self.pause_file.lock().unwrap() = Some(path);
    }
    /// Continue a timed pause saved by an earlier process. An expired, missing
    /// or unreadable pause file leaves recording on. Pauses without a duration
    /// are never saved, so a forgotten scripted pause cannot outlive a restart.
    pub fn restore_pause(&self, now: i64) -> Result<bool, String> {
        let Some(until) = self.saved_pause() else {
            return Ok(false);
        };
        if until <= now {
            self.save_pause(None);
            return Ok(false);
        }
        self.pause(now, Some((until - now).min(86_400_000)))?;
        Ok(true)
    }
    fn saved_pause(&self) -> Option<i64> {
        let path = self.pause_file.lock().unwrap().clone()?;
        let bytes = std::fs::read(path).ok()?;
        serde_json::from_slice::<serde_json::Value>(&bytes)
            .ok()?
            .get("paused_until")?
            .as_i64()
    }
    fn save_pause(&self, until: Option<i64>) {
        let Some(path) = self.pause_file.lock().unwrap().clone() else {
            return;
        };
        // A failed write only loses the pause across a restart; recording state
        // in memory stays correct, so the error is not surfaced to the caller.
        let _ = match until {
            None => match std::fs::remove_file(&path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                other => other,
            },
            Some(until) => (|| {
                let parent = path
                    .parent()
                    .ok_or_else(|| std::io::Error::other("no parent"))?;
                let mut file = tempfile::NamedTempFile::new_in(parent)?;
                std::io::Write::write_all(
                    &mut file,
                    json!({ "paused_until": until }).to_string().as_bytes(),
                )?;
                file.persist(&path).map_err(std::io::Error::other)?;
                Ok(())
            })(),
        };
    }
    pub fn status(&self) -> Status {
        self.control.lock().unwrap().status.clone()
    }
    pub fn pause(&self, now: i64, duration_ms: Option<i64>) -> Result<(), String> {
        if duration_ms.is_some_and(|duration| duration <= 0 || duration > 86_400_000) {
            return Err("pause duration must be between 1 ms and 24 hours".into());
        }
        let mut control = self.control.lock().unwrap();
        let mut storage = self.storage.lock().unwrap();
        computer::begin(&mut storage, &mut control, now).map_err(|e| e.to_string())?;
        computer::close_observed(&mut storage, &control.status.computer, now)
            .map_err(|e| e.to_string())?;
        control.status.computer.recording = "paused".into();
        control.status.computer.activity = "unknown".into();
        control.status.computer.window_available = false;
        control.status.computer.reason = Some("Recording paused by user".into());
        storage
            .record_interval(
                "computer",
                &serde_json::to_value(&control.status.computer).unwrap(),
                now,
                now,
            )
            .map_err(|e| e.to_string())?;
        if !control.paused {
            control.pause_started = now;
        }
        control.paused = true;
        control.pause_generation = control.pause_generation.wrapping_add(1);
        control.status.current_window = None;
        control.status.idle_seconds = None;
        control.status.state = "paused".into();
        control.status.paused_until = duration_ms.map(|duration| now.saturating_add(duration));
        let until = control.status.paused_until;
        drop(storage);
        drop(control);
        self.save_pause(until);
        Ok(())
    }
    pub fn shutdown(&self) -> Result<(), StorageError> {
        // Stop polling before flushing; use the same lock order as Tracker.
        let mut control = self.control.lock().unwrap();
        control.shutdown = true;
        let result = computer::stop(&mut self.storage.lock().unwrap(), &mut control, now_ms());
        self.shutdown_signal.notify_one();
        result
    }
    pub async fn wait_for_shutdown(&self) {
        let notified = self.shutdown_signal.notified();
        if !self.control.lock().unwrap().shutdown {
            notified.await;
        }
    }
    pub fn resume(&self) {
        let mut control = self.control.lock().unwrap();
        control.paused = false;
        control.status.paused_until = None;
        control.status.state = "starting".into();
        drop(control);
        self.save_pause(None);
    }
}

pub struct Tracker {
    shared: Shared,
    last_tick: Option<i64>,
    had_window: bool,
    had_idle: bool,
    pause_generation: u64,
}

fn backend(name: &str, error: Option<&WatcherError>) -> BackendStatus {
    BackendStatus {
        name: name.into(),
        state: match error {
            None => "ready",
            Some(WatcherError::Unsupported(_)) => "unsupported",
            Some(WatcherError::PermissionDenied(_)) => "permission_required",
            Some(_) => "error",
        }
        .into(),
        error: error.map(ToString::to_string),
    }
}

impl Tracker {
    pub fn new(shared: Shared) -> Self {
        Self {
            shared,
            last_tick: None,
            had_window: false,
            had_idle: false,
            pause_generation: 0,
        }
    }
    pub fn poll_once(
        &mut self,
        now: i64,
        window: &impl WindowWatcher,
        idle: &impl IdleWatcher,
    ) -> Result<(), StorageError> {
        let mut control = self.shared.control.lock().unwrap();
        if control.shutdown {
            return Ok(());
        }
        {
            let mut storage = self.shared.storage.lock().unwrap();
            computer::begin(&mut storage, &mut control, now)?;
        }
        let expired = control.paused
            && control
                .status
                .paused_until
                .is_some_and(|until| now >= until);
        if expired {
            control.paused = false;
            control.status.paused_until = None;
            control.status.computer.recording = "recording".into();
            control.pause_generation = control.pause_generation.wrapping_add(1);
            self.shared.save_pause(None);
        }
        let generation = control.pause_generation;
        if generation != self.pause_generation {
            self.last_tick = None;
            self.had_window = false;
            self.had_idle = false;
            self.pause_generation = generation;
        }
        use chrono::{Datelike, Timelike};
        let scheduled_pause = {
            let preferences = self.shared.preferences.lock().unwrap();
            preferences.schedule.as_ref().is_some_and(|schedule| {
                chrono::DateTime::from_timestamp_millis(now).is_none_or(|date| {
                    let local = date.with_timezone(&chrono::Local);
                    !schedule.allows(
                        local.weekday().number_from_monday(),
                        local.hour() * 60 + local.minute(),
                    )
                })
            })
        };
        if scheduled_pause && !control.paused {
            control.status.computer.recording = "paused".into();
            control.status.computer.reason = Some("Outside recording schedule".into());
        }
        let blocked = scheduled_pause
            || control.paused
            || control.status.computer.machine != "awake"
            || control.status.computer.locked == Some(true);
        if blocked {
            control.status.updated_ts = now;
            control.status.current_window = None;
            let mut storage = self.shared.storage.lock().unwrap();
            let max_gap = computer::continuation_gap(&storage, &control.status.computer)?;
            let mut batch = storage.buffered_batch(now);
            batch.extend_last("computer", now, max_gap)?;
            batch.heartbeat(
                "computer",
                &serde_json::to_value(&control.status.computer).unwrap(),
                now,
                3000,
            )?;
            batch.commit()?;
            computer::update_status(&mut control.status);
            self.last_tick = None;
            self.had_window = false;
            self.had_idle = false;
            return Ok(());
        }
        drop(control);
        let idle = idle.idle_seconds();
        let away = idle
            .as_ref()
            .is_ok_and(|seconds| *seconds > self.shared.idle_threshold);
        let window = if away {
            Ok(None)
        } else {
            window.active_window()
        };
        let observed_window = window.as_ref().is_ok_and(|w| w.is_some());
        let window = window.map(|window| self.shared.preferences.lock().unwrap().filter(window));
        let excluded = observed_window && window.as_ref().is_ok_and(|w| w.is_none());
        let mut control = self.shared.control.lock().unwrap();
        if control.shutdown || control.paused || control.pause_generation != generation {
            return Ok(());
        }
        control.status.backends = vec![
            backend("window", window.as_ref().err()),
            backend("idle", idle.as_ref().err()),
        ];
        control.status.current_window = window.as_ref().ok().cloned().flatten();
        control.status.idle_seconds = idle.as_ref().ok().copied();
        control.status.updated_ts = now;
        control.status.error = None;
        let previous = (self.had_window, self.had_idle);
        let mut storage = self.shared.storage.lock().unwrap();
        if self
            .last_tick
            .is_some_and(|last| now.saturating_sub(last) >= 3000)
        {
            if let Some(last) = storage.last_event("computer")? {
                if last.end_ts < now {
                    computer::unknown_gap(&mut storage, last.end_ts, now)?;
                }
            }
        }
        let mut batch = storage.buffered_batch(now);
        if control.status.computer.recording == "paused" {
            batch.extend_last("computer", now, i64::MAX)?;
        }
        if self
            .last_tick
            .is_some_and(|last| now >= last && now - last < 3000)
        {
            if self.had_window {
                batch.extend_last("window", now, 3000)?;
            }
            if self.had_idle {
                batch.extend_last("afk", now, 3000)?;
            }
            batch.extend_last("computer", now, 3000)?;
        }
        let pulse = if self.last_tick.is_none() { 1 } else { 3000 };
        self.had_idle = idle.is_ok();
        if idle.is_ok() {
            batch.heartbeat(
                "afk",
                &json!({"status":if away {"away"}else{"active"}}),
                now,
                pulse,
            )?;
        }
        self.had_window = false;
        if !away && idle.is_ok() {
            if let Ok(Some(window)) = &window {
                batch.heartbeat("window", &serde_json::to_value(window).unwrap(), now, pulse)?;
                self.had_window = true;
            }
        }
        let mut state = control.status.computer.clone();
        state.machine = "awake".into();
        state.activity = if away {
            "away"
        } else if idle.is_ok() {
            "active"
        } else {
            "unknown"
        }
        .into();
        state.window_available = self.had_window;
        let mut failures = Vec::new();
        if let Err(error) = &idle {
            failures.push(format!("idle: {error}"));
        }
        if !away {
            match &window {
                Err(error) => failures.push(format!("window: {error}")),
                Ok(None) if !excluded => failures.push("window: no focused window reported".into()),
                _ => {}
            }
        }
        state.recording = if failures.is_empty() {
            "recording"
        } else {
            "unobserved"
        }
        .into();
        state.reason = (!failures.is_empty()).then(|| failures.join("; "));
        batch.heartbeat(
            "computer",
            &serde_json::to_value(&state).unwrap(),
            now,
            pulse,
        )?;
        batch.commit()?;
        if previous != (self.had_window, self.had_idle) {
            storage.flush()?;
        }
        if let Some(last) = self
            .last_tick
            .filter(|last| now > *last && now - *last < 3000)
        {
            storage.auto_tag(last, now)?;
        }
        control.status.computer = state;
        computer::update_status(&mut control.status);
        self.last_tick = Some(now);
        Ok(())
    }
}

#[cfg(test)]
mod history_tests {
    use super::*;
    #[test]
    fn cooperative_history_matches_storage_across_chunks_and_spanning_events() {
        let mut db = Storage::open_in_memory().unwrap();
        for i in 0..8300 {
            db.record_interval("afk", &json!({"status":"active"}), i * 10 + 1, i * 10 + 9)
                .unwrap();
        }
        db.record_interval("window", &json!({"app":"Editor"}), 1, 83_000)
            .unwrap();
        db.tag_interval("Work", "note", 1, 83_000, false).unwrap();
        db.record_interval("computer", &json!({"machine":"awake","activity":"active","recording":"recording","window_available":true}), 82_000, 82_999).unwrap();
        let report = db.report(1, 83_000).unwrap();
        let archive = db.export_archive(1, 83_000).unwrap();
        let shared = Shared::new(db, "test".into(), 300);
        let actual = shared.report(1, 83_000).unwrap();
        assert_eq!(actual.active_ms, report.active_ms);
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::to_value(report).unwrap()
        );
        assert_eq!(
            serde_json::to_value(shared.export_archive(1, 83_000).unwrap()).unwrap(),
            serde_json::to_value(archive).unwrap()
        );
        assert!(shared.report(-1, 2).is_err());
        assert!(shared.export_archive(2, 1).is_err());
    }
}
