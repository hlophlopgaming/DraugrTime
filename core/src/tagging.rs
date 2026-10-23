use super::*;

#[derive(Clone, Debug)]
struct RowChange<T> {
    id: i64,
    before: Option<T>,
    after: Option<T>,
}
#[derive(Clone, Debug, Default)]
pub struct TagUndo {
    tags: Vec<RowChange<Tag>>,
    ranges: Vec<RowChange<TagRange>>,
    rules: Vec<RowChange<Rule>>,
}
impl TagUndo {
    pub(crate) fn inserted(ranges: Vec<TagRange>) -> Self {
        Self {
            ranges: ranges
                .into_iter()
                .map(|r| RowChange {
                    id: r.id,
                    before: None,
                    after: Some(r),
                })
                .collect(),
            ..Self::default()
        }
    }
    #[cfg(test)]
    pub(crate) fn retained_rows(&self) -> usize {
        fn count<T>(rows: &[RowChange<T>]) -> usize {
            rows.iter()
                .map(|r| usize::from(r.before.is_some()) + usize::from(r.after.is_some()))
                .sum()
        }
        count(&self.tags) + count(&self.ranges) + count(&self.rules)
    }
}
fn invalid(message: &str) -> StorageError {
    StorageError::InvalidRecord(message.into())
}
fn differences<T: PartialEq>(
    before: Vec<T>,
    after: Vec<T>,
    id: impl Fn(&T) -> i64,
) -> Vec<RowChange<T>> {
    let mut before: std::collections::BTreeMap<_, _> =
        before.into_iter().map(|r| (id(&r), r)).collect();
    let mut result = Vec::new();
    for row in after {
        let id = id(&row);
        let old = before.remove(&id);
        if old.as_ref() != Some(&row) {
            result.push(RowChange {
                id,
                before: old,
                after: Some(row),
            });
        }
    }
    result.extend(before.into_iter().map(|(id, row)| RowChange {
        id,
        before: Some(row),
        after: None,
    }));
    result
}
fn resolve_tag_path(c: &Connection, path: &str) -> Result<(i64, Vec<i64>), StorageError> {
    let names: Vec<_> = path.split(',').map(str::trim).collect();
    if names.iter().any(|s| s.is_empty()) {
        return Err(invalid("Use nonempty comma-separated tags"));
    }
    let mut inserted = Vec::new();
    let mut parent: Option<i64> = None;
    for name in names {
        let existing=c.query_row("SELECT id FROM tags WHERE name=?1 AND parent_id IS ?2 AND archived=0 ORDER BY id LIMIT 1",params![name,parent],|r|r.get::<_,i64>(0)).optional()?;
        parent = Some(if let Some(id) = existing {
            id
        } else {
            c.execute(
                "INSERT INTO tags(name,parent_id) VALUES(?1,?2)",
                params![name, parent],
            )?;
            let id = c.last_insert_rowid();
            inserted.push(id);
            id
        });
    }
    Ok((parent.unwrap(), inserted))
}
fn ids(ids: impl Iterator<Item = i64>) -> String {
    ids.map(|id| id.to_string()).collect::<Vec<_>>().join(",")
}
fn read_tags(c: &Connection, predicate: &str) -> Result<Vec<Tag>, StorageError> {
    let mut query = c.prepare(&format!(
        "SELECT id,name,color,parent_id,archived FROM tags WHERE {predicate} ORDER BY id"
    ))?;
    let rows = query
        .query_map([], |r| {
            Ok(Tag {
                id: r.get(0)?,
                name: r.get(1)?,
                color: r.get(2)?,
                parent_id: r.get(3)?,
                archived: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}
fn read_ranges(c: &Connection, predicate: &str) -> Result<Vec<TagRange>, StorageError> {
    let mut query = c.prepare(&format!(
        "SELECT id,tag_id,start_ts,end_ts,note,precedence_id FROM tag_ranges WHERE {predicate} ORDER BY id"
    ))?;
    let rows = query
        .query_map([], |r| {
            Ok(TagRange {
                precedence_id: r.get(5)?,
                id: r.get(0)?,
                tag_id: r.get(1)?,
                start_ts: r.get(2)?,
                end_ts: r.get(3)?,
                note: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}
fn read_rules(c: &Connection, predicate: &str) -> Result<Vec<Rule>, StorageError> {
    let mut query = c.prepare(&format!(
        "SELECT id,field,pattern,tag_id,priority,enabled FROM rules WHERE {predicate} ORDER BY id"
    ))?;
    let rows = query
        .query_map([], |r| {
            Ok(Rule {
                id: r.get(0)?,
                field: r.get(1)?,
                pattern: r.get(2)?,
                tag_id: r.get(3)?,
                priority: r.get(4)?,
                enabled: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}
impl Storage {
    fn tag_change(
        &mut self,
        scope: [&str; 3],
        change: impl FnOnce(&Connection) -> Result<[Vec<i64>; 2], StorageError>,
    ) -> Result<TagUndo, StorageError> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let tags = read_tags(&tx, scope[0])?;
        let ranges = read_ranges(&tx, scope[1])?;
        let rules = read_rules(&tx, scope[2])?;
        let [inserted_tags, inserted_ranges] = change(&tx)?;
        let after_tags = read_tags(
            &tx,
            &format!(
                "id IN ({})",
                ids(tags.iter().map(|r| r.id).chain(inserted_tags))
            ),
        )?;
        let after_ranges = read_ranges(
            &tx,
            &format!(
                "id IN ({})",
                ids(ranges.iter().map(|r| r.id).chain(inserted_ranges))
            ),
        )?;
        let after_rules = read_rules(&tx, &format!("id IN ({})", ids(rules.iter().map(|r| r.id))))?;
        let undo = TagUndo {
            tags: differences(tags, after_tags, |r| r.id),
            ranges: differences(ranges, after_ranges, |r| r.id),
            rules: differences(rules, after_rules, |r| r.id),
        };
        tx.commit()?;
        self.metadata_version = self.metadata_version.wrapping_add(1);
        Ok(undo)
    }
    pub fn undo_tags(&mut self, undo: TagUndo) -> Result<(), StorageError> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        // Validate inside the write transaction so API edits cannot be lost.
        for row in &undo.tags {
            let current = read_tags(&tx, &format!("id={}", row.id))?.pop();
            if current != row.after {
                return Err(invalid(
                    "Tags changed since this action; undo refused to protect newer edits",
                ));
            }
        }
        for row in &undo.ranges {
            let current = read_ranges(&tx, &format!("id={}", row.id))?.pop();
            if current != row.after {
                return Err(invalid(
                    "Tags changed since this action; undo refused to protect newer edits",
                ));
            }
        }
        for row in &undo.rules {
            let current = read_rules(&tx, &format!("id={}", row.id))?.pop();
            if current != row.after {
                return Err(invalid(
                    "Tags changed since this action; undo refused to protect newer edits",
                ));
            }
        }
        tx.execute_batch("PRAGMA defer_foreign_keys=ON")?;
        for row in &undo.rules {
            tx.execute("DELETE FROM rules WHERE id=?1", [row.id])?;
        }
        for row in &undo.ranges {
            tx.execute("DELETE FROM tag_ranges WHERE id=?1", [row.id])?;
        }
        for row in &undo.tags {
            tx.execute("DELETE FROM tags WHERE id=?1", [row.id])?;
        }
        for row in undo.tags.into_iter().filter_map(|r| r.before) {
            tx.execute(
                "INSERT INTO tags(id,name,color,parent_id,archived) VALUES(?1,?2,?3,?4,?5)",
                params![row.id, row.name, row.color, row.parent_id, row.archived],
            )?;
        }
        for row in undo.ranges.into_iter().filter_map(|r| r.before) {
            tx.execute(
                "INSERT INTO tag_ranges(id,tag_id,start_ts,end_ts,note,precedence_id) VALUES(?1,?2,?3,?4,?5,?6)",
                params![row.id, row.tag_id, row.start_ts, row.end_ts, row.note, row.precedence_id],
            )?;
        }
        for row in undo.rules.into_iter().filter_map(|r| r.before) {
            tx.execute("INSERT INTO rules(id,field,pattern,tag_id,priority,enabled) VALUES(?1,?2,?3,?4,?5,?6)", params![row.id,row.field,row.pattern,row.tag_id,row.priority,row.enabled])?;
        }
        tx.commit()?;
        self.metadata_version = self.metadata_version.wrapping_add(1);
        Ok(())
    }
    /// Replace removes intersecting blocks; split preserves their outside portions.
    pub fn tag_interval(
        &mut self,
        path: &str,
        note: &str,
        start: i64,
        end: i64,
        replace: bool,
    ) -> Result<TagUndo, StorageError> {
        self.edit_tag_interval(None, path, note, start, end, replace)
    }
    pub fn edit_tag_interval(
        &mut self,
        id: Option<i64>,
        path: &str,
        note: &str,
        start: i64,
        end: i64,
        replace: bool,
    ) -> Result<TagUndo, StorageError> {
        let names: Vec<_> = path.split(',').map(str::trim).collect();
        if start < 0 || end <= start || names.iter().any(|s| s.is_empty()) {
            return Err(invalid("Use nonempty comma-separated tags and start < end"));
        }
        let ranges = format!(
            "start_ts<{end} AND end_ts>{start} OR id={}",
            id.unwrap_or(0)
        );
        self.tag_change(["0", &ranges, "0"], |connection| {
            let mut inserted = Vec::new();
            if let Some(id) = id { if connection.execute("DELETE FROM tag_ranges WHERE id=?1", [id])? == 0 { return Err(StorageError::NotFound); } }
            let (parent,inserted_tags)=resolve_tag_path(connection,path)?;
            let mut query = connection.prepare("SELECT id,tag_id,start_ts,end_ts,note,COALESCE(precedence_id,id) FROM tag_ranges WHERE start_ts<?2 AND end_ts>?1")?;
            let overlaps = query.query_map(params![start,end], |r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,i64>(5)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
            for (id,tag,from,to,note,precedence) in overlaps {
                connection.execute("DELETE FROM tag_ranges WHERE id=?1", [id])?;
                if !replace {
                    for (a,b) in [(from,start.min(to)),(end.max(from),to)] {
                        if a < b { connection.execute("INSERT INTO tag_ranges(tag_id,start_ts,end_ts,note,precedence_id) VALUES(?1,?2,?3,?4,?5)", params![tag,a,b,note,precedence])?; inserted.push(connection.last_insert_rowid()); }
                    }
                }
            }
            connection.execute("INSERT INTO tag_ranges(tag_id,start_ts,end_ts,note) VALUES(?1,?2,?3,?4)", params![parent,start,end,note])?;
            inserted.push(connection.last_insert_rowid());
            Ok([inserted_tags, inserted])
        })
    }
    pub fn delete_tag_range(&mut self, id: i64) -> Result<TagUndo, StorageError> {
        self.tag_change(["0", &format!("id={id}"), "0"], |c| {
            if c.execute("DELETE FROM tag_ranges WHERE id=?1", [id])? == 0 {
                return Err(StorageError::NotFound);
            }
            Ok([Vec::new(), Vec::new()])
        })
    }
    pub fn create_tag_path(&mut self, path: &str) -> Result<(i64, TagUndo), StorageError> {
        let mut id = 0;
        let undo = self.tag_change(["0", "0", "0"], |c| {
            let (tag, inserted) = resolve_tag_path(c, path)?;
            id = tag;
            Ok([inserted, Vec::new()])
        })?;
        Ok((id, undo))
    }
    pub fn update_tag_details(
        &mut self,
        id: i64,
        name: &str,
        color: Option<&str>,
        archived: bool,
        parent: Option<i64>,
    ) -> Result<TagUndo, StorageError> {
        if name.trim().is_empty()
            || color.is_some_and(|c| {
                c.len() != 7
                    || !c.starts_with('#')
                    || !c[1..].bytes().all(|b| b.is_ascii_hexdigit())
            })
        {
            return Err(invalid("Nonempty name and #RRGGBB color required"));
        }
        self.tag_change([&format!("id={id}"),"0","0"],|c| {
            if let Some(parent)=parent {
                let exists:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM tags WHERE id=?1)",[parent],|r|r.get(0))?;
                if !exists {return Err(StorageError::NotFound);}
                let cycle:bool=c.query_row("WITH RECURSIVE tree(id) AS (SELECT ?1 UNION SELECT tags.id FROM tags JOIN tree ON parent_id=tree.id) SELECT EXISTS(SELECT 1 FROM tree WHERE id=?2)",params![id,parent],|r|r.get(0))?;
                if cycle {return Err(invalid("tag parent would create a cycle"));}
            }
            if c.execute("UPDATE tags SET name=?2,color=?3,archived=?4,parent_id=?5 WHERE id=?1",params![id,name.trim(),color,archived,parent])?==0 {return Err(StorageError::NotFound);}
            Ok([Vec::new(),Vec::new()])
        })
    }
    pub fn update_tag(
        &mut self,
        id: i64,
        name: &str,
        color: Option<&str>,
        archived: bool,
    ) -> Result<TagUndo, StorageError> {
        if name.trim().is_empty()
            || color.is_some_and(|c| {
                c.len() != 7
                    || !c.starts_with('#')
                    || !c[1..].bytes().all(|b| b.is_ascii_hexdigit())
            })
        {
            return Err(invalid("Nonempty name and #RRGGBB color required"));
        }
        self.tag_change([&format!("id={id}"), "0", "0"], |c| {
            if c.execute(
                "UPDATE tags SET name=?2,color=?3,archived=?4 WHERE id=?1",
                params![id, name.trim(), color, archived],
            )? == 0
            {
                return Err(StorageError::NotFound);
            }
            Ok([Vec::new(), Vec::new()])
        })
    }
    pub fn merge_tags(&mut self, source: i64, target: i64) -> Result<TagUndo, StorageError> {
        if source == target {
            return Err(invalid("Select another tag to merge into"));
        }
        self.tag_change([&format!("id={source} OR parent_id={source}"), &format!("tag_id={source}"), &format!("tag_id={source}")], |c| {
            let cycle: bool = c.query_row("WITH RECURSIVE tree(id) AS (SELECT ?1 UNION SELECT tags.id FROM tags JOIN tree ON parent_id=tree.id) SELECT EXISTS(SELECT 1 FROM tree WHERE id=?2)", params![source,target], |r|r.get(0))?;
            if cycle { return Err(invalid("Cannot merge into a descendant")); }
            let exists: bool = c.query_row("SELECT EXISTS(SELECT 1 FROM tags WHERE id=?1)", [target], |r|r.get(0))?;
            if !exists { return Err(StorageError::NotFound); }
            c.execute("UPDATE tag_ranges SET tag_id=?2 WHERE tag_id=?1", params![source,target])?;
            c.execute("UPDATE rules SET tag_id=?2 WHERE tag_id=?1", params![source,target])?;
            c.execute("UPDATE tags SET parent_id=?2 WHERE parent_id=?1", params![source,target])?;
            if c.execute("DELETE FROM tags WHERE id=?1", [source])? == 0 { return Err(StorageError::NotFound); }
            Ok([Vec::new(), Vec::new()])
        })
    }
    pub fn delete_tag_tree(&mut self, id: i64) -> Result<TagUndo, StorageError> {
        let tree = format!("WITH RECURSIVE tree(id) AS (SELECT {id} UNION SELECT tags.id FROM tags JOIN tree ON parent_id=tree.id) SELECT id FROM tree");
        self.tag_change([&format!("id IN ({tree})"), &format!("tag_id IN ({tree})"), &format!("tag_id IN ({tree})")], |c| {
            let tree = "WITH RECURSIVE tree(id) AS (SELECT ?1 UNION SELECT tags.id FROM tags JOIN tree ON parent_id=tree.id)";
            c.execute(&format!("{tree} DELETE FROM tag_ranges WHERE tag_id IN tree"), [id])?;
            c.execute(&format!("{tree} DELETE FROM rules WHERE tag_id IN tree"), [id])?;
            if c.execute(&format!("{tree} DELETE FROM tags WHERE id IN tree"), [id])? == 0 { return Err(StorageError::NotFound); }
            Ok([Vec::new(), Vec::new()])
        })
    }
}

impl Storage {
    /// Latest completed AFK run, using full UTC intervals independently of the shown day.
    pub fn completed_afk(&self) -> Result<Option<(i64, i64)>, StorageError> {
        let valid = |e: &Event, activity: &str| {
            e.data["machine"] == "awake"
                && e.data["locked"] != true
                && e.data["activity"] == activity
                && matches!(
                    e.data["recording"].as_str(),
                    Some("recording" | "unobserved")
                )
        };
        let Some(last) = self.last_event("computer")? else {
            return Ok(None);
        };
        if !valid(&last, "active") {
            return Ok(None);
        }
        let mut boundary = last.start_ts;
        let mut end = None;
        let mut query=self.connection.prepare("SELECT id,bucket,start_ts,end_ts,data FROM events WHERE bucket='computer' AND start_ts<?1 ORDER BY start_ts DESC,id DESC")?;
        let events = query.query_map([last.start_ts], event_from_row)?;
        for event in events {
            let event = event?;
            if event.end_ts != boundary {
                break;
            }
            if end.is_none() && valid(&event, "active") {
                boundary = event.start_ts;
                continue;
            }
            if !valid(&event, "away") {
                break;
            }
            end.get_or_insert(event.end_ts);
            boundary = event.start_ts;
        }
        Ok(end.map(|end| (boundary, end)))
    }
}
