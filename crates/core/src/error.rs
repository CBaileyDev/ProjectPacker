use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("invalid target: {0}")]
    InvalidTarget(String),

    #[error("path does not exist: {0}")]
    PathNotFound(PathBuf),

    #[error("github clone failed: {0}")]
    CloneFailed(String),

    #[error("file walk failed: {0}")]
    WalkFailed(String),

    #[error("io error reading {path}: {source}")]
    FileIo {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("xml emission failed: {0}")]
    XmlWrite(String),

    #[error("tokenizer not available for model: {0}")]
    TokenizerUnavailable(String),

    #[error("tokenizer encode failed: {0}")]
    TokenizerEncodeFailed(String),

    #[error("plan validation failed: {errors:?}")]
    PlanInvalid { errors: Vec<String> },

    #[error("cancelled by user")]
    Cancelled,

    #[error("internal: {0}")]
    Internal(String),
}

pub type CoreResult<T> = Result<T, CoreError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_target_displays_as_expected() {
        let e = CoreError::InvalidTarget("not a url".into());
        assert_eq!(e.to_string(), "invalid target: not a url");
    }

    #[test]
    fn cancelled_has_no_args() {
        let e = CoreError::Cancelled;
        assert_eq!(e.to_string(), "cancelled by user");
    }

    #[test]
    fn plan_invalid_includes_errors() {
        let e = CoreError::PlanInvalid {
            errors: vec!["missing Summary".into(), "no rationale on Step 2".into()],
        };
        let s = e.to_string();
        assert!(s.contains("missing Summary"));
        assert!(s.contains("no rationale on Step 2"));
    }
}
