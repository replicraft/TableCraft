//! Qualified read-layout selection, separate from header recognition and limits.
use crate::{FileFormat, Header, InspectError};

const QUALIFIED_SUBVERSION: u8 = 0;
const QUALIFIED_OFFICE_SUBVERSION: u8 = 1;

/// Qualified bounded catalog/scalar/index read family. Individual field and index
/// eligibility checks still apply. No variant grants mutation or codec support.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadProfile {
    /// Jet 4 layout; does not itself supply a page codec.
    Jet4,
    /// ACE 2007 layout.
    Ace2007,
    /// ACE 2010 layout, also used by the qualified Agile reference inputs.
    Ace2010,
    /// Access 2016-family layout, code 5.
    Ace2016,
    /// Access 2019-family layout, code 6.
    Ace2019,
}

pub(crate) fn select(header: Header) -> Result<ReadProfile, InspectError> {
    if header.encoded_pages() && !header.has_decoder() {
        return Err(InspectError::Encoded);
    }

    if header.subversion() != QUALIFIED_SUBVERSION
        && !(matches!(
            header.format(),
            FileFormat::Ace2007 | FileFormat::Ace2010 | FileFormat::Ace2016 | FileFormat::Ace2019
        ) && header.subversion() == QUALIFIED_OFFICE_SUBVERSION)
    {
        return Err(InspectError::Unsupported);
    }

    match header.format() {
        FileFormat::Jet4 => Ok(ReadProfile::Jet4),
        FileFormat::Ace2007 => Ok(ReadProfile::Ace2007),
        FileFormat::Ace2010 => Ok(ReadProfile::Ace2010),
        FileFormat::Ace2016 => Ok(ReadProfile::Ace2016),
        FileFormat::Ace2019 => Ok(ReadProfile::Ace2019),
        _ => Err(InspectError::Unsupported),
    }
}

#[cfg(test)]
mod tests;
