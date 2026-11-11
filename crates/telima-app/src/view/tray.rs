//! Tray menu and window toolbar models (canvas boards 07-11). Pure: the tray adapter and the
//! toolbar render these and send back the [`AppCommand`] of the clicked item.
//! One definition per string; telima-tray and telima-ui hold no Korean copy.

use telima_core::profile::{Profile, ProfileName};

use super::StatusKind;
use crate::command::AppCommand;
use crate::profiles::Lifecycle;

/// Tray menu item that brings the window back.
pub const TRAY_OPEN_WINDOW: &str = "창 열기";
/// Tray menu item and toolbar button that read the profile list again.
pub const REFRESH: &str = "새로고침";
/// Tray menu item that ends the app.
pub const TRAY_QUIT: &str = "telima 종료";
/// Toolbar button that opens the new-profile form.
pub const NEW_PROFILE: &str = "새 프로필";
/// Last item of the toolbar profile dropdown.
pub const NEW_PROFILE_MENU: &str = "새 프로필…";

/// Title of the notice shown the first time the window closes while the tray stays.
pub const CLOSE_NOTICE_TITLE: &str = "telima";
/// Body of that notice.
pub const CLOSE_NOTICE_BODY: &str =
    "telima는 메뉴 막대에서 계속 실행됩니다. 종료하려면 메뉴의 'telima 종료'를 누르세요.";
/// Linux without a tray host: closing the window still quits.
pub const TRAY_UNAVAILABLE_NOTE: &str =
    "트레이를 표시할 수 없는 환경에서는 창을 닫으면 앱이 종료됩니다.";

/// What runs on a profile right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusyOp {
    Lifecycle(Lifecycle),
    /// Create, edit or delete.
    Other,
}

/// What last went wrong. Shown as the first tray row until the caller clears it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayError {
    /// A lifecycle command on `name` failed.
    Lifecycle { name: ProfileName, cmd: Lifecycle },
    /// The profile list could not be read.
    List,
}

/// A clickable menu item or toolbar button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandItem {
    pub label: String,
    pub command: AppCommand,
    pub enabled: bool,
}

/// One profile in the tray menu: a status line and a submenu of three commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayProfileRow {
    pub name: String,
    pub kind: StatusKind,
    /// `실행 중`, `중지`, `시작 중`, `오류`…
    pub status: String,
    pub start: CommandItem,
    pub stop: CommandItem,
    pub restart: CommandItem,
}

/// The whole tray menu, top to bottom. Equal models need no menu update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayModel {
    /// Error row above the summary; opens the window.
    pub error: Option<CommandItem>,
    /// `Colima 실행 중 1 · 중지 1`. Not clickable.
    pub summary: String,
    pub profiles: Vec<TrayProfileRow>,
    pub open_window: CommandItem,
    pub refresh: CommandItem,
    pub quit: CommandItem,
}

/// Build the tray menu. `busy` tells what runs on a profile, if anything.
/// Busy and failed profiles count in neither number of the summary.
pub fn tray_model(
    profiles: &[Profile],
    busy: impl Fn(&ProfileName) -> Option<BusyOp>,
    error: Option<&TrayError>,
) -> TrayModel {
    let _ = (profiles, &busy, error);
    todo!("RED")
}

/// Which tray icon to show. One at a time: error, then busy, then running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrayIconState {
    /// No profile runs (or there is none).
    AllStopped,
    /// At least one profile runs.
    Running,
    /// A start, stop or other operation is in progress.
    Busy,
    /// The last operation failed, or a profile is in an unknown state.
    Error,
}

impl TrayIconState {
    /// Tooltip and accessibility label of the icon.
    pub fn tooltip(self) -> &'static str {
        todo!("RED")
    }
}

pub fn tray_icon_state(model: &TrayModel) -> TrayIconState {
    let _ = model;
    todo!("RED")
}

