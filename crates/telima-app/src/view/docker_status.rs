//! Korean status text for a container row, built from the raw Docker status line
//! (`Up 33 minutes`, `Exited (0) 4 hours ago`, ...). Matches Containers.dc.html.

use telima_core::container::ContainerState;

/// Korean status text such as `실행 중 · 33분`, `종료됨 · 코드 0`, `오류 · 종료 코드 1`.
/// Unknown text falls back to the Korean state label.
pub fn container_status_text(raw: &str, state: ContainerState) -> String {
    let _ = (raw, state);
    unimplemented!("RED: container_status_text")
}

#[cfg(test)]
mod tests {
    use telima_core::container::ContainerState;

    use super::container_status_text;

    fn text(raw: &str, state: ContainerState) -> String {
        container_status_text(raw, state)
    }

    #[test]
    fn running_shows_korean_duration() {
        assert_eq!(
            text("Up 33 minutes", ContainerState::Running),
            "실행 중 · 33분"
        );
        assert_eq!(
            text("Up 2 hours (healthy)", ContainerState::Running),
            "실행 중 · 2시간"
        );
        assert_eq!(
            text("Up About an hour", ContainerState::Running),
            "실행 중 · 1시간"
        );
        assert_eq!(text("Up 1 minute", ContainerState::Running), "실행 중 · 1분");
        assert_eq!(text("Up 5 seconds", ContainerState::Running), "실행 중 · 5초");
        assert_eq!(text("Up 3 days", ContainerState::Running), "실행 중 · 3일");
        assert_eq!(text("Up 2 weeks", ContainerState::Running), "실행 중 · 2주");
    }

    #[test]
    fn exited_zero_shows_code_zero() {
        assert_eq!(
            text("Exited (0) 4 hours ago", ContainerState::Exited),
            "종료됨 · 코드 0"
        );
    }

    #[test]
    fn exited_nonzero_is_an_error_with_exit_code() {
        assert_eq!(
            text("Exited (1) 1 minute ago", ContainerState::Exited),
            "오류 · 종료 코드 1"
        );
        assert_eq!(
            text("Exited (137) 2 minutes ago", ContainerState::Exited),
            "오류 · 종료 코드 137"
        );
    }

    #[test]
    fn restarting_shows_last_exit_code() {
        assert_eq!(
            text("Restarting (1) 3 seconds ago", ContainerState::Restarting),
            "재시작 중 · 종료 코드 1"
        );
    }

    #[test]
    fn plain_states_use_state_label() {
        assert_eq!(text("Created", ContainerState::Created), "생성됨");
        assert_eq!(text("Paused", ContainerState::Paused), "일시정지");
        assert_eq!(text("Dead", ContainerState::Dead), "오류");
    }

    #[test]
    fn unknown_text_falls_back_to_state_label() {
        assert_eq!(text("something odd", ContainerState::Running), "실행 중");
        assert_eq!(text("", ContainerState::Exited), "종료됨");
    }
}
