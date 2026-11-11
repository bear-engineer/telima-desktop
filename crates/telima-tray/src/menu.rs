//! `TrayModel` → 플랫폼 중립 메뉴 트리. OS 호출 없는 순수 함수.

use telima_app::AppCommand;
use telima_app::view::tray::TrayModel;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuNode {
    /// `command`가 없으면 눌러도 아무 일 없는 표시 줄(요약).
    Item {
        label: String,
        enabled: bool,
        command: Option<AppCommand>,
    },
    Separator,
    Submenu {
        label: String,
        items: Vec<MenuNode>,
    },
}

/// 위에서 아래: [오류 행] 요약, 구분선, [프로필별 하위 메뉴, 구분선], 창 열기, 새로고침, 구분선, 종료.
pub fn menu_nodes(_model: &TrayModel) -> Vec<MenuNode> {
    todo!()
}

#[cfg(test)]
mod tests {
    use telima_app::view::tray::{BusyOp, tray_model};
    use telima_core::profile::{Profile, ProfileName, ProfileStatus, Runtime};

    use super::*;

    fn profile(raw: &str, status: ProfileStatus) -> Profile {
        Profile {
            name: ProfileName::parse(raw).unwrap(),
            status,
            arch: "aarch64".into(),
            cpus: 2,
            memory_bytes: 2 << 30,
            disk_bytes: 20 << 30,
            address: None,
            runtime: Runtime::Docker,
        }
    }

    fn model(profiles: &[Profile]) -> TrayModel {
        tray_model(profiles, |_| None::<BusyOp>, None)
    }

    fn item(label: &str, enabled: bool, command: Option<AppCommand>) -> MenuNode {
        MenuNode::Item {
            label: label.into(),
            enabled,
            command,
        }
    }

    #[test]
    fn empty_model_has_summary_then_fixed_tail_without_double_separator() {
        let m = model(&[]);
        let nodes = menu_nodes(&m);
        assert_eq!(
            nodes,
            vec![
                item(&m.summary, false, None),
                MenuNode::Separator,
                item(&m.open_window.label, true, Some(AppCommand::OpenWindow)),
                item(&m.refresh.label, true, Some(AppCommand::Refresh)),
                MenuNode::Separator,
                item(&m.quit.label, true, Some(AppCommand::Quit)),
            ]
        );
    }

    #[test]
    fn each_profile_is_a_submenu_with_start_stop_restart() {
        let m = model(&[
            profile("default", ProfileStatus::Running),
            profile("dev", ProfileStatus::Stopped),
        ]);
        let nodes = menu_nodes(&m);
        let subs: Vec<_> = nodes
            .iter()
            .filter_map(|n| match n {
                MenuNode::Submenu { label, items } => Some((label.clone(), items.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(subs.len(), 2);
        let (label, items) = &subs[0];
        assert!(label.starts_with("default"), "{label}");
        assert!(label.contains(&m.profiles[0].status), "{label}");
        let name = |raw: &str| ProfileName::parse(raw).unwrap();
        assert_eq!(
            items,
            &vec![
                item(
                    &m.profiles[0].start.label,
                    false,
                    Some(AppCommand::Start(name("default")))
                ),
                item(
                    &m.profiles[0].stop.label,
                    true,
                    Some(AppCommand::Stop(name("default")))
                ),
                item(
                    &m.profiles[0].restart.label,
                    true,
                    Some(AppCommand::Restart(name("default")))
                ),
            ]
        );
        let MenuNode::Item { enabled, .. } = &subs[1].1[0] else {
            panic!()
        };
        assert!(enabled, "stopped profile can start");
    }

    #[test]
    fn error_row_comes_first_and_opens_window() {
        use telima_app::view::tray::TrayError;
        let m = tray_model(&[], |_| None::<BusyOp>, Some(&TrayError::List));
        let nodes = menu_nodes(&m);
        let err = m.error.as_ref().unwrap();
        assert_eq!(
            nodes[0],
            item(&err.label, true, Some(AppCommand::OpenWindow))
        );
        assert_eq!(nodes[1], item(&m.summary, false, None));
    }

    #[test]
    fn busy_profile_actions_are_disabled() {
        let name = ProfileName::parse("dev").unwrap();
        let m = tray_model(
            &[profile("dev", ProfileStatus::Stopped)],
            |n| (*n == name).then_some(BusyOp::Other),
            None,
        );
        let nodes = menu_nodes(&m);
        let MenuNode::Submenu { items, .. } = nodes
            .iter()
            .find(|n| matches!(n, MenuNode::Submenu { .. }))
            .unwrap()
        else {
            unreachable!()
        };
        assert!(
            items
                .iter()
                .all(|i| matches!(i, MenuNode::Item { enabled: false, .. }))
        );
    }
}
