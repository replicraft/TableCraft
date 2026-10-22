//! Geometry checks precede dependency metadata/decompression allocations.
use super::{
    MAX_EXPANDED_BYTES, MAX_PACKAGE_BYTES, MAX_PARTS, TemplateError, allocate, check, name,
};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read, Seek, SeekFrom};
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};
use zip::{CompressionMethod, ZipArchive};

const END_BYTES: usize = 22;
const COMMENT_MAX_BYTES: usize = u16::MAX as usize;
const CENTRAL_BYTES: usize = 46;
const LOCAL_BYTES: usize = 30;
const CHUNK_BYTES: usize = 32 * 1024;
const METADATA_BYTES_PER_ENTRY: usize = 4096;
const DECODER_WORK_BYTES: u64 = 256 * 1024;
const END_SIGNATURE: &[u8] = b"PK\x05\x06";
const CENTRAL_SIGNATURE: &[u8] = b"PK\x01\x02";
const LOCAL_SIGNATURE: &[u8] = b"PK\x03\x04";
const DATA_DESCRIPTOR_FLAG: usize = 0x0008;
const ALLOWED_FLAGS: usize = 0x080e;
const END_DISK: usize = 4;
const END_CENTRAL_DISK: usize = 6;
const END_DISK_ENTRIES: usize = 8;
const END_ENTRIES: usize = 10;
const END_CENTRAL_SIZE: usize = 12;
const END_CENTRAL_START: usize = 16;
const END_COMMENT_LENGTH: usize = 20;
const CENTRAL_FLAGS: usize = 8;
const CENTRAL_METHOD: usize = 10;
const CENTRAL_CRC: usize = 16;
const CENTRAL_COMPRESSED_SIZE: usize = 20;
const CENTRAL_EXPANDED_SIZE: usize = 24;
const CENTRAL_NAME_LENGTH: usize = 28;
const CENTRAL_EXTRA_LENGTH: usize = 30;
const CENTRAL_COMMENT_LENGTH: usize = 32;
const CENTRAL_DISK: usize = 34;
const CENTRAL_LOCAL_START: usize = 42;
const LOCAL_FLAGS: usize = 6;
const LOCAL_METHOD: usize = 8;
const LOCAL_CRC: usize = 14;
const LOCAL_COMPRESSED_SIZE: usize = 18;
const LOCAL_EXPANDED_SIZE: usize = 22;
const LOCAL_NAME_LENGTH: usize = 26;
const LOCAL_EXTRA_LENGTH: usize = 28;
const ZIP64_EXTRA_TAG: usize = 1;
const EXTRA_HEADER_BYTES: usize = 2 * size_of::<u16>();

const STORED_METHOD: usize = 0;
const DEFLATE_METHOD: usize = 8;

pub(super) fn read<R: Read + Seek>(
    source: &mut R,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<BTreeMap<String, Vec<u8>>, TemplateError> {
    check(budget, cancelled)?;
    let length = source.seek(SeekFrom::End(0)).map_err(InspectError::from)?;
    if length < END_BYTES as u64 || length > MAX_PACKAGE_BYTES as u64 {
        return Err(TemplateError::Unsupported);
    }
    let mut bytes = allocate(length as usize, budget)?;
    tablecraft_io::read_exact_at(source, 0, &mut bytes, budget, cancelled)
        .map_err(InspectError::from)?;
    let count = preflight(&bytes, budget, cancelled)?;
    budget
        .charge(Resource::Bytes, DECODER_WORK_BYTES)
        .map_err(InspectError::from)?;
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|_| TemplateError::Malformed)?;
    if archive.len() != count || archive.offset() != 0 {
        return Err(TemplateError::Malformed);
    }
    if archive
        .has_overlapping_entries()
        .map_err(|_| TemplateError::Malformed)?
    {
        return Err(TemplateError::Malformed);
    }
    let mut parts = BTreeMap::new();
    let mut decoded_names = BTreeSet::new();
    for index in 0..count {
        check(budget, cancelled)?;
        let mut file = archive
            .by_index(index)
            .map_err(|_| TemplateError::Malformed)?;
        if !file.is_file()
            || file.encrypted()
            || !matches!(
                file.compression(),
                CompressionMethod::Stored | CompressionMethod::Deflated
            )
        {
            return Err(TemplateError::Unsupported);
        }
        let part_name = file
            .name()
            .map_err(|_| TemplateError::Malformed)?
            .into_owned();
        name(&part_name)?;
        if !decoded_names.insert(part_name.to_ascii_lowercase()) {
            return Err(TemplateError::Malformed);
        }
        let mut data = allocate(file.size() as usize, budget)?;
        for chunk in data.chunks_mut(CHUNK_BYTES) {
            check(budget, cancelled)?;
            file.read_exact(chunk)
                .map_err(|_| TemplateError::Malformed)?;
        }
        check(budget, cancelled)?;
        // Read through EOF to validate the CRC even for empty entries.
        if file.read(&mut [0]).map_err(|_| TemplateError::Malformed)? != 0 {
            return Err(TemplateError::Malformed);
        }
        if parts.insert(part_name, data).is_some() {
            return Err(TemplateError::Malformed);
        }
    }
    Ok(parts)
}

fn word(bytes: &[u8], at: usize) -> Result<usize, TemplateError> {
    Ok(u16::from_le_bytes(
        bytes
            .get(at..at + size_of::<u16>())
            .ok_or(TemplateError::Malformed)?
            .try_into()
            .unwrap(),
    ) as usize)
}
fn dword(bytes: &[u8], at: usize) -> Result<usize, TemplateError> {
    Ok(u32::from_le_bytes(
        bytes
            .get(at..at + size_of::<u32>())
            .ok_or(TemplateError::Malformed)?
            .try_into()
            .unwrap(),
    ) as usize)
}

