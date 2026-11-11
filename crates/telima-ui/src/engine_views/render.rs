//! Rendering of [`EnginePanel`] after Containers.dc.html: toolbar (filter, status segments,
//! bulk remove, refresh), 28px tables with hover/selection icon actions and a ⋯ menu, list
//! states, confirm dialog and the Console log panel. Only theme tokens and shared components.
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use telima_app::view::notice::NoticeKind;
use telima_app::view::{ContainerRow, ImageRow, VolumeRow, short_image_id};

use super::icons::{Icon, icon};
use super::model::{
    LineTone, StatusFilter, container_columns, container_status, log_count_text, port_cell,
    row_actions, volume_chip,
};
use super::{Dialog, EnginePanel, ListState, LogPanel, copy};
use crate::components::button::{ButtonVariant, button, icon_button};
use crate::components::chip::status_chip;
use crate::components::dialog::modal;
use crate::components::segmented::segmented;
use crate::components::skeleton::skeleton_row;
use crate::components::status::{Status, status_shape};
use crate::components::switch::switch;
use crate::shell::Tab;
use crate::style::{hsla, palette, space};
use crate::theme::{Palette, Rgb, font, metric, motion};

/// Dialog items listed before "외 N개".
const DIALOG_ITEMS: usize = 8;
/// Skeleton bars of the loading state, as fractions of the row.
const SKELETON: [f32; 4] = [0.9, 0.75, 0.6, 0.45];
/// Group name for row hover reveal.
const ROW_GROUP: &str = "engine-row";

fn text(s: impl Into<SharedString>, color: Rgb) -> Div {
    div()
        .text_size(px(font::BODY.size))
        .line_height(px(font::BODY.line_height))
        .text_color(hsla(color))
        .child(s.into())
}

fn small(s: impl Into<SharedString>, color: Rgb) -> Div {
    div()
        .text_size(px(font::SMALL.size))
        .line_height(px(font::SMALL.line_height))
        .text_color(hsla(color))
        .child(s.into())
}

fn mono(s: impl Into<SharedString>, color: Rgb) -> Div {
    div()
        .font_family(font::MONO)
        .text_size(px(font::CODE.size))
        .line_height(px(font::CODE.line_height))
        .text_color(hsla(color))
        .child(s.into())
}

/// Ellipsis cell (CSS .ell): grows by its share of the width left after the fixed lead and
/// actions slots, clips with "…" (M2, N1).
fn cell(share: f32) -> Div {
    div()
        .flex_basis(px(0.))
        .flex_grow(share)
        .flex_shrink_0()
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
}

/// A column cell: share 0 is the fixed short-ID width, any other share grows like [`cell`].
fn sized(share: f32) -> Div {
    if share > 0. {
        cell(share)
    } else {
        cell(0.).flex_none().w(px(metric::COL_ID))
    }
}

/// Indices of every column, for tables that never hide one.
fn all_cols(cols: &[(&str, f32)]) -> Vec<usize> {
    (0..cols.len()).collect()
}

