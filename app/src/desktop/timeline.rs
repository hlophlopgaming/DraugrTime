use super::{
    model::{self, Range, Track},
    Message,
};
use iced::{
    keyboard, mouse,
    widget::canvas::{self, Canvas, Frame, Geometry, Path, Stroke, Text},
    Color, Event, Point, Rectangle, Renderer, Size, Theme,
};
use std::{
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
    time::Instant,
};

fn wheel_zoom_factor(delta: mouse::ScrollDelta) -> f64 {
    let steps = match delta {
        mouse::ScrollDelta::Lines { y, .. } => f64::from(y),
        mouse::ScrollDelta::Pixels { y, .. } => f64::from(y) / 100.0,
    };
    0.9_f64.powf(steps.clamp(-10.0, 10.0))
}

pub const RULER: f32 = 32.0;
#[derive(Default)]
pub struct PaintStats {
    pub frames: AtomicUsize,
    pub base_builds: AtomicUsize,
    pub max_build_us: AtomicU64,
}
#[derive(Default)]
pub struct Interaction {
    drag: Option<Drag>,
    hover: Option<(usize, usize)>,
    shift: bool,
    last_click: Option<(Instant, Point, String)>,
    double: bool,
    tag_drag: Option<(i64, Range, i8)>,
}
struct Drag {
    x: f32,
    old: Option<Range>,
    shift: bool,
    moved: bool,
}
pub struct Timeline<'a> {
    pub tracks: &'a [Track],
    pub filtered: &'a [Vec<usize>],
    pub visible: &'a [usize],
    pub view: Range,
    pub selection: Option<Range>,
    pub highlight: Option<&'a str>,
    pub highlight_from: &'a [(Option<String>, f32)],
    pub highlight_progress: f32,
    pub heights: [f32; 4],
    pub track_progress: [f32; 4],
    pub tag_growth: Option<(Range, f32)>,
    pub grid_cache: &'a canvas::Cache,
    pub cache: &'a canvas::Cache,
    pub stats: &'a PaintStats,
    pub lanes: [Vec<model::Summary>; 4],
    pub gaps: Vec<Range>,
    pub now: Option<i64>,
}
impl Timeline<'_> {
    fn track_top(&self, track: usize) -> f32 {
        RULER
            + self
                .visible
                .iter()
                .take_while(|&&t| t != track)
                .map(|&t| self.heights[t])
                .sum::<f32>()
    }
    fn lane_index(&self, track: usize, key: &str) -> Option<usize> {
        self.lanes[track]
            .iter()
            .position(|r| r.key.as_ref() == key)
            .or_else(|| {
                self.lanes[track]
                    .iter()
                    .position(|r| r.key.as_ref() == "__other")
            })
    }
    fn block_layout(&self, track: usize, key: &str) -> Option<(f32, f32)> {
        let top = self.track_top(track);
        let collapsed_y = super::design::bar_offset(track);
        let collapsed_h = super::design::bar_height(track);
        if self.lanes[track].is_empty() {
            return Some((top + collapsed_y, collapsed_h));
        }
        let lane = self.lane_index(track, key)?;
        let p = self.track_progress[track];
        let y = top + super::design::TRACK_HEAD + lane as f32 * super::design::LANE
            - super::design::MOTION_OFFSET * (1.0 - p);
        Some((
            y,
            super::design::LANE_BAR.min((top + self.heights[track] - y).max(0.0)),
        ))
    }
    fn lane_hit(&self, track: usize, key: &str, y: f32) -> bool {
        self.lanes[track].is_empty()
            || self
                .block_layout(track, key)
                .is_some_and(|(top, height)| y >= top && y <= top + height)
    }

    fn hit(&self, point: Point, width: f32) -> Option<(usize, usize)> {
        if point.y < RULER {
            return None;
        }
        let track = self.visible.iter().copied().find(|&track| {
            let top = self.track_top(track);
            point.y >= top && point.y < top + self.heights[track]
        })?;
        let ts = model::pixel_range(self.view, width, point.x, point.x).start;
        self.filtered[track]
            .iter()
            .rev()
            .copied()
            .find(|index| {
                let segment = &self.tracks[track].segments[*index];
                segment.range.start <= ts
                    && segment.range.end > ts
                    && self.lane_hit(track, segment.key.as_ref(), point.y)
            })
            .map(|index| (track, index))
    }
    fn extended(value: Range, old: Option<Range>) -> Range {
        old.map(|old| Range::new(old.start.min(value.start), old.end.max(value.end)))
            .unwrap_or(value)
    }
}
impl canvas::Program<Message> for Timeline<'_> {
    type State = Interaction;
    fn update(
        &self,
        state: &mut Interaction,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if let Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) = event {
            state.shift = modifiers.shift();
            return None;
        }
        let inside = cursor.position_in(bounds);
        let point = inside.or_else(|| {
            state
                .drag
                .as_ref()
                .and_then(|_| cursor.position())
                .map(|p| {
                    Point::new(
                        (p.x - bounds.x).clamp(0.0, bounds.width),
                        (p.y - bounds.y).clamp(0.0, bounds.height),
                    )
                })
        });
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let point = inside?;
                let hit = self.hit(point, bounds.width);
                let key = hit
                    .map(|(track, index)| self.tracks[track].segments[index].key.to_string())
                    .unwrap_or_default();
                state.double = state
                    .last_click
                    .as_ref()
                    .is_some_and(|(time, position, last)| {
                        time.elapsed().as_millis() < 350
                            && position.distance(point) < 6.0
                            && *last == key
                    });
                state.last_click = Some((Instant::now(), point, key));
                state.tag_drag = hit
                    .filter(|(track, _)| *track == 0 && !state.shift)
                    .and_then(|(track, index)| {
                        let segment = &self.tracks[track].segments[index];
                        let (id, original_range) = segment.tag_record?;
                        let x = |ts| {
                            (ts - self.view.start) as f32 / self.view.duration() as f32
                                * bounds.width
                        };
                        let edge = if (point.x - x(original_range.start)).abs() < 6.0 {
                            -1
                        } else if (point.x - x(original_range.end)).abs() < 6.0 {
                            1
                        } else {
                            0
                        };
                        Some((id, original_range, edge))
                    });
                state.drag = Some(Drag {
                    x: point.x,
                    old: self.selection,
                    shift: state.shift,
                    moved: false,
                });
                if state.double {
                    if let Some((id, _, _)) = state.tag_drag {
                        return Some(
                            canvas::Action::publish(Message::Edit(super::tagging::Edit::Range(id)))
                                .and_capture(),
                        );
                    }
                    if let Some((track, index)) = hit {
                        return Some(
                            canvas::Action::publish(Message::Select(Some(model::continuous_span(
                                &self.tracks[track].segments[index],
                                &self.tracks[track].segments,
                            ))))
                            .and_capture(),
                        );
                    }
                }
                Some(canvas::Action::capture())
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(drag) = state.drag.as_mut() {
                    let point = point?;
                    if (point.x - drag.x).abs() > 3.0 {
                        drag.moved = true;
                        let range = if let Some((_, range, edge)) = state.tag_drag {
                            let delta = ((point.x - drag.x) / bounds.width
                                * self.view.duration() as f32)
                                as i64;
                            super::tagging::drag_range(range, delta, edge)
                        } else {
                            model::pixel_range(self.view, bounds.width, drag.x, point.x)
                        };
                        return Some(
                            canvas::Action::publish(Message::SelectDragging(Some(Self::extended(
                                range,
                                if drag.shift { drag.old } else { None },
                            ))))
                            .and_capture(),
                        );
                    }
                    return Some(canvas::Action::capture());
                }
                let hit = inside.and_then(|point| self.hit(point, bounds.width));
                if hit != state.hover {
                    state.hover = hit;
                    return Some(canvas::Action::publish(Message::Hover(hit)));
                }
                None
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let drag = state.drag.take()?;
                if !state.double && drag.moved {
                    if let (Some((id, range, edge)), Some(point)) = (state.tag_drag.take(), point) {
                        let delta = ((point.x - drag.x) / bounds.width
                            * self.view.duration() as f32)
                            as i64;
                        return Some(
                            canvas::Action::publish(Message::Edit(
                                super::tagging::Edit::Transform(
                                    id,
                                    super::tagging::drag_range(range, delta, edge),
                                ),
                            ))
                            .and_capture(),
                        );
                    }
                }
                state.tag_drag = None;
                if state.double || drag.moved {
                    return Some(canvas::Action::publish(Message::SelectionReleased).and_capture());
                }
                if let Some(point) = point {
                    if self.visible.contains(&0)
                        && point.y >= self.track_top(0)
                        && point.y < self.track_top(0) + super::design::track_height(0, 0)
                    {
                        let ts =
                            model::pixel_range(self.view, bounds.width, point.x, point.x).start;
                        if let Some(range) = self.gaps.iter().find(|r| r.start <= ts && r.end > ts)
                        {
                            return Some(
                                canvas::Action::publish(Message::TagGap(*range)).and_capture(),
                            );
                        }
                    }
                }
                let value = point
                    .and_then(|p| self.hit(p, bounds.width))
                    .map(|(track, index)| self.tracks[track].segments[index].range);
                Some(
                    canvas::Action::publish(Message::Select(value.map(|value| {
                        Self::extended(value, if drag.shift { drag.old } else { None })
                    })))
                    .and_capture(),
                )
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if inside.is_some() => {
                let (x, y) = match delta {
                    mouse::ScrollDelta::Lines { x, y } | mouse::ScrollDelta::Pixels { x, y } => {
                        (*x, *y)
                    }
                };
                if state.shift || x.abs() > y.abs() {
                    Some(
                        canvas::Action::publish(Message::PanBy(if x + y > 0.0 {
                            -0.2
                        } else {
                            0.2
                        }))
                        .and_capture(),
                    )
                } else {
                    Some(
                        canvas::Action::publish(Message::Zoom(
                            wheel_zoom_factor(*delta),
                            inside.map(|point| {
                                model::pixel_range(self.view, bounds.width, point.x, point.x).start
                            }),
                        ))
                        .and_capture(),
                    )
                }
            }
            _ => None,
        }
    }
    fn draw(
        &self,
        _state: &Interaction,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let started = Instant::now();
        let p = super::design::Palette::new(theme.palette().background.r < 0.5);
        let muted = p.text3;
        let line = p.grid_hour;
        let x = |ts: i64| {
            (ts - self.view.start) as f64 / self.view.duration().max(1) as f64 * bounds.width as f64
        };
        let grid = self.grid_cache.draw(renderer, bounds.size(), |frame| {
            let span = self.view.duration();
            let tick = if span > 12 * 3_600_000 {
                2 * 3_600_000
            } else if span > 4 * 3_600_000 {
                3_600_000
            } else if span > 3_600_000 {
                30 * 60_000
            } else if span > 15 * 60_000 {
                5 * 60_000
            } else {
                60_000
            };
            let mut ts = self.view.start.div_euclid(tick) * tick;
            while ts < self.view.end {
                if ts >= self.view.start {
                    let left = x(ts) as f32;
                    frame.stroke(
                        &Path::line(Point::new(left, RULER), Point::new(left, bounds.height)),
                        Stroke::default().with_color(line),
                    );
                    frame.fill_text(Text {
                        content: model::time_label(ts),
                        position: Point::new(left + 5.0, 8.0),
                        color: muted,
                        font: super::design::MONO,
                        size: 11.0.into(),
                        ..Text::default()
                    });
                }
                ts += tick;
            }
            let half = 30 * 60_000;
            let mut ts = self.view.start.div_euclid(half) * half;
            while ts < self.view.end {
                if ts >= self.view.start && ts.rem_euclid(3_600_000) != 0 {
                    let left = x(ts) as f32;
                    frame.stroke(
                        &Path::line(Point::new(left, RULER), Point::new(left, bounds.height)),
                        Stroke::default().with_color(p.grid_half),
                    );
                }
                ts += half;
            }
            for &track in self.visible {
                let top = self.track_top(track);
                let height = self.heights[track];
                frame.stroke(
                    &Path::line(
                        Point::new(0.0, top + height),
                        Point::new(bounds.width, top + height),
                    ),
                    Stroke::default().with_color(line),
                );
            }
        });
        let base = self.cache.draw(renderer, bounds.size(), |frame| {
            self.stats.base_builds.fetch_add(1, Ordering::Relaxed);
            for track in self.visible {
                let top = self.track_top(*track);
                frame.with_clip(
                    Rectangle::new(
                        Point::new(0.0, top),
                        Size::new(bounds.width, self.heights[*track]),
                    ),
                    |frame| {
                        for (lane, _) in self.lanes[*track].iter().enumerate() {
                            let y = top
                                + super::design::TRACK_HEAD
                                + lane as f32 * super::design::LANE
                                + 5.0;
                            frame.stroke(
                                &Path::line(Point::new(0.0, y), Point::new(bounds.width, y)),
                                Stroke {
                                    line_dash: canvas::LineDash {
                                        segments: &[1.0, 3.0],
                                        offset: 0,
                                    },
                                    ..Stroke::default()
                                        .with_color(p.line.scale_alpha(self.track_progress[*track]))
                                },
                            );
                        }
                        let mut previous = vec![-1; self.lanes[*track].len().max(1)];
                        let mut previous_collapsed = -1;
                        for index in &self.filtered[*track] {
                            let segment = &self.tracks[*track].segments[*index];
                            if *track == 0 && segment.key.as_ref() == "untagged" {
                                continue;
                            }
                            let Some(range) = model::clip(segment.range, self.view) else {
                                continue;
                            };
                            let left = x(range.start) as f32;
                            let width = (x(range.end) - x(range.start)) as f32;
                            let lane = if self.lanes[*track].is_empty() {
                                0
                            } else {
                                self.lane_index(*track, &segment.key).unwrap_or(0)
                            };
                            if width < 1.0 && left.floor() as i32 == previous[lane] {
                                continue;
                            }
                            previous[lane] = left.floor() as i32;
                            let rgb = if *track == 1 && segment.key.as_ref() == "active" {
                                [p.accent.r, p.accent.g, p.accent.b]
                            } else {
                                segment.color
                            };
                            let color = Color {
                                r: rgb[0],
                                g: rgb[1],
                                b: rgb[2],
                                a: if self.lanes[*track].is_empty() {
                                    1.0
                                } else {
                                    self.track_progress[*track]
                                },
                            };
                            if !self.lanes[*track].is_empty()
                                && self.track_progress[*track] < 1.0
                                && (width >= 1.0 || left.floor() as i32 != previous_collapsed)
                            {
                                previous_collapsed = left.floor() as i32;
                                frame.fill_rectangle(
                                    Point::new(left, top + super::design::bar_offset(*track)),
                                    Size::new(
                                        (width - 1.0).max(1.0),
                                        super::design::bar_height(*track),
                                    ),
                                    Color {
                                        a: 1.0 - self.track_progress[*track],
                                        ..color
                                    },
                                );
                            }
                            let Some((bar_top, bar_height)) =
                                self.block_layout(*track, segment.key.as_ref())
                            else {
                                continue;
                            };
                            let bounds = Rectangle::new(
                                Point::new(left, bar_top),
                                Size::new((width - 1.0).max(1.0), bar_height),
                            );
                            if *track == 1 && segment.key.as_ref() == "away" {
                                hatched(frame, bounds, p);
                            } else {
                                frame.fill_rectangle(bounds.position(), bounds.size(), color);
                            }
                            // shortcut: stored ranges have no rule-origin field; add provenance before showing AUTO badges.
                            if *track == 0 && width > 48.0 {
                                frame.fill_text(Text {
                                    content: super::localization::segment_label(segment)
                                        .chars()
                                        .take((width / 7.0) as usize - 1)
                                        .collect(),
                                    position: Point::new(left + 6.0, bar_top + 9.0),
                                    color: super::design::categorical_ink(Color::from_rgb(
                                        rgb[0], rgb[1], rgb[2],
                                    )),
                                    font: super::design::UI,
                                    size: 11.0.into(),
                                    ..Text::default()
                                });
                            }
                        }
                    },
                );
            }
        });
        let mut overlay = Frame::new(renderer, bounds.size());
        if self.highlight.is_some()
            || (!self.highlight_from.is_empty() && self.highlight_progress < 1.0)
            || self.tag_growth.is_some_and(|(_, p)| p < 1.0)
        {
            for &track in self.visible {
                for &index in &self.filtered[track] {
                    let segment = &self.tracks[track].segments[index];
                    if track == 0 && segment.key.as_ref() == "untagged" {
                        continue;
                    }
                    let Some(range) = model::clip(segment.range, self.view) else {
                        continue;
                    };
                    let Some((top, height)) = self.block_layout(track, segment.key.as_ref()) else {
                        continue;
                    };
                    let alpha = |key: Option<&str>| {
                        if key.is_some_and(|key| {
                            key != segment.key.as_ref()
                                && !(track == 3
                                    && segment
                                        .key
                                        .strip_prefix(key)
                                        .is_some_and(|suffix| suffix.starts_with('\0')))
                        }) {
                            if self.lanes[track].is_empty() {
                                0.16
                            } else {
                                0.30
                            }
                        } else {
                            1.0
                        }
                    };
                    let to = alpha(self.highlight);
                    let from: f32 =
                        if self.highlight_progress < 1.0 && !self.highlight_from.is_empty() {
                            self.highlight_from
                                .iter()
                                .map(|(key, weight)| alpha(key.as_deref()) * weight)
                                .sum()
                        } else {
                            to
                        };
                    let opacity = from + (to - from) * self.highlight_progress;
                    let left = x(range.start) as f32;
                    let width = ((x(range.end) - x(range.start)) as f32 - 1.0).max(1.0);
                    if opacity < 1.0 {
                        overlay.fill_rectangle(
                            Point::new(left, top),
                            Size::new(width, height),
                            p.surf.scale_alpha(1.0 - opacity),
                        );
                    }
                    if track == 0 {
                        if let Some((growth, progress)) = self.tag_growth.filter(|(_, p)| *p < 1.0)
                        {
                            let end = growth.start
                                + (growth.duration() as f64 * f64::from(progress)) as i64;
                            let mask = Range::new(
                                range.start.max(end).max(growth.start),
                                range.end.min(growth.end),
                            );
                            if mask.start < mask.end {
                                overlay.fill_rectangle(
                                    Point::new(x(mask.start) as f32, top),
                                    Size::new((x(mask.end) - x(mask.start)) as f32, height),
                                    p.surf,
                                );
                            }
                        }
                    }
                }
            }
        }
        if self.visible.contains(&0) {
            let top = self.track_top(0)
                + (super::design::track_height(0, 0) - super::design::bar_height(0)) / 2.0;
            for range in &self.gaps {
                let Some(range) = model::clip(*range, self.view) else {
                    continue;
                };
                let left = x(range.start) as f32;
                let width = (x(range.end) - x(range.start)) as f32;
                if width < 4.0 {
                    continue;
                }
                let path = Path::rectangle(
                    Point::new(left, top),
                    Size::new((width - 1.0).max(0.0), super::design::bar_height(0)),
                );
                overlay.stroke(
                    &path,
                    Stroke {
                        line_dash: canvas::LineDash {
                            segments: &[3.0, 3.0],
                            offset: 0,
                        },
                        ..Stroke::default().with_color(p.text3)
                    },
                );
                if width > 40.0 {
                    overlay.fill_text(Text {
                        content: super::tr("+ Tag"),
                        position: Point::new(left + 6.0, top + 9.0),
                        color: p.text2,
                        font: super::design::UI,
                        size: 11.0.into(),
                        ..Default::default()
                    });
                }
            }
        }
        if let Some(now) = self
            .now
            .filter(|now| *now >= self.view.start && *now < self.view.end)
        {
            let left = x(now) as f32;
            overlay.fill_rectangle(
                Point::new(left, RULER),
                Size::new(1.0, (bounds.height - RULER).max(0.0)),
                p.danger,
            );
            let badge = (left - 36.0).clamp(0.0, (bounds.width - 40.0).max(0.0));
            overlay.fill_rectangle(Point::new(badge, 3.0), Size::new(40.0, 18.0), p.danger);
            overlay.fill_text(Text {
                content: model::time_label(now),
                position: Point::new(badge + 3.0, 5.0),
                font: super::design::MONO,
                size: 11.0.into(),
                color: p.accent_ink,
                ..Default::default()
            });
        }
        if let Some((track, index)) = _state.hover {
            let segment = &self.tracks[track].segments[index];
            if let Some(range) = model::clip(segment.range, self.view) {
                let top = self.track_top(track);
                let y = if self.lanes[track].is_empty() {
                    top + super::design::bar_offset(track)
                } else {
                    top + super::design::TRACK_HEAD
                        + self.lane_index(track, &segment.key).unwrap_or(0) as f32
                            * super::design::LANE
                };
                overlay.stroke(
                    &Path::rectangle(
                        Point::new(x(range.start) as f32, y),
                        Size::new(
                            ((x(range.end) - x(range.start)) as f32 - 1.0).max(1.0),
                            if self.lanes[track].is_empty() {
                                super::design::bar_height(track)
                            } else {
                                super::design::LANE_BAR
                            },
                        ),
                    ),
                    Stroke::default().with_color(p.text),
                );
            }
        }
        if let Some(range) = self
            .selection
            .and_then(|range| model::clip(range, self.view))
        {
            let left = x(range.start) as f32;
            let width = (x(range.end) - x(range.start)) as f32;
            overlay.fill_rectangle(
                Point::new(left, RULER),
                Size::new(width, bounds.height - RULER),
                Color {
                    a: 0.10,
                    ..p.accent
                },
            );
            // Filled borders keep every pixel inside tiny-skia's damage bounds.
            let height = (bounds.height - RULER).max(0.0);
            let border = p.accent;
            let edge = width.min(1.0);
            let cap = height.min(1.0);
            for (position, size) in [
                (Point::new(left, RULER), Size::new(width, cap)),
                (
                    Point::new(left, RULER + height - cap),
                    Size::new(width, cap),
                ),
                (Point::new(left, RULER), Size::new(edge, height)),
                (
                    Point::new(left + width - edge, RULER),
                    Size::new(edge, height),
                ),
            ] {
                overlay.fill_rectangle(position, size, border);
            }
        }
        self.stats.frames.fetch_add(1, Ordering::Relaxed);
        self.stats.max_build_us.fetch_max(
            started.elapsed().as_micros().min(u64::MAX as u128) as u64,
            Ordering::Relaxed,
        );
        vec![grid, base, overlay.into_geometry()]
    }
    fn mouse_interaction(
        &self,
        state: &Interaction,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.drag.is_some() || cursor.is_over(bounds) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}
