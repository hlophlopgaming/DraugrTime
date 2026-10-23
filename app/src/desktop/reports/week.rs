use super::*;
use iced::widget::{column, progress_bar};

type Pixel = Option<(Option<i64>, [f32; 3])>;
pub(super) fn rasterize(day: &Day, report: &Report, width: f32, now: i64) -> [Vec<Pixel>; 2] {
    let mut pixels: [Vec<Pixel>; 2] = std::array::from_fn(|_| vec![None; width.ceil() as usize]);
    for i in day_intervals(report, day.range) {
        let color = model::color(&i.application);
        for (start, end) in clock_spans(day, Range::new(i.start_ts, i.end_ts.min(now))) {
            let x = |minute: f32| {
                ((minute / 60.0 - design::WEEK_FIRST_HOUR)
                    / (design::WEEK_LAST_HOUR - design::WEEK_FIRST_HOUR)
                    * width)
                    .clamp(0.0, width)
            };
            let start = x(start).floor() as usize;
            let end = (x(end).ceil() as usize).min(pixels[0].len());
            if end > start {
                pixels[0][start..end].fill(Some((i.tag_id, [0.0; 3])));
                pixels[1][start..end].fill(Some((i.tag_id, color)));
            }
        }
    }
    pixels
}
pub(super) fn previous_week(start: NaiveDate) -> Option<Range> {
    Some(Range::new(
        model::day_bounds(start.checked_sub_days(Days::new(7))?).start,
        model::day_bounds(start).start,
    ))
}
fn sessions(report: &Report, range: Range) -> usize {
    let mut end = range.start - 1;
    let mut count = 0;
    for i in day_intervals(report, range) {
        if i.start_ts.max(range.start) > end {
            count += 1;
        }
        end = end.max(i.end_ts.min(range.end));
    }
    count
}
fn gaps(report: &Report) -> Vec<Range> {
    let mut result: Vec<Range> = Vec::new();
    for i in report.intervals.iter().filter(|i| i.tag_id.is_none()) {
        if let Some(last) = result.last_mut().filter(|r| r.end == i.start_ts) {
            last.end = i.end_ts;
        } else {
            result.push(Range::new(i.start_ts, i.end_ts));
        }
    }
    result
}
fn tag_rows(report: &Report) -> Vec<draugr_time_core::ReportRow> {
    let mut rows = report.rows(GroupBy::Tag, None);
    rows.sort_by_key(|r| (r.tag_id.is_none(), std::cmp::Reverse(r.duration_ms)));
    rows
}
fn recorded_days(days: &[Day]) -> usize {
    days.iter()
        .filter(|d| d.active_ms > 0 || d.bands[0].iter().any(|b| b.end > b.start))
        .count()
}
fn weekday(date: NaiveDate) -> String {
    tr([
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ][date.weekday().num_days_from_monday() as usize])
    .chars()
    .take(3)
    .collect()
}
fn hours(ms: i64) -> String {
    if ms == 0 {
        "–".into()
    } else {
        format!("{:.1}", ms as f64 / 3_600_000.0)
    }
}
pub(super) fn timesheet_csv(report: &Report, days: &[Day]) -> String {
    fn csv(s: &str) -> String {
        if s.contains([',', '"', '\n', '\r']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.into()
        }
    }
    let mut result = format!("{},", csv(&tr("Tags")));
    for day in days {
        result.push_str(&format!("{},", day.date));
    }
    result.push_str(&format!("{}\n", csv(&tr("Total"))));
    for r in report.rows(GroupBy::Tag, None) {
        result.push_str(&csv(&translate_label(&r.label)));
        for day in days {
            result.push_str(&format!(
                ",{:.4}",
                day.tag_totals.get(&r.tag_id).copied().unwrap_or(0) as f64 / 3_600_000.0
            ));
        }
        result.push_str(&format!(",{:.4}\n", r.duration_ms as f64 / 3_600_000.0));
    }
    result.push_str(&csv(&tr("Total")));
    for day in days {
        result.push_str(&format!(",{:.4}", day.active_ms as f64 / 3_600_000.0));
    }
    result.push_str(&format!(",{:.4}\n", report.active_ms as f64 / 3_600_000.0));
    result
}
fn line(p: design::Palette) -> Element<'static, Message> {
    container(space::horizontal())
        .height(design::HAIRLINE)
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(p.line_soft.into()),
            ..Default::default()
        })
        .into()
}
fn bar(value: i64, maximum: i64, color: Color, p: design::Palette) -> Element<'static, Message> {
    progress_bar(0.0..=1.0, value as f32 / maximum.max(1) as f32)
        .girth(design::PROGRESS)
        .style(move |_| progress_bar::Style {
            background: p.line_soft.into(),
            bar: color.into(),
            border: iced::Border::default(),
        })
        .into()
}
fn swatch(color: Color, untagged: bool, p: design::Palette) -> Element<'static, Message> {
    let color = if untagged { p.text3 } else { color };
    let rgb = format!(
        "rgb({},{},{})",
        (color.r * 255.0) as u8,
        (color.g * 255.0) as u8,
        (color.b * 255.0) as u8
    );
    let asset = if untagged {
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="{rgb}" stroke-dasharray="2 2"/></svg>"#
        )
    } else {
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="{rgb}"/></svg>"#
        )
    };
    container(
        svg(svg::Handle::from_memory(asset.into_bytes()))
            .width(design::WEEK_SWATCH_SIZE)
            .height(design::WEEK_SWATCH_SIZE),
    )
    .width(design::WEEK_SWATCH)
    .into()
}
impl State {
    pub(in crate::desktop) fn week_view(&self, narrow: bool) -> Element<'_, Message> {
        let ui = &self.report_ui;
        let p = design::Palette::new(self.theme.palette().background.r < 0.5);
        let Some(data) = &ui.data else {
            return column![
                text(tr("Week")).size(design::TITLE),
                text(if ui.loading {
                    tr("Loading…")
                } else {
                    localization::error(&ui.error)
                }),
                button(text(tr("Refresh"))).on_press(Message::Report(Edit::Refresh))
            ]
            .spacing(design::GAP)
            .into();
        };
        let total = data.report.active_ms;
        let untagged = data
            .report
            .rows(GroupBy::Tag, None)
            .iter()
            .filter(|r| r.tag_id.is_none())
            .map(|r| r.duration_ms)
            .sum::<i64>();
        let count = recorded_days(&data.days);
        let subtitle = localized(
            "Week {week} · {first} – {last} · {count} days recorded",
            &[
                ("week", data.days[0].date.iso_week().week().to_string()),
                ("first", localization::short_date_label(data.days[0].date)),
                (
                    "last",
                    localization::short_date_label(data.days.last().unwrap().date),
                ),
                ("count", count.to_string()),
            ],
        );
        let title = column![
            text(tr("Week")).size(design::TITLE).font(design::SEMIBOLD),
            text(subtitle).size(design::LABEL).color(p.text3)
        ]
        .spacing(design::SMALL_GAP);
        let mut metrics = row![].spacing(design::CONTROL_GAP);
        for (index, label, value) in [
            (0, "Active", total),
            (1, "Tagged", total - untagged),
            (2, "Untagged", untagged),
        ] {
            if index > 0 {
                metrics = metrics.push(
                    container(space::vertical())
                        .height(design::SUMMARY_DIVIDER)
                        .width(design::HAIRLINE)
                        .style(move |_| container::Style {
                            background: Some(p.line.into()),
                            ..Default::default()
                        }),
                );
            }
            let mut cell = column![
                text(tr(label).to_uppercase())
                    .size(design::CAPTION)
                    .color(if index == 2 { p.away_text } else { p.text3 }),
                text(model::duration(value))
                    .size(design::SUMMARY_VALUE)
                    .font(design::MONO)
            ]
            .spacing(design::SUMMARY_GAP);
            cell = match index {
                0 => cell.push(
                    text(localized(
                        "{duration} / day",
                        &[("duration", model::duration(total / count.max(1) as i64))],
                    ))
                    .size(design::LABEL)
                    .color(p.text3),
                ),
                1 => {
                    let rgb = design::CATEGORICAL[3];
                    cell.push(bar(
                        value,
                        total,
                        Color::from_rgb8(rgb[0], rgb[1], rgb[2]),
                        p,
                    ))
                }
                _ => cell.push(
                    button(
                        text(localized(
                            "Review {count} gaps →",
                            &[("count", gaps(&data.report).len().to_string())],
                        ))
                        .size(design::LABEL),
                    )
                    .on_press_maybe(
                        gaps(&data.report)
                            .first()
                            .copied()
                            .map(|r| Message::Report(Edit::ReviewGap(r))),
                    )
                    .style(design::link)
                    .padding(0)
                    .height(design::DENSE_TARGET),
                ),
            };
            metrics = metrics.push(container(cell).width(Length::Fill));
        }
        let strip = container(metrics)
            .padding(design::CONTROL_GAP as u16)
            .style(panel);
        let heading: Element<'_, Message> = if narrow {
            column![title, strip].spacing(design::GAP).into()
        } else {
            row![title.width(Length::Fill), strip.width(Length::Fill)]
                .spacing(design::GAP)
                .align_y(Alignment::Center)
                .into()
        };
        let first = data.days[0].date;
        let last = data.days.last().unwrap().date;
        let without_weekday = |date| {
            localization::short_date_label(date)
                .split_once(' ')
                .unwrap()
                .1
                .to_owned()
        };
        let range = if first.month() == last.month() && first.year() == last.year() {
            format!("{}–{}", first.day(), without_weekday(last))
        } else if first.year() == last.year() {
            format!("{} – {}", without_weekday(first), without_weekday(last))
        } else {
            format!(
                "{} {} – {} {}",
                without_weekday(first),
                first.year(),
                without_weekday(last),
                last.year()
            )
        };
        let nav = row![
            button(text("‹"))
                .on_press(Message::Shift(-1))
                .style(design::ghost)
                .width(design::DENSE_TARGET),
            button(text(
                if data
                    .days
                    .iter()
                    .any(|d| d.date == Local::now().date_naive())
                {
                    format!("{} · {range}", tr("This week"))
                } else {
                    range
                }
            ))
            .on_press(Message::Today)
            .style(design::ghost),
            button(text("›"))
                .on_press(Message::Shift(1))
                .style(design::ghost)
                .width(design::DENSE_TARGET)
        ]
        .align_y(Alignment::Center);
        let mut dates = column![space::vertical().height(WEEK_RULER)];
        let mut totals = column![space::vertical().height(WEEK_RULER)];
        let now = draugr_time_daemon::now_ms();
        for day in &data.days {
            let today = day.range.start <= now && now < day.range.end;
            let future = day.range.start > now;
            let caption = if today {
                tr("Today")
            } else if future {
                String::new()
            } else {
                localized(
                    "{count} sessions",
                    &[("count", sessions(&data.report, day.range).to_string())],
                )
            };
            let date_style = move |theme: &Theme, status| {
                let mut style = design::ghost(theme, status);
                style.border.radius = 0.0.into();
                if today {
                    style.background = Some(
                        Color {
                            a: design::TODAY_TINT,
                            ..p.accent
                        }
                        .into(),
                    );
                }
                style.text_color = if today {
                    p.accent_text
                } else if future {
                    p.text3
                } else {
                    p.text
                };
                style
            };
            dates = dates
                .push(
                    button(
                        column![
                            text(format!("{} {}", weekday(day.date), day.date.day()))
                                .font(design::SEMIBOLD)
                                .size(design::BODY),
                            text(caption).size(design::LABEL).color(p.text3)
                        ]
                        .spacing(design::SMALL_GAP),
                    )
                    .on_press(Message::Report(Edit::OpenDay(day.date)))
                    .height(WEEK_ROW)
                    .width(Length::Fill)
                    .style(date_style)
                    .padding(design::SMALL_GAP as u16),
                )
                .push(line(p));
            totals = totals
                .push(
                    button(
                        container(
                            text(if future {
                                "–".into()
                            } else {
                                model::duration(day.active_ms)
                            })
                            .font(design::MONO)
                            .size(design::META)
                            .color(if future {
                                p.text3
                            } else {
                                p.text
                            }),
                        )
                        .align_y(Alignment::Center)
                        .align_x(Alignment::End)
                        .height(Length::Fill)
                        .width(Length::Fill),
                    )
                    .on_press(Message::Report(Edit::OpenDay(day.date)))
                    .style(date_style)
                    .padding(0)
                    .height(WEEK_ROW)
                    .width(Length::Fill),
                )
                .push(line(p));
        }
        let timeline = row![
            dates.width(design::WEEK_DATE),
            Canvas::new(WeekCanvas {
                days: &data.days,
                report: &data.report,
                tags: &data.tags,
                highlight: &ui.highlight,
                now,
                stats: &self.stats
            })
            .width(Length::Fill)
            .height(WEEK_RULER + (WEEK_ROW + design::HAIRLINE) * data.days.len() as f32),
            totals.width(design::WEEK_TOTAL)
        ]
        .spacing(0);
        let timeline_header: Element<'_, Message> = if narrow {
            column![
                container(nav).style(design::segment_track),
                text(tr(
                    "Click a day to open it · top lane = tags, bottom = activity"
                ))
                .size(design::LABEL)
                .color(p.text3)
            ]
            .spacing(design::CONTROL_GAP)
            .into()
        } else {
            row![
                container(nav).style(design::segment_track),
                space::horizontal(),
                text(tr(
                    "Click a day to open it · top lane = tags, bottom = activity"
                ))
                .size(design::LABEL)
                .color(p.text3)
            ]
            .align_y(Alignment::Center)
            .into()
        };
        let timeline_card =
            container(column![timeline_header, timeline].spacing(design::CONTROL_GAP))
                .padding(design::CARD_PADDING)
                .style(panel);
        let mut body =
            column![heading, timeline_card, self.week_timesheet(data, narrow)].spacing(design::GAP);
        if !ui.error.is_empty() {
            body = body.push(text(localization::error(&ui.error)).size(design::META));
        }
        if narrow {
            body.into()
        } else {
            scrollable(body)
                .spacing(design::SMALL_GAP)
                .style(design::scrollbars)
                .into()
        }
    }
    fn week_timesheet<'a>(&'a self, data: &'a Data, narrow: bool) -> Element<'a, Message> {
        let p = design::Palette::new(self.theme.palette().background.r < 0.5);
        let heading = row![
            text(tr("Timesheet"))
                .font(design::SEMIBOLD)
                .size(design::BODY),
            text(tr("tags × days · hours"))
                .size(design::LABEL)
                .color(p.text3),
            space::horizontal(),
            button(text(tr("Export CSV")).size(design::LABEL))
                .on_press(Message::Report(Edit::Export(false)))
                .height(design::DENSE_TARGET)
                .style(design::link)
        ]
        .spacing(design::CONTROL_GAP)
        .align_y(Alignment::Center);
        let table = |name: Element<'a, Message>,
                     values: Vec<Element<'a, Message>>,
                     total: Element<'a, Message>| {
            let mut r =
                row![container(name).width(Length::FillPortion(3))].align_y(Alignment::Center);
            for value in values {
                r = r.push(container(value).width(Length::Fill).align_x(Alignment::End));
            }
            r.push(container(total).width(Length::Fill).align_x(Alignment::End))
                .height(design::WEEK_TABLE_ROW)
        };
        let header = table(
            text(tr("Tags").to_uppercase())
                .size(design::CAPTION)
                .color(p.text3)
                .into(),
            data.days
                .iter()
                .map(|d| {
                    text(weekday(d.date).to_uppercase())
                        .size(design::CAPTION)
                        .color(if d.date == Local::now().date_naive() {
                            p.accent_text
                        } else {
                            p.text3
                        })
                        .into()
                })
                .collect(),
            text(tr("Total").to_uppercase())
                .size(design::CAPTION)
                .color(p.text3)
                .into(),
        );
        let mut sheet = column![header.height(design::WEEK_TABLE_HEADER), line(p)];
        for r in tag_rows(&data.report) {
            let next = if self.report_ui.highlight == Some((r.tag_id, None)) {
                None
            } else {
                Some((r.tag_id, None))
            };
            let label = button(
                row![
                    swatch(tag_color(&data.tags, r.tag_id), r.tag_id.is_none(), p),
                    text(translate_label(&r.label))
                        .size(design::META)
                        .color(p.text)
                ]
                .align_y(Alignment::Center),
            )
            .on_press(Message::Report(Edit::Highlight(next)))
            .padding(0)
            .style(design::ghost)
            .height(design::DENSE_TARGET);
            sheet = sheet
                .push(table(
                    label.into(),
                    data.days
                        .iter()
                        .map(|d| {
                            text(hours(d.tag_totals.get(&r.tag_id).copied().unwrap_or(0)))
                                .size(design::LABEL)
                                .font(design::MONO)
                                .color(p.text2)
                                .into()
                        })
                        .collect(),
                    text(hours(r.duration_ms))
                        .size(design::LABEL)
                        .font(design::MONO)
                        .into(),
                ))
                .push(line(p));
        }
        if data.report.intervals.is_empty() {
            sheet = sheet.push(text(tr("No activity in this range")).size(design::BODY));
        }
        let sheet: Element<'a, Message> = if narrow {
            scrollable(sheet.width(design::WEEK_TABLE_MIN))
                .direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::default(),
                ))
                .style(design::scrollbars)
                .into()
        } else {
            sheet.into()
        };
        container(column![heading, sheet].spacing(design::CONTROL_GAP))
            .padding([design::GAP as u16, design::CARD_PADDING])
            .style(panel)
            .into()
    }
    pub(in crate::desktop) fn week_summary(&self) -> Element<'_, Message> {
        let p = design::Palette::new(self.theme.palette().background.r < 0.5);
        let mut body = column![row![
            text(tr("This week"))
                .size(design::HEADING)
                .font(design::SEMIBOLD),
            text(tr("vs last week"))
                .size(design::CAPTION)
                .color(p.text3)
        ]
        .spacing(design::CONTROL_GAP)
        .align_y(Alignment::Center)]
        .spacing(design::GAP);
        if let Some(data) = &self.report_ui.data {
            let rows = tag_rows(&data.report);
            let max = rows.iter().map(|r| r.duration_ms).max().unwrap_or(1);
            for r in rows {
                let color = tag_color(&data.tags, r.tag_id);
                let delta = r.duration_ms - data.previous.get(&r.tag_id).copied().unwrap_or(0);
                let next = if self.report_ui.highlight == Some((r.tag_id, None)) {
                    None
                } else {
                    Some((r.tag_id, None))
                };
                let label = row![
                    swatch(color, r.tag_id.is_none(), p),
                    text(translate_label(&r.label))
                        .color(p.text)
                        .size(design::BODY)
                        .width(Length::Fill),
                    text(model::duration(r.duration_ms))
                        .color(p.text)
                        .font(design::MONO)
                        .size(design::META)
                ]
                .align_y(Alignment::Center);
                let delta_row = row![
                    bar(r.duration_ms, max, color, p),
                    text(format!(
                        "{}{}",
                        if delta >= 0 { "+" } else { "−" },
                        model::duration(delta.abs())
                    ))
                    .size(design::LABEL)
                    .font(design::MONO)
                    .color(if delta == 0 {
                        p.text3
                    } else if (delta < 0) == r.tag_id.is_some() {
                        p.away_text
                    } else {
                        p.accent_text
                    })
                ]
                .spacing(design::CONTROL_GAP)
                .align_y(Alignment::Center);
                body = body.push(
                    button(
                        column![
                            label,
                            container(delta_row)
                                .padding(iced::Padding::default().left(design::WEEK_SWATCH))
                        ]
                        .spacing(design::CONTROL_GAP),
                    )
                    .on_press(Message::Report(Edit::Highlight(next)))
                    .style(design::ghost)
                    .padding(design::SMALL_GAP as u16)
                    .width(Length::Fill),
                );
            }
            let mut apps = column![text(tr("Most used apps").to_uppercase())
                .size(design::CAPTION)
                .color(p.text3)]
            .spacing(design::CONTROL_GAP);
            for r in data
                .report
                .rows(GroupBy::Application, None)
                .into_iter()
                .take(design::WEEK_APP_LIMIT)
            {
                apps = apps.push(
                    row![
                        self.app_icon(&r.label, &r.label),
                        text(translate_application(&r.label))
                            .size(design::META)
                            .width(Length::Fill),
                        text(model::duration(r.duration_ms))
                            .font(design::MONO)
                            .size(design::LABEL)
                            .color(p.text2)
                    ]
                    .align_y(Alignment::Center),
                );
            }
            body = body.push(
                container(apps)
                    .padding(design::CONTROL_GAP as u16)
                    .style(panel),
            );
        } else {
            body = body.push(text(tr("Loading…")).size(design::BODY));
        }
        column![
            scrollable(body)
                .style(design::scrollbars)
                .height(Length::Fill),
            text(tr("Stored on this computer only"))
                .size(design::LABEL)
                .color(p.text3),
            button(text(tr("Open full report →")).size(design::META))
                .on_press(Message::Report(Edit::OpenFullReport))
                .style(design::link)
                .padding(0)
        ]
        .spacing(design::GAP)
        .height(Length::Fill)
        .into()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn report() -> (NaiveDate, Report) {
        let date = NaiveDate::from_ymd_opt(2026, 10, 19).unwrap();
        let day = model::day_bounds(date);
        let at = |hour: i64| day.start + hour * 3_600_000;
        let mut report = Report {
            from: day.start,
            to: day.end,
            ..Default::default()
        };
        for (start, end, tag) in [
            (8, 9, None),
            (9, 10, None),
            (11, 12, Some(1)),
            (12, 13, Some(1)),
            (14, 15, None),
        ] {
            report.intervals.push(draugr_time_core::ReportInterval {
                start_ts: at(start),
                end_ts: at(end),
                tag_id: tag,
                tag: if tag.is_none() { "Untagged" } else { "Work" }.into(),
                application: "Editor".into(),
                document: "draft".into(),
            });
            report.active_ms += at(end) - at(start);
        }
        (date, report)
    }
    #[test]
    fn sessions_and_gaps_merge_adjacent_context_changes_but_keep_breaks() {
        let (date, report) = report();
        let day = Day::summarize(date, model::day_bounds(date), &report);
        assert_eq!(sessions(&report, day.range), 3);
        let gaps = gaps(&report);
        assert_eq!(gaps.len(), 2);
        assert_eq!(gaps[0].duration(), 2 * 3_600_000);
        assert_eq!(
            gaps.iter().map(|r| r.duration()).sum::<i64>(),
            3 * 3_600_000
        );
        assert_eq!(recorded_days(&[day]), 1);
    }
    #[test]
    fn timesheet_exports_exact_hour_values_and_escapes_tag_names() {
        let (date, mut report) = report();
        report.intervals[2].tag = "Client, \"Work\"".into();
        report.intervals[3].tag = report.intervals[2].tag.clone();
        let days = vec![Day::summarize(date, model::day_bounds(date), &report)];
        let csv = timesheet_csv(&report, &days);
        assert!(csv.contains("2026-10-19"));
        assert!(csv.contains("\"Client, \"\"Work\"\"\",2.0000,2.0000"));
        assert!(csv.contains("Untagged,3.0000,3.0000"));
        assert!(csv.contains("Total,5.0000,5.0000"));
    }
    #[test]
    fn dense_subpixel_activity_survives_and_outside_hours_is_clipped() {
        let date = NaiveDate::from_ymd_opt(2026, 10, 19).unwrap();
        let range = model::day_bounds(date);
        let tracks = model::fixture(range);
        let report = from_tracks(&tracks, &[], range).unwrap();
        let day = Day::summarize(date, range, &report);
        let pixels = rasterize(&day, &report, 400.0, range.end);
        assert!(report.intervals.len() >= 5000);
        assert!(pixels[1].iter().filter(|p| p.is_some()).count() > 250);
        assert!(pixels[0]
            .iter()
            .any(|p| p.is_some_and(|(id, _)| id.is_some())));
        assert!(pixels[1][0].is_none());
        assert!(pixels[1][399].is_none());
        let future = rasterize(&day, &report, 400.0, range.start);
        assert!(future.iter().flatten().all(Option::is_none));
    }
    #[test]
    fn review_gap_opens_day_preserves_selection_and_releases_week() {
        let (date, report) = report();
        let mut state = State::boot().0;
        state.fixture = true;
        state.screen = "Week";
        let gap = gaps(&report)[0];
        let _ = state.edit_report(Edit::ReviewGap(gap));
        assert_eq!(state.screen, "Day");
        assert_eq!(state.date, date);
        assert_eq!(state.selection, Some(gap));
        assert!(state.report_ui.data.is_none());
    }
    #[test]
    fn full_report_opens_the_week_being_reviewed() {
        let mut state = State::boot().0;
        state.fixture = true;
        state.screen = "Week";
        state.date = NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();
        let _ = state.edit_report(Edit::Refresh);
        let _ = state.edit_report(Edit::OpenFullReport);
        assert_eq!(state.screen, "Reports");
        assert_eq!(state.report_ui.from, "2026-10-19");
        assert_eq!(state.report_ui.to, "2026-10-25");
        assert_eq!(state.report_ui.data.as_ref().unwrap().days.len(), 7);
    }
    #[test]
    fn load_previous_week_keeps_only_totals_and_clips_its_boundaries() {
        let date = NaiveDate::from_ymd_opt(2026, 10, 19).unwrap();
        let range = model::day_bounds(date);
        let previous = previous_week(date).unwrap();
        let shared = draugr_time_daemon::Shared::new(
            draugr_time_core::Storage::open_in_memory().unwrap(),
            "test".into(),
            300,
        );
        {
            let mut db = shared.storage.lock().unwrap();
            for (start, end) in [
                (previous.start - 60_000, previous.start + 60_000),
                (range.start - 60_000, range.start + 120_000),
            ] {
                db.record_interval("afk", &serde_json::json!({"status":"active"}), start, end)
                    .unwrap();
                db.record_interval(
                    "window",
                    &serde_json::json!({"app":"Editor","title":"draft"}),
                    start,
                    end,
                )
                .unwrap();
            }
        }
        let data = load(
            shared,
            date.to_string(),
            date.checked_add_days(Days::new(6)).unwrap().to_string(),
            true,
        )
        .unwrap();
        assert_eq!(data.previous[&None], 120_000);
        assert_eq!(data.report.active_ms, 120_000);
        assert_eq!(data.days.len(), 7);
    }
    #[test]
    #[ignore = "writes Week review screenshots; run explicitly with --test-threads=1"]
    fn render_week_review_screenshots() {
        use iced::advanced::{
            layout, mouse, renderer, renderer::Headless as _, widget::Tree, Layout, Renderer as _,
        };
        use std::borrow::Cow;
        {
            let mut fonts = iced::advanced::graphics::text::font_system()
                .write()
                .unwrap();
            fonts.load_font(Cow::Borrowed(include_bytes!(
                "../../../fonts/InterTight.ttf"
            )));
            fonts.load_font(Cow::Borrowed(include_bytes!(
                "../../../fonts/JetBrainsMono-Regular.ttf"
            )));
        }
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/screenshots");
        std::fs::create_dir_all(&path).unwrap();
        let (mut state, _) = State::boot();
        state.fixture = true;
        state.settings_ui.preferences.onboarded = true;
        state.status.state = "tracking".into();
        state.date = NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();
        state.screen = "Week";
        let _ = state.edit_report(Edit::Refresh);
        for language in [
            localization::Language::En,
            localization::Language::Ru,
            localization::Language::De,
        ] {
            localization::set(language);
            for dark in [true, false] {
                for (width, height) in [(1280, 820), (1280, 1000), (700, 820), (700, 1700)] {
                    localization::set(language);
                    state.theme = native_theme(dark);
                    let size = Size::new(width as f32, height as f32);
                    let viewport = Rectangle::with_size(size);
                    let mut renderer = Renderer::new(design::UI, 13.0.into());
                    let mut view = state.view_sized(size.width);
                    let mut tree = Tree::new(view.as_widget());
                    let node = view.as_widget_mut().layout(
                        &mut tree,
                        &renderer,
                        &layout::Limits::new(Size::ZERO, size),
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
                    let rgba = renderer.screenshot(
                        iced::Size::new(width, height),
                        1.0,
                        state.theme.palette().background,
                    );
                    let file = std::fs::File::create(path.join(format!(
                        "m12-week-{}-{}-{width}x{height}.png",
                        language.code(),
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
                        "{} {} {width}×{height}: canvas {:.2}ms",
                        language.code(),
                        if dark { "dark" } else { "light" },
                        state.stats.max_build_us.load(Ordering::Relaxed) as f64 / 1000.0
                    );
                    assert!(state.stats.max_build_us.load(Ordering::Relaxed) < 100_000);
                }
            }
        }
        localization::set(localization::Language::En);
    }
    #[test]
    fn previous_period_uses_local_dates_across_month_and_year_boundaries() {
        let start = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let previous = previous_week(start).unwrap();
        assert_eq!(
            previous.start,
            model::day_bounds(NaiveDate::from_ymd_opt(2025, 12, 25).unwrap()).start
        );
        assert_eq!(previous.end, model::day_bounds(start).start);
    }
}
