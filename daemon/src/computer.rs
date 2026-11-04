use crate::{Control, Shared, Status};
use draugr_time_core::{Storage, StorageError};
use draugr_time_watchers::{LifecycleEvent, LifecycleKind};
use serde::{Deserialize, Serialize};

/// Independent dimensions: a missing title does not erase valid idle evidence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComputerState {
    pub machine: String,
    pub activity: String,
    pub recording: String,
    pub locked: Option<bool>,
    pub window_available: bool,
    pub source: String,
    pub reason: Option<String>,
}
impl Default for ComputerState {
    fn default() -> Self {
        Self {
            machine: "awake".into(),
            activity: "unknown".into(),
            recording: "recording".into(),
            locked: None,
            window_available: false,
            source: "running process".into(),
            reason: None,
        }
    }
}
fn data(state: &ComputerState) -> serde_json::Value {
    serde_json::to_value(state).expect("computer state serializes")
}
pub(crate) fn close_observed(
    storage: &mut Storage,
    state: &ComputerState,
    now: i64,
) -> Result<(), StorageError> {
    let gap = continuation_gap(storage, state)?;
    let mut batch = storage.buffered_batch(now);
    if state.machine == "awake"
        && state.locked != Some(true)
        && ["recording", "unobserved"].contains(&state.recording.as_str())
    {
        if state.window_available {
            batch.extend_last("window", now, 3000)?;
        }
        if state.activity != "unknown" {
            batch.extend_last("afk", now, 3000)?;
        }
    }
    batch.extend_last("computer", now, gap)?;
    batch.commit()?;
    storage.flush()
}
pub(crate) fn continuation_gap(
    storage: &Storage,
    state: &ComputerState,
) -> Result<i64, StorageError> {
    if state.machine == "asleep" || state.locked == Some(true) {
        let same = storage.last_event("computer")?.is_some_and(|e| {
            e.data["machine"] == state.machine
                && e.data["locked"] == serde_json::to_value(state.locked).unwrap()
        });
        // A failed pre-sleep write must never extend an old active row over sleep.
        Ok(if same { i64::MAX } else { 0 })
    } else {
        Ok(3000)
    }
}
pub(crate) fn begin(
    storage: &mut Storage,
    control: &mut Control,
    now: i64,
) -> Result<(), StorageError> {
    recover_clock(storage, control, now)?;
    if control.session_active {
        return Ok(());
    }
    if let Some(last) = storage.last_event("computer")? {
        if now < last.end_ts {
            return Err(StorageError::OutOfOrderHeartbeat);
        }
        if last.end_ts < now {
            let stopped = last.data["recording"] == "stopped";
            let gap = ComputerState {
                machine: "unknown".into(),
                recording: if stopped { "stopped" } else { "unknown" }.into(),
                source: "tracker lifecycle".into(),
                reason: Some(
                    if stopped {
                        "Tracker was stopped; computer power state is unknown"
                    } else {
                        "Recording gap after interruption; no verified power history"
                    }
                    .into(),
                ),
                ..ComputerState::default()
            };
            storage.record_interval("computer", &data(&gap), last.end_ts, now)?;
        }
    }
    control.session_active = true;
    control.status.computer = ComputerState::default();
    Ok(())
}
fn recover_clock(
    storage: &mut Storage,
    control: &mut Control,
    now: i64,
) -> Result<(), StorageError> {
    // Sub-poll differences can be delayed notifications rather than a clock reset.
    let mut previous = storage.last_event("computer")?.map(|event| event.end_ts);
    if !control.session_active {
        for bucket in ["window", "afk"] {
            previous = previous.max(storage.last_event(bucket)?.map(|event| event.end_ts));
        }
    }
    if let Some(previous) = previous.filter(|previous| previous.saturating_sub(now) >= 3000) {
        if let Some(path) = storage.recover_clock_rollback(now)? {
            control.status.clock_recovery_backup = Some(path);
            control.status.error = None;
            control.status.updated_ts = now;
            control.status.current_window = None;
            control.status.idle_seconds = None;
            control.pause_generation = control.pause_generation.wrapping_add(1);
            control.pause_started = now;
            if let Some(until) = &mut control.status.paused_until {
                *until = now.saturating_add(until.saturating_sub(previous).max(0));
            }
        }
    }
    Ok(())
}
pub(crate) fn stop(
    storage: &mut Storage,
    control: &mut Control,
    now: i64,
) -> Result<(), StorageError> {
    if !control.session_active {
        return Ok(());
    }
    recover_clock(storage, control, now)?;
    close_observed(storage, &control.status.computer, now)?;
    let state = ComputerState {
        machine: "unknown".into(),
        recording: "stopped".into(),
        source: "tracker lifecycle".into(),
        reason: Some("Tracker stopped by user; this is not an OS shutdown".into()),
        ..ComputerState::default()
    };
    storage.record_interval("computer", &data(&state), now, now)?;
    control.status.computer = state;
    control.session_active = false;
    Ok(())
}
pub(crate) fn unknown_gap(storage: &mut Storage, from: i64, to: i64) -> Result<(), StorageError> {
    let state = ComputerState {
        machine: "unknown".into(),
        recording: "unknown".into(),
        source: "sampling gap".into(),
        reason: Some("No samples or verified OS lifecycle boundary; power state is unknown".into()),
        ..ComputerState::default()
    };
    storage.record_interval("computer", &data(&state), from, to)
}
impl Shared {
    pub fn stop_recording(&self, now: i64) -> Result<(), StorageError> {
        let mut control = self.control.lock().unwrap();
        stop(&mut self.storage.lock().unwrap(), &mut control, now)
    }
    pub fn begin_recording(&self, now: i64) -> Result<(), StorageError> {
        let mut control = self.control.lock().unwrap();
        begin(&mut self.storage.lock().unwrap(), &mut control, now)
    }
    pub fn lifecycle(&self, event: LifecycleEvent) -> Result<(), StorageError> {
        let mut control = self.control.lock().unwrap();
        if control.shutdown {
            return Ok(());
        }
        let mut storage = self.storage.lock().unwrap();
        if !control.session_active {
            begin(&mut storage, &mut control, event.ts)?;
        }
        let ts = storage
            .last_event("computer")?
            .map_or(event.ts, |e| event.ts.max(e.end_ts));
        let previous = control.status.computer.clone();
        let mut state = control.status.computer.clone();
        state.source = event.source;
        state.window_available = false;
        state.activity = "unknown".into();
        state.reason = None;
        match event.kind {
            LifecycleKind::Unavailable => {
                state.machine = "awake".into();
                state.locked = None;
                state.reason = Some(format!("computer lifecycle backend: {}", state.source));
                control.status.computer_capabilities = state.source.clone();
            }
            LifecycleKind::Sleep => {
                state.machine = "asleep".into();
                state.reason = Some(
                    "OS suspend preparation to resume; notification receipt boundaries".into(),
                );
            }
            LifecycleKind::Wake => {
                state.machine = "awake".into();
            }
            LifecycleKind::Lock => {
                state.locked = Some(true);
            }
            LifecycleKind::Unlock => {
                state.locked = Some(false);
            }
            LifecycleKind::ShutdownRequested => {
                state.machine = "awake".into();
                state.reason =
                    Some("OS shutdown requested; completed power-off is unverified".into());
            }
            LifecycleKind::ShutdownCancelled => {
                state.machine = "awake".into();
            }
        }
        state.recording = if control.paused {
            "paused"
        } else {
            "recording"
        }
        .into();
        let persisted = close_observed(&mut storage, &previous, ts)
            .and_then(|_| storage.record_interval("computer", &data(&state), ts, ts));
        control.pause_generation = control.pause_generation.wrapping_add(1);
        control.pause_started = ts;
        control.status.current_window = None;
        control.status.updated_ts = ts;
        control.status.computer = state;
        update_status(&mut control.status);
        persisted
    }
}
pub(crate) fn update_status(status: &mut Status) {
    let state = &status.computer;
    status.state = if state.recording == "paused" {
        "paused"
    } else if state.machine == "asleep" {
        "asleep"
    } else if state.locked == Some(true) {
        "locked"
    } else if state.recording == "unobserved" {
        "error"
    } else if state.activity == "away" {
        "away"
    } else if state.activity == "unknown" {
        "starting"
    } else {
        "tracking"
    }
    .into();
}
