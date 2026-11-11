use telima_core::ports::PortError;
use telima_core::profile::ProfileName;
use telima_core::rules::RuleViolation;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AppError {
    #[error("the settings break {} rule(s)", .0.len())]
    Invalid(Vec<RuleViolation>),
    #[error("{0} is busy with another operation")]
    Busy(ProfileName),
    #[error("type the profile name to confirm")]
    ConfirmationMismatch,
    #[error("{0} is in use")]
    InUse(String),
    #[error(transparent)]
    Port(#[from] PortError),
}
