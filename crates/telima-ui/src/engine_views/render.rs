//! Rendering of [`EnginePanel`]: 28px tables, list states, confirm dialog, Console log panel.
//! Colors and sizes come only from `theme` through `style`.
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use telima_app::view::{ContainerRow, ImageRow, VolumeRow};
use telima_core::container::{ContainerState, LogStream};

use super::{Dialog, EnginePanel, ListState, LogPanel};
use crate::shell::Tab;
use crate::style::{hsla, palette, space};
use crate::theme::{Palette, font, metric};

/// Dialog items listed before "외 N개".
const DIALOG_ITEMS: usize = 8;

fn text(s: impl Into<SharedString>, color: crate::theme::Rgb) -> Div {
    div()
        .text_size(px(font::BODY.size))
        .text_color(hsla(color))
        .child(s.into())
}

fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    danger: bool,
    p: &Palette,
) -> Stateful<Div> {
    let (bg, fg, hover) = if danger {
        (p.err, p.on_accent, p.err)
    } else {
        (p.bg_raised, p.text, p.bg_hover)
    };
    div()
        .id(id.into())
        .h(px(metric::ROW - 6.))
        .px(space(1))
        .flex()
        .items_center()
        .rounded(px(metric::RADIUS_SM))
        .border_1()
        .border_color(hsla(if danger { p.err_border } else { p.border }))
        .bg(hsla(bg))
        .text_color(hsla(fg))
        .text_size(px(font::SMALL.size))
        .cursor_pointer()
        .hover(move |s| s.bg(hsla(hover)))
        .child(label.into())
}

/// Running: filled dot. Stopped: hollow ring. In progress: arc. Dead: triangle.
fn state_shape(state: ContainerState, p: &Palette) -> Div {
    let size = px(8.);
    let base = div().size(size).flex_none().rounded_full();
    match state {
        ContainerState::Running => base.bg(hsla(p.ok)),
        ContainerState::Restarting => base
            .border_1()
            .border_color(hsla(p.track))
            .border_t_1()
            .border_r_1(),
        ContainerState::Dead => div()
            .flex_none()
            .text_size(size)
            .line_height(size)
            .text_color(hsla(p.err))
            .child("▲"),
        _ => base.border_1().border_color(hsla(p.idle)),
    }
}

