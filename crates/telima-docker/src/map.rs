//! Pure mappings from bollard Docker Engine API models to telima-core entities.
//! No I/O here: every function takes an already-decoded engine model.

#[cfg(test)]
mod tests {
    use super::*;
    use bollard::container::LogOutput;
    use bollard::models::{
        BuildCacheDiskUsage, ContainerSummary, ContainerSummaryStateEnum, ContainersDiskUsage,
        ImageDeleteResponseItem, ImagePruneResponse, ImageSummary, ImagesDiskUsage,
        SystemDataUsageResponse, Volume, VolumePruneResponse, VolumeUsageData, VolumesDiskUsage,
    };
    use telima_core::container::{ContainerState, LogStream};

    #[test]
    fn container_name_drops_leading_slash_and_maps_state() {
        let summary = ContainerSummary {
            id: Some("abc123".into()),
            names: Some(vec!["/web".into()]),
            image: Some("nginx:1.27".into()),
            state: Some(ContainerSummaryStateEnum::RUNNING),
            status: Some("Up 2 minutes".into()),
            ..Default::default()
        };
        let c = map_container(&summary);
        assert_eq!(c.id, "abc123");
        assert_eq!(c.name, "web");
        assert_eq!(c.image, "nginx:1.27");
        assert_eq!(c.state, ContainerState::Running);
        assert_eq!(c.status_text, "Up 2 minutes");
    }

    #[test]
    fn container_missing_fields_fall_back_to_empty_and_other() {
        let c = map_container(&ContainerSummary::default());
        assert_eq!(c.id, "");
        assert_eq!(c.name, "");
        assert_eq!(c.image, "");
        assert_eq!(c.state, ContainerState::Other);
        assert_eq!(c.status_text, "");
    }

    #[test]
    fn container_exited_state_is_parsed() {
        let summary = ContainerSummary {
            state: Some(ContainerSummaryStateEnum::EXITED),
            ..Default::default()
        };
        assert_eq!(map_container(&summary).state, ContainerState::Exited);
    }

    #[test]
    fn image_maps_tags_size_and_known_container_count() {
        let summary = ImageSummary {
            id: "sha256:aaa".into(),
            repo_tags: vec!["alpine:3".into(), "alpine:latest".into()],
            size: 7_340_032,
            containers: 2,
            ..Default::default()
        };
        let img = map_image(&summary);
        assert_eq!(img.id, "sha256:aaa");
        assert_eq!(img.tags, vec!["alpine:3", "alpine:latest"]);
        assert_eq!(img.size_bytes, 7_340_032);
        assert_eq!(img.containers, Some(2));
    }

    #[test]
    fn image_negative_container_count_is_unknown() {
        let summary = ImageSummary {
            containers: -1,
            size: -5,
            ..Default::default()
        };
        let img = map_image(&summary);
        assert_eq!(img.containers, None);
        assert_eq!(img.size_bytes, 0);
    }

    #[test]
    fn volume_without_usage_data_has_unknown_size_and_refs() {
        let v = Volume {
            name: "data".into(),
            usage_data: None,
            ..Default::default()
        };
        let vol = map_volume(&v);
        assert_eq!(vol.name, "data");
        assert_eq!(vol.size_bytes, None);
        assert_eq!(vol.ref_count, None);
    }

    #[test]
    fn volume_usage_data_maps_size_and_ref_count() {
        let v = Volume {
            name: "pgdata".into(),
            usage_data: Some(VolumeUsageData { size: 4096, ref_count: 0 }),
            ..Default::default()
        };
        let vol = map_volume(&v);
        assert_eq!(vol.size_bytes, Some(4096));
        assert_eq!(vol.ref_count, Some(0));
        assert!(vol.is_unused());
    }

    #[test]
    fn volume_usage_sentinel_minus_one_is_unknown() {
        let v = Volume {
            name: "x".into(),
            usage_data: Some(VolumeUsageData { size: -1, ref_count: -1 }),
            ..Default::default()
        };
        let vol = map_volume(&v);
        assert_eq!(vol.size_bytes, None);
        assert_eq!(vol.ref_count, None);
    }

