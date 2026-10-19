//! Shared native read/output errors at the command boundary.
use crate::output::OutputError;
use std::fmt;
use tablecraft_ace::InspectError;

pub(crate) enum Error {
    Read(InspectError),
    Output(OutputError),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(e) => e.fmt(f),
            Self::Output(e) => e.fmt(f),
        }
    }
}
