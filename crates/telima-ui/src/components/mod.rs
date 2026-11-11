//! Shared widgets built only from `theme` tokens. Views compose these; they never restyle them.
//!
//! - [`status`]: status shape (dot, ring, busy arc, error triangle)
//! - [`chip`]: status chip
//! - [`button`]: default / primary / danger (outline) / ghost / icon buttons
//! - [`segmented`], [`switch`]: choice controls
//! - [`dialog`]: scrim and dialog frame with a fade-in
//! - [`skeleton`]: loading row
//! - [`paragraph`]: Korean-safe text that wraps only between words (eojeol)
//! - [`icons`]: line icons (plus, play, stop, restart)
//! - [`banner`]: inline error / warning notice
//! - [`clock`]: wall clock for uptime and relative times
pub mod banner;
pub mod button;
pub mod chip;
pub mod clock;
pub mod dialog;
pub mod icons;
pub mod paragraph;
pub mod segmented;
pub mod skeleton;
pub mod status;
pub mod switch;

use gpui_kit::Hsla;

use crate::style::hsla;
use crate::theme::Rgba;

/// GPUI color for a token color with alpha.
pub fn hsla_alpha(color: Rgba) -> Hsla {
    hsla(color.rgb).opacity(color.alpha)
}

#[cfg(test)]
mod tests;
