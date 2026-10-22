use super::model::{self, Range, Track};
use crate::icons;
use draugr_time_core::{Storage, Tag, TagRange};
use draugr_time_daemon::{now_ms, paths, Shared, Tracker};
use draugr_time_watchers::{
    IdleWatcher, MockWatcher, NativeWatcher, WatcherError, WindowInfo, WindowWatcher,
};
use std::{
    collections::HashMap,
    sync::mpsc,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Default)]
pub struct LoadCursor {
    pub starts: HashMap<String, i64>,
    pub metadata_version: Option<(u64, i64)>,
    pub icons: std::collections::HashSet<String>,
}
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub completed_afk: Option<Range>,
    pub tag_metadata: Option<Box<TagMetadata>>,
    pub tracks: Vec<Track>,
    pub cursor: Box<LoadCursor>,
    pub replace_from: HashMap<String, i64>,
    pub tags_changed: bool,
    pub icons: HashMap<String, Option<icons::IconData>>,
}
#[derive(Clone, Debug)]
pub struct TagMetadata {
    pub tags: Vec<Tag>,
    pub ranges: Vec<TagRange>,
    pub recent: Vec<i64>,
}
struct LoadRequest {
    range: Range,
    cursor: LoadCursor,
    result: tokio::sync::oneshot::Sender<Result<Snapshot, String>>,
}
pub struct Loader {
    sender: mpsc::SyncSender<LoadRequest>,
}
impl Loader {
    fn new(shared: Shared) -> Self {
        let (sender, receiver) = mpsc::sync_channel::<LoadRequest>(1);
        std::thread::spawn(move || {
            while let Ok(request) = receiver.recv() {
                if !request.result.is_closed() {
                    let _ = request.result.send(Runtime::load(
                        shared.clone(),
                        request.range,
                        request.cursor,
                    ));
                }
            }
        });
        Self { sender }
    }
    pub fn load(
        &self,
        range: Range,
        cursor: LoadCursor,
    ) -> impl std::future::Future<Output = Result<Snapshot, String>> + Send + 'static {
        let (result, receiver) = tokio::sync::oneshot::channel();
        let submitted = self
            .sender
            .try_send(LoadRequest {
                range,
                cursor,
                result,
            })
            .map_err(|error| error.to_string());
        async move {
            submitted?;
            receiver.await.map_err(|error| error.to_string())?
        }
    }
}
pub struct Runtime {
    #[cfg(target_os = "macos")]
    _computer: Option<draugr_time_watchers::NativeComputerWatcher>,
    pub shared: Shared,
    pub loader: Loader,
    #[cfg(target_os = "macos")]
    sampler: Sampler,
    #[cfg(target_os = "macos")]
    sender: mpsc::SyncSender<(i64, MockWatcher)>,
}
impl Runtime {
    #[cfg(test)]
    pub fn for_test(shared: Shared) -> Self {
        Self {
            loader: Loader::new(shared.clone()),
            shared,
            #[cfg(target_os = "macos")]
            _computer: None,
            #[cfg(target_os = "macos")]
            sampler: Sampler::new(true),
            #[cfg(target_os = "macos")]
            sender: mpsc::sync_channel(1).0,
        }
    }

