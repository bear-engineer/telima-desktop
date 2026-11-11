//! Pure view logic of the engine panel: filter, row actions, status shapes, volume chips and
//! log line parsing. Kept free of GPUI so it is unit-tested directly.
use telima_app::view::{ChipKind, ContainerRow, VolumeRow};
use telima_core::container::ContainerState;

use crate::components::status::Status;

/// Status group of the containers toolbar (전체 / 실행 / 종료).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatusFilter {
    #[default]
    All,
    Running,
    Exited,
}

impl StatusFilter {
    pub const ALL: [StatusFilter; 3] = [
        StatusFilter::All,
        StatusFilter::Running,
        StatusFilter::Exited,
    ];

    pub fn label(self) -> &'static str {
        unimplemented!()
    }

    pub fn matches(self, state: ContainerState) -> bool {
        let _ = state;
        unimplemented!()
    }
}

/// Containers per toolbar group, for the segment counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FilterCounts {
    pub all: usize,
    pub running: usize,
    pub exited: usize,
}

impl FilterCounts {
    pub fn of(rows: &[ContainerRow]) -> Self {
        let _ = rows;
        unimplemented!()
    }

    pub fn get(self, f: StatusFilter) -> usize {
        let _ = f;
        unimplemented!()
    }
}

/// Rows matching the status group and the text query (name or image, case-insensitive).
pub fn filter_rows<'a>(
    rows: &'a [ContainerRow],
    query: &str,
    filter: StatusFilter,
) -> Vec<&'a ContainerRow> {
    let _ = (rows, query, filter);
    unimplemented!()
}

/// Lifecycle actions a container row offers in its state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RowActions {
    pub start: bool,
    pub stop: bool,
    pub restart: bool,
}

pub fn row_actions(state: ContainerState) -> RowActions {
    let _ = state;
    unimplemented!()
}

/// Status shape of a container row. A non-zero exit is an error (triangle).
pub fn container_status(row: &ContainerRow) -> Status {
    let _ = row;
    unimplemented!()
}

/// Status shown by a volume usage chip. Unknown usage draws the idle chip.
pub fn volume_chip(row: &VolumeRow) -> Status {
    let _ = row;
    unimplemented!()
}

/// How a log line is colored. Only error-level lines are red, whatever the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineTone {
    Normal,
    Error,
}

pub fn line_tone(text: &str) -> LineTone {
    let _ = text;
    unimplemented!()
}

/// Splits a leading Docker RFC 3339 timestamp (`2026-11-10T09:14:02.118123Z msg`) into
/// `09:14:02.118` and the message. Lines without one are returned whole.
pub fn split_timestamp(text: &str) -> (Option<String>, &str) {
    (None, text)
}

/// `1204` -> `1,204`.
pub fn group_thousands(n: usize) -> String {
    n.to_string()
}

