//! CAB geometry gates ahead of the library's parser/decompressor allocations.
mod bounded;
mod hybrid;
mod layout;
pub(super) use hybrid::{Selected, read as read_with_spool};
mod spool;
use super::AccessInputError;
pub(super) use spool::read as read_spooled;
use std::io::{Cursor, Read, Seek, SeekFrom};
use tablecraft_ace::{FileFormat, Header, InspectError};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAX_PACKAGE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_DATABASE_BYTES: usize = 64 * 1024 * 1024;
const HEADER_BYTES: usize = 36;
const SIZE_OFFSET: usize = 8;
const FILE_TABLE_OFFSET: usize = 16;
const VERSION_OFFSET: usize = 24;
const FOLDER_COUNT_OFFSET: usize = 26;
const FILE_COUNT_OFFSET: usize = 28;
const FLAGS_OFFSET: usize = 30;
const SET_INDEX_OFFSET: usize = 34;
const RESERVE_PRESENT: u16 = 4;
const RESERVE_SIZES_BYTES: usize = 4;
const FOLDER_BYTES: usize = 8;
const FILE_BYTES: usize = 16;
const MAX_NAME_BYTES: usize = 255;
const DATA_HEADER_BYTES: usize = 8;
const MAX_BLOCK_BYTES: usize = 32 * 1024;
const MAX_COMPRESSED_BLOCK_BYTES: usize = MAX_BLOCK_BYTES + 6144;
const MAX_LZX_WINDOW_BITS: u16 = 21;
const DECOMPRESSOR_WORK_BYTES: u64 = 4 * 1024 * 1024;
const COPY_CHUNK_BYTES: usize = 32 * 1024;

