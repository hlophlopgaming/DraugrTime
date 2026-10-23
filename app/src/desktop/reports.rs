mod week;
use super::*;
use chrono::{Datelike, TimeZone, Timelike};
use draugr_time_core::{Event, GroupBy, Report, Tag, TagRange};
use iced::widget::canvas::{self, Canvas, Frame, Geometry, Path, Stroke, Text};
use iced::widget::column;
use iced::{mouse, Point, Rectangle, Renderer, Size};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

pub type Highlight = Option<(Option<i64>, Option<String>)>;
#[derive(Clone, Debug)]
pub enum Edit {
    Refresh,
    Loaded(u64, Result<Data, String>),
    From(String),
    To(String),
    Preset(u8),
    Group(usize),
    Filter(Option<i64>),
    Highlight(Highlight),
    Expand,
    OpenDay(NaiveDate),
    ReviewGap(Range),
    OpenFullReport,
    Export(bool),
    Import,
    File(Option<std::path::PathBuf>),
    ImportDone(Result<usize, String>),
    ExportDone(Result<(), String>),
    Copy,
}
#[derive(Clone, Debug)]
pub struct Day {
    pub date: NaiveDate,
    pub range: Range,
    pub active_ms: i64,
    pub tag_totals: BTreeMap<Option<i64>, i64>,
    pub bands: [Vec<Band>; 3],
    cache: Arc<Mutex<WeekCache>>,
    hours: Vec<(Range, f32)>,
    away: Vec<Range>,
}
impl Day {
    fn summarize(date: NaiveDate, range: Range, report: &Report) -> Self {
        let mut tag_totals = BTreeMap::new();
        for interval in day_intervals(report, range) {
            let duration = interval.end_ts.min(range.end) - interval.start_ts.max(range.start);
            *tag_totals.entry(interval.tag_id).or_default() += duration;
        }
        let day = Self {
            date,
            range,
            active_ms: tag_totals.values().sum(),
            tag_totals,
            bands: Default::default(),
            cache: Arc::new(Mutex::new(WeekCache::default())),
            hours: clock_hours(range),
            away: Vec::new(),
        };
        day
    }
}
fn day_intervals(
    report: &Report,
    range: Range,
) -> impl Iterator<Item = &draugr_time_core::ReportInterval> {
    day_interval_slice(report, range).iter()
}
fn day_interval_slice(report: &Report, range: Range) -> &[draugr_time_core::ReportInterval] {
    let start = report
        .intervals
        .partition_point(|i| i.end_ts <= range.start);
    let end = start + report.intervals[start..].partition_point(|i| i.start_ts < range.end);
    &report.intervals[start..end]
}
#[derive(Clone, Debug, PartialEq)]
pub struct Band {
    start: u16,
    end: u16,
    color: [f32; 3],
}
type Cells = [[Option<[f32; 3]>; 1440]; 3];
fn empty_cells() -> Cells {
    [[None; 1440]; 3]
}
fn fill_cells(cells: &mut [Option<[f32; 3]>; 1440], range: Range, color: [f32; 3], day: Range) {
    let Some(range) = model::clip(range, day) else {
        return;
    };
    let duration = day.duration();
    let start = ((range.start - day.start) as i128 * 1440 / duration as i128) as usize;
    let end = (((range.end - day.start) as i128 * 1440 + duration as i128 - 1) / duration as i128)
        as usize;
    cells[start..end.min(1440)].fill(Some(color));
}
fn fill_states(cells: &mut Cells, tracks: &[Track], range: Range) {
    for s in &tracks[1].segments {
        fill_cells(&mut cells[0], s.range, s.color, range);
    }
    for s in tracks[2]
        .segments
        .iter()
        .filter(|s| s.key.as_ref() == "unobserved")
    {
        fill_cells(
            &mut cells[1],
            s.range,
            [195.0 / 255.0, 95.0 / 255.0, 95.0 / 255.0],
            range,
        );
    }
}
fn fill_tags(cells: &mut Cells, report: &Report, range: Range, tags: &[Tag]) {
    for i in day_intervals(report, range) {
        let color = if i.tag_id.is_none() {
            Color::from_rgb8(145, 155, 170)
        } else {
            tag_color(tags, i.tag_id)
        };
        fill_cells(
            &mut cells[2],
            Range::new(i.start_ts, i.end_ts),
            [color.r, color.g, color.b],
            range,
        );
    }
}
fn compress_cells(cells: Cells) -> [Vec<Band>; 3] {
    cells.map(|row| {
        let mut bands: Vec<Band> = Vec::new();
        for (index, color) in row.into_iter().enumerate() {
            let Some(color) = color else {
                continue;
            };
            if let Some(last) = bands
                .last_mut()
                .filter(|last| last.end as usize == index && last.color == color)
            {
                last.end += 1;
            } else {
                bands.push(Band {
                    start: index as u16,
                    end: index as u16 + 1,
                    color,
                });
            }
        }
        bands
    })
}
fn bands_from_tracks(
    tracks: &[Track],
    report: &Report,
    range: Range,
    tags: &[Tag],
) -> [Vec<Band>; 3] {
    let mut cells = empty_cells();
    fill_states(&mut cells, tracks, range);
    fill_tags(&mut cells, report, range, tags);
    compress_cells(cells)
}
#[derive(Debug, Default)]
struct WeekCache {
    geometry: canvas::Cache,
    theme: Option<Theme>,
    highlight: Highlight,
    minute: i64,
}
const WEEK_RULER: f32 = design::WEEK_RULER;
const WEEK_ROW: f32 = design::WEEK_ROW;
struct WeekCanvas<'a> {
    days: &'a [Day],
    report: &'a Report,
    tags: &'a [Tag],
    highlight: &'a Highlight,
    now: i64,
    stats: &'a timeline::PaintStats,
}
// Split at real local-hour boundaries so repeated/missing DST hours stay on the shared clock.
fn clock_spans(day: &Day, range: Range) -> impl Iterator<Item = (f32, f32)> + '_ {
    day.hours.iter().filter_map(move |(hour, minute)| {
        model::clip(*hour, range).map(|r| {
            (
                minute + (r.start - hour.start) as f32 / 60_000.0,
                minute + (r.end - hour.start) as f32 / 60_000.0,
            )
        })
    })
}
impl WeekCanvas<'_> {
    fn hours(&self) -> (f32, f32) {
        (design::WEEK_FIRST_HOUR, design::WEEK_LAST_HOUR)
    }
    fn day_at(&self, p: Point) -> Option<&Day> {
        if p.y < WEEK_RULER {
            return None;
        }
        self.days
            .get(((p.y - WEEK_RULER) / (WEEK_ROW + design::HAIRLINE)) as usize)
    }
    fn hover(&self, p: Point, size: Size) -> Option<(Range, Option<i64>, &str)> {
        let day = self.day_at(p)?;
        let (first, last) = self.hours();
        let minute = (first + p.x / size.width * (last - first)) * 60.0;
        let intervals = day_interval_slice(self.report, day.range);
        let index = intervals.iter().position(|i| {
            clock_spans(day, Range::new(i.start_ts, i.end_ts.min(self.now)))
                .any(|(start, end)| start <= minute && minute < end)
        })?;
        let mut left = index;
        let mut right = index;
        while left > 0
            && intervals[left - 1].tag_id == intervals[index].tag_id
            && intervals[left - 1].end_ts == intervals[left].start_ts
        {
            left -= 1;
        }
        while right + 1 < intervals.len()
            && intervals[right + 1].tag_id == intervals[index].tag_id
            && intervals[right].end_ts == intervals[right + 1].start_ts
        {
            right += 1;
        }
        Some((
            Range::new(
                intervals[left].start_ts.max(day.range.start),
                intervals[right].end_ts.min(day.range.end).min(self.now),
            ),
            intervals[index].tag_id,
            &intervals[index].tag,
        ))
    }
}
impl canvas::Program<Message> for WeekCanvas<'_> {
    type State = ();
    fn update(
        &self,
        _: &mut (),
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => cursor
                .position_in(bounds)
                .and_then(|p| self.day_at(p))
                .map(|day| {
                    canvas::Action::publish(Message::Report(Edit::OpenDay(day.date))).and_capture()
                }),
            iced::Event::Mouse(mouse::Event::CursorMoved { .. } | mouse::Event::CursorLeft) => {
                Some(canvas::Action::request_redraw())
            }
            _ => None,
        }
    }
    fn mouse_interaction(
        &self,
        _: &(),
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor
            .position_in(bounds)
            .and_then(|p| self.day_at(p))
            .is_some()
        {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let Some(first_day) = self.days.first() else {
            return Vec::new();
        };
        let hover = cursor
            .position_in(bounds)
            .and_then(|p| self.hover(p, bounds.size()));
        let highlight = hover
            .as_ref()
            .map(|h| (h.1, None))
            .or_else(|| self.highlight.clone());
        let mut cache = first_day.cache.lock().unwrap();
        if cache.theme.as_ref() != Some(theme)
            || cache.highlight != highlight
            || cache.minute != self.now / 60_000
        {
            cache.geometry.clear();
            cache.theme = Some(theme.clone());
            cache.highlight = highlight.clone();
            cache.minute = self.now / 60_000;
        }
        let (first, last) = self.hours();
        let x = |minute: f32| {
            ((minute / 60.0 - first) / (last - first) * bounds.width).clamp(0.0, bounds.width)
        };
        let p = design::Palette::new(theme.palette().background.r < 0.5);
        let colors = theme.extended_palette();
        let geometry = cache.geometry.draw(renderer, bounds.size(), |frame| {
            let started = Instant::now();
            for (index, day) in self.days.iter().enumerate() {
                let y = WEEK_RULER + index as f32 * (WEEK_ROW + design::HAIRLINE);
                let today = day.range.start <= self.now && self.now < day.range.end;
                if today {
                    frame.fill_rectangle(
                        Point::new(0.0, y),
                        Size::new(bounds.width, WEEK_ROW),
                        Color {
                            a: design::TODAY_TINT,
                            ..p.accent
                        },
                    );
                }
                for hour in first as u32..=last as u32 {
                    frame.stroke(
                        &Path::line(
                            Point::new(x(hour as f32 * 60.0), y + design::WEEK_TAG_Y),
                            Point::new(
                                x(hour as f32 * 60.0),
                                y + design::WEEK_ACTIVITY_Y + design::WEEK_ACTIVITY_HEIGHT,
                            ),
                        ),
                        Stroke::default()
                            .with_color(p.grid_half)
                            .with_width(design::HAIRLINE),
                    );
                }
                // Rasterize dense context to pixels before drawing; subpixel events stay visible.
                for band in &day.bands[0] {
                    let range = Range::new(
                        day.range.start + day.range.duration() * i64::from(band.start) / 1440,
                        (day.range.start + day.range.duration() * i64::from(band.end) / 1440)
                            .min(self.now),
                    );
                    for (start, end) in clock_spans(day, range) {
                        frame.fill_rectangle(
                            Point::new(x(start), y + design::WEEK_ACTIVITY_Y),
                            Size::new(
                                (x(end) - x(start) - design::HAIRLINE).max(0.0),
                                design::WEEK_ACTIVITY_HEIGHT,
                            ),
                            Color::from_rgb(band.color[0], band.color[1], band.color[2]),
                        );
                    }
                }
                for range in &day.away {
                    for (start, end) in
                        clock_spans(day, Range::new(range.start, range.end.min(self.now)))
                    {
                        let left = x(start);
                        let width = (x(end) - left).max(0.0);
                        frame.fill_rectangle(
                            Point::new(left, y + design::WEEK_ACTIVITY_Y),
                            Size::new(width, design::WEEK_ACTIVITY_HEIGHT),
                            p.away_dim,
                        );
                        let mut hatch = left;
                        while hatch < left + width {
                            let run = (left + width - hatch).min(design::WEEK_ACTIVITY_HEIGHT);
                            frame.stroke(
                                &Path::line(
                                    Point::new(
                                        hatch,
                                        y + design::WEEK_ACTIVITY_Y + design::WEEK_ACTIVITY_HEIGHT,
                                    ),
                                    Point::new(
                                        hatch + run,
                                        y + design::WEEK_ACTIVITY_Y + design::WEEK_ACTIVITY_HEIGHT
                                            - run,
                                    ),
                                ),
                                Stroke::default()
                                    .with_color(p.away)
                                    .with_width(design::HAIRLINE),
                            );
                            hatch += design::HATCH_STEP;
                        }
                    }
                }
                let pixels = week::rasterize(day, self.report, bounds.width, self.now);
                for (lane, cells) in pixels.iter().enumerate() {
                    let mut start = 0;
                    while start < cells.len() {
                        let mut end = start + 1;
                        while end < cells.len() && cells[end] == cells[start] {
                            end += 1;
                        }
                        if let Some((id, rgb)) = cells[start] {
                            let alpha = if highlight.as_ref().is_none_or(|h| h.0 == id) {
                                1.0
                            } else {
                                design::DIMMED_ALPHA
                            };
                            let (offset, height) = if lane == 0 {
                                (design::WEEK_TAG_Y, design::WEEK_TAG_HEIGHT)
                            } else {
                                (design::WEEK_ACTIVITY_Y, design::WEEK_ACTIVITY_HEIGHT)
                            };
                            let width =
                                ((end - start) as f32 - design::HAIRLINE).max(design::HAIRLINE);
                            let color = if lane == 0 {
                                tag_color(self.tags, id)
                            } else {
                                Color::from_rgb(rgb[0], rgb[1], rgb[2])
                            };
                            if lane == 0 && id.is_none() {
                                let mut dash = start as f32;
                                while dash < start as f32 + width {
                                    let right =
                                        (dash + design::DASH_LENGTH).min(start as f32 + width);
                                    for dy in [offset, offset + height] {
                                        frame.stroke(
                                            &Path::line(
                                                Point::new(dash, y + dy),
                                                Point::new(right, y + dy),
                                            ),
                                            Stroke::default()
                                                .with_color(Color {
                                                    a: alpha,
                                                    ..p.text3
                                                })
                                                .with_width(design::HAIRLINE),
                                        );
                                    }
                                    dash += design::DASH_LENGTH * 2.0;
                                }
                            } else {
                                frame.fill_rectangle(
                                    Point::new(start as f32, y + offset),
                                    Size::new(width, height),
                                    Color { a: alpha, ..color },
                                );
                            }
                        }
                        start = end;
                    }
                }
                for band in &day.bands[1] {
                    let range = Range::new(
                        day.range.start + day.range.duration() * i64::from(band.start) / 1440,
                        (day.range.start + day.range.duration() * i64::from(band.end) / 1440)
                            .min(self.now),
                    );
                    for (start, end) in clock_spans(day, range) {
                        frame.fill_rectangle(
                            Point::new(x(start), y + design::WEEK_ACTIVITY_Y),
                            Size::new((x(end) - x(start)).max(0.0), design::WEEK_DIAGNOSTIC_HEIGHT),
                            p.danger,
                        );
                    }
                }
                if day.active_ms == 0 {
                    frame.fill_text(Text {
                        content: tr(if day.range.start > self.now {
                            "No data yet"
                        } else {
                            "No activity in this range"
                        }),
                        position: Point::new(design::CONTROL_GAP as f32, y + design::WEEK_TAG_Y),
                        font: design::UI,
                        size: (design::LABEL as f32).into(),
                        color: p.text3,
                        ..Default::default()
                    });
                }
                frame.stroke(
                    &Path::line(
                        Point::new(0.0, y + WEEK_ROW),
                        Point::new(bounds.width, y + WEEK_ROW),
                    ),
                    Stroke::default()
                        .with_color(p.line_soft)
                        .with_width(design::HAIRLINE),
                );
            }
            let step = if bounds.width / ((last - first) / 2.0)
                < design::WEEK_AXIS_LABEL + design::CONTROL_GAP as f32
            {
                4
            } else {
                2
            };
            for hour in (first as u32..=last as u32).step_by(step) {
                frame.fill_text(Text {
                    content: format!("{hour:02}:00"),
                    position: Point::new(
                        x(hour as f32 * 60.0).clamp(
                            (design::WEEK_AXIS_LABEL / 2.0).min(bounds.width / 2.0),
                            (bounds.width - design::WEEK_AXIS_LABEL / 2.0).max(bounds.width / 2.0),
                        ),
                        design::HAIRLINE,
                    ),
                    align_x: iced::alignment::Horizontal::Center.into(),
                    font: design::MONO,
                    size: (design::CAPTION as f32).into(),
                    color: p.text3,
                    ..Default::default()
                });
            }
            self.stats.frames.fetch_add(1, Ordering::Relaxed);
            self.stats
                .max_build_us
                .fetch_max(started.elapsed().as_micros() as u64, Ordering::Relaxed);
        });
        let mut result = vec![geometry];
        if let (Some((range, _, tag)), Some(p)) = (hover, cursor.position_in(bounds)) {
            let label = format!(
                "{}–{} · {} · {}",
                model::time_label(range.start),
                model::time_label(range.end),
                translate_label(tag),
                model::duration(range.duration())
            );
            let width = (label.chars().count() as f32 * 7.5 + 16.0).min(bounds.width);
            let point = Point::new(
                (p.x + 12.0).min(bounds.width - width).max(0.0),
                (p.y + 16.0).min(bounds.height - 30.0),
            );
            let mut overlay = Frame::new(renderer, bounds.size());
            overlay.fill_rectangle(
                point,
                Size::new(width, 28.0),
                colors.background.strong.color,
            );
            overlay.fill_text(Text {
                content: label,
                position: Point::new(point.x + 8.0, point.y + 6.0),
                size: 12.0.into(),
                color: colors.background.base.text,
                ..Default::default()
            });
            result.push(overlay.into_geometry());
        }
        result
    }
}
fn clock_minute(ts: i64) -> f32 {
    let time = Local.timestamp_millis_opt(ts).unwrap();
    (time.hour() * 60 + time.minute()) as f32 + time.second() as f32 / 60.0
}
fn clock_hours(range: Range) -> Vec<(Range, f32)> {
    let mut starts: Vec<_> = hour_ticks(range).into_iter().map(|(ts, _)| ts).collect();
    if starts.first() != Some(&range.start) {
        starts.insert(0, range.start);
    }
    starts.push(range.end);
    starts
        .windows(2)
        .map(|pair| (Range::new(pair[0], pair[1]), clock_minute(pair[0])))
        .collect()
}
#[derive(Clone, Debug)]
pub struct Data {
    pub from: String,
    pub to: String,
    pub report: Report,
    pub days: Vec<Day>,
    pub tags: Vec<Tag>,
    pub previous: BTreeMap<Option<i64>, i64>,
}
#[derive(Default)]
pub struct Ui {
    pub from: String,
    pub to: String,
    pub group: usize,
    pub filter: Option<i64>,
    pub expanded: bool,
    pub highlight: Highlight,
    pub generation: u64,
    pub data: Option<Data>,
    pub day: Report,
    overview_timeline_cache: canvas::Cache,
    pub loading: bool,
    pub error: String,
}
#[derive(Clone, Debug)]
pub struct Slice {
    pub tag_id: Option<i64>,
    pub tag: String,
    pub application: String,
    pub duration: i64,
    pub members: Vec<String>,
}
impl Slice {
    fn key(&self) -> Highlight {
        Some((self.tag_id, Some(self.application.clone())))
    }
}
pub fn slices(report: &Report, expanded: bool) -> Vec<Slice> {
    let mut groups: BTreeMap<(Option<i64>, &str), BTreeMap<&str, i64>> = BTreeMap::new();
    for i in &report.intervals {
        *groups
            .entry((i.tag_id, i.tag.as_ref()))
            .or_default()
            .entry(i.application.as_ref())
            .or_default() += i.end_ts - i.start_ts;
    }
    let mut slices = Vec::new();
    for ((tag_id, tag), apps) in groups {
        let mut other = Slice {
            tag_id,
            tag: tag.into(),
            application: "Other".into(),
            duration: 0,
            members: Vec::new(),
        };
        for (application, duration) in apps {
            if !expanded && (duration as f64) < report.active_ms as f64 * 0.02 {
                other.duration += duration;
                other.members.push(application.into());
            } else {
                slices.push(Slice {
                    tag_id,
                    tag: tag.into(),
                    application: application.into(),
                    duration,
                    members: Vec::new(),
                });
            }
        }
        if other.duration > 0 {
            slices.push(other);
        }
    }
    slices
}
/// Convert already classified Day tracks to the shared calculation, preserving Day's activity basis.
pub fn from_tracks(tracks: &[Track], tags: &[Tag], range: Range) -> Result<Report, String> {
    let mut events = Vec::new();
    for s in &tracks[1].segments {
        if s.key.as_ref() == "active" {
            events.push(Event {
                id: events.len() as i64 + 1,
                bucket: "afk".into(),
                start_ts: s.range.start,
                end_ts: s.range.end,
                data: serde_json::json!({"status":"active"}),
            });
        }
    }
    for s in &tracks[3].segments {
        let data = match s.bucket {
            "window" => {
                serde_json::json!({"app":s.app.as_deref(),"app_name":s.app_name.as_deref(),"title":s.label.as_ref()})
            }
            "web" => serde_json::json!({"url":s.detail.as_ref()}),
            _ => continue,
        };
        events.push(Event {
            id: events.len() as i64 + 1,
            bucket: s.bucket.into(),
            start_ts: s.range.start,
            end_ts: s.range.end,
            data,
        });
    }
    let mut tags = tags.to_vec();
    let mut ranges = Vec::new();
    for s in &tracks[0].segments {
        if let Some((id, original)) = s.tag_record {
            let tag_id = s.key.parse::<i64>().map_err(|e| e.to_string())?;
            if !tags.iter().any(|t| t.id == tag_id) {
                tags.push(Tag {
                    id: tag_id,
                    name: s.label.to_string(),
                    color: None,
                    parent_id: None,
                    archived: 0,
                });
            }
            ranges.push(TagRange {
                precedence_id: s.tag_precedence,
                id,
                tag_id,
                start_ts: original.start,
                end_ts: original.end,
                note: None,
            });
        }
    }
    Report::calculate(&events, &tags, &ranges, range.start, range.end).map_err(|e| e.to_string())
}
fn date_range(from: &str, to: &str) -> Result<(NaiveDate, NaiveDate, Range), String> {
    let from =
        NaiveDate::parse_from_str(from, "%Y-%m-%d").map_err(|_| tr("Use YYYY-MM-DD dates"))?;
    let to = NaiveDate::parse_from_str(to, "%Y-%m-%d").map_err(|_| tr("Use YYYY-MM-DD dates"))?;
    if to < from || (to - from).num_days() > 365 {
        return Err(tr("Choose up to 366 days"));
    }
    let range = Range::new(model::day_bounds(from).start, model::day_bounds(to).end);
    if range.start < 0 || range.duration() == 0 {
        return Err(tr("Choose a valid date range"));
    }
    Ok((from, to, range))
}
pub(super) fn write_export(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temporary.write_all(bytes).map_err(|e| e.to_string())?;
    temporary.as_file().sync_all().map_err(|e| e.to_string())?;
    temporary.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
fn hour_ticks_with<T: TimeZone>(
    range: Range,
    clock: impl Fn(i64) -> chrono::DateTime<T>,
) -> Vec<(i64, String)> {
    (range.start..range.end)
        .step_by(60_000)
        .filter_map(|ts| {
            let time = clock(ts);
            (time.minute() == 0).then(|| (ts, format!("{:02}:00", time.hour())))
        })
        .collect()
}
fn hour_ticks(range: Range) -> Vec<(i64, String)> {
    hour_ticks_with(range, |ts| Local.timestamp_millis_opt(ts).unwrap())
}
fn fixture_tracks(range: Range, apps: &[(String, String)], now: i64) -> Vec<Track> {
    let mut tracks = model::fixture_with_apps(range, apps);
    for track in &mut tracks {
        track.segments.retain_mut(|s| {
            s.range.end = s.range.end.min(now);
            s.range.end > s.range.start
        });
    }
    tracks
}
fn load(
    shared: draugr_time_daemon::Shared,
    from: String,
    to: String,
    week: bool,
) -> Result<Data, String> {
    let (start, end, range) = date_range(&from, &to)?;
    let report = shared
        .report(range.start, range.end)
        .map_err(|e| e.to_string())?;
    let tags = if week {
        shared
            .storage
            .lock()
            .map_err(|e| e.to_string())?
            .tags()
            .map_err(|e| e.to_string())?
    } else {
        Vec::new()
    };
    let first_computer = if week {
        shared
            .storage
            .lock()
            .map_err(|e| e.to_string())?
            .first_event_start("computer")
            .map_err(|e| e.to_string())?
    } else {
        None
    };
    let mut days = Vec::new();
    let mut date = start;
    loop {
        let range = model::day_bounds(date);
        let mut day = Day::summarize(date, range, &report);
        if week {
            let mut cells = empty_cells();
            fill_tags(&mut cells, &report, range, &tags);
            for bucket in ["computer", "afk"] {
                let end = if bucket == "afk" {
                    first_computer.unwrap_or(range.end).min(range.end)
                } else {
                    range.end
                };
                let mut cursor = range.start;
                while cursor < end {
                    let (mut events, mut next) = shared
                        .storage
                        .lock()
                        .map_err(|e| e.to_string())?
                        .event_chunk(Some(bucket), cursor, end, 8193)
                        .map_err(|e| e.to_string())?;
                    if next <= cursor {
                        next = cursor + 1;
                        events = shared
                            .storage
                            .lock()
                            .map_err(|e| e.to_string())?
                            .event_chunk(Some(bucket), cursor, next, i64::MAX)
                            .map_err(|e| e.to_string())?
                            .0;
                    }
                    events.retain(|e| e.start_ts < next);
                    for e in &mut events {
                        e.start_ts = e.start_ts.max(cursor);
                        e.end_ts = e.end_ts.min(next);
                    }
                    events.retain(|e| e.end_ts > e.start_ts);
                    let tracks = model::tracks(&events, &[], &[], range);
                    day.away.extend(
                        tracks[1]
                            .segments
                            .iter()
                            .filter(|s| s.key.as_ref() == "away")
                            .map(|s| s.range),
                    );
                    fill_states(&mut cells, &tracks, range);
                    cursor = next;
                }
            }
            day.bands = compress_cells(cells);
        }
        days.push(day);
        if date == end {
            break;
        }
        date = date
            .succ_opt()
            .ok_or_else(|| tr("Choose a valid date range"))?;
    }
    let previous = if week {
        let range = week::previous_week(start).ok_or_else(|| tr("Choose a valid date range"))?;
        shared
            .report(range.start, range.end)
            .map_err(|e| e.to_string())?
            .rows(GroupBy::Tag, None)
            .into_iter()
            .map(|r| (r.tag_id, r.duration_ms))
            .collect()
    } else {
        BTreeMap::new()
    };
    Ok(Data {
        previous,
        from,
        to,
        report,
        days,
        tags,
    })
}
impl State {
    pub(super) fn release_reports(&mut self) {
        self.report_ui.data = None;
        self.report_ui.loading = false;
        self.report_ui.generation = self.report_ui.generation.wrapping_add(1);
        memory::SystemReclaimer.reclaim();
    }
    pub fn edit_report(&mut self, event: Edit) -> Task<Message> {
        match event {
            Edit::From(s) => {
                self.report_ui.from = s;
                self.report_ui.data = None;
                self.report_ui.generation += 1;
                self.report_ui.loading = false;
            }
            Edit::To(s) => {
                self.report_ui.to = s;
                self.report_ui.data = None;
                self.report_ui.generation += 1;
                self.report_ui.loading = false;
            }
            Edit::Preset(p) => {
                let from = match p {
                    1 => self
                        .date
                        .checked_sub_days(Days::new(u64::from(
                            self.date.weekday().num_days_from_monday(),
                        )))
                        .unwrap_or(self.date),
                    2 => self.date.with_day(1).unwrap_or(self.date),
                    _ => self.date,
                };
                let to = match p {
                    1 => from.checked_add_days(Days::new(6)).unwrap_or(from),
                    2 => from
                        .checked_add_months(chrono::Months::new(1))
                        .and_then(|d| d.pred_opt())
                        .unwrap_or(from),
                    _ => from,
                };
                self.report_ui.from = from.to_string();
                self.report_ui.to = to.to_string();
                return self.edit_report(Edit::Refresh);
            }
            Edit::Group(g) => self.report_ui.group = g,
            Edit::Filter(id) => self.report_ui.filter = id,
            Edit::Highlight(h) => self.report_ui.highlight = h,
            Edit::Expand => self.report_ui.expanded = !self.report_ui.expanded,
            Edit::OpenFullReport => {
                let dates = self
                    .report_ui
                    .data
                    .as_ref()
                    .map(|data| (data.from.clone(), data.to.clone()));
                self.set_screen("Reports");
                if let Some((from, to)) = dates {
                    self.report_ui.from = from;
                    self.report_ui.to = to;
                }
                return self.edit_report(Edit::Refresh);
            }
            Edit::ReviewGap(range) => {
                let date = Local
                    .timestamp_millis_opt(range.start)
                    .unwrap()
                    .date_naive();
                self.set_screen("Day");
                self.change_date(date);
                self.selection = model::clip(range, self.day);
                self.summary();
            }
            Edit::OpenDay(date) => {
                self.set_screen("Day");
                self.change_date(date);
            }
            Edit::Refresh => {
                if self.screen == "Overview" {
                    return Task::none();
                }
                let (from, to) = if self.screen == "Week" {
                    let start = self
                        .date
                        .checked_sub_days(Days::new(u64::from(
                            self.date.weekday().num_days_from_monday(),
                        )))
                        .unwrap_or(self.date);
                    (
                        start.to_string(),
                        start
                            .checked_add_days(Days::new(6))
                            .unwrap_or(start)
                            .to_string(),
                    )
                } else {
                    if self.report_ui.from.is_empty() {
                        self.report_ui.from = self.date.to_string();
                        self.report_ui.to = self.date.to_string();
                    }
                    (self.report_ui.from.clone(), self.report_ui.to.clone())
                };
                if let Err(error) = date_range(&from, &to) {
                    self.report_ui.error = error;
                    return Task::none();
                }
                self.report_ui.generation += 1;
                let generation = self.report_ui.generation;
                self.report_ui.error.clear();
                self.report_ui.data = None;
                if self.fixture {
                    let (start, end, _) = date_range(&from, &to).unwrap();
                    let mut date = start;
                    let mut days = Vec::new();
                    let mut total = Report::default();
                    loop {
                        let range = model::day_bounds(date);
                        let now = draugr_time_daemon::now_ms();
                        let tracks = fixture_tracks(range, &self.preview_apps, now);
                        let report = if range.start >= now {
                            Report {
                                from: range.start,
                                to: range.end,
                                ..Default::default()
                            }
                        } else {
                            from_tracks(&tracks, &[], Range::new(range.start, range.end.min(now)))
                                .unwrap()
                        };
                        total.active_ms += report.active_ms;
                        let mut day = Day::summarize(date, range, &report);
                        if self.screen == "Week" && range.start < now {
                            day.bands = bands_from_tracks(&tracks, &report, range, &[]);
                            day.away = tracks[1]
                                .segments
                                .iter()
                                .filter(|s| s.key.as_ref() == "away")
                                .map(|s| s.range)
                                .collect();
                        }
                        days.push(day);
                        total.intervals.extend(report.intervals);
                        if date == end {
                            break;
                        }
                        date = date.succ_opt().unwrap();
                    }
                    total.from = days[0].range.start;
                    total.to = days.last().unwrap().range.end;
                    let previous = if self.screen == "Week" {
                        let range = week::previous_week(start).unwrap();
                        let mut total = Report::default();
                        let mut date = start.checked_sub_days(Days::new(7)).unwrap();
                        for _ in 0..7 {
                            let day = model::day_bounds(date);
                            let tracks = fixture_tracks(
                                day,
                                &self.preview_apps,
                                draugr_time_daemon::now_ms(),
                            );
                            total
                                .intervals
                                .extend(from_tracks(&tracks, &[], day).unwrap().intervals);
                            date = date.succ_opt().unwrap();
                        }
                        total.from = range.start;
                        total.to = range.end;
                        total.active_ms =
                            total.intervals.iter().map(|i| i.end_ts - i.start_ts).sum();
                        total
                            .rows(GroupBy::Tag, None)
                            .into_iter()
                            .map(|r| (r.tag_id, r.duration_ms))
                            .collect()
                    } else {
                        BTreeMap::new()
                    };
                    self.report_ui.data = Some(Data {
                        previous,
                        from,
                        to,
                        report: total,
                        days,
                        tags: self.tags.clone(),
                    });
                    return Task::none();
                }
                let Some(runtime) = &self.runtime else {
                    self.report_ui.error = tr("Start recording to load history");
                    return Task::none();
                };
                let shared = runtime.shared.clone();
                let week = self.screen == "Week";
                self.report_ui.loading = true;
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || load(shared, from, to, week))
                            .await
                            .map_err(|e| e.to_string())
                            .and_then(|v| v)
                    },
                    move |r| Message::Report(Edit::Loaded(generation, r)),
                );
            }
            Edit::Loaded(g, r) => {
                if g != self.report_ui.generation {
                    drop(r);
                    memory::SystemReclaimer.reclaim();
                    return Task::none();
                }
                self.report_ui.loading = false;
                match r {
                    Ok(data) => {
                        self.report_ui.data = Some(data);
                        self.report_ui.error.clear();
                    }
                    Err(e) => self.report_ui.error = e,
                }
            }
            Edit::Copy => {
                let Some(data) = &self.report_ui.data else {
                    return Task::none();
                };
                return iced::clipboard::write(
                    data.report
                        .csv(group(self.report_ui.group), self.report_ui.filter),
                );
            }
            Edit::Export(json) => {
                let Some(data) = &self.report_ui.data else {
                    return Task::none();
                };
                let report = data.report.clone();
                let timesheet =
                    (self.screen == "Week").then(|| week::timesheet_csv(&data.report, &data.days));
                let grouping = group(self.report_ui.group);
                let filter = self.report_ui.filter;
                let shared = self.runtime.as_ref().map(|r| r.shared.clone());
                return Task::perform(
                    async move {
                        let Some(file) = rfd::AsyncFileDialog::new()
                            .set_file_name(if json {
                                "draugr-history.json"
                            } else {
                                "draugr-report.csv"
                            })
                            .save_file()
                            .await
                        else {
                            return Ok(());
                        };
                        tokio::task::spawn_blocking(move || {
                            let bytes = if json {
                                let shared = shared.ok_or_else(|| {
                                    "Start recording to export history".to_owned()
                                })?;
                                let archive = shared
                                    .export_archive(report.from, report.to)
                                    .map_err(|e| e.to_string())?;
                                serde_json::to_vec_pretty(&archive).map_err(|e| e.to_string())?
                            } else {
                                timesheet
                                    .unwrap_or_else(|| report.csv(grouping, filter))
                                    .into_bytes()
                            };
                            write_export(file.path(), &bytes)
                        })
                        .await
                        .map_err(|e| e.to_string())
                        .and_then(|v| v)
                    },
                    |r| Message::Report(Edit::ExportDone(r)),
                );
            }
            Edit::ExportDone(r) => {
                self.report_ui.error = r.err().unwrap_or_else(|| tr("Export complete"))
            }
            Edit::Import => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .add_filter("JSON", &["json"])
                            .pick_file()
                            .await
                            .map(|f| f.path().to_path_buf())
                    },
                    |p| Message::Report(Edit::File(p)),
                )
            }
            Edit::File(Some(path)) => {
                let Some(runtime) = &self.runtime else {
                    self.error = tr("Start recording to import history");
                    return Task::none();
                };
                let shared = runtime.shared.clone();
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
                            shared
                                .storage
                                .lock()
                                .map_err(|e| e.to_string())?
                                .import_json(&bytes)
                                .map_err(|e| e.to_string())
                        })
                        .await
                        .map_err(|e| e.to_string())
                        .and_then(|v| v)
                    },
                    |r| Message::Report(Edit::ImportDone(r)),
                );
            }
            Edit::File(None) => {}
            Edit::ImportDone(r) => match r {
                Ok(count) => {
                    self.error = format!("{}: {count}", tr("Imported events"));
                    self.invalidate_load();
                    self.report_ui.data = None;
                }
                Err(e) => self.error = e,
            },
        }
        Task::none()
    }
    pub fn reports_view(&self) -> Element<'_, Message> {
        if self.screen == "Overview" {
            return self.overview_view();
        }
        if self.screen == "Week" {
            return self.week_view(false);
        }
        let ui = &self.report_ui;
        let mut body = column![text(tr(self.screen)).size(22)].spacing(8);
        if self.screen == "Reports" {
            body = body.push(
                row![
                    button(text(tr("Day"))).on_press(Message::Report(Edit::Preset(0))),
                    button(text(tr("Week"))).on_press(Message::Report(Edit::Preset(1))),
                    button(text(tr("Month"))).on_press(Message::Report(Edit::Preset(2)))
                ]
                .spacing(6),
            );
            body = body.push(
                row![
                    text_input("YYYY-MM-DD", &ui.from).on_input(|s| Message::Report(Edit::From(s))),
                    text_input("YYYY-MM-DD", &ui.to).on_input(|s| Message::Report(Edit::To(s))),
                    button(text(tr("Load report"))).on_press(Message::Report(Edit::Refresh))
                ]
                .spacing(6),
            );
        } else if ui.data.is_none() {
            body = body.push(button(text(tr("Refresh"))).on_press(Message::Report(Edit::Refresh)));
        }
        if ui.loading {
            body = body.push(text(tr("Loading…")));
        }
        if !ui.error.is_empty() {
            body = body.push(text(localization::error(&ui.error)));
        }
        let Some(data) = &ui.data else {
            return body.push(text(tr("Choose dates and load a report"))).into();
        };
        let summary = format!(
            "{} – {} · {}: {}",
            data.from,
            data.to,
            tr("Active"),
            model::duration(data.report.active_ms)
        );
        body = body.push(text(summary));
        {
            body = body.push(
                row![
                    button(text(tr("Tags"))).on_press(Message::Report(Edit::Group(0))),
                    button(text(tr("Applications"))).on_press(Message::Report(Edit::Group(1))),
                    button(text(tr("Documents"))).on_press(Message::Report(Edit::Group(2)))
                ]
                .spacing(6),
            );
            let mut filters =
                column![button(text(tr("All tags"))).on_press(Message::Report(Edit::Filter(None)))]
                    .spacing(3);
            for r in data.report.rows(GroupBy::Tag, None) {
                if let Some(id) = r.tag_id {
                    filters = filters.push(
                        button(text(r.label)).on_press(Message::Report(Edit::Filter(Some(id)))),
                    );
                }
            }
            body = body.push(scrollable(filters).height(85));
            for r in data.report.rows(group(ui.group), ui.filter) {
                body = body.push(
                    row![
                        text(if ui.group == 1 {
                            translate_application(&r.label)
                        } else {
                            translate_label(&r.label)
                        })
                        .width(Length::Fill),
                        text(model::duration(r.duration_ms)).width(85),
                        text(format!("{:.1}%", r.share * 100.0)).width(65)
                    ]
                    .spacing(6),
                );
                let share = (r.share * 1000.0).round().clamp(1.0, 999.0) as u16;
                body = body.push(row![
                    container(space::horizontal())
                        .height(5)
                        .width(Length::FillPortion(share))
                        .style(|_: &Theme| container::Style {
                            background: Some(Color::from_rgb8(100, 140, 195).into()),
                            ..Default::default()
                        }),
                    space::horizontal().width(Length::FillPortion(1000 - share))
                ]);
            }
            body = body.push(
                row![
                    button(text(tr("Export CSV"))).on_press(Message::Report(Edit::Export(false))),
                    button(text(tr("Export JSON"))).on_press(Message::Report(Edit::Export(true))),
                    button(text(tr("Copy as text"))).on_press(Message::Report(Edit::Copy))
                ]
                .spacing(6),
            );
            body = body.push(text(tr("Timesheet · tags × local days")).size(18));
            let mut table = row![text(tr("Tags")).width(150)].spacing(8);
            for day in &data.days {
                table = table.push(text(day.date.format("%m-%d").to_string()).width(80));
            }
            table = table.push(text(tr("Total")).width(80));
            let mut sheet = column![table].spacing(5);
            for r in data.report.rows(GroupBy::Tag, ui.filter) {
                let mut line = row![text(translate_label(&r.label)).width(150)].spacing(8);
                for day in &data.days {
                    let ms = day.tag_totals.get(&r.tag_id).copied().unwrap_or(0);
                    line = line.push(text(model::duration(ms)).width(80));
                }
                sheet = sheet.push(line.push(text(model::duration(r.duration_ms)).width(80)));
            }
            let mut totals = row![text(tr("Total")).width(150)].spacing(8);
            for day in &data.days {
                let total = day
                    .tag_totals
                    .iter()
                    .filter(|(id, _)| ui.filter.is_none_or(|filter| **id == Some(filter)))
                    .map(|(_, ms)| *ms)
                    .sum();
                totals = totals.push(text(model::duration(total)).width(80));
            }
            sheet = sheet.push(
                totals.push(
                    text(model::duration(
                        data.report
                            .rows(GroupBy::Tag, ui.filter)
                            .iter()
                            .map(|r| r.duration_ms)
                            .sum(),
                    ))
                    .width(80),
                ),
            );
            body = body.push(
                scrollable(sheet).direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::default(),
                )),
            );
        }
        scrollable(body).into()
    }
    fn overview_view(&self) -> Element<'_, Message> {
        self.report_ui.overview_timeline_cache.clear();
        let report = &self.report_ui.day;
        let chart_slices = slices(report, self.report_ui.expanded);
        let mut body=column![text(tr("Day overview")).size(22),text(tr("Overlapping tags use the newest range; child tags count under their top-level project."))].spacing(8);
        if report.active_ms == 0 {
            body=body.push(text(tr("No observed active time for this day. Inactive time and recording gaps remain on the timeline.")));
        } else {
            body = body.push(text(tr("↑ / ↓ highlight · E expand · Esc clear")));
            body = body.push(
                widget::responsive(move |size| {
                    let chart: Element<'_, Message> = Canvas::new(Donut {
                        report,
                        slices: chart_slices.clone(),
                        tags: &self.tags,
                        stats: &self.stats,
                        highlight: self.report_ui.highlight.clone(),
                    })
                    .width(Length::Fill)
                    .height(360)
                    .into();
                    let mut legend = column![overview_columns(
                        text(tr("Name")).into(),
                        tr("Duration"),
                        tr("Share")
                    )]
                    .spacing(4);
                    let mut previous = None;
                    for slice in &chart_slices {
                        let key = (slice.tag_id, slice.tag.clone());
                        if previous.as_ref() != Some(&key) {
                            let total = report
                                .intervals
                                .iter()
                                .filter(|i| i.tag_id == slice.tag_id)
                                .map(|i| i.end_ts - i.start_ts)
                                .sum::<i64>();
                            legend = legend.push(
                                button(overview_columns(
                                    row![
                                        container(text("●")).width(20),
                                        text(translate_label(&slice.tag))
                                    ]
                                    .spacing(6)
                                    .into(),
                                    model::duration(total),
                                    format!(
                                        "{:.1}%",
                                        total as f64 / report.active_ms as f64 * 100.0
                                    ),
                                ))
                                .width(Length::Fill)
                                .on_press(Message::Report(Edit::Highlight(Some((
                                    slice.tag_id,
                                    None,
                                )))))
                                .style(
                                    if self.report_ui.highlight == Some((slice.tag_id, None)) {
                                        design::selected
                                    } else {
                                        design::ghost
                                    },
                                ),
                            );
                            previous = Some(key);
                        }
                        let label = format!(
                            "{} · {} · {:.1}%",
                            translate_application(&slice.application),
                            model::duration(slice.duration),
                            slice.duration as f64 / report.active_ms as f64 * 100.0
                        );
                        legend = legend.push(tooltip(
                            button(overview_columns(
                                row![
                                    self.app_icon(&slice.application, &slice.application),
                                    text(translate_application(&slice.application)).size(13)
                                ]
                                .spacing(6)
                                .align_y(Alignment::Center)
                                .into(),
                                model::duration(slice.duration),
                                format!(
                                    "{:.1}%",
                                    slice.duration as f64 / report.active_ms as f64 * 100.0
                                ),
                            ))
                            .width(Length::Fill)
                            .style(if self.report_ui.highlight == slice.key() {
                                design::selected
                            } else {
                                design::ghost
                            })
                            .on_press(Message::Report(Edit::Highlight(slice.key()))),
                            text(label),
                            tooltip::Position::FollowCursor,
                        ));
                        if !slice.members.is_empty() {
                            legend = legend.push(
                                container(
                                    text(
                                        slice
                                            .members
                                            .iter()
                                            .map(|name| translate_application(name))
                                            .collect::<Vec<_>>()
                                            .join(", "),
                                    )
                                    .size(11),
                                )
                                .padding([0, 12]),
                            );
                        }
                    }
                    legend = legend
                        .push(
                            button(text(tr(if self.report_ui.expanded {
                                "Collapse small slices"
                            } else {
                                "Expand small slices"
                            })))
                            .on_press(Message::Report(Edit::Expand)),
                        )
                        .push(
                            button(text(tr("Clear highlight")))
                                .on_press(Message::Report(Edit::Highlight(None))),
                        );
                    if size.width >= 750.0 {
                        row![chart, scrollable(legend).width(Length::Fill).height(360)]
                            .spacing(12)
                            .into()
                    } else {
                        column![chart, scrollable(legend).height(220)]
                            .spacing(8)
                            .into()
                    }
                })
                .height(Length::Shrink),
            );
        }
        body = body.push(
            Canvas::new(Compact {
                cache: &self.report_ui.overview_timeline_cache,
                tracks: &self.all,
                range: self.day,
                report,
                highlight: self.report_ui.highlight.as_ref(),
                tags: &self.tags,
                stats: &self.stats,
                other: slices(report, false)
                    .into_iter()
                    .filter(|s| s.application == "Other")
                    .collect(),
            })
            .width(Length::Fill)
            .height(88),
        );
        body = body
            .push(text(tr(
                "Tagged / untagged · AFK · sleep / off · pause · no recording · Unobserved",
            )))
            .push(button(text(tr("Back to day"))).on_press(Message::Screen("Day")));
        scrollable(body).into()
    }
}
fn group(i: usize) -> GroupBy {
    match i {
        1 => GroupBy::Application,
        2 => GroupBy::Document,
        _ => GroupBy::Tag,
    }
}
fn translate_application(name: &str) -> String {
    translate_label(&crate::icons::application_name(name))
}
fn translate_label(s: &str) -> String {
    if [
        "Untagged",
        "Unknown application",
        "Unknown document",
        "Other",
    ]
    .contains(&s)
    {
        tr(s)
    } else {
        s.into()
    }
}
fn tag_color(tags: &[Tag], id: Option<i64>) -> Color {
    if id.is_none() {
        return Color::from_rgb8(145, 155, 170);
    }
    let rgb = id
        .and_then(|id| tags.iter().find(|t| t.id == id))
        .and_then(|t| t.color.as_deref())
        .and_then(|s| s.strip_prefix('#'))
        .and_then(|s| u32::from_str_radix(s, 16).ok());
    if let Some(rgb) = rgb {
        Color::from_rgb8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
    } else {
        let rgb = model::color(
            &id.map(|v| v.to_string())
                .unwrap_or_else(|| "untagged".into()),
        );
        Color::from_rgb(rgb[0], rgb[1], rgb[2])
    }
}
fn selected(i: &draugr_time_core::ReportInterval, h: &Highlight, other: &[Slice]) -> bool {
    h.as_ref().is_some_and(|(tag, app)| {
        i.tag_id == *tag
            && app.as_ref().is_none_or(|app| {
                i.application.as_ref() == app
                    || (app == "Other"
                        && other.iter().any(|s| {
                            s.tag_id == *tag
                                && s.members.iter().any(|m| m == i.application.as_ref())
                        }))
            })
    })
}
fn overview_columns<'a>(
    name: Element<'a, Message>,
    duration: impl Into<String>,
    share: impl Into<String>,
) -> Element<'a, Message> {
    row![
        container(name).width(Length::Fill),
        container(text(duration.into()).size(13))
            .width(90)
            .align_right(90),
        container(text(share.into()).size(13))
            .width(65)
            .align_right(65)
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}
struct Compact<'a> {
    cache: &'a canvas::Cache,
    tracks: &'a [Track],
    range: Range,
    report: &'a Report,
    highlight: Option<&'a (Option<i64>, Option<String>)>,
    other: Vec<Slice>,
    tags: &'a [Tag],
    stats: &'a timeline::PaintStats,
}
impl canvas::Program<Message> for Compact<'_> {
    type State = ();
    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let started = Instant::now();
        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            let x = |ts| {
                ((ts - self.range.start) as f64 / self.range.duration().max(1) as f64
                    * bounds.width as f64) as f32
            };
            let colors = theme.extended_palette();
            let row_height = (bounds.height - 18.0) / 2.0;
            frame.fill_rectangle(
                Point::new(0.0, 18.0),
                Size::new(bounds.width, row_height * 2.0),
                colors.background.weak.color,
            );
            for s in &self.tracks[1].segments {
                frame.fill_rectangle(
                    Point::new(x(s.range.start), 18.0),
                    Size::new((x(s.range.end) - x(s.range.start)).max(0.6), row_height),
                    Color::from_rgb(s.color[0], s.color[1], s.color[2]),
                );
            }
            for segment in self.tracks[2]
                .segments
                .iter()
                .filter(|s| s.key.as_ref() == "unobserved")
            {
                frame.fill_rectangle(
                    Point::new(x(segment.range.start), 18.0),
                    Size::new(
                        (x(segment.range.end) - x(segment.range.start)).max(0.6),
                        6.0,
                    ),
                    Color::from_rgb8(195, 95, 95),
                );
            }
            for i in &self.report.intervals {
                let color = if selected(i, &self.highlight.cloned(), &self.other) {
                    colors.primary.strong.color
                } else if i.tag_id.is_none() {
                    Color::from_rgb8(145, 155, 170)
                } else {
                    tag_color(self.tags, i.tag_id)
                };
                frame.fill_rectangle(
                    Point::new(x(i.start_ts), 18.0 + row_height),
                    Size::new((x(i.end_ts) - x(i.start_ts)).max(0.6), row_height),
                    color,
                );
            }
            for (ts, label) in hour_ticks(self.range).into_iter().filter(|(_, label)| {
                matches!(label.as_str(), "00:00" | "06:00" | "12:00" | "18:00")
            }) {
                frame.fill_text(Text {
                    content: label,
                    position: Point::new(x(ts), 0.0),
                    size: 11.0.into(),
                    color: colors.background.base.text,
                    ..Default::default()
                });
            }
            self.stats.frames.fetch_add(1, Ordering::Relaxed);
            self.stats
                .max_build_us
                .fetch_max(started.elapsed().as_micros() as u64, Ordering::Relaxed);
        });
        vec![geometry]
    }
}
struct Donut<'a> {
    report: &'a Report,
    slices: Vec<Slice>,
    tags: &'a [Tag],
    stats: &'a timeline::PaintStats,
    highlight: Highlight,
}
impl Donut<'_> {
    fn hit(&self, p: Point, size: Size) -> Highlight {
        let center = Point::new(size.width / 2.0, size.height / 2.0);
        let dx = p.x - center.x;
        let dy = p.y - center.y;
        let radius = dx.hypot(dy);
        let outer = size.width.min(size.height) * 0.44;
        if radius < outer * 0.46
            || radius > outer
            || (radius > outer * 0.72 && radius < outer * 0.75)
        {
            return None;
        }
        let angle = (dy.atan2(dx) + std::f32::consts::FRAC_PI_2).rem_euclid(std::f32::consts::TAU);
        let mut position = 0.0;
        for s in &self.slices {
            position +=
                s.duration as f32 / self.report.active_ms.max(1) as f32 * std::f32::consts::TAU;
            if angle < position {
                return Some((
                    s.tag_id,
                    if radius > outer * 0.73 {
                        Some(s.application.clone())
                    } else {
                        None
                    },
                ));
            }
        }
        None
    }
}
impl canvas::Program<Message> for Donut<'_> {
    type State = ();
    fn update(
        &self,
        _: &mut (),
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if matches!(
            event,
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
        ) {
            return cursor
                .position_in(bounds)
                .and_then(|p| self.hit(p, bounds.size()))
                .map(|h| {
                    canvas::Action::publish(Message::Report(Edit::Highlight(Some(h)))).and_capture()
                });
        }
        None
    }
    fn mouse_interaction(
        &self,
        _: &(),
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor
            .position_in(bounds)
            .and_then(|p| self.hit(p, bounds.size()))
            .is_some()
        {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let started = Instant::now();
        let mut frame = Frame::new(renderer, bounds.size());
        let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
        let radius = bounds.width.min(bounds.height) * 0.44;
        let mut angle = -std::f32::consts::FRAC_PI_2;
        let hover = cursor
            .position_in(bounds)
            .and_then(|p| self.hit(p, bounds.size()));
        let mut tooltip = None;
        for (index, s) in self.slices.iter().enumerate() {
            let end = angle
                + s.duration as f32 / self.report.active_ms.max(1) as f32 * std::f32::consts::TAU;
            let base = tag_color(self.tags, s.tag_id);
            let shade = 0.65 + (index % 5) as f32 * 0.07;
            let app_color = Color {
                r: base.r * shade,
                g: base.g * shade,
                b: base.b * shade,
                ..base
            };
            let first = index == 0 || self.slices[index - 1].tag_id != s.tag_id;
            let project_end = angle
                + self
                    .slices
                    .iter()
                    .skip(index)
                    .take_while(|slice| slice.tag_id == s.tag_id)
                    .map(|slice| slice.duration)
                    .sum::<i64>() as f32
                    / self.report.active_ms.max(1) as f32
                    * std::f32::consts::TAU;
            for (inner, outer, color, sector_end) in [
                (radius * 0.46, radius * 0.72, base, project_end),
                (radius * 0.75, radius, app_color, end),
            ] {
                if inner == radius * 0.46 && !first {
                    continue;
                }
                let steps = ((sector_end - angle) * radius / 3.0).ceil().max(2.0) as usize;
                let path = Path::new(|b| {
                    for j in 0..=steps {
                        let a = angle + (sector_end - angle) * j as f32 / steps as f32;
                        let p = Point::new(center.x + outer * a.cos(), center.y + outer * a.sin());
                        if j == 0 {
                            b.move_to(p);
                        } else {
                            b.line_to(p);
                        }
                    }
                    for j in (0..=steps).rev() {
                        let a = angle + (sector_end - angle) * j as f32 / steps as f32;
                        b.line_to(Point::new(
                            center.x + inner * a.cos(),
                            center.y + inner * a.sin(),
                        ));
                    }
                    b.close();
                });
                frame.fill(&path, color);
                if self.highlight.as_ref().is_some_and(|(tag, app)| {
                    *tag == s.tag_id && app.as_ref().is_none_or(|a| a == &s.application)
                }) || hover.as_ref().is_some_and(|(tag, app)| {
                    *tag == s.tag_id && app.as_ref().is_none_or(|a| a == &s.application)
                }) {
                    frame.stroke(
                        &path,
                        Stroke::default()
                            .with_color(theme.extended_palette().background.base.text)
                            .with_width(2.0),
                    );
                }
            }
            if hover
                .as_ref()
                .is_some_and(|(tag, app)| *tag == s.tag_id && app.as_ref() == Some(&s.application))
            {
                tooltip = Some(format!(
                    "{} · {} · {:.1}%",
                    translate_application(&s.application),
                    model::duration(s.duration),
                    s.duration as f64 / self.report.active_ms as f64 * 100.0
                ));
            }
            angle = end;
        }
        if let Some((tag, None)) = &hover {
            let total = self
                .report
                .intervals
                .iter()
                .filter(|i| i.tag_id == *tag)
                .map(|i| i.end_ts - i.start_ts)
                .sum::<i64>();
            let name = self
                .report
                .intervals
                .iter()
                .find(|i| i.tag_id == *tag)
                .map(|i| translate_label(&i.tag))
                .unwrap_or_default();
            tooltip = Some(format!(
                "{name} · {} · {:.1}%",
                model::duration(total),
                total as f64 / self.report.active_ms.max(1) as f64 * 100.0
            ));
        }
        frame.fill_text(Text {
            content: format!(
                "{}\n{}",
                model::duration(self.report.active_ms),
                tr("Active")
            ),
            position: Point::new(center.x, center.y - 15.0),
            align_x: iced::alignment::Horizontal::Center.into(),
            color: theme.extended_palette().background.base.text,
            size: 20.0.into(),
            ..Default::default()
        });
        if let Some(label) = tooltip {
            frame.fill_text(Text {
                content: label,
                position: Point::new(8.0, bounds.height - 16.0),
                color: theme.extended_palette().background.base.text,
                size: 12.0.into(),
                ..Default::default()
            });
        }
        self.stats.frames.fetch_add(1, Ordering::Relaxed);
        self.stats
            .max_build_us
            .fetch_max(started.elapsed().as_micros() as u64, Ordering::Relaxed);
        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use canvas::Program;
    #[test]
    fn overview_timeline_reuses_geometry_until_view_changes() {
        let (mut state, _) = State::boot();
        state.all = model::fixture(state.day);
        state.rebuild();
        let renderer = Renderer::new(iced::Font::default(), iced::Pixels(16.0));
        let theme = native_theme(true);
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(900.0, 88.0));
        let draw = || {
            let compact = Compact {
                cache: &state.report_ui.overview_timeline_cache,
                tracks: &state.all,
                range: state.day,
                report: &state.report_ui.day,
                highlight: state.report_ui.highlight.as_ref(),
                other: vec![],
                tags: &state.tags,
                stats: &state.stats,
            };
            compact.draw(&(), &renderer, &theme, bounds, mouse::Cursor::Unavailable);
        };
        let before = state.stats.frames.load(Ordering::Relaxed);
        draw();
        draw();
        assert_eq!(state.stats.frames.load(Ordering::Relaxed), before + 1);
        let _ = state.overview_view();
        draw();
        assert_eq!(state.stats.frames.load(Ordering::Relaxed), before + 2);
    }
    #[test]
    fn native_donut_pointer_selects_project_or_application_and_shows_cursor() {
        let report = Report {
            from: 0,
            to: 100,
            active_ms: 100,
            intervals: vec![draugr_time_core::ReportInterval {
                start_ts: 0,
                end_ts: 100,
                tag_id: Some(1),
                tag: "Work".into(),
                application: "Editor".into(),
                document: "draft".into(),
            }],
        };
        let stats = timeline::PaintStats::default();
        let donut = Donut {
            report: &report,
            slices: slices(&report, false),
            tags: &[],
            stats: &stats,
            highlight: None,
        };
        let bounds = Rectangle::with_size(Size::new(300.0, 300.0));
        let outer = mouse::Cursor::Available(Point::new(150.0, 35.0));
        let inner = mouse::Cursor::Available(Point::new(150.0, 70.0));
        for (cursor, app) in [(outer, Some("Editor".to_owned())), (inner, None)] {
            let action = donut
                .update(
                    &mut (),
                    &iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                    bounds,
                    cursor,
                )
                .unwrap();
            let (message, _, status) = action.into_inner();
            assert_eq!(status, iced::event::Status::Captured);
            assert!(
                matches!(message,Some(Message::Report(Edit::Highlight(Some((Some(1),value))))) if value==app)
            );
            assert_eq!(
                donut.mouse_interaction(&(), bounds, cursor),
                mouse::Interaction::Pointer
            );
        }
        assert_eq!(donut.hit(Point::new(150.0, 150.0), bounds.size()), None);
        let renderer = Renderer::new(iced::Font::DEFAULT, 16.0.into());
        assert_eq!(
            donut
                .draw(&(), &renderer, &native_theme(true), bounds, outer)
                .len(),
            1
        );
        assert_eq!(stats.frames.load(Ordering::Relaxed), 1);
    }
    #[test]
    fn week_and_report_totals_equal_day_tracks_and_share_core_rows() {
        let shared = draugr_time_daemon::Shared::new(
            draugr_time_core::Storage::open_in_memory().unwrap(),
            "test".into(),
            300,
        );
        let date = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        let day = model::day_bounds(date);
        {
            let mut db = shared.storage.lock().unwrap();
            db.record_interval(
                "afk",
                &serde_json::json!({"status":"active"}),
                day.end - 60_000,
                day.end + 120_000,
            )
            .unwrap();
            db.record_interval(
                "window",
                &serde_json::json!({"app":"editor","app_name":"Editor","title":"draft"}),
                day.end - 60_000,
                day.end + 120_000,
            )
            .unwrap();
            db.tag_interval("Client,Task", "", day.end - 60_000, day.end + 60_000, false)
                .unwrap();
        }
        let data = load(
            shared.clone(),
            date.to_string(),
            date.succ_opt().unwrap().to_string(),
            true,
        )
        .unwrap();
        let tags = shared.storage.lock().unwrap().tags().unwrap();
        assert_eq!(
            data.report.active_ms,
            data.days.iter().map(|d| d.active_ms).sum::<i64>()
        );
        for d in &data.days {
            assert_eq!(
                {
                    let db = shared.storage.lock().unwrap();
                    let tracks = model::tracks(
                        &db.events(None, d.range.start, d.range.end).unwrap(),
                        &tags,
                        &db.tag_ranges(d.range.start, d.range.end).unwrap(),
                        d.range,
                    );
                    from_tracks(&tracks, &tags, d.range).unwrap().active_ms
                },
                d.active_ms
            );
        }
        assert_eq!(
            data.report
                .rows(GroupBy::Tag, None)
                .iter()
                .map(|r| r.duration_ms)
                .sum::<i64>(),
            180_000
        );
    }
}

