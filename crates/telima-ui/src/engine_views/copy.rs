//! Korean copy of the engine panel chrome, one definition per string.
//! Copy that belongs to the domain (status text, errors, confirms) comes from `telima_app::view`.
use telima_app::view::Confirm;

pub const FILTER_PLACEHOLDER: &str = "이름·이미지로 필터";
pub const NO_PROFILE: &str = "프로필을 선택하세요.";
pub const LOAD_FAILED: &str = "목록을 불러오지 못했습니다.";
pub const RETRY: &str = "다시 시도";
pub const EMPTY_CONTAINERS: &str = "컨테이너가 없습니다.";
pub const EMPTY_FILTERED: &str = "조건에 맞는 컨테이너가 없습니다.";
pub const EMPTY_IMAGES: &str = "이미지가 없습니다.";
pub const EMPTY_VOLUMES: &str = "볼륨이 없습니다.";
pub const PRUNE_IMAGES: &str = "미사용 이미지 정리";
pub const PRUNE_VOLUMES: &str = "미사용 볼륨 정리";
pub const MENU_LOGS: &str = "로그 보기";
pub const MENU_RESTART: &str = "재시작";
pub const MENU_REMOVE: &str = "삭제";
pub const CANCEL: &str = "취소";
pub const LOG_TIME: &str = "시각";
pub const LOG_FOLLOW: &str = "따라가기";
pub const LOG_EMPTY: &str = "로그가 없습니다.";
pub const LOG_FOLLOWING: &str = "따라가는 중 · 새 줄이 오면 맨 아래로 이동";
pub const LOG_PAUSED: &str = "따라가기 꺼짐 · 지금까지의 로그";
/// Shown in the 포트 / 시작 cells while the engine does not report them.
pub const NONE: &str = "—";

pub fn remove_exited(n: usize) -> String {
    format!("종료된 {n}개 삭제")
}

pub fn more_items(n: usize) -> String {
    format!("외 {n}개")
}

pub fn line_count(lines: &str) -> String {
    format!("{lines}줄")
}

pub fn confirm_remove_exited(n: usize) -> Confirm {
    Confirm {
        title: format!("종료된 컨테이너 {n}개를 삭제할까요?"),
        body: "중지된 컨테이너만 삭제합니다. 실행 중인 컨테이너와 볼륨은 그대로 둡니다.".into(),
        confirm_label: format!("{n}개 삭제"),
        can_confirm: true,
        typed_name: None,
    }
}

/// Column headers of the containers table: (label, share of the row width).
pub const CONTAINER_COLS: [(&str, f32); 6] = [
    ("이름", 0.2),
    ("ID", 0.11),
    ("이미지", 0.19),
    ("상태", 0.19),
    ("포트", 0.17),
    ("시작", 0.1),
];
pub const IMAGE_COLS: [(&str, f32); 4] = [
    ("태그", 0.4),
    ("ID", 0.3),
    ("크기", 0.15),
    ("컨테이너", 0.15),
];
pub const VOLUME_COLS: [(&str, f32); 3] = [("이름", 0.55), ("크기", 0.2), ("사용", 0.25)];
