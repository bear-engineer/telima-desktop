//! View models: plain data the UI renders, built from use-case results. No GPUI types here.
//! Korean labels follow the canvas (design/canvas-v1). Pure functions, synchronous tests.

use telima_core::container::{Container, ContainerState, Image, Volume};
use telima_core::profile::{
    MountType, Profile, ProfileName, ProfileSpec, ProfileStatus, Runtime, VmType,
};

const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

/// One decimal place, without a trailing `.0`: 8.0 -> "8", 3.14 -> "3.1".
fn trim_decimal(v: f64) -> String {
    let s = format!("{v:.1}");
    s.strip_suffix(".0").unwrap_or(&s).to_owned()
}

/// Decimal (SI) size such as `docker system df` prints: 1_200_000_000 -> "1.2GB".
pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1_000 {
        return format!("{bytes}B");
    }
    const UNITS: [&str; 5] = ["KB", "MB", "GB", "TB", "PB"];
    // Start at KB: the first division already happened.
    let mut value = bytes as f64 / 1_000.0;
    let mut unit = 0;
    while value >= 1_000.0 && unit < UNITS.len() - 1 {
        value /= 1_000.0;
        unit += 1;
    }
    format!("{}{}", trim_decimal(value), UNITS[unit])
}

/// Binary GiB for VM resources: 8 GiB -> "8 GiB".
pub fn format_gib(bytes: u64) -> String {
    format!("{} GiB", trim_decimal(bytes as f64 / GIB))
}

/// Sidebar and header status. Shape and colour are chosen by the UI from this kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Running,
    Stopped,
    Busy,
    Error,
}

impl StatusKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "실행 중",
            Self::Stopped => "중지",
            Self::Busy => "처리 중",
            Self::Error => "오류",
        }
    }
}

fn status_kind(profile: &Profile, busy: bool) -> StatusKind {
    if busy {
        return StatusKind::Busy;
    }
    match profile.status {
        ProfileStatus::Running => StatusKind::Running,
        ProfileStatus::Stopped => StatusKind::Stopped,
        ProfileStatus::Other(_) => StatusKind::Error,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileRow {
    pub name: String,
    pub kind: StatusKind,
    /// `8C · 8G`: cpus and memory in GiB.
    pub resource_line: String,
}

pub fn profile_rows(
    profiles: &[Profile],
    is_busy: impl Fn(&ProfileName) -> bool,
) -> Vec<ProfileRow> {
    profiles
        .iter()
        .map(|p| ProfileRow {
            name: p.name.as_str().to_owned(),
            kind: status_kind(p, is_busy(&p.name)),
            resource_line: format!(
                "{}C · {}G",
                p.cpus,
                trim_decimal(p.memory_bytes as f64 / GIB)
            ),
        })
        .collect()
}

/// Which profile buttons are enabled. All false while an operation runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileActions {
    pub start: bool,
    pub stop: bool,
    pub restart: bool,
    pub edit: bool,
    pub delete: bool,
}

pub fn profile_actions(kind: StatusKind, busy: bool) -> ProfileActions {
    if busy || kind == StatusKind::Busy {
        return ProfileActions {
            start: false,
            stop: false,
            restart: false,
            edit: false,
            delete: false,
        };
    }
    // Error keeps every lifecycle action so the user can recover the VM.
    let (start, stop, restart) = match kind {
        StatusKind::Running => (false, true, true),
        StatusKind::Stopped => (true, false, false),
        StatusKind::Error => (true, true, true),
        StatusKind::Busy => (false, false, false),
    };
    ProfileActions {
        start,
        stop,
        restart,
        edit: true,
        delete: true,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileDetail {
    pub name: String,
    pub kind: StatusKind,
    pub status_label: &'static str,
    pub cpus: u32,
    pub memory: String,
    pub disk: String,
    pub vm_type: String,
    pub mount_type: String,
    pub runtime: String,
    pub arch: String,
    /// IP address, or `—` when the VM has none.
    pub address: String,
    pub actions: ProfileActions,
}

fn vm_label(vm: VmType) -> &'static str {
    match vm {
        VmType::Vz => "vz",
        VmType::Qemu => "qemu",
        VmType::Krunkit => "krunkit",
    }
}

fn mount_label(mount: MountType) -> &'static str {
    match mount {
        MountType::Virtiofs => "virtiofs",
        MountType::Sshfs => "sshfs",
        MountType::NineP => "9p",
    }
}

fn runtime_label(runtime: Runtime) -> &'static str {
    match runtime {
        Runtime::Docker => "docker",
        Runtime::Containerd => "containerd",
        Runtime::Incus => "incus",
    }
}

