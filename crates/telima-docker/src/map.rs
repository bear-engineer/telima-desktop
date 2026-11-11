//! Pure mappings from bollard Docker Engine API models to telima-core entities.
//! No I/O here: every function takes an already-decoded engine model.

use std::collections::HashMap;

use bollard::container::LogOutput;
use bollard::models::{
    ContainerSummary, ContainerSummaryStateEnum, ImagePruneResponse, ImageSummary,
    SystemDataUsageResponse, Volume, VolumePruneResponse,
};
use telima_core::container::{
    Container, ContainerState, DiskUsage, Image, LogLine, LogStream, PruneReport,
    Volume as DomainVolume,
};

/// Engine sizes and counts are signed; negatives mean "unknown" and map to zero or `None`.
fn non_negative(v: i64) -> u64 {
    u64::try_from(v).unwrap_or(0)
}

fn known_count(v: i64) -> Option<u64> {
    u64::try_from(v).ok()
}

/// Container summary to the domain container. Engine names carry a leading `/`.
pub fn map_container(summary: &ContainerSummary) -> Container {
    let name = summary
        .names
        .as_ref()
        .and_then(|names| names.first())
        .map(|n| n.strip_prefix('/').unwrap_or(n).to_owned())
        .unwrap_or_default();
    let state = summary
        .state
        .map(|s: ContainerSummaryStateEnum| ContainerState::parse(s.as_ref()))
        .unwrap_or(ContainerState::Other);
    Container {
        id: summary.id.clone().unwrap_or_default(),
        name,
        image: summary.image.clone().unwrap_or_default(),
        state,
        status_text: summary.status.clone().unwrap_or_default(),
    }
}

/// Image summary to the domain image. A negative container count means unknown.
pub fn map_image(summary: &ImageSummary) -> Image {
    Image {
        id: summary.id.clone(),
        tags: summary.repo_tags.clone(),
        size_bytes: non_negative(summary.size),
        containers: known_count(summary.containers),
    }
}

/// Volume to the domain volume. Usage data is optional; negative values mean unknown.
pub fn map_volume(volume: &Volume) -> DomainVolume {
    let usage = volume.usage_data.as_ref();
    DomainVolume {
        name: volume.name.clone(),
        size_bytes: usage.and_then(|u| known_count(u.size)),
        ref_count: usage.and_then(|u| known_count(u.ref_count)),
    }
}

/// Volume list merged with per-name usage from `df`, as `(name, size, ref_count)`.
/// The list endpoint leaves usage empty, so `df` supplies it. Usage already on a
/// listed volume wins; a name missing from `df` stays unknown.
pub fn map_volumes(list: &[Volume], df: &[(String, i64, i64)]) -> Vec<DomainVolume> {
    let by_name: HashMap<&str, (i64, i64)> = df
        .iter()
        .map(|(name, size, refs)| (name.as_str(), (*size, *refs)))
        .collect();
    list.iter()
        .map(|volume| {
            let usage = volume
                .usage_data
                .as_ref()
                .map(|u| (u.size, u.ref_count))
                .or_else(|| by_name.get(volume.name.as_str()).copied());
            DomainVolume {
                name: volume.name.clone(),
                size_bytes: usage.and_then(|(size, _)| known_count(size)),
                ref_count: usage.and_then(|(_, refs)| known_count(refs)),
            }
        })
        .collect()
}

/// System df response to disk usage per section; missing sections count as zero.
pub fn map_disk_usage(df: &SystemDataUsageResponse) -> DiskUsage {
    DiskUsage {
        images_bytes: df
            .image_usage
            .as_ref()
            .and_then(|u| u.total_size)
            .map_or(0, non_negative),
        containers_bytes: df
            .container_usage
            .as_ref()
            .and_then(|u| u.total_size)
            .map_or(0, non_negative),
        volumes_bytes: df
            .volume_usage
            .as_ref()
            .and_then(|u| u.total_size)
            .map_or(0, non_negative),
        build_cache_bytes: df
            .build_cache_usage
            .as_ref()
            .and_then(|u| u.total_size)
            .map_or(0, non_negative),
    }
}

/// Image prune response to a report. Each removal prefers the deleted id, then the untagged ref.
pub fn map_image_prune(resp: &ImagePruneResponse) -> PruneReport {
    let removed = resp
        .images_deleted
        .iter()
        .flatten()
        .filter_map(|item| item.deleted.clone().or_else(|| item.untagged.clone()))
        .collect();
    PruneReport {
        removed,
        reclaimed_bytes: resp.space_reclaimed.map_or(0, non_negative),
    }
}

/// Volume prune response to a report.
pub fn map_volume_prune(resp: &VolumePruneResponse) -> PruneReport {
    PruneReport {
        removed: resp.volumes_deleted.clone().unwrap_or_default(),
        reclaimed_bytes: resp.space_reclaimed.map_or(0, non_negative),
    }
}

