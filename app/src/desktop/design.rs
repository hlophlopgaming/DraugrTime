use iced::{Color, Font};

pub const UI: Font = Font::with_name("Inter Tight");
pub const MONO: Font = Font::with_name("JetBrains Mono");
pub const RAIL: f32 = 72.0;
pub const RAIL_ITEM: f32 = 56.0;
pub const RAIL_INSET: u16 = 8;
pub const PANEL: f32 = 344.0;
pub const SETTINGS_PANEL: f32 = 220.0;
pub const PADDING: u16 = 24;
pub const GAP: u32 = 16;
pub const RADIUS: f32 = 12.0;
pub const TITLE: u32 = 30;
pub const HEADING: u32 = 20;
pub const BODY: u32 = 13;
pub const META: u32 = 12;
pub const LABEL: u32 = 11;
pub const SMALL_GAP: u32 = 4;
pub const CONTROL_GAP: u32 = 8;
pub const RAIL_PADDING: u16 = 10;
pub const TOP_PADDING: u16 = 8;
pub const LEGACY_WIDTH: f32 = 900.0;
pub const NARROW_BODY_HEIGHT: f32 = 540.0;
pub const NARROW_MAIN_HEIGHT: f32 = 880.0;
pub const LOGO: f32 = 36.0;
pub const TARGET: f32 = 52.0;

pub fn narrow(width: f32) -> bool {
    width < 760.0
}

fn navigation_label(label: &str) -> iced::advanced::graphics::text::Paragraph {
    use iced::advanced::text::Paragraph as _;
    iced::advanced::graphics::text::Paragraph::with_text(iced::advanced::Text {
        content: label,
        bounds: iced::Size::INFINITE,
        size: (LABEL as f32).into(),
        line_height: iced::advanced::text::LineHeight::default(),
        font: UI,
        align_x: iced::advanced::text::Alignment::Center,
        align_y: iced::alignment::Vertical::Center,
        shaping: iced::advanced::text::Shaping::Advanced,
        wrapping: iced::advanced::text::Wrapping::None,
    })
}
#[derive(Clone, Copy)]
pub struct Navigation {
    pub item_width: f32,
    pub item_height: f32,
}
pub fn navigation_metrics(language: super::localization::Language) -> Navigation {
    use iced::advanced::text::Paragraph as _;
    struct Cached {
        language: super::localization::Language,
        fonts: iced::advanced::graphics::text::Version,
        metrics: Navigation,
    }
    static CACHE: std::sync::OnceLock<std::sync::Mutex<Option<Cached>>> =
        std::sync::OnceLock::new();
    let fonts = iced::advanced::graphics::text::font_system()
        .read()
        .unwrap()
        .version();
    let mut cache = CACHE
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap();
    if let Some(value) = cache
        .as_ref()
        .filter(|v| v.language == language && v.fonts == fonts)
    {
        return value.metrics;
    }
    let mut metrics = Navigation {
        item_width: RAIL_ITEM,
        item_height: TARGET,
    };
    for key in ["Day", "Week", "Tags", "Rules", "Settings"] {
        let label = navigation_label(&super::localization::translate(language, key));
        metrics.item_width = metrics
            .item_width
            .max((label.min_width() + 2.0 * CONTROL_GAP as f32).ceil());
        metrics.item_height = metrics
            .item_height
            .max((ICON + SMALL_GAP as f32 + label.min_height() + 2.0 * CONTROL_GAP as f32).ceil());
    }
    *cache = Some(Cached {
        language,
        fonts,
        metrics,
    });
    metrics
}

