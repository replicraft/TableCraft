//! Native encryption inspection and explicit password-based page codecs.
mod agile;
pub(crate) mod legacy;
use crate::{FileFormat, InspectError, inspect_header};
pub use agile::{
    AgileEncryption, AgileHashAlgorithm, AgilePageDecoder, AgileParameters, read_agile_encryption,
};
use std::io::{Read, Seek};
use tablecraft_core::limits::{Resource, ResourceBudget};

const OFFICE_INFO_LENGTH_OFFSET: usize = 0x299;
const OFFICE_INFO_OFFSET: usize = OFFICE_INFO_LENGTH_OFFSET + size_of::<u16>();
const VERSION_BYTES: usize = 2 * size_of::<u16>();
const FLAGS_END: usize = VERSION_BYTES + size_of::<u32>();
const AGILE_RESERVED: u32 = 0x40;
const RC4_SALT_BYTES: usize = 16;
const RC4_VERIFIER_BYTES: usize = 16;
const RC4_VERIFIER_HASH_BYTES: usize = 16;
const BINARY_RC4_INFO_BYTES: usize =
    VERSION_BYTES + RC4_SALT_BYTES + RC4_VERIFIER_BYTES + RC4_VERIFIER_HASH_BYTES;
const QUALIFIED_SUBVERSION: u8 = 0;
const OFFICE_ENVELOPE_SUBVERSION: u8 = 1;

/// Recognized Office encryption envelope, not a qualified cipher configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OfficeEncryptionFamily {
    /// Version 1.1 binary-document RC4 envelope.
    BinaryRc4,
    /// Version 2/3/4.2; the remaining header selects the actual cipher/provider.
    StandardOrCryptoApi,
    /// Version 3/4.3 external-provider envelope.
    Extensible,
    /// Version 4.4 XML-based envelope; XML parameters remain unvalidated.
    Agile,
    /// An unrecognized version whose bounded envelope can still be inspected.
    Unknown,
}

/// Non-secret Office envelope metadata. Does not retain descriptor payload bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OfficeEncryptionInfo {
    major: u16,
    minor: u16,
    bytes: u16,
    family: OfficeEncryptionFamily,
    flags: Option<u32>,
}

impl OfficeEncryptionInfo {
    /// Major and minor Office encryption version, independent of the ACE version.
    pub const fn version(self) -> (u16, u16) {
        (self.major, self.minor)
    }
    /// Declared descriptor bytes, including its version fields.
    pub const fn descriptor_bytes(self) -> u16 {
        self.bytes
    }
    /// Envelope family only; no cipher, password or page mapping qualification.
    pub const fn family(self) -> OfficeEncryptionFamily {
        self.family
    }
    /// Raw outer flags for standard/CryptoAPI or extensible envelopes.
    /// Agile's reserved word is not a flags field.
    pub const fn flags(self) -> Option<u32> {
        self.flags
    }
}

/// Page-encoding metadata, independent of database passwords/workgroup permissions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncryptionInspection {
    /// Zero encoding key. Does not establish the absence of a legacy password.
    UnencodedPages,
    /// Nonzero Jet 4 encoding key; password protection is a separate feature.
    JetEncoding,
    /// Bounded ACE Office descriptor envelope; decryption is not implemented.
    Office(OfficeEncryptionInfo),
}

/// Inspect inert encoding metadata on a stable/offline source without credentials.
///
/// A nonzero ACE key causes one bounded full-page read. Only envelope framing and
/// known fixed prefixes are checked: XML, algorithms, verifiers and credentials
/// are not validated. Unknown versions remain visible. No data pages are decoded,
/// source bytes changed, or payloads/secrets included in the returned metadata.
/// Jet 3 remains unsupported. ACE envelope inspection accepts subversions zero
/// and one independently of the narrower table-layout qualification.
pub fn inspect_encryption<R: Read + Seek>(
    source: &mut R,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<EncryptionInspection, InspectError> {
    let header = inspect_header(source, budget, cancelled)?;
    crate::header::check(budget, cancelled)?;
    let qualified_subversion = header.subversion() == QUALIFIED_SUBVERSION
        || (header.format() != FileFormat::Jet4
            && header.subversion() == OFFICE_ENVELOPE_SUBVERSION);
    if !qualified_subversion || header.format() == FileFormat::Jet3 {
        return Err(InspectError::Unsupported);
    }
    if !header.encoded_pages() {
        return Ok(EncryptionInspection::UnencodedPages);
    }
    if header.format() == FileFormat::Jet4 {
        return Ok(EncryptionInspection::JetEncoding);
    }
    let mut page = [0; crate::format::page::PAGE_BYTES];
    budget.charge(Resource::Bytes, page.len() as u64)?;
    tablecraft_io::read_exact_at(source, 0, &mut page, budget, cancelled)?;
    let result = parse_office(&page)?;
    crate::header::check(budget, cancelled)?;
    Ok(EncryptionInspection::Office(result))
}

fn parse_office(page: &[u8]) -> Result<OfficeEncryptionInfo, InspectError> {
    let length_bytes = page
        .get(OFFICE_INFO_LENGTH_OFFSET..OFFICE_INFO_OFFSET)
        .ok_or(InspectError::Truncated)?;
    let bytes = u16::from_le_bytes(length_bytes.try_into().unwrap());
    let info = page
        .get(OFFICE_INFO_OFFSET..OFFICE_INFO_OFFSET + usize::from(bytes))
        .ok_or(InspectError::MalformedEncryption)?;
    if info.len() < VERSION_BYTES {
        return Err(InspectError::MalformedEncryption);
    }
    let major = u16::from_le_bytes(info[..size_of::<u16>()].try_into().unwrap());
    let minor = u16::from_le_bytes(info[size_of::<u16>()..VERSION_BYTES].try_into().unwrap());
    let family = match (major, minor) {
        (1, 1) => OfficeEncryptionFamily::BinaryRc4,
        (2..=4, 2) => OfficeEncryptionFamily::StandardOrCryptoApi,
        (3..=4, 3) => OfficeEncryptionFamily::Extensible,
        (4, 4) => OfficeEncryptionFamily::Agile,
        _ => OfficeEncryptionFamily::Unknown,
    };
    let word = || {
        info.get(VERSION_BYTES..FLAGS_END)
            .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
            .ok_or(InspectError::MalformedEncryption)
    };
    let flags = match family {
        OfficeEncryptionFamily::Agile => {
            if word()? != AGILE_RESERVED || info.len() <= FLAGS_END {
                return Err(InspectError::MalformedEncryption);
            }
            None
        }
        OfficeEncryptionFamily::StandardOrCryptoApi | OfficeEncryptionFamily::Extensible => {
            Some(word()?)
        }
        OfficeEncryptionFamily::BinaryRc4 => {
            if info.len() < BINARY_RC4_INFO_BYTES {
                return Err(InspectError::MalformedEncryption);
            }
            None
        }
        OfficeEncryptionFamily::Unknown => None,
    };
    Ok(OfficeEncryptionInfo {
        major,
        minor,
        bytes,
        family,
        flags,
    })
}

#[cfg(test)]
mod tests;
