//! Delete confirmation: the user must type the profile name before the button turns on.
use std::sync::{Arc, Mutex};

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use telima_app::AppError;
use telima_app::runtime::App as Telima;
use telima_app::view::Confirm;
use telima_app::view::notice::error_text;
use telima_core::profile::ProfileName;

use super::widgets::{Tone, button, edit_text, error_line, text_box};
use crate::components::dialog::{dialog_frame, fade_in};
use crate::components::paragraph::paragraph;
use crate::style::{hsla, palette, space};
use crate::theme::{font, metric};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteEvent {
    Deleted(ProfileName),
    Cancelled,
}

type Inbox = Arc<Mutex<Vec<Result<(), AppError>>>>;

pub struct DeleteConfirm {
    app: Telima,
    name: ProfileName,
    copy: Confirm,
    typed: String,
    deleting: bool,
    error: Option<String>,
    inbox: Inbox,
    focus: FocusHandle,
    /// Set until the first render moves focus into the dialog.
    focus_on_open: bool,
    _poll: Task<()>,
}

impl EventEmitter<DeleteEvent> for DeleteConfirm {}

impl DeleteConfirm {
    /// `copy` comes from `telima_app::view::confirm_delete_profile`.
    pub fn new(app: Telima, name: ProfileName, copy: Confirm, cx: &mut Context<Self>) -> Self {
        Self {
            app,
            name,
            copy,
            typed: String::new(),
            deleting: false,
            error: None,
            inbox: Inbox::default(),
            focus: cx.focus_handle(),
            focus_on_open: true,
            _poll: super::poll(cx, Self::pump),
        }
    }

    pub fn typed(&self) -> &str {
        &self.typed
    }

    /// True while keyboard focus is inside the dialog (it takes focus when it opens, N3).
    pub fn has_focus(&self, window: &Window) -> bool {
        self.focus.is_focused(window)
    }

    pub fn is_deleting(&self) -> bool {
        self.deleting
    }

    /// True only when the typed text equals the profile name and nothing is running.
    pub fn can_delete(&self) -> bool {
        let wanted = self
            .copy
            .typed_name
            .as_deref()
            .unwrap_or(self.name.as_str());
        self.copy.can_confirm && !self.deleting && self.typed == wanted
    }

    pub fn set_typed(&mut self, text: &str, cx: &mut Context<Self>) {
        if self.deleting {
            return;
        }
        self.typed = text.to_owned();
        cx.notify();
    }

    pub fn confirm(&mut self, cx: &mut Context<Self>) {
        if !self.can_delete() {
            return;
        }
        self.deleting = true;
        self.error = None;
        let inbox = self.inbox.clone();
        self.app.delete_profile(
            self.name.clone(),
            self.typed.clone(),
            telima_core::profile::DeleteOptions::default(),
            move |r| inbox.lock().unwrap().push(r),
        );
        cx.notify();
    }

    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        if !self.deleting {
            cx.emit(DeleteEvent::Cancelled);
        }
    }

    /// Applies the finished delete. Called by the poll task (and by tests).
    pub fn pump(&mut self, cx: &mut Context<Self>) {
        let results = std::mem::take(&mut *self.inbox.lock().unwrap());
        if results.is_empty() {
            return;
        }
        for r in results {
            self.deleting = false;
            match r {
                Ok(()) => cx.emit(DeleteEvent::Deleted(self.name.clone())),
                Err(e) => self.error = Some(error_text(&e)),
            }
        }
        cx.notify();
    }
}

impl Render for DeleteConfirm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.focus_on_open) {
            window.focus(&self.focus, cx);
        }
        let p = palette(window);
        let can = self.can_delete();
        let label = if self.deleting {
            "삭제 중…".to_owned()
        } else {
            self.copy.confirm_label.clone()
        };
        let input = text_box(
            "delete-typed",
            &self.typed,
            self.name.as_str(),
            self.deleting,
            &self.focus,
            window,
            p,
        )
        .on_click(cx.listener(|v, _, window, cx| window.focus(&v.focus, cx)))
        .on_key_down(cx.listener(|v, ev: &KeyDownEvent, _, cx| {
            let mut text = v.typed.clone();
            if edit_text(&mut text, ev) {
                v.set_typed(&text, cx);
            } else if ev.keystroke.key == "enter" {
                v.confirm(cx);
            }
        }));

        // Escape bubbles here from anywhere inside the frame, not only from the input.
        let frame = dialog_frame(metric::DIALOG_W, p)
            .on_key_down(cx.listener(|v, ev: &KeyDownEvent, _, cx| {
                if ev.keystroke.key == "escape" {
                    v.cancel(cx);
                }
            }))
            .font_family(font::SANS)
            .child(
                div()
                    .text_size(px(font::TITLE.size))
                    .line_height(px(font::TITLE.line_height))
                    .child(self.copy.title.clone()),
            )
            .child(
                div()
                    .text_size(px(font::BODY.size))
                    .line_height(px(font::BODY.line_height))
                    .text_color(hsla(p.text_2))
                    .child(paragraph(&self.copy.body)),
            )
            .child(
                v_flex()
                    .gap(space(1))
                    .child(
                        paragraph(format!("확인하려면 '{}'을(를) 입력하세요", self.name))
                            .text_size(px(font::SMALL.size)),
                    )
                    .child(input),
            )
            .children(
                self.error
                    .as_ref()
                    .map(|e| error_line("delete-error", format!("삭제하지 못했습니다. {e}"), p)),
            )
            .child(
                h_flex()
                    .gap(space(1))
                    .justify_end()
                    .child(
                        button("delete-cancel", "취소", !self.deleting, Tone::Default, p)
                            .when(!self.deleting, |b| {
                                b.on_click(cx.listener(|v, _, _, cx| v.cancel(cx)))
                            }),
                    )
                    .child(
                        button("delete-confirm", label, can, Tone::Danger, p).when(can, |b| {
                            b.on_click(cx.listener(|v, _, _, cx| v.confirm(cx)))
                        }),
                    ),
            );
        fade_in("delete-dialog", frame)
    }
}
