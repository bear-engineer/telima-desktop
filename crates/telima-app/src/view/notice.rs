//! Korean copy for application errors. Views never show `to_string()` of an error.

use telima_core::ports::PortError;

use crate::AppError;

/// What kind of notice to show. Drives the empty-state / banner layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeKind {
    NotInstalled,
    EngineUnavailable,
    NotFound,
    Busy,
    Invalid,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub kind: NoticeKind,
    pub message: String,
    /// A command or next step the user can run, e.g. `brew install colima`.
    pub hint: Option<&'static str>,
}

/// Classify an error and attach the Korean copy for it.
pub fn notice(err: &AppError) -> Notice {
    let (kind, message, hint) = match err {
        AppError::Port(PortError::Unavailable(detail)) if detail.contains("not installed") => (
            NoticeKind::NotInstalled,
            "Colima가 설치되어 있지 않습니다.",
            Some("brew install colima"),
        ),
        AppError::Port(PortError::Unavailable(_)) => (
            NoticeKind::EngineUnavailable,
            "Colima VM 또는 Docker 엔진에 연결할 수 없습니다. 프로필이 실행 중인지 확인해 주세요.",
            None,
        ),
        AppError::Port(PortError::NotFound(_)) => (
            NoticeKind::NotFound,
            "요청한 항목을 찾을 수 없습니다. 목록을 새로고침한 뒤 다시 시도해 주세요.",
            None,
        ),
        AppError::Port(PortError::Failed(_)) => (
            NoticeKind::Failed,
            "작업을 완료하지 못했습니다. 잠시 후 다시 시도해 주세요.",
            None,
        ),
        AppError::Busy(_) => (
            NoticeKind::Busy,
            "다른 작업이 진행 중입니다. 끝난 뒤 다시 시도해 주세요.",
            None,
        ),
        AppError::InUse(_) => (
            NoticeKind::Busy,
            "사용 중이라 지금은 처리할 수 없습니다.",
            None,
        ),
        AppError::Invalid(violations) => (
            NoticeKind::Invalid,
            if violations.len() == 1 {
                "설정 값 1개가 규칙에 맞지 않습니다."
            } else {
                "설정 값이 규칙에 맞지 않습니다."
            },
            None,
        ),
        AppError::ConfirmationMismatch => (
            NoticeKind::Invalid,
            "입력한 이름이 프로필 이름과 다릅니다.",
            None,
        ),
    };
    Notice {
        kind,
        message: message.to_owned(),
        hint,
    }
}

/// Korean message for an error (no technical detail).
pub fn error_text(err: &AppError) -> String {
    notice(err).message
}

#[cfg(test)]
mod tests {
    use telima_core::ports::PortError;
    use telima_core::profile::ProfileName;
    use telima_core::rules::RuleViolation;

    use super::{NoticeKind, error_text, notice};
    use crate::AppError;

    fn busy() -> AppError {
        AppError::Busy(ProfileName::parse("dev").unwrap())
    }

    #[test]
    fn missing_colima_is_not_installed_with_install_hint() {
        let err = AppError::Port(PortError::Unavailable("colima is not installed".into()));
        let n = notice(&err);
        assert_eq!(n.kind, NoticeKind::NotInstalled);
        assert_eq!(n.message, "Colima가 설치되어 있지 않습니다.");
        assert_eq!(n.hint, Some("brew install colima"));
    }

    #[test]
    fn other_unavailable_is_engine_unavailable_without_hint() {
        let err = AppError::Port(PortError::Unavailable("socket missing".into()));
        let n = notice(&err);
        assert_eq!(n.kind, NoticeKind::EngineUnavailable);
        assert!(n.message.contains("연결할 수 없습니다"));
        assert_eq!(n.hint, None);
    }

    #[test]
    fn each_error_kind_maps_to_its_notice() {
        assert_eq!(
            notice(&AppError::Port(PortError::NotFound("x".into()))).kind,
            NoticeKind::NotFound
        );
        assert_eq!(notice(&busy()).kind, NoticeKind::Busy);
        assert_eq!(notice(&AppError::InUse("pg".into())).kind, NoticeKind::Busy);
        assert_eq!(
            notice(&AppError::Invalid(vec![RuleViolation::RosettaUnavailable])).kind,
            NoticeKind::Invalid
        );
        assert_eq!(
            notice(&AppError::ConfirmationMismatch).kind,
            NoticeKind::Invalid
        );
        assert_eq!(
            notice(&AppError::Port(PortError::Failed("boom".into()))).kind,
            NoticeKind::Failed
        );
    }

    #[test]
    fn error_text_is_korean_and_hides_english_detail() {
        let text = error_text(&busy());
        assert_eq!(
            text,
            "다른 작업이 진행 중입니다. 끝난 뒤 다시 시도해 주세요."
        );
        assert!(!text.contains("busy"));
        assert_eq!(
            error_text(&AppError::Port(PortError::Failed("secret english".into()))),
            "작업을 완료하지 못했습니다. 잠시 후 다시 시도해 주세요."
        );
        assert_eq!(
            error_text(&AppError::ConfirmationMismatch),
            "입력한 이름이 프로필 이름과 다릅니다."
        );
    }
}
