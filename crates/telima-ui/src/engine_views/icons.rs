//! Line icons of the engine panel drawn as paths in a 16-unit box (the canvas SVG viewBox),
//! scaled to `metric::ICON`. Strokes use `metric::ICON_STROKE`.
use std::f32::consts::TAU;

use gpui_kit::{
    AnyElement, Bounds, Hsla, IntoElement, PathBuilder, Pixels, Point, Styled, canvas, point, px,
};

use crate::theme::metric;

/// Units of the canvas SVG viewBox.
const BOX: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    More,
    Trash,
    Refresh,
    Close,
    Filter,
    Terminal,
    Follow,
    Start,
    Stop,
    Search,
}

type Seg = &'static [(f32, f32)];

/// Polylines of each stroked icon, in viewBox units (from Containers.dc.html).
fn strokes(icon: Icon) -> Vec<Vec<(f32, f32)>> {
    let lines = |v: &[Seg]| v.iter().map(|s| s.to_vec()).collect::<Vec<_>>();
    match icon {
        Icon::Trash => lines(&[
            &[(3., 4.5), (13., 4.5)],
            &[(6.5, 4.5), (6.5, 3.), (9.5, 3.), (9.5, 4.5)],
            &[(4.5, 4.5), (5., 13.), (11., 13.), (11.5, 4.5)],
        ]),
        Icon::Close => lines(&[&[(4.5, 4.5), (11.5, 11.5)], &[(11.5, 4.5), (4.5, 11.5)]]),
        Icon::Filter => lines(&[
            &[(3., 4.), (13., 4.)],
            &[(5., 8.), (11., 8.)],
            &[(7., 12.), (9., 12.)],
        ]),
        Icon::Terminal => lines(&[
            &[
                (4., 3.),
                (12., 3.),
                (13.5, 4.5),
                (13.5, 11.5),
                (12., 13.),
                (4., 13.),
                (2.5, 11.5),
                (2.5, 4.5),
                (4., 3.),
            ],
            &[(5., 6.5), (7., 8.), (5., 9.5)],
            &[(8.5, 10.), (11., 10.)],
        ]),
        Icon::Start => lines(&[&[(5., 3.5), (12.5, 8.), (5., 12.5), (5., 3.5)]]),
        Icon::Stop => lines(&[&[(4., 4.), (12., 4.), (12., 12.), (4., 12.), (4., 4.)]]),
        Icon::Search => {
            let ring = (0..=24)
                .map(|i| {
                    let a = i as f32 / 24. * TAU;
                    (7. + 3.75 * a.cos(), 7. + 3.75 * a.sin())
                })
                .collect();
            vec![ring, vec![(10., 10.), (13., 13.)]]
        }
        Icon::Follow => lines(&[
            &[(8., 3.), (8., 11.)],
            &[(4.5, 7.5), (8., 11.), (11.5, 7.5)],
            &[(3.5, 13.), (12.5, 13.)],
        ]),
        Icon::Refresh => {
            // Three-quarter ring from the canvas path `M13 8a5 5 0 1 1-1.46-3.54`, plus its arrow head.
            let ring = (0..=24)
                .map(|i| {
                    let a = i as f32 / 24. * TAU * 0.875;
                    (8. + 5. * a.cos(), 8. + 5. * a.sin())
                })
                .collect();
            vec![ring, vec![(13., 2.5), (13., 5.5), (10., 5.5)]]
        }
        Icon::More => Vec::new(),
    }
}

fn at(b: Bounds<Pixels>, (x, y): (f32, f32)) -> Point<Pixels> {
    let s = b.size.width / BOX;
    point(b.origin.x + s * x, b.origin.y + s * y)
}

/// One icon, `metric::ICON` square, in `color`.
pub fn icon(kind: Icon, color: Hsla) -> AnyElement {
    canvas(
        |_, _, _| {},
        move |b, _, window, _| match kind {
            Icon::More => {
                for x in [4., 8., 12.] {
                    let c = at(b, (x, 8.));
                    let r = b.size.width / BOX;
                    let mut path = PathBuilder::fill();
                    let n = 12;
                    let pts: Vec<_> = (0..n)
                        .map(|i| {
                            let a = i as f32 / n as f32 * TAU;
                            point(c.x + r * a.cos(), c.y + r * a.sin())
                        })
                        .collect();
                    path.add_polygon(&pts, true);
                    if let Ok(path) = path.build() {
                        window.paint_path(path, color);
                    }
                }
            }
            _ => {
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
            }
        },
    )
    .flex_none()
    .size(px(metric::ICON))
    .into_any_element()
}