fn header(cols: &[(&'static str, f32)], p: &Palette) -> Div {
    h_flex()
        .h(px(metric::ROW))
        .px(space(2))
        .gap(space(2))
        .border_b_1()
        .border_color(hsla(p.border))
        .text_size(px(font::SMALL.size))
        .text_color(hsla(p.text_2))
        .children(
            cols.iter()
                .map(|(label, w)| div().w(relative(*w)).child(*label)),
        )
}

fn row(id: impl Into<ElementId>, p: &Palette) -> Stateful<Div> {
    h_flex()
        .id(id.into())
        .h(px(metric::ROW))
        .px(space(2))
        .gap(space(2))
        .border_b_1()
        .border_color(hsla(p.border))
        .hover(|s| s.bg(hsla(p.bg_hover)))
}

fn cell(w: f32) -> Div {
    div()
        .w(relative(w))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
}

fn mono(s: impl Into<SharedString>, p: &Palette) -> Div {
    div()
        .font_family(font::MONO)
        .text_size(px(font::CODE.size))
        .text_color(hsla(p.text_2))
        .child(s.into())
}

fn centered(p: &Palette) -> Div {
    v_flex()
        .flex_1()
        .items_center()
        .justify_center()
        .gap(space(2))
        .p(space(5))
        .text_color(hsla(p.text_2))
}

impl EnginePanel {
    /// Shared loading / error / empty handling; `Some` means the list has rows to draw.
    fn state_view<'a, T>(
        &self,
        state: &'a ListState<T>,
        empty: &'static str,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Result<&'a [T], AnyElement> {
        match state {
            ListState::Idle => Err(centered(p)
                .child(text("프로필을 선택하세요.", p.text_2))
                .into_any_element()),
            ListState::Loading => Err(v_flex()
                .children((0..4).map(|i| {
                    h_flex().h(px(metric::ROW)).px(space(2)).child(
                        div()
                            .h(px(10.))
                            .w(relative(0.9 - i as f32 * 0.15))
                            .rounded(px(metric::RADIUS_SM))
                            .bg(hsla(p.skeleton)),
                    )
                }))
                .into_any_element()),
            ListState::Failed(msg) => Err(centered(p)
                .child(
                    h_flex()
                        .gap(space(1))
                        .child(div().text_color(hsla(p.err)).child("▲"))
                        .child(text("목록을 불러오지 못했습니다.", p.text)),
                )
                .child(mono(msg.clone(), p))
                .child(
                    button("retry", "다시 시도", false, p)
                        .on_click(cx.listener(|v, _, _, cx| v.retry(cx))),
                )
                .into_any_element()),
            ListState::Ready(rows) if rows.is_empty() => {
                Err(centered(p).child(text(empty, p.text_2)).into_any_element())
            }
            ListState::Ready(rows) => Ok(rows),
        }
    }

    fn containers_table(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let rows: Vec<ContainerRow> =
            match self.state_view(&self.containers, "컨테이너가 없습니다.", p, cx) {
                Ok(rows) => rows.to_vec(),
                Err(el) => return el,
            };
        let body = rows.into_iter().map(|r| {
            let (restart, remove, logs) = (r.id.clone(), r.id.clone(), r.id.clone());
            row(SharedString::from(format!("ctr-{}", r.id)), p)
                .child(
                    cell(0.25).child(
                        h_flex()
                            .gap(space(1))
                            .child(state_shape(r.state, p))
                            .child(text(r.name.clone(), p.text)),
                    ),
                )
                .child(cell(0.25).child(mono(r.image.clone(), p)))
                .child(cell(0.15).child(text(r.state_label, p.text_2)))
                .child(cell(0.2).child(text(r.status_text.clone(), p.text_2)))
                .child(
                    h_flex()
                        .gap(space(0))
                        .flex_none()
                        .child(
                            button(
                                SharedString::from(format!("logs-{}", r.id)),
                                "로그",
                                false,
                                p,
                            )
                            .on_click(cx.listener(move |v, _, _, cx| v.open_logs(&logs, cx))),
                        )
                        .child(
                            button(
                                SharedString::from(format!("restart-{}", r.id)),
                                "재시작",
                                false,
                                p,
                            )
                            .on_click(cx.listener(move |v, _, _, cx| v.restart(&restart, cx))),
                        )
                        .child(
                            button(
                                SharedString::from(format!("remove-{}", r.id)),
                                "삭제",
                                false,
                                p,
                            )
                            .on_click(
                                cx.listener(move |v, _, _, cx| v.request_remove(&remove, cx)),
                            ),
                        ),
                )
        });
        v_flex()
            .child(header(
                &[
                    ("이름", 0.25),
                    ("이미지", 0.25),
                    ("상태", 0.15),
                    ("상태 설명", 0.2),
                    ("", 0.15),
                ],
                p,
            ))
            .children(body)
            .into_any_element()
    }

    fn images_table(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let rows: Vec<ImageRow> = match self.state_view(&self.images, "이미지가 없습니다.", p, cx)
        {
            Ok(rows) => rows.to_vec(),
            Err(el) => return el,
        };
        let body = rows.into_iter().map(|r| {
            row(SharedString::from(format!("img-{}", r.id)), p)
                .child(cell(0.4).child(text(
                    r.tags.clone(),
                    if r.dangling { p.text_2 } else { p.text },
                )))
                .child(cell(0.3).child(mono(r.id.chars().take(12).collect::<String>(), p)))
                .child(cell(0.15).child(mono(r.size.clone(), p)))
                .child(cell(0.15).child(text(r.containers.clone(), p.text_2)))
        });
        v_flex()
            .child(header(
                &[
                    ("태그", 0.4),
                    ("ID", 0.3),
                    ("크기", 0.15),
                    ("컨테이너", 0.15),
                ],
                p,
            ))
            .children(body)
            .into_any_element()
    }

    fn volumes_table(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let rows: Vec<VolumeRow> = match self.state_view(&self.volumes, "볼륨이 없습니다.", p, cx)
        {
            Ok(rows) => rows.to_vec(),
            Err(el) => return el,
        };
        let body = rows.into_iter().map(|r| {
            let (fg, bg) = if r.in_use {
                (p.ok, p.ok_bg)
            } else {
                (p.idle, p.idle_bg)
            };
            row(SharedString::from(format!("vol-{}", r.name)), p)
                .child(cell(0.55).child(text(r.name.clone(), p.text)))
                .child(cell(0.2).child(mono(r.size.clone(), p)))
                .child(
                    cell(0.25).child(
                        div()
                            .px(space(1))
                            .rounded(px(metric::RADIUS_SM))
                            .bg(hsla(bg))
                            .text_size(px(font::SMALL.size))
                            .text_color(hsla(fg))
                            .child(r.in_use_label),
                    ),
                )
        });
        v_flex()
            .child(header(&[("이름", 0.55), ("크기", 0.2), ("사용", 0.25)], p))
            .children(body)
            .into_any_element()
    }

    fn toolbar(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let bar = h_flex()
            .h(px(metric::ROW + 8.))
            .px(space(2))
            .gap(space(1))
            .justify_end()
            .child(
                button("refresh", "새로고침", false, p)
                    .on_click(cx.listener(|v, _, _, cx| v.retry(cx))),
            );
        match self.tab {
            Tab::Containers => bar,
            Tab::Images => bar.child(
                button("prune-images", "미사용 이미지 정리", false, p)
                    .on_click(cx.listener(|v, _, _, cx| v.request_image_prune(cx))),
            ),
            Tab::Volumes => bar.child(
                button("prune-volumes", "미사용 볼륨 정리", false, p)
                    .on_click(cx.listener(|v, _, _, cx| v.request_volume_prune(cx))),
            ),
        }
    }

    fn log_panel(&self, log: &LogPanel, p: &Palette, cx: &mut Context<Self>) -> Div {
        let follow = log.follow;
        let lines = log.lines.iter().map(|l| {
            let color = match l.stream {
                LogStream::Stdout => p.text,
                LogStream::Stderr => p.err,
            };
            div()
                .font_family(font::MONO)
                .text_size(px(font::CODE.size))
                .line_height(px(font::CODE.line_height))
                .text_color(hsla(color))
                .child(l.text.clone())
        });
        v_flex()
            .h(px(240.))
            .flex_none()
            .border_t_1()
            .border_color(hsla(p.border))
            .bg(hsla(p.bg_sunken))
            .child(
                h_flex()
                    .h(px(metric::ROW))
                    .px(space(2))
                    .gap(space(1))
                    .border_b_1()
                    .border_color(hsla(p.border))
                    .child(text(format!("{} 로그", log.name), p.text).flex_1())
                    .child(
                        button(
                            "log-follow",
                            if follow {
                                "따라가기 켬"
                            } else {
                                "따라가기 끔"
                            },
                            false,
                            p,
                        )
                        .when(follow, |b| b.bg(hsla(p.bg_selected)))
                        .on_click(cx.listener(move |v, _, _, cx| v.set_follow(!follow, cx))),
                    )
                    .child(
                        button("log-close", "닫기", false, p)
                            .on_click(cx.listener(|v, _, _, cx| v.close_logs(cx))),
                    ),
            )
            .child(
                v_flex()
                    .id("log-lines")
                    .flex_1()
                    .px(space(2))
                    .py(space(1))
                    .overflow_y_scroll()
                    .when(log.lines.is_empty() && log.error.is_none(), |d| {
                        d.child(text("로그가 없습니다.", p.text_2))
                    })
                    .children(lines)
                    .when_some(log.error.clone(), |d, e| {
                        d.child(mono(e, p).text_color(hsla(p.err)))
                    }),
            )
    }

    fn dialog_view(&self, d: &Dialog, p: &Palette, cx: &mut Context<Self>) -> Div {
        let more = d.items.len().saturating_sub(DIALOG_ITEMS);
        let items = d
            .items
            .iter()
            .take(DIALOG_ITEMS)
            .map(|i| mono(i.clone(), p));
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(hsla(p.bg_chrome).opacity(0.6))
            .child(
                v_flex()
                    .id("dialog")
                    .occlude()
                    .w(px(360.))
                    .p(space(4))
                    .gap(space(2))
                    .rounded(px(metric::RADIUS_LG))
                    .border_1()
                    .border_color(hsla(p.border_strong))
                    .bg(hsla(p.bg_raised))
                    .child(
                        div()
                            .text_size(px(font::TITLE.size))
                            .text_color(hsla(p.text))
                            .child(d.confirm.title.clone()),
                    )
                    .child(text(d.confirm.body.clone(), p.text_2))
                    .when(!d.items.is_empty(), |c| {
                        c.child(
                            v_flex()
                                .p(space(1))
                                .rounded(px(metric::RADIUS_SM))
                                .bg(hsla(p.bg_sunken))
                                .children(items)
                                .when(more > 0, |l| {
                                    l.child(text(format!("외 {more}개"), p.text_2))
                                }),
                        )
                    })
                    .child(
                        h_flex()
                            .gap(space(1))
                            .justify_end()
                            .child(
                                button("dialog-cancel", "취소", false, p)
                                    .on_click(cx.listener(|v, _, _, cx| v.dismiss(cx))),
                            )
                            .when(d.confirm.can_confirm, |b| {
                                b.child(
                                    button("dialog-ok", d.confirm.confirm_label.clone(), true, p)
                                        .on_click(cx.listener(|v, _, _, cx| v.accept(cx))),
                                )
                            }),
                    ),
            )
    }
}

impl Render for EnginePanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(window);
        let table = match self.tab {
            Tab::Containers => self.containers_table(p, cx),
            Tab::Images => self.images_table(p, cx),
            Tab::Volumes => self.volumes_table(p, cx),
        };
        let log = self.log.clone();
        let dialog = self.dialog.clone();
        v_flex()
            .relative()
            .size_full()
            .bg(hsla(p.bg))
            .child(self.toolbar(p, cx))
            .when_some(self.notice.clone(), |d, n| {
                d.child(
                    div()
                        .mx(space(2))
                        .px(space(2))
                        .py(space(1))
                        .rounded(px(metric::RADIUS_SM))
                        .border_1()
                        .border_color(hsla(p.err_border))
                        .bg(hsla(p.err_bg))
                        .child(text(n, p.err)),
                )
            })
            .child(
                div()
                    .id("engine-table")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(table),
            )
            .when_some(log, |d, log| d.child(self.log_panel(&log, p, cx)))
            .when_some(dialog, |d, dlg| d.child(self.dialog_view(&dlg, p, cx)))
    }
}
