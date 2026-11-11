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

/// Palette that follows the window's current OS appearance.
pub fn palette(window: &Window) -> &'static Palette {
    theme::palette(appearance_of(window.appearance()))
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
    fn hsla_keeps_the_token_color() {
        let white = hsla(Rgb(0xFFFFFF));
        assert_eq!(white.l, 1.0);
        assert_eq!(white.a, 1.0);
    }
}