// The complete spec palette is shared with subsequent screen slices.
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color,
    pub side: Color,
    pub surf: Color,
    pub raised: Color,
    pub hover: Color,
    pub line: Color,
    pub line_soft: Color,
    pub grid_hour: Color,
    pub grid_half: Color,
    pub text: Color,
    pub text2: Color,
    pub text3: Color,
    pub accent: Color,
    pub accent_text: Color,
    pub accent_dim: Color,
    pub accent_ink: Color,
    pub away: Color,
    pub away_dim: Color,
    pub away_text: Color,
    pub danger: Color,
    pub danger_text: Color,
}
impl Palette {
    pub fn new(dark: bool) -> Self {
        let color = |dark_value, light_value| {
            let value = if dark { dark_value } else { light_value };
            Color::from_rgb8((value >> 16) as u8, (value >> 8) as u8, value as u8)
        };
        Self {
            bg: color(0x0D1015, 0xF4F2EE),
            side: color(0x10141A, 0xFBFAF7),
            surf: color(0x151A21, 0xFFFFFF),
            raised: color(0x1B2129, 0xF0EEE9),
            hover: color(0x222A34, 0xE9E6E0),
            line: color(0x252D38, 0xE3DFD7),
            line_soft: color(0x1E252E, 0xECE9E3),
            grid_hour: color(0x262E39, 0xDDD9D1),
            grid_half: color(0x1A2028, 0xEFECE7),
            text: color(0xECE7DC, 0x1A1F26),
            text2: color(0xA7AFBA, 0x4E5764),
            text3: color(0x8E97A4, 0x646C78),
            accent: color(0x5FD4B8, 0x2BB596),
            accent_text: color(0x5FD4B8, 0x137A66),
            accent_dim: color(0x1E3A35, 0xDDF3EC),
            accent_ink: color(0x0B1F1A, 0x0B1F1A),
            away: color(0xD9A54A, 0xD9A54A),
            away_dim: color(0x3A2E1A, 0xF4E2BD),
            away_text: color(0xD9A54A, 0x8F6210),
            danger: color(0xE0715F, 0xD55A46),
            danger_text: color(0xF09484, 0xB4412F),
        }
    }
}

pub const TRACK_LABEL: f32 = 132.0;
pub const CARD_PADDING: u16 = 20;
pub const HOUR_CARD: f32 = 114.0;
pub const BAR_HEIGHTS: [f32; 4] = [34.0, 18.0, 26.0, 18.0];
pub const TRACK_HEIGHTS: [f32; 4] = [76.0, 66.0, 116.0, 156.0];
pub const LANE: f32 = 18.0;
pub fn bar_height(track: usize) -> f32 {
    BAR_HEIGHTS[track]
}
pub fn track_height(track: usize, lanes: usize) -> f32 {
    if lanes == 0 {
        TRACK_HEIGHTS[track]
    } else {
        48.0 + lanes as f32 * LANE
    }
}
pub const CATEGORICAL: [[u8; 3]; 7] = [
    [199, 146, 234],
    [231, 182, 92],
    [108, 198, 232],
    [124, 156, 240],
    [155, 214, 122],
    [232, 138, 155],
    [95, 212, 184],
];

pub use super::components::button;
fn colors(theme: &iced::Theme) -> Palette {
    Palette::new(theme.palette().background.r < 0.5)
}
pub fn secondary(
    theme: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let p = colors(theme);
    iced::widget::button::Style {
        background: matches!(status, iced::widget::button::Status::Hovered)
            .then_some(p.hover.into()),
        text_color: if matches!(status, iced::widget::button::Status::Disabled) {
            p.text3
        } else {
            p.text
        },
        border: iced::Border {
            color: p.line,
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    }
}
pub fn ghost(
    theme: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let p = colors(theme);
    let mut style = secondary(theme, status);
    style.border.width = 0.0;
    style.text_color = p.text2;
    style
}
pub fn primary(
    theme: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let p = colors(theme);
    let mut style = secondary(theme, status);
    style.background = Some(p.accent.into());
    style.text_color = p.accent_ink;
    style.border.radius = 9.0.into();
    style
}
pub fn destructive(
    theme: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let p = colors(theme);
    let mut style = secondary(theme, status);
    style.background = Some(
        iced::Color {
            a: 0.05,
            ..p.danger
        }
        .into(),
    );
    style.text_color = p.danger_text;
    style.border.color = p.danger;
    style
}
pub fn segment(
    active: bool,
) -> impl Fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style {
    move |theme, status| {
        let p = colors(theme);
        let mut style = ghost(theme, status);
        if active {
            style.background = Some(p.hover.into());
            style.text_color = p.text;
        } else {
            style.text_color = if matches!(status, iced::widget::button::Status::Hovered) {
                p.text2
            } else {
                p.text3
            };
        }
        style.border.radius = 7.0.into();
        style
    }
}

pub fn field(
    theme: &iced::Theme,
    status: iced::widget::text_input::Status,
) -> iced::widget::text_input::Style {
    let p = colors(theme);
    iced::widget::text_input::Style {
        background: p.surf.into(),
        border: iced::Border {
            color: if matches!(status, iced::widget::text_input::Status::Focused { .. }) {
                p.accent_text
            } else {
                p.line
            },
            width: if matches!(status, iced::widget::text_input::Status::Focused { .. }) {
                2.0
            } else {
                1.0
            },
            radius: 8.0.into(),
        },
        icon: p.text3,
        placeholder: p.text3,
        value: p.text,
        selection: p.accent_dim,
    }
}
pub fn text_input<'a, Message: Clone + 'a>(
    placeholder: &str,
    value: &str,
) -> iced::widget::TextInput<'a, Message> {
    iced::widget::text_input(placeholder, value)
        .size(BODY)
        .padding([FIELD_VERTICAL, FIELD_HORIZONTAL])
        .style(field)
}

