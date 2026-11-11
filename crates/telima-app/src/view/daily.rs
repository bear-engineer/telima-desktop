//! L6 copy: versions, ports, log filter, delete data option, mounts, Kubernetes, cache prune.

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
        assert_eq!((none.colima.as_str(), none.docker.as_str()), ("Colima —", "Docker —"));
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
        assert_eq!(ports_text(&[port(None, 5432, PortProtocol::Tcp)]), "5432/tcp");
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
            Mount { path: "/Users/me/work".into(), writable: true },
            Mount { path: "/data/ro".into(), writable: false },
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