#[cfg(test)]
mod monthly_tests {
    use super::*;
    #[test]
    fn month_report_equals_daily_timesheet_and_clips_month_boundary() {
        let shared = draugr_time_daemon::Shared::new(
            draugr_time_core::Storage::open_in_memory().unwrap(),
            "test".into(),
            300,
        );
        let mut date = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        let first = date;
        for index in 0..31 {
            let range = model::day_bounds(date);
            let start = range.start + 9 * 3_600_000;
            let mut db = shared.storage.lock().unwrap();
            db.record_interval(
                "afk",
                &serde_json::json!({"status":"active"}),
                start,
                start + 60_000,
            )
            .unwrap();
            db.record_interval(
                "window",
                &serde_json::json!({"app":"editor","title":"work"}),
                start,
                start + 60_000,
            )
            .unwrap();
            if index % 2 == 0 {
                db.tag_interval("Client,Task", "", start, start + 60_000, false)
                    .unwrap();
            }
            drop(db);
            date = date.succ_opt().unwrap();
        }
        let last = model::day_bounds(date).start;
        shared
            .storage
            .lock()
            .unwrap()
            .record_interval(
                "afk",
                &serde_json::json!({"status":"active"}),
                last - 30_000,
                last + 30_000,
            )
            .unwrap();
        let data = load(
            shared.clone(),
            first.to_string(),
            "2026-10-31".into(),
            false,
        )
        .unwrap();
        assert_eq!(data.days.len(), 31);
        assert_eq!(data.report.active_ms, 31 * 60_000 + 30_000);
        assert_eq!(
            data.report.active_ms,
            data.days.iter().map(|d| d.active_ms).sum::<i64>()
        );
        assert_eq!(data.report.to, last);
        assert!(data.report.intervals.iter().all(|i| i.end_ts <= last));
        for row in data.report.rows(GroupBy::Tag, None) {
            assert_eq!(
                row.duration_ms,
                data.days
                    .iter()
                    .map(|d| d.tag_totals.get(&row.tag_id).copied().unwrap_or(0))
                    .sum::<i64>()
            );
        }
    }
}