pub fn selected(
    theme: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    segment(true)(theme, status)
}
pub const SEMIBOLD: iced::Font = iced::Font {
    weight: iced::font::Weight::Semibold,
    ..UI
};

pub const CAPTION: u32 = 10;
pub const SUMMARY_VALUE: u32 = 17;
pub const SUMMARY_GAP: u32 = 2;
pub const CONTROL: f32 = 36.0;
pub const DENSE_TARGET: f32 = 28.0;
pub const ICON: f32 = 18.0;
pub const APP_ICON: f32 = 24.0;
pub const TAG_PICKER: f32 = 150.0;
pub const POPOVER_WIDTH: f32 = 240.0;
pub const PROGRESS: f32 = 3.0;
pub const TRACK_HEAD: f32 = 48.0;
pub const LANE_BAR: f32 = 10.0;
pub const TRACK_LABEL_TOP: [f32; 4] = [20.0, 16.0, 46.0, 45.0];
pub const LANE_DOT: f32 = 6.0;
pub fn bar_offset(track: usize) -> f32 {
    if track == 3 {
        49.0
    } else {
        (TRACK_HEIGHTS[track] - BAR_HEIGHTS[track]) / 2.0
    }
}
pub const TOOLTIP_DELAY: u64 = 120;
pub fn popover(theme: &iced::Theme) -> iced::widget::container::Style {
    let p = colors(theme);
    iced::widget::container::Style {
        background: Some(p.raised.into()),
        border: iced::Border {
            width: 1.0,
            color: p.line,
            radius: 10.0.into(),
        },
        ..Default::default()
    }
}
pub fn rail_icon(screen: &str, color: Color) -> iced::widget::Svg<'static> {
    let path=match screen {
        "Day"=>"M5 3v4m8-4v4M3 8h12M5 11h3v3H5zM4 5h10a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1z",
        "Week"=>"M5 3v4m8-4v4M3 8h12M7 8v8m4-8v8M4 5h10a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1z",
        "Tags"=>"M3 3h6l6 6-6 6-6-6zM6 6h.01",
        "Rules"=>"M3 5h12M5 9h8M7 13h4",
        _=>"M9 1v2m0 12v2M1 9h2m12 0h2M3 3l2 2m8 8 2 2M3 15l2-2m8-8 2-2M9 5a4 4 0 1 0 0 8 4 4 0 0 0 0-8",
    };
    iced::widget::svg(iced::widget::svg::Handle::from_memory(format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 18 18"><path d="{path}" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/></svg>"#).into_bytes())).width(ICON).height(ICON).style(move |_,_|iced::widget::svg::Style {color:Some(color)})
}

pub const SEGMENT_PADDING: u16 = 3;
pub const SUMMARY_DIVIDER: f32 = 64.0;
pub fn segment_track(theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(colors(theme).side.into()),
        border: iced::Border::default().rounded(10),
        ..Default::default()
    }
}

pub fn scrollbars(
    theme: &iced::Theme,
    status: iced::widget::scrollable::Status,
) -> iced::widget::scrollable::Style {
    let p = colors(theme);
    let mut style = iced::widget::scrollable::default(theme, status);
    for rail in [&mut style.vertical_rail, &mut style.horizontal_rail] {
        rail.background = Some(p.line_soft.into());
        rail.scroller.background = p.text3.into();
    }
    style
}

