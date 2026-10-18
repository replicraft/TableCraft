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
    /// Unencoded Access 2016-family layout, code 5, subversion zero.
    Ace2016,
    /// Unencoded Access 2019-family layout, code 6, subversion zero.
    Ace2019,
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
        FileFormat::Ace2016 => Ok(ReadProfile::Ace2016),
        FileFormat::Ace2019 => Ok(ReadProfile::Ace2019),
        _ => Err(InspectError::Unsupported),
    }
}

#[cfg(test)]
mod tests;