/// One log chunk to a log line. Trailing newline is trimmed; invalid UTF-8 is replaced.
/// Stdin echoes are not log output and yield `None`.
pub fn map_log_output(output: &LogOutput) -> Option<LogLine> {
    let (stream, bytes) = match output {
        LogOutput::StdOut { message } | LogOutput::Console { message } => {
            (LogStream::Stdout, message)
        }
        LogOutput::StdErr { message } => (LogStream::Stderr, message),
        LogOutput::StdIn { .. } => return None,
    };
    let text = String::from_utf8_lossy(bytes);
    let trimmed = text.strip_suffix('\n').unwrap_or(&text);
    Some(LogLine {
        stream,
        text: trimmed.to_owned(),
    })
}

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
            usage_data: Some(VolumeUsageData {
                size: 4096,
                ref_count: 0,
            }),
            ..Default::default()
        };
        let vol = map_volume(&v);
        assert_eq!(vol.size_bytes, Some(4096));
        assert_eq!(vol.ref_count, Some(0));
        assert!(vol.is_unused());
    }

    fn named(name: &str) -> Volume {
        Volume {
            name: name.into(),
            ..Default::default()
        }
    }

    fn usage(name: &str, size: i64, ref_count: i64) -> (String, i64, i64) {
        (name.to_owned(), size, ref_count)
    }

    #[test]
    fn list_volumes_take_size_and_ref_count_from_df_by_name() {
        let list = vec![named("pgdata"), named("cache")];
        let df = vec![usage("cache", 512, 0), usage("pgdata", 4096, 2)];
        let out = map_volumes(&list, &df);
        assert_eq!(out[0].name, "pgdata");
        assert_eq!(out[0].size_bytes, Some(4096));
        assert_eq!(out[0].ref_count, Some(2));
        assert_eq!(out[1].size_bytes, Some(512));
        assert_eq!(out[1].ref_count, Some(0));
        assert!(out[1].is_unused());
    }

    #[test]
    fn volume_missing_from_df_stays_unknown() {
        let out = map_volumes(&[named("orphan")], &[usage("other", 1, 1)]);
        assert_eq!(out[0].size_bytes, None);
        assert_eq!(out[0].ref_count, None);
    }

    #[test]
    fn empty_df_keeps_list_volumes_unknown() {
        let out = map_volumes(&[named("data")], &[]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].size_bytes, None);
        assert_eq!(out[0].ref_count, None);
    }

    #[test]
    fn list_usage_wins_over_df() {
        let listed = Volume {
            name: "data".into(),
            usage_data: Some(VolumeUsageData {
                size: 10,
                ref_count: 1,
            }),
            ..Default::default()
        };
        let out = map_volumes(&[listed], &[usage("data", 999, 9)]);
        assert_eq!(out[0].size_bytes, Some(10));
        assert_eq!(out[0].ref_count, Some(1));
    }

    #[test]
    fn df_sentinel_minus_one_is_unknown() {
        let out = map_volumes(&[named("x")], &[usage("x", -1, -1)]);
        assert_eq!(out[0].size_bytes, None);
        assert_eq!(out[0].ref_count, None);
    }

    #[test]
    fn volume_usage_sentinel_minus_one_is_unknown() {
        let v = Volume {
            name: "x".into(),
            usage_data: Some(VolumeUsageData {
                size: -1,
                ref_count: -1,
            }),
            ..Default::default()
        };
        let vol = map_volume(&v);
        assert_eq!(vol.size_bytes, None);
        assert_eq!(vol.ref_count, None);
    }

    #[test]
    fn disk_usage_maps_each_section_total_size() {
        let df = SystemDataUsageResponse {
            image_usage: Some(ImagesDiskUsage {
                total_size: Some(100),
                ..Default::default()
            }),
            container_usage: Some(ContainersDiskUsage {
                total_size: Some(20),
                ..Default::default()
            }),
            volume_usage: Some(VolumesDiskUsage {
                total_size: Some(300),
                ..Default::default()
            }),
            build_cache_usage: Some(BuildCacheDiskUsage {
                total_size: Some(4),
                ..Default::default()
            }),
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
        let out = LogOutput::StdOut {
            message: "hello\n".into(),
        };
        let err = LogOutput::StdErr {
            message: "boom".into(),
        };
        assert_eq!(
            map_log_output(&out),
            Some(LogLine {
                stream: LogStream::Stdout,
                text: "hello".into()
            })
        );
        assert_eq!(
            map_log_output(&err),
            Some(LogLine {
                stream: LogStream::Stderr,
                text: "boom".into()
            })
        );
    }

    #[test]
    fn console_output_maps_to_stdout() {
        let c = LogOutput::Console {
            message: "tty\n".into(),
        };
        assert_eq!(
            map_log_output(&c),
            Some(LogLine {
                stream: LogStream::Stdout,
                text: "tty".into()
            })
        );
    }

    #[test]
    fn stdin_echo_is_not_a_log_line() {
        let i = LogOutput::StdIn {
            message: "typed\n".into(),
        };
        assert_eq!(map_log_output(&i), None);
    }

    #[test]
    fn invalid_utf8_is_replaced_not_dropped() {
        let out = LogOutput::StdOut {
            message: vec![0x66, 0x6f, 0xff].into(),
        };
        let line = map_log_output(&out).expect("stdout line");
        assert_eq!(line.text, "fo\u{fffd}");
    }
}
