use std::fmt;

/// Every failure a use case can report. Adapters translate their own errors
/// into one of these, so callers never see an `io::Error` or a git exit code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HubError {
    /// The request named a repository or tool that is not tracked.
    NotFound(String),
    /// The input was rejected before anything ran.
    Invalid(String),
    /// An external command ran and failed; the text is its last output.
    Command(String),
    /// Reading or writing local state failed.
    Storage(String),
}

pub type HubResult<T> = Result<T, HubError>;

impl fmt::Display for HubError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HubError::NotFound(what) => write!(f, "not found: {what}"),
            HubError::Invalid(why) => write!(f, "{why}"),
            HubError::Command(output) => write!(f, "{output}"),
            HubError::Storage(why) => write!(f, "storage: {why}"),
        }
    }
}

impl std::error::Error for HubError {}
