//! Consistent copies of the database file and their verification.

use crate::{Storage, StorageError, SCHEMA_VERSION};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};

impl Storage {
    /// Write buffered event ends and return the database file to copy from,
    /// or `None` for an in-memory database. Copying from that file through a
    /// separate connection does not block recording.
    pub fn backup_source(&mut self) -> Result<Option<PathBuf>, StorageError> {
        self.flush()?;
        Ok(self
            .connection
            .path()
            .filter(|path| !path.is_empty())
            .map(PathBuf::from))
    }

    /// Copy this database into a new file at `destination` while holding it.
    /// Used for in-memory databases; files use [`backup_file`].
    pub fn backup_to(&mut self, destination: &Path) -> Result<(), StorageError> {
        self.flush()?;
        vacuum_into(&self.connection, destination)?;
        verify_backup(destination)
    }
}

/// Copy the database file `source` into a new file at `destination` and check
/// the copy. The copy is a single compacted file with no WAL beside it.
pub fn backup_file(source: &Path, destination: &Path) -> Result<(), StorageError> {
    let connection = Connection::open_with_flags(
        source,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection.busy_timeout(std::time::Duration::from_secs(5))?;
    vacuum_into(&connection, destination)?;
    drop(connection);
    verify_backup(destination)
}

fn vacuum_into(connection: &Connection, destination: &Path) -> Result<(), StorageError> {
    let target = destination
        .to_str()
        .ok_or_else(|| StorageError::InvalidRecord("backup path is not valid UTF-8".into()))?;
    connection.execute("VACUUM INTO ?1", [target])?;
    Ok(())
}

/// Check that `path` is a complete Draugr Time database this build can open.
pub fn verify_backup(path: &Path) -> Result<(), StorageError> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let check: String = connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if check != "ok" {
        return Err(StorageError::InvalidRecord(format!(
            "backup is damaged: {check}"
        )));
    }
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version > SCHEMA_VERSION {
        return Err(StorageError::UnsupportedSchema(version));
    }
    let tables: i64 = connection.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('events','tags','tag_ranges')",
        [],
        |row| row.get(0),
    )?;
    if version == 0 || tables != 3 {
        return Err(StorageError::InvalidRecord(
            "file is not a Draugr Time database".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn history(db: &mut Storage) {
        db.heartbeat("window", &json!({"app":"Editor"}), 1_000, 3_000)
            .unwrap();
        // Buffered tail: only in memory until flushed.
        db.heartbeat("window", &json!({"app":"Editor"}), 2_500, 3_000)
            .unwrap();
        db.tag_interval("Work, Draugr", "note", 1_000, 2_000, false)
            .unwrap();
    }

    #[test]
    fn file_backup_copies_buffered_history_and_opens_as_storage() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = Storage::open(dir.path().join("activity.sqlite")).unwrap();
        history(&mut db);
        let source = db.backup_source().unwrap().unwrap();
        let copy = dir.path().join("copy.sqlite");
        backup_file(&source, &copy).unwrap();
        // Recording continues while the copy exists.
        db.heartbeat("window", &json!({"app":"Other"}), 9_000, 3_000)
            .unwrap();
        let restored = Storage::open(&copy).unwrap();
        let events = restored.events(Some("window"), 0, 10_000).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].end_ts, 2_500);
        assert_eq!(restored.tag_ranges(0, 10_000).unwrap().len(), 1);
    }

    #[test]
    fn memory_backup_and_existing_destination_and_bad_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = Storage::open_in_memory().unwrap();
        history(&mut db);
        assert_eq!(db.backup_source().unwrap(), None);
        let copy = dir.path().join("memory.sqlite");
        db.backup_to(&copy).unwrap();
        assert_eq!(
            Storage::open(&copy)
                .unwrap()
                .events(Some("window"), 0, 10_000)
                .unwrap()
                .len(),
            1
        );
        // VACUUM INTO never overwrites an existing file.
        assert!(db.backup_to(&copy).is_err());

        let junk = dir.path().join("junk.sqlite");
        std::fs::write(&junk, b"definitely not sqlite").unwrap();
        assert!(verify_backup(&junk).is_err());
        let empty = dir.path().join("empty.sqlite");
        Connection::open(&empty)
            .unwrap()
            .execute_batch("CREATE TABLE other(x)")
            .unwrap();
        assert!(verify_backup(&empty).is_err());
        let newer = dir.path().join("newer.sqlite");
        std::fs::copy(&copy, &newer).unwrap();
        Connection::open(&newer)
            .unwrap()
            .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
        assert!(matches!(
            verify_backup(&newer),
            Err(StorageError::UnsupportedSchema(_))
        ));
        assert!(verify_backup(&dir.path().join("missing.sqlite")).is_err());
    }
}
