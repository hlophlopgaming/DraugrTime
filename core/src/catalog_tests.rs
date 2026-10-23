use super::*;
use serde_json::json;
#[test]
fn catalog_unions_subtrees_splits_day_edges_and_tracks_last_use() {
    let mut db = Storage::open_in_memory().unwrap();
    let root = db
        .put(Collection::Tags, None, json!({"name":"Root"}))
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    let child = db
        .put(
            Collection::Tags,
            None,
            json!({"name":"Child","parent_id":root}),
        )
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    db.put(
        Collection::TagRanges,
        None,
        json!({"tag_id":root,"start_ts":5,"end_ts":15}),
    )
    .unwrap();
    db.put(
        Collection::TagRanges,
        None,
        json!({"tag_id":child,"start_ts":10,"end_ts":25}),
    )
    .unwrap();
    let stats = db.tag_statistics(&[0, 10, 20, 30, 40, 50, 60, 70]).unwrap();
    let root = stats.iter().find(|s| s.id == root).unwrap();
    assert_eq!(root.all_ms, 20);
    assert_eq!(root.days, [5, 10, 5, 0, 0, 0, 0]);
    assert_eq!(root.last_used, Some(25));
    assert_eq!(root.range_count, 2);
    assert!(db.tag_statistics(&[0, 10, 10, 30, 40, 50, 60, 70]).is_err());
}
#[test]
fn tag_creation_and_reparent_are_atomic_undoable_and_reject_cycles() {
    let mut db = Storage::open_in_memory().unwrap();
    let (_child, undo) = db.create_tag_path("Root, Child").unwrap();
    assert_eq!(db.tags().unwrap().len(), 2);
    assert!(db.tag_ranges(0, 100).unwrap().is_empty());
    db.undo_tags(undo).unwrap();
    assert!(db.tags().unwrap().is_empty());
    let (child, _) = db.create_tag_path("Root, Child").unwrap();
    let root = db
        .tags()
        .unwrap()
        .iter()
        .find(|t| t.parent_id.is_none())
        .unwrap()
        .id;
    assert!(db
        .update_tag_details(root, "Root", None, false, Some(child))
        .is_err());
    let undo = db
        .update_tag_details(child, "Renamed", Some("#5FD4B8"), false, None)
        .unwrap();
    assert!(db
        .tags()
        .unwrap()
        .iter()
        .find(|t| t.id == child)
        .unwrap()
        .parent_id
        .is_none());
    db.undo_tags(undo).unwrap();
    assert_eq!(
        db.tags()
            .unwrap()
            .iter()
            .find(|t| t.id == child)
            .unwrap()
            .parent_id,
        Some(root)
    );
    assert!(db
        .update_tag_details(child, "Bad", None, false, Some(999))
        .is_err());
    assert_eq!(
        db.tags()
            .unwrap()
            .iter()
            .find(|t| t.id == child)
            .unwrap()
            .name,
        "Child"
    );
    let _ = child;
}
#[test]
fn rule_order_is_atomic_and_coverage_distinguishes_existing_tags() {
    let mut db = Storage::open_in_memory().unwrap();
    db.create_tag_path("Work").unwrap();
    for pattern in ["editor", "browser"] {
        db.put(
            Collection::Rules,
            None,
            json!({"field":"app","pattern":pattern,"tag_id":1}),
        )
        .unwrap();
    }
    db.reorder_rules(&[2, 1]).unwrap();
    assert_eq!(
        db.rules().unwrap().iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![2, 1]
    );
    assert!(db.reorder_rules(&[1, 1]).is_err());
    assert_eq!(db.rules().unwrap()[0].id, 2);
    db.record_interval("window", &json!({"app":"editor","title":"Draft"}), 0, 100)
        .unwrap();
    db.record_interval("afk", &json!({"status":"active"}), 0, 100)
        .unwrap();
    db.tag_interval("Work", "", 40, 60, false).unwrap();
    let coverage = db.rule_coverage(0, 100, &db.rules().unwrap()[1]).unwrap();
    assert_eq!(coverage.matched_ms, 100);
    assert_eq!(coverage.covered_ms, 20);
    assert!(coverage.samples.iter().all(|m| m.sample == "editor"));
}

#[test]
fn selected_rule_application_preserves_other_rules_and_rejects_stale_preview() {
    let mut db = Storage::open_in_memory().unwrap();
    db.put(Collection::Tags, None, json!({"name":"Work"}))
        .unwrap();
    for pattern in ["editor", "browser"] {
        db.put(
            Collection::Rules,
            None,
            json!({"field":"app","pattern":pattern,"tag_id":1,"priority":0,"enabled":true}),
        )
        .unwrap();
    }
    for (app, start, end) in [("editor", 0, 50), ("browser", 50, 100)] {
        db.record_interval("window", &json!({"app":app,"title":"Work"}), start, end)
            .unwrap();
    }
    db.record_interval(
        "computer",
        &json!({"machine":"awake","activity":"active","recording":"recording"}),
        0,
        100,
    )
    .unwrap();
    let preview: Vec<_> = db
        .preview_rules(0, 100, None)
        .unwrap()
        .into_iter()
        .filter(|r| r.rule_id == 1)
        .collect();
    assert_eq!(preview.len(), 1);
    let (_, undo) = db.apply_rule_checked(0, 100, 1, &preview).unwrap();
    assert_eq!(db.tag_ranges(0, 100).unwrap()[0].end_ts, 50);
    assert!(db.apply_rule_checked(0, 100, 1, &preview).is_err());
    db.undo_tags(undo).unwrap();
    assert!(db.tag_ranges(0, 100).unwrap().is_empty());
    let escaped = crate::literal_rule_pattern("editor[2].exe");
    assert!(!crate::rule_match_ranges(&escaped, "editor[2].exe")
        .unwrap()
        .is_empty());
    assert!(crate::rule_match_ranges(&escaped, "editor2.exe")
        .unwrap()
        .is_empty());
}

#[test]
fn coverage_keeps_actual_executable_boundaries_when_report_labels_merge() {
    let mut db = Storage::open_in_memory().unwrap();
    db.put(Collection::Tags, None, json!({"name":"Work"}))
        .unwrap();
    for (app, start, end) in [("one", 0, 50), ("two", 50, 100)] {
        db.record_interval(
            "window",
            &json!({"app":app,"app_name":"Shared name","title":"Shared title"}),
            start,
            end,
        )
        .unwrap();
    }
    db.record_interval(
        "computer",
        &json!({"machine":"awake","activity":"active","recording":"recording"}),
        0,
        100,
    )
    .unwrap();
    assert_eq!(db.report(0, 100).unwrap().intervals.len(), 1);
    let rule = Rule {
        id: 0,
        field: "app".into(),
        pattern: "^two$".into(),
        tag_id: 1,
        priority: 0,
        enabled: true,
    };
    let coverage = db.rule_coverage(0, 100, &rule).unwrap();
    assert_eq!(coverage.matched_ms, 50);
    assert_eq!(coverage.samples[0].start_ts, 50);
}