pub const SEGMENT_ITEM: f32 = 30.0;
pub const PANEL_GAP: u32 = 12;
pub fn pick_list(
    theme: &iced::Theme,
    _: iced::widget::pick_list::Status,
) -> iced::widget::pick_list::Style {
    let p = colors(theme);
    iced::widget::pick_list::Style {
        text_color: p.text,
        placeholder_color: p.text3,
        handle_color: p.text2,
        background: p.surf.into(),
        border: iced::Border {
            width: 1.0,
            color: p.line,
            radius: 8.0.into(),
        },
    }
}

fn luminance(c: Color) -> f32 {
    let linear = |v: f32| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(c.r) + 0.7152 * linear(c.g) + 0.0722 * linear(c.b)
}
pub fn categorical_ink(color: Color) -> Color {
    let ink = Palette::new(true).accent_ink;
    let background = luminance(color);
    let preferred = luminance(ink);
    if (background.max(preferred) + 0.05) / (background.min(preferred) + 0.05) >= 4.5 {
        ink
    } else if (1.0 + 0.05) / (background + 0.05) >= 4.5 {
        Color::WHITE
    } else {
        Color::BLACK
    }
}

pub fn link(
    theme: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let p = colors(theme);
    let mut style = ghost(theme, status);
    style.background = None;
    style.text_color = p.accent_text;
    style
}

// Week view dimensions shared by layout and cached canvas.
pub const HAIRLINE: f32 = 1.0;
pub const WEEK_ROW: f32 = 64.0;
pub const WEEK_RULER: f32 = 24.0;
pub const WEEK_DATE: f32 = 112.0;
pub const WEEK_TOTAL: f32 = 70.0;
pub const WEEK_SWATCH: f32 = 20.0;
pub const WEEK_SWATCH_SIZE: f32 = 10.0;
pub const WEEK_TABLE_ROW: f32 = 30.0;
pub const WEEK_TABLE_HEADER: f32 = 24.0;
pub const WEEK_TABLE_MIN: f32 = 720.0;
pub const WEEK_APP_LIMIT: usize = 5;
pub const TODAY_TINT: f32 = 0.08;
pub const WEEK_TAG_Y: f32 = 16.0;
pub const WEEK_ACTIVITY_Y: f32 = 30.0;
pub const WEEK_TAG_HEIGHT: f32 = 10.0;
pub const WEEK_ACTIVITY_HEIGHT: f32 = 16.0;
pub const WEEK_DIAGNOSTIC_HEIGHT: f32 = 3.0;
pub const WEEK_FIRST_HOUR: f32 = 7.0;
pub const WEEK_LAST_HOUR: f32 = 21.0;

pub const HATCH_STEP: f32 = 6.0;
pub const DASH_LENGTH: f32 = 3.0;
pub const DIMMED_ALPHA: f32 = 0.16;
pub const WEEK_AXIS_LABEL: f32 = 36.0;

pub const TAG_SWATCH: f32 = 10.0;
pub const SWITCH_KNOB: f32 = 16.0;
pub const SWITCH_WIDTH: f32 = 34.0;
pub const ERROR_UNDERLINE: f32 = 4.0;
pub const MANAGER_SEARCH: f32 = 220.0;
pub const MANAGER_DURATION: f32 = 70.0;
pub const MANAGER_ALL: f32 = 100.0;
pub const MANAGER_RULE_COUNT: f32 = 80.0;
pub const MANAGER_LAST: f32 = 88.0;
pub const MANAGER_TABLE_MIN: f32 = 760.0;
pub const TREE_INDENT: f32 = 20.0;
pub const RULE_NUMBER: f32 = 24.0;
pub const RULE_SWITCH_CELL: f32 = 56.0;
pub const RULE_FIELD_CELL: f32 = 72.0;
pub const FOCUS_WIDTH: f32 = 2.0;
pub const CHART_AXIS: f32 = 18.0;
pub const TAG_CHART_HEIGHT: f32 = 70.0;
pub const MANAGER_TABLE_PADDING: u16 = 12;