    pub fn start(fixture: bool) -> Result<Self, String> {
        let directory = paths::data_dir().map_err(|e| e.to_string())?;
        let listener = std::net::TcpListener::bind((
            std::net::Ipv4Addr::LOCALHOST,
            if fixture { 0 } else { paths::PORT },
        ))
        .map_err(|_| {
            "Another tracker is using the local API port. Quit it, then press Start recording."
                .to_string()
        })?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let token = paths::token(&directory.join("api-token")).map_err(|e| e.to_string())?;
        let shared = Shared::new(
            Storage::open(directory.join("activity.sqlite")).map_err(|e| e.to_string())?,
            token,
            300,
        );
        *shared.preferences.lock().unwrap() =
            draugr_time_daemon::preferences::Preferences::load(&directory.join("settings.json"))
                .map_err(|e| e.to_string())?;
        shared
            .begin_recording(now_ms())
            .map_err(|e| e.to_string())?;
        #[cfg(target_os = "macos")]
        let computer = if fixture {
            None
        } else {
            draugr_time_daemon::service::start_computer(&shared)
        };
        draugr_time_daemon::service::spawn_api(shared.clone(), listener);
        let (sender, _) = spawn_worker(shared.clone(), fixture);
        #[cfg(not(target_os = "macos"))]
        drop(sender);
        Ok(Self {
            #[cfg(target_os = "macos")]
            _computer: computer,
            loader: Loader::new(shared.clone()),
            shared,
            #[cfg(target_os = "macos")]
            sampler: Sampler::new(fixture),
            #[cfg(target_os = "macos")]
            sender,
        })
    }
    #[cfg(target_os = "macos")]
    pub fn pulse(&mut self) {
        let Some(blocked) = self
            .shared
            .control
            .try_lock()
            .ok()
            .map(|control| Sampler::blocked(&control.status, control.paused, now_ms()))
        else {
            return;
        };
        let sample = self.sampler.sample_blocked(blocked);
        let _ = self.sender.try_send((now_ms(), sample));
    }
    pub fn status(&self) -> Option<draugr_time_daemon::Status> {
        self.shared
            .control
            .try_lock()
            .ok()
            .map(|control| control.status.clone())
    }
    pub fn pause(&self, duration: Option<i64>) -> Result<(), String> {
        self.shared.pause(now_ms(), duration)
    }
    pub fn resume(&self) {
        self.shared.resume();
    }
    pub fn load(shared: Shared, range: Range, mut cursor: LoadCursor) -> Result<Snapshot, String> {
        let storage = shared.storage.lock().unwrap();
        let completed_afk = storage
            .completed_afk()
            .map_err(|e| e.to_string())?
            .map(|(start, end)| Range::new(start, end));
        let version = storage.metadata_version().map_err(|e| e.to_string())?;
        let tags_changed = cursor.metadata_version != Some(version);
        // An external writer may have edited history rather than appended a heartbeat.
        if cursor
            .metadata_version
            .is_some_and(|old| old.1 != version.1)
        {
            cursor.starts.clear();
        }
        let mut events = Vec::new();
        let mut replace_from = HashMap::new();
        let computer_start = storage
            .first_event_start("computer")
            .map_err(|e| e.to_string())?;
        for bucket in ["afk", "window", "web", "computer"] {
            let from = cursor.starts.get(bucket).copied().unwrap_or(range.start);
            replace_from.insert(bucket.into(), from.max(range.start));
            let to = if bucket == "afk" {
                computer_start.unwrap_or(range.end).min(range.end)
            } else {
                range.end
            };
            let mut tail = if to <= from.max(range.start) {
                Vec::new()
            } else {
                storage
                    .events(Some(bucket), from.max(range.start), to)
                    .map_err(|e| e.to_string())?
            };
            if bucket == "afk" {
                for event in &mut tail {
                    event.end_ts = event.end_ts.min(to);
                }
            }
            tail.retain(|event| event.start_ts >= from || from == range.start);
            if let Some(last) = tail.last() {
                cursor.starts.insert(bucket.into(), last.start_ts);
            }
            events.extend(tail);
        }
        let (tags, ranges) = if tags_changed {
            (
                storage.tags().map_err(|e| e.to_string())?,
                storage
                    .tag_ranges(range.start, range.end)
                    .map_err(|e| e.to_string())?,
            )
        } else {
            (Vec::new(), Vec::new())
        };
        let recent = if tags_changed {
            storage.recent_tag_ids().map_err(|e| e.to_string())?
        } else {
            Vec::new()
        };
        drop(storage);
        cursor.metadata_version = Some(version);
        let mut tracks = model::tracks(&events, &tags, &ranges, range);
        for track in &mut tracks {
            track.segments.retain(|s| s.bucket != "derived");
        }
        let apps: Vec<_> = tracks[2]
            .segments
            .iter()
            .filter_map(|segment| {
                Some((
                    segment.app.as_ref()?.to_string(),
                    segment.app_name.as_ref()?.to_string(),
                ))
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .filter(|(process, name)| {
                cursor
                    .icons
                    .insert(model::application_identity(process, name))
            })
            .collect();
        Ok(Snapshot {
            completed_afk,
            tag_metadata: tags_changed.then(|| {
                Box::new(TagMetadata {
                    tags,
                    ranges,
                    recent,
                })
            }),
            tracks,
            cursor: Box::new(cursor),
            replace_from,
            tags_changed,
            icons: icons::application_icons(apps),
        })
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        if let Err(error) = self.shared.shutdown() {
            eprintln!("Shutdown flush failed: {error}");
        }
    }
}

struct Sampler {
    source: Result<NativeWatcher, WatcherError>,
    fixture: bool,
    retry: Instant,
}
impl Sampler {
    fn new(fixture: bool) -> Self {
        Self {
            source: if fixture {
                Err(WatcherError::Unsupported("Test fixture"))
            } else {
                NativeWatcher::new()
            },
            fixture,
            retry: Instant::now(),
        }
    }
    /// Whether this tick must not observe the desktop (asleep, locked or paused).
    fn blocked(status: &draugr_time_daemon::Status, paused: bool, now: i64) -> bool {
        status.computer.machine != "awake"
            || status.computer.locked == Some(true)
            || paused && !status.paused_until.is_some_and(|until| until <= now)
    }
    #[cfg(test)]
    fn sample(&mut self, status: &draugr_time_daemon::Status, paused: bool) -> MockWatcher {
        self.sample_blocked(Self::blocked(status, paused, now_ms()))
    }
    fn sample_blocked(&mut self, blocked: bool) -> MockWatcher {
        if blocked {
            MockWatcher {
                window: Err(WatcherError::Platform("Paused".into())),
                idle: Err(WatcherError::Platform("Paused".into())),
            }
        } else if self.fixture {
            MockWatcher::new(
                Some(WindowInfo {
                    app: std::env::current_exe()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|_| "Runtime fixture".into()),
                    app_name: "Draugr Time".into(),
                    title: "Iced runtime fixture".into(),
                    pid: Some(std::process::id()),
                }),
                0,
            )
        } else {
            if self.source.is_err() && self.retry.elapsed() > Duration::from_secs(10) {
                self.source = NativeWatcher::new();
                self.retry = Instant::now();
            }
            match &self.source {
                Ok(source) => MockWatcher {
                    window: source.active_window(),
                    idle: source.idle_seconds(),
                },
                Err(error) => MockWatcher {
                    window: Err(error.clone()),
                    idle: Err(error.clone()),
                },
            }
        }
    }
}

/// Delay after resume before trimming, so the post-wake burst has finished.
const WAKE_RECLAIM_DELAY: Duration = Duration::from_secs(20);
/// Background trim for a long-running tray process; malloc_trim is cheap at this rate.
const RECLAIM_PERIOD: Duration = Duration::from_secs(30 * 60);

fn spawn_worker(
    shared: Shared,
    fixture: bool,
) -> (
    mpsc::SyncSender<(i64, MockWatcher)>,
    std::thread::JoinHandle<()>,
) {
    let (sender, receiver) = mpsc::sync_channel(1);
    let handle = std::thread::spawn(move || {
        #[cfg(not(target_os = "macos"))]
        let computer = if fixture {
            None
        } else {
            draugr_time_daemon::service::start_computer(&shared)
        };
        let mut tracker = Tracker::new(shared.clone());
        #[cfg(not(target_os = "macos"))]
        let mut sampler = Sampler::new(fixture);
        #[cfg(not(target_os = "macos"))]
        let _receiver = receiver;
        #[cfg(target_os = "macos")]
        let _ = fixture;
        let mut last_status = None;
        let mut last_machine = String::from("awake");
        let mut reclaim_at = Some(Instant::now() + RECLAIM_PERIOD);
        while !shared.control.lock().unwrap().shutdown {
            #[cfg(not(target_os = "macos"))]
            let tick_started = Instant::now();
            #[cfg(not(target_os = "macos"))]
            let (now, sample) = {
                // Read the gate under the lock instead of cloning the whole status.
                let blocked = {
                    let control = shared.control.lock().unwrap();
                    Sampler::blocked(&control.status, control.paused, now_ms())
                };
                let sample = sampler.sample_blocked(blocked);
                (now_ms(), sample)
            };
            #[cfg(target_os = "macos")]
            let (now, sample) = match receiver.recv_timeout(Duration::from_millis(200)) {
                Ok(sample) => sample,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };
            if let Err(error) = tracker.poll_once(now, &sample, &sample) {
                let mut control = shared.control.lock().unwrap();
                control.status.state = "error".into();
                control.status.error = Some(error.to_string());
            }
            #[cfg(not(target_os = "macos"))]
            if let Some(computer) = &computer {
                use draugr_time_watchers::ComputerWatcher;
                let capabilities = computer.capabilities();
                let mut control = shared.control.lock().unwrap();
                if control.status.computer_capabilities != capabilities {
                    control.status.computer_capabilities = capabilities;
                }
            }
            // One status copy per tick: it doubles as the change-detection key and is
            // only cloned again when the tray actually needs a new status.
            let mut comparison = shared.status();
            let (updated_ts, idle_seconds) = (comparison.updated_ts, comparison.idle_seconds);
            comparison.updated_ts = 0;
            comparison.idle_seconds = None;
            if comparison.computer.machine == "awake" && last_machine != "awake" {
                // Wake and the display reconfiguration that follows touch many
                // threads at once; return their freed pages once things settle.
                reclaim_at = Some(Instant::now() + WAKE_RECLAIM_DELAY);
            }
            if last_machine != comparison.computer.machine {
                last_machine.clone_from(&comparison.computer.machine);
            }
            if reclaim_at.is_some_and(|at| Instant::now() >= at) {
                use super::memory::MemoryReclaimer;
                super::memory::SystemReclaimer.reclaim();
                reclaim_at = Some(Instant::now() + RECLAIM_PERIOD);
            }
            if last_status.as_ref() != Some(&comparison) {
                let mut status = comparison.clone();
                status.updated_ts = updated_ts;
                status.idle_seconds = idle_seconds;
                super::tray::push_status(status);
                last_status = Some(comparison);
            }
            #[cfg(not(target_os = "macos"))]
            std::thread::sleep(Duration::from_secs(1).saturating_sub(tick_started.elapsed()));
        }
    });
    (sender, handle)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn recording_continues_without_gui_pulses() {
        let shared = Shared::new(Storage::open_in_memory().unwrap(), "test".into(), 300);
        let (sender, worker) = spawn_worker(shared.clone(), true);
        drop(sender);
        std::thread::sleep(Duration::from_millis(2200));
        shared.shutdown().unwrap();
        worker.join().unwrap();
        let events = shared
            .storage
            .lock()
            .unwrap()
            .events(Some("afk"), 0, i64::MAX)
            .unwrap();
        assert_eq!(
            events.len(),
            1,
            "background recording must not need GUI pulses"
        );
        assert!(events[0].end_ts - events[0].start_ts >= 1000);
        let windows = shared
            .storage
            .lock()
            .unwrap()
            .events(Some("window"), 0, i64::MAX)
            .unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].start_ts, events[0].start_ts);
        assert_eq!(windows[0].end_ts, events[0].end_ts);
    }
}

