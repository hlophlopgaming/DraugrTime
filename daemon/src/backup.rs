//! Backups of the history database: on request, automatically on a schedule,
//! and restoring one on the next launch.
//!
//! Backups live in `<data folder>/backups`. Names carry their kind and local
//! time, e.g. `draugr-time-auto-2026-11-01_093000.sqlite`. Only automatic
//! backups are pruned; manual and pre-restore copies stay until the user
//! deletes them.

use crate::Shared;
use serde::{Deserialize, Serialize};
use std::{
    io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

const PREFIX: &str = "draugr-time-";
const SUFFIX: &str = ".sqlite";
const PENDING: &str = "restore-pending.sqlite";
const DATABASE: &str = "activity.sqlite";
const DAY_MS: i64 = 86_400_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Interval {
    Daily,
    Weekly,
}
impl Interval {
    pub fn ms(self) -> i64 {
        match self {
            Self::Daily => DAY_MS,
            Self::Weekly => 7 * DAY_MS,
        }
    }
}

/// Stored in `backup.json`, separate from `settings.json` so older builds
/// keep reading their settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub automatic: bool,
    pub interval: Interval,
    /// Automatic backups to keep; older ones are deleted.
    pub keep: usize,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            automatic: true,
            interval: Interval::Daily,
            keep: 7,
        }
    }
}
pub const KEEP_CHOICES: [usize; 4] = [3, 7, 14, 30];

