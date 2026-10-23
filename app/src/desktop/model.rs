use chrono::{Datelike, Local, NaiveDate, TimeZone};
use draugr_time_core::{Event, Tag, TagRange};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    pub start: i64,
    pub end: i64,
}
impl Range {
    pub const fn new(start: i64, end: i64) -> Self {
        Self { start, end }
    }
    pub fn duration(self) -> i64 {
        (self.end - self.start).max(0)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub tag_record: Option<(i64, Range)>,
    pub tag_precedence: Option<i64>,
    pub range: Range,
    pub bucket: &'static str,
    pub key: Arc<str>,
    pub label: Arc<str>,
    pub detail: Arc<str>,
    pub app: Option<Arc<str>>,
    pub app_name: Option<Arc<str>>,
    pub color: [f32; 3],
}
impl Segment {
    pub fn new(range: Range, key: &str, label: &str) -> Self {
        Self {
            tag_record: None,
            tag_precedence: None,
            range,
            bucket: "tags",
            key: key.into(),
            label: label.into(),
            detail: "".into(),
            app: None,
            app_name: None,
            color: color(key),
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub name: &'static str,
    pub segments: Vec<Segment>,
}
#[derive(Clone, Debug)]
pub struct Summary {
    pub key: Arc<str>,
    pub label: Arc<str>,
    pub duration: i64,
    pub share: f64,
    pub color: [f32; 3],
    pub app: Option<Arc<str>>,
    pub app_name: Option<Arc<str>>,
}

pub fn clip(value: Range, bounds: Range) -> Option<Range> {
    let range = Range::new(value.start.max(bounds.start), value.end.min(bounds.end));
    (range.end > range.start).then_some(range)
}
fn merged(values: &[Range], bounds: Range) -> Vec<Range> {
    let mut values: Vec<_> = values
        .iter()
        .filter_map(|value| clip(*value, bounds))
        .collect();
    values.sort_by_key(|value| (value.start, value.end));
    let mut result: Vec<Range> = Vec::new();
    for value in values {
        if let Some(last) = result.last_mut().filter(|last| last.end >= value.start) {
            last.end = last.end.max(value.end);
        } else {
            result.push(value);
        }
    }
    result
}
pub fn union_duration(values: &[Range], bounds: Range) -> i64 {
    merged(values, bounds)
        .iter()
        .map(|value| value.duration())
        .sum()
}
pub fn uncovered(base: &[Range], covers: &[Range], bounds: Range) -> Vec<Range> {
    let mut result = Vec::new();
    let covers = merged(covers, bounds);
    for range in merged(base, bounds) {
        let mut cursor = range.start;
        for cover in &covers {
            if cover.end <= cursor {
                continue;
            }
            if cover.start >= range.end {
                break;
            }
            if cover.start > cursor {
                result.push(Range::new(cursor, cover.start.min(range.end)));
            }
            cursor = cursor.max(cover.end);
            if cursor >= range.end {
                break;
            }
        }
        if cursor < range.end {
            result.push(Range::new(cursor, range.end));
        }
    }
    result
}
#[cfg(test)]
pub fn summarize(segments: &[Segment], range: Range) -> Vec<Summary> {
    summarize_iter(segments.iter(), range)
}
pub fn summarize_iter<'a>(
    segments: impl IntoIterator<Item = &'a Segment>,
    range: Range,
) -> Vec<Summary> {
    let mut groups: HashMap<&str, Vec<&Segment>> = HashMap::new();
    for segment in segments {
        if clip(segment.range, range).is_some() {
            groups.entry(&segment.key).or_default().push(segment);
        }
    }
    let mut result: Vec<_> = groups
        .into_iter()
        .map(|(key, group)| {
            let duration =
                union_duration(&group.iter().map(|s| s.range).collect::<Vec<_>>(), range);
            Summary {
                key: key.into(),
                label: group[0].label.clone(),
                duration,
                share: duration as f64 / range.duration().max(1) as f64 * 100.0,
                color: group[0].color,
                app: group[0].app.clone(),
                app_name: group[0].app_name.clone(),
            }
        })
        .collect();
    result.sort_by(|a, b| {
        b.duration
            .cmp(&a.duration)
            .then_with(|| a.label.cmp(&b.label))
    });
    result
}
#[cfg(test)]
pub fn application_documents(
    documents: &[Segment],
    application: &str,
    range: Range,
) -> Vec<Summary> {
    application_documents_iter(documents.iter(), application, range)
}
pub fn application_documents_iter<'a>(
    documents: impl IntoIterator<Item = &'a Segment>,
    application: &str,
    range: Range,
) -> Vec<Summary> {
    summarize_iter(
        documents.into_iter().filter(|segment| {
            segment
                .app
                .as_deref()
                .zip(segment.app_name.as_deref())
                .is_some_and(|(app, name)| application_identity(app, name) == application)
        }),
        range,
    )
}

pub fn pixel_range(view: Range, width: f32, a: f32, b: f32) -> Range {
    let time = |x: f32| {
        view.start
            + (x.clamp(0.0, width.max(1.0)) as f64 / width.max(1.0) as f64 * view.duration() as f64)
                .round() as i64
    };
    Range::new(time(a).min(time(b)), time(a).max(time(b)))
}
pub fn zoom(day: Range, view: Range, factor: f64, anchor: Option<i64>) -> Range {
    let span = ((view.duration() as f64 * factor).round() as i64).clamp(60_000, day.duration());
    let anchor = anchor.unwrap_or(view.start + view.duration() / 2);
    let position = (anchor - view.start) as f64 / view.duration().max(1) as f64;
    let start = (anchor - (span as f64 * position).round() as i64).clamp(day.start, day.end - span);
    Range::new(start, start + span)
}
pub fn continuous_span(segment: &Segment, values: &[Segment]) -> Range {
    let mut range = segment.range;
    for value in values.iter().rev() {
        if value.key == segment.key && value.range.end == range.start {
            range.start = value.range.start;
        }
    }
    for value in values {
        if value.key == segment.key
            && value.range.start <= range.end
            && value.range.end >= range.start
        {
            range.end = range.end.max(value.range.end);
        }
    }
    range
}
pub fn process_label(app: &str) -> &str {
    app.rsplit(['/', '\\'])
        .find(|part| !part.is_empty())
        .unwrap_or(app)
}
pub fn application_identity(process: &str, app_name: &str) -> String {
    format!("{process}\0{app_name}")
}
pub fn color(app: &str) -> [f32; 3] {
    let hash = app.chars().fold(2166136261u32, |hash, c| {
        (hash ^ c as u32).wrapping_mul(16777619)
    });
    super::design::CATEGORICAL[hash as usize % super::design::CATEGORICAL.len()]
        .map(|c| f32::from(c) / 255.0)
}
pub fn day_bounds(date: NaiveDate) -> Range {
    let midnight = |date: NaiveDate| {
        Local
            .with_ymd_and_hms(date.year(), date.month(), date.day(), 0, 0, 0)
            .earliest()
            .map(|date| date.timestamp_millis())
            .unwrap_or(0)
    };
    Range::new(midnight(date), midnight(date.succ_opt().unwrap_or(date)))
}
pub fn time_label(ts: i64) -> String {
    Local
        .timestamp_millis_opt(ts)
        .single()
        .map(|time| time.format("%H:%M").to_string())
        .unwrap_or_default()
}
pub fn duration(ms: i64) -> String {
    super::localization::duration_in(super::localization::current(), ms)
}
pub fn empty_tracks() -> Vec<Track> {
    ["Tags", "Computer", "Applications", "Documents"]
        .into_iter()
        .map(|name| Track {
            name,
            segments: Vec::new(),
        })
        .collect()
}
pub fn tracks(events: &[Event], tags: &[Tag], ranges: &[TagRange], day: Range) -> Vec<Track> {
    let mut result = empty_tracks();
    // Index tags once: a day can hold many ranges, and each needs its tag and path.
    let by_id: std::collections::HashMap<i64, &Tag> = tags.iter().map(|t| (t.id, t)).collect();
    let mut paths: std::collections::HashMap<i64, Arc<str>> = std::collections::HashMap::new();
    for value in ranges {
        let tag_id = value.tag_id;
        let original_range = Range::new(value.start_ts, value.end_ts);
        let Some(range) = clip(original_range, day) else {
            continue;
        };
        let tag = by_id.get(&tag_id).copied();
        let name = tag.map(|t| t.name.as_str()).unwrap_or("Unknown tag");
        let mut segment = Segment::new(range, &tag_id.to_string(), name);
        segment.tag_record = Some((value.id, original_range));
        segment.tag_precedence = value.precedence_id;
        segment.label = paths
            .entry(tag_id)
            .or_insert_with(|| tag_path(tags, tag_id).into())
            .clone();
        segment.detail = value.note.as_deref().unwrap_or_default().into();
        if let Some(hex) = tag
            .and_then(|tag| tag.color.as_deref())
            .and_then(|hex| hex.strip_prefix('#'))
            .filter(|hex| hex.len() == 6)
        {
            if let Ok(rgb) = u32::from_str_radix(hex, 16) {
                segment.color = [
                    ((rgb >> 16) & 255) as f32 / 255.0,
                    ((rgb >> 8) & 255) as f32 / 255.0,
                    (rgb & 255) as f32 / 255.0,
                ];
            }
        }
        result[0].segments.push(segment);
    }
    let computer_start = events
        .iter()
        .filter(|e| e.bucket == "computer")
        .map(|e| e.start_ts)
        .min();
    for event in events {
        let bounds = if event.bucket == "afk" {
            Range::new(day.start, computer_start.unwrap_or(day.end).min(day.end))
        } else {
            day
        };
        let Some(range) = clip(Range::new(event.start_ts, event.end_ts), bounds) else {
            continue;
        };
        match event.bucket.as_str() {
            "computer" => {
                let machine = event.data["machine"].as_str().unwrap_or("unknown");
                let activity = event.data["activity"].as_str().unwrap_or("unknown");
                let recording = event.data["recording"].as_str().unwrap_or("unknown");
                let (key, label, color) = if machine == "asleep" {
                    ("asleep", "Sleep", [0.43, 0.49, 0.65])
                } else if machine == "off" {
                    ("off", "Off (OS evidence)", [0.36, 0.40, 0.45])
                } else if event.data["locked"] == true {
                    ("locked", "Locked", [0.60, 0.52, 0.68])
                } else if recording == "paused" {
                    ("paused", "Paused", [0.72, 0.64, 0.43])
                } else if recording == "stopped" {
                    ("stopped", "Tracker stopped", [0.49, 0.53, 0.58])
                } else if recording == "unknown" {
                    ("unknown", "No recording", [0.49, 0.53, 0.58])
                } else if activity == "active" {
                    ("active", "Active", [0.37, 0.65, 0.58])
                } else if activity == "away" {
                    ("away", "AFK", [0.83, 0.69, 0.40])
                } else if recording == "unobserved" {
                    ("unobserved", "Unobserved", [0.75, 0.43, 0.40])
                } else {
                    ("pending", "Awaiting sample", [0.49, 0.53, 0.58])
                };
                let mut segment = Segment::new(range, key, label);
                segment.bucket = "computer";
                segment.color = color;
                segment.detail = format!(
                    "{}{}{}",
                    event.data["reason"].as_str().unwrap_or(""),
                    if event.data["reason"].is_string() {
                        " · "
                    } else {
                        ""
                    },
                    event.data["source"]
                        .as_str()
                        .unwrap_or("recorded computer state")
                )
                .into();
                result[1].segments.push(segment.clone());
                if recording == "unobserved"
                    && event.data["window_available"] == false
                    && activity != "away"
                    && machine == "awake"
                    && event.data["locked"] != true
                {
                    segment.key = "unobserved".into();
                    segment.label = "Unobserved".into();
                    segment.color = [0.75, 0.43, 0.40];
                    result[2].segments.push(segment.clone());
                    result[3].segments.push(segment);
                }
            }
            "afk" => {
                let state = event.data["status"].as_str().unwrap_or("unknown");
                let mut segment = Segment::new(
                    range,
                    state,
                    match state {
                        "active" => "Active",
                        "away" => "Away",
                        _ => "Unobserved",
                    },
                );
                segment.color = match state {
                    "active" => [0.37, 0.65, 0.58],
                    "away" => [0.83, 0.69, 0.40],
                    _ => [0.49, 0.53, 0.58],
                };
                segment.bucket = "afk";
                result[1].segments.push(segment);
            }
            "window" => {
                let app = event.data["app"].as_str().unwrap_or("Unknown application");
                let app_name = event.data["app_name"]
                    .as_str()
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or_else(|| process_label(app));
                let title = event.data["title"].as_str().unwrap_or("Untitled window");
                let identity = application_identity(app, app_name);
                let mut segment =
                    Segment::new(range, &identity, &crate::icons::application_name(app_name));
                segment.detail = title.into();
                segment.app = Some(app.into());
                segment.app_name = Some(app_name.into());
                segment.bucket = "window";
                result[2].segments.push(segment.clone());
                segment.key = format!("{identity}\0{title}").into();
                segment.label = title.into();
                segment.detail = app.into();
                result[3].segments.push(segment);
            }
            "web" => {
                let url = event.data["url"].as_str().unwrap_or_default();
                let title = event.data["title"].as_str().unwrap_or(url);
                let mut segment = Segment::new(range, url, title);
                segment.detail = url.into();
                segment.bucket = "web";
                result[3].segments.push(segment);
            }
            _ => {}
        }
    }
    derive_tracks(&mut result, day);
    let mut strings: HashSet<Arc<str>> = HashSet::new();
    for track in &mut result {
        for segment in &mut track.segments {
            for value in [&mut segment.key, &mut segment.label, &mut segment.detail]
                .into_iter()
                .chain(segment.app.iter_mut())
                .chain(segment.app_name.iter_mut())
            {
                if let Some(shared) = strings.get(value.as_ref()) {
                    *value = shared.clone();
                } else {
                    strings.insert(value.clone());
                }
            }
        }
    }
    result
}
pub fn derive_tracks(tracks: &mut [Track], day: Range) {
    for track in tracks.iter_mut() {
        track.segments.retain(|s| s.bucket != "derived");
    }
    let observed: Vec<_> = tracks[1]
        .segments
        .iter()
        .filter(|s| s.key.as_ref() == "active" || s.key.as_ref() == "away")
        .map(|s| s.range)
        .collect();
    let tagged: Vec<_> = tracks[0].segments.iter().map(|s| s.range).collect();
    for range in uncovered(&observed, &tagged, day) {
        let mut segment = Segment::new(range, "untagged", "Untagged");
        segment.color = [0.64, 0.68, 0.72];
        segment.bucket = "derived";
        tracks[0].segments.push(segment);
    }
    for track in &mut *tracks {
        track.segments.sort_by_key(|s| (s.range.start, s.range.end));
    }
    let known: Vec<_> = tracks[1].segments.iter().map(|s| s.range).collect();
    for pair in known.windows(2) {
        if pair[0].end < pair[1].start {
            let mut segment = Segment::new(
                Range::new(pair[0].end, pair[1].start),
                "unknown",
                "No recording",
            );
            segment.detail = "No verified observations for this gap; power state is unknown".into();
            segment.color = [0.49, 0.53, 0.58];
            segment.bucket = "derived";
            tracks[1].segments.push(segment);
        }
    }
    tracks[1].segments.sort_by_key(|s| s.range.start);
}
#[cfg(test)]
pub fn fixture(day: Range) -> Vec<Track> {
    fixture_with_apps(
        day,
        &[
            ("Editor".into(), "Editor".into()),
            ("Browser".into(), "Browser".into()),
            ("Terminal".into(), "Terminal".into()),
            ("Mail".into(), "Mail".into()),
            ("Design".into(), "Design".into()),
        ],
    )
}
pub fn fixture_with_apps(day: Range, apps: &[(String, String)]) -> Vec<Track> {
    let mut events = Vec::new();
    let at = |hour: i64| day.start + hour * 3_600_000;
    let fallback = [
        ("Editor", "Editor"),
        ("Browser", "Browser"),
        ("Terminal", "Terminal"),
        ("Mail", "Mail"),
        ("Design", "Design"),
    ];
    let apps: Vec<(&str, &str)> = if apps.is_empty() {
        fallback.to_vec()
    } else {
        apps.iter()
            .map(|(process, name)| (process.as_str(), name.as_str()))
            .collect()
    };
    let documents = [
        "Project plan",
        "Draft notes",
        "Research",
        "Design system",
        "Weekly review",
        "Prototype",
        "Build output",
        "Inbox",
    ];
    let morning = 2222;
    for i in 0..5000 {
        let (index, count, base, span) = if i < morning {
            (i, morning, at(8), 4 * 3_600_000)
        } else {
            (i - morning, 5000 - morning, at(13), 5 * 3_600_000)
        };
        events.push(Event {
            id: i + 1,
            bucket: "window".into(),
            start_ts: base + index * span / count,
            end_ts: base + (index + 1) * span / count,
            data: serde_json::json!({"app":apps[i as usize % apps.len()].0,"app_name":apps[i as usize % apps.len()].1,"title":format!("{} — {}", apps[i as usize % apps.len()].1, documents[i as usize % 8])}),
        });
    }
    for (start, end, status) in [(8, 12, "active"), (12, 13, "away"), (13, 18, "active")] {
        events.push(Event {
            id: events.len() as i64 + 1,
            bucket: "afk".into(),
            start_ts: at(start),
            end_ts: at(end),
            data: serde_json::json!({"status":status}),
        });
    }
    let tags = [
        Tag {
            id: 1,
            name: "Project Atlas".into(),
            color: Some("#778dc3".into()),
            parent_id: None,
            archived: 0,
        },
        Tag {
            id: 2,
            name: "Studio Paper".into(),
            color: Some("#5baf9d".into()),
            parent_id: None,
            archived: 0,
        },
    ];
    let ranges = [
        TagRange {
            precedence_id: None,
            id: 1,
            tag_id: 1,
            start_ts: at(8),
            end_ts: at(10),
            note: None,
        },
        TagRange {
            precedence_id: None,
            id: 2,
            tag_id: 2,
            start_ts: at(13),
            end_ts: at(16),
            note: None,
        },
    ];
    tracks(&events, &tags, &ranges, day)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overlapping_intervals_and_selection_shares_are_exact() {
        let ranges = [Range::new(0, 15), Range::new(5, 20), Range::new(20, 25)];
        assert_eq!(union_duration(&ranges, Range::new(10, 30)), 15);
        assert_eq!(clip(Range::new(0, 10), Range::new(10, 20)), None);
        let segments = vec![
            Segment::new(Range::new(0, 15), "editor", "Editor"),
            Segment::new(Range::new(5, 20), "editor", "Editor"),
        ];
        let rows = summarize(&segments, Range::new(10, 30));
        assert_eq!(rows[0].duration, 10);
        assert_eq!(rows[0].share, 50.0);
    }
    #[test]
    fn drag_and_zoom_are_clamped_and_reversible() {
        assert_eq!(
            pixel_range(Range::new(100, 200), 100.0, 90.0, 10.0),
            Range::new(110, 190)
        );
        assert_eq!(
            pixel_range(Range::new(100, 200), 100.0, -20.0, 120.0),
            Range::new(100, 200)
        );
        assert_eq!(
            zoom(
                Range::new(0, 86_400_000),
                Range::new(0, 86_400_000),
                0.5,
                None
            ),
            Range::new(21_600_000, 64_800_000)
        );
    }
    #[test]
    fn zoom_keeps_the_anchor_at_the_same_screen_position() {
        let day = Range::new(0, 86_400_000);
        let view = Range::new(10_000_000, 20_000_000);
        let anchor = 12_000_000;
        let zoomed = zoom(day, view, 0.9, Some(anchor));
        assert_eq!(zoomed, Range::new(10_200_000, 19_200_000));
        assert_eq!(zoom(day, zoomed, 1.0 / 0.9, Some(anchor)), view);
    }
    #[test]
    fn fixture_contains_5000_windows_four_tracks_and_correct_totals() {
        let day = Range::new(0, 86_400_000);
        let data = fixture(day);
        assert_eq!(data[2].segments.len(), 5000);
        assert_eq!(
            union_duration(
                &data[1]
                    .segments
                    .iter()
                    .filter(|s| s.key.as_ref() == "active")
                    .map(|s| s.range)
                    .collect::<Vec<_>>(),
                day
            ),
            9 * 3_600_000
        );
        assert_eq!(data.len(), 4);
        assert_eq!(color("Editor"), color("Editor"));
        assert_ne!(color("Editor"), color("Browser"));
    }

    #[test]
    fn fixture_preserves_supplied_process_paths_for_application_identity() {
        let day = Range::new(0, 86_400_000);
        let apps = vec![
            ("/usr/bin/steam".to_string(), "Balatro".to_string()),
            ("/usr/bin/steam".to_string(), "Aseprite".to_string()),
        ];
        let data = fixture_with_apps(day, &apps);
        let process_paths: std::collections::HashSet<_> = data[2]
            .segments
            .iter()
            .filter_map(|segment| segment.app.as_deref())
            .collect();
        assert_eq!(process_paths, ["/usr/bin/steam"].into_iter().collect());
        let names: std::collections::HashSet<_> = data[2]
            .segments
            .iter()
            .filter_map(|segment| segment.app_name.as_deref())
            .collect();
        assert_eq!(names, ["Balatro", "Aseprite"].into_iter().collect());
    }
    #[test]
    fn process_identity_survives_restart_without_merging_other_executables() {
        let segments = vec![
            Segment::new(Range::new(0, 10), "/one/editor", "editor"),
            Segment::new(Range::new(10, 20), "/one/editor", "editor"),
            Segment::new(Range::new(20, 30), "/two/editor", "editor"),
        ];
        let rows = summarize(&segments, Range::new(0, 30));
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].duration, 20);
        assert_eq!(continuous_span(&segments[1], &segments), Range::new(0, 20));
    }

    #[test]
    fn application_summary_separates_apps_sharing_a_launcher_process() {
        let day = Range::new(0, 100);
        let events = [
            Event {
                id: 1,
                bucket: "window".into(),
                start_ts: 0,
                end_ts: 10,
                data: serde_json::json!({"app":"/usr/bin/steam","app_name":"Balatro","title":"Balatro"}),
            },
            Event {
                id: 2,
                bucket: "window".into(),
                start_ts: 10,
                end_ts: 20,
                data: serde_json::json!({"app":"/usr/bin/steam","app_name":"Aseprite","title":"Aseprite"}),
            },
        ];
        let tracks = tracks(&events, &[], &[], day);
        let summary = summarize(&tracks[2].segments, day);
        let labels: std::collections::HashSet<_> =
            summary.iter().map(|row| row.label.as_ref()).collect();
        assert_eq!(labels, ["Aseprite", "Balatro"].into_iter().collect());
        assert_eq!(summary.len(), 2);
    }

    #[test]
    fn application_documents_rank_clipped_merged_time_and_keep_apps_separate() {
        let events = [
            (0, 60, "Browser", "Long page"),
            (40, 80, "Browser", "Long page"),
            (80, 100, "Browser", "Short page"),
            (0, 100, "Editor", "Long page"),
            (100, 120, "Browser", "Outside selection"),
        ].into_iter().enumerate().map(|(id, (start, end, name, title))| Event {
            id: id as i64,
            bucket: "window".into(),
            start_ts: start,
            end_ts: end,
            data: serde_json::json!({"app":"/usr/bin/shared-launcher", "app_name":name, "title":title}),
        }).collect::<Vec<_>>();
        let tracks = tracks(&events, &[], &[], Range::new(0, 120));
        assert_eq!(summarize(&tracks[3].segments, Range::new(0, 120)).len(), 4);
        let rows = application_documents(
            &tracks[3].segments,
            &application_identity("/usr/bin/shared-launcher", "Browser"),
            Range::new(20, 100),
        );
        assert_eq!(
            rows.iter()
                .map(|r| (r.label.as_ref(), r.duration))
                .collect::<Vec<_>>(),
            vec![("Long page", 60), ("Short page", 20)]
        );
        assert_eq!(rows[0].share, 75.0);
        assert!(
            application_documents(&tracks[3].segments, "missing", Range::new(0, 120)).is_empty()
        );
    }
}