/// `config` supplies VM type and mount type, which `colima ls` does not report.
/// Without it those fields show `—`.
pub fn profile_detail(
    profile: &Profile,
    config: Option<&ProfileSpec>,
    busy: bool,
) -> ProfileDetail {
    let kind = status_kind(profile, busy);
    ProfileDetail {
        name: profile.name.as_str().to_owned(),
        kind,
        status_label: kind.label(),
        cpus: profile.cpus,
        memory: format_gib(profile.memory_bytes),
        disk: format_gib(profile.disk_bytes),
        vm_type: config.map_or("—", |c| vm_label(c.vm_type)).to_owned(),
        mount_type: config.map_or("—", |c| mount_label(c.mount_type)).to_owned(),
        runtime: runtime_label(profile.runtime).to_owned(),
        arch: profile.arch.clone(),
        address: profile.address.clone().unwrap_or_else(|| "—".to_owned()),
        actions: profile_actions(kind, busy),
    }
}

pub fn container_state_label(state: ContainerState) -> &'static str {
    match state {
        ContainerState::Created => "생성됨",
        ContainerState::Running => "실행 중",
        ContainerState::Paused => "일시정지",
        ContainerState::Restarting => "재시작 중",
        ContainerState::Exited => "종료됨",
        ContainerState::Dead => "오류",
        ContainerState::Other => "알 수 없음",
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContainerRow {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: ContainerState,
    pub state_label: &'static str,
    pub status_text: String,
}

pub fn container_rows(containers: &[Container]) -> Vec<ContainerRow> {
    containers
        .iter()
        .map(|c| ContainerRow {
            id: c.id.clone(),
            name: c.name.clone(),
            image: c.image.clone(),
            state: c.state,
            state_label: container_state_label(c.state),
            status_text: c.status_text.clone(),
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRow {
    pub id: String,
    /// Comma-separated tags, `<none>` when there are none.
    pub tags: String,
    pub size: String,
    /// `2개`, or `—` when the engine did not report usage.
    pub containers: String,
    pub dangling: bool,
}

pub fn image_rows(images: &[Image]) -> Vec<ImageRow> {
    images
        .iter()
        .map(|img| {
            let tags: Vec<&str> = img
                .tags
                .iter()
                .map(String::as_str)
                .filter(|t| *t != "<none>:<none>")
                .collect();
            let dangling = img.is_dangling();
            ImageRow {
                id: img.id.clone(),
                tags: if dangling {
                    "<none>".to_owned()
                } else {
                    tags.join(", ")
                },
                size: format_bytes(img.size_bytes),
                containers: img.containers.map_or("—".to_owned(), |n| format!("{n}개")),
                dangling,
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeRow {
    pub name: String,
    pub size: String,
    /// Unknown reference counts count as in use, as the domain rule says.
    pub in_use: bool,
    pub in_use_label: &'static str,
}

pub fn volume_rows(volumes: &[Volume]) -> Vec<VolumeRow> {
    volumes
        .iter()
        .map(|v| VolumeRow {
            name: v.name.clone(),
            size: v.size_bytes.map_or("—".to_owned(), format_bytes),
            in_use: !v.is_unused(),
            in_use_label: match v.ref_count {
                Some(0) => "미사용",
                Some(_) => "사용 중",
                None => "확인 불가",
            },
        })
        .collect()
}

/// Copy for a destructive confirmation dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirm {
    pub title: String,
    pub body: String,
    pub confirm_label: String,
    /// False when the action must not run at all (in-use volume, nothing to prune).
    pub can_confirm: bool,
    /// When set, the user must type this name before confirming.
    pub typed_name: Option<String>,
}

fn simple_confirm(title: &str, body: String, label: String) -> Confirm {
    Confirm {
        title: title.to_owned(),
        body,
        confirm_label: label,
        can_confirm: true,
        typed_name: None,
    }
}

fn refused_confirm(title: &str, body: String, label: &str) -> Confirm {
    Confirm {
        can_confirm: false,
        confirm_label: label.to_owned(),
        ..simple_confirm(title, body, String::new())
    }
}

pub fn confirm_delete_profile(
    name: &ProfileName,
    containers: usize,
    volumes: usize,
    volume_bytes: u64,
) -> Confirm {
    Confirm {
        typed_name: Some(name.as_str().to_owned()),
        ..simple_confirm(
            "프로필 삭제",
            format!(
                "VM 디스크, 컨테이너 {containers}개, 볼륨 {volumes}개가 함께 지워집니다. 되돌릴 수 없습니다."
            ),
            format!("볼륨 {volumes}개, {} 삭제", format_bytes(volume_bytes)),
        )
    }
}

pub fn confirm_remove_volume(name: &str, in_use: bool) -> Confirm {
    if in_use {
        return refused_confirm(
            "볼륨 삭제",
            format!(
                "사용 중인 볼륨 '{name}'은 삭제할 수 없습니다. 이 볼륨을 쓰는 컨테이너를 먼저 지워 주세요."
            ),
            "볼륨 삭제",
        );
    }
    simple_confirm(
        "볼륨 삭제",
        format!("볼륨 '{name}'의 데이터가 지워집니다. 되돌릴 수 없습니다."),
        "볼륨 삭제".to_owned(),
    )
}

pub fn confirm_prune_volumes(volumes: &[Volume]) -> Confirm {
    if volumes.is_empty() {
        return refused_confirm(
            "미사용 볼륨 정리",
            "정리할 미사용 볼륨이 없습니다.".to_owned(),
            "볼륨 정리",
        );
    }
    let bytes: u64 = volumes.iter().filter_map(|v| v.size_bytes).sum();
    let n = volumes.len();
    simple_confirm(
        "미사용 볼륨 정리",
        format!("미사용 볼륨 {n}개를 지웁니다. 되돌릴 수 없습니다."),
        format!("볼륨 {n}개, {} 삭제", format_bytes(bytes)),
    )
}

pub fn confirm_prune_images(images: &[Image]) -> Confirm {
    if images.is_empty() {
        return refused_confirm(
            "미사용 이미지 정리",
            "정리할 미사용 이미지가 없습니다.".to_owned(),
            "이미지 정리",
        );
    }
    let bytes: u64 = images.iter().map(|i| i.size_bytes).sum();
    let n = images.len();
    simple_confirm(
        "미사용 이미지 정리",
        format!("미사용 이미지 {n}개를 지웁니다. 되돌릴 수 없습니다."),
        format!("이미지 {n}개, {} 삭제", format_bytes(bytes)),
    )
}

pub fn confirm_remove_container(name: &str, state: ContainerState) -> Confirm {
    let body = match state {
        ContainerState::Running | ContainerState::Paused | ContainerState::Restarting => {
            format!("실행 중인 컨테이너 '{name}'을 강제로 중지하고 삭제합니다. 되돌릴 수 없습니다.")
        }
        _ => format!("컨테이너 '{name}'을 삭제합니다. 되돌릴 수 없습니다."),
    };
    simple_confirm("컨테이너 삭제", body, "컨테이너 삭제".to_owned())
}

#[cfg(test)]
mod tests {
    use telima_core::container::{Container, ContainerState, Image, Volume};
    use telima_core::profile::{
        MountType, Profile, ProfileName, ProfileSpec, ProfileStatus, Runtime, VmType,
    };

    use super::*;

    const GIB: u64 = 1024 * 1024 * 1024;

    fn name(raw: &str) -> ProfileName {
        ProfileName::parse(raw).unwrap()
    }

    fn profile(raw: &str, status: ProfileStatus, address: Option<&str>) -> Profile {
        Profile {
            name: name(raw),
            status,
            arch: "aarch64".into(),
            cpus: 8,
            memory_bytes: 8 * GIB,
            disk_bytes: 100 * GIB,
            address: address.map(String::from),
            runtime: Runtime::Docker,
        }
    }

    fn spec(raw: &str) -> ProfileSpec {
        ProfileSpec {
            name: name(raw),
            cpus: 8,
            memory_gib: 8,
            disk_gib: 100,
            vm_type: VmType::Vz,
            mount_type: MountType::Virtiofs,
            runtime: Runtime::Docker,
            rosetta: false,
            network_address: true,
        }
    }

    #[test]
    fn formats_bytes_in_decimal_units() {
        assert_eq!(format_bytes(0), "0B");
        assert_eq!(format_bytes(999), "999B");
        assert_eq!(format_bytes(1_000), "1KB");
        assert_eq!(format_bytes(1_200_000_000), "1.2GB");
        assert_eq!(format_bytes(6_800_000_000), "6.8GB");
        assert_eq!(format_bytes(100_000_000_000), "100GB");
    }

    #[test]
    fn formats_gib_for_profile_resources() {
        assert_eq!(format_gib(8 * GIB), "8 GiB");
        assert_eq!(format_gib(3_328_599_654), "3.1 GiB");
        assert_eq!(format_gib(100 * GIB), "100 GiB");
    }

    #[test]
    fn profile_rows_show_kind_and_resource_line() {
        let profiles = vec![
            profile("dev", ProfileStatus::Running, None),
            profile("old", ProfileStatus::Stopped, None),
            profile("bad", ProfileStatus::Other("weird".into()), None),
            profile("busy", ProfileStatus::Stopped, None),
        ];
        let rows = profile_rows(&profiles, |n| n.as_str() == "busy");
        let kinds: Vec<_> = rows.iter().map(|r| (r.name.as_str(), r.kind)).collect();
        assert_eq!(
            kinds,
            vec![
                ("dev", StatusKind::Running),
                ("old", StatusKind::Stopped),
                ("bad", StatusKind::Error),
                ("busy", StatusKind::Busy),
            ]
        );
        assert_eq!(rows[0].resource_line, "8C · 8G");
    }

    #[test]
    fn status_kind_labels_are_korean() {
        assert_eq!(StatusKind::Running.label(), "실행 중");
        assert_eq!(StatusKind::Stopped.label(), "중지");
        assert_eq!(StatusKind::Busy.label(), "처리 중");
        assert_eq!(StatusKind::Error.label(), "오류");
    }

    #[test]
    fn actions_follow_status() {
        let running = profile_actions(StatusKind::Running, false);
        assert_eq!(
            (running.start, running.stop, running.restart),
            (false, true, true)
        );
        let stopped = profile_actions(StatusKind::Stopped, false);
        assert_eq!(
            (stopped.start, stopped.stop, stopped.restart),
            (true, false, false)
        );
        let error = profile_actions(StatusKind::Error, false);
        assert_eq!((error.start, error.stop, error.restart), (true, true, true));
    }

    #[test]
    fn busy_disables_every_action() {
        for kind in [StatusKind::Running, StatusKind::Stopped, StatusKind::Error] {
            let a = profile_actions(kind, true);
            assert!(!a.start && !a.stop && !a.restart && !a.edit && !a.delete);
        }
        let busy = profile_actions(StatusKind::Busy, true);
        assert!(!busy.start && !busy.stop && !busy.restart && !busy.edit && !busy.delete);
    }

    #[test]
    fn edit_and_delete_are_enabled_when_idle() {
        let a = profile_actions(StatusKind::Stopped, false);
        assert!(a.edit && a.delete);
    }

    #[test]
    fn detail_uses_config_for_vm_mount_and_address_fallback() {
        let p = profile("dev", ProfileStatus::Running, Some("192.168.64.2"));
        let d = profile_detail(&p, Some(&spec("dev")), false);
        assert_eq!(d.status_label, "실행 중");
        assert_eq!(d.cpus, 8);
        assert_eq!(d.memory, "8 GiB");
        assert_eq!(d.disk, "100 GiB");
        assert_eq!(d.vm_type, "vz");
        assert_eq!(d.mount_type, "virtiofs");
        assert_eq!(d.runtime, "docker");
        assert_eq!(d.arch, "aarch64");
        assert_eq!(d.address, "192.168.64.2");
        assert!(d.actions.stop && !d.actions.start);

        let no_addr = profile("dev", ProfileStatus::Stopped, None);
        let d = profile_detail(&no_addr, None, false);
        assert_eq!(d.address, "—");
        assert_eq!(d.vm_type, "—");
        assert_eq!(d.mount_type, "—");
        assert!(d.actions.start);
    }

    #[test]
    fn detail_busy_shows_processing_and_locks_actions() {
        let p = profile("dev", ProfileStatus::Running, None);
        let d = profile_detail(&p, None, true);
        assert_eq!(d.status_label, "처리 중");
        assert!(!d.actions.stop && !d.actions.restart && !d.actions.delete);
    }

    #[test]
    fn container_rows_map_state_and_status_text() {
        let c = |state, text: &str| Container {
            id: "3f2a9c1e".into(),
            name: "pg-main".into(),
            image: "postgres:17".into(),
            state,
            status_text: text.into(),
        };
        let rows = container_rows(&[
            c(ContainerState::Running, "실행 중 · 3시간"),
            c(ContainerState::Exited, "종료됨 · 코드 0"),
            c(ContainerState::Dead, "오류 · 종료 코드 1"),
        ]);
        assert_eq!(rows[0].state_label, "실행 중");
        assert_eq!(rows[1].state_label, "종료됨");
        assert_eq!(rows[2].state_label, "오류");
        assert_eq!(rows[0].name, "pg-main");
        assert_eq!(rows[0].image, "postgres:17");
        assert_eq!(rows[0].status_text, "실행 중 · 3시간");
        assert_eq!(rows[0].state, ContainerState::Running);
    }

    #[test]
    fn container_state_labels_cover_every_state() {
        let labels: Vec<_> = [
            ContainerState::Created,
            ContainerState::Running,
            ContainerState::Paused,
            ContainerState::Restarting,
            ContainerState::Exited,
            ContainerState::Dead,
            ContainerState::Other,
        ]
        .into_iter()
        .map(container_state_label)
        .collect();
        assert_eq!(
            labels,
            vec![
                "생성됨",
                "실행 중",
                "일시정지",
                "재시작 중",
                "종료됨",
                "오류",
                "알 수 없음"
            ]
        );
    }

    #[test]
    fn image_rows_show_tags_size_and_usage() {
        let images = vec![
            Image {
                id: "sha256:a".into(),
                tags: vec!["alpine:3".into(), "alpine:latest".into()],
                size_bytes: 1_200_000_000,
                containers: Some(2),
            },
            Image {
                id: "sha256:b".into(),
                tags: vec!["<none>:<none>".into()],
                size_bytes: 5_000,
                containers: None,
            },
        ];
        let rows = image_rows(&images);
        assert_eq!(rows[0].tags, "alpine:3, alpine:latest");
        assert_eq!(rows[0].size, "1.2GB");
        assert_eq!(rows[0].containers, "2개");
        assert!(!rows[0].dangling);
        assert_eq!(rows[1].tags, "<none>");
        assert_eq!(rows[1].size, "5KB");
        assert_eq!(rows[1].containers, "—");
        assert!(rows[1].dangling);
    }

    #[test]
    fn volume_rows_flag_in_use_and_unknown_counts() {
        let vols = vec![
            Volume {
                name: "pg".into(),
                size_bytes: Some(1_200_000_000),
                ref_count: Some(1),
            },
            Volume {
                name: "cache".into(),
                size_bytes: None,
                ref_count: Some(0),
            },
            Volume {
                name: "tmp".into(),
                size_bytes: Some(10),
                ref_count: None,
            },
        ];
        let rows = volume_rows(&vols);
        assert_eq!((rows[0].in_use, rows[0].in_use_label), (true, "사용 중"));
        assert_eq!(rows[0].size, "1.2GB");
        assert_eq!((rows[1].in_use, rows[1].in_use_label), (false, "미사용"));
        assert_eq!(rows[1].size, "—");
        assert_eq!((rows[2].in_use, rows[2].in_use_label), (true, "확인 불가"));
    }

    #[test]
    fn delete_profile_confirm_copy_lists_what_is_lost() {
        let c = confirm_delete_profile(&name("dev"), 6, 3, 1_200_000_000);
        assert_eq!(c.title, "프로필 삭제");
        assert_eq!(
            c.body,
            "VM 디스크, 컨테이너 6개, 볼륨 3개가 함께 지워집니다. 되돌릴 수 없습니다."
        );
        assert_eq!(c.confirm_label, "볼륨 3개, 1.2GB 삭제");
        assert_eq!(c.typed_name.as_deref(), Some("dev"));
        assert!(c.can_confirm);
    }

    #[test]
    fn volume_confirm_is_refused_while_in_use() {
        let used = confirm_remove_volume("pg", true);
        assert!(!used.can_confirm);
        assert!(used.body.contains("사용 중"));
        let idle = confirm_remove_volume("cache", false);
        assert!(idle.can_confirm);
        assert_eq!(idle.typed_name, None);
    }

    #[test]
    fn prune_confirm_counts_targets_and_refuses_empty() {
        let vols = vec![
            Volume {
                name: "a".into(),
                size_bytes: Some(1_200_000_000),
                ref_count: Some(0),
            },
            Volume {
                name: "b".into(),
                size_bytes: None,
                ref_count: Some(0),
            },
        ];
        let c = confirm_prune_volumes(&vols);
        assert_eq!(c.confirm_label, "볼륨 2개, 1.2GB 삭제");
        assert!(c.can_confirm);
        assert!(!confirm_prune_volumes(&[]).can_confirm);

        let imgs = vec![Image {
            id: "sha256:b".into(),
            tags: vec![],
            size_bytes: 2_000_000_000,
            containers: Some(0),
        }];
        let ci = confirm_prune_images(&imgs);
        assert_eq!(ci.confirm_label, "이미지 1개, 2GB 삭제");
        assert!(ci.can_confirm);
    }

    #[test]
    fn remove_container_confirm_warns_when_running() {
        let running = confirm_remove_container("pg-main", ContainerState::Running);
        assert!(running.body.contains("강제로 중지"));
        let exited = confirm_remove_container("job", ContainerState::Exited);
        assert!(!exited.body.contains("강제로"));
        assert_eq!(exited.confirm_label, "컨테이너 삭제");
    }
}