#[cfg(test)]
mod incremental_tests {
    use super::*;
    use draugr_time_core::Collection;
    use serde_json::json;

    #[test]
    fn incremental_load_replaces_tail_and_refreshes_tag_revision() {
        let shared = Shared::new(Storage::open_in_memory().unwrap(), "test".into(), 300);
        let day = Range::new(0, 100_000);
        {
            let mut storage = shared.storage.lock().unwrap();
            for (ts, app) in [(1000, "a"), (2000, "a"), (3000, "b"), (4000, "b")] {
                storage
                    .heartbeat("window", &json!({"app":app,"title":app}), ts, 3000)
                    .unwrap();
            }
        }
        let first = Runtime::load(shared.clone(), day, LoadCursor::default()).unwrap();
        assert_eq!(first.tracks[2].segments.len(), 2);
        shared
            .storage
            .lock()
            .unwrap()
            .heartbeat("window", &json!({"app":"b","title":"b"}), 5000, 3000)
            .unwrap();
        let next = Runtime::load(shared.clone(), day, *first.cursor).unwrap();
        assert_eq!(next.tracks[2].segments.len(), 1, "load only the last event");
        assert_eq!(next.tracks[2].segments[0].range.end, 5000);
        assert!(!next.tags_changed);
        shared
            .storage
            .lock()
            .unwrap()
            .put(Collection::Tags, None, json!({"name":"Project"}))
            .unwrap();
        let next = Runtime::load(shared, day, *next.cursor).unwrap();
        assert!(next.tags_changed);
    }
}

