//! Design tokens: the single source of truth for colors, type and metrics in the app.
//! Values come from the "telima Instrument v0" design (canvas artboard 06, telima.css).
//! Views read these tokens; they never hard-code colors or sizes.

/// A 24-bit sRGB color, `0xRRGGBB`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u32);

impl Rgb {
    /// WCAG 2.x relative luminance.
    pub fn luminance(self) -> f64 {
        todo!()
    }

    /// WCAG 2.x contrast ratio between two colors (1.0 to 21.0).
    pub fn contrast(self, other: Rgb) -> f64 {
        let _ = other;
        todo!()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    Light,
    Dark,
}

/// Semantic colors for one appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub bg: Rgb,
    pub bg_sunken: Rgb,
    pub bg_chrome: Rgb,
    pub bg_hover: Rgb,
    pub bg_raised: Rgb,
    pub bg_selected: Rgb,
    pub border: Rgb,
    pub border_strong: Rgb,
    pub text: Rgb,
    pub text_2: Rgb,
    pub text_disabled: Rgb,
    pub accent: Rgb,
    pub accent_text: Rgb,
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
}

/// Palette for `appearance`.
pub fn palette(appearance: Appearance) -> &'static Palette {
    let _ = appearance;
    todo!()
}

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
        size: 0.0,
        line_height: 0.0,
    };
    pub const BODY: TypeStyle = TypeStyle {
        size: 0.0,
        line_height: 0.0,
    };
    pub const SMALL: TypeStyle = TypeStyle {
        size: 0.0,
        line_height: 0.0,
    };
    pub const CODE: TypeStyle = TypeStyle {
        size: 0.0,
        line_height: 0.0,
    };
}

/// Spacing, radii and fixed sizes in px.
pub mod metric {
    /// 4pt spacing scale: SPACE[n] = step n.
    pub const SPACE: [f32; 7] = [0.0; 7];
    pub const RADIUS_SM: f32 = 0.0;
    pub const RADIUS_MD: f32 = 0.0;
    pub const RADIUS_LG: f32 = 0.0;
    pub const RADIUS_WINDOW: f32 = 0.0;
    pub const ROW: f32 = 0.0;
    pub const TITLEBAR: f32 = 0.0;
    pub const SIDEBAR: f32 = 0.0;
    pub const INSPECTOR: f32 = 0.0;
    pub const ICON: f32 = 0.0;
    pub const ICON_STROKE: f32 = 0.0;
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