pub fn canvas<'a>(program: Timeline<'a>) -> Canvas<Timeline<'a>, Message> {
    let height = RULER
        + program
            .visible
            .iter()
            .map(|&t| program.heights[t])
            .sum::<f32>();
    Canvas::new(program)
        .width(iced::Length::Fill)
        .height(height)
}

pub fn hatched(frame: &mut Frame, bounds: Rectangle, p: super::design::Palette) {
    if bounds.height <= 0.0 {
        return;
    }
    frame.fill_rectangle(bounds.position(), bounds.size(), p.away_dim);
    let mut offset = -bounds.height;
    while offset < bounds.width {
        let a = offset.max(0.0);
        let b = (offset + bounds.height).min(bounds.width);
        if b > a {
            frame.stroke(
                &Path::line(
                    Point::new(bounds.x + a, bounds.y + bounds.height - (a - offset)),
                    Point::new(bounds.x + b, bounds.y + bounds.height - (b - offset)),
                ),
                Stroke::default().with_color(p.away).with_width(2.0),
            );
        }
        offset += 6.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::canvas::Program;

    #[test]
    fn subpixel_events_in_different_lanes_are_not_dropped_together() {
        use iced::advanced::{graphics::geometry::Renderer as _, renderer::Headless as _};
        let mut tracks = model::empty_tracks();
        let mut a = model::Segment::new(Range::new(0, 1), "a", "A");
        a.color = [0.9, 0.2, 0.3];
        let mut b = model::Segment::new(Range::new(1, 2), "b", "B");
        b.color = [0.2, 0.8, 0.4];
        tracks[2].segments = vec![a, b];
        let lanes = std::array::from_fn(|t| {
            if t == 2 {
                model::summarize_iter(tracks[2].segments.iter(), Range::new(0, 1000))
            } else {
                Vec::new()
            }
        });
        let filtered = vec![vec![], vec![], vec![0, 1], vec![]];
        let program = Timeline {
            tracks: &tracks,
            filtered: &filtered,
            visible: &[2],
            view: Range::new(0, 1000),
            selection: None,
            highlight: None,
            highlight_from: &[],
            highlight_progress: 1.0,
            heights: super::super::design::TRACK_HEIGHTS,
            track_progress: [1.0; 4],
            tag_growth: None,
            lanes,
            gaps: vec![],
            now: None,
            grid_cache: &canvas::Cache::new(),
            cache: &canvas::Cache::new(),
            stats: &PaintStats::default(),
        };
        let mut renderer = Renderer::new(super::super::design::UI, 13.0.into());
        for geometry in program.draw(
            &Interaction::default(),
            &renderer,
            &Theme::Dark,
            Rectangle::with_size(Size::new(100.0, 116.0)),
            mouse::Cursor::Unavailable,
        ) {
            renderer.draw_geometry(geometry);
        }
        let pixels = renderer.screenshot(Size::new(100, 116), 1.0, Color::BLACK);
        assert!(
            pixels[(83 * 100) * 4] > 100,
            "first application's subpixel bar must remain visible"
        );
        assert!(
            pixels[(101 * 100) * 4 + 1] > 100,
            "second application's subpixel bar must remain visible in its own lane"
        );
    }

    #[test]
    fn selection_pixels_stay_inside_renderer_damage_bounds() {
        use iced::advanced::{graphics::geometry::Renderer as _, renderer::Headless as _};

        let program = Timeline {
            tracks: &[],
            filtered: &[],
            visible: &[],
            view: Range::new(0, 240),
            selection: Some(Range::new(50, 100)),
            highlight: None,
            highlight_from: &[],
            highlight_progress: 1.0,
            heights: super::super::design::TRACK_HEIGHTS,
            track_progress: [1.0; 4],
            tag_growth: None,
            grid_cache: &canvas::Cache::new(),
            cache: &canvas::Cache::new(),
            stats: &PaintStats::default(),
            lanes: std::array::from_fn(|_| Vec::new()),
            gaps: Vec::new(),
            now: None,
        };
        let mut renderer = Renderer::new(iced::Font::DEFAULT, 16.0.into());
        let overlay = program
            .draw(
                &Interaction::default(),
                &renderer,
                &Theme::Dark,
                Rectangle::with_size(Size::new(240.0, 212.0)),
                mouse::Cursor::Unavailable,
            )
            .pop()
            .unwrap();
        renderer.draw_geometry(overlay);
        let damage: Vec<_> = renderer
            .layers()
            .iter()
            .flat_map(|layer| &layer.primitives)
            .flat_map(|item| item.as_slice())
            .map(|primitive| primitive.visible_bounds())
            .collect();
        let pixels = renderer.screenshot(Size::new(240, 212), 1.0, Color::BLACK);
        for (index, pixel) in pixels.as_chunks::<4>().0.iter().enumerate() {
            if *pixel != [0, 0, 0, 255] {
                let point = Point::new((index % 240) as f32 + 0.5, (index / 240) as f32 + 0.5);
                assert!(
                    damage.iter().any(|bounds| bounds.contains(point)),
                    "selection pixel {point:?} lies outside repaint bounds"
                );
            }
        }
    }

    #[test]
    fn wheel_zoom_scales_with_scroll_distance() {
        let line = wheel_zoom_factor(mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 });
        let pixel = wheel_zoom_factor(mouse::ScrollDelta::Pixels { x: 0.0, y: 1.0 });
        assert!((line - 0.9).abs() < 1e-9);
        assert!(pixel > 0.998 && pixel < 1.0);
        assert_eq!(
            wheel_zoom_factor(mouse::ScrollDelta::Lines { x: 0.0, y: 0.0 }),
            1.0
        );
        let reverse = wheel_zoom_factor(mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 });
        assert!((line * reverse - 1.0).abs() < 1e-9);
    }

    #[test]
    fn native_canvas_drag_and_shift_extension_publish_correct_selection() {
        let tracks = model::fixture(Range::new(0, 86_400_000));
        let cache = canvas::Cache::new();
        let stats = PaintStats::default();
        let visible = [0, 1, 2, 3];
        let program = Timeline {
            tracks: &tracks,
            filtered: &tracks
                .iter()
                .map(|t| (0..t.segments.len()).collect())
                .collect::<Vec<_>>(),
            visible: &visible,
            view: Range::new(0, 86_400_000),
            selection: Some(Range::new(8 * 3_600_000, 9 * 3_600_000)),
            highlight: None,
            highlight_from: &[],
            highlight_progress: 1.0,
            heights: super::super::design::TRACK_HEIGHTS,
            track_progress: [1.0; 4],
            tag_growth: None,
            grid_cache: &canvas::Cache::new(),
            cache: &cache,
            stats: &stats,
            lanes: std::array::from_fn(|_| Vec::new()),
            gaps: Vec::new(),
            now: None,
        };
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(240.0, 212.0));
        let mut state = Interaction::default();
        program.update(
            &mut state,
            &Event::Keyboard(keyboard::Event::ModifiersChanged(
                keyboard::Modifiers::SHIFT,
            )),
            bounds,
            mouse::Cursor::Available(Point::new(100.0, 200.0)),
        );
        program.update(
            &mut state,
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(Point::new(100.0, 200.0)),
        );
        let action = program
            .update(
                &mut state,
                &Event::Mouse(mouse::Event::CursorMoved {
                    position: Point::new(110.0, 200.0),
                }),
                bounds,
                mouse::Cursor::Available(Point::new(110.0, 200.0)),
            )
            .unwrap();
        match action.into_inner().0 {
            Some(Message::SelectDragging(Some(range))) => {
                assert_eq!(range, Range::new(8 * 3_600_000, 11 * 3_600_000))
            }
            message => panic!("unexpected action {message:?}"),
        }
    }
}