/// Buttons on the right of the window toolbar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolbarActions {
    /// Lifecycle buttons that fit the selected profile, left to right. Empty without a
    /// selection and while an operation runs (the toolbar shows the progress status instead).
    pub lifecycle: Vec<CommandItem>,
    pub refresh: CommandItem,
    pub new_profile: CommandItem,
}

/// `selected` is the profile shown in the window and its sidebar status.
pub fn toolbar_actions(selected: Option<(&ProfileName, StatusKind)>) -> ToolbarActions {
    let _ = selected;
    todo!("RED")
}

#[cfg(test)]
mod tests {
    use telima_core::profile::{Profile, ProfileName, ProfileStatus, Runtime};

    use super::*;

    fn name(raw: &str) -> ProfileName {
        ProfileName::parse(raw).unwrap()
    }

    fn profile(raw: &str, status: ProfileStatus) -> Profile {
        Profile {
            name: name(raw),
            status,
            arch: "aarch64".into(),
            cpus: 2,
            memory_bytes: 2 << 30,
            disk_bytes: 20 << 30,
            address: None,
            runtime: Runtime::Docker,
        }
    }

    /// Canvas board 10: default runs, dev is stopped, telima-test-a.
    fn canvas_profiles() -> Vec<Profile> {
        vec![
            profile("default", ProfileStatus::Running),
            profile("dev", ProfileStatus::Stopped),
            profile("telima-test-a", ProfileStatus::Stopped),
        ]
    }

    fn idle(_: &ProfileName) -> Option<BusyOp> {
        None
    }

    fn starting_test_a(n: &ProfileName) -> Option<BusyOp> {
        (n.as_str() == "telima-test-a").then_some(BusyOp::Lifecycle(Lifecycle::Start))
    }

    fn item(label: &str, command: AppCommand, enabled: bool) -> CommandItem {
        CommandItem {
            label: label.to_owned(),
            command,
            enabled,
        }
    }

    #[test]
    fn tray_summary_counts_running_and_stopped_but_not_busy() {
        let model = tray_model(&canvas_profiles(), starting_test_a, None);
        assert_eq!(model.summary, "Colima 실행 중 1 · 중지 1");
        assert_eq!(
            tray_model(&[], idle, None).summary,
            "Colima 실행 중 0 · 중지 0"
        );
    }

    #[test]
    fn tray_has_one_row_per_profile_with_status_text() {
        let model = tray_model(&canvas_profiles(), starting_test_a, None);
        let rows: Vec<_> = model
            .profiles
            .iter()
            .map(|r| (r.name.as_str(), r.kind, r.status.as_str()))
            .collect();
        assert_eq!(
            rows,
            [
                ("default", StatusKind::Running, "실행 중"),
                ("dev", StatusKind::Stopped, "중지"),
                ("telima-test-a", StatusKind::Busy, "시작 중"),
            ]
        );
    }

    #[test]
    fn tray_busy_status_names_the_operation() {
        let status = |op| {
            let model = tray_model(&canvas_profiles(), |_| Some(op), None);
            model.profiles[0].status.clone()
        };
        assert_eq!(status(BusyOp::Lifecycle(Lifecycle::Stop)), "중지 중");
        assert_eq!(status(BusyOp::Lifecycle(Lifecycle::Restart)), "재시작 중");
        assert_eq!(status(BusyOp::Other), "처리 중");
    }

    #[test]
    fn tray_unknown_colima_status_is_an_error_row() {
        let model = tray_model(
            &[profile("dev", ProfileStatus::Other("Broken".into()))],
            idle,
            None,
        );
        assert_eq!(model.profiles[0].kind, StatusKind::Error);
        assert_eq!(model.profiles[0].status, "오류");
        assert_eq!(model.summary, "Colima 실행 중 0 · 중지 0");
    }

