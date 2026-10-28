//! Shared native read/output errors at the command boundary.
use crate::output::OutputError;
use std::fmt;
use tablecraft_ace::InspectError;

pub(crate) enum Error {
    Input(tablecraft_access::AccessInputError),
    Read(InspectError),
    Template(tablecraft_formats::template::TemplateError),
    Output(OutputError),
    Copy(std::io::ErrorKind),
    NewPasswordRequired,
    PasswordMismatch,
    NoUserTables,
    AmbiguousUserTable,
    ScalarAuditUnsupported,
}

impl Error {
    pub(super) fn scalar_audit(error: InspectError) -> Self {
        match error {
            InspectError::Unsupported => Self::ScalarAuditUnsupported,
            other => Self::Read(other),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(e) => e.fmt(f),
            Self::Read(e) => e.fmt(f),
            Self::Template(e) => e.fmt(f),
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
            Self::NoUserTables => {
                f.write_str("no user tables found; specify TABLE explicitly or use --all")
            }
            Self::AmbiguousUserTable => {
                f.write_str("more than one user table found; specify TABLE or use --all")
            }
            Self::ScalarAuditUnsupported => f.write_str(concat!(
                "this audit requires a qualified ordinary scalar table; calculated fields, ",
                "collections and other unqualified layouts are not checked. Use rows for ",
                "supported stored-value browsing or inspect --table TABLE --features for declarations"
            )),
        }
    }
}
