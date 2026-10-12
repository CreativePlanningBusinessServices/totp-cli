use serde_json::{Value, json};

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// Index file or other local I/O problem.
    #[error("{0}")]
    Io(String),
    /// Bad arguments, bad secret or URI, unknown or duplicate account.
    #[error("{0}")]
    Usage(String),
    /// The OS credential store (or the file store) failed or lacks the secret.
    #[error("{0}")]
    Store(String),
}

impl CliError {
    pub fn exit_code(&self) -> u8 {
        match self {
            CliError::Io(_) => 1,
            CliError::Usage(_) => 2,
            CliError::Store(_) => 3,
        }
    }

    pub fn to_json(&self) -> Value {
        json!({"error": self.to_string(), "exit_code": self.exit_code()})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_match_error_kinds() {
        assert_eq!(CliError::Io("disk".into()).exit_code(), 1);
        assert_eq!(CliError::Usage("bad name".into()).exit_code(), 2);
        assert_eq!(CliError::Store("keychain".into()).exit_code(), 3);
    }

    #[test]
    fn json_carries_message_and_exit_code() {
        let error = CliError::Usage("no account named 'x'".into());
        assert_eq!(
            error.to_json(),
            json!({"error": "no account named 'x'", "exit_code": 2})
        );
    }
}