/// Ids of the exited containers the "종료된 N개 삭제" button removes.
pub fn exited_ids(rows: &[ContainerRow]) -> Vec<String> {
    let _ = rows;
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use telima_app::view::{VolumeUsage, container_rows, volume_rows};
    use telima_core::container::{Container, Volume};

    use super::*;

    fn rows() -> Vec<ContainerRow> {
        let c = |id: &str, name: &str, image: &str, state, raw: &str| Container {
            id: id.into(),
            name: name.into(),
            image: image.into(),
            state,
            status_text: raw.into(),
        };
        container_rows(&[
            c(
                "a1",
                "pg-main",
                "postgres:17",
                ContainerState::Running,
                "Up 3 hours",
            ),
            c(
                "b2",
                "cache",
                "redis:7",
                ContainerState::Running,
                "Up 3 hours",
            ),
            c(
                "c3",
                "migrate",
                "alpine:3.20",
                ContainerState::Exited,
                "Exited (0) 1 hour ago",
            ),
            c(
                "d4",
                "worker",
                "alpine:3.20",
                ContainerState::Exited,
                "Exited (1) 1 hour ago",
            ),
        ])
    }

    fn names(v: Vec<&ContainerRow>) -> Vec<&str> {
        v.into_iter().map(|r| r.name.as_str()).collect()
    }

    #[test]
    fn filter_by_status_group() {
        let rows = rows();
        assert_eq!(filter_rows(&rows, "", StatusFilter::All).len(), 4);
        assert_eq!(
            names(filter_rows(&rows, "", StatusFilter::Running)),
            ["pg-main", "cache"]
        );
        assert_eq!(
            names(filter_rows(&rows, "", StatusFilter::Exited)),
            ["migrate", "worker"]
        );
    }

    #[test]
    fn filter_by_name_or_image_ignoring_case() {
        let rows = rows();
        assert_eq!(
            names(filter_rows(&rows, "PG", StatusFilter::All)),
            ["pg-main"]
        );
        assert_eq!(
            names(filter_rows(&rows, "alpine", StatusFilter::All)),
            ["migrate", "worker"]
        );
        assert_eq!(
            names(filter_rows(&rows, " alpine ", StatusFilter::Running)),
            Vec::<&str>::new()
        );
    }

    #[test]
    fn counts_per_group_and_labels() {
        let c = FilterCounts::of(&rows());
        assert_eq!((c.all, c.running, c.exited), (4, 2, 2));
        assert_eq!(c.get(StatusFilter::Exited), 2);
        let labels: Vec<_> = StatusFilter::ALL.iter().map(|f| f.label()).collect();
        assert_eq!(labels, ["전체", "실행", "종료"]);
        assert_eq!(exited_ids(&rows()), ["c3", "d4"]);
    }

    #[test]
    fn actions_are_valid_for_the_state() {
        let running = row_actions(ContainerState::Running);
        assert!(running.stop && running.restart && !running.start);
        let exited = row_actions(ContainerState::Exited);
        assert!(exited.start && !exited.stop && !exited.restart);
        let created = row_actions(ContainerState::Created);
        assert!(created.start && !created.restart);
        let paused = row_actions(ContainerState::Paused);
        assert!(paused.stop && !paused.start);
        let dead = row_actions(ContainerState::Dead);
        assert!(!dead.stop && !dead.restart);
    }

    #[test]
    fn status_shape_marks_non_zero_exit_as_error() {
        let shapes: Vec<_> = rows().iter().map(container_status).collect();
        assert_eq!(
            shapes,
            [
                Status::Running,
                Status::Running,
                Status::Stopped,
                Status::Error
            ]
        );
    }

    #[test]
    fn volume_chip_is_tri_state() {
        let v = |name: &str, refs| Volume {
            name: name.into(),
            size_bytes: Some(1),
            ref_count: refs,
        };
        let rows = volume_rows(&[v("used", Some(2)), v("free", Some(0)), v("unknown", None)]);
        let got: Vec<_> = rows
            .iter()
            .map(|r| (r.usage, volume_chip(r), r.in_use_label))
            .collect();
        assert_eq!(
            got,
            [
                (VolumeUsage::InUse, Status::Running, "사용 중"),
                (VolumeUsage::Unused, Status::Stopped, "미사용"),
                (VolumeUsage::Unknown, Status::Stopped, "확인 불가"),
            ]
        );
    }

    #[test]
    fn only_error_level_lines_are_red() {
        assert_eq!(line_tone("LOG:  listening on port 5432"), LineTone::Normal);
        assert_eq!(
            line_tone("2026/11/10 09:14:02 [notice] 1#1: start worker"),
            LineTone::Normal
        );
        assert_eq!(
            line_tone("ERROR:  relation does not exist"),
            LineTone::Error
        );
        assert_eq!(
            line_tone("time=\"x\" level=error msg=\"boom\""),
            LineTone::Error
        );
        assert_eq!(line_tone("FATAL: password failed"), LineTone::Error);
        assert_eq!(line_tone("0 errors found"), LineTone::Normal);
    }

    #[test]
    fn docker_timestamp_is_split_off() {
        assert_eq!(
            split_timestamp("2026-11-10T09:14:02.118123456Z LOG: ready"),
            (Some("09:14:02.118".to_string()), "LOG: ready")
        );
        assert_eq!(
            split_timestamp("2026-11-10T09:14:02Z hi"),
            (Some("09:14:02.000".to_string()), "hi")
        );
        assert_eq!(split_timestamp("plain line"), (None, "plain line"));
        assert_eq!(group_thousands(1204), "1,204");
        assert_eq!(group_thousands(12), "12");
        assert_eq!(group_thousands(1_000_000), "1,000,000");
    }
}
