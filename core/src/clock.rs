use super::*;
use std::path::PathBuf;

impl Storage {
    /// Preserve the complete database before replacing samples from an invalid future.
    pub fn recover_clock_rollback(&mut self, now: i64) -> Result<Option<PathBuf>, StorageError> {
        if now < 0 {
            return Err(StorageError::InvalidHeartbeat);
        }
        let mut future = false;
        for bucket in ["computer", "window", "afk"] {
            future |= self
                .last_event(bucket)?
                .is_some_and(|event| event.end_ts > now);
        }
        if !future {
            return Ok(None);
        }
        self.flush()?;
        let version = self.metadata_version()?.1;
        let directory = self
            .connection
            .path()
            .filter(|path| !path.is_empty())
            .and_then(|path| Path::new(path).parent())
            .map(Path::to_path_buf)
            .unwrap_or_else(std::env::temp_dir);
        let backup = tempfile::Builder::new()
            .prefix("clock-rollback-")
            .suffix(".sqlite")
            .tempfile_in(directory)?;
        self.backup_to(backup.path())?;
        backup.as_file().sync_all()?;
        let (_, path) = backup.keep().map_err(|error| error.error)?;
        #[cfg(unix)]
        std::fs::File::open(path.parent().unwrap())?.sync_all()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current: i64 = transaction.query_row("PRAGMA data_version", [], |row| row.get(0))?;
        if current != version {
            return Err(StorageError::InvalidRecord(
                "History changed during clock recovery; retry".into(),
            ));
        }
        transaction.execute(
            "DELETE FROM events WHERE bucket IN ('computer','window','afk') AND start_ts>=?1",
            [now],
        )?;
        transaction.execute(
            "UPDATE events SET end_ts=?1 WHERE bucket IN ('computer','window','afk') AND end_ts>?1",
            [now],
        )?;
        transaction.commit()?;
        self.tails.clear();
        self.auto_tail = None;
        self.last_flush = None;
        self.metadata_version = self.metadata_version.wrapping_add(1);
        Ok(Some(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rollback_backs_up_live_tails_then_trims_only_native_samples() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("activity.sqlite");
        let mut storage = Storage::open(&path).unwrap();
        for bucket in ["computer", "window", "afk", "web"] {
            storage
                .record_interval(bucket, &json!({"value": "old"}), 1000, 2000)
                .unwrap();
            storage
                .record_interval(bucket, &json!({"value": "future"}), 10_000, 11_000)
                .unwrap();
        }
        let mut batch = storage.buffered_batch(12_000);
        batch
            .heartbeat("window", &json!({"value": "future"}), 12_000, 3000)
            .unwrap();
        batch.commit().unwrap();
        storage
            .tag_interval("Work", "keep note", 10_000, 12_000, false)
            .unwrap();
        let tags = storage.tags().unwrap();
        let ranges = storage.tag_ranges(0, 20_000).unwrap();
        let previous = storage.metadata_version().unwrap();
        assert!(storage.recover_clock_rollback(-1).is_err());
        let backup = storage.recover_clock_rollback(1500).unwrap().unwrap();
        assert_eq!(backup.parent(), path.parent());
        let saved = Storage::open(&backup).unwrap();
        for bucket in ["computer", "window", "afk"] {
            let events = storage.events(Some(bucket), 0, 20_000).unwrap();
            assert_eq!(events.len(), 1);
            assert_eq!((events[0].start_ts, events[0].end_ts), (1000, 1500));
            assert_eq!(saved.events(Some(bucket), 0, 20_000).unwrap().len(), 2);
        }
        assert_eq!(saved.last_event("window").unwrap().unwrap().end_ts, 12_000);
        assert_eq!(storage.last_event("web").unwrap().unwrap().end_ts, 11_000);
        assert_eq!(storage.tags().unwrap(), tags);
        assert_eq!(storage.tag_ranges(0, 20_000).unwrap(), ranges);
        assert_eq!(saved.tag_ranges(0, 20_000).unwrap(), ranges);
        assert_ne!(storage.metadata_version().unwrap(), previous);
        assert!(storage.recover_clock_rollback(1500).unwrap().is_none());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&backup).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        storage
            .heartbeat("window", &json!({"value":"new"}), 1500, 3000)
            .unwrap();
        storage
            .heartbeat("window", &json!({"value":"new"}), 2500, 3000)
            .unwrap();
        drop(storage);
        let storage = Storage::open(&path).unwrap();
        assert_eq!(storage.last_event("window").unwrap().unwrap().end_ts, 2500);
    }

    #[test]
    #[cfg(unix)]
    fn failed_backup_preserves_history() {
        let folder = tempfile::tempdir().unwrap();
        let directory = folder.path().join("original");
        std::fs::create_dir(&directory).unwrap();
        let mut storage = Storage::open(directory.join("activity.sqlite")).unwrap();
        storage
            .record_interval("computer", &json!({}), 10_000, 11_000)
            .unwrap();
        let before = storage.last_event("computer").unwrap();
        std::fs::rename(&directory, folder.path().join("moved")).unwrap();
        assert!(storage.recover_clock_rollback(1000).is_err());
        assert_eq!(storage.last_event("computer").unwrap(), before);
    }

    #[test]
    fn failed_trim_rolls_back_and_keeps_the_backup() {
        let folder = tempfile::tempdir().unwrap();
        let mut storage = Storage::open(folder.path().join("activity.sqlite")).unwrap();
        storage
            .record_interval("computer", &json!({}), 10_000, 11_000)
            .unwrap();
        storage.connection.execute_batch("CREATE TRIGGER fail_trim BEFORE DELETE ON events BEGIN SELECT RAISE(ABORT,'write failure'); END;").unwrap();
        let before = storage.last_event("computer").unwrap();
        assert!(storage.recover_clock_rollback(1000).is_err());
        assert_eq!(storage.last_event("computer").unwrap(), before);
        assert!(std::fs::read_dir(folder.path())
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| entry
                .file_name()
                .to_string_lossy()
                .starts_with("clock-rollback-")));
    }
}
