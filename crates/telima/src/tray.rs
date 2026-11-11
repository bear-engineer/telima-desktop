//! 트레이 명령을 앱 동작으로 옮기는 순수 계획과 보조 상태. GPUI에 의존하지 않는다.
//!
//! `main.rs`가 GPUI 타이머에서 `AppCommand`를 꺼내 [`apply`]에 넘기고, [`Shell`] 구현이
//! 창·종료·폴링을 실제로 수행한다. 시험은 호출을 기록하는 가짜 `Shell`로 한다.

use std::collections::HashMap;

use telima_app::AppCommand;
use telima_app::profiles::Lifecycle;
use telima_app::view::tray::BusyOp;
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
    let _ = (cmd, window_open);
    todo!()
}

/// [`Step`]을 수행하는 쪽.
pub trait Shell {
    fn window_open(&self) -> bool;
    fn show_window(&mut self);
    fn track(&mut self, name: ProfileName, cmd: Lifecycle);
    fn run(&mut self, cmd: &AppCommand);
    fn poll_now(&mut self);
    fn quit(&mut self);
}

pub fn apply(cmd: &AppCommand, shell: &mut impl Shell) {
    let _ = (cmd, shell);
    todo!()
}

/// 트레이에서 보낸 작업의 종류를 기억했다가, 앱이 그 프로필을 처리 중일 때 `BusyOp`로 돌려준다.
#[derive(Debug, Default)]
pub struct Tracked(HashMap<ProfileName, Lifecycle>);

impl Tracked {
    pub fn insert(&mut self, name: ProfileName, cmd: Lifecycle) {
        let _ = (name, cmd);
        todo!()
    }

    /// `is_busy`가 거짓이면 기억을 지우고 `None`. 참이면 기억한 종류, 모르면 `Other`.
    pub fn busy_op(&mut self, name: &ProfileName, is_busy: bool) -> Option<BusyOp> {
        let _ = (name, is_busy);
        todo!()
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
        let _ = (tray_present, windows_left);
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(s: &str) -> ProfileName {
        ProfileName::parse(s).unwrap()
    }

    #[derive(Default)]
    struct Fake {
        open: bool,
        calls: Vec<String>,
    }

    impl Shell for Fake {
        fn window_open(&self) -> bool {
            self.open
        }
        fn show_window(&mut self) {
            self.open = true;
            self.calls.push("show".into());
        }
        fn track(&mut self, name: ProfileName, cmd: Lifecycle) {
            self.calls.push(format!("track {} {cmd:?}", name.as_str()));
        }
        fn run(&mut self, cmd: &AppCommand) {
            let text = match cmd {
                AppCommand::Restart(n) => format!("Restart({})", n.as_str()),
                other => format!("{other:?}"),
            };
            self.calls.push(format!("run {text}"));
        }
        fn poll_now(&mut self) {
            self.calls.push("poll".into());
        }
        fn quit(&mut self) {
            self.calls.push("quit".into());
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
            ["show", "track dev Restart", "run Restart(dev)"]
        );
    }

    #[test]
    fn apply_quit_records_quit() {
        let mut fake = Fake {
            open: true,
            ..Fake::default()
        };
        apply(&AppCommand::Quit, &mut fake);
        assert_eq!(fake.calls, ["quit"]);
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

    #[test]
    fn close_notice_never_shows_without_a_tray() {
        let mut n = CloseNotice::default();
        assert!(!n.take(false, 0));
        assert!(n.take(true, 0), "a skipped check does not use up the notice");
    }
}
