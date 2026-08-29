use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("skill '{0}' not found")]
    SkillNotFound(String),

    #[error("skill '{skill}' failed: {message}")]
    SkillFailed { skill: String, message: String },

    #[error("invalid input for tool '{tool}': {message}")]
    InvalidInput { tool: String, message: String },

    #[error("llm request failed: {0}")]
    Llm(String),

    #[error(
        "credential '{0}' not found in the OS keychain (service 'otto-assistant') - set it there, \
         or export it as OTTO_<KEY> in SCREAMING_SNAKE_CASE (e.g. '{0}' -> OTTO_{upper})",
        upper = .0.to_uppercase().replace('-', "_")
    )]
    CredentialNotFound(String),

    #[error("credential store error: {0}")]
    Credential(String),

    #[error("history store error: {0}")]
    History(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type CoreResult<T> = Result<T, CoreError>;
