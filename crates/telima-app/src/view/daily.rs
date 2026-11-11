//! L6 copy: versions, ports, log filter, delete data option, mounts, Kubernetes, cache prune.

use telima_core::container::{LogLine, PortBinding, PortProtocol};
use telima_core::profile::{DeleteOptions, Mount, Runtime, RuntimeInfo};
use telima_core::rules::{KUBERNETES_RUNTIME, RuleViolation};
use telima_core::version::{ColimaVersion, LAST_TESTED, MIN_SUPPORTED, VersionSupport};

use super::{Confirm, simple_confirm};

const UNKNOWN: &str = "—";

/// Sidebar footer lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionFooter {
    /// `Colima 0.10.3`
    pub colima: String,
    /// `Docker 29.5.2` (engine in the VM; the client version when the engine is unknown)
    pub docker: String,
}

pub fn version_footer(info: &RuntimeInfo) -> VersionFooter {
    let docker = info
        .docker_server
        .as_deref()
        .or(info.docker_client.as_deref())
        .unwrap_or(UNKNOWN);
    VersionFooter {
        colima: format!(
            "Colima {}",
            info.colima
                .map_or_else(|| UNKNOWN.to_owned(), |v| v.to_string())
        ),
        docker: format!("Docker {docker}"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionNoticeSeverity {
    /// Older than the minimum: the app should not be used until Colima is updated.
    Blocking,
    /// Newer than the last tested version: works as far as known.
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionNotice {
    pub severity: VersionNoticeSeverity,
    pub title: &'static str,
    pub message: String,
    /// A command to run, shown as code.
    pub hint: Option<&'static str>,
}

/// Startup notice for the installed Colima; `None` when the version is in the tested range.
pub fn version_notice(version: ColimaVersion, support: VersionSupport) -> Option<VersionNotice> {
    match support {
        VersionSupport::Supported => None,
        VersionSupport::TooOld => Some(VersionNotice {
            severity: VersionNoticeSeverity::Blocking,
            title: "Colima 업데이트 필요",
            message: format!(
                "설치된 Colima {version}은 지원하는 최소 버전({MIN_SUPPORTED})보다 낮습니다. 업데이트한 뒤 다시 실행해 주세요."
            ),
            hint: Some("brew upgrade colima"),
        }),
        VersionSupport::Untested => Some(VersionNotice {
            severity: VersionNoticeSeverity::Warning,
            title: "검증되지 않은 Colima 버전",
            message: format!(
                "설치된 Colima {version}은 아직 확인하지 못한 버전입니다(확인된 최신 {LAST_TESTED}). 대부분 동작하지만 일부 기능이 다를 수 있습니다."
            ),
            hint: None,
        }),
    }
}

fn protocol_word(p: PortProtocol) -> &'static str {
    match p {
        PortProtocol::Tcp => "tcp",
        PortProtocol::Udp => "udp",
        PortProtocol::Sctp => "sctp",
    }
}

/// `8080→8080/tcp`; unpublished ports show the container side (`5432/tcp`). IPv4 and IPv6
/// bindings of one port collapse into one entry. No ports: `—`.
pub fn ports_text(ports: &[PortBinding]) -> String {
    let mut seen: Vec<String> = Vec::new();
    for p in ports {
        let proto = protocol_word(p.protocol);
        let text = match p.host_port {
            Some(host) => format!("{host}→{}/{proto}", p.container_port),
            None => format!("{}/{proto}", p.container_port),
        };
        if !seen.contains(&text) {
            seen.push(text);
        }
    }
    if seen.is_empty() {
        UNKNOWN.to_owned()
    } else {
        seen.join(", ")
    }
}

/// Case-insensitive substring match. A blank query matches every line.
pub fn log_matches(text: &str, query: &str) -> bool {
    let query = query.trim();
    query.is_empty() || text.to_lowercase().contains(&query.to_lowercase())
}

pub fn filter_log<'a>(lines: &'a [LogLine], query: &str) -> Vec<&'a LogLine> {
    lines
        .iter()
        .filter(|l| log_matches(&l.text, query))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteOptionCopy {
    pub label: &'static str,
    pub description: &'static str,
}

/// The "delete data disk too" checkbox (off by default).
pub fn delete_data_option() -> DeleteOptionCopy {
    DeleteOptionCopy {
        label: "데이터 디스크도 삭제",
        description: "컨테이너 이미지와 볼륨이 담긴 데이터 디스크까지 지웁니다. 끄면 VM만 지워지고 같은 이름으로 다시 만들 때 데이터를 이어서 쓸 수 있습니다.",
    }
}

/// What the chosen option costs, for the delete dialog body.
pub fn delete_consequence(options: DeleteOptions) -> &'static str {
    if options.data {
        "VM과 데이터 디스크가 함께 지워집니다. 이미지와 볼륨의 데이터는 되돌릴 수 없습니다."
    } else {
        "VM만 지워지고 데이터 디스크는 남습니다. 같은 이름으로 다시 만들면 이미지와 볼륨을 이어서 쓸 수 있습니다."
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountRow {
    pub path: String,
    /// `읽기·쓰기` or `읽기 전용`
    pub mode: &'static str,
}

pub fn mount_rows(mounts: &[Mount]) -> Vec<MountRow> {
    mounts
        .iter()
        .map(|m| MountRow {
            path: m.path.clone(),
            mode: if m.writable {
                "읽기·쓰기"
            } else {
                "읽기 전용"
            },
        })
        .collect()
}

pub const MOUNTS_EMPTY: &str = "마운트한 디렉터리가 없습니다.";
pub const MOUNTS_HELP: &str =
    "VM 안에서 볼 호스트 디렉터리입니다. 절대 경로로 적고, 호스트에 있는 폴더여야 합니다.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KubernetesCopy {
    pub label: &'static str,
    pub description: &'static str,
    pub available: bool,
    /// Why the switch is locked; `None` when it can be used.
    pub disabled_reason: Option<&'static str>,
}

pub fn kubernetes_copy(runtime: Runtime) -> KubernetesCopy {
    let available = runtime == KUBERNETES_RUNTIME;
    KubernetesCopy {
        label: "Kubernetes 사용",
        description: "k3s 클러스터를 VM 안에서 함께 시작합니다. 시작이 더 오래 걸립니다.",
        available,
        disabled_reason: (!available).then_some("Kubernetes는 docker 런타임에서만 쓸 수 있습니다."),
    }
}

/// Korean copy for the mount and Kubernetes rule violations; `None` for every other violation.
pub fn mount_violation_text(v: &RuleViolation) -> Option<String> {
    match v {
        RuleViolation::MountPathNotAbsolute(path) => Some(format!(
            "'{path}'은 절대 경로가 아닙니다. /로 시작해야 합니다"
        )),
        RuleViolation::DuplicateMount(path) => Some(format!("'{path}'이 두 번 이상 들어 있습니다")),
        RuleViolation::KubernetesNeedsDocker(_) => {
            Some("Kubernetes는 docker 런타임에서만 쓸 수 있습니다".to_owned())
        }
        _ => None,
    }
}

fn field_label(field: &str) -> &str {
    match field {
        "cpus" => "CPU",
        "memory_gib" => "메모리",
        "disk_gib" => "디스크",
        "mount_type" => "마운트 방식",
        "rosetta" => "Rosetta",
        "network_address" => "네트워크 주소",
        "mounts" => "마운트",
        "kubernetes" => "Kubernetes",
        other => other,
    }
}

/// Note under the form when saving needs a restart; `None` when no field needs one.
pub fn restart_required_text(fields: &[&str]) -> Option<String> {
    if fields.is_empty() {
        return None;
    }
    let names: Vec<&str> = fields.iter().map(|f| field_label(f)).collect();
    Some(format!(
        "{} 변경은 프로필을 재시작해야 적용됩니다.",
        names.join(", ")
    ))
}

pub fn confirm_prune_cache() -> Confirm {
    simple_confirm(
        "캐시 정리",
        "Colima가 내려받아 둔 VM 이미지 등 캐시 파일을 지웁니다. 프로필, 컨테이너, 볼륨에는 영향이 없고 필요하면 다음 시작 때 다시 내려받습니다.".to_owned(),
        "캐시 정리".to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use telima_core::container::{LogLine, LogStream, PortBinding, PortProtocol};
    use telima_core::profile::{DeleteOptions, Mount, Runtime, RuntimeInfo};
    use telima_core::version::{ColimaVersion, LAST_TESTED, MIN_SUPPORTED, VersionSupport};

    use super::*;

    fn port(host: Option<u16>, container: u16, protocol: PortProtocol) -> PortBinding {
        PortBinding {
            host_ip: host.map(|_| "0.0.0.0".to_owned()),
            host_port: host,
            container_port: container,
            protocol,
        }
    }

    fn line(text: &str) -> LogLine {
        LogLine {
            stream: LogStream::Stdout,
            text: text.into(),
            timestamp: None,
        }
    }

    #[test]
    fn footer_shows_colima_and_docker_versions() {
        let info = RuntimeInfo {
            colima: Some(ColimaVersion::new(0, 10, 3)),
            docker_client: Some("29.9.3".into()),
            docker_server: Some("29.5.2".into()),
            ..RuntimeInfo::default()
        };
        let f = version_footer(&info);
        assert_eq!(f.colima, "Colima 0.10.3");
        assert_eq!(f.docker, "Docker 29.5.2");
    }

    #[test]
    fn footer_falls_back_to_client_then_dash() {
        let client_only = RuntimeInfo {
            docker_client: Some("29.9.3".into()),
            ..RuntimeInfo::default()
        };
        assert_eq!(version_footer(&client_only).docker, "Docker 29.9.3");
        let none = version_footer(&RuntimeInfo::default());
        assert_eq!(
            (none.colima.as_str(), none.docker.as_str()),
            ("Colima —", "Docker —")
        );
    }

    #[test]
    fn supported_version_has_no_notice() {
        assert_eq!(version_notice(LAST_TESTED, VersionSupport::Supported), None);
    }

    #[test]
    fn too_old_version_blocks_with_upgrade_hint() {
        let n = version_notice(ColimaVersion::new(0, 9, 0), VersionSupport::TooOld).unwrap();
        assert_eq!(n.severity, VersionNoticeSeverity::Blocking);
        assert_eq!(n.title, "Colima 업데이트 필요");
        assert!(n.message.contains("0.9.0"));
        assert!(n.message.contains(&MIN_SUPPORTED.to_string()));
        assert_eq!(n.hint, Some("brew upgrade colima"));
    }

    #[test]
    fn untested_version_warns_softly() {
        let n = version_notice(ColimaVersion::new(0, 99, 0), VersionSupport::Untested).unwrap();
        assert_eq!(n.severity, VersionNoticeSeverity::Warning);
        assert_eq!(n.title, "검증되지 않은 Colima 버전");
        assert!(n.message.contains("0.99.0"));
        assert!(n.message.contains(&LAST_TESTED.to_string()));
    }

    #[test]
    fn ports_use_the_short_form() {
        assert_eq!(
            ports_text(&[port(Some(8080), 8080, PortProtocol::Tcp)]),
            "8080→8080/tcp"
        );
        assert_eq!(
            ports_text(&[port(Some(5353), 53, PortProtocol::Udp)]),
            "5353→53/udp"
        );
    }

    #[test]
    fn unpublished_ports_show_container_side_only() {
        assert_eq!(
            ports_text(&[port(None, 5432, PortProtocol::Tcp)]),
            "5432/tcp"
        );
    }

    #[test]
    fn ipv4_and_ipv6_duplicates_collapse_and_order_is_kept() {
        let ports = [
            port(Some(8080), 8080, PortProtocol::Tcp),
            port(Some(8080), 8080, PortProtocol::Tcp),
            port(Some(443), 8443, PortProtocol::Tcp),
        ];
        assert_eq!(ports_text(&ports), "8080→8080/tcp, 443→8443/tcp");
    }

    #[test]
    fn no_ports_is_a_dash() {
        assert_eq!(ports_text(&[]), "—");
    }

    #[test]
    fn log_filter_is_case_insensitive_substring() {
        let lines = [line("GET /Health 200"), line("started"), line("ERROR boom")];
        let hits = filter_log(&lines, "error");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].text, "ERROR boom");
        assert_eq!(filter_log(&lines, "HEALTH").len(), 1);
        assert_eq!(filter_log(&lines, "nope").len(), 0);
    }

    #[test]
    fn empty_or_blank_filter_keeps_every_line() {
        let lines = [line("a"), line("b")];
        assert_eq!(filter_log(&lines, "").len(), 2);
        assert_eq!(filter_log(&lines, "  ").len(), 2);
        assert!(log_matches("한글 로그", "로그"));
    }

    #[test]
    fn delete_data_option_is_off_by_default_and_explained() {
        let c = delete_data_option();
        assert_eq!(c.label, "데이터 디스크도 삭제");
        assert!(c.description.contains("이미지"));
        assert!(c.description.contains("볼륨"));
        assert!(!DeleteOptions::default().data);
    }

    #[test]
    fn delete_dialog_says_what_stays_or_goes_once() {
        // F9: body, switch description and consequence line must not repeat each other.
        let name = telima_core::profile::ProfileName::parse("dev").unwrap();
        for data in [false, true] {
            let options = DeleteOptions { data };
            let body = crate::view::confirm_delete_profile(&name, 2, &[None], options).body;
            let all = format!(
                "{body} {} {}",
                delete_data_option().description,
                delete_consequence(options)
            );
            for phrase in ["데이터 디스크", "남습니다", "되돌릴 수 없습니다"] {
                assert!(
                    all.matches(phrase).count() <= 1,
                    "{phrase:?} repeats: {all}"
                );
            }
        }
    }

    #[test]
    fn delete_consequence_differs_by_option() {
        let keep = delete_consequence(DeleteOptions { data: false });
        let wipe = delete_consequence(DeleteOptions { data: true });
        assert!(keep.contains("남습니다"));
        assert!(wipe.contains("되돌릴 수 없습니다"));
        assert_ne!(keep, wipe);
    }

    #[test]
    fn mount_rows_show_access_mode() {
        let rows = mount_rows(&[
            Mount {
                path: "/Users/me/work".into(),
                writable: true,
            },
            Mount {
                path: "/data/ro".into(),
                writable: false,
            },
        ]);
        assert_eq!(rows[0].path, "/Users/me/work");
        assert_eq!(rows[0].mode, "읽기·쓰기");
        assert_eq!(rows[1].mode, "읽기 전용");
    }

    #[test]
    fn mounts_have_empty_and_help_copy() {
        assert_eq!(MOUNTS_EMPTY, "마운트한 디렉터리가 없습니다.");
        assert!(MOUNTS_HELP.contains("절대 경로"));
    }

    #[test]
    fn kubernetes_copy_explains_and_gates_on_docker() {
        let on = kubernetes_copy(Runtime::Docker);
        assert_eq!(on.label, "Kubernetes 사용");
        assert!(on.available);
        assert_eq!(on.disabled_reason, None);
        let off = kubernetes_copy(Runtime::Containerd);
        assert!(!off.available);
        assert!(off.disabled_reason.unwrap().contains("docker"));
    }

    #[test]
    fn mount_and_kubernetes_violations_have_copy() {
        let abs = mount_violation_text(&RuleViolation::MountPathNotAbsolute("work".into()));
        assert!(abs.unwrap().contains("절대 경로"));
        let dup = mount_violation_text(&RuleViolation::DuplicateMount("/a".into()));
        assert!(dup.unwrap().contains("/a"));
        let k8s = mount_violation_text(&RuleViolation::KubernetesNeedsDocker(Runtime::Containerd));
        assert!(k8s.unwrap().contains("docker"));
        assert_eq!(
            mount_violation_text(&RuleViolation::RosettaUnavailable),
            None
        );
    }

    #[test]
    fn restart_notice_names_changed_fields() {
        assert_eq!(restart_required_text(&[]), None);
        let t = restart_required_text(&["mounts", "kubernetes"]).unwrap();
        assert!(t.contains("마운트") && t.contains("Kubernetes"));
        assert!(t.contains("재시작"));
    }

    #[test]
    fn prune_cache_confirm_says_what_it_does() {
        let c = confirm_prune_cache();
        assert_eq!(c.title, "캐시 정리");
        assert!(c.body.contains("내려받"));
        assert!(c.body.contains("컨테이너") && c.body.contains("영향"));
        assert_eq!(c.confirm_label, "캐시 정리");
        assert!(c.can_confirm);
        assert_eq!(c.typed_name, None);
    }
}
