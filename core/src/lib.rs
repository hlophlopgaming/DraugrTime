//! Platform-independent storage and tracking logic for Draugr Time.

use std::{collections::HashMap, path::Path, time::Duration};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::Value;

/// An activity interval. Timestamps are Unix milliseconds in UTC.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Event {
    pub id: i64,
    pub bucket: String,
    pub start_ts: i64,
    pub end_ts: i64,
    pub data: Value,
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("database schema version {0} is newer than this application supports")]
    UnsupportedSchema(i64),
    #[error(
        "bucket must not be empty, timestamp must be nonnegative, and pulsetime must be positive"
    )]
    InvalidHeartbeat,
    #[error("heartbeat timestamp precedes the last event in its bucket")]
    OutOfOrderHeartbeat,
    #[error("invalid record: {0}")]
    InvalidRecord(String),
    #[error("record not found")]
    NotFound,
}

/// Local SQLite storage. Opening a database applies pending migrations atomically.
pub struct Storage {
    connection: Connection,
    tails: HashMap<String, Event>,
    last_flush: Option<i64>,
    metadata_version: u64,
    auto_tail: Option<(i64, i64, i64, u64)>,
}

impl Storage {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        Self::initialize(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self, StorageError> {
        Self::initialize(Connection::open_in_memory()?)
    }

    fn initialize(mut connection: Connection) -> Result<Self, StorageError> {
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.pragma_update(None, "cache_size", -512)?;
        // The tracker writes every second. WAL appends to a log instead of rewriting
        // pages through a rollback journal, and NORMAL syncs at checkpoints rather
        // than on every commit: still crash-safe for the application, ~10x less I/O.
        // In-memory databases report "memory" and keep working unchanged.
        connection
            .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get::<_, String>(0))?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version: i64 = transaction.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version > 5 {
            return Err(StorageError::UnsupportedSchema(version));
        }
        if version == 0 {
            transaction.execute_batch(include_str!("../migrations/001_initial.sql"))?;
        }
        if version < 2 {
            transaction.execute_batch(include_str!("../migrations/002_tag_archive.sql"))?;
            transaction.pragma_update(None, "user_version", 2)?;
        }
        if version < 3 {
            transaction.execute_batch(include_str!("../migrations/003_tag_range_index.sql"))?;
            transaction.pragma_update(None, "user_version", 3)?;
        }
        if version < 4 {
            transaction.execute_batch(include_str!("../migrations/004_rules_enabled.sql"))?;
            transaction.pragma_update(None, "user_version", 4)?;
        }
        if version < 5 {
            transaction
                .execute_batch(include_str!("../migrations/005_tag_range_precedence.sql"))?;
            transaction.pragma_update(None, "user_version", 5)?;
        }
        transaction.commit()?;
        Ok(Self {
            connection,
            tails: HashMap::new(),
            last_flush: None,
            metadata_version: 0,
            auto_tail: None,
        })
    }

    /// Record a heartbeat, merging only with the latest event in this bucket.
    /// `ts` and `pulsetime` are milliseconds. New events have zero duration;
    /// subsequent matching heartbeats extend them to their observed timestamp.
    /// Timestamps must be nondecreasing within each bucket.
    pub fn heartbeat(
        &mut self,
        bucket: &str,
        data: &Value,
        ts: i64,
        pulsetime: i64,
    ) -> Result<Event, StorageError> {
        let batch = self.batch()?;
        let event = batch.heartbeat(bucket, data, ts, pulsetime)?;
        batch.commit()?;
        Ok(event)
    }

    /// Start a group of writes that is committed atomically with one disk sync.
    /// Dropping the batch without calling [`Batch::commit`] rolls it back.
    pub fn batch(&mut self) -> Result<Batch<'_>, StorageError> {
        self.flush()?;
        self.tails.clear();
        Ok(Batch {
            transaction: self
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)?,
        })
    }
}

/// Several event writes in one immediate transaction. The tracker records a whole
/// sampling tick through one batch instead of one transaction per write.
pub struct Batch<'a> {
    transaction: rusqlite::Transaction<'a>,
}

