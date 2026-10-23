use super::*;
use iced::widget::{canvas, column, progress_bar};

impl State {
    pub(super) fn active_bounds(&self) -> Option<Range> {
        let segments = &self.all.get(2)?.segments;
        Some(Range::new(
            segments.iter().map(|s| s.range.start).min()?,
            segments.iter().map(|s| s.range.end).max()?,
        ))
    }
    pub(super) fn day_gaps(&self) -> Vec<Range> {
        let active: Vec<_> = self.all[1]
            .segments
            .iter()
            .filter(|s| s.key.as_ref() == "active")
            .map(|s| s.range)
            .collect();
        let tagged: Vec<_> = self.all[0]
            .segments
            .iter()
            .filter(|s| s.key.as_ref() != "untagged")
            .map(|s| s.range)
            .collect();
        model::uncovered(&active, &tagged, self.day)
    }
    pub(super) fn day_lanes(&self) -> [Vec<Summary>; 4] {
        std::array::from_fn(|track| {
            if !self.expanded_tracks[track] {
                return Vec::new();
            }
            let mut rows = model::summarize_iter(self.all[track].segments.iter(), self.day);
            let max = if track == 2 { 6 } else { 8 };
            if rows.len() > max {
                let rest = rows.split_off(max);
                rows.push(Summary {
                    key: "__other".into(),
                    label: tr("Other").into(),
                    duration: rest.iter().map(|r| r.duration).sum(),
                    share: 0.0,
                    color: model::color("__other"),
                    app: None,
                    app_name: None,
                });
            }
            rows
        })
    }
    pub(super) fn day_view(&self, narrow: bool) -> Element<'_, Message> {
        column![
            self.day_header(narrow),
            self.day_timeline(narrow),
            self.hourly_view()
        ]
        .spacing(design::GAP)
        .height(Length::Fill)
        .into()
    }
    fn day_header(&self, narrow: bool) -> Element<'_, Message> {
        let p = design::Palette::new(self.theme.palette().background.r < 0.5);
        let subtitle = self
            .active_bounds()
            .map(|r| {
                localized(
                    "First activity {first} · last {last} · {span} span",
                    &[
                        ("first", model::time_label(r.start)),
                        ("last", model::time_label(r.end)),
                        ("span", model::duration(r.duration())),
                    ],
                )
            })
            .unwrap_or_else(|| tr("No activity in this range yet."));
        let title = column![
            text(tr("Timeline"))
                .font(design::SEMIBOLD)
                .size(design::TITLE),
            text(subtitle).size(design::LABEL).color(p.text3)
        ]
        .spacing(design::SMALL_GAP);
        let mut totals = row![].spacing(design::GAP);
        for (label, value, caption) in [
            (
                "Active",
                self.metrics[0],
                localized(
                    "away {duration}",
                    &[("duration", model::duration(self.metrics[1]))],
                ),
            ),
            ("Tagged", self.metrics[2], String::new()),
            ("Untagged", self.metrics[3], String::new()),
        ] {
            let mut metric = column![
                text(tr(label).to_uppercase()).size(design::CAPTION).color(
                    if label == "Untagged" {
                        p.away_text
                    } else {
                        p.text3
                    }
                ),
                text(model::duration(value))
                    .font(design::MONO)
                    .size(design::SUMMARY_VALUE)
            ]
            .spacing(design::SUMMARY_GAP);
            if label == "Untagged" {
                metric = metric.push(
                    button(
                        text(localized(
                            "Review {count} gaps →",
                            &[("count", self.day_gaps().len().to_string())],
                        ))
                        .size(design::LABEL),
                    )
                    .on_press(Message::ReviewGaps)
                    .height(design::DENSE_TARGET)
                    .style(design::link)
                    .padding(0),
                );
            } else if label == "Tagged" {
                metric = metric.push(
                    progress_bar(
                        0.0..=1.0,
                        self.metrics[2] as f32 / self.metrics[0].max(1) as f32,
                    )
                    .girth(design::PROGRESS)
                    .style(move |_| progress_bar::Style {
                        background: p.line_soft.into(),
                        bar: Color::from_rgb8(
                            design::CATEGORICAL[3][0],
                            design::CATEGORICAL[3][1],
                            design::CATEGORICAL[3][2],
                        )
                        .into(),
                        border: iced::Border::default(),
                    }),
                );
            } else {
                metric = metric.push(text(caption).size(design::LABEL).color(p.text3));
            }
            if label != "Active" {
                totals = totals.push(
                    container(space::vertical())
                        .width(1)
                        .height(design::SUMMARY_DIVIDER)
                        .style(move |_| container::Style {
                            background: Some(p.line.into()),
                            ..Default::default()
                        }),
                );
            }
            totals = totals.push(container(metric).width(Length::Fill));
        }
        if narrow {
            return column![
                title,
                container(totals)
                    .padding(design::CONTROL_GAP as u16)
                    .style(panel)
            ]
            .spacing(design::GAP)
            .into();
        }
        row![
            title.width(Length::Fill),
            container(totals)
                .padding(design::CONTROL_GAP as u16)
                .width(Length::Fill)
                .style(panel)
        ]
        .spacing(design::GAP)
        .align_y(Alignment::Center)
        .into()
    }
    fn day_timeline(&self, narrow: bool) -> Element<'_, Message> {
        let p = design::Palette::new(self.theme.palette().background.r < 0.5);
        let mut date_nav = row![button(text("‹"))
            .on_press(Message::Shift(-1))
            .height(design::SEGMENT_ITEM)
            .width(design::DENSE_TARGET)
            .padding(0)
            .style(design::ghost)]
        .spacing(design::SMALL_GAP)
        .align_y(Alignment::Center);
        if self.date != Local::now().date_naive() {
            date_nav = date_nav
                .push(
                    button(text(tr("Today")).font(design::SEMIBOLD).size(design::BODY))
                        .on_press(Message::Today)
                        .height(design::SEGMENT_ITEM)
                        .padding(0)
                        .style(design::ghost),
                )
                .push(text("·").size(design::BODY));
        }
        date_nav = date_nav
            .push(
                text(localization::short_date_label(self.date))
                    .font(design::SEMIBOLD)
                    .size(design::BODY),
            )
            .push(
                button(text("›"))
                    .on_press(Message::Shift(1))
                    .height(design::SEGMENT_ITEM)
                    .width(design::DENSE_TARGET)
                    .padding(0)
                    .style(design::ghost),
            );
        let mut heading = row![container(date_nav)
            .padding(design::SEGMENT_PADDING)
            .style(design::segment_track)]
        .align_y(Alignment::Center)
        .spacing(design::CONTROL_GAP);
        let mut ranges = row![].spacing(design::SUMMARY_GAP);
        for (i, label) in ["Full day", "Active hours", "Fit selection"]
            .into_iter()
            .enumerate()
        {
            ranges = ranges.push(
                button(text(tr(label)).size(design::LABEL))
                    .on_press_maybe(
                        (i != 2 || self.selection.is_some()).then_some(Message::DayRange(i)),
                    )
                    .height(design::SEGMENT_ITEM)
                    .style(design::segment(self.range_mode == i)),
            );
        }
        let lanes = self.day_lanes();
        let labels = self.visible.iter().fold(
            column![space::vertical().height(timeline::RULER)],
            |labels, &track| {
                let meta = match track {
                    0 => localized(
                        "{duration} tagged",
                        &[("duration", model::duration(self.metrics[2]))],
                    ),
                    1 => localized(
                        "{duration} active",
                        &[("duration", model::duration(self.metrics[0]))],
                    ),
                    2 => localized(
                        "{count} apps · {switches} switches",
                        &[
                            (
                                "count",
                                self.all[2]
                                    .segments
                                    .iter()
                                    .map(|s| &s.key)
                                    .collect::<std::collections::HashSet<_>>()
                                    .len()
                                    .to_string(),
                            ),
                            (
                                "switches",
                                self.all[2].segments.len().saturating_sub(1).to_string(),
                            ),
                        ],
                    ),
                    _ => localized(
                        "{count} windows",
                        &[(
                            "count",
                            self.all[3]
                                .segments
                                .iter()
                                .map(|s| &s.key)
                                .collect::<std::collections::HashSet<_>>()
                                .len()
                                .to_string(),
                        )],
                    ),
                };
                let name = if track == 1 {
                    tr("Computer")
                } else {
                    tr(self.all[track].name)
                };
                let caption = column![
                    text(if track >= 2 {
                        format!(
                            "{} {}",
                            if self.expanded_tracks[track] {
                                "⌄"
                            } else {
                                "›"
                            },
                            name
                        )
                    } else {
                        name
                    })
                    .font(design::SEMIBOLD)
                    .size(design::BODY),
                    text(meta).size(design::LABEL).color(p.text3)
                ]
                .spacing(design::SMALL_GAP);
                let label: Element<'_, Message> = if track >= 2 {
                    button(caption)
                        .height(Length::Shrink)
                        .padding(0)
                        .on_press(Message::ExpandTrack(track))
                        .style(design::ghost)
                        .into()
                } else {
                    caption.into()
                };
                let mut body = column![label].spacing(design::SMALL_GAP);
                if !lanes[track].is_empty() {
                    body = body.push(space::vertical().height(design::SMALL_GAP));
                    for lane in &lanes[track] {
                        let color = Color::from_rgb(lane.color[0], lane.color[1], lane.color[2]);
                        body = body.push(
                            row![
                                container(space::horizontal())
                                    .width(design::LANE_DOT)
                                    .height(design::LANE_DOT)
                                    .style(move |_| container::Style {
                                        background: Some(color.into()),
                                        ..Default::default()
                                    }),
                                text(format!(
                                    "{} · {}",
                                    lane.label,
                                    model::duration(lane.duration)
                                ))
                                .size(design::LABEL)
                            ]
                            .spacing(design::SMALL_GAP)
                            .align_y(Alignment::Center)
                            .height(design::LANE),
                        );
                    }
                }
                labels.push(
                    container(body)
                        .padding(iced::Padding {
                            top: if lanes[track].is_empty() {
                                design::TRACK_LABEL_TOP[track]
                            } else {
                                0.0
                            },
                            ..iced::Padding::ZERO
                        })
                        .height(design::track_height(track, lanes[track].len())),
                )
            },
        );
        let plot = timeline::canvas(timeline::Timeline {
            tracks: &self.all,
            filtered: &self.filtered,
            visible: &self.visible,
            view: self.view,
            selection: self.selection,
            highlight: self.highlight.as_deref(),
            grid_cache: &self.grid_cache,
            cache: &self.cache,
            stats: &self.stats,
            lanes: lanes.clone(),
            gaps: self.day_gaps(),
            now: Some(draugr_time_daemon::now_ms()),
        });
        let chart: Element<'_, Message> = if let Some(s) = self
            .hover
            .and_then(|(t, i)| self.all.get(t)?.segments.get(i))
        {
            let title = text(s.detail.to_string()).size(design::META);
            let title = if s.detail.as_ref() == draugr_time_daemon::preferences::MASKED_TITLE {
                title.font(iced::Font {
                    style: iced::font::Style::Italic,
                    ..design::UI
                })
            } else {
                title
            };
            tooltip(
                plot,
                container(
                    column![
                        text(localization::segment_label(s))
                            .font(
                                if s.label.as_ref() == draugr_time_daemon::preferences::MASKED_TITLE
                                {
                                    iced::Font {
                                        style: iced::font::Style::Italic,
                                        ..design::UI
                                    }
                                } else {
                                    design::UI
                                }
                            )
                            .size(design::BODY),
                        title,
                        text(format!(
                            "{}–{} · {}",
                            model::time_label(s.range.start),
                            model::time_label(s.range.end),
                            model::duration(s.range.duration())
                        ))
                        .font(design::MONO)
                        .size(design::LABEL)
                    ]
                    .spacing(design::SMALL_GAP),
                )
                .padding(design::RAIL_PADDING),
                tooltip::Position::FollowCursor,
            )
            .delay(Duration::from_millis(design::TOOLTIP_DELAY))
            .style(design::popover)
            .into()
        } else {
            plot.into()
        };
        if let Some(key) = &self.highlight {
            let label = self
                .rows
                .iter()
                .find(|r| r.key.as_ref() == key)
                .map(|r| r.label.as_ref())
                .unwrap_or(key);
            heading = heading.push(
                button(
                    text(localized(
                        "Highlighting {name} ×",
                        &[("name", label.to_string())],
                    ))
                    .size(design::LABEL),
                )
                .on_press(Message::Highlight(key.clone()))
                .style(design::ghost),
            );
        }
        heading = heading.push(space::horizontal());
        heading = heading.push(
            container(ranges)
                .padding(design::SEGMENT_PADDING)
                .style(design::segment_track),
        );
        let heading: Element<'_, Message> = if narrow {
            heading.wrap().into()
        } else {
            heading.into()
        };
        let mut layers = widget::stack![scrollable(row![
            container(labels).width(design::TRACK_LABEL),
            chart
        ])
        .style(design::scrollbars)
        .height(Length::Fill)]
        .width(Length::Fill)
        .height(Length::Fill);
        if self.block_focus {
            if let Some(s) = self
                .hover
                .and_then(|(t, i)| self.all.get(t)?.segments.get(i))
            {
                layers = layers.push(
                    container(
                        container(column![
                            text(localization::segment_label(s))
                                .font(
                                    if s.label.as_ref()
                                        == draugr_time_daemon::preferences::MASKED_TITLE
                                    {
                                        iced::Font {
                                            style: iced::font::Style::Italic,
                                            ..design::UI
                                        }
                                    } else {
                                        design::UI
                                    }
                                )
                                .size(design::BODY),
                            text(s.detail.to_string()).size(design::META),
                            text(format!(
                                "{}–{} · {}",
                                model::time_label(s.range.start),
                                model::time_label(s.range.end),
                                model::duration(s.range.duration())
                            ))
                            .font(design::MONO)
                            .size(design::LABEL)
                        ])
                        .padding(design::RAIL_PADDING)
                        .style(design::popover),
                    )
                    .align_right(Length::Fill)
                    .align_top(Length::Fill),
                );
            }
        }
        if let Some(range) = self.selection {
            layers = layers.push(
                container(
                    container(
                        row![
                            text(format!(
                                "{}–{} · {}",
                                model::time_label(range.start),
                                model::time_label(range.end),
                                model::duration(range.duration())
                            ))
                            .font(design::MONO)
                            .size(design::LABEL),
                            iced::widget::pick_list(
                                self.tags
                                    .iter()
                                    .filter(|t| t.archived != 1)
                                    .map(|t| model::tag_path(&self.tags, t.id))
                                    .collect::<Vec<_>>(),
                                (!self.selection_tag.is_empty()).then_some(&self.selection_tag),
                                Message::SelectionTag
                            )
                            .placeholder(tr("Tag as"))
                            .style(design::pick_list)
                            .text_size(design::META)
                            .width(design::TAG_PICKER),
                            button(text(tr("Tag")))
                                .on_press(Message::ApplySelectionTag)
                                .style(design::primary),
                            button(text(tr("Exclude")))
                                .on_press(Message::ExcludeSelection)
                                .style(design::ghost),
                            button(text("×"))
                                .on_press(Message::Select(None))
                                .style(design::ghost)
                        ]
                        .spacing(design::CONTROL_GAP)
                        .align_y(Alignment::Center)
                        .wrap(),
                    )
                    .padding(design::CONTROL_GAP as u16)
                    .style(design::popover),
                )
                .align_right(Length::Fill)
                .align_bottom(Length::Fill),
            );
        }
        if let Some((range, _)) = self.tag_toast {
            layers = layers.push(
                container(
                    button(text(localized(
                        "Tagged {duration} · Undo",
                        &[("duration", model::duration(range.duration()))],
                    )))
                    .on_press(Message::Edit(tagging::Edit::Undo))
                    .style(design::ghost),
                )
                .align_bottom(Length::Fill),
            );
        }
        let mut body = column![heading, layers].spacing(design::CONTROL_GAP);
        body = body.push(text(tr("Active · Away · Untagged · Now     Drag to select · Shift-drag to extend · Enter to tag")).size(design::LABEL).color(p.text3));
        container(body)
            .padding(design::CARD_PADDING)
            .height(Length::Fill)
            .style(panel)
            .into()
    }
    pub(super) fn day_summary(&self) -> Element<'_, Message> {
        let p = design::Palette::new(self.theme.palette().background.r < 0.5);
        let tabs = [(1, "Apps"), (2, "Docs"), (0, "Tags")].into_iter().fold(
            row![].spacing(design::SMALL_GAP),
            |tabs, (index, label)| {
                tabs.push(
                    button(text(tr(label)).size(design::META))
                        .width(Length::Fill)
                        .on_press(Message::Tab(index))
                        .height(design::SEGMENT_ITEM)
                        .style(design::segment(self.tab == index)),
                )
            },
        );
        let mut rows = column![].spacing(design::GAP);
        for entry in self.rows.iter().take(self.limit) {
            let highlighted = self.expanded_summary.as_deref() == Some(entry.key.as_ref());
            let share_percent = entry.share as f32;
            let label = if self.tab == 0 && entry.key.as_ref() == "untagged" {
                tr("Untagged")
            } else {
                entry.label.to_string()
            };
            let color = Color::from_rgb(entry.color[0], entry.color[1], entry.color[2]);
            let share = progress_bar(0.0..=100.0, share_percent)
                .girth(design::PROGRESS)
                .style(move |_| progress_bar::Style {
                    background: p.line_soft.into(),
                    bar: color.into(),
                    border: iced::Border::default(),
                });
            rows = rows.push(
                row![
                    button(text(if highlighted { "⌄" } else { "›" }))
                        .on_press(Message::ExpandSummary(entry.key.to_string()))
                        .style(design::ghost)
                        .padding(0),
                    self.app_icon(
                        entry.app.as_deref().unwrap_or(&entry.key),
                        entry.app_name.as_deref().unwrap_or(&entry.label)
                    ),
                    column![
                        button(
                            text(label)
                                .font(
                                    if entry.label.as_ref()
                                        == draugr_time_daemon::preferences::MASKED_TITLE
                                    {
                                        iced::Font {
                                            style: iced::font::Style::Italic,
                                            ..design::UI
                                        }
                                    } else {
                                        design::UI
                                    }
                                )
                                .size(design::BODY)
                        )
                        .on_press(Message::Highlight(entry.key.to_string()))
                        .style(design::ghost)
                        .padding(0),
                        share
                    ]
                    .spacing(design::SMALL_GAP)
                    .width(Length::Fill),
                    column![
                        text(model::duration(entry.duration))
                            .font(design::MONO)
                            .size(design::META),
                        text(format!("{:.0}%", share_percent))
                            .size(design::LABEL)
                            .color(p.text3)
                    ]
                    .align_x(Alignment::End),
                    button(text("⌖"))
                        .on_press(Message::Highlight(entry.key.to_string()))
                        .style(design::ghost)
                ]
                .spacing(design::CONTROL_GAP)
                .align_y(Alignment::Center),
            );
            if highlighted && self.tab == 1 {
                for doc in &self.documents {
                    rows = rows.push(
                        row![
                            text(format!("└ {}", doc.label))
                                .size(design::META)
                                .color(p.text2)
                                .width(Length::Fill),
                            text(model::duration(doc.duration))
                                .font(design::MONO)
                                .size(design::LABEL)
                                .color(p.text3)
                        ]
                        .spacing(design::CONTROL_GAP),
                    );
                }
            }
        }
        if self.rows.is_empty() {
            rows = rows.push(text(tr("No matching activity.")).size(design::BODY));
        }
        if self.rows.len() > self.limit {
            rows = rows.push(
                button(text(tr("Show 50 more")))
                    .on_press(Message::More)
                    .style(design::ghost),
            );
        }
        column![
            row![
                text(tr("Whole day"))
                    .font(design::SEMIBOLD)
                    .size(design::HEADING),
                text(model::duration(self.metrics[0]))
                    .size(design::LABEL)
                    .color(p.text3)
            ]
            .spacing(design::CONTROL_GAP)
            .align_y(Alignment::Center),
            container(tabs)
                .padding(design::SEGMENT_PADDING)
                .style(design::segment_track),
            text_input(&tr("Filter apps and window titles"), &self.summary_search)
                .on_input(Message::SummaryFilter)
                .id("day-filter")
                .size(design::META)
                .padding(design::RAIL_PADDING),
            row![
                text(tr(["Tags", "Application", "Documents"][self.tab]).to_uppercase())
                    .size(design::CAPTION)
                    .color(p.text3)
                    .width(Length::Fill),
                text(tr("Time").to_uppercase())
                    .size(design::CAPTION)
                    .color(p.text3)
            ]
            .padding(iced::Padding {
                left: design::DENSE_TARGET,
                ..iced::Padding::ZERO
            }),
            scrollable(rows)
                .style(design::scrollbars)
                .height(Length::Fill),
            container(
                text(localized(
                    "Stored on this computer only · titles masked for {count} apps",
                    &[(
                        "count",
                        self.settings_ui.preferences.masked_apps.len().to_string()
                    )]
                ))
                .size(design::LABEL)
                .color(p.text3)
            )
            .padding(design::RAIL_PADDING)
            .style(panel)
        ]
        .spacing(design::PANEL_GAP)
        .height(Length::Fill)
        .into()
    }
    fn hourly_view(&self) -> Element<'_, Message> {
        container(
            column![
                row![
                    text(tr("Activity by hour")).size(design::BODY),
                    text(self.hourly_insight()).size(design::LABEL)
                ]
                .spacing(design::CONTROL_GAP),
                canvas::Canvas::new(Hourly {
                    bins: &self.hourly,
                    cache: &self.hourly_cache
                })
                .width(Length::Fill)
                .height(Length::Fill)
            ]
            .spacing(design::SMALL_GAP),
        )
        .padding(design::CARD_PADDING)
        .height(design::HOUR_CARD)
        .style(panel)
        .into()
    }
}
struct Hourly<'a> {
    bins: &'a [[i64; 3]; 24],
    cache: &'a canvas::Cache,
}
impl canvas::Program<Message> for Hourly<'_> {
    type State = ();
    fn draw(
        &self,
        _: &(),
        renderer: &iced::Renderer,
        theme: &Theme,
        bounds: iced::Rectangle,
        _: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let p = design::Palette::new(theme.palette().background.r < 0.5);
        let geometry = self.cache.draw(renderer, bounds.size(), |f| {
            let width = bounds.width / 24.0;
            for hour in 0..24 {
                let bin = self.bins[hour];
                let capacity = bin[2].max(1) as f32;
                let active = bin[0] as f32 / capacity;
                let away = bin[1] as f32 / capacity;
                let x = hour as f32 * width;
                let height = (bounds.height - 16.0).max(0.0);
                f.fill_rectangle(
                    iced::Point::new(x, 0.0),
                    iced::Size::new((width - 4.0).max(1.0), height),
                    p.side,
                );
                f.fill_rectangle(
                    iced::Point::new(x, height * (1.0 - active)),
                    iced::Size::new((width - 4.0).max(1.0), height * active),
                    p.accent,
                );
                timeline::hatched(
                    f,
                    iced::Rectangle::new(
                        iced::Point::new(x, height * (1.0 - active - away)),
                        iced::Size::new((width - 4.0).max(1.0), height * away),
                    ),
                    p,
                );
                f.fill_text(canvas::Text {
                    content: format!("{hour:02}"),
                    position: iced::Point::new(x, height + 4.0),
                    color: if hour % 3 == 0 { p.text2 } else { p.text3 },
                    font: design::MONO,
                    size: 10.0.into(),
                    ..Default::default()
                });
            }
        });
        vec![geometry]
    }
}

