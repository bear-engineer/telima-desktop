//! Design tokens: the single source of truth for colors, type and metrics in the app.
//! Values come from the "telima Instrument v0" design (canvas artboard 06, telima.css).
//! Views read these tokens; they never hard-code colors or sizes.

/// A 24-bit sRGB color, `0xRRGGBB`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u32);

impl Rgb {
    /// WCAG 2.x relative luminance.
    pub fn luminance(self) -> f64 {
        let channel = |shift: u32| {
            let c = f64::from((self.0 >> shift) & 0xFF) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
    }

    /// WCAG 2.x contrast ratio between two colors (1.0 to 21.0).
    pub fn contrast(self, other: Rgb) -> f64 {
        let (a, b) = (self.luminance(), other.luminance());
        let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }
}

/// A token color with alpha (CSS `rgba()`), for overlays such as the scrim.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    pub rgb: Rgb,
    pub alpha: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    Light,
    Dark,
}

/// Semantic colors for one appearance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub bg: Rgb,
    pub bg_sunken: Rgb,
    pub bg_chrome: Rgb,
    pub bg_hover: Rgb,
    pub bg_raised: Rgb,
    pub bg_selected: Rgb,
    /// Pressed (active) background of default and ghost buttons and segments.
    pub bg_pressed: Rgb,
    pub border: Rgb,
    pub border_strong: Rgb,
    pub text: Rgb,
    pub text_2: Rgb,
    pub text_disabled: Rgb,
    pub accent: Rgb,
    pub accent_text: Rgb,
    /// Primary button hover (CSS accent + brightness 1.08); keeps `on_accent` >= 4.5:1.
    pub accent_hover: Rgb,
    /// Primary button pressed (accent - 8% brightness).
    pub accent_pressed: Rgb,
    pub on_accent: Rgb,
    pub focus: Rgb,
    pub ok: Rgb,
    pub ok_bg: Rgb,
    pub idle: Rgb,
    pub idle_bg: Rgb,
    pub busy: Rgb,
    pub busy_bg: Rgb,
    pub err: Rgb,
    pub err_bg: Rgb,
    pub err_border: Rgb,
    pub bar: Rgb,
    pub track: Rgb,
    pub skeleton: Rgb,
    /// Modal backdrop (CSS --scrim).
    pub scrim: Rgba,
}

/// Palette for `appearance`.
pub fn palette(appearance: Appearance) -> &'static Palette {
    match appearance {
        Appearance::Light => &LIGHT,
        Appearance::Dark => &DARK,
    }
}

/// Neutral ramp shared by both appearances (CSS --gray-*).
mod gray {
    use super::Rgb;
    pub const G0: Rgb = Rgb(0xFFFFFF);
    pub const G25: Rgb = Rgb(0xFAFAFA);
    pub const G50: Rgb = Rgb(0xF5F5F6);
    pub const G100: Rgb = Rgb(0xECECEE);
    pub const G200: Rgb = Rgb(0xE4E4E7);
    pub const G300: Rgb = Rgb(0xD1D1D6);
    pub const G400: Rgb = Rgb(0xA1A1AB);
    pub const G600: Rgb = Rgb(0x5E5E67);
    pub const G700: Rgb = Rgb(0x3A3A40);
    pub const G750: Rgb = Rgb(0x2C2C31);
    pub const G800: Rgb = Rgb(0x242428);
    pub const G850: Rgb = Rgb(0x1C1C1F);
    pub const G900: Rgb = Rgb(0x161618);
    pub const G950: Rgb = Rgb(0x111113);
}

const LIGHT: Palette = Palette {
    bg: gray::G0,
    bg_sunken: gray::G25,
    bg_chrome: gray::G50,
    bg_hover: gray::G100,
    bg_raised: gray::G0,
    bg_selected: Rgb(0xEEEBFC),
    bg_pressed: gray::G200,
    border: gray::G200,
    border_strong: gray::G300,
    text: gray::G900,
    text_2: gray::G600,
    text_disabled: gray::G400,
    accent: Rgb(0x6240E0),
    accent_text: Rgb(0x5634D0),
    accent_hover: Rgb(0x6A45F2),
    accent_pressed: Rgb(0x5A3BCE),
    on_accent: gray::G0,
    focus: Rgb(0x6240E0),
    ok: Rgb(0x12733A),
    ok_bg: Rgb(0xE8F5EC),
    idle: gray::G600,
    idle_bg: Rgb(0xEFEFF1),
    busy: Rgb(0x1F5FD1),
    busy_bg: Rgb(0xE9F0FD),
    err: Rgb(0xC42B2B),
    err_bg: Rgb(0xFCECEC),
    err_border: Rgb(0xEBC4C4),
    bar: gray::G700,
    track: gray::G100,
    skeleton: gray::G100,
    scrim: Rgba {
        rgb: gray::G900,
        alpha: 0.32,
    },
};

