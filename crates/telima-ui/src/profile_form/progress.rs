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
use telima_app::view::notice::error_text;
use telima_app::view::start_steps::{self, START_STEPS, StartProgress as StepProgress, StepState};
use telima_core::profile::{ProfileName, ProgressEvent, ProgressLevel};

use super::widgets::{Tone, button};
use crate::components::chip::status_chip;
use crate::components::status::Status;
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
    /// Keeps the log pinned to its newest line.
    scroll: ScrollHandle,
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
            scroll: ScrollHandle::new(),
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

    /// Which of the six start steps the log has reached (Starting.dc.html, M8).
    pub fn step_progress(&self) -> StepProgress {
        start_steps::start_progress(self.lines.iter().map(|l| l.event.message.as_str()))
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
                Msg::Line(line) => {
                    self.lines.push(line);
                    self.scroll.scroll_to_bottom();
                }
                Msg::Done(Ok(())) => self.finish(ProgressEnd::Done, cx),
                Msg::Done(Err(e)) => self.finish(ProgressEnd::Failed(error_text(&e)), cx),
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

fn status_of(end: Option<&ProgressEnd>) -> Status {
    match end {
        None => Status::Busy,
        Some(ProgressEnd::Done) => Status::Running,
        Some(ProgressEnd::Cancelled) => Status::Stopped,
        Some(ProgressEnd::Failed(_)) => Status::Error,
    }
}

/// Six bars with step names: done = `bar`, current = `busy`, pending = empty track.
fn step_bars(steps: StepProgress, p: &Palette) -> Div {
    let states = steps.steps();
    v_flex()
        .gap(space(1))
        .child(
            h_flex()
                .gap(space(1))
                .items_baseline()
                .child(
                    div()
                        .text_size(px(font::BODY.size))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("시작 단계"),
                )
                .child(
                    div()
                        .font_family(font::MONO)
                        .text_size(px(font::SMALL.size))
                        .text_color(hsla(p.text_2))
                        .child(steps.text()),
                ),
        )
        .child(
            h_flex()
                .gap(space(0))
                .items_start()
                .children(START_STEPS.iter().zip(states).map(|(name, state)| {
                    let (fill, text) = match state {
                        StepState::Done => (Some(p.bar), p.text),
                        StepState::Current => (Some(p.busy), p.busy),
                        StepState::Pending => (None, p.text_2),
                    };
                    v_flex()
                        .flex_1()
                        .min_w(px(0.0))
                        .gap(space(1))
                        .child(
                            div()
                                .h(px(metric::BAR_H))
                                .rounded_full()
                                .bg(hsla(fill.unwrap_or(p.track))),
                        )
                        .child(
                            div()
                                .text_size(px(font::SMALL.size))
                                .text_color(hsla(text))
                                .when(state == StepState::Current, |d| {
                                    d.font_weight(FontWeight::SEMIBOLD)
                                })
                                .child(*name),
                        )
                })),
        )
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
                    .text_size(px(font::TITLE.size))
                    .line_height(px(font::TITLE.line_height))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(hsla(p.text))
                    .child(self.name.to_string()),
            )
            .child(status_chip(
                "progress-status",
                status_of(self.end.as_ref()),
                self.title(),
                p,
            ))
            .child(
                div()
                    .font_family(font::MONO)
                    .text_size(px(font::SMALL.size))
                    .text_color(hsla(if running { p.busy } else { p.text_2 }))
                    .child(format!("{} 경과", self.elapsed_label())),
            )
            .child(div().flex_1())
            .when(running && self.cmd == Lifecycle::Start, |h| {
                h.child(
                    button("progress-cancel", "시작 취소", true, Tone::Default, p)
                        .on_click(cx.listener(|v, _, _, cx| v.cancel(cx))),
                )
            });

        let log = v_flex()
            .id("progress-log")
            .flex_1()
            .min_h(px(0.0))
            .track_scroll(&self.scroll)
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
            .text_color(hsla(p.text))
            .font_family(font::SANS)
            .child(header)
            .when(self.cmd != Lifecycle::Stop, |v| {
                v.child(step_bars(self.step_progress(), p))
            })
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
