//! Line icons drawn as paths in the canvas SVG viewBox (16 units), scaled to
//! `metric::ICON` and stroked at `metric::ICON_STROKE` (CSS `.ic`: fill none).
use std::f32::consts::TAU;

use gpui_kit::{
    AnyElement, Bounds, Hsla, IntoElement, PathBuilder, Pixels, Point, Styled, canvas, point, px,
};

use crate::theme::metric;

/// Units of the canvas SVG viewBox.
pub const BOX: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    /// `M8 3.5v9M3.5 8h9` (프로필 추가).
    Plus,
    /// `M5 3.5v9l7-4.5z` (시작).
    Play,
    /// `rect 4,4 8x8 rx 1.5` (중지).
    Stop,
    /// `M13 8a5 5 0 1 1-1.46-3.54` + `M13 2.5v3h-3` (재시작).
    Restart,
}

/// Polylines of `icon`, in viewBox units (from Main.dc.html).
pub fn strokes(icon: Icon) -> Vec<Vec<(f32, f32)>> {
    match icon {
        Icon::Plus => vec![vec![(8., 3.5), (8., 12.5)], vec![(3.5, 8.), (12.5, 8.)]],
        Icon::Play => vec![vec![(5., 3.5), (5., 12.5), (12., 8.), (5., 3.5)]],
        Icon::Stop => {
            // Rounded square: corners cut at the radius (1.5 units).
            let (lo, hi, r) = (4., 12., 1.5);
            vec![vec![
                (lo + r, lo),
                (hi - r, lo),
                (hi, lo + r),
                (hi, hi - r),
                (hi - r, hi),
                (lo + r, hi),
                (lo, hi - r),
                (lo, lo + r),
                (lo + r, lo),
            ]]
        }
        Icon::Restart => {
            let ring = (0..=24)
                .map(|i| {
                    let a = i as f32 / 24. * TAU * 0.875;
                    (8. + 5. * a.cos(), 8. + 5. * a.sin())
                })
                .collect();
            vec![ring, vec![(13., 2.5), (13., 5.5), (10., 5.5)]]
        }
    }
}

fn at(b: Bounds<Pixels>, (x, y): (f32, f32)) -> Point<Pixels> {
    let s = b.size.width / BOX;
    point(b.origin.x + s * x, b.origin.y + s * y)
}

/// One icon, `metric::ICON` square, stroked in `color`.
pub fn icon(kind: Icon, color: Hsla) -> AnyElement {
    canvas(
        |_, _, _| {},
        move |b, _, window, _| {
            for line in strokes(kind) {
                let mut path = PathBuilder::stroke(px(metric::ICON_STROKE));
                let mut pts = line.into_iter();
                if let Some(first) = pts.next() {
                    path.move_to(at(b, first));
                }
                for p in pts {
                    path.line_to(at(b, p));
                }
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
            }
        },
    )
    .flex_none()
    .size(px(metric::ICON))
    .into_any_element()
}
