//! Streamed CAB geometry qualification before decoder allocations or spool creation.
use super::{
    AccessInputError, DATA_HEADER_BYTES, FILE_BYTES, FILE_COUNT_OFFSET, FILE_TABLE_OFFSET,
    FLAGS_OFFSET, FOLDER_BYTES, FOLDER_COUNT_OFFSET, HEADER_BYTES, MAX_BLOCK_BYTES,
    MAX_COMPRESSED_BLOCK_BYTES, MAX_LZX_WINDOW_BITS, MAX_NAME_BYTES, RESERVE_PRESENT,
    RESERVE_SIZES_BYTES, SET_INDEX_OFFSET, SIZE_OFFSET, VERSION_OFFSET, check,
};
use std::io::{Read, Seek};
use tablecraft_ace::{Header, InspectError};
use tablecraft_core::limits::{Resource, ResourceBudget};

// These are CAB's own single-folder limits, distinct from the native source envelope.
const MAX_CABINET_BYTES: u64 = i32::MAX as u64;
const MAX_DATABASE_BYTES: u64 = u16::MAX as u64 * MAX_BLOCK_BYTES as u64;
const MAX_OVERLAY_BYTES: u64 = 64 * 1024 * 1024;
const MIN_LZX_WINDOW_BITS: u16 = 15;
const BLOCK_WORK_BYTES: u64 = 128;
// Covers decoded filename expansion, retained FileEntry clones and our fallible name copy.
const FILE_WORK_BYTES: u64 = 4096;
const CAB_SIGNATURE: &[u8] = b"MSCF";
const CAB_VERSION: &[u8] = &[3, 1];
const FOLDER_DATA_OFFSET: usize = 0;
const FOLDER_BLOCK_COUNT_OFFSET: usize = size_of::<u32>();
const FOLDER_COMPRESSION_OFFSET: usize = FOLDER_BLOCK_COUNT_OFFSET + size_of::<u16>();
const FILE_SIZE_OFFSET: usize = 0;
const FILE_DATA_OFFSET: usize = size_of::<u32>();
const FILE_FOLDER_OFFSET: usize = FILE_DATA_OFFSET + size_of::<u32>();
const BLOCK_COMPRESSED_SIZE_OFFSET: usize = size_of::<u32>();
const BLOCK_PLAIN_SIZE_OFFSET: usize = BLOCK_COMPRESSED_SIZE_OFFSET + size_of::<u16>();
const HEADER_RESERVE_SIZE_OFFSET: usize = 0;
const FOLDER_RESERVE_SIZE_OFFSET: usize = size_of::<u16>();
const DATA_RESERVE_SIZE_OFFSET: usize = FOLDER_RESERVE_SIZE_OFFSET + size_of::<u8>();
const COMPRESSION_METHOD_MASK: u16 = 0x000f;
const COMPRESSION_RESERVED_MASK: u16 = 0x00f0;
const COMPRESSION_WINDOW_SHIFT: u32 = 8;
const COMPRESSION_NONE: u16 = 0;
const COMPRESSION_MSZIP: u16 = 1;
const COMPRESSION_LZX: u16 = 3;

pub(super) struct Layout {
    pub physical_bytes: u64,
    pub cabinet_bytes: u64,
    pub database_bytes: u64,
    pub allocation_bytes: u64,
}