impl Batch<'_> {
    /// Same semantics as [`Storage::heartbeat`], inside this batch.
    pub fn heartbeat(
        &self,
        bucket: &str,
        data: &Value,
        ts: i64,
        pulsetime: i64,
    ) -> Result<Event, StorageError> {
        heartbeat_in(&self.transaction, bucket, data, ts, pulsetime)
    }

    /// Same semantics as [`Storage::extend_last`], inside this batch.
    pub fn extend_last(&self, bucket: &str, ts: i64, max_gap: i64) -> Result<(), StorageError> {
        records::extend_last_in(&self.transaction, bucket, ts, max_gap)
    }

    pub fn commit(self) -> Result<(), StorageError> {
        self.transaction.commit()?;
        Ok(())
    }
}

fn heartbeat_in(
    transaction: &Connection,
    bucket: &str,
    data: &Value,
    ts: i64,
    pulsetime: i64,
) -> Result<Event, StorageError> {
    if bucket.is_empty() || ts < 0 || pulsetime <= 0 {
        return Err(StorageError::InvalidHeartbeat);
    }
    let last = transaction
        .query_row(
            "SELECT id, bucket, start_ts, end_ts, data FROM events
             WHERE bucket = ?1 ORDER BY start_ts DESC, id DESC LIMIT 1",
            [bucket],
            event_from_row,
        )
        .optional()?;

    if let Some(mut event) = last {
        if ts < event.end_ts {
            return Err(StorageError::OutOfOrderHeartbeat);
        }
        if event.data == *data && ts - event.end_ts < pulsetime {
            transaction.execute(
                "UPDATE events SET end_ts = ?1 WHERE id = ?2",
                params![ts, event.id],
            )?;
            event.end_ts = ts;
            return Ok(event);
        }
    }

    transaction.execute(
        "INSERT INTO events (bucket, start_ts, end_ts, data) VALUES (?1, ?2, ?2, ?3)",
        params![bucket, ts, serde_json::to_string(data)?],
    )?;
    Ok(Event {
        id: transaction.last_insert_rowid(),
        bucket: bucket.to_owned(),
        start_ts: ts,
        end_ts: ts,
        data: data.clone(),
    })
}

