//! Bridge from design tokens (`theme`) to GPUI values. Views read colors only through here.
use gpui_kit::{Hsla, Pixels, Window, WindowAppearance, px, rgb};

use crate::theme::{self, Appearance, Palette, Rgb};

/// GPUI color for a token color.
pub fn hsla(color: Rgb) -> Hsla {
    rgb(color.0).into()
}

/// Token appearance for the OS appearance GPUI reports.
pub fn appearance_of(os: WindowAppearance) -> Appearance {
    match os {
        WindowAppearance::Dark | WindowAppearance::VibrantDark => Appearance::Dark,
        WindowAppearance::Light | WindowAppearance::VibrantLight => Appearance::Light,
    }
}

/// Parse `TELIMA_APPEARANCE` (`light` or `dark`, any case, surrounding spaces ignored).
pub fn parse_appearance(raw: &str) -> Option<Appearance> {
    let _ = raw;
    None
}

/// Appearance to use: the override only counts in debug builds (`debug`) and when it parses.
pub fn resolve_appearance(debug: bool, raw: Option<&str>, os: Appearance) -> Appearance {
    let _ = (debug, raw);
    os
}

/// Palette that follows the window's OS appearance. Debug builds honour `TELIMA_APPEARANCE`.
pub fn palette(window: &Window) -> &'static Palette {
    static FORCED: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    let forced = FORCED.get_or_init(|| std::env::var("TELIMA_APPEARANCE").ok());
    let os = appearance_of(window.appearance());
    theme::palette(resolve_appearance(
        cfg!(debug_assertions),
        forced.as_deref(),
        os,
    ))
}

/// Spacing step `i` of the 4pt scale (`theme::metric::SPACE`).
pub fn space(i: usize) -> Pixels {
    px(theme::metric::SPACE[i])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_appearance_maps_to_token_appearance() {
        assert_eq!(
            appearance_of(WindowAppearance::VibrantDark),
            Appearance::Dark
        );
        assert_eq!(appearance_of(WindowAppearance::Light), Appearance::Light);
    }

    #[test]
    fn parses_light_and_dark_loosely() {
        assert_eq!(parse_appearance("light"), Some(Appearance::Light));
        assert_eq!(parse_appearance(" Dark\n"), Some(Appearance::Dark));
        assert_eq!(parse_appearance("DARK"), Some(Appearance::Dark));
    }

    #[test]
    fn rejects_unknown_appearance() {
        assert_eq!(parse_appearance(""), None);
        assert_eq!(parse_appearance("auto"), None);
        assert_eq!(parse_appearance("darker"), None);
    }

    #[test]
    fn override_wins_only_in_debug() {
        let os = Appearance::Dark;
        assert_eq!(
            resolve_appearance(true, Some("light"), os),
            Appearance::Light
        );
        assert_eq!(resolve_appearance(false, Some("light"), os), os);
    }

    #[test]
    fn bad_or_missing_override_follows_the_os() {
        let os = Appearance::Light;
        assert_eq!(resolve_appearance(true, Some("x"), os), os);
        assert_eq!(resolve_appearance(true, None, os), os);
    }

    #[test]
    fn hsla_keeps_the_token_color() {
        let white = hsla(Rgb(0xFFFFFF));
        assert_eq!(white.l, 1.0);
        assert_eq!(white.a, 1.0);
    }
}