#[cfg(test)]
mod memory_tests {
    use super::*;
    #[tokio::test]
    async fn worker_loads_only_day_ranges_and_recovers_after_invalid_request() {
        let mut storage = Storage::open_in_memory().unwrap();
        storage.tag_interval("Old", "", 1, 10, false).unwrap();
        storage
            .tag_interval("Current", "", 100, 200, false)
            .unwrap();
        let shared = Shared::new(storage, "test".into(), 300);
        let loader = Loader::new(shared);
        assert!(loader
            .load(Range::new(-1, 0), Default::default())
            .await
            .is_err());
        let snapshot = loader
            .load(Range::new(150, 250), Default::default())
            .await
            .unwrap();
        let metadata = snapshot.tag_metadata.unwrap();
        assert_eq!(metadata.ranges.len(), 1);
        assert_eq!(metadata.ranges[0].start_ts, 100);
        assert_eq!(metadata.recent.len(), 2);
    }
}

#[cfg(test)]
mod contention_tests {
    use super::*;
    #[test]
    fn sampler_obeys_paused_snapshot_even_when_status_has_an_error() {
        let mut status = draugr_time_daemon::Status::default();
        status.computer.machine = "awake".into();
        status.computer.recording = "paused".into();
        status.state = "error".into();
        let mut sampler = Sampler::new(true);
        assert!(sampler.sample(&status, true).window.is_err());
        // Resume clears control.paused before the next tick updates computer.recording.
        assert!(sampler.sample(&status, false).window.unwrap().is_some());
    }
    #[test]
    fn status_does_not_wait_for_tracker_control() {
        let shared = Shared::new(Storage::open_in_memory().unwrap(), "test".into(), 300);
        let runtime = Runtime::for_test(shared.clone());
        let control = shared.control.lock().unwrap();
        assert!(runtime.status().is_none());
        drop(control);
        assert!(runtime.status().is_some());
    }
}
