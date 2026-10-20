//! Bounded native encryption creation; all output remains unpublished.
use super::{
    EncryptionCopy, agile, legacy,
    native_header::NativeHeader,
    rewrite::{PlainCandidate, capture},
};
use crate::{FileFormat, Header, InspectError, format::page::PAGE_BYTES, header::check};
use std::io::{Cursor, Read, Seek};
use tablecraft_core::limits::{Resource, ResourceBudget};
use zeroize::Zeroizing;

/// Validate an unencoded source, rewrite cap and any existing Jet password before
/// requesting a new credential. Actual creation rechecks the stable/offline source.
pub fn check_encryption_add<R: Read + Seek>(
    source: &mut R,
    password: Option<&str>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    let _span = tracing::debug_span!("native.encryption.add.credentials").entered();
    plain_header(source, password, budget, cancelled)?;
    check(budget, cancelled)
}
fn plain_header<R: Read + Seek>(
    source: &mut R,
    password: Option<&str>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Header, NativeHeader), InspectError> {
    let header = crate::inspect_header(source, budget, cancelled)?;
    if header.encoded_pages() {
        return Err(InspectError::AlreadyEncrypted);
    }
    header.read_profile()?;
    if u64::from(header.page_count()) * u64::from(header.page_size())
        > crate::limits::MAX_IMAGE_BYTES
    {
        return Err(InspectError::StructureLimit);
    }
    budget.charge(Resource::Bytes, PAGE_BYTES as u64)?;
    let mut page = Zeroizing::new([0; PAGE_BYTES]);
    tablecraft_io::read_exact_at(source, 0, &mut *page, budget, cancelled)?;
    let native = NativeHeader::read(&*page)?;
    if header.format() == FileFormat::Jet4 {
        native.verify_jet_password(password)?;
    } else if native.has_password()? {
        return Err(InspectError::UnsupportedEncryption);
    }
    Ok((header, native))
}

pub(super) fn add<R: Read + Seek>(
    source: &mut R,
    password: Option<&str>,
    new_password: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<EncryptionCopy, InspectError> {
    let _span = tracing::debug_span!("native.encryption.add").entered();
    check(budget, cancelled)?;
    if new_password.is_empty() {
        return Err(InspectError::EmptyPassword);
    }
    let (header, mut native) = plain_header(source, password, budget, cancelled)?;
    let mut bytes = capture(source, header, budget, cancelled)?;
    let old_key = native.sid_key()?;
    let mut encoding_key = Zeroizing::new([0; size_of::<u32>()]);
    getrandom::fill(&mut *encoding_key).map_err(|_| InspectError::EntropyUnavailable)?;
    // A zero key means unencoded native pages. Fail rather than publishing such
    // a header; no unbounded RNG retry or nonrandom substitute is used.
    if *encoding_key == [0; size_of::<u32>()] {
        return Err(InspectError::EntropyUnavailable);
    }
    let agile = if header.format() == FileFormat::Jet4 {
        native.jet_password(new_password)?;
        native.encoding_key(*encoding_key);
        native.write(&mut bytes[..PAGE_BYTES])?;
        None
    } else {
        native.office_key(*encoding_key)?;
        native.write(&mut bytes[..PAGE_BYTES])?;
        Some(agile::create(
            &mut bytes[..PAGE_BYTES],
            new_password,
            *encoding_key,
            header.page_count(),
            budget,
            cancelled,
        )?)
    };
    let new_key = native.sid_key()?;
    let mut view = PlainCandidate(Cursor::new(bytes.as_slice()));
    let patches = crate::table::remask(&mut view, &old_key, &new_key, budget, cancelled)?;
    patches.apply(&mut bytes, budget, cancelled)?;
    let jet = if agile.is_none() {
        Some(
            legacy::read(&mut Cursor::new(bytes.as_slice()), budget, cancelled)?
                .unlock(None, budget, cancelled)?,
        )
    } else {
        None
    };
    for (number, page) in bytes
        .as_chunks_mut::<PAGE_BYTES>()
        .0
        .iter_mut()
        .enumerate()
        .skip(1)
    {
        if let Some(encoder) = &agile {
            encoder.encode_page(number as u32, page, budget, cancelled)?;
        } else if let Some(encoder) = &jet {
            encoder.encode_page(number as u32, page, budget, cancelled)?;
        }
    }
    check(budget, cancelled)?;
    Ok(EncryptionCopy { bytes })
}
