//! Page-buffered encryption copies into an unpublished host-owned destination.
use super::{
    AgilePageDecoder, EncryptionInspection, EncryptionRewrite, OfficeEncryptionFamily, PageDecoder,
    agile, legacy, native_header::NativeHeader,
};
use crate::{
    FileFormat, Header, InspectError, NativeReadSource, format::page::PAGE_BYTES, header::check,
};
use std::io::{self, Read, Seek, SeekFrom, Write};
use tablecraft_core::limits::{Resource, ResourceBudget};
use zeroize::Zeroizing;

/// Rewrite a stable/offline database into an empty, unpublished destination.
///
/// This uses zeroizing page buffers and budgeted SID patches rather than capturing
/// the whole image. The native read-file ceiling and the supplied cumulative
/// allowance apply; the separate image API retains its smaller capture ceiling.
/// Source and destination must be distinct resources. This function never
/// publishes or synchronizes the destination. Any failure can leave a prefix:
/// the host must discard it, and publish a fresh output only after success and
/// its required synchronization. No ordinary database edits are authorized.
pub fn rewrite_encryption_to<R: Read + Seek, W: Write + Seek>(
    source: &mut R,
    target: &mut W,
    operation: EncryptionRewrite<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    let _span = tracing::debug_span!("native.encryption.copy_pages").entered();
    check(budget, cancelled)?;
    if target
        .seek(SeekFrom::End(0))
        .map_err(|e| InspectError::Io(e.kind()))?
        != 0
    {
        return Err(InspectError::Io(io::ErrorKind::AlreadyExists));
    }

    let header = crate::inspect_header(source, budget, cancelled)?;
    header.decoded().read_profile()?;
    budget.charge(Resource::Bytes, (2 * PAGE_BYTES) as u64)?;
    let mut original = Zeroizing::new([0; PAGE_BYTES]);
    tablecraft_io::read_exact_at(source, 0, &mut *original, budget, cancelled)?;
    let mut page = Zeroizing::new(*original);
    let mut native = NativeHeader::read(&*original)?;
    let old_key = native.sid_key()?;
    let jet = header.format() == FileFormat::Jet4;
    let (password, replacement, adding) = match operation {
        EncryptionRewrite::Add {
            password,
            new_password,
        } => (password, Some(new_password), true),
        EncryptionRewrite::Remove { password } => (password, None, false),
        EncryptionRewrite::ChangePassword {
            password,
            new_password,
        } => (password, Some(new_password), false),
    };
    if replacement == Some("") {
        return Err(InspectError::EmptyPassword);
    }
    if jet {
        native.verify_jet_password(password)?;
    }
    if adding {
        if header.encoded_pages() {
            return Err(InspectError::AlreadyEncrypted);
        }
        if !jet && native.has_password()? {
            return Err(InspectError::UnsupportedEncryption);
        }
    } else if !header.encoded_pages() && !(jet && native.has_password()?) {
        return Err(InspectError::NotEncrypted);
    }

    let agile_change = !adding
        && replacement.is_some()
        && matches!(
            super::inspect_encryption(source, budget, cancelled)?,
            EncryptionInspection::Office(info) if info.family() == OfficeEncryptionFamily::Agile
        );
    let decoder = if header.encoded_pages() && !agile_change {
        Some(PageDecoder::open(source, password, budget, cancelled)?)
    } else {
        None
    };
    let encoder = if adding {
        create_encoder(
            &mut native,
            &mut *page,
            header,
            replacement.unwrap(),
            budget,
            cancelled,
        )?
    } else if agile_change {
        agile::change_password(
            source,
            &mut *page,
            password,
            replacement.unwrap(),
            budget,
            cancelled,
        )?;
        None
    } else if let Some(replacement) = replacement {
        if jet {
            native.jet_password(replacement)?;
            native.write(&mut *page)?;
            if header.encoded_pages() {
                Some(Encoder::Legacy(
                    legacy::read(source, budget, cancelled)?.unlock(password, budget, cancelled)?,
                ))
            } else {
                None
            }
        } else {
            Some(Encoder::Legacy(legacy::change_password(
                source,
                &mut *page,
                replacement,
                budget,
                cancelled,
            )?))
        }
    } else {
        native.clear()?;
        native.write(&mut *page)?;
        if !jet {
            page[super::OFFICE_INFO_LENGTH_OFFSET..super::OFFICE_INFO_REGION_END].fill(0);
        }
        None
    };

    // Agile password replacement retains the content key, SID key and every
    // ciphertext data page. Other transformations remask qualified native SIDs.
    let new_key = native.sid_key()?;
    let mut view = DecodedView {
        source,
        header,
        decoder: decoder.as_ref(),
    };
    let mut patches = if agile_change {
        crate::table::remask(&mut view, &old_key, &old_key, budget, cancelled)?
    } else {
        crate::table::remask(&mut view, &old_key, &new_key, budget, cancelled)?
    }
    .into_pages();
    verify_header(view.source, header, &original, budget, cancelled)?;
    tablecraft_io::write_exact_at(target, 0, &*page, budget, cancelled)?;

    for number in 1..header.page_count() {
        check(budget, cancelled)?;
        budget.charge(Resource::Bytes, (2 * PAGE_BYTES) as u64)?;
        let offset = u64::from(number) * PAGE_BYTES as u64;
        tablecraft_io::read_exact_at(view.source, offset, &mut *page, budget, cancelled)?;
        if let Some(decoder) = &decoder {
            decoder.decode_page(number, &mut *page, budget, cancelled)?;
        }
        patches.apply(offset as usize, &mut *page, budget, cancelled)?;
        if let Some(encoder) = &encoder {
            encoder.encode(number, &mut *page, budget, cancelled)?;
        }
        tablecraft_io::write_exact_at(target, offset, &*page, budget, cancelled)?;
    }

    patches.finish()?;
    verify_header(view.source, header, &original, budget, cancelled)?;
    check(budget, cancelled)
}

