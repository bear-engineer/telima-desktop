//! 트레이 명령을 앱 동작으로 옮기는 순수 계획과 보조 상태. GPUI에 의존하지 않는다.
//!
//! `main.rs`가 GPUI 타이머에서 `AppCommand`를 꺼내 [`apply`]에 넘기고, [`Shell`] 구현이
//! 창·종료·폴링을 실제로 수행한다. 시험은 호출을 기록하는 가짜 `Shell`로 한다.

use std::collections::HashMap;

use telima_app::AppCommand;
use telima_app::profiles::Lifecycle;
use telima_app::view::tray::{BusyOp, TrayError};
use telima_core::profile::ProfileName;

/// 트레이 명령 하나가 일으키는 동작 한 단계.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// 창이 있으면 앞으로, 없으면 새로 연다.
    ShowWindow,
    /// 트레이 아이콘에 "시작 중" 같은 작업 종류를 보이려고 기억해 둔다.
    Track(ProfileName, Lifecycle),
    /// `MainView::run_command`로 창에 적용한다.
    Run(AppCommand),
    /// 프로필 목록을 지금 다시 읽는다.
    PollNow,
    Quit,
}

/// `window_open`: 메인 창이 지금 열려 있는지.
pub fn plan(cmd: &AppCommand, window_open: bool) -> Vec<Step> {
    let lifecycle = match cmd {
        AppCommand::Start(n) => Some((n, Lifecycle::Start)),
        AppCommand::Stop(n) => Some((n, Lifecycle::Stop)),
        AppCommand::Restart(n) => Some((n, Lifecycle::Restart)),
        _ => None,
    };
    let mut steps = Vec::new();
    match cmd {
        AppCommand::OpenWindow => return vec![Step::ShowWindow],
        AppCommand::Quit => return vec![Step::Quit],
        // 창이 없으면 트레이 쪽만 다시 읽는다.
        AppCommand::Refresh if !window_open => return vec![Step::PollNow],
        _ => {}
    }
    if !window_open {
        steps.push(Step::ShowWindow);
    }
    if let Some((name, lc)) = lifecycle {
        steps.push(Step::Track(name.clone(), lc));
    }
    steps.push(Step::Run(cmd.clone()));
    if *cmd == AppCommand::Refresh {
        steps.push(Step::PollNow);
    }
    steps
}

/// [`Step`]을 수행하는 쪽.
pub trait Shell {
    /// 메인 창 하나를 가리키는 핸들.
    type Window: Clone;
    /// 이 프로세스가 연 메인 창 핸들, 오래된 것부터. 이미 닫힌 것이 섞일 수 있다.
    fn windows(&self) -> Vec<Self::Window>;
    /// `window`가 아직 살아 있는지(갱신이 성공하는지).
    fn alive(&mut self, window: &Self::Window) -> bool;
    /// `window`를 앞으로 가져온다.
    fn activate(&mut self, window: &Self::Window);
    /// 새 메인 창을 열고 그 핸들을 돌려준다.
    fn open(&mut self) -> Self::Window;
    fn track(&mut self, name: ProfileName, cmd: Lifecycle);
    /// `cmd`를 `window`에서 실행한다. 창이 사라졌으면 오류.
    fn run(&mut self, window: &Self::Window, cmd: &AppCommand) -> Result<(), String>;
    fn poll_now(&mut self);
    fn quit(&mut self);
    /// 삼키면 안 되는 실패를 남긴다(표준 오류).
    fn log(&mut self, message: &str);
}

/// 살아 있는 첫 메인 창.
pub fn live_window<S: Shell>(shell: &mut S) -> Option<S::Window> {
    shell.windows().into_iter().find(|w| shell.alive(w))
}

/// 살아 있는 창을 앞으로 가져오거나, 없으면 하나 연다. 명령을 실행할 창을 돌려준다.
pub fn show_window<S: Shell>(shell: &mut S) -> S::Window {
    match live_window(shell) {
        Some(window) => {
            shell.activate(&window);
            window
        }
        None => shell.open(),
    }
}

