//! Start progress log (canvas `Starting.dc.html`): monospace `INFO[0012] message` lines,
//! elapsed time, and a cancel button. Runs a lifecycle command as soon as it is created.
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use telima_app::AppError;
use telima_app::profiles::Lifecycle;
use telima_app::runtime::{App as Telima, Task as Work};
use telima_core::profile::{ProfileName, ProgressEvent, ProgressLevel};

use super::widgets::{Tone, button};
use crate::style::{hsla, palette, space};
use crate::theme::{Palette, font, metric};

/// How the command ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgressEnd {
    Done,
    Failed(String),
    Cancelled,
}

struct Line {
    at: Duration,
    event: ProgressEvent,
}

impl Line {
    fn text(&self) -> String {
        format!(
            "{}[{:04}] {}",
            level_label(self.event.level),
            self.at.as_secs(),
            self.event.message
        )
    }
}

fn level_label(level: ProgressLevel) -> &'static str {
    match level {
        ProgressLevel::Info => "INFO",
        ProgressLevel::Warn => "WARN",
        ProgressLevel::Error => "ERRO",
    }
}

enum Msg {
    Line(Line),
    Done(Result<(), AppError>),
}

type Inbox = Arc<Mutex<Vec<Msg>>>;

pub struct StartProgress {
    name: ProfileName,
    cmd: Lifecycle,
    started: Instant,
    /// Frozen when the command ends so the label stops counting.
    finished_after: Option<Duration>,
    lines: Vec<Line>,
    end: Option<ProgressEnd>,
    work: Work,
    inbox: Inbox,
    _poll: Task<()>,
}

impl EventEmitter<ProgressEnd> for StartProgress {}

impl StartProgress {
    /// Starts `cmd` on `name` and collects its progress lines.
    pub fn new(app: Telima, name: ProfileName, cmd: Lifecycle, cx: &mut Context<Self>) -> Self {
        let inbox = Inbox::default();
        let started = Instant::now();
        let lines = inbox.clone();
        let done = inbox.clone();
        let work = app.run_profile(
            name.clone(),
            cmd,
            move |event| {
                let at = started.elapsed();
                lines.lock().unwrap().push(Msg::Line(Line { at, event }));
            },
            move |r| done.lock().unwrap().push(Msg::Done(r)),
        );
        Self {
            name,
            cmd,
            started,
            finished_after: None,
            lines: Vec::new(),
            end: None,
            work,
            inbox,
            // Also redraws the elapsed label while the command runs.
            _poll: super::poll(cx, |v: &mut Self, cx| {
                v.pump(cx);
                if v.is_running() {
                    cx.notify();
                }
            }),
        }
    }

    pub fn name(&self) -> &ProfileName {
        &self.name
    }

    pub fn is_running(&self) -> bool {
        self.end.is_none()
    }

    pub fn is_cancelled(&self) -> bool {
        self.end == Some(ProgressEnd::Cancelled)
    }

    pub fn end(&self) -> Option<&ProgressEnd> {
        self.end.as_ref()
    }

    /// Log lines as shown, e.g. `INFO[0012] starting colima`.
    pub fn lines(&self) -> Vec<String> {
        self.lines.iter().map(Line::text).collect()
    }

    /// `mm:ss` since the command started (stops when it ends).
    pub fn elapsed_label(&self) -> String {
        let secs = self
            .finished_after
            .unwrap_or_else(|| self.started.elapsed())
            .as_secs();
        format!("{:02}:{:02}", secs / 60, secs % 60)
    }