impl State {
    pub(super) fn active_hours(&self) -> Option<Range> {
        let r = self.active_bounds()?;
        use chrono::Timelike;
        let local_hour = |ts| {
            chrono::DateTime::from_timestamp_millis(ts)
                .map(|t| i64::from(t.with_timezone(&Local).minute()) * 60_000)
                .unwrap_or(0)
        };
        let start = r.start - local_hour(r.start) - r.start.rem_euclid(60_000);
        let end = r.end - local_hour(r.end) - r.end.rem_euclid(60_000);
        Some(Range::new(
            start.max(self.day.start),
            (end + i64::from(end < r.end) * 3_600_000).min(self.day.end),
        ))
    }
    pub(super) fn rec_popover(&self) -> Element<'_, Message> {
        let mut menu = column![
            text(self.status_label()).size(design::BODY),
            text(tr("Don't track for")).size(design::HEADING)
        ]
        .spacing(design::CONTROL_GAP);
        if self.runtime.is_none() && !self.fixture {
            menu = menu.push(button(text(tr("Start recording"))).on_press(Message::Start));
        }
        if self.status.state == "paused" {
            menu = menu.push(button(text(tr("Resume now"))).on_press(Message::Resume));
            let remaining = self
                .status
                .paused_until
                .unwrap_or(0)
                .saturating_sub(draugr_time_daemon::now_ms())
                .max(0);
            for minutes in [5, 15] {
                menu = menu.push(
                    button(text(format!(
                        "+{}",
                        localized("{count} min", &[("count", minutes.to_string())])
                    )))
                    .on_press(Message::Pause(Some(remaining + minutes * 60_000))),
                );
            }
        }
        for minutes in [5, 10, 15, 30, 60] {
            menu = menu.push(
                button(text(localized(
                    "{count} min",
                    &[("count", minutes.to_string())],
                )))
                .on_press(Message::Pause(Some(minutes * 60_000)))
                .style(design::ghost),
            );
        }
        menu = menu
            .push(text(tr("Turns back on by itself")).size(design::LABEL))
            .push(
                button(text(tr("Close")))
                    .on_press(Message::RecMenu)
                    .style(design::ghost),
            );
        container(menu)
            .padding(design::CARD_PADDING)
            .width(design::POPOVER_WIDTH)
            .style(design::popover)
            .into()
    }
}