enum Encoder {
    Agile(AgilePageDecoder),
    Legacy(legacy::Decoder),
}

impl Encoder {
    fn encode(
        &self,
        number: u32,
        page: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        match self {
            Self::Agile(encoder) => encoder.encode_page(number, page, budget, cancelled),
            Self::Legacy(encoder) => encoder.encode_page(number, page, budget, cancelled),
        }
    }
}

fn create_encoder(
    native: &mut NativeHeader,
    page: &mut [u8],
    header: Header,
    password: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<Encoder>, InspectError> {
    let mut key = Zeroizing::new([0; size_of::<u32>()]);
    getrandom::fill(&mut *key).map_err(|_| InspectError::EntropyUnavailable)?;
    if *key == [0; size_of::<u32>()] {
        return Err(InspectError::EntropyUnavailable);
    }
    let encoder = if header.format() == FileFormat::Jet4 {
        native.jet_password(password)?;
        native.encoding_key(*key);
        native.write(page)?;
        let length = u64::from(header.page_count()) * PAGE_BYTES as u64;
        Encoder::Legacy(legacy::from_page(page, length)?.unlock(None, budget, cancelled)?)
    } else {
        native.office_key(*key)?;
        native.write(page)?;
        Encoder::Agile(agile::create(
            page,
            password,
            *key,
            header.page_count(),
            budget,
            cancelled,
        )?)
    };
    Ok(Some(encoder))
}

struct DecodedView<'a, R> {
    source: &'a mut R,
    header: Header,
    decoder: Option<&'a PageDecoder>,
}

impl<R: Read> Read for DecodedView<'_, R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.source.read(bytes)
    }
}

impl<R: Seek> Seek for DecodedView<'_, R> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.source.seek(from)
    }
}

impl<R: Read + Seek> NativeReadSource for DecodedView<'_, R> {
    fn qualify_header(&self, header: Header) -> Result<Header, InspectError> {
        if header != self.header {
            return Err(InspectError::Malformed);
        }
        Ok(header.decoded())
    }

    fn decode_native_page(
        &mut self,
        number: u32,
        page: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        if let Some(decoder) = self.decoder {
            decoder.decode_page(number, page, budget, cancelled)?;
        }
        Ok(())
    }
}

fn verify_header<R: Read + Seek>(
    source: &mut R,
    header: Header,
    original: &[u8; PAGE_BYTES],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    if crate::inspect_header(source, budget, cancelled)? != header {
        return Err(InspectError::Malformed);
    }
    budget.charge(Resource::Bytes, PAGE_BYTES as u64)?;
    let mut page = Zeroizing::new([0; PAGE_BYTES]);
    tablecraft_io::read_exact_at(source, 0, &mut *page, budget, cancelled)?;
    if *page != *original {
        return Err(InspectError::Malformed);
    }
    Ok(())
}
