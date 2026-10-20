//! Shared native read/output errors at the command boundary.
use crate::output::OutputError;
use std::fmt;
use tablecraft_ace::InspectError;

pub(crate) enum Error {
    Input(tablecraft_access::AccessInputError),
    Read(InspectError),
    Output(OutputError),
    Copy(std::io::ErrorKind),
    NewPasswordRequired,
    PasswordMismatch,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(e) => e.fmt(f),
            Self::Read(e) => e.fmt(f),
            Self::Output(e) => e.fmt(f),
            Self::Copy(kind) => write!(
                f,
                "could not create or publish a fresh database copy ({kind:?})"
            ),
            Self::NewPasswordRequired => f.write_str(
                "new database password is required; use --new-password for noninteractive input",
            ),
            Self::PasswordMismatch => {
                f.write_str("new database password confirmation does not match")
            }
        }
    }
}