    #[test]
    fn tray_row_commands_follow_the_profile_state() {
        let model = tray_model(&canvas_profiles(), starting_test_a, None);
        let [running, stopped, busy] = &model.profiles[..] else {
            panic!("three rows expected");
        };
        assert_eq!(
            running.start,
            item("시작", AppCommand::Start(name("default")), false)
        );
        assert_eq!(
            running.stop,
            item("중지", AppCommand::Stop(name("default")), true)
        );
        assert_eq!(
            running.restart,
            item("재시작", AppCommand::Restart(name("default")), true)
        );
        assert_eq!(
            (
                stopped.start.enabled,
                stopped.stop.enabled,
                stopped.restart.enabled
            ),
            (true, false, false)
        );
        assert_eq!(stopped.start.command, AppCommand::Start(name("dev")));
        assert_eq!(
            (busy.start.enabled, busy.stop.enabled, busy.restart.enabled),
            (false, false, false)
        );
    }

    #[test]
    fn tray_fixed_items_open_refresh_and_quit() {
        let model = tray_model(&[], idle, None);
        assert_eq!(model.error, None);
        assert!(model.profiles.is_empty());
        assert_eq!(
            model.open_window,
            item("창 열기", AppCommand::OpenWindow, true)
        );
        assert_eq!(model.refresh, item("새로고침", AppCommand::Refresh, true));
        assert_eq!(model.quit, item("telima 종료", AppCommand::Quit, true));
    }

    #[test]
    fn tray_error_row_names_the_failed_command_and_opens_the_window() {
        let error = TrayError::Lifecycle {
            name: name("telima-test-a"),
            cmd: Lifecycle::Start,
        };
        let model = tray_model(&canvas_profiles(), idle, Some(&error));
        assert_eq!(
            model.error,
            Some(item(
                "오류: telima-test-a 시작 실패 — 창에서 보기",
                AppCommand::OpenWindow,
                true
            ))
        );
        // Canvas board 10, error case: the failed profile shows 오류 and leaves the summary.
        let failed = &model.profiles[2];
        assert_eq!(
            (failed.kind, failed.status.as_str()),
            (StatusKind::Error, "오류")
        );
        assert_eq!(model.summary, "Colima 실행 중 1 · 중지 1");
        // Every lifecycle command stays available so the user can recover.
        assert!(failed.start.enabled && failed.stop.enabled && failed.restart.enabled);

        let label = |cmd| {
            let error = TrayError::Lifecycle {
                name: name("dev"),
                cmd,
            };
            tray_model(&canvas_profiles(), idle, Some(&error))
                .error
                .unwrap()
                .label
        };
        assert_eq!(label(Lifecycle::Stop), "오류: dev 중지 실패 — 창에서 보기");
        assert_eq!(
            label(Lifecycle::Restart),
            "오류: dev 재시작 실패 — 창에서 보기"
        );
    }

    #[test]
    fn tray_error_row_for_an_unreadable_profile_list() {
        let model = tray_model(&[], idle, Some(&TrayError::List));
        assert_eq!(
            model.error,
            Some(item(
                "오류: 프로필 목록을 읽지 못함 — 창에서 보기",
                AppCommand::OpenWindow,
                true
            ))
        );
    }

    #[test]
    fn tray_retry_on_a_failed_profile_shows_busy() {
        let error = TrayError::Lifecycle {
            name: name("telima-test-a"),
            cmd: Lifecycle::Start,
        };
        let model = tray_model(&canvas_profiles(), starting_test_a, Some(&error));
        assert_eq!(model.profiles[2].status, "시작 중");
        assert!(model.error.is_some());
    }

    #[test]
    fn tray_equal_inputs_give_equal_models() {
        let build = || tray_model(&canvas_profiles(), starting_test_a, None);
        assert_eq!(build(), build());
        assert_ne!(build(), tray_model(&canvas_profiles(), idle, None));
    }

    #[test]
    fn tray_icon_state_all_stopped() {
        let stopped = [profile("dev", ProfileStatus::Stopped)];
        assert_eq!(
            tray_icon_state(&tray_model(&stopped, idle, None)),
            TrayIconState::AllStopped
        );
        assert_eq!(
            tray_icon_state(&tray_model(&[], idle, None)),
            TrayIconState::AllStopped
        );
    }

