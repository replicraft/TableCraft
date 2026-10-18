use crate::InspectError;
use crate::{
    format::page::{JET3_PAGE_BYTES, PAGE_BYTES},
    profile::MAX_FILE_BYTES,
};
use std::io::{Read, Seek, SeekFrom};
use tablecraft_core::limits::{Resource, ResourceBudget};

const FILE_SIGNATURE: [u8; 4] = [0, 1, 0, 0];
const ENGINE_SIGNATURE_RANGE: std::ops::Range<usize> = 4..20;
const FORMAT_CODE_OFFSET: usize = 20;
const SUBVERSION_OFFSET: usize = 21;
const RESERVED_RANGE: std::ops::Range<usize> = 22..24;
const ENCODING_KEY_RANGE: std::ops::Range<usize> = 62..66;
const ENCODED_ZERO_KEY: [u8; 4] = [0xfb, 0x8a, 0xbc, 0x4e];
const JET_SIGNATURE: &[u8] = b"Standard Jet DB\0";
const ACE_SIGNATURE: &[u8] = b"Standard ACE DB\0";
const JET3_CODE: u8 = 0;
const JET4_CODE: u8 = 1;
const ACE2007_CODE: u8 = 2;
const ACE2010_CODE: u8 = 3;
const ACE2016_CODE: u8 = 5;
const ACE2019_CODE: u8 = 6;
const MIN_FILE_PAGES: u64 = 3;