const DARK: Palette = Palette {
    bg: gray::G900,
    bg_sunken: gray::G950,
    bg_chrome: gray::G850,
    bg_hover: gray::G800,
    bg_raised: gray::G800,
    bg_selected: Rgb(0x272338),
    bg_pressed: gray::G750,
    border: gray::G750,
    border_strong: gray::G700,
    text: gray::G100,
    text_2: gray::G400,
    text_disabled: gray::G600,
    accent: Rgb(0x6D4AEB),
    accent_text: Rgb(0xA891FF),
    accent_hover: Rgb(0x7650FE),
    accent_pressed: Rgb(0x6444D8),
    on_accent: gray::G0,
    focus: Rgb(0xA891FF),
    ok: Rgb(0x3CC37A),
    ok_bg: Rgb(0x16271D),
    idle: gray::G400,
    idle_bg: gray::G800,
    busy: Rgb(0x6AA3FF),
    busy_bg: Rgb(0x16213A),
    err: Rgb(0xFF6E6E),
    err_bg: Rgb(0x33191A),
    err_border: Rgb(0x5A2A2B),
    bar: gray::G300,
    track: gray::G800,
    skeleton: gray::G800,
    scrim: Rgba {
        rgb: Rgb(0x000000),
        alpha: 0.5,
    },
};

/// Font size and line height in px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypeStyle {
    pub size: f32,
    pub line_height: f32,
}

pub mod font {
    use super::TypeStyle;

    pub const SANS: &str = "IBM Plex Sans KR";
    pub const MONO: &str = "IBM Plex Mono";
    pub const TITLE: TypeStyle = TypeStyle {
        size: 16.0,
        line_height: 22.0,
    };
    pub const BODY: TypeStyle = TypeStyle {
        size: 13.0,
        line_height: 20.0,
    };
    pub const SMALL: TypeStyle = TypeStyle {
        size: 12.0,
        line_height: 16.0,
    };
    pub const CODE: TypeStyle = TypeStyle {
        size: 12.0,
        line_height: 18.0,
    };
}

/// Spacing, radii and fixed sizes in px.
pub mod metric {
    /// 4pt spacing scale (CSS --sp-1..--sp-6, --sp-8).
    pub const SPACE: [f32; 7] = [4.0, 8.0, 12.0, 16.0, 20.0, 24.0, 32.0];
    pub const RADIUS_SM: f32 = 4.0;
    pub const RADIUS_MD: f32 = 6.0;
    pub const RADIUS_LG: f32 = 8.0;
    pub const RADIUS_WINDOW: f32 = 10.0;
    pub const ROW: f32 = 28.0;
    pub const TITLEBAR: f32 = 40.0;
    pub const SIDEBAR: f32 = 220.0;
    pub const INSPECTOR: f32 = 260.0;
    pub const ICON: f32 = 16.0;
    pub const ICON_STROKE: f32 = 1.5;
    /// Status dot / ring diameter.
    pub const DOT: f32 = 8.0;
    /// Status chip height (CSS .chip).
    pub const CHIP_H: f32 = 22.0;
    /// Chip leading padding (CSS .chip padding-left), before the status shape.
    pub const CHIP_PAD_START: f32 = 6.0;
    /// Button heights (CSS .btn; small variant for dense rows).
    pub const BTN: f32 = 28.0;
    pub const BTN_SM: f32 = 24.0;
    /// Button horizontal padding (CSS .btn padding 0 10px).
    pub const BTN_PAD_X: f32 = 10.0;
    /// Gap between a segmented control's border and its segments.
    pub const SEGMENT_INSET: f32 = 2.0;
    /// Tab bar height (CSS .tab).
    pub const TAB_H: f32 = 36.0;
    /// Key-value label column (CSS .kv) and form label column.
    pub const KV_LABEL: f32 = 88.0;
    pub const FIELD_LABEL: f32 = 96.0;
    /// Number input and select widths in forms.
    pub const FIELD_NUM: f32 = 64.0;
    pub const FIELD_SELECT: f32 = 220.0;
    /// Search field in a panel header (Containers.dc.html 로그 검색).
    pub const FIELD_SEARCH: f32 = 160.0;
    /// Short ID table column: 12 IBM Plex Mono characters at the code size (Containers.dc.html).
    pub const COL_ID: f32 = 88.0;
    /// Engine panel width below which the containers table hides its ID column (the ID is
    /// then in the 이름 cell's tooltip), so 상태 and 포트 keep their room at the 600 px main pane.
    pub const ID_COL_BREAKPOINT: f32 = 720.0;
    /// Dialog widths (confirm dialogs use the small one).
    pub const DIALOG_W: f32 = 420.0;
    pub const DIALOG_W_SM: f32 = 360.0;
    /// Console log panel height.
    pub const LOG_PANEL_H: f32 = 240.0;
    /// Least height of the start/stop log box: six CODE lines plus its padding and border, so a
    /// running progress never shrinks to one line (N4).
    pub const LOG_MIN_H: f32 = 6.0 * 18.0 + 2.0 * 8.0 + 2.0;
    /// Skeleton bar height (CSS .skel) and progress bar height (CSS .bar).
    pub const SKELETON_H: f32 = 10.0;
    pub const BAR_H: f32 = 4.0;
    /// Focus ring and tab indicator width.
    pub const FOCUS_RING: f32 = 2.0;
    /// Switch track and knob (CSS .switch).
    pub const SWITCH_W: f32 = 28.0;
    pub const SWITCH_H: f32 = 16.0;
    pub const SWITCH_KNOB: f32 = 12.0;
    /// Initial window size (CSS .win).
    pub const WINDOW_W: f32 = 1080.0;
    pub const WINDOW_H: f32 = 720.0;
    /// macOS traffic lights: 12 px buttons centred in the sidebar's titlebar strip.
    pub const TRAFFIC_LIGHT_X: f32 = 14.0;
    pub const TRAFFIC_LIGHT_Y: f32 = (TITLEBAR - 12.0) / 2.0;
}