#[cfg(test)]
mod computer_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn computer_evidence_overrides_legacy_afk_and_keeps_diagnostics_separate() {
        let e = |id, bucket: &str, start, end, data| Event {
            id,
            bucket: bucket.into(),
            start_ts: start,
            end_ts: end,
            data,
        };
        let events = vec![
            e(1, "afk", 0, 10_000, json!({"status":"active"})),
            e(
                2,
                "computer",
                2000,
                4000,
                json!({"machine":"awake","activity":"active","recording":"unobserved","window_available":false,"reason":"window: permission denied","source":"mock"}),
            ),
            e(
                3,
                "computer",
                4000,
                8000,
                json!({"machine":"asleep","activity":"unknown","recording":"recording","source":"mock OS"}),
            ),
            e(
                4,
                "computer",
                8000,
                10_000,
                json!({"machine":"awake","activity":"away","recording":"recording","source":"mock"}),
            ),
        ];
        let tracks = tracks(&events, &[], &[], Range::new(0, 10_000));
        assert_eq!(
            union_duration(
                &tracks[1]
                    .segments
                    .iter()
                    .filter(|s| s.key.as_ref() == "active")
                    .map(|s| s.range)
                    .collect::<Vec<_>>(),
                Range::new(0, 10_000)
            ),
            4000
        );
        assert!(tracks[1]
            .segments
            .iter()
            .any(|s| s.label.as_ref() == "Sleep"));
        assert!(tracks[2]
            .segments
            .iter()
            .any(|s| s.label.as_ref() == "Unobserved" && s.detail.contains("permission denied")));
    }
    #[test]
    fn unexplained_gap_is_not_a_window_observation_failure() {
        let events = vec![
            Event {
                id: 1,
                bucket: "afk".into(),
                start_ts: 0,
                end_ts: 1000,
                data: json!({"status":"active"}),
            },
            Event {
                id: 2,
                bucket: "afk".into(),
                start_ts: 3000,
                end_ts: 4000,
                data: json!({"status":"active"}),
            },
        ];
        let tracks = tracks(&events, &[], &[], Range::new(0, 4000));
        let gap = tracks[1]
            .segments
            .iter()
            .find(|s| s.range == Range::new(1000, 3000))
            .unwrap();
        assert_eq!(gap.label.as_ref(), "No recording");
        assert_ne!(gap.label.as_ref(), "Unobserved");
    }
}

