use gpui_kit::{
    Bounds, Context, IntoElement, Modifiers, ParentElement, Render, Styled, TestAppContext, Window,
    div, point, px, size,
};

use super::button::{ButtonVariant, button, button_colors, button_disabled, icon_button};
use super::chip::status_chip;
use super::dialog::{dialog_frame, modal, scrim};
use super::segmented::segmented;
use super::skeleton::skeleton_row;
use super::status::{Shape, Status, arc_start_angle, status_shape, triangle_points};
use super::switch::{knob_offset, switch};
use crate::theme::{self, Appearance, metric};

const BOTH: [Appearance; 2] = [Appearance::Light, Appearance::Dark];

#[test]
fn every_status_has_its_own_shape_and_color() {
    let p = theme::palette(Appearance::Light);
    assert_eq!(Status::Running.shape(), Shape::Dot);
    assert_eq!(Status::Stopped.shape(), Shape::Ring);
    assert_eq!(Status::Busy.shape(), Shape::Arc);
    assert_eq!(Status::Error.shape(), Shape::Triangle);
    assert_eq!(Status::Busy.color(p), p.busy);
    assert_eq!(Status::Error.chip_bg(p), p.err_bg);
}

#[test]
fn error_triangle_fills_the_icon_box() {
    let b = Bounds::new(
        point(px(0.), px(0.)),
        size(px(metric::ICON), px(metric::ICON)),
    );
    let [apex, left, right] = triangle_points(b);
    assert_eq!(apex.x, px(metric::ICON / 2.));
    assert!(apex.y < left.y && left.y == right.y);
    assert!(left.x < right.x && right.x <= px(metric::ICON));
}

#[test]
fn busy_arc_turns_once_per_cycle() {
    use std::f32::consts::TAU;
    assert_eq!(arc_start_angle(0.0), 0.0);
    assert!((arc_start_angle(0.5) - TAU / 2.0).abs() < 1e-6);
}

#[test]
fn button_colors_follow_the_canvas() {
    for a in BOTH {
        let p = theme::palette(a);
        let d = button_colors(ButtonVariant::Default, p);
        assert_eq!(
            (d.bg, d.border, d.hover, d.text),
            (Some(p.bg_raised), Some(p.border_strong), p.bg_hover, p.text)
        );
        let pr = button_colors(ButtonVariant::Primary, p);
        assert_eq!(
            (pr.bg, pr.hover, pr.pressed, pr.text),
            (
                Some(p.accent),
                p.accent_hover,
                p.accent_pressed,
                p.on_accent
            )
        );
        let dg = button_colors(ButtonVariant::Danger, p);
        assert_eq!(
            (dg.bg, dg.border, dg.text, dg.pressed),
            (Some(p.bg_raised), Some(p.err_border), p.err, p.err_bg)
        );
        let g = button_colors(ButtonVariant::Ghost, p);
        assert_eq!((g.bg, g.border, g.text), (None, None, p.text_2));
        for v in [
            ButtonVariant::Default,
            ButtonVariant::Primary,
            ButtonVariant::Danger,
            ButtonVariant::Ghost,
        ] {
            let c = button_colors(v, p);
            for bg in [c.hover, c.pressed] {
                assert!(
                    c.text.contrast(bg) >= 4.5,
                    "{a:?} {v:?}: {:?} on {bg:?}",
                    c.text
                );
            }
        }
    }
}

#[test]
fn switch_knob_slides_from_left_to_right() {
    assert_eq!(knob_offset(false), 2.0);
    assert_eq!(
        knob_offset(true),
        metric::SWITCH_W - metric::SWITCH_KNOB - 2.0
    );
}

struct Harness {
    on: bool,
    selected: usize,
}

impl Render for Harness {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::style::palette(window);
        let entity = cx.entity();
        let e2 = entity.clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(switch("sw", self.on, p, move |on, _, cx| {
                entity.update(cx, |h, cx| {
                    h.on = on;
                    cx.notify();
                })
            }))
            .child(segmented(
                "seg",
                vec!["vz".into(), "qemu".into(), "krunkit".into()],
                self.selected,
                p,
                move |i, _, cx| {
                    e2.update(cx, |h, cx| {
                        h.selected = i;
                        cx.notify();
                    })
                },
            ))
            .children(
                [
                    Status::Running,
                    Status::Stopped,
                    Status::Busy,
                    Status::Error,
                ]
                .into_iter()
                .enumerate()
                .map(|(i, s)| status_shape(("st", i), s, p)),
            )
            .child(status_chip("chip", Status::Busy, "시작 중", p))
            .child(button("b1", "만들기", ButtonVariant::Primary, p))
            .child(button("b2", "삭제", ButtonVariant::Danger, p))
            .child(button_disabled("b3", "중지", p))
            .child(icon_button("b4", div().child("+"), p))
            .child(skeleton_row(&[0.4, 0.2], p))
            .child(dialog_frame(metric::DIALOG_W, p).child("본문"))
            .child(scrim(p))
            .child(modal("m", metric::DIALOG_W_SM, p, div().child("확인")))
    }
}

#[gpui_kit::test]
fn switch_toggles_on_click(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, _| Harness {
        on: false,
        selected: 0,
    });
    cx.run_until_parked();
    let b = cx.debug_bounds("switch-sw").expect("switch rendered");
    cx.simulate_click(b.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(view.read_with(cx, |h, _| h.on));
}

#[gpui_kit::test]
fn segmented_selects_the_clicked_option(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, _| Harness {
        on: false,
        selected: 0,
    });
    cx.run_until_parked();
    let b = cx.debug_bounds("segment-seg-2").expect("segment rendered");
    cx.simulate_click(b.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |h, _| h.selected), 2);
}

#[gpui_kit::test]
fn init_registers_bundled_fonts_without_panicking(cx: &mut TestAppContext) {
    cx.update(crate::init);
    assert_eq!(crate::fonts::FONTS.len(), 5);
    for (name, bytes) in crate::fonts::FONTS {
        assert_eq!(&bytes[..4], &[0, 1, 0, 0], "{name} is a TrueType file");
    }
}

#[test]
fn paragraph_splits_on_spaces_and_keeps_korean_words_whole() {
    use super::paragraph::eojeols;
    let words = eojeols("VM 디스크와 컨테이너, 볼륨이 함께 지워집니다.  되돌릴 수 없습니다.");
    assert_eq!(
        words,
        [
            "VM",
            "디스크와",
            "컨테이너,",
            "볼륨이",
            "함께",
            "지워집니다.",
            "되돌릴",
            "수",
            "없습니다."
        ]
    );
    assert!(eojeols("  ").is_empty());
}

#[test]
fn every_icon_has_strokes_inside_the_box() {
    use super::icons::{BOX, Icon, strokes};
    for i in [Icon::Plus, Icon::Play, Icon::Stop, Icon::Restart] {
        let lines = strokes(i);
        assert!(!lines.is_empty(), "{i:?}");
        for (x, y) in lines.into_iter().flatten() {
            assert!((0.0..=BOX).contains(&x) && (0.0..=BOX).contains(&y), "{i:?}");
        }
    }
}