pub fn apply<S: Shell>(cmd: &AppCommand, shell: &mut S) {
    let mut window = live_window(shell);
    for step in plan(cmd, window.is_some()) {
        match step {
            Step::ShowWindow => window = Some(show_window(shell)),
            Step::Track(name, lc) => shell.track(name, lc),
            Step::Run(cmd) => {
                let result = match &window {
                    Some(w) => shell.run(w, &cmd),
                    None => Err("no main window".into()),
                };
                if let Err(err) = result {
                    shell.log(&format!("{cmd:?} failed: {err}"));
                }
            }
            Step::PollNow => shell.poll_now(),
            Step::Quit => shell.quit(),
        }
    }
}

/// 트레이에서 보낸 작업의 종류를 기억했다가, 앱이 그 프로필을 처리 중일 때 `BusyOp`로 돌려준다.
#[derive(Debug, Default)]
pub struct Tracked(HashMap<ProfileName, Lifecycle>);

impl Tracked {
    pub fn insert(&mut self, name: ProfileName, cmd: Lifecycle) {
        self.0.insert(name, cmd);
    }

    /// `is_busy`가 거짓이면 기억을 지우고 `None`. 참이면 기억한 종류, 모르면 `Other`.
    pub fn busy_op(&mut self, name: &ProfileName, is_busy: bool) -> Option<BusyOp> {
        if !is_busy {
            self.0.remove(name);
            return None;
        }
        Some(
            self.0
                .get(name)
                .map_or(BusyOp::Other, |lc| BusyOp::Lifecycle(*lc)),
        )
    }
}

/// 창에서 끝난 시작·중지·재시작 가운데 마지막 실패. 트레이 오류 행과 오류 아이콘의 근거다.
#[derive(Debug, Default)]
pub struct LastFailure(Option<(ProfileName, Lifecycle)>);

impl LastFailure {
    /// 끝난 명령 하나를 반영한다. 실패면 기억하고, 같은 프로필의 성공이면 지운다.
    pub fn record(&mut self, name: ProfileName, cmd: Lifecycle, ok: bool) {
        if !ok {
            self.0 = Some((name, cmd));
        } else if self.0.as_ref().is_some_and(|(n, _)| *n == name) {
            self.0 = None;
        }
    }

    /// 트레이에서 고른 명령을 본다. 창 열기(오류 행 포함)면 지운다.
    pub fn on_command(&mut self, cmd: &AppCommand) {
        if *cmd == AppCommand::OpenWindow {
            self.clear();
        }
    }

    /// 사용자가 창을 열면(Dock 클릭) 지운다.
    pub fn clear(&mut self) {
        self.0 = None;
    }

    /// 트레이에 보일 오류. 명령 실패가 목록 읽기 실패보다 앞선다.
    pub fn error(&self, list_failed: bool) -> Option<TrayError> {
        match &self.0 {
            Some((name, cmd)) => Some(TrayError::Lifecycle {
                name: name.clone(),
                cmd: *cmd,
            }),
            None => list_failed.then_some(TrayError::List),
        }
    }
}

/// 창을 처음 닫을 때 한 번만 안내한다. 저장소가 없어 실행 한 번 안에서만 기억한다.
#[derive(Debug, Default)]
pub struct CloseNotice {
    shown: bool,
}

