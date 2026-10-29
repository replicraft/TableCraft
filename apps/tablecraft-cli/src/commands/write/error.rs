//! Contextual host errors without database payloads or filesystem paths.
use std::{fmt, io::ErrorKind};
use tablecraft_access::{AccessInputError, engineering::EngineeringError};
use tablecraft_ace::InspectError;

#[derive(Debug)]
pub(super) enum Error {
    Input(String),
    File(ErrorKind),
    Source(AccessInputError),
    Native(InspectError),
    Preview(EngineeringError),
    Publish(super::publication::Error),
    Encrypted,
    Deployment,
}

impl From<InspectError> for Error {
    fn from(error: InspectError) -> Self {
        Self::Native(error)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(message) => f.write_str(message),
            Self::File(kind) => write!(f, "could not capture request input ({kind:?})"),
            Self::Source(error) => error.fmt(f),
            Self::Native(error) => error.fmt(f),
            Self::Preview(error) => error.fmt(f),
            Self::Publish(error) => error.fmt(f),
            Self::Encrypted => f.write_str(
                "encrypted rewrite is unsupported; no decrypted/plaintext output is published",
            ),
            Self::Deployment => {
                f.write_str("deployment packages cannot be used as engineering write sources")
            }
        }
    }
}

pub(super) fn input(message: &str) -> Error {
    Error::Input(message.to_owned())
}
