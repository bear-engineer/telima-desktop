//! Copy of the lifecycle progress panel (canvas `Starting.dc.html`): status, log title,
//! elapsed time and cancel. One definition per string; telima-ui only renders it.

use crate::profiles::Lifecycle;

/// Where a lifecycle command is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressOutcome {
    Running,
    Done,
    Failed,
    Cancelled,
}

/// Status of the command, e.g. `시작 중`, `중지됨`, `실패`.
pub fn progress_status(cmd: Lifecycle, outcome: ProgressOutcome) -> &'static str {
    match (cmd, outcome) {
        (_, ProgressOutcome::Cancelled) => "취소됨",
        (_, ProgressOutcome::Failed) => "실패",
        (Lifecycle::Start, ProgressOutcome::Running) => "시작 중",
        (Lifecycle::Stop, ProgressOutcome::Running) => "중지 중",
        (Lifecycle::Restart, ProgressOutcome::Running) => "재시작 중",
        (Lifecycle::Start | Lifecycle::Restart, ProgressOutcome::Done) => "실행 중",
        (Lifecycle::Stop, ProgressOutcome::Done) => "중지됨",
    }
}

/// Title above the log: `시작 로그`, `재시작 로그`, `중지 로그`.
pub fn progress_log_title(cmd: Lifecycle) -> &'static str {
    match cmd {
        Lifecycle::Start => "시작 로그",
        Lifecycle::Restart => "재시작 로그",
        Lifecycle::Stop => "중지 로그",
    }
}

/// `00:31 경과` from an `mm:ss` label.
pub fn elapsed_text(mm_ss: &str) -> String {
    format!("{mm_ss} 경과")
}

/// `17줄`
pub fn log_line_count(lines: usize) -> String {
    format!("{lines}줄")
}

/// Heading of the six step bars.
pub const STEPS_TITLE: &str = "시작 단계";
/// Cancel button while a start runs.
pub const CANCEL_START: &str = "시작 취소";
/// What cancelling a start does.
pub const CANCEL_START_HINT: &str = "취소하면 VM을 멈춥니다. 만든 디스크는 남습니다.";
/// Empty log while the command runs.
pub const LOG_WAITING: &str = "로그를 기다리는 중…";
/// Empty log after the command ended.
pub const LOG_EMPTY: &str = "로그 없음";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_title_names_the_running_command() {
        // F4: a stop log is not a 시작 로그.
        assert_eq!(progress_log_title(Lifecycle::Start), "시작 로그");
        assert_eq!(progress_log_title(Lifecycle::Restart), "재시작 로그");
        assert_eq!(progress_log_title(Lifecycle::Stop), "중지 로그");
    }

    #[test]
    fn status_follows_the_command_and_its_outcome() {
        use ProgressOutcome::*;
        assert_eq!(progress_status(Lifecycle::Start, Running), "시작 중");
        assert_eq!(progress_status(Lifecycle::Stop, Running), "중지 중");
        assert_eq!(progress_status(Lifecycle::Restart, Running), "재시작 중");
        assert_eq!(progress_status(Lifecycle::Restart, Done), "실행 중");
        assert_eq!(progress_status(Lifecycle::Stop, Done), "중지됨");
        assert_eq!(progress_status(Lifecycle::Stop, Failed), "실패");
        assert_eq!(progress_status(Lifecycle::Start, Cancelled), "취소됨");
    }

    #[test]
    fn elapsed_and_line_count_copy() {
        assert_eq!(elapsed_text("00:31"), "00:31 경과");
        assert_eq!(log_line_count(17), "17줄");
        assert_eq!(STEPS_TITLE, "시작 단계");
        assert_eq!(CANCEL_START, "시작 취소");
    }
}