impl Settings {
    pub fn load(data_dir: &Path) -> io::Result<Self> {
        match std::fs::read(data_dir.join("backup.json")) {
            Ok(bytes) => {
                let settings: Self = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
                Ok(Self {
                    keep: settings.keep.clamp(1, 365),
                    ..settings
                })
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error),
        }
    }
    pub fn save(&self, data_dir: &Path) -> io::Result<()> {
        std::fs::create_dir_all(data_dir)?;
        let mut file = tempfile::NamedTempFile::new_in(data_dir)?;
        io::Write::write_all(&mut file, &serde_json::to_vec_pretty(self)?)?;
        file.as_file().sync_all()?;
        file.persist(data_dir.join("backup.json"))
            .map_err(io::Error::other)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Manual,
    Auto,
    /// The history that was replaced by a restore.
    BeforeRestore,
}
impl Kind {
    fn tag(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Auto => "auto",
            Self::BeforeRestore => "before-restore",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Backup {
    pub path: PathBuf,
    pub kind: Kind,
    /// Local time from the name, `YYYY-MM-DD HH:MM:SS`.
    pub label: String,
    /// UTC milliseconds from the name; used for ordering and the schedule.
    pub created: i64,
    pub bytes: u64,
}

pub fn directory(data_dir: &Path) -> PathBuf {
    data_dir.join("backups")
}

fn local(ts: i64) -> chrono::DateTime<chrono::Local> {
    chrono::DateTime::from_timestamp_millis(ts)
        .unwrap_or_default()
        .with_timezone(&chrono::Local)
}

fn file_name(kind: Kind, ts: i64) -> String {
    format!(
        "{PREFIX}{}-{}{SUFFIX}",
        kind.tag(),
        local(ts).format("%Y-%m-%d_%H%M%S")
    )
}

fn parse(path: &Path) -> Option<Backup> {
    let name = path.file_name()?.to_str()?;
    let rest = name.strip_prefix(PREFIX)?.strip_suffix(SUFFIX)?;
    let (kind, stamp) = [Kind::BeforeRestore, Kind::Manual, Kind::Auto]
        .into_iter()
        .find_map(|kind| Some((kind, rest.strip_prefix(kind.tag())?.strip_prefix('-')?)))?;
    // A same-second collision adds "-2", "-3", … after the time.
    let stamp = stamp.get(..17)?;
    let naive = chrono::NaiveDateTime::parse_from_str(stamp, "%Y-%m-%d_%H%M%S").ok()?;
    let created = naive
        .and_local_timezone(chrono::Local)
        .earliest()?
        .timestamp_millis();
    Some(Backup {
        path: path.to_owned(),
        kind,
        label: naive.format("%Y-%m-%d %H:%M:%S").to_string(),
        created,
        bytes: std::fs::metadata(path).ok()?.len(),
    })
}

/// Backups in `data_dir`, newest first. Unrelated files are ignored.
pub fn list(data_dir: &Path) -> Vec<Backup> {
    let mut backups: Vec<Backup> = std::fs::read_dir(directory(data_dir))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| parse(&entry.path()))
        .collect();
    backups.sort_by(|a, b| b.created.cmp(&a.created).then(b.path.cmp(&a.path)));
    backups
}

/// Back up the history now. The storage lock is held only to write buffered
/// event ends; the copy runs from the file through a separate connection.
pub fn create(shared: &Shared, data_dir: &Path, kind: Kind, now: i64) -> Result<Backup, String> {
    let dir = directory(data_dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let base = file_name(kind, now);
    let mut path = dir.join(&base);
    for n in 2.. {
        if !path.exists() {
            break;
        }
        path = dir.join(base.replace(SUFFIX, &format!("-{n}{SUFFIX}")));
    }
    let partial = path.with_extension("sqlite.partial");
    let _ = std::fs::remove_file(&partial);
    let result = (|| {
        let source = shared
            .storage
            .lock()
            .unwrap()
            .backup_source()
            .map_err(|e| e.to_string())?;
        match source {
            Some(source) => draugr_time_core::backup_file(&source, &partial),
            None => shared.storage.lock().unwrap().backup_to(&partial),
        }
        .map_err(|e| e.to_string())?;
        std::fs::rename(&partial, &path).map_err(|e| e.to_string())
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(&partial);
        return Err(error);
    }
    parse(&path).ok_or_else(|| "backup name could not be read back".into())
}

/// Delete automatic backups beyond the newest `keep`.
pub fn prune(data_dir: &Path, keep: usize) -> io::Result<usize> {
    let mut removed = 0;
    for backup in list(data_dir)
        .into_iter()
        .filter(|b| b.kind == Kind::Auto)
        .skip(keep.max(1))
    {
        std::fs::remove_file(&backup.path)?;
        removed += 1;
    }
    Ok(removed)
}

/// Whether an automatic backup should run now.
pub fn due(settings: &Settings, backups: &[Backup], now: i64) -> bool {
    settings.automatic
        && backups
            .iter()
            .filter(|b| b.kind == Kind::Auto)
            .map(|b| b.created)
            .max()
            // A clock moved backwards also counts as due, so backups never stop.
            .is_none_or(|last| now - last >= settings.interval.ms() || last > now)
}

/// Run an automatic backup if one is due, then prune. Returns the new backup.
pub fn run_if_due(shared: &Shared, data_dir: &Path, now: i64) -> Result<Option<Backup>, String> {
    let settings = Settings::load(data_dir).map_err(|e| e.to_string())?;
    if !due(&settings, &list(data_dir), now) {
        return Ok(None);
    }
    let backup = create(shared, data_dir, Kind::Auto, now)?;
    prune(data_dir, settings.keep).map_err(|e| e.to_string())?;
    Ok(Some(backup))
}

static SCHEDULER: AtomicBool = AtomicBool::new(false);

/// Check once a minute, on a background thread, whether an automatic backup
/// is due. Starts at most one thread per process; it ends at shutdown.
pub fn spawn_scheduler(shared: Shared, data_dir: PathBuf) {
    if SCHEDULER.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::Builder::new()
        .name("draugr-backup".into())
        .stack_size(256 * 1024)
        .spawn(move || {
            // Let startup and the first samples settle before copying.
            let mut wait = std::time::Duration::from_secs(120);
            loop {
                let deadline = std::time::Instant::now() + wait;
                while std::time::Instant::now() < deadline {
                    if shared.control.lock().unwrap().shutdown {
                        SCHEDULER.store(false, Ordering::SeqCst);
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_secs(5));
                }
                if let Err(error) = run_if_due(&shared, &data_dir, crate::now_ms()) {
                    eprintln!("Automatic backup failed: {error}");
                }
                wait = std::time::Duration::from_secs(60);
            }
        })
        .map(drop)
        .unwrap_or_else(|_| SCHEDULER.store(false, Ordering::SeqCst));
}

/// Check `backup` and stage it to replace the history on the next launch.
pub fn stage_restore(data_dir: &Path, backup: &Path) -> Result<(), String> {
    draugr_time_core::verify_backup(backup).map_err(|e| e.to_string())?;
    let pending = data_dir.join(PENDING);
    let partial = data_dir.join(format!("{PENDING}.partial"));
    std::fs::copy(backup, &partial).map_err(|e| e.to_string())?;
    std::fs::rename(&partial, &pending).map_err(|e| e.to_string())
}

pub fn restore_pending(data_dir: &Path) -> bool {
    data_dir.join(PENDING).exists()
}

pub fn cancel_restore(data_dir: &Path) -> io::Result<()> {
    match std::fs::remove_file(data_dir.join(PENDING)) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// Before the database is opened: if a restore is staged, move the current
/// history into the backups folder as a `before-restore` copy and put the
/// staged backup in its place.
pub fn apply_pending_restore(data_dir: &Path, now: i64) -> Result<bool, String> {
    let pending = data_dir.join(PENDING);
    if !pending.exists() {
        return Ok(false);
    }
    if let Err(error) = draugr_time_core::verify_backup(&pending) {
        let _ = std::fs::remove_file(&pending);
        return Err(format!("Restore cancelled: {error}"));
    }
    let database = data_dir.join(DATABASE);
    if database.exists() {
        let dir = directory(data_dir);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut kept = dir.join(file_name(Kind::BeforeRestore, now));
        for n in 2.. {
            if !kept.exists() {
                break;
            }
            kept = dir.join(
                file_name(Kind::BeforeRestore, now).replace(SUFFIX, &format!("-{n}{SUFFIX}")),
            );
        }
        // Opening and closing the database checkpoints its WAL into the main
        // file, so the single file below holds the complete history.
        drop(draugr_time_core::Storage::open(&database).map_err(|e| e.to_string())?);
        if PathBuf::from(format!("{}-wal", database.display())).exists() {
            return Err(
                "Restore postponed: another program still has the history open. Quit it and restart Draugr Time."
                    .into(),
            );
        }
        std::fs::rename(&database, &kept).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&pending, &database).map_err(|e| e.to_string())?;
    Ok(true)
}
