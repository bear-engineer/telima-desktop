//! Status shown by shape and color: filled dot running, hollow ring stopped, spinning arc busy,
//! filled triangle error (H4, L4).
use std::f32::consts::TAU;

use gpui_kit::{
    Animation, AnimationExt, AnyElement, Bounds, ElementId, IntoElement, ParentElement,
    PathBuilder, Pixels, Point, Styled, canvas, div, point, px, size,
};

use crate::style::hsla;
use crate::theme::{Palette, Rgb, metric, motion};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Running,
    Stopped,
    Busy,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Dot,
    Ring,
    Arc,
    Triangle,
}

/// Alpha of the busy ring's track (canvas: 0.3-opacity ring under the arc).
const BUSY_TRACK_ALPHA: f32 = 0.3;

impl Status {
    pub fn shape(self) -> Shape {
        match self {
            Status::Running => Shape::Dot,
            Status::Stopped => Shape::Ring,
            Status::Busy => Shape::Arc,
            Status::Error => Shape::Triangle,
        }
    }

    /// Foreground color (shape and chip text).
    pub fn color(self, p: &Palette) -> Rgb {
        match self {
            Status::Running => p.ok,
            Status::Stopped => p.idle,
            Status::Busy => p.busy,
            Status::Error => p.err,
        }
    }

    /// Chip background.
    pub fn chip_bg(self, p: &Palette) -> Rgb {
        match self {
            Status::Running => p.ok_bg,
            Status::Stopped => p.idle_bg,
            Status::Busy => p.busy_bg,
            Status::Error => p.err_bg,
        }
    }
}

/// Apex, bottom-left, bottom-right of the error triangle inside `b` (an ICON box).
pub fn triangle_points(b: Bounds<Pixels>) -> [Point<Pixels>; 3] {
    let inset = px(metric::ICON_STROKE);
    let (x0, y0) = (b.origin.x, b.origin.y);
    let (w, h) = (b.size.width, b.size.height);
    [
        point(x0 + w / 2., y0 + inset * 2.),
        point(x0 + inset, y0 + h - inset * 2.),
        point(x0 + w - inset, y0 + h - inset * 2.),
    ]
}

/// Start angle (radians) of the busy arc at animation progress `t` in 0..=1.
pub fn arc_start_angle(t: f32) -> f32 {
    t * TAU
}

fn ring_point(c: Point<Pixels>, r: Pixels, angle: f32) -> Point<Pixels> {
    point(c.x + r * angle.cos(), c.y + r * angle.sin())
}

/// Status shape. Dot, ring and arc are `metric::DOT` wide; the triangle fills `metric::ICON`.
pub fn status_shape(id: impl Into<ElementId>, status: Status, p: &Palette) -> AnyElement {
    let color = hsla(status.color(p));
    let dot = px(metric::DOT);
    match status.shape() {
        Shape::Dot => div()
            .size(dot)
            .flex_none()
            .rounded_full()
            .bg(color)
            .into_any_element(),
        Shape::Ring => div()
            .size(dot)
            .flex_none()
            .rounded_full()
            .border(px(metric::ICON_STROKE))
            .border_color(color)
            .into_any_element(),
        Shape::Arc => div()
            .size(dot)
            .flex_none()
            .rounded_full()
            .border(px(metric::ICON_STROKE))
            .border_color(color.opacity(BUSY_TRACK_ALPHA))
            .with_animation(id, Animation::new(motion::SPIN).repeat(), move |el, t| {
                el.child(
                    canvas(
                        |_, _, _| {},
                        move |b, _, window, _| {
                            let stroke = px(metric::ICON_STROKE);
                            let r = b.size.width / 2. - stroke / 2.;
                            let c = b.center();
                            let a0 = arc_start_angle(t);
                            let a1 = a0 + TAU / 4.;
                            let mut path = PathBuilder::stroke(stroke);
                            path.move_to(ring_point(c, r, a0));
                            path.arc_to(point(r, r), px(0.), false, true, ring_point(c, r, a1));
                            if let Ok(path) = path.build() {
                                window.paint_path(path, color);
                            }
                        },
                    )
                    .absolute()
                    .size_full(),
                )
            })
            .into_any_element(),
        Shape::Triangle => div()
            .size(px(metric::ICON))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .child(
                canvas(
                    |_, _, _| {},
                    move |b, _, window, _| {
                        let box_ = Bounds::new(
                            b.center() - point(px(metric::ICON / 2.), px(metric::ICON / 2.)),
                            size(px(metric::ICON), px(metric::ICON)),
                        );
                        let [a, l, r] = triangle_points(box_);
                        let mut path = PathBuilder::fill();
                        path.add_polygon(&[a, l, r], true);
                        if let Ok(path) = path.build() {
                            window.paint_path(path, color);
                        }
                    },
                )
                .size_full(),
            )
            .into_any_element(),
    }
}
