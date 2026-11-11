//! Copy of the lifecycle progress panel (canvas `Starting.dc.html`): status, log title,
//! elapsed time and cancel. One definition per string; telima-ui only renders it.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::Lifecycle;

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
