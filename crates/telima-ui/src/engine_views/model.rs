//! Pure view logic of the engine panel: filter, row actions, status shapes, volume chips and
//! log line parsing. Kept free of GPUI so it is unit-tested directly.
use telima_app::view::{ChipKind, ContainerRow, VolumeRow};
use telima_core::container::ContainerState;

use crate::components::status::Status;

/// Prefix of `container_status_text` for a non-zero exit (telima-app view copy).
const ERROR_PREFIX: &str = "오류";

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
        match self {
            StatusFilter::All => "전체",
            StatusFilter::Running => "실행",
            StatusFilter::Exited => "종료",
        }
    }

    pub fn matches(self, state: ContainerState) -> bool {
        match self {
            StatusFilter::All => true,
            StatusFilter::Running => matches!(
                state,
                ContainerState::Running | ContainerState::Paused | ContainerState::Restarting
            ),
            StatusFilter::Exited => matches!(state, ContainerState::Exited | ContainerState::Dead),
        }
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
        let n = |f: StatusFilter| rows.iter().filter(|r| f.matches(r.state)).count();
        Self {
            all: rows.len(),
            running: n(StatusFilter::Running),
            exited: n(StatusFilter::Exited),
        }
    }

    pub fn get(self, f: StatusFilter) -> usize {
        match f {
            StatusFilter::All => self.all,
            StatusFilter::Running => self.running,
            StatusFilter::Exited => self.exited,
        }
    }
}

/// Rows matching the status group and the text query (name or image, case-insensitive).
pub fn filter_rows<'a>(
    rows: &'a [ContainerRow],
    query: &str,
    filter: StatusFilter,
) -> Vec<&'a ContainerRow> {
    let q = query.trim().to_lowercase();
    rows.iter()
        .filter(|r| filter.matches(r.state))
        .filter(|r| {
            q.is_empty()
                || r.name.to_lowercase().contains(&q)
                || r.image.to_lowercase().contains(&q)
        })
        .collect()
}

/// Lifecycle actions a container row offers in its state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RowActions {
    pub start: bool,
    pub stop: bool,
    pub restart: bool,
}

pub fn row_actions(state: ContainerState) -> RowActions {
    match state {
        ContainerState::Running => RowActions {
            start: false,
            stop: true,
            restart: true,
        },
        ContainerState::Paused | ContainerState::Restarting => RowActions {
            stop: true,
            ..RowActions::default()
        },
        ContainerState::Created | ContainerState::Exited => RowActions {
            start: true,
            ..RowActions::default()
        },
        _ => RowActions::default(),
    }
}

/// Status shape of a container row. A non-zero exit is an error (triangle).
pub fn container_status(row: &ContainerRow) -> Status {
    match row.state {
        ContainerState::Running => Status::Running,
        ContainerState::Restarting => Status::Busy,
        ContainerState::Dead => Status::Error,
        _ if row.status_text.starts_with(ERROR_PREFIX) => Status::Error,
        _ => Status::Stopped,
    }
}

/// Status shown by a volume usage chip. Unknown usage draws the idle chip.
pub fn volume_chip(row: &VolumeRow) -> Status {
    match row.chip {
        ChipKind::Ok => Status::Running,
        ChipKind::Idle => Status::Stopped,
    }
}

/// How a log line is colored. Only error-level lines are red, whatever the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineTone {
    Normal,
    Error,
}

/// Level words that mark a line as an error (postgres, logfmt, Go, nginx `[error]`).
const ERROR_LEVELS: [&str; 6] = ["error", "fatal", "panic", "crit", "critical", "emerg"];

pub fn line_tone(text: &str) -> LineTone {
    let logfmt = text
        .split_whitespace()
        .filter_map(|w| w.strip_prefix("level="))
        .any(|l| ERROR_LEVELS.contains(&l.trim_matches('"').to_lowercase().as_str()));
    let leading = text
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .take(6)
        .any(|w| {
            let upper = w.chars().all(|c| c.is_ascii_uppercase());
            let bracketed = text.contains(&format!("[{w}]"));
            (upper || bracketed) && ERROR_LEVELS.contains(&w.to_lowercase().as_str())
        });
    if logfmt || leading {
        LineTone::Error
    } else {
        LineTone::Normal
    }
}

/// Splits a leading Docker RFC 3339 timestamp (`2026-11-10T09:14:02.118123Z msg`) into
/// `09:14:02.118` and the message. Lines without one are returned whole.
pub fn split_timestamp(text: &str) -> (Option<String>, &str) {
    let Some((stamp, rest)) = text.split_once(' ') else {
        return (None, text);
    };
    let b = stamp.as_bytes();
    let shaped = b.len() >= 20
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && b[16] == b':'
        && stamp.ends_with('Z')
        && [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18]
            .iter()
            .all(|&i| b[i].is_ascii_digit());
    if !shaped {
        return (None, text);
    }
    let frac: String = stamp[19..stamp.len() - 1]
        .trim_start_matches('.')
        .chars()
        .chain(std::iter::repeat('0'))
        .take(3)
        .collect();
    (Some(format!("{}.{frac}", &stamp[11..19])), rest)
}

/// `1204` -> `1,204`.
pub fn group_thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Ids of the exited containers the "종료된 N개 삭제" button removes.
pub fn exited_ids(rows: &[ContainerRow]) -> Vec<String> {
    rows.iter()
        .filter(|r| StatusFilter::Exited.matches(r.state))
        .map(|r| r.id.clone())
        .collect()
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