    /// Aborts the command at its next await point. Colima stops the VM; the disk stays.
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        if !self.is_running() {
            return;
        }
        self.work.cancel();
        self.finish(ProgressEnd::Cancelled, cx);
    }

    fn finish(&mut self, end: ProgressEnd, cx: &mut Context<Self>) {
        self.finished_after = Some(self.started.elapsed());
        self.end = Some(end.clone());
        cx.emit(end);
        cx.notify();
    }

    /// Applies new lines and the result. Called by the poll task (and by tests).
    pub fn pump(&mut self, cx: &mut Context<Self>) {
        let msgs = std::mem::take(&mut *self.inbox.lock().unwrap());
        if msgs.is_empty() {
            return;
        }
        for msg in msgs {
            // After a cancel nothing late is shown.
            if self.is_cancelled() {
                break;
            }
            match msg {
                Msg::Line(line) => self.lines.push(line),
                Msg::Done(Ok(())) => self.finish(ProgressEnd::Done, cx),
                Msg::Done(Err(e)) => self.finish(ProgressEnd::Failed(e.to_string()), cx),
            }
        }
        cx.notify();
    }

    fn title(&self) -> &'static str {
        match (self.cmd, &self.end) {
            (_, Some(ProgressEnd::Cancelled)) => "취소됨",
            (_, Some(ProgressEnd::Failed(_))) => "실패",
            (Lifecycle::Start, None) => "시작 중",
            (Lifecycle::Stop, None) => "중지 중",
            (Lifecycle::Restart, None) => "재시작 중",
            (Lifecycle::Start | Lifecycle::Restart, Some(ProgressEnd::Done)) => "실행 중",
            (Lifecycle::Stop, Some(ProgressEnd::Done)) => "중지됨",
        }
    }
}

fn level_color(level: ProgressLevel, p: &Palette) -> Hsla {
    hsla(match level {
        ProgressLevel::Info => p.text_2,
        ProgressLevel::Warn => p.busy,
        ProgressLevel::Error => p.err,
    })
}

impl Render for StartProgress {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(window);
        let running = self.is_running();
        let header = h_flex()
            .h(px(metric::ROW))
            .gap(space(2))
            .child(
                div()
                    .text_size(px(font::BODY.size))
                    .text_color(hsla(p.text))
                    .child(format!("{} {}", self.name, self.title())),
            )
            .child(
                div()
                    .font_family(font::MONO)
                    .text_size(px(font::SMALL.size))
                    .text_color(hsla(p.text_2))
                    .child(format!("{} 경과", self.elapsed_label())),
            )
            .child(div().flex_1())
            .when(running && self.cmd == Lifecycle::Start, |h| {
                h.child(
                    button("progress-cancel", "시작 취소", true, Tone::Plain, p)
                        .on_click(cx.listener(|v, _, _, cx| v.cancel(cx))),
                )
            });

        let log = v_flex()
            .id("progress-log")
            .flex_1()
            .overflow_y_scroll()
            .p(space(2))
            .rounded(px(metric::RADIUS_MD))
            .border_1()
            .border_color(hsla(p.border))
            .bg(hsla(p.bg_sunken))
            .font_family(font::MONO)
            .text_size(px(font::CODE.size))
            .line_height(px(font::CODE.line_height))
            .when(self.lines.is_empty(), |l| {
                l.child(div().text_color(hsla(p.text_disabled)).child(if running {
                    "로그를 기다리는 중…"
                } else {
                    "로그 없음"
                }))
            })
            .children(self.lines.iter().map(|line| {
                let level = level_label(line.event.level);
                h_flex()
                    .gap(space(2))
                    .child(
                        div()
                            .text_color(level_color(line.event.level, p))
                            .child(format!("{level}[{:04}]", line.at.as_secs())),
                    )
                    .child(
                        div()
                            .text_color(hsla(p.text))
                            .child(line.event.message.clone()),
                    )
            }));

        let footer = h_flex()
            .gap(space(2))
            .text_size(px(font::SMALL.size))
            .text_color(hsla(p.text_2))
            .child(match &self.end {
                Some(ProgressEnd::Failed(e)) => format!("▲ {e}"),
                _ if self.cmd == Lifecycle::Start => {
                    "취소하면 VM을 멈춥니다. 만든 디스크는 남습니다.".to_owned()
                }
                _ => String::new(),
            })
            .child(div().flex_1())
            .child(format!("{}줄", self.lines.len()));

        v_flex()
            .size_full()
            .p(space(3))
            .gap(space(2))
            .bg(hsla(p.bg))
            .font_family(font::SANS)
            .child(header)
            .child(
                div()
                    .text_size(px(font::SMALL.size))
                    .text_color(hsla(p.text_2))
                    .child("시작 로그"),
            )
            .child(log)
            .child(footer)
    }
}
