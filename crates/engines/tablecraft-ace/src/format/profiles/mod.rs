//! Qualified read-layout selection, separate from header recognition and limits.
use crate::{FileFormat, Header, InspectError};

const QUALIFIED_SUBVERSION: u8 = 0;

/// Qualified bounded catalog/scalar/index read family. Individual field and index
/// eligibility checks still apply. No variant grants mutation or codec support.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadProfile {
    /// Unencoded Jet 4, subversion zero.
    Jet4,
    /// Unencoded ACE 2007, subversion zero.
    Ace2007,
}

pub(crate) fn select(header: Header) -> Result<ReadProfile, InspectError> {
    if header.encoded_pages() {
        return Err(InspectError::Encoded);
    }

    if header.subversion() != QUALIFIED_SUBVERSION {
        return Err(InspectError::Unsupported);
    }

    match header.format() {
        FileFormat::Jet4 => Ok(ReadProfile::Jet4),
        FileFormat::Ace2007 => Ok(ReadProfile::Ace2007),
        _ => Err(InspectError::Unsupported),
    }
}

#[cfg(test)]
mod tests;