pub const FIELD_VERTICAL: u16 = 9;
pub const FIELD_HORIZONTAL: u16 = 10;
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_automatically_fits_every_translated_label_on_one_line() {
        use iced::advanced::text::Paragraph as _;
        iced::advanced::graphics::text::font_system()
            .write()
            .unwrap()
            .load_font(std::borrow::Cow::Borrowed(include_bytes!(
                "../../fonts/InterTight.ttf"
            )));
        for language in super::super::localization::Language::ALL {
            let metrics = navigation_metrics(language);
            let width = metrics.item_width;
            for key in ["Day", "Week", "Tags", "Rules", "Settings"] {
                let label = super::super::localization::translate(language, key);
                let measured = navigation_label(&label);
                assert!(
                    measured.min_width() + 2.0 * CONTROL_GAP as f32 <= width,
                    "{}: {label} exceeds its navigation target",
                    language.code()
                );
                assert!(width >= RAIL_ITEM);
                assert!(
                    measured.min_height() + ICON + SMALL_GAP as f32 + 2.0 * CONTROL_GAP as f32
                        <= metrics.item_height
                );
            }
        }
        assert!(
            navigation_metrics(super::super::localization::Language::De).item_width > RAIL_ITEM
        );
    }
    #[test]
    fn day_track_geometry_keeps_bars_thin_and_lanes_separate() {
        assert_eq!(track_height(0, 0), 76.0);
        assert_eq!(track_height(2, 0), 116.0);
        assert_eq!(track_height(2, 6), 156.0);
        assert_eq!(track_height(3, 8), 192.0);
        assert_eq!(bar_height(0), 34.0);
        assert_eq!(bar_height(1), 18.0);
        assert_eq!(bar_height(2), 26.0);
        assert_eq!(bar_offset(3), 49.0);
    }

    #[test]
    fn narrow_layout_changes_at_the_spec_boundary() {
        assert!(narrow(759.0));
        assert!(!narrow(760.0));
        assert!(!narrow(1280.0));
    }

    #[test]
    fn button_text_keeps_contrast_in_both_themes_and_hover_states() {
        fn l(c: Color) -> f32 {
            let linear = |v: f32| {
                if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * linear(c.r) + 0.7152 * linear(c.g) + 0.0722 * linear(c.b)
        }
        for dark in [false, true] {
            let theme = super::super::native_theme(dark);
            let p = Palette::new(dark);
            for status in [
                iced::widget::button::Status::Active,
                iced::widget::button::Status::Hovered,
                iced::widget::button::Status::Disabled,
            ] {
                for style in [
                    secondary(&theme, status),
                    ghost(&theme, status),
                    primary(&theme, status),
                    link(&theme, status),
                    destructive(&theme, status),
                    segment(false)(&theme, status),
                    segment(true)(&theme, status),
                ] {
                    for surface in [p.bg, p.side, p.surf, p.raised] {
                        let bg = match style.background {
                            Some(iced::Background::Color(c)) => Color {
                                r: c.r * c.a + surface.r * (1.0 - c.a),
                                g: c.g * c.a + surface.g * (1.0 - c.a),
                                b: c.b * c.a + surface.b * (1.0 - c.a),
                                a: 1.0,
                            },
                            _ => surface,
                        };
                        let a = l(style.text_color);
                        let b = l(bg);
                        assert!(
                            (a.max(b) + 0.05) / (a.min(b) + 0.05) >= 4.5,
                            "dark={dark}, status={status:?}, text={:?}, bg={bg:?}",
                            style.text_color
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn custom_dark_and_mid_tone_tag_colors_keep_readable_labels() {
        for rgb in [[0, 0, 0], [128, 128, 128], [100, 100, 100], [255, 255, 255]]
            .into_iter()
            .chain(CATEGORICAL)
        {
            let bg = Color::from_rgb8(rgb[0], rgb[1], rgb[2]);
            let ink = categorical_ink(bg);
            let a = luminance(bg);
            let b = luminance(ink);
            assert!((a.max(b) + 0.05) / (a.min(b) + 0.05) >= 4.5);
        }
    }

    #[test]
    fn text_tokens_have_readable_contrast_on_surfaces() {
        fn luminance(c: iced::Color) -> f32 {
            let linear = |v: f32| {
                if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * linear(c.r) + 0.7152 * linear(c.g) + 0.0722 * linear(c.b)
        }
        for dark in [false, true] {
            let p = Palette::new(dark);
            for ink in [
                p.text,
                p.text2,
                p.text3,
                p.accent_text,
                p.away_text,
                p.danger_text,
            ] {
                for surface in [p.bg, p.side, p.surf] {
                    let a = luminance(ink);
                    let b = luminance(surface);
                    assert!((a.max(b) + 0.05) / (a.min(b) + 0.05) >= 4.5);
                }
            }
        }
    }
}