/// Table header over the `shown` columns (indices keep their selectors stable); `actions` reserves the row actions slot so columns line up with the rows.
fn header(
    key: &'static str,
    cols: &[(&'static str, f32)],
    shown: &[usize],
    lead: bool,
    actions: bool,
    p: &Palette,
) -> Div {
    h_flex()
        .h(px(metric::ROW))
        .px(space(1))
        .gap(space(1))
        .border_t_1()
        .border_b_1()
        .border_color(hsla(p.border))
        .text_size(px(font::SMALL.size))
        .text_color(hsla(p.text_2))
        .when(lead, |h| h.child(div().w(px(metric::ICON)).flex_none()))
        .children(shown.iter().map(|&i| {
            sized(cols[i].1)
                .debug_selector(move || format!("{key}-header-{i}"))
                .child(cols[i].0)
        }))
        .when(actions, |h| {
            h.child(actions_slot().debug_selector(move || format!("{key}-header-actions")))
        })
}

/// The row actions slot: a fixed width for two icon buttons, so the data columns can never
/// squeeze it out of the row (N1).
fn actions_slot() -> Div {
    h_flex().flex_none().w(px(metric::BTN_SM * 2.0) + space(0))
}

fn row(id: impl Into<ElementId>, selected: bool, p: &Palette) -> Stateful<Div> {
    let hover = hsla(p.bg_hover);
    h_flex()
        .id(id.into())
        .group(ROW_GROUP)
        .h(px(metric::ROW))
        .px(space(1))
        .gap(space(1))
        .border_b_1()
        .border_color(hsla(p.border))
        .when(selected, |r| r.bg(hsla(p.bg_selected)))
        .when(!selected, |r| r.hover(move |s| s.bg(hover)))
}

/// 24px ghost icon button with an accessible label in its tooltip-less id.
fn row_icon(id: impl Into<ElementId>, kind: Icon, p: &Palette) -> Stateful<Div> {
    icon_button(id, icon(kind, hsla(p.text)), p)
        .w(px(metric::BTN_SM))
        .h(px(metric::BTN_SM))
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

fn status_color(status: Status, p: &Palette) -> Rgb {
    match status {
        Status::Running => p.text,
        Status::Stopped => p.text_2,
        Status::Busy => p.busy,
        Status::Error => p.err,
    }
}

impl EnginePanel {
    /// Shared loading / error / empty handling; `Ok` means the list has rows to draw.
    fn state_view<'a, T>(
        &self,
        state: &'a ListState<T>,
        empty: &'static str,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Result<&'a [T], AnyElement> {
        match state {
            ListState::Idle => Err(centered(p)
                .child(text(copy::NO_PROFILE, p.text_2))
                .into_any_element()),
            ListState::Loading => Err(v_flex()
                .px(space(4))
                .children(SKELETON.iter().map(|w| skeleton_row(&[*w], p)))
                .into_any_element()),
            ListState::Failed(msg) => Err(centered(p)
                .child(
                    h_flex()
                        .gap(space(1))
                        .child(status_shape("list-failed", Status::Error, p))
                        .child(text(copy::LOAD_FAILED, p.text)),
                )
                .child(text(msg.clone(), p.text_2))
                .child(
                    button("retry", copy::RETRY, ButtonVariant::Default, p)
                        .on_click(cx.listener(|v, _, _, cx| v.retry(cx))),
                )
                .into_any_element()),
            ListState::Ready(rows) if rows.is_empty() => {
                Err(centered(p).child(text(empty, p.text_2)).into_any_element())
            }
            ListState::Ready(rows) => Ok(rows),
        }
    }

    fn row_menu(&self, r: &ContainerRow, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let actions = row_actions(r.state);
        let item = |id: String, label: &'static str, danger: bool| {
            let hover = hsla(p.bg_hover);
            div()
                .id(SharedString::from(id))
                .h(px(metric::ROW))
                .px(space(1))
                .flex()
                .items_center()
                .rounded(px(metric::RADIUS_SM))
                .text_size(px(font::BODY.size))
                .text_color(hsla(if danger { p.err } else { p.text }))
                .hover(move |s| s.bg(hover))
                .child(label)
        };
        let (logs, restart, remove) = (r.id.clone(), r.id.clone(), r.id.clone());
        deferred(
            // The menu is a child of the ⋯ button's wrapper, so its Local origin already sits
            // at the button's bottom edge: x = button width puts the menu's top-right at the
            // button's right edge, y = 0 keeps its top edge at the button's bottom (L6).
            anchored()
                .position_mode(AnchoredPositionMode::Local)
                .position(point(px(metric::BTN_SM), px(0.)))
                .anchor(Anchor::TopRight)
                .child(
                    v_flex()
                        .id("row-menu")
                        .debug_selector(|| "row-menu".into())
                        .occlude()
                        .p(space(0))
                        .rounded(px(metric::RADIUS_MD))
                        .border_1()
                        .border_color(hsla(p.border_strong))
                        .bg(hsla(p.bg_raised))
                        .shadow_lg()
                        .track_focus(&self.menu_focus)
                        .on_key_down(cx.listener(|v, ev: &KeyDownEvent, _, cx| {
                            if ev.keystroke.key == "escape" {
                                cx.stop_propagation();
                                v.close_menu(cx);
                            }
                        }))
                        .on_mouse_down_out(cx.listener(|v, _, _, cx| v.close_menu(cx)))
                        .child({
                            let key = r.id.clone();
                            item(format!("menu-logs-{}", r.id), copy::MENU_LOGS, false)
                                .debug_selector(move || format!("menu-logs-{key}"))
                                .on_click(cx.listener(move |v, _, window, cx| {
                                    v.open_logs(&logs, cx);
                                    // Focus the panel so Escape closes it (N6).
                                    window.focus(&v.log_focus, cx);
                                }))
                        })
                        .when(actions.restart, |m| {
                            m.child(
                                item(format!("menu-restart-{}", r.id), copy::MENU_RESTART, false)
                                    .on_click(cx.listener(move |v, _, _, cx| {
                                        v.close_menu(cx);
                                        v.restart(&restart, cx)
                                    })),
                            )
                        })
                        .child(
                            item(format!("menu-remove-{}", r.id), copy::MENU_REMOVE, true)
                                .on_click(cx.listener(move |v, _, _, cx| {
                                    v.close_menu(cx);
                                    v.request_remove(&remove, cx)
                                })),
                        ),
                ),
        )
        .into_any_element()
    }

    /// Icon actions of a row: revealed on hover or when the row is selected (M3).
    fn row_actions_view(
        &self,
        r: &ContainerRow,
        selected: bool,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let (start, stop, more) = (r.id.clone(), r.id.clone(), r.id.clone());
        let open = self.menu.as_deref() == Some(r.id.as_str());
        let key = r.id.clone();
        actions_slot()
            .debug_selector(move || format!("ctr-actions-{key}"))
            .justify_end()
            .gap(space(0))
            .when(!(selected || open), |a| {
                a.invisible().group_hover(ROW_GROUP, |s| s.visible())
            })
            // The state-valid lifecycle action (M3): 시작 or 중지, never both; 재시작 is in ⋯.
            .when(r.can_start, |a| {
                let key = r.id.clone();
                a.child(
                    row_icon(
                        SharedString::from(format!("start-{}", r.id)),
                        Icon::Start,
                        p,
                    )
                    .debug_selector(move || format!("ctr-start-{key}"))
                    .on_click(cx.listener(move |v, _, _, cx| v.start(&start, cx))),
                )
            })
            .when(r.can_stop, |a| {
                let key = r.id.clone();
                a.child(
                    row_icon(SharedString::from(format!("stop-{}", r.id)), Icon::Stop, p)
                        .debug_selector(move || format!("ctr-stop-{key}"))
                        .on_click(cx.listener(move |v, _, _, cx| v.stop(&stop, cx))),
                )
            })
            .child(
                div()
                    .child(
                        row_icon(SharedString::from(format!("more-{}", r.id)), Icon::More, p)
                            .debug_selector({
                                let key = r.id.clone();
                                move || format!("ctr-more-{key}")
                            })
                            .on_click(cx.listener(move |v, _, window, cx| {
                                v.toggle_menu(&more, cx);
                                // Focus the menu so Escape closes it.
                                window.focus(&v.menu_focus, cx);
                            })),
                    )
                    .when(open, |d| d.child(self.row_menu(r, p, cx))),
            )
    }

    fn containers_table(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        if let Err(el) = self.state_view(&self.containers, copy::EMPTY_CONTAINERS, p, cx) {
            return el;
        }
        let rows = self.visible_containers();
        if rows.is_empty() {
            return centered(p)
                .child(text(copy::EMPTY_FILTERED, p.text_2))
                .into_any_element();
        }
        let cols = copy::CONTAINER_COLS;
        let shown = container_columns(self.width);
        let show_id = shown.contains(&copy::CONTAINER_ID_COL);
        let body = rows.iter().map(|r| {
            let selected = self.selected.as_deref() == Some(r.id.as_str());
            let status = container_status(r);
            let select = r.id.clone();
            let (key, id) = (r.id.clone(), r.id.clone());
            let col = move |i: usize| {
                let id = id.clone();
                sized(cols[i].1).debug_selector(move || format!("ctr-cell-{id}-{i}"))
            };
            row(SharedString::from(format!("ctr-{}", r.id)), selected, p)
                .debug_selector(move || format!("ctr-row-{key}"))
                .on_click(cx.listener(move |v, _, _, cx| v.select(&select, cx)))
                .child(
                    div()
                        .w(px(metric::ICON))
                        .flex_none()
                        .flex()
                        .justify_center()
                        .child(status_shape(
                            SharedString::from(format!("ctr-shape-{}", r.id)),
                            status,
                            p,
                        )),
                )
                .child(
                    col(0)
                        .text_size(px(font::BODY.size))
                        .text_color(hsla(p.text))
                        .when(selected, |c| c.font_weight(FontWeight::MEDIUM))
                        .child({
                            // Below the ID breakpoint the short ID moves into this tooltip.
                            let short = SharedString::from(short_image_id(&r.id));
                            let name = r.id.clone();
                            div()
                                .id(SharedString::from(format!("ctr-name-{}", r.id)))
                                .debug_selector(move || format!("ctr-name-{name}"))
                                .when(!show_id, |c| {
                                    c.tooltip(move |window, cx| {
                                        gpui_kit::component::tooltip::Tooltip::new(short.clone())
                                            .build(window, cx)
                                    })
                                })
                                .child(r.name.clone())
                        }),
                )
                .when(show_id, |row| {
                    row.child(
                        col(copy::CONTAINER_ID_COL).child(mono(short_image_id(&r.id), p.text_2)),
                    )
                })
                .child(col(2).child(mono(r.image.clone(), p.text)))
                .child(
                    col(3)
                        .text_size(px(font::BODY.size))
                        .text_color(hsla(status_color(status, p)))
                        .when(status == Status::Error, |c| {
                            c.font_weight(FontWeight::MEDIUM)
                        })
                        .child(r.status_text.clone()),
                )
                .child({
                    let full = SharedString::from(r.ports.clone());
                    col(4)
                        .id(SharedString::from(format!("ctr-ports-{}", r.id)))
                        .when(full.contains(", "), |c| {
                            c.tooltip(move |window, cx| {
                                gpui_kit::component::tooltip::Tooltip::new(full.clone())
                                    .build(window, cx)
                            })
                        })
                        .child(mono(port_cell(&r.ports), p.text_2))
                })
                .child(col(5).child(text(r.started.clone(), p.text_2)))
                .child(self.row_actions_view(r, selected, p, cx))
        });
        v_flex()
            .px(space(4))
            .child(header("ctr", &cols, &shown, true, true, p))
            .children(body)
            .into_any_element()
    }

    fn images_table(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let rows: Vec<ImageRow> = match self.state_view(&self.images, copy::EMPTY_IMAGES, p, cx) {
            Ok(rows) => rows.to_vec(),
            Err(el) => return el,
        };
        let cols = copy::IMAGE_COLS;
        let body = rows.into_iter().map(|r| {
            row(SharedString::from(format!("img-{}", r.id)), false, p)
                .child(cell(cols[0].1).child(text(
                    r.tags.clone(),
                    if r.dangling { p.text_2 } else { p.text },
                )))
                .child(cell(cols[1].1).child(mono(r.short_id.clone(), p.text_2)))
                .child(cell(cols[2].1).child(mono(r.size.clone(), p.text_2)))
                .child(cell(cols[3].1).child(text(r.containers.clone(), p.text_2)))
        });
        v_flex()
            .px(space(4))
            .child(header("tbl", &cols, &all_cols(&cols), false, false, p))
            .children(body)
            .into_any_element()
    }

    fn volumes_table(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let rows: Vec<VolumeRow> = match self.state_view(&self.volumes, copy::EMPTY_VOLUMES, p, cx)
        {
            Ok(rows) => rows.to_vec(),
            Err(el) => return el,
        };
        let cols = copy::VOLUME_COLS;
        let body = rows.into_iter().map(|r| {
            row(SharedString::from(format!("vol-{}", r.name)), false, p)
                .child(cell(cols[0].1).child(text(r.name.clone(), p.text)))
                .child(cell(cols[1].1).child(mono(r.size.clone(), p.text_2)))
                // The chip hugs its label (M6): a flex row that does not stretch it.
                .child(cell(cols[2].1).flex().child(status_chip(
                    SharedString::from(format!("vol-chip-{}", r.name)),
                    volume_chip(&r),
                    r.in_use_label,
                    p,
                )))
        });
        v_flex()
            .px(space(4))
            .child(header("tbl", &cols, &all_cols(&cols), false, false, p))
            .children(body)
            .into_any_element()
    }

    fn filter_field(&self, window: &Window, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let focused = self.focus.is_focused(window);
        let (value, color) = if self.query.is_empty() {
            (copy::FILTER_PLACEHOLDER.to_owned(), p.text_disabled)
        } else {
            (self.query.clone(), p.text)
        };
        div()
            .id("container-filter")
            .track_focus(&self.focus)
            .w(px(metric::FIELD_SELECT))
            .h(px(metric::BTN))
            .px(space(1))
            .gap(space(1))
            .flex()
            .items_center()
            .rounded(px(metric::RADIUS_MD))
            .border_1()
            .border_color(hsla(if focused { p.focus } else { p.border_strong }))
            .bg(hsla(p.bg_raised))
            .child(icon(Icon::Filter, hsla(p.text_2)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_size(px(font::BODY.size))
                    .text_color(hsla(color))
                    .child(value),
            )
            .on_click(cx.listener(|v, _, window, cx| window.focus(&v.focus, cx)))
            .on_key_down(cx.listener(|v, ev: &KeyDownEvent, _, cx| {
                let mut q = v.query.clone();
                if edit_query(&mut q, ev) {
                    v.set_query(q, cx);
                }
            }))
    }

    fn toolbar(&self, window: &Window, p: &Palette, cx: &mut Context<Self>) -> Div {
        let bar = h_flex().flex_none().px(space(4)).py(space(1)).gap(space(1));
        let refresh = icon_button("refresh", icon(Icon::Refresh, hsla(p.text_2)), p)
            .on_click(cx.listener(|v, _, _, cx| v.retry(cx)));
        match self.tab {
            Tab::Containers => {
                let counts = self.filter_counts();
                let options = StatusFilter::ALL
                    .iter()
                    .map(|f| SharedString::from(format!("{} {}", f.label(), counts.get(*f))))
                    .collect();
                let selected = StatusFilter::ALL
                    .iter()
                    .position(|f| *f == self.filter)
                    .unwrap_or(0);
                let entity = cx.entity().downgrade();
                bar.child(self.filter_field(window, p, cx))
                    .child(segmented(
                        "status-filter",
                        options,
                        selected,
                        p,
                        move |i, _, cx| {
                            if let Some(view) = entity.upgrade() {
                                view.update(cx, |v, cx| {
                                    v.set_status_filter(StatusFilter::ALL[i], cx)
                                });
                            }
                        },
                    ))
                    .child(div().flex_1())
                    .when(counts.exited > 0, |b| {
                        b.child(
                            button(
                                "remove-exited",
                                copy::remove_exited(counts.exited),
                                ButtonVariant::Default,
                                p,
                            )
                            .child(icon(Icon::Trash, hsla(p.text_2)))
                            .debug_selector(|| "remove-exited".to_string())
                            .on_click(cx.listener(|v, _, _, cx| v.request_remove_exited(cx))),
                        )
                    })
                    .child(refresh)
            }
            Tab::Images => bar
                .child(div().flex_1())
                .child(
                    button(
                        "prune-images",
                        copy::PRUNE_IMAGES,
                        ButtonVariant::Default,
                        p,
                    )
                    .on_click(cx.listener(|v, _, _, cx| v.request_image_prune(cx))),
                )
                .child(refresh),
            Tab::Volumes => bar
                .child(div().flex_1())
                .child(
                    button(
                        "prune-volumes",
                        copy::PRUNE_VOLUMES,
                        ButtonVariant::Default,
                        p,
                    )
                    .on_click(cx.listener(|v, _, _, cx| v.request_volume_prune(cx))),
                )
                .child(refresh),
        }
    }

    /// The 로그 검색 field (Containers.dc.html): Escape clears it, then closes the panel.
    fn log_search(
        &self,
        log: &LogPanel,
        window: &Window,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let focused = self.log_search_focus.is_focused(window);
        let (value, color) = if log.query.is_empty() {
            (copy::LOG_SEARCH.to_owned(), p.text_disabled)
        } else {
            (log.query.clone(), p.text)
        };
        div()
            .id("log-search")
            .debug_selector(|| "log-search".into())
            .track_focus(&self.log_search_focus)
            .flex_none()
            .w(px(metric::FIELD_SEARCH))
            .h(px(metric::BTN_SM))
            .px(space(1))
            .gap(space(0))
            .flex()
            .items_center()
            .rounded(px(metric::RADIUS_MD))
            .border_1()
            .border_color(hsla(if focused { p.focus } else { p.border_strong }))
            .bg(hsla(p.bg_raised))
            .child(icon(Icon::Search, hsla(p.text_2)))
            .child(
                small(value, color)
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap(),
            )
            .on_click(cx.listener(|v, _, window, cx| window.focus(&v.log_search_focus, cx)))
            .on_key_down(cx.listener(|v, ev: &KeyDownEvent, _, cx| {
                let mut q = v.log_query().to_owned();
                if edit_query(&mut q, ev) {
                    cx.stop_propagation();
                    v.set_log_query(q, cx);
                }
            }))
    }

    fn log_panel(
        &self,
        log: &LogPanel,
        window: &Window,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (follow, show_time) = (log.follow, log.show_time);
        let rows = self.log_rows();
        let count = log_count_text(rows.len(), log.lines.len(), !log.query.is_empty());
        let lines = rows.iter().map(|l| {
            let color = match l.tone {
                LineTone::Error => p.err,
                LineTone::Normal => p.text,
            };
            h_flex()
                .gap(space(2))
                .font_family(font::MONO)
                .text_size(px(font::CODE.size))
                .line_height(px(font::CODE.line_height))
                .when(show_time, |r| {
                    r.child(
                        div()
                            .flex_none()
                            .text_color(hsla(p.text_2))
                            .child(l.time.clone()),
                    )
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_color(hsla(color))
                        .child(l.text.to_owned()),
                )
        });
        let entity = cx.entity().downgrade();
        let entity2 = entity.clone();
        let meta = if log.image.is_empty() {
            short_image_id(&log.container)
        } else {
            format!("{} · {}", log.image, short_image_id(&log.container))
        };
        v_flex()
            .id("log-panel")
            .debug_selector(|| "log-panel".into())
            .track_focus(&self.log_focus)
            .on_key_down(cx.listener(|v, ev: &KeyDownEvent, _, cx| {
                if ev.keystroke.key == "escape" {
                    cx.stop_propagation();
                    v.close_logs(cx);
                }
            }))
            .h(px(metric::LOG_PANEL_H))
            .flex_none()
            .border_t_1()
            .border_color(hsla(p.border_strong))
            .bg(hsla(p.bg))
            .child(
                h_flex()
                    .h(px(metric::TAB_H))
                    .flex_none()
                    .pl(space(4))
                    .pr(space(2))
                    .gap(space(1))
                    .child(
                        div()
                            .flex_none()
                            .child(icon(Icon::Terminal, hsla(p.text_2))),
                    )
                    // Name and meta give way with an ellipsis; the controls never do (N6).
                    .child(
                        div()
                            .min_w_0()
                            .flex_shrink(1.)
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(px(font::BODY.size))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(hsla(p.text))
                            .child(log.name.clone()),
                    )
                    .child(
                        mono(meta, p.text_2)
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis(),
                    )
                    .child(self.log_search(log, window, p, cx))
                    .child(
                        h_flex()
                            .flex_none()
                            .gap(space(1))
                            .debug_selector(|| "log-switches".into())
                            .child(
                                h_flex()
                                    .gap(space(0))
                                    .child(switch("log-time", show_time, p, move |on, _, cx| {
                                        if let Some(view) = entity.upgrade() {
                                            view.update(cx, |v, cx| v.set_show_time(on, cx));
                                        }
                                    }))
                                    .child(small(copy::LOG_TIME, p.text)),
                            )
                            .child(
                                h_flex()
                                    .gap(space(0))
                                    .child(switch("log-follow", follow, p, move |on, _, cx| {
                                        if let Some(view) = entity2.upgrade() {
                                            view.update(cx, |v, cx| v.set_follow(on, cx));
                                        }
                                    }))
                                    .child(
                                        small(copy::LOG_FOLLOW, p.text)
                                            .font_weight(FontWeight::MEDIUM),
                                    ),
                            ),
                    )
                    .child(
                        icon_button("log-close", icon(Icon::Close, hsla(p.text_2)), p)
                            .flex_none()
                            .debug_selector(|| "log-close".into())
                            .on_click(cx.listener(|v, _, _, cx| v.close_logs(cx))),
                    ),
            )
            .child(
                v_flex()
                    .id("log-lines")
                    .flex_1()
                    .min_h_0()
                    .px(space(1))
                    .py(space(0))
                    .border_t_1()
                    .border_color(hsla(p.border))
                    .bg(hsla(p.bg_sunken))
                    .overflow_y_scroll()
                    .when(log.lines.is_empty() && log.error.is_none(), |d| {
                        d.child(text(copy::LOG_EMPTY, p.text_2))
                    })
                    .children(lines)
                    .when_some(log.error.clone(), |d, e| d.child(text(e, p.err))),
            )
            .child(
                h_flex()
                    .h(px(metric::ROW))
                    .flex_none()
                    .px(space(4))
                    .gap(space(2))
                    .border_t_1()
                    .border_color(hsla(p.border))
                    .bg(hsla(p.bg_chrome))
                    .child(
                        h_flex()
                            .gap(space(0))
                            .when(follow, |f| f.child(icon(Icon::Follow, hsla(p.text_2))))
                            .child(small(
                                if follow {
                                    copy::LOG_FOLLOWING
                                } else {
                                    copy::LOG_PAUSED
                                },
                                p.text_2,
                            )),
                    )
                    .child(div().flex_1())
                    .child(mono(count, p.text_2)),
            )
            .with_animation(
                SharedString::from(format!("log-open-{}", log.container)),
                log_open_animation(),
                |el, t| el.opacity(t),
            )
            .into_any_element()
    }

    fn dialog_view(&self, d: &Dialog, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let more = d.items.len().saturating_sub(DIALOG_ITEMS);
        let items = d
            .items
            .iter()
            .take(DIALOG_ITEMS)
            .map(|i| mono(i.clone(), p.text_2));
        let content = v_flex()
            .id("dialog")
            .occlude()
            .gap(space(2))
            .child(
                div()
                    .text_size(px(font::TITLE.size))
                    .line_height(px(font::TITLE.line_height))
                    .font_weight(FontWeight::SEMIBOLD)
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
                            l.child(text(copy::more_items(more), p.text_2))
                        }),
                )
            })
            .child(
                h_flex()
                    .gap(space(1))
                    .justify_end()
                    .child(
                        button("dialog-cancel", copy::CANCEL, ButtonVariant::Default, p)
                            .on_click(cx.listener(|v, _, _, cx| v.dismiss(cx))),
                    )
                    .when(d.confirm.can_confirm, |b| {
                        b.child(
                            button(
                                "dialog-ok",
                                d.confirm.confirm_label.clone(),
                                ButtonVariant::Danger,
                                p,
                            )
                            .on_click(cx.listener(|v, _, _, cx| v.accept(cx))),
                        )
                    }),
            );
        modal("engine-dialog", metric::DIALOG_W_SM, p, content).into_any_element()
    }

    fn notice_view(&self, p: &Palette) -> Option<Div> {
        let n = self.notice.as_ref()?;
        let tone = if n.kind == NoticeKind::Busy {
            (p.busy, p.busy_bg)
        } else {
            (p.err, p.err_bg)
        };
        Some(
            h_flex()
                .mx(space(4))
                .px(space(2))
                .py(space(1))
                .gap(space(1))
                .rounded(px(metric::RADIUS_MD))
                .border_1()
                .border_color(hsla(p.err_border))
                .bg(hsla(tone.1))
                .child(status_shape("notice-shape", Status::Error, p))
                .child(text(n.message.clone(), p.text))
                .when_some(n.hint, |d, h| d.child(mono(h, tone.0))),
        )
    }
}