fn preflight(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, TemplateError> {
    let start = bytes.len().saturating_sub(END_BYTES + COMMENT_MAX_BYTES);
    let mut end = None;
    for at in (start..=bytes.len() - END_BYTES).rev() {
        check(budget, cancelled)?;
        if bytes.get(at..at + END_SIGNATURE.len()) == Some(END_SIGNATURE)
            && at + END_BYTES + word(bytes, at + END_COMMENT_LENGTH)? == bytes.len()
        {
            end = Some(at);
            break;
        }
    }
    let end = end.ok_or(TemplateError::Malformed)?;
    let count = word(bytes, end + END_ENTRIES)?;
    let central_size = dword(bytes, end + END_CENTRAL_SIZE)?;
    let central_start = dword(bytes, end + END_CENTRAL_START)?;
    if word(bytes, end + END_DISK)? != 0
        || word(bytes, end + END_CENTRAL_DISK)? != 0
        || word(bytes, end + END_DISK_ENTRIES)? != count
        || count == 0
        || count > MAX_PARTS
        || central_start.checked_add(central_size) != Some(end)
    {
        return Err(TemplateError::Unsupported);
    }
    budget
        .charge(
            Resource::Bytes,
            (count * METADATA_BYTES_PER_ENTRY + 2 * central_size) as u64,
        )
        .map_err(InspectError::from)?;
    let mut at = central_start;
    let mut expanded = 0usize;
    let mut names = BTreeSet::new();
    for _ in 0..count {
        check(budget, cancelled)?;
        if bytes.get(at..at + CENTRAL_SIGNATURE.len()) != Some(CENTRAL_SIGNATURE)
            || word(bytes, at + CENTRAL_DISK)? != 0
        {
            return Err(TemplateError::Malformed);
        }
        let flags = word(bytes, at + CENTRAL_FLAGS)?;
        let method = word(bytes, at + CENTRAL_METHOD)?;
        if flags & !ALLOWED_FLAGS != 0 || !matches!(method, STORED_METHOD | DEFLATE_METHOD) {
            return Err(TemplateError::Unsupported);
        }
        let packed = dword(bytes, at + CENTRAL_COMPRESSED_SIZE)?;
        let size = dword(bytes, at + CENTRAL_EXPANDED_SIZE)?;
        expanded = expanded
            .checked_add(size)
            .ok_or(TemplateError::Unsupported)?;
        if expanded > MAX_EXPANDED_BYTES {
            return Err(TemplateError::Unsupported);
        }
        let name_len = word(bytes, at + CENTRAL_NAME_LENGTH)?;
        let extra = word(bytes, at + CENTRAL_EXTRA_LENGTH)?;
        let comment = word(bytes, at + CENTRAL_COMMENT_LENGTH)?;
        let local = dword(bytes, at + CENTRAL_LOCAL_START)?;
        if local >= central_start || central_start - local < LOCAL_BYTES {
            return Err(TemplateError::Malformed);
        }
        let raw_name = bytes
            .get(at + CENTRAL_BYTES..at + CENTRAL_BYTES + name_len)
            .ok_or(TemplateError::Malformed)?;
        let part_name = std::str::from_utf8(raw_name).map_err(|_| TemplateError::Unsupported)?;
        name(part_name)?;
        if !names.insert(part_name.to_ascii_lowercase()) {
            return Err(TemplateError::Malformed);
        }
        // OPC does not use ZIP64 in this bounded profile. Reject its extra field
        // before the dependency can replace classic size/offset metadata with it.
        let extra_start = at + CENTRAL_BYTES + name_len;
        let extra_end = extra_start + extra;
        let mut field = extra_start;
        while field < extra_end {
            check(budget, cancelled)?;
            if word(bytes, field)? == ZIP64_EXTRA_TAG {
                return Err(TemplateError::Unsupported);
            }
            field += EXTRA_HEADER_BYTES + word(bytes, field + size_of::<u16>())?;
            if field > extra_end {
                return Err(TemplateError::Malformed);
            }
        }
        let local_name = local
            .checked_add(LOCAL_BYTES)
            .ok_or(TemplateError::Malformed)?;
        let data = local_name
            + word(bytes, local + LOCAL_NAME_LENGTH)?
            + word(bytes, local + LOCAL_EXTRA_LENGTH)?;
        if bytes.get(local..local + LOCAL_SIGNATURE.len()) != Some(LOCAL_SIGNATURE)
            || flags != word(bytes, local + LOCAL_FLAGS)?
            || method != word(bytes, local + LOCAL_METHOD)?
            || name_len != word(bytes, local + LOCAL_NAME_LENGTH)?
            || bytes.get(local_name..local_name + name_len) != Some(raw_name)
            || data
                .checked_add(packed)
                .is_none_or(|end| end > central_start)
        {
            return Err(TemplateError::Malformed);
        }
        if flags & DATA_DESCRIPTOR_FLAG == 0
            && (dword(bytes, local + LOCAL_CRC)? != dword(bytes, at + CENTRAL_CRC)?
                || dword(bytes, local + LOCAL_COMPRESSED_SIZE)? != packed
                || dword(bytes, local + LOCAL_EXPANDED_SIZE)? != size)
        {
            return Err(TemplateError::Malformed);
        }
        at = extra_end + comment;
        if at > end {
            return Err(TemplateError::Malformed);
        }
    }
    if at != end {
        return Err(TemplateError::Malformed);
    }
    Ok(count)
}
