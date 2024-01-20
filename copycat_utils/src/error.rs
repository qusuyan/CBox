use std::fmt;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CopycatError(pub String);

impl fmt::Display for CopycatError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0) // do not display literal quotes
    }
}

// Helper macro for saving boiler-plate `impl From<T>`s for transparent
// conversion from various common error types to `SummersetError`.
macro_rules! impl_from_error {
    ($error:ty) => {
        impl From<$error> for CopycatError {
            fn from(e: $error) -> Self {
                // just store the source error's string representation
                CopycatError(e.to_string())
            }
        }
    };
}

impl_from_error!(mailbox_utils::MailboxError);
impl_from_error!(tokio::task::JoinError);
impl_from_error!(bincode::Error);
impl_from_error!(toml::de::Error);