impl CloseNotice {
    /// 트레이가 있고 메인 창이 모두 닫혔고 아직 안 보였으면 참(그리고 보인 것으로 기록).
    pub fn take(&mut self, tray_present: bool, windows_left: usize) -> bool {
        if !tray_present || windows_left > 0 || self.shown {
            return false;
        }
        self.shown = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(s: &str) -> ProfileName {
        ProfileName::parse(s).unwrap()
    }

    /// 창 핸들은 번호, `windows[i].1`이 거짓이면 닫힌(죽은) 핸들.
    #[derive(Default)]
    struct Fake {
        windows: Vec<(u32, bool)>,
        /// 참이면 `run`이 실패한다.
        run_fails: bool,
        calls: Vec<String>,
    }

    impl Fake {
        fn with(windows: &[(u32, bool)]) -> Self {
            Self {
                windows: windows.to_vec(),
                ..Self::default()
            }
        }
        fn opened(&self) -> usize {
            self.calls.iter().filter(|c| c.starts_with("open")).count()
        }
    }

    impl Shell for Fake {
        type Window = u32;
        fn windows(&self) -> Vec<u32> {
            self.windows.iter().map(|(id, _)| *id).collect()
        }
        fn alive(&mut self, window: &u32) -> bool {
            self.windows.iter().any(|(id, ok)| id == window && *ok)
        }
        fn activate(&mut self, window: &u32) {
            self.calls.push(format!("show {window}"));
        }
        fn open(&mut self) -> u32 {
            let id = self.windows.iter().map(|(id, _)| id + 1).max().unwrap_or(1);
            self.windows.push((id, true));
            self.calls.push(format!("open {id}"));
            id
        }
        fn track(&mut self, name: ProfileName, cmd: Lifecycle) {
            self.calls.push(format!("track {} {cmd:?}", name.as_str()));
        }
        fn run(&mut self, window: &u32, cmd: &AppCommand) -> Result<(), String> {
            let text = match cmd {
                AppCommand::Restart(n) => format!("Restart({})", n.as_str()),
                AppCommand::Stop(n) => format!("Stop({})", n.as_str()),
                other => format!("{other:?}"),
            };
            self.calls.push(format!("run {window} {text}"));
            if self.run_fails || !self.alive(window) {
                return Err("window is gone".into());
            }
            Ok(())
        }
        fn poll_now(&mut self) {
            self.calls.push("poll".into());
        }
        fn quit(&mut self) {
            self.calls.push("quit".into());
        }
        fn log(&mut self, message: &str) {
            self.calls.push(format!("log {message}"));
        }
    }

    #[test]
    fn open_window_only_shows_the_window() {
        assert_eq!(plan(&AppCommand::OpenWindow, true), [Step::ShowWindow]);
        assert_eq!(plan(&AppCommand::OpenWindow, false), [Step::ShowWindow]);
    }

    #[test]
    fn quit_only_quits() {
        assert_eq!(plan(&AppCommand::Quit, true), [Step::Quit]);
    }

    #[test]
    fn lifecycle_commands_are_tracked_then_run_in_the_window() {
        for (cmd, lc) in [
            (AppCommand::Start(name("dev")), Lifecycle::Start),
            (AppCommand::Stop(name("dev")), Lifecycle::Stop),
            (AppCommand::Restart(name("dev")), Lifecycle::Restart),
        ] {
            assert_eq!(
                plan(&cmd, true),
                [Step::Track(name("dev"), lc), Step::Run(cmd.clone())]
            );
        }
    }

    #[test]
    fn lifecycle_command_with_closed_window_reopens_it_first() {
        assert_eq!(
            plan(&AppCommand::Start(name("dev")), false),
            [
                Step::ShowWindow,
                Step::Track(name("dev"), Lifecycle::Start),
                Step::Run(AppCommand::Start(name("dev")))
            ]
        );
    }

    #[test]
    fn new_profile_reopens_a_closed_window_and_opens_the_form() {
        assert_eq!(
            plan(&AppCommand::NewProfile, false),
            [Step::ShowWindow, Step::Run(AppCommand::NewProfile)]
        );
        assert_eq!(
            plan(&AppCommand::NewProfile, true),
            [Step::Run(AppCommand::NewProfile)]
        );
    }

    #[test]
    fn refresh_polls_the_tray_and_reloads_the_window_only_when_open() {
        assert_eq!(
            plan(&AppCommand::Refresh, true),
            [Step::Run(AppCommand::Refresh), Step::PollNow]
        );
        assert_eq!(plan(&AppCommand::Refresh, false), [Step::PollNow]);
    }

    #[test]
    fn apply_records_calls_in_order() {
        let mut fake = Fake::default();
        apply(&AppCommand::Restart(name("dev")), &mut fake);
        assert_eq!(
            fake.calls,
            ["open 1", "track dev Restart", "run 1 Restart(dev)"]
        );
    }

    #[test]
    fn apply_quit_records_quit() {
        let mut fake = Fake::with(&[(1, true)]);
        apply(&AppCommand::Quit, &mut fake);
        assert_eq!(fake.calls, ["quit"]);
    }

    #[test]
    fn lifecycle_command_runs_on_the_live_window_past_a_dead_handle() {
        let mut fake = Fake::with(&[(1, false), (2, true)]);
        apply(&AppCommand::Stop(name("dev")), &mut fake);
        assert_eq!(fake.calls, ["track dev Stop", "run 2 Stop(dev)"]);
        assert_eq!(fake.opened(), 0, "never a second window");
    }

    #[test]
    fn open_window_activates_the_live_window_past_a_dead_handle() {
        let mut fake = Fake::with(&[(1, false), (2, true)]);
        apply(&AppCommand::OpenWindow, &mut fake);
        assert_eq!(fake.calls, ["show 2"]);
    }

    #[test]
    fn only_dead_handles_open_one_window_and_run_there() {
        let mut fake = Fake::with(&[(1, false)]);
        apply(&AppCommand::Stop(name("dev")), &mut fake);
        assert_eq!(fake.calls, ["open 2", "track dev Stop", "run 2 Stop(dev)"]);
    }

    #[test]
    fn a_failed_run_is_logged_not_dropped() {
        let mut fake = Fake {
            run_fails: true,
            ..Fake::with(&[(1, true)])
        };
        apply(&AppCommand::Stop(name("dev")), &mut fake);
        assert_eq!(fake.calls.len(), 3, "{:?}", fake.calls);
        assert!(fake.calls[2].starts_with("log "), "{:?}", fake.calls);
        assert!(fake.calls[2].contains("window is gone"), "{:?}", fake.calls);
    }

    #[test]
    fn tracked_reports_the_sent_kind_while_busy_and_forgets_after() {
        let mut t = Tracked::default();
        t.insert(name("dev"), Lifecycle::Start);
        assert_eq!(
            t.busy_op(&name("dev"), true),
            Some(BusyOp::Lifecycle(Lifecycle::Start))
        );
        assert_eq!(t.busy_op(&name("dev"), false), None);
        assert_eq!(t.busy_op(&name("dev"), true), Some(BusyOp::Other));
    }

    #[test]
    fn close_notice_shows_once_when_the_last_window_closes_with_a_tray() {
        let mut n = CloseNotice::default();
        assert!(!n.take(true, 1), "a window is still open");
        assert!(n.take(true, 0));
        assert!(!n.take(true, 0), "only once");
    }

    fn failed(n: &str, cmd: Lifecycle) -> Option<TrayError> {
        Some(TrayError::Lifecycle { name: name(n), cmd })
    }

    #[test]
    fn a_failed_command_puts_the_error_row_in_the_tray_model() {
        let mut f = LastFailure::default();
        assert_eq!(f.error(false), None);
        f.record(name("dev"), Lifecycle::Start, false);
        assert_eq!(f.error(false), failed("dev", Lifecycle::Start));
        let model = telima_app::view::tray::tray_model(&[], |_| None, f.error(false).as_ref());
        assert_eq!(
            model.error.map(|e| (e.label, e.command)),
            Some((
                "오류: dev 시작 실패 — 창에서 보기".to_owned(),
                AppCommand::OpenWindow
            ))
        );
    }

    #[test]
    fn the_next_success_on_that_profile_clears_the_error() {
        let mut f = LastFailure::default();
        f.record(name("dev"), Lifecycle::Stop, false);
        f.record(name("work"), Lifecycle::Start, true);
        assert_eq!(
            f.error(false),
            failed("dev", Lifecycle::Stop),
            "a success elsewhere keeps it"
        );
        f.record(name("dev"), Lifecycle::Restart, true);
        assert_eq!(f.error(false), None);
    }

    #[test]
    fn a_later_failure_replaces_the_earlier_one() {
        let mut f = LastFailure::default();
        f.record(name("dev"), Lifecycle::Start, false);
        f.record(name("work"), Lifecycle::Restart, false);
        assert_eq!(f.error(false), failed("work", Lifecycle::Restart));
    }

    #[test]
    fn opening_the_window_clears_the_error() {
        let mut f = LastFailure::default();
        f.record(name("dev"), Lifecycle::Start, false);
        f.on_command(&AppCommand::Refresh);
        assert_eq!(f.error(false), failed("dev", Lifecycle::Start));
        f.on_command(&AppCommand::OpenWindow);
        assert_eq!(f.error(false), None, "창 열기 and the error row");
        f.record(name("dev"), Lifecycle::Start, false);
        f.clear();
        assert_eq!(f.error(false), None, "Dock reopen");
    }

    #[test]
    fn a_command_failure_wins_over_a_list_failure() {
        let mut f = LastFailure::default();
        assert_eq!(f.error(true), Some(TrayError::List));
        f.record(name("dev"), Lifecycle::Stop, false);
        assert_eq!(f.error(true), failed("dev", Lifecycle::Stop));
    }

    #[test]
    fn close_notice_never_shows_without_a_tray() {
        let mut n = CloseNotice::default();
        assert!(!n.take(false, 0));
        assert!(
            n.take(true, 0),
            "a skipped check does not use up the notice"
        );
    }
}