fn event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Event> {
    let data: String = row.get(4)?;
    let data = serde_json::from_str(&data).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(Event {
        id: row.get(0)?,
        bucket: row.get(1)?,
        start_ts: row.get(2)?,
        end_ts: row.get(3)?,
        data,
    })
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;

#[cfg(test)]
mod crud_tests;

mod records;
pub use records::{Collection, Tag, TagRange};

impl Storage {
    /// Stage a sampling tick without opening a SQLite transaction.
    pub fn buffered_batch(&mut self, now: i64) -> BufferedBatch<'_> {
        BufferedBatch {
            tails: self.tails.clone(),
            storage: self,
            writes: Vec::new(),
            now,
        }
    }

    /// Persist the live overlay. On failure the overlay remains available for retry.
    pub fn flush(&mut self) -> Result<(), StorageError> {
        if self.tails.is_empty() {
            return Ok(());
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        for event in self.tails.values() {
            transaction.execute(
                "UPDATE events SET end_ts=?1 WHERE id=?2 AND end_ts<>?1",
                params![event.end_ts, event.id],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Local CRUD revision and external-connection revision (WAL included).
    pub fn metadata_version(&self) -> Result<(u64, i64), StorageError> {
        Ok((
            self.metadata_version,
            self.connection
                .query_row("PRAGMA data_version", [], |r| r.get(0))?,
        ))
    }
}

/// Dropping a staged tick leaves both the overlay and SQLite unchanged.
pub struct BufferedBatch<'a> {
    storage: &'a mut Storage,
    tails: HashMap<String, Event>,
    writes: Vec<Event>,
    now: i64,
}
impl BufferedBatch<'_> {
    fn last(&mut self, bucket: &str) -> Result<Option<Event>, StorageError> {
        if let Some(event) = self.tails.get(bucket) {
            return Ok(Some(event.clone()));
        }
        let event = self.storage.connection.query_row(
            "SELECT id,bucket,start_ts,end_ts,data FROM events WHERE bucket=?1 ORDER BY start_ts DESC,id DESC LIMIT 1",
            [bucket], event_from_row).optional()?;
        if let Some(event) = &event {
            self.tails.insert(bucket.into(), event.clone());
        }
        Ok(event)
    }
    pub fn heartbeat(
        &mut self,
        bucket: &str,
        data: &Value,
        ts: i64,
        pulsetime: i64,
    ) -> Result<(), StorageError> {
        if bucket.is_empty() || ts < 0 || pulsetime <= 0 {
            return Err(StorageError::InvalidHeartbeat);
        }
        if let Some(mut last) = self.last(bucket)? {
            if ts < last.end_ts {
                return Err(StorageError::OutOfOrderHeartbeat);
            }
            if last.data == *data && ts - last.end_ts < pulsetime {
                last.end_ts = ts;
                self.tails.insert(bucket.into(), last);
                return Ok(());
            }
            self.writes.push(last);
        }
        self.tails.insert(
            bucket.into(),
            Event {
                id: 0,
                bucket: bucket.into(),
                start_ts: ts,
                end_ts: ts,
                data: data.clone(),
            },
        );
        Ok(())
    }
    pub fn extend_last(&mut self, bucket: &str, ts: i64, max_gap: i64) -> Result<(), StorageError> {
        if let Some(mut last) = self.last(bucket)? {
            if ts >= last.end_ts && ts - last.end_ts < max_gap {
                last.end_ts = ts;
                self.tails.insert(bucket.into(), last);
            }
        }
        Ok(())
    }
    pub fn commit(mut self) -> Result<(), StorageError> {
        let flush = !self.writes.is_empty()
            || self.tails.values().any(|e| e.id == 0)
            || self
                .storage
                .last_flush
                .is_none_or(|ts| self.now.saturating_sub(ts) >= 30_000);
        if flush {
            let transaction = self
                .storage
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
            for event in self.writes.iter_mut().chain(self.tails.values_mut()) {
                if event.id == 0 {
                    transaction.execute(
                        "INSERT INTO events(bucket,start_ts,end_ts,data) VALUES(?1,?2,?3,?4)",
                        params![
                            event.bucket,
                            event.start_ts,
                            event.end_ts,
                            serde_json::to_string(&event.data)?
                        ],
                    )?;
                    event.id = transaction.last_insert_rowid();
                } else {
                    transaction.execute(
                        "UPDATE events SET end_ts=?1 WHERE id=?2 AND end_ts<>?1",
                        params![event.end_ts, event.id],
                    )?;
                }
            }
            transaction.commit()?;
            self.storage.last_flush = Some(self.now);
        }
        self.storage.tails = self.tails;
        Ok(())
    }
}

impl Storage {
    /// Latest event, including zero-length lifecycle markers and live tails.
    pub fn last_event(&self, bucket: &str) -> Result<Option<Event>, StorageError> {
        if let Some(event) = self.tails.get(bucket) {
            return Ok(Some(event.clone()));
        }
        Ok(self.connection.query_row("SELECT id,bucket,start_ts,end_ts,data FROM events WHERE bucket=?1 ORDER BY start_ts DESC,id DESC LIMIT 1", [bucket], event_from_row).optional()?)
    }
    /// Append an evidence-backed interval; preserve the per-bucket non-overlap invariant.
    pub fn record_interval(
        &mut self,
        bucket: &str,
        data: &Value,
        from: i64,
        to: i64,
    ) -> Result<(), StorageError> {
        if from < 0 || to < from {
            return Err(StorageError::InvalidRecord(
                "invalid evidence interval".into(),
            ));
        }
        let batch = self.batch()?;
        batch.heartbeat(bucket, data, from, 1)?;
        batch.extend_last(bucket, to, i64::MAX)?;
        batch.commit()
    }
}

impl Storage {
    pub fn first_event_start(&self, bucket: &str) -> Result<Option<i64>, StorageError> {
        Ok(self
            .connection
            .query_row(
                "SELECT start_ts FROM events WHERE bucket=?1 ORDER BY start_ts,id LIMIT 1",
                [bucket],
                |row| row.get(0),
            )
            .optional()?)
    }
}

#[cfg(test)]
mod tagging_tests;

mod tagging;
pub use tagging::TagUndo;

#[cfg(test)]
mod rules_tests;

mod rules;
pub use rules::{
    literal_rule_pattern, rule_match_ranges, validate_rule_pattern, Rule, RuleCoverage, RuleMatch,
};

mod reports;
pub use reports::{Archive, GroupBy, Report, ReportInterval, ReportRow};
#[cfg(test)]
mod report_tests;

mod privacy;
#[cfg(test)]
mod privacy_tests;

mod catalog;
pub use catalog::TagStatistics;
#[cfg(test)]
mod catalog_tests;
