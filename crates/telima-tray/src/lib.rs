//! OS 메뉴 막대·시스템 트레이 어댑터 (macOS: tray-icon, Linux: ksni).
//!
//! 의존 방향: telima-app만 의존한다. GPUI·tokio·bollard·telima-colima·telima-docker·telima-ui 금지.
//!
//! 메뉴 내용은 `telima_app::view::tray::TrayModel`에서 [`menu::menu_nodes`]로 만든다.
//! 메뉴를 누르면 [`AppCommand`]가 채널로 나온다. 받는 쪽(GPUI 타이머)이 꺼내 쓴다.

pub mod icons;
pub mod menu;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod mac;

use std::sync::mpsc::{Receiver, channel};

use telima_app::AppCommand;
use telima_app::view::tray::{TrayIconState, TrayModel, tray_icon_state};

pub use icons::{PanelVariant, TrayIcons};

#[cfg(target_os = "linux")]
use linux::Inner;
#[cfg(target_os = "macos")]
use mac::Inner;
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
use unsupported::Inner;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod unsupported {
    use std::sync::mpsc::Sender;

    use telima_app::AppCommand;
    use telima_app::view::tray::TrayModel;

    use crate::icons::TrayIcons;

    pub struct Inner;

    impl Inner {
        pub fn new(_: &TrayModel, _: TrayIcons, _: Sender<AppCommand>) -> Option<Self> {
            None
        }
        pub fn update(&mut self, _: &TrayModel) {}
    }
}

/// 트레이 아이콘 하나. 만든 스레드에서 갱신한다(macOS는 메인 스레드).
/// 버리면 아이콘이 사라진다.
pub struct TrayHandle {
    inner: Inner,
    last: TrayModel,
}

impl TrayHandle {
    /// 트레이를 띄운다. 트레이 호스트가 없으면(Linux에 SNI 호스트 없음 등) `None`.
    pub fn new(model: &TrayModel, icons: TrayIcons) -> Option<(Self, Receiver<AppCommand>)> {
        let (tx, rx) = channel();
        let inner = Inner::new(model, icons, tx)?;
        Some((
            Self {
                inner,
                last: model.clone(),
            },
            rx,
        ))
    }

    /// 모델이 바뀐 경우에만 메뉴·아이콘·툴팁을 다시 그린다.
    pub fn update(&mut self, model: &TrayModel) {
        if !needs_update(&self.last, model) {
            return;
        }
        self.inner.update(model);
        self.last = model.clone();
    }
}

fn needs_update(last: &TrayModel, next: &TrayModel) -> bool {
    last != next
}

pub(crate) fn icon_state(model: &TrayModel) -> TrayIconState {
    tray_icon_state(model)
}

#[cfg(test)]
mod tests {
    use telima_app::view::tray::{BusyOp, tray_model};
    use telima_core::profile::{Profile, ProfileName, ProfileStatus, Runtime};

    use super::*;

    fn model(status: ProfileStatus) -> TrayModel {
        let p = Profile {
            name: ProfileName::parse("default").unwrap(),
            status,
            arch: "aarch64".into(),
            cpus: 2,
            memory_bytes: 2 << 30,
            disk_bytes: 20 << 30,
            address: None,
            runtime: Runtime::Docker,
        };
        tray_model(&[p], |_| None::<BusyOp>, None)
    }

    #[test]
    fn unchanged_model_needs_no_update() {
        let m = model(ProfileStatus::Running);
        assert!(!needs_update(&m, &m.clone()));
    }

    #[test]
    fn changed_model_needs_update() {
        assert!(needs_update(
            &model(ProfileStatus::Running),
            &model(ProfileStatus::Stopped)
        ));
    }

    #[test]
    fn icon_follows_model() {
        assert_eq!(
            icon_state(&model(ProfileStatus::Running)),
            TrayIconState::Running
        );
        assert_eq!(
            icon_state(&model(ProfileStatus::Stopped)),
            TrayIconState::AllStopped
        );
    }

    /// SNI 호스트(D-Bus 세션)가 필요하다: `cargo test -p telima-tray -- --ignored`.
    /// macOS는 메인 스레드에서만 만들 수 있어 시험 스레드에서는 못 돈다 — GPUI run 콜백에서 확인한다.
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "needs a StatusNotifierItem host on the session bus"]
    fn creates_a_tray_when_a_host_is_present() {
        let m = model(ProfileStatus::Running);
        assert!(TrayHandle::new(&m, TrayIcons::default()).is_some());
    }
}
