//! Explicit bounded output copies; never writes the supplied source.
use super::{
    EncryptionInspection, OfficeEncryptionFamily, PageDecoder, agile, legacy,
    native_header::NativeHeader,
};
use crate::{
    FileFormat, Header, InspectError, NativeReadSource, format::page::PAGE_BYTES, header::check,
};
use std::io::{self, Cursor, Read, Seek, SeekFrom};
use tablecraft_core::limits::{Resource, ResourceBudget};
use zeroize::Zeroizing;

/// An explicit encryption-only transformation of an offline source.
/// Credentials are borrowed and are never included in Debug or diagnostics.
pub enum EncryptionRewrite<'a> {
    /// Remove page encoding and a qualified native database password.
    Remove {
        /// Current password, if the source uses one.
        password: Option<&'a str>,
    },
    /// Replace a password while retaining the existing encryption scheme.
    ChangePassword {
        /// Current password, if the source uses one.
        password: Option<&'a str>,
        /// Nonempty replacement, within the source scheme's native length bound.
        new_password: &'a str,
    },
}

/// Unpublished encryption rewrite image, bounded to the existing 64 MiB image cap.
/// This is not a normal edit plan, transaction or permission grant. Hosts must
/// publish to a fresh explicit output and leave the offline source unchanged.
pub struct EncryptionCopy {
    bytes: Zeroizing<Vec<u8>>,
}
impl EncryptionCopy {
    /// Explicit output bytes. Removal intentionally contains plaintext data;
    /// the owned buffer is erased on drop, unlike host-created copies.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Check source geometry, rewrite size, scheme and current credentials before a
/// host requests a new password. This does not inspect all SID rows or authorize
/// an edit; actual rewriting rechecks the stable/offline source and credentials.
pub fn check_encryption_rewrite<R: Read + Seek>(
    source: &mut R,
    password: Option<&str>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    let _span = tracing::debug_span!("native.encryption.credentials").entered();
    let header = crate::inspect_header(source, budget, cancelled)?;
    header.decoded().read_profile()?;
    if u64::from(header.page_count()) * u64::from(header.page_size())
        > crate::limits::MAX_IMAGE_BYTES
    {
        return Err(InspectError::StructureLimit);
    }
    budget.charge(Resource::Bytes, PAGE_BYTES as u64)?;
    let mut page = Zeroizing::new([0; PAGE_BYTES]);
    tablecraft_io::read_exact_at(source, 0, &mut *page, budget, cancelled)?;
    if header.format() == FileFormat::Jet4 {
        let native = NativeHeader::read(&*page)?;
        native.verify_jet_password(password)?;
        if !header.encoded_pages() && !native.has_password()? {
            return Err(InspectError::NotEncrypted);
        }
    } else if !header.encoded_pages() {
        return Err(InspectError::NotEncrypted);
    }
    if header.encoded_pages() {
        PageDecoder::open(source, password, budget, cancelled)?;
    }
    check(budget, cancelled)
}

/// Prepare a fresh encryption/password output without writing the input source.
/// Uses bounded authenticated codecs, same-width unindexed SID remasking and
/// preserves decoded content outside required header/SID changes. Unsupported schemes/layouts fail before
/// returning a candidate. No macros or other database content are executed.
pub fn rewrite_encryption<R: Read + Seek>(
    source: &mut R,
    operation: EncryptionRewrite<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<EncryptionCopy, InspectError> {
    let changing = matches!(&operation, EncryptionRewrite::ChangePassword { .. });
    let _span =
        tracing::debug_span!("native.encryption.rewrite", change_password = changing).entered();
    check(budget, cancelled)?;
    let (password, new_password) = match operation {
        EncryptionRewrite::Remove { password } => (password, None),
        EncryptionRewrite::ChangePassword {
            password,
            new_password,
        } => {
            if new_password.is_empty() {
                return Err(InspectError::EmptyPassword);
            }
            (password, Some(new_password))
        }
    };
    let header = crate::inspect_header(source, budget, cancelled)?;
    header.decoded().read_profile()?;
    let length = u64::from(header.page_count()) * u64::from(header.page_size());
    if length > crate::limits::MAX_IMAGE_BYTES {
        return Err(InspectError::StructureLimit);
    }
    budget.charge(Resource::Bytes, PAGE_BYTES as u64)?;
    let mut page = Zeroizing::new([0; PAGE_BYTES]);
    tablecraft_io::read_exact_at(source, 0, &mut *page, budget, cancelled)?;
    let mut native = NativeHeader::read(&*page)?;
    let jet = header.format() == FileFormat::Jet4;
    if jet {
        native.verify_jet_password(password)?;
    }
    if !header.encoded_pages() && !(jet && native.has_password()?) {
        return Err(InspectError::NotEncrypted);
    }
    let agile_change = new_password.is_some()
        && matches!(super::inspect_encryption(source, budget, cancelled)?, EncryptionInspection::Office(info) if info.family() == OfficeEncryptionFamily::Agile);
    let decoder = if header.encoded_pages() && !agile_change {
        Some(PageDecoder::open(source, password, budget, cancelled)?)
    } else {
        None
    };
    if agile_change {
        agile::change_password(
            source,
            &mut *page,
            password,
            new_password.unwrap(),
            budget,
            cancelled,
        )?;
    }
    budget.charge(Resource::Bytes, length)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length as usize)
        .map_err(|_| InspectError::Allocation)?;
    bytes.resize(length as usize, 0);
    let mut bytes = Zeroizing::new(bytes);
    tablecraft_io::read_exact_at(source, 0, bytes.as_mut_slice(), budget, cancelled)?;
    if Header::parse(&bytes, length)? != header {
        return Err(InspectError::Malformed);
    }
    if agile_change {
        bytes[..PAGE_BYTES].copy_from_slice(&*page);
        check(budget, cancelled)?;
        return Ok(EncryptionCopy { bytes });
    }
    for (number, page) in bytes
        .as_chunks_mut::<PAGE_BYTES>()
        .0
        .iter_mut()
        .enumerate()
        .skip(1)
    {
        if let Some(decoder) = &decoder {
            decoder.decode_page(number as u32, page, budget, cancelled)?;
        } else {
            check(budget, cancelled)?;
        }
    }
    let old_sid_key = native.sid_key()?;
    let encoder = if let Some(new_password) = new_password {
        if jet {
            native.jet_password(new_password)?;
            native.write(&mut bytes[..PAGE_BYTES])?;
            if header.encoded_pages() {
                Some(legacy::read(source, budget, cancelled)?.unlock(password, budget, cancelled)?)
            } else {
                None
            }
        } else {
            Some(legacy::change_password(
                source,
                &mut bytes[..PAGE_BYTES],
                new_password,
                budget,
                cancelled,
            )?)
        }
    } else {
        native.clear()?;
        native.write(&mut bytes[..PAGE_BYTES])?;
        if !jet {
            bytes[super::OFFICE_INFO_LENGTH_OFFSET..super::OFFICE_INFO_REGION_END].fill(0);
        }
        None
    };
    let new_sid_key = native.sid_key()?;
    let mut view = PlainCandidate(Cursor::new(bytes.as_slice()));
    let patches = crate::table::remask(&mut view, &old_sid_key, &new_sid_key, budget, cancelled)?;
    patches.apply(&mut bytes, budget, cancelled)?;
    if let Some(encoder) = encoder {
        for (number, page) in bytes
            .as_chunks_mut::<PAGE_BYTES>()
            .0
            .iter_mut()
            .enumerate()
            .skip(1)
        {
            encoder.encode_page(number as u32, page, budget, cancelled)?;
        }
    }
    check(budget, cancelled)?;
    Ok(EncryptionCopy { bytes })
}

// Explicit plaintext staging context, private to this rewrite implementation.
// Raw sources never receive this context merely by changing header bytes.
struct PlainCandidate<'a>(Cursor<&'a [u8]>);
impl Read for PlainCandidate<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.0.read(bytes)
    }
}
impl Seek for PlainCandidate<'_> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.0.seek(from)
    }
}
impl NativeReadSource for PlainCandidate<'_> {
    fn qualify_header(&self, header: Header) -> Result<Header, InspectError> {
        Ok(header.decoded())
    }
}
