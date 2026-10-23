use super::*;
#[derive(Clone, Debug, Default)]
pub struct TagStatistics {
    pub id: i64,
    pub all_ms: i64,
    pub days: [i64; 7],
    pub last_used: Option<i64>,
    pub range_count: usize,
}
impl Storage {
    /// Union raw tagged ranges per subtree, streaming history without retaining it.
    pub fn tag_statistics(&self, days: &[i64; 8]) -> Result<Vec<TagStatistics>, StorageError> {
        if days[0] < 0 || days.windows(2).any(|w| w[1] <= w[0]) {
            return Err(StorageError::InvalidRecord(
                "expected increasing local-day boundaries".into(),
            ));
        }
        let mut stats: std::collections::BTreeMap<_, _> = self
            .tags()?
            .into_iter()
            .map(|t| {
                (
                    t.id,
                    (
                        TagStatistics {
                            id: t.id,
                            ..Default::default()
                        },
                        0,
                        [0; 7],
                    ),
                )
            })
            .collect();
        let mut q=self.connection.prepare("WITH RECURSIVE family(root,id) AS (SELECT id,id FROM tags UNION ALL SELECT family.root,t.id FROM tags t JOIN family ON t.parent_id=family.id) SELECT f.root,r.start_ts,r.end_ts FROM family f JOIN tag_ranges r ON r.tag_id=f.id ORDER BY f.root,r.start_ts,r.end_ts")?;
        let mut rows = q.query([])?;
        while let Some(r) = rows.next()? {
            let id: i64 = r.get(0)?;
            let start: i64 = r.get(1)?;
            let end: i64 = r.get(2)?;
            let (s, last, ends) = stats.get_mut(&id).ok_or(StorageError::NotFound)?;
            s.all_ms += (end - start.max(*last)).max(0);
            *last = (*last).max(end);
            s.last_used = Some(s.last_used.unwrap_or(0).max(end));
            s.range_count += 1;
            for i in 0..7 {
                let a = start.max(days[i]);
                let b = end.min(days[i + 1]);
                if b > a {
                    s.days[i] += (b - a.max(ends[i])).max(0);
                    ends[i] = ends[i].max(b);
                }
            }
        }
        Ok(stats.into_values().map(|s| s.0).collect())
    }
}