/// Recognized header format code; not the version/build of the creating Access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileFormat {
    /// Jet 3 header, format code 0.
    Jet3,
    /// Jet 4 header, format code 1.
    Jet4,
    /// ACE 2007 format family, code 2.
    Ace2007,
    /// ACE 2010 format family, code 3.
    Ace2010,
    /// ACE 2016 format family, code 5.
    Ace2016,
    /// ACE 2019 format family, code 6.
    Ace2019,
}
/// Validated header summary. No password/key material is exposed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    format: FileFormat,
    page_size: u32,
    page_count: u32,
    subversion: u8,
    encoded: bool,
}
impl Header {
    /// Minimum prefix size for header parsing and its exact read-byte charge.
    pub const PREFIX_BYTES: usize = 66;
    /// Parse a header prefix plus total file length, without decoding pages.
    /// Inputs must contain at least 66 bytes. Total size is bounded to 64 MiB.
    pub fn parse(prefix: &[u8], length: u64) -> Result<Self, InspectError> {
        if prefix.len() < Self::PREFIX_BYTES {
            return Err(InspectError::Truncated);
        }
        if prefix[..FILE_SIGNATURE.len()] != FILE_SIGNATURE {
            return Err(InspectError::Malformed);
        }
        let format = match (&prefix[ENGINE_SIGNATURE_RANGE], prefix[FORMAT_CODE_OFFSET]) {
            (JET_SIGNATURE, JET3_CODE) => FileFormat::Jet3,
            (JET_SIGNATURE, JET4_CODE) => FileFormat::Jet4,
            (ACE_SIGNATURE, ACE2007_CODE) => FileFormat::Ace2007,
            (ACE_SIGNATURE, ACE2010_CODE) => FileFormat::Ace2010,
            (ACE_SIGNATURE, ACE2016_CODE) => FileFormat::Ace2016,
            (ACE_SIGNATURE, ACE2019_CODE) => FileFormat::Ace2019,
            _ => return Err(InspectError::Unsupported),
        };
        if prefix[RESERVED_RANGE] != [0, 0] {
            return Err(InspectError::Unsupported);
        }
        let page_size = if format == FileFormat::Jet3 {
            JET3_PAGE_BYTES as u32
        } else {
            PAGE_BYTES as u32
        };
        if length < u64::from(page_size) * MIN_FILE_PAGES {
            return Err(InspectError::Truncated);
        }
        if length > MAX_FILE_BYTES {
            return Err(InspectError::StructureLimit);
        }
        if !length.is_multiple_of(u64::from(page_size)) {
            return Err(InspectError::Malformed);
        }
        // Encoded representation of a zero page-encoding key at 0x3e. This is
        // a format constant, not password decryption or a cryptographic primitive.
        let encoded = prefix[ENCODING_KEY_RANGE] != ENCODED_ZERO_KEY;
        Ok(Self {
            format,
            page_size,
            page_count: (length / u64::from(page_size)) as u32,
            subversion: prefix[SUBVERSION_OFFSET],
            encoded,
        })
    }
    /// Recognized format family.
    pub const fn format(self) -> FileFormat {
        self.format
    }
    /// Page bytes inferred from the recognized format.
    pub const fn page_size(self) -> u32 {
        self.page_size
    }
    /// Whole pages in the provided file length.
    pub const fn page_count(self) -> u32 {
        self.page_count
    }
    /// Raw compatibility subversion byte; no feature inference is made.
    pub const fn subversion(self) -> u8 {
        self.subversion
    }
    /// Whether the page-encoding key is nonzero; this does not qualify a codec.
    pub const fn encoded_pages(self) -> bool {
        self.encoded
    }
}
/// Read a bounded prefix from an explicitly supplied stable/offline source.
/// Read/seek callbacks own their I/O timeouts. No writes, lock files, providers,
/// password attempts or application startup occur. Header recognition is not a
/// claim that the catalog, objects or records can be decoded.
pub fn inspect_header<R: Read + Seek>(
    source: &mut R,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Header, InspectError> {
    check(budget, cancelled)?;
    let length = source.seek(SeekFrom::End(0))?;
    budget.charge(Resource::Bytes, Header::PREFIX_BYTES as u64)?;
    let mut prefix = [0; Header::PREFIX_BYTES];
    tablecraft_io::read_exact_at(source, 0, &mut prefix, budget, cancelled)?;
    Header::parse(&prefix, length)
}
pub(crate) fn check(
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    if cancelled() {
        return Err(InspectError::Cancelled);
    }
    budget.charge(Resource::Steps, 1)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    fn prefix() -> Vec<u8> {
        let mut p = vec![0; 66];
        p[..4].copy_from_slice(&[0, 1, 0, 0]);
        p[4..20].copy_from_slice(b"Standard ACE DB\0");
        p[20] = 2;
        p[62..66].copy_from_slice(&[0xfb, 0x8a, 0xbc, 0x4e]);
        p
    }
    #[test]
    fn signatures_sizes_and_encoding_are_qualified_independently() {
        let mut p = prefix();
        let h = Header::parse(&p, 4096 * 3).unwrap();
        assert_eq!(
            (h.format(), h.page_size(), h.page_count(), h.encoded_pages()),
            (FileFormat::Ace2007, 4096, 3, false)
        );
        p[62] ^= 1;
        assert!(Header::parse(&p, 4096 * 3).unwrap().encoded_pages());
        p[20] = 4;
        assert_eq!(Header::parse(&p, 4096 * 3), Err(InspectError::Unsupported));
        assert_eq!(Header::parse(&[], 4096 * 3), Err(InspectError::Truncated));
        assert_eq!(
            Header::parse(&prefix(), 4096 * 3 + 1),
            Err(InspectError::Malformed)
        );
        assert_eq!(Header::parse(&prefix(), 4096), Err(InspectError::Truncated));
        assert_eq!(
            Header::parse(&prefix(), 128 * 1024 * 1024),
            Err(InspectError::StructureLimit)
        );
    }
    #[test]
    fn read_limits_cancellation_and_truncation_are_not_hidden() {
        let mut data = std::io::Cursor::new(prefix());
        assert_eq!(
            inspect_header(&mut data, &mut ResourceBudget::new(0, 0, 10), &|| false),
            Err(InspectError::Limit(tablecraft_core::limits::LimitError {
                resource: Resource::Bytes,
                remaining: 0
            }))
        );
        assert_eq!(
            inspect_header(&mut data, &mut ResourceBudget::new(66, 0, 10), &|| true),
            Err(InspectError::Cancelled)
        );
        assert_eq!(
            inspect_header(&mut data, &mut ResourceBudget::new(66, 0, 10), &|| false),
            Err(InspectError::Truncated)
        );
    }
    proptest! {
        #![proptest_config(proptest::test_runner::Config {cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x4143454845414452),..Default::default()})]
        #[test]
        fn arbitrary_prefixes_return_without_panicking(bytes in prop::collection::vec(any::<u8>(),0..256),length in any::<u64>()) {let _=Header::parse(&bytes,length);}
    }
}