/// Durations and easing. Enter/exit and state changes use ease-out; constant motion is linear.
/// GPUI's `with_animation` renders the end state when the OS asks to reduce motion.
pub mod motion {
    use std::time::Duration;

    /// Hover/press feedback, small fades (scrim).
    pub const FAST: Duration = Duration::from_millis(150);
    /// Dialogs, panels, tab changes.
    pub const BASE: Duration = Duration::from_millis(200);
    /// One full turn of the busy arc (linear, repeating).
    pub const SPIN: Duration = Duration::from_millis(900);

    /// Cubic ease-out: starts fast, settles gently.
    pub fn ease_out(t: f32) -> f32 {
        let inv = 1.0 - t.clamp(0.0, 1.0);
        1.0 - inv * inv * inv
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AA: f64 = 4.5;
    const BOTH: [Appearance; 2] = [Appearance::Light, Appearance::Dark];

    #[test]
    fn contrast_matches_wcag_reference_values() {
        assert!((Rgb(0x000000).contrast(Rgb(0xFFFFFF)) - 21.0).abs() < 1e-9);
        assert!((Rgb(0x777777).contrast(Rgb(0xFFFFFF)) - 4.48).abs() < 0.01);
        assert_eq!(Rgb(0x123456).contrast(Rgb(0x123456)), 1.0);
    }

    #[test]
    fn values_match_the_design_canvas() {
        let l = palette(Appearance::Light);
        let d = palette(Appearance::Dark);
        assert_eq!(
            (l.bg, l.text, l.accent, l.accent_text),
            (Rgb(0xFFFFFF), Rgb(0x161618), Rgb(0x6240E0), Rgb(0x5634D0))
        );
        assert_eq!(
            (d.bg, d.text, d.accent, d.accent_text),
            (Rgb(0x161618), Rgb(0xECECEE), Rgb(0x6D4AEB), Rgb(0xA891FF))
        );
        assert_eq!(
            (l.ok, l.busy, l.err),
            (Rgb(0x12733A), Rgb(0x1F5FD1), Rgb(0xC42B2B))
        );
        assert_eq!(
            (d.ok, d.busy, d.err),
            (Rgb(0x3CC37A), Rgb(0x6AA3FF), Rgb(0xFF6E6E))
        );
    }

    #[test]
    fn body_text_is_readable_on_every_surface() {
        for a in BOTH {
            let p = palette(a);
            for fg in [p.text, p.text_2, p.accent_text] {
                for bg in [
                    p.bg,
                    p.bg_sunken,
                    p.bg_chrome,
                    p.bg_hover,
                    p.bg_raised,
                    p.bg_selected,
                ] {
                    assert!(
                        fg.contrast(bg) >= AA,
                        "{a:?}: {fg:?} on {bg:?} = {:.2}",
                        fg.contrast(bg)
                    );
                }
            }
        }
    }

    #[test]
    fn status_colors_are_readable_on_their_chip_and_on_bg() {
        for a in BOTH {
            let p = palette(a);
            for (fg, chip) in [
                (p.ok, p.ok_bg),
                (p.idle, p.idle_bg),
                (p.busy, p.busy_bg),
                (p.err, p.err_bg),
            ] {
                assert!(fg.contrast(chip) >= AA, "{a:?}: {fg:?} on chip {chip:?}");
                assert!(fg.contrast(p.bg) >= AA, "{a:?}: {fg:?} on bg");
            }
        }
    }

    #[test]
    fn primary_button_label_is_readable() {
        for a in BOTH {
            let p = palette(a);
            assert!(p.on_accent.contrast(p.accent) >= AA, "{a:?}");
        }
    }

    #[test]
    fn button_states_keep_text_readable() {
        for a in BOTH {
            let p = palette(a);
            for bg in [p.accent_hover, p.accent_pressed] {
                assert!(p.on_accent.contrast(bg) >= AA, "{a:?}: on_accent on {bg:?}");
            }
            for bg in [p.bg_raised, p.bg_hover, p.err_bg] {
                assert!(p.err.contrast(bg) >= AA, "{a:?}: danger text on {bg:?}");
            }
            for bg in [p.bg_hover, p.bg_pressed] {
                assert!(p.text.contrast(bg) >= AA, "{a:?}: text on {bg:?}");
                assert!(p.text_2.contrast(bg) >= AA, "{a:?}: ghost text on {bg:?}");
            }
        }
    }

    #[test]
    fn scrim_matches_the_canvas_per_appearance() {
        let l = palette(Appearance::Light).scrim;
        let d = palette(Appearance::Dark).scrim;
        assert_eq!((l.rgb, l.alpha), (Rgb(0x161618), 0.32));
        assert_eq!((d.rgb, d.alpha), (Rgb(0x000000), 0.5));
    }

    #[test]
    fn motion_is_150_to_200ms_and_eases_out() {
        use std::time::Duration;
        assert_eq!(motion::FAST, Duration::from_millis(150));
        assert_eq!(motion::BASE, Duration::from_millis(200));
        assert_eq!((motion::ease_out(0.0), motion::ease_out(1.0)), (0.0, 1.0));
        // Ease-out: ahead of linear in the first half.
        assert!(motion::ease_out(0.25) > 0.25 && motion::ease_out(0.5) > 0.5);
        assert!(motion::SPIN > motion::BASE);
    }

    #[test]
    fn named_sizes_match_the_canvas() {
        use metric::*;
        assert_eq!((DIALOG_W, DIALOG_W_SM, LOG_PANEL_H), (420.0, 360.0, 240.0));
        assert_eq!(
            (KV_LABEL, FIELD_LABEL, FIELD_NUM, FIELD_SELECT),
            (88.0, 96.0, 64.0, 220.0)
        );
        assert_eq!((CHIP_H, BTN, BTN_SM, TAB_H), (22.0, 28.0, 24.0, 36.0));
        assert_eq!(FIELD_SEARCH, 160.0);
        assert_eq!((COL_ID, ID_COL_BREAKPOINT), (88.0, 720.0));
        // The main pane (window minus sidebar and inspector) is below the ID breakpoint.
        const { assert!(WINDOW_W - SIDEBAR - INSPECTOR < ID_COL_BREAKPOINT) };
        assert_eq!((DOT, SKELETON_H, BAR_H, FOCUS_RING), (8.0, 10.0, 4.0, 2.0));
        assert_eq!((SWITCH_W, SWITCH_H, SWITCH_KNOB), (28.0, 16.0, 12.0));
        assert_eq!((WINDOW_W, WINDOW_H), (1080.0, 720.0));
        // Traffic lights (12 px) sit centred in the sidebar's titlebar strip.
        assert_eq!(TRAFFIC_LIGHT_Y, (TITLEBAR - 12.0) / 2.0);
        assert_eq!(TRAFFIC_LIGHT_X, 14.0);
    }

    #[test]
    fn type_scale_has_four_sizes_and_spacing_is_on_the_4pt_grid() {
        use font::*;
        assert_eq!(
            [TITLE.size, BODY.size, SMALL.size, CODE.size],
            [16.0, 13.0, 12.0, 12.0]
        );
        assert_eq!(
            [
                TITLE.line_height,
                BODY.line_height,
                SMALL.line_height,
                CODE.line_height
            ],
            [22.0, 20.0, 16.0, 18.0]
        );
        assert!(metric::SPACE.iter().all(|s| *s > 0.0 && s % 4.0 == 0.0));
        assert_eq!(
            (metric::ROW, metric::SIDEBAR, metric::INSPECTOR),
            (28.0, 220.0, 260.0)
        );
    }
}