#[cfg(test)]
mod memory_regression_tests {
    use super::*;
    #[test]
    fn daily_totals_split_intervals_without_copying_history() {
        let date = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        let first = model::day_bounds(date);
        let second = model::day_bounds(date.succ_opt().unwrap());
        let report = Report {
            from: first.start,
            to: second.end,
            active_ms: 180_000,
            intervals: vec![draugr_time_core::ReportInterval {
                start_ts: first.end - 60_000,
                end_ts: first.end + 120_000,
                tag_id: Some(1),
                tag: "Work".into(),
                application: "Editor".into(),
                document: "draft".into(),
            }],
        };
        let a = Day::summarize(date, first, &report);
        let b = Day::summarize(date.succ_opt().unwrap(), second, &report);
        assert_eq!(a.active_ms, 60_000);
        assert_eq!(b.active_ms, 120_000);
        assert_eq!(
            a.tag_totals[&Some(1)] + b.tag_totals[&Some(1)],
            report.active_ms
        );
    }
    #[test]
    fn minute_bands_are_bounded_and_grid_cache_tracks_theme_size_and_highlight() {
        let date = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        let range = model::day_bounds(date);
        let tracks = model::fixture(range);
        let report = from_tracks(&tracks, &[], range).unwrap();
        let mut day = Day::summarize(date, range, &report);
        day.bands = bands_from_tracks(&tracks, &report, range, &[]);
        assert!(day.bands.iter().map(Vec::len).sum::<usize>() <= 3 * 1440);
        let days = [day];
        let stats = timeline::PaintStats::default();
        let mut grid = WeekCanvas {
            days: &days,
            report: &report,
            tags: &[],
            highlight: &None,
            now: range.end,
            stats: &stats,
        };
        let renderer = Renderer::new(iced::Font::DEFAULT, 16.0.into());
        use canvas::Program;
        let bounds = Rectangle::with_size(Size::new(800.0, 76.0));
        for _ in 0..2 {
            grid.draw(
                &(),
                &renderer,
                &native_theme(false),
                bounds,
                mouse::Cursor::Unavailable,
            );
        }
        assert_eq!(stats.frames.load(Ordering::Relaxed), 1);
        grid.draw(
            &(),
            &renderer,
            &native_theme(true),
            bounds,
            mouse::Cursor::Unavailable,
        );
        assert_eq!(stats.frames.load(Ordering::Relaxed), 2);
        grid.draw(
            &(),
            &renderer,
            &native_theme(true),
            Rectangle::with_size(Size::new(900.0, 76.0)),
            mouse::Cursor::Unavailable,
        );
        assert_eq!(stats.frames.load(Ordering::Relaxed), 3);
        let selected = Some((None, None));
        grid.highlight = &selected;
        grid.draw(
            &(),
            &renderer,
            &native_theme(true),
            bounds,
            mouse::Cursor::Unavailable,
        );
        assert_eq!(stats.frames.load(Ordering::Relaxed), 4);
    }
    #[test]
    fn week_grid_uses_common_hours_hover_and_day_navigation() {
        use canvas::Program;
        let date = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        let range = model::day_bounds(date);
        let start = Local
            .with_ymd_and_hms(2026, 10, 1, 10, 40, 0)
            .unwrap()
            .timestamp_millis();
        let end = Local
            .with_ymd_and_hms(2026, 10, 1, 12, 15, 0)
            .unwrap()
            .timestamp_millis();
        let report = Report {
            from: range.start,
            to: range.end,
            active_ms: end - start,
            intervals: vec![draugr_time_core::ReportInterval {
                start_ts: start,
                end_ts: end,
                tag_id: Some(1),
                tag: "Project X".into(),
                application: "Editor".into(),
                document: "".into(),
            }],
        };
        let days = [Day::summarize(date, range, &report)];
        let stats = timeline::PaintStats::default();
        let grid = WeekCanvas {
            days: &days,
            report: &report,
            tags: &[],
            highlight: &None,
            now: range.end,
            stats: &stats,
        };
        assert_eq!(grid.hours(), (7.0, 21.0));
        let bounds = Rectangle::with_size(Size::new(600.0, 76.0));
        let cursor = mouse::Cursor::Available(Point::new(180.0, 50.0));
        let hover = grid.hover(Point::new(180.0, 50.0), bounds.size()).unwrap();
        assert_eq!(hover.0, Range::new(start, end));
        assert_eq!(hover.1, Some(1));
        assert_eq!(hover.2, "Project X");
        let action = grid
            .update(
                &mut (),
                &iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                bounds,
                cursor,
            )
            .unwrap();
        assert!(
            matches!(action.into_inner().0, Some(Message::Report(Edit::OpenDay(d))) if d==date)
        );
        let future = WeekCanvas {
            now: start - 1,
            ..grid
        };
        assert!(future
            .hover(Point::new(180.0, 50.0), bounds.size())
            .is_none());
    }
}