pub(super) fn read<R: Read + Seek>(
    source: &mut R,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, AccessInputError> {
    let _span = tracing::debug_span!("access.deployment.read").entered();
    check(budget, cancelled)?;
    let length = source.seek(SeekFrom::End(0)).map_err(InspectError::from)?;
    if !(HEADER_BYTES as u64..=MAX_PACKAGE_BYTES).contains(&length) {
        return Err(InspectError::StructureLimit.into());
    }
    let mut bytes = allocate(length as usize, budget)?;
    tablecraft_io::read_exact_at(source, 0, &mut bytes, budget, cancelled)
        .map_err(InspectError::from)?;
    let size = preflight(&bytes, budget, cancelled)?;
    // Preflight validated this logical end. Retain the physical capture charge,
    // but never present an unverified signature overlay to the CAB decoder.
    let cabinet_bytes = dword(&bytes, SIZE_OFFSET)?;
    bytes.truncate(cabinet_bytes);
    budget
        .charge(Resource::Bytes, DECOMPRESSOR_WORK_BYTES)
        .map_err(InspectError::from)?;
    let mut cabinet =
        cab::Cabinet::new(Cursor::new(bytes)).map_err(|_| AccessInputError::MalformedPackage)?;
    // Preflight requires exactly one file. Its original name is data only.
    let name = cabinet
        .folder_entries()
        .next()
        .and_then(|folder| folder.file_entries().next())
        .ok_or(AccessInputError::MalformedPackage)?
        .name()
        .to_owned();
    let mut data = allocate(size, budget)?;
    let mut reader = cabinet
        .read_file(&name)
        .map_err(|_| AccessInputError::MalformedPackage)?;
    for chunk in data.chunks_mut(COPY_CHUNK_BYTES) {
        check(budget, cancelled)?;
        tablecraft_io::read_exact(&mut reader, chunk, budget, cancelled).map_err(|error| {
            match error {
                tablecraft_io::IoError::Io(_) => AccessInputError::MalformedPackage,
                error => AccessInputError::Native(error.into()),
            }
        })?;
    }
    let header = Header::parse(&data, size as u64)?;
    if !matches!(
        header.format(),
        FileFormat::Ace2007 | FileFormat::Ace2010 | FileFormat::Ace2016 | FileFormat::Ace2019
    ) {
        return Err(AccessInputError::UnsupportedPackage);
    }
    check(budget, cancelled)?;
    Ok(data)
}

fn allocate(size: usize, budget: &mut ResourceBudget) -> Result<Vec<u8>, AccessInputError> {
    budget
        .charge(Resource::Bytes, size as u64)
        .map_err(InspectError::from)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(|_| InspectError::Allocation)?;
    bytes.resize(size, 0);
    Ok(bytes)
}
fn check(
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), AccessInputError> {
    if cancelled() {
        return Err(InspectError::Cancelled.into());
    }
    budget
        .charge(Resource::Steps, 1)
        .map_err(InspectError::from)?;
    Ok(())
}
fn word(bytes: &[u8], at: usize) -> Result<u16, AccessInputError> {
    Ok(u16::from_le_bytes(
        bytes
            .get(at..at + size_of::<u16>())
            .ok_or(AccessInputError::MalformedPackage)?
            .try_into()
            .unwrap(),
    ))
}
fn dword(bytes: &[u8], at: usize) -> Result<usize, AccessInputError> {
    Ok(u32::from_le_bytes(
        bytes
            .get(at..at + size_of::<u32>())
            .ok_or(AccessInputError::MalformedPackage)?
            .try_into()
            .unwrap(),
    ) as usize)
}

fn preflight(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, AccessInputError> {
    check(budget, cancelled)?;
    let malformed = AccessInputError::MalformedPackage;
    if bytes.get(..4) != Some(b"MSCF")
        || bytes.get(VERSION_OFFSET..VERSION_OFFSET + 2) != Some(&[3, 1])
    {
        return Err(malformed);
    }
    let end = dword(bytes, SIZE_OFFSET)?;
    if !(HEADER_BYTES..=bytes.len()).contains(&end) {
        return Err(malformed);
    }
    let bytes = &bytes[..end]; // Any trailing signature bytes are not input to the CAB decoder.
    if word(bytes, FOLDER_COUNT_OFFSET)? != 1
        || word(bytes, FILE_COUNT_OFFSET)? != 1
        || word(bytes, FLAGS_OFFSET)? & !RESERVE_PRESENT != 0
        || word(bytes, SET_INDEX_OFFSET)? != 0
    {
        return Err(AccessInputError::UnsupportedPackage);
    }
    let mut folder = HEADER_BYTES;
    let (folder_reserve, data_reserve) = if word(bytes, FLAGS_OFFSET)? & RESERVE_PRESENT != 0 {
        let header_reserve = usize::from(word(bytes, HEADER_BYTES)?);
        let sizes = bytes
            .get(HEADER_BYTES..HEADER_BYTES + RESERVE_SIZES_BYTES)
            .ok_or(AccessInputError::MalformedPackage)?;
        folder += RESERVE_SIZES_BYTES + header_reserve;
        (usize::from(sizes[2]), usize::from(sizes[3]))
    } else {
        (0, 0)
    };
    let mut block = dword(bytes, folder)?;
    let blocks = word(bytes, folder + 4)?;
    let compression = word(bytes, folder + 6)?;
    match compression & 15 {
        0 | 1 if compression <= 1 => (),
        3 if (15..=MAX_LZX_WINDOW_BITS).contains(&(compression >> 8))
            && compression & 0xf0 == 0 => {}
        _ => return Err(AccessInputError::UnsupportedPackage),
    }
    let file = dword(bytes, FILE_TABLE_OFFSET)?;
    if file < folder + FOLDER_BYTES + folder_reserve {
        return Err(AccessInputError::MalformedPackage);
    }
    let size = dword(bytes, file)?;
    if !(Header::PREFIX_BYTES..=MAX_DATABASE_BYTES).contains(&size) {
        return Err(InspectError::StructureLimit.into());
    }
    // Offset zero prevents hidden decompression work before the selected file;
    // folder zero rejects split cabinet file markers and ambiguous selection.
    if dword(bytes, file + 4)? != 0 || word(bytes, file + 8)? != 0 || blocks == 0 {
        return Err(AccessInputError::UnsupportedPackage);
    }
    let name_start = file + FILE_BYTES;
    let name = bytes
        .get(name_start..)
        .ok_or(AccessInputError::MalformedPackage)?;
    let name_length = name
        .iter()
        .take(MAX_NAME_BYTES + 1)
        .position(|b| *b == 0)
        .ok_or(AccessInputError::MalformedPackage)?;
    let name = &name[..name_length];
    if name.is_empty()
        || name
            .iter()
            .any(|b| *b < b' ' || matches!(b, b':' | b'/' | b'\\'))
        || name == b"."
        || name == b".."
    {
        return Err(AccessInputError::UnsupportedPackage);
    }
    if block < name_start + name.len() + 1 {
        return Err(AccessInputError::MalformedPackage);
    }
    let mut expanded = 0;
    for _ in 0..blocks {
        check(budget, cancelled)?;
        let compressed = usize::from(word(bytes, block + 4)?);
        let plain = usize::from(word(bytes, block + 6)?);
        if !(1..=MAX_COMPRESSED_BLOCK_BYTES).contains(&compressed)
            || !(1..=MAX_BLOCK_BYTES).contains(&plain)
            || (compression == 0 && compressed != plain)
        {
            return Err(AccessInputError::MalformedPackage);
        }
        block += DATA_HEADER_BYTES + data_reserve + compressed;
        expanded += plain;
        if block > bytes.len() || expanded > size {
            return Err(AccessInputError::MalformedPackage);
        }
    }
    if expanded != size {
        return Err(AccessInputError::MalformedPackage);
    }
    Ok(size)
}