    #[test]
    fn disk_usage_maps_each_section_total_size() {
        let df = SystemDataUsageResponse {
            image_usage: Some(ImagesDiskUsage { total_size: Some(100), ..Default::default() }),
            container_usage: Some(ContainersDiskUsage { total_size: Some(20), ..Default::default() }),
            volume_usage: Some(VolumesDiskUsage { total_size: Some(300), ..Default::default() }),
            build_cache_usage: Some(BuildCacheDiskUsage { total_size: Some(4), ..Default::default() }),
            ..Default::default()
        };
        let usage = map_disk_usage(&df);
        assert_eq!(usage.images_bytes, 100);
        assert_eq!(usage.containers_bytes, 20);
        assert_eq!(usage.volumes_bytes, 300);
        assert_eq!(usage.build_cache_bytes, 4);
    }

    #[test]
    fn disk_usage_missing_sections_are_zero() {
        let usage = map_disk_usage(&SystemDataUsageResponse::default());
        assert_eq!(usage, DiskUsage::default());
    }

    #[test]
    fn image_prune_lists_deleted_ids_and_space() {
        let resp = ImagePruneResponse {
            images_deleted: Some(vec![
                ImageDeleteResponseItem {
                    deleted: Some("sha256:one".into()),
                    untagged: None,
                },
                ImageDeleteResponseItem {
                    deleted: None,
                    untagged: Some("old:tag".into()),
                },
            ]),
            space_reclaimed: Some(2048),
        };
        let report = map_image_prune(&resp);
        assert_eq!(report.removed, vec!["sha256:one", "old:tag"]);
        assert_eq!(report.reclaimed_bytes, 2048);
    }

    #[test]
    fn volume_prune_lists_names_and_space() {
        let resp = VolumePruneResponse {
            volumes_deleted: Some(vec!["a".into(), "b".into()]),
            space_reclaimed: Some(10),
        };
        let report = map_volume_prune(&resp);
        assert_eq!(report.removed, vec!["a", "b"]);
        assert_eq!(report.reclaimed_bytes, 10);
    }

    #[test]
    fn empty_prune_response_is_empty_report() {
        assert_eq!(
            map_volume_prune(&VolumePruneResponse::default()),
            PruneReport::default()
        );
        assert_eq!(
            map_image_prune(&ImagePruneResponse::default()),
            PruneReport::default()
        );
    }

    #[test]
    fn negative_reclaimed_space_clamps_to_zero() {
        let resp = VolumePruneResponse {
            volumes_deleted: None,
            space_reclaimed: Some(-1),
        };
        assert_eq!(map_volume_prune(&resp).reclaimed_bytes, 0);
    }

    #[test]
    fn stdout_and_stderr_chunks_map_to_log_lines() {
        let out = LogOutput::StdOut { message: "hello\n".into() };
        let err = LogOutput::StdErr { message: "boom".into() };
        assert_eq!(
            map_log_output(&out),
            Some(LogLine { stream: LogStream::Stdout, text: "hello".into() })
        );
        assert_eq!(
            map_log_output(&err),
            Some(LogLine { stream: LogStream::Stderr, text: "boom".into() })
        );
    }

    #[test]
    fn console_output_maps_to_stdout() {
        let c = LogOutput::Console { message: "tty\n".into() };
        assert_eq!(
            map_log_output(&c),
            Some(LogLine { stream: LogStream::Stdout, text: "tty".into() })
        );
    }

    #[test]
    fn stdin_echo_is_not_a_log_line() {
        let i = LogOutput::StdIn { message: "typed\n".into() };
        assert_eq!(map_log_output(&i), None);
    }

    #[test]
    fn invalid_utf8_is_replaced_not_dropped() {
        let out = LogOutput::StdOut { message: vec![0x66, 0x6f, 0xff].into() };
        let line = map_log_output(&out).expect("stdout line");
        assert_eq!(line.text, "fo\u{fffd}");
    }
}