#[cfg(test)]
mod power_boundary_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn sleep_across_utc_day_boundary_does_not_count_as_active_or_untagged_work() {
        let hour = 3_600_000;
        let events = vec![
            Event {
                id: 1,
                bucket: "computer".into(),
                start_ts: 23 * hour,
                end_ts: 25 * hour,
                data: json!({"machine":"asleep","activity":"unknown","recording":"recording","source":"mock OS lifecycle"}),
            },
            Event {
                id: 2,
                bucket: "computer".into(),
                start_ts: 25 * hour,
                end_ts: 26 * hour,
                data: json!({"machine":"awake","activity":"active","recording":"recording","window_available":true,"source":"mock"}),
            },
        ];
        let day = Range::new(24 * hour, 48 * hour);
        let tracks = tracks(&events, &[], &[], day);
        let sleep = tracks[1]
            .segments
            .iter()
            .find(|s| s.key.as_ref() == "asleep")
            .unwrap();
        assert_eq!(sleep.range, Range::new(24 * hour, 25 * hour));
        assert_eq!(
            union_duration(
                &tracks[0]
                    .segments
                    .iter()
                    .map(|s| s.range)
                    .collect::<Vec<_>>(),
                day
            ),
            hour
        );
    }
    #[test]
    fn verified_off_is_displayable_but_unknown_gap_is_not_relabelled_off() {
        let events = vec![
            Event {
                id: 1,
                bucket: "computer".into(),
                start_ts: 1000,
                end_ts: 2000,
                data: json!({"machine":"off","activity":"unknown","recording":"unknown","source":"mock verified OS history"}),
            },
            Event {
                id: 2,
                bucket: "computer".into(),
                start_ts: 2000,
                end_ts: 3000,
                data: json!({"machine":"unknown","activity":"unknown","recording":"unknown","source":"sampling gap"}),
            },
        ];
        let tracks = tracks(&events, &[], &[], Range::new(0, 4000));
        assert!(tracks[1]
            .segments
            .iter()
            .any(|s| s.key.as_ref() == "off" && s.detail.contains("verified OS history")));
        assert!(tracks[1]
            .segments
            .iter()
            .any(|s| s.range == Range::new(2000, 3000) && s.label.as_ref() == "No recording"));
        assert!(tracks[0].segments.is_empty());
    }
}

pub fn tag_path(tags: &[Tag], id: i64) -> String {
    let mut names = Vec::new();
    let mut current = Some(id);
    for _ in 0..tags.len() {
        let Some(tag) = tags.iter().find(|t| Some(t.id) == current) else {
            break;
        };
        names.push(tag.name.as_str());
        current = tag.parent_id;
        if current.is_none() {
            break;
        }
    }
    names.reverse();
    names.join(", ")
}
