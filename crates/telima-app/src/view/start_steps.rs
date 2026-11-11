//! Maps colima start log messages onto the six start steps of Starting.dc.html.

/// The six start steps, in order, as the canvas labels them.
pub const START_STEPS: [&str; 6] = [
    "설정 확인",
    "네트워크",
    "VM 부팅",
    "프로비저닝",
    "docker 시작",
    "상태 확인",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepState {
    Done,
    Current,
    Pending,
}

/// Progress through the start steps. `current` is the 1-based step reached (0 = none yet).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartProgress {
    pub current: usize,
    pub total: usize,
    pub label: &'static str,
}

impl StartProgress {
    /// `4/6`
    pub fn text(&self) -> String {
        format!("{}/{}", self.current, self.total)
    }

    pub fn steps(&self) -> [StepState; 6] {
        step_states(self.current)
    }
}

/// The 0-based step a colima log message belongs to, if any.
pub fn start_step_of(message: &str) -> Option<usize> {
    let _ = message;
    unimplemented!("RED: start_step_of")
}

/// Fold log messages into progress; the furthest step seen wins.
pub fn start_progress<'a>(messages: impl IntoIterator<Item = &'a str>) -> StartProgress {
    let _ = messages;
    unimplemented!("RED: start_progress")
}

/// Per-step state for `current` (1-based step reached, 0 = none).
pub fn step_states(current: usize) -> [StepState; 6] {
    let _ = current;
    unimplemented!("RED: step_states")
}

#[cfg(test)]
mod tests {
    use super::{START_STEPS, StartProgress, StepState, start_progress, start_step_of, step_states};

    #[test]
    fn step_names_match_canvas() {
        assert_eq!(
            START_STEPS,
            [
                "설정 확인",
                "네트워크",
                "VM 부팅",
                "프로비저닝",
                "docker 시작",
                "상태 확인"
            ]
        );
    }

    #[test]
    fn each_log_message_maps_to_its_step() {
        assert_eq!(start_step_of("checking existing profile config"), Some(0));
        assert_eq!(start_step_of("configuring network address"), Some(1));
        assert_eq!(start_step_of("starting VM"), Some(2));
        assert_eq!(start_step_of("provisioning docker"), Some(3));
        assert_eq!(start_step_of("starting docker"), Some(4));
        assert_eq!(start_step_of("docker is ready"), Some(5));
    }

    #[test]
    fn already_is_not_ready() {
        assert_eq!(start_step_of("already provisioned"), Some(3));
        assert_eq!(start_step_of("profile already exists"), Some(0));
    }

    #[test]
    fn unrelated_messages_map_to_no_step() {
        assert_eq!(start_step_of("something else entirely"), None);
        assert_eq!(start_step_of(""), None);
    }

    #[test]
    fn progress_keeps_the_furthest_step() {
        let p = start_progress(["starting VM", "random", "starting docker", "network"]);
        assert_eq!(
            p,
            StartProgress {
                current: 5,
                total: 6,
                label: "docker 시작"
            }
        );
        assert_eq!(p.text(), "5/6");
    }

    #[test]
    fn no_messages_is_preparing() {
        let p = start_progress(std::iter::empty());
        assert_eq!(p.current, 0);
        assert_eq!(p.text(), "0/6");
        assert_eq!(p.label, "준비 중");
    }

    #[test]
    fn step_states_mark_done_current_pending() {
        use StepState::{Current, Done, Pending};
        assert_eq!(
            step_states(4),
            [Done, Done, Done, Current, Pending, Pending]
        );
        assert_eq!(step_states(0), [Pending; 6]);
    }
}