pub(super) fn hourly_totals(tracks: &[Track], day: Range) -> [[i64; 3]; 24] {
    use chrono::Timelike;
    let mut bins = [[0; 3]; 24];
    let mut start = day.start;
    while start < day.end {
        let Some(local) =
            chrono::DateTime::from_timestamp_millis(start).map(|t| t.with_timezone(&Local))
        else {
            break;
        };
        let remaining = 3_600_000
            - i64::from(local.minute()) * 60_000
            - i64::from(local.second()) * 1000
            - i64::from(local.timestamp_subsec_millis());
        let end = (start + remaining).min(day.end);
        let bin = &mut bins[local.hour() as usize];
        bin[2] += end - start;
        for (index, key) in ["active", "away"].into_iter().enumerate() {
            bin[index] += model::union_duration(
                &tracks[1]
                    .segments
                    .iter()
                    .filter(|s| s.key.as_ref() == key)
                    .map(|s| s.range)
                    .collect::<Vec<_>>(),
                Range::new(start, end),
            );
        }
        start = end;
    }
    bins
}

impl State {
    fn hourly_insight(&self) -> String {
        let hour = self
            .hourly
            .windows(2)
            .enumerate()
            .max_by_key(|(_, b)| b[0][0] + b[1][0])
            .map(|(h, _)| h)
            .unwrap_or(0);
        let mut best: Option<(&str, Range)> = None;
        let mut current: Option<(&str, &str, Range)> = None;
        for s in &self.all[2].segments {
            let Some(range) = model::clip(s.range, self.day) else {
                continue;
            };
            let span = match current {
                Some((key, _, previous))
                    if key == s.key.as_ref() && previous.end == range.start =>
                {
                    Range::new(previous.start, range.end)
                }
                _ => range,
            };
            current = Some((&s.key, &s.label, span));
            if best.is_none_or(|(_, r)| span.duration() > r.duration()) {
                best = Some((&s.label, span));
            }
        }
        best.map(|(app, r)| {
            localized(
                "00–24 · most focused {hours} · longest stretch {duration} in {app}",
                &[
                    ("hours", format!("{hour:02}–{:02}", hour + 2)),
                    ("duration", model::duration(r.duration())),
                    ("app", app.to_string()),
                ],
            )
        })
        .unwrap_or_else(|| tr("No activity in this range yet."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::advanced::{
        layout, mouse, renderer, renderer::Headless as _, widget::Tree, Layout, Renderer as _,
    };

    #[test]
    fn gaps_ignore_the_synthetic_untagged_track_blocks() {
        let (mut state, _) = State::boot();
        state.all = model::fixture(state.day);
        state.rebuild();
        assert!(!state.day_gaps().is_empty());
        let covers: Vec<_> = state.all[0]
            .segments
            .iter()
            .filter(|s| s.key.as_ref() != "untagged")
            .map(|s| s.range)
            .collect();
        for gap in state.day_gaps() {
            assert_eq!(model::union_duration(&covers, gap), 0);
        }
    }

    #[test]
    #[ignore = "writes review screenshots; run explicitly with --test-threads=1"]
    fn render_day_review_screenshots() {
        use std::borrow::Cow;
        {
            let mut fonts = iced::advanced::graphics::text::font_system()
                .write()
                .unwrap();
            fonts.load_font(Cow::Borrowed(include_bytes!("../../fonts/InterTight.ttf")));
            fonts.load_font(Cow::Borrowed(include_bytes!(
                "../../fonts/JetBrainsMono-Regular.ttf"
            )));
        }
        localization::set(localization::Language::En);
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/screenshots");
        std::fs::create_dir_all(&path).unwrap();
        for dark in [true, false] {
            let mut baseline_plot_height = None;
            for (width, height, scenario) in [
                (1280, 820, ""),
                (700, 820, ""),
                (700, 1600, "-full"),
                (1280, 820, "-selected"),
                (1280, 820, "-expanded"),
            ] {
                let (mut state, _) = State::boot();
                state.fixture = true;
                state.settings_ui.preferences.onboarded = true;
                state.theme = native_theme(dark);
                state.date = NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();
                state.day = model::day_bounds(state.date);
                state.all = model::fixture(state.day);
                state.status.state = "tracking".into();
                state.rebuild();
                if scenario == "-selected" {
                    state.selection = Some(Range::new(
                        state.view.start + 100 * 60_000,
                        state.view.start + 190 * 60_000,
                    ));
                    let key = state.rows[0].key.to_string();
                    state.highlight = Some(key.clone());
                    state.expanded_summary = Some(key);
                    state.summary();
                }
                if scenario == "-expanded" {
                    state.expanded_tracks[2] = true;
                    state.expanded_tracks[3] = true;
                }
                let size = iced::Size::new(width as f32, height as f32);
                let viewport = iced::Rectangle::with_size(size);
                let mut renderer = iced::Renderer::new(design::UI, 13.0.into());
                let mut view = state.view_sized(size.width);
                let mut tree = Tree::new(view.as_widget());
                let node = view.as_widget_mut().layout(
                    &mut tree,
                    &renderer,
                    &layout::Limits::new(iced::Size::ZERO, size),
                );
                renderer.reset(viewport);
                view.as_widget().draw(
                    &tree,
                    &mut renderer,
                    &state.theme,
                    &renderer::Style {
                        text_color: state.theme.palette().text,
                    },
                    Layout::new(&node),
                    mouse::Cursor::Unavailable,
                    &viewport,
                );
                if width == 1280 && (scenario.is_empty() || scenario == "-selected") {
                    let plot = renderer
                        .layers()
                        .iter()
                        .filter(|l| {
                            !l.primitives.is_empty()
                                && l.bounds.x > 100.0
                                && l.bounds.height > 200.0
                        })
                        .max_by(|a, b| a.bounds.height.total_cmp(&b.bounds.height))
                        .expect("timeline has a clipped canvas layer");
                    if scenario.is_empty() {
                        baseline_plot_height = Some(plot.bounds.height);
                    } else {
                        assert_eq!(
                            Some(plot.bounds.height),
                            baseline_plot_height,
                            "selection/highlight overlays must not hide document blocks"
                        );
                    }
                }
                let rgba = renderer.screenshot(
                    iced::Size::new(width, height),
                    1.0,
                    state.theme.palette().background,
                );
                let file = std::fs::File::create(path.join(format!(
                    "m12-day-{}-{width}{scenario}.png",
                    if dark { "dark" } else { "light" }
                )))
                .unwrap();
                let mut encoder = png::Encoder::new(file, width, height);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                encoder
                    .write_header()
                    .unwrap()
                    .write_image_data(&rgba)
                    .unwrap();
                eprintln!(
                    "{} {width}x{height}{scenario}: max canvas build {:.2} ms",
                    if dark { "dark" } else { "light" },
                    state.stats.max_build_us.load(Ordering::Relaxed) as f64 / 1000.0
                );
                assert!(
                    state.stats.max_build_us.load(Ordering::Relaxed) < 100_000,
                    "5000 events must render within 100ms in debug"
                );
            }
        }
    }
}

#[cfg(test)]
mod hourly_tests {
    use super::*;
    #[test]
    fn hourly_bins_preserve_clipped_totals_on_short_and_long_days() {
        for hours in [23, 24, 25] {
            let day = Range::new(1_792_627_200_000, 1_792_627_200_000 + hours * 3_600_000);
            let mut tracks = model::empty_tracks();
            tracks[1].segments = vec![model::Segment::new(
                Range::new(day.start - 60_000, day.end + 60_000),
                "active",
                "Active",
            )];
            let bins = hourly_totals(&tracks, day);
            assert_eq!(bins.iter().map(|b| b[0]).sum::<i64>(), day.duration());
            assert_eq!(bins.iter().map(|b| b[2]).sum::<i64>(), day.duration());
            assert_eq!(bins.iter().map(|b| b[1]).sum::<i64>(), 0);
        }
    }
}

#[cfg(test)]
mod interaction_tests {
    use super::*;
    #[test]
    fn panel_filter_and_expansion_do_not_change_timeline_or_highlight() {
        let (mut state, _) = State::boot();
        state.all = model::fixture(state.day);
        state.rebuild();
        let old = state.filtered.clone();
        let key = state.rows[0].key.to_string();
        let _ = state.update(Message::ExpandSummary(key.clone()));
        assert_eq!(state.expanded_summary.as_deref(), Some(key.as_str()));
        assert!(state.highlight.is_none());
        assert!(!state.documents.is_empty());
        let _ = state.update(Message::SummaryFilter("does not exist".into()));
        assert!(state.rows.is_empty());
        assert_eq!(state.filtered, old);
        let _ = state.update(Message::SummaryFilter(String::new()));
        assert!((state.rows.iter().map(|r| r.share).sum::<f64>() - 100.0).abs() < 0.001);
    }
    #[test]
    fn reviewing_gaps_advances_and_wraps_without_tagging_or_deleting() {
        let (mut state, _) = State::boot();
        state.all = model::fixture(state.day);
        state.rebuild();
        let gaps = state.day_gaps();
        assert!(gaps.len() > 1);
        for gap in gaps.iter().chain(gaps.iter().take(1)) {
            let _ = state.update(Message::ReviewGaps);
            assert_eq!(state.selection, Some(*gap));
            assert!(state.editor.is_none());
        }
    }
    #[test]
    fn default_active_hours_and_keyboard_block_details_preserve_manual_zoom() {
        let (mut state, _) = State::boot();
        state.all = model::fixture(state.day);
        state.rebuild();
        assert_ne!(state.view, state.day);
        let _ = state.update(Message::Zoom(0.5, None));
        let zoom = state.view;
        state.rebuild();
        assert_eq!(state.view, zoom);
        let _ = state.update(Message::Key(
            keyboard::Key::Named(keyboard::key::Named::ArrowDown),
            keyboard::Modifiers::ALT,
        ));
        assert!(state.block_focus);
        assert!(state.hover.is_some());
        assert!(state.selection.is_some());
    }
}

#[cfg(test)]
mod whole_day_tests {
    use super::*;
    #[test]
    fn whole_day_panel_keeps_daily_totals_when_a_tagging_range_is_selected() {
        let (mut state, _) = State::boot();
        state.all = model::fixture(state.day);
        state.rebuild();
        let totals: Vec<_> = state
            .rows
            .iter()
            .map(|r| (r.key.clone(), r.duration))
            .collect();
        let _ = state.update(Message::Select(Some(Range::new(
            state.view.start,
            state.view.start + 60_000,
        ))));
        assert_eq!(
            state
                .rows
                .iter()
                .map(|r| (r.key.clone(), r.duration))
                .collect::<Vec<_>>(),
            totals
        );
    }
}
