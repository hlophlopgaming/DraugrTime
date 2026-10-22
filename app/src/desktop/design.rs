use iced::{Color, Font};

pub const UI: Font = Font::with_name("Inter Tight");
pub const MONO: Font = Font::with_name("JetBrains Mono");
pub const RAIL: f32 = 72.0;
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
pub const SEARCH_WIDTH: f32 = 250.0;
pub const LEGACY_WIDTH: f32 = 900.0;
pub const NARROW_BODY_HEIGHT: f32 = 540.0;
pub const NARROW_MAIN_HEIGHT: f32 = 880.0;
pub const LOGO: f32 = 36.0;
pub const TARGET: f32 = 52.0;

pub fn narrow(width: f32) -> bool {
    width < 760.0
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_layout_changes_at_the_spec_boundary() {
        assert!(narrow(759.0));
        assert!(!narrow(760.0));
        assert!(!narrow(1280.0));
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