pub(super) fn read<R: Read + Seek>(
    source: &mut R,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Layout, AccessInputError> {
    check(budget, cancelled)?;
    let physical =
        tablecraft_io::seek_to_end(source, budget, cancelled).map_err(InspectError::from)?;
    if !(HEADER_BYTES as u64..=MAX_CABINET_BYTES + MAX_OVERLAY_BYTES).contains(&physical) {
        return Err(InspectError::StructureLimit.into());
    }
    let header = at::<HEADER_BYTES, _>(source, 0, physical, budget, cancelled)?;
    if &header[..CAB_SIGNATURE.len()] != CAB_SIGNATURE
        || &header[VERSION_OFFSET..VERSION_OFFSET + CAB_VERSION.len()] != CAB_VERSION
    {
        return Err(AccessInputError::MalformedPackage);
    }
    let end = dword(&header, SIZE_OFFSET);
    if end > MAX_CABINET_BYTES || physical.saturating_sub(end) > MAX_OVERLAY_BYTES {
        return Err(InspectError::StructureLimit.into());
    }
    if !(HEADER_BYTES as u64..=physical).contains(&end) {
        return Err(AccessInputError::MalformedPackage);
    }
    if word(&header, FOLDER_COUNT_OFFSET) != 1
        || word(&header, FILE_COUNT_OFFSET) != 1
        || word(&header, FLAGS_OFFSET) & !RESERVE_PRESENT != 0
        || word(&header, SET_INDEX_OFFSET) != 0
    {
        return Err(AccessInputError::UnsupportedPackage);
    }

    let mut folder_offset = HEADER_BYTES as u64;
    let mut header_reserve = 0;
    let mut folder_reserve = 0;
    let mut data_reserve = 0;
    if word(&header, FLAGS_OFFSET) & RESERVE_PRESENT != 0 {
        let sizes = at::<RESERVE_SIZES_BYTES, _>(source, folder_offset, end, budget, cancelled)?;
        header_reserve = u64::from(word(&sizes, HEADER_RESERVE_SIZE_OFFSET));
        folder_reserve = u64::from(sizes[FOLDER_RESERVE_SIZE_OFFSET]);
        data_reserve = u64::from(sizes[DATA_RESERVE_SIZE_OFFSET]);
        folder_offset += RESERVE_SIZES_BYTES as u64 + header_reserve;
    }
    let folder = at::<FOLDER_BYTES, _>(source, folder_offset, end, budget, cancelled)?;
    let mut block_offset = dword(&folder, FOLDER_DATA_OFFSET);
    let blocks = word(&folder, FOLDER_BLOCK_COUNT_OFFSET);
    let compression = word(&folder, FOLDER_COMPRESSION_OFFSET);
    match compression & COMPRESSION_METHOD_MASK {
        COMPRESSION_NONE | COMPRESSION_MSZIP if compression <= COMPRESSION_MSZIP => (),
        COMPRESSION_LZX
            if (MIN_LZX_WINDOW_BITS..=MAX_LZX_WINDOW_BITS)
                .contains(&(compression >> COMPRESSION_WINDOW_SHIFT))
                && compression & COMPRESSION_RESERVED_MASK == 0 => {}
        _ => return Err(AccessInputError::UnsupportedPackage),
    }
    let file_offset = dword(&header, FILE_TABLE_OFFSET);
    if file_offset < folder_offset + FOLDER_BYTES as u64 + folder_reserve {
        return Err(AccessInputError::MalformedPackage);
    }
    let file = at::<FILE_BYTES, _>(source, file_offset, end, budget, cancelled)?;
    let size = dword(&file, FILE_SIZE_OFFSET);
    if !(Header::PREFIX_BYTES as u64..=MAX_DATABASE_BYTES).contains(&size) {
        return Err(InspectError::StructureLimit.into());
    }
    if dword(&file, FILE_DATA_OFFSET) != 0 || word(&file, FILE_FOLDER_OFFSET) != 0 || blocks == 0 {
        return Err(AccessInputError::UnsupportedPackage);
    }

    let name_start = file_offset + FILE_BYTES as u64;
    let mut name_length = 0;
    let mut dots_only = true;
    loop {
        if name_length > MAX_NAME_BYTES {
            return Err(AccessInputError::MalformedPackage);
        }
        let [byte] = at::<1, _>(
            source,
            name_start + name_length as u64,
            end,
            budget,
            cancelled,
        )?;
        if byte == 0 {
            break;
        }
        if byte < b' ' || matches!(byte, b':' | b'/' | b'\\') {
            return Err(AccessInputError::UnsupportedPackage);
        }
        dots_only &= byte == b'.';
        name_length += 1;
    }
    if name_length == 0 || (dots_only && name_length <= 2) {
        return Err(AccessInputError::UnsupportedPackage);
    }
    if block_offset < name_start + name_length as u64 + 1 {
        return Err(AccessInputError::MalformedPackage);
    }

    let mut expanded = 0u64;
    for _ in 0..blocks {
        check(budget, cancelled)?;
        let block = at::<DATA_HEADER_BYTES, _>(source, block_offset, end, budget, cancelled)?;
        let compressed = usize::from(word(&block, BLOCK_COMPRESSED_SIZE_OFFSET));
        let plain = usize::from(word(&block, BLOCK_PLAIN_SIZE_OFFSET));
        if !(1..=MAX_COMPRESSED_BLOCK_BYTES).contains(&compressed)
            || !(1..=MAX_BLOCK_BYTES).contains(&plain)
            || (compression == COMPRESSION_NONE && compressed != plain)
        {
            return Err(AccessInputError::MalformedPackage);
        }
        block_offset = block_offset
            .checked_add(DATA_HEADER_BYTES as u64 + data_reserve + compressed as u64)
            .ok_or(AccessInputError::MalformedPackage)?;
        expanded = expanded
            .checked_add(plain as u64)
            .ok_or(AccessInputError::MalformedPackage)?;
        if block_offset > end || expanded > size {
            return Err(AccessInputError::MalformedPackage);
        }
    }
    if expanded != size {
        return Err(AccessInputError::MalformedPackage);
    }
    Ok(Layout {
        physical_bytes: physical,
        cabinet_bytes: end,
        database_bytes: size,
        allocation_bytes: header_reserve
            + folder_reserve
            + FILE_WORK_BYTES
            + u64::from(blocks) * (BLOCK_WORK_BYTES + data_reserve),
    })
}

fn at<const N: usize, R: Read + Seek>(
    source: &mut R,
    offset: u64,
    end: u64,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<[u8; N], AccessInputError> {
    if offset.checked_add(N as u64).is_none_or(|next| next > end) {
        return Err(AccessInputError::MalformedPackage);
    }
    budget
        .charge(Resource::Bytes, N as u64)
        .map_err(InspectError::from)?;
    let mut bytes = [0; N];
    tablecraft_io::read_exact_at(source, offset, &mut bytes, budget, cancelled)
        .map_err(InspectError::from)?;
    Ok(bytes)
}

fn word(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + size_of::<u16>()].try_into().unwrap())
}

fn dword(bytes: &[u8], offset: usize) -> u64 {
    u64::from(u32::from_le_bytes(
        bytes[offset..offset + size_of::<u32>()].try_into().unwrap(),
    ))
}

#[cfg(test)]
mod tests;