#[cfg(test)]
mod band_boundary_tests {
    use super::*;
    #[test]
    fn minute_cells_keep_gaps_and_subminute_edges_on_dst_length_days() {
        for hours in [23, 25] {
            let day = Range::new(1000, 1000 + hours * 3_600_000);
            let mut cells = [None; 1440];
            let active = [0.37, 0.65, 0.58];
            let away = [0.83, 0.69, 0.40];
            fill_cells(
                &mut cells,
                Range::new(day.start, day.start + 1),
                active,
                day,
            );
            fill_cells(&mut cells, Range::new(day.end - 1, day.end), away, day);
            assert_eq!(cells[0], Some(active));
            assert_eq!(cells[1439], Some(away));
            assert!(cells[1..1439].iter().all(Option::is_none));
            let bands = compress_cells([cells, [None; 1440], [None; 1440]]);
            assert_eq!(bands[0].len(), 2);
        }
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;
    #[test]
    fn export_replaces_existing_file_and_leaves_no_temporary_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.csv");
        std::fs::write(&path, b"old").unwrap();
        write_export(&path, b"complete replacement").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"complete replacement");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        let target = dir.path().join("directory");
        std::fs::create_dir(&target).unwrap();
        assert!(write_export(&target, b"failure").is_err());
        assert!(target.is_dir());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }
    #[test]
    fn local_hour_ticks_follow_real_clock_on_short_and_long_days() {
        use chrono::{Offset, TimeZone, Timelike};
        // A deterministic timezone with a one-hour DST transition at 02:00.
        for (hours, before, after) in [(23, 3600, 7200), (25, 7200, 3600)] {
            let date = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
            let start = chrono::FixedOffset::east_opt(before)
                .unwrap()
                .from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
                .unwrap()
                .timestamp_millis();
            let end = start + hours * 3_600_000;
            let ticks = hour_ticks_with(Range::new(start, end), |ts| {
                let offset = if ts < start + 2 * 3_600_000 {
                    before
                } else {
                    after
                };
                chrono::FixedOffset::east_opt(offset)
                    .unwrap()
                    .timestamp_millis_opt(ts)
                    .unwrap()
            });
            assert!(ticks.iter().all(|(_, label)| label.ends_with(":00")));
            assert_eq!(ticks.len(), hours as usize);
            let six = ticks.iter().find(|(_, label)| label == "06:00").unwrap().0;
            let clock = chrono::FixedOffset::east_opt(after)
                .unwrap()
                .timestamp_millis_opt(six)
                .unwrap();
            assert_eq!(clock.hour(), 6);
            assert_eq!(clock.offset().fix().local_minus_utc(), after);
        }
    }
    #[test]
    fn week_legacy_afk_matches_loaded_day_after_computer_history_started() {
        let shared = draugr_time_daemon::Shared::new(
            draugr_time_core::Storage::open_in_memory().unwrap(),
            "test".into(),
            300,
        );
        let date = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        let range = model::day_bounds(date);
        {
            let mut db = shared.storage.lock().unwrap();
            db.record_interval(
                "computer",
                &serde_json::json!({"machine":"awake","activity":"active","recording":"recording"}),
                range.start - 86_400_000,
                range.start - 86_399_000,
            )
            .unwrap();
            db.record_interval(
                "afk",
                &serde_json::json!({"status":"active"}),
                range.start + 1000,
                range.start + 2000,
            )
            .unwrap();
        }
        let data = load(shared.clone(), date.to_string(), date.to_string(), true).unwrap();
        let snapshot = Runtime::load(shared, range, Default::default()).unwrap();
        assert_eq!(
            data.report.active_ms,
            from_tracks(&snapshot.tracks, &[], range).unwrap().active_ms
        );
        assert!(data.days[0].bands[0].is_empty());
    }
}