/// How the log panel opens: the motion tokens, never literal timings (M13).
pub(super) fn log_open_animation() -> Animation {
    Animation::new(motion::BASE).with_easing(motion::ease_out)
}

/// Typing into the filter field: printable characters and backspace.
fn edit_query(value: &mut String, ev: &KeyDownEvent) -> bool {
    let k = &ev.keystroke;
    if k.modifiers.platform || k.modifiers.control {
        return false;
    }
    match k.key.as_str() {
        "backspace" => value.pop().is_some(),
        "escape" => {
            let had = !value.is_empty();
            value.clear();
            had
        }
        _ => match &k.key_char {
            Some(ch) if !ch.chars().any(char::is_control) => {
                value.push_str(ch);
                true
            }
            _ => false,
        },
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
        let this = cx.entity();
        // Measures the panel each frame so the containers table can drop its ID column.
        let measure = canvas(
            move |bounds, _, cx| {
                let w = f32::from(bounds.size.width);
                this.update(cx, |v, cx| v.set_width(w, cx));
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();
        v_flex()
            .relative()
            .size_full()
            .bg(hsla(p.bg))
            .child(measure)
            .child(self.toolbar(window, p, cx))
            .when_some(self.notice_view(p), |d, n| d.child(n))
            .child(
                div()
                    .id("engine-table")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(table),
            )
            .when_some(log, |d, log| d.child(self.log_panel(&log, window, p, cx)))
            .when_some(dialog, |d, dlg| d.child(self.dialog_view(&dlg, p, cx)))
    }
}