    #[test]
    fn tray_icon_state_running_when_any_profile_runs() {
        assert_eq!(
            tray_icon_state(&tray_model(&canvas_profiles(), idle, None)),
            TrayIconState::Running
        );
    }

    #[test]
    fn tray_icon_state_busy_beats_running() {
        assert_eq!(
            tray_icon_state(&tray_model(&canvas_profiles(), starting_test_a, None)),
            TrayIconState::Busy
        );
    }

    #[test]
    fn tray_icon_state_error_beats_busy_and_running() {
        let model = tray_model(&canvas_profiles(), starting_test_a, Some(&TrayError::List));
        assert_eq!(tray_icon_state(&model), TrayIconState::Error);
        let broken = [
            profile("default", ProfileStatus::Running),
            profile("dev", ProfileStatus::Other("Broken".into())),
        ];
        assert_eq!(
            tray_icon_state(&tray_model(&broken, idle, None)),
            TrayIconState::Error
        );
    }

    #[test]
    fn tray_icon_tooltip_names_busy_and_error() {
        assert_eq!(TrayIconState::AllStopped.tooltip(), "telima");
        assert_eq!(TrayIconState::Running.tooltip(), "telima");
        assert_eq!(TrayIconState::Busy.tooltip(), "telima, 작업 중");
        assert_eq!(TrayIconState::Error.tooltip(), "telima, 오류");
    }

    #[test]
    fn toolbar_without_selection_keeps_refresh_and_new_profile() {
        let actions = toolbar_actions(None);
        assert!(actions.lifecycle.is_empty());
        assert_eq!(actions.refresh, item("새로고침", AppCommand::Refresh, true));
        assert_eq!(
            actions.new_profile,
            item("새 프로필", AppCommand::NewProfile, true)
        );
    }

    #[test]
    fn toolbar_shows_only_the_lifecycle_buttons_that_fit() {
        let dev = name("dev");
        let lifecycle = |kind| toolbar_actions(Some((&dev, kind))).lifecycle;
        // Canvas board 07: running shows 재시작, 중지.
        assert_eq!(
            lifecycle(StatusKind::Running),
            [
                item("재시작", AppCommand::Restart(dev.clone()), true),
                item("중지", AppCommand::Stop(dev.clone()), true),
            ]
        );
        // Canvas board 08: stopped shows 시작.
        assert_eq!(
            lifecycle(StatusKind::Stopped),
            [item("시작", AppCommand::Start(dev.clone()), true)]
        );
        // Canvas board 09: a running operation replaces the buttons with its status.
        assert!(lifecycle(StatusKind::Busy).is_empty());
        assert_eq!(
            lifecycle(StatusKind::Error),
            [
                item("시작", AppCommand::Start(dev.clone()), true),
                item("재시작", AppCommand::Restart(dev.clone()), true),
                item("중지", AppCommand::Stop(dev.clone()), true),
            ]
        );
        let busy = toolbar_actions(Some((&dev, StatusKind::Busy)));
        assert!(busy.refresh.enabled && busy.new_profile.enabled);
    }

    #[test]
    fn close_notice_and_linux_fallback_copy() {
        assert_eq!(CLOSE_NOTICE_TITLE, "telima");
        assert_eq!(
            CLOSE_NOTICE_BODY,
            "telima는 메뉴 막대에서 계속 실행됩니다. 종료하려면 메뉴의 'telima 종료'를 누르세요."
        );
        assert!(CLOSE_NOTICE_BODY.contains(TRAY_QUIT));
        assert_eq!(
            TRAY_UNAVAILABLE_NOTE,
            "트레이를 표시할 수 없는 환경에서는 창을 닫으면 앱이 종료됩니다."
        );
        assert_eq!(NEW_PROFILE_MENU, "새 프로필…");
    }
}
