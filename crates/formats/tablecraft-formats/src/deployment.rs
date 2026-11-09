//! CAB geometry gates ahead of the library's parser/decompressor allocations.
mod bounded;
mod hybrid;
mod layout;
pub use hybrid::{Selected, read as read_with_spool, read_validated as read_with_spool_validated};
mod spool;
use crate::ReadError;
pub use spool::read as read_spooled;
use std::io::{Cursor, Read, Seek, SeekFrom};
use tablecraft_core::limits::{Resource, ResourceBudget};
use thiserror::Error;

/// Minimum retained prefix, preserving the qualified deployment envelope.
const PAYLOAD_PREFIX_BYTES: usize = 66;

/// Failures from bounded CAB decoding; native payload qualification belongs to the caller.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DeploymentError {
    /// I/O, allocation, cancellation or shared resource failure.
    #[error(transparent)]
    Read(#[from] ReadError),
    /// Inconsistent CAB framing or compressed payload.
    #[error("malformed Access deployment package")]
    MalformedPackage,
    /// Unsupported CAB geometry or compression.
    #[error("Access deployment package layout or compression is not supported")]
    UnsupportedPackage,
}

/// Completed host storage with an opaque prefix for caller-owned payload qualification.
/// No native database header is interpreted by the container decoder.
pub struct SpooledPayload<S> {
    storage: S,
    prefix: [u8; PAYLOAD_PREFIX_BYTES],
    length: u64,
}

impl<S> SpooledPayload<S> {
    /// Prefix captured under the same decoder budget before publication.
    pub fn prefix(&self) -> &[u8] {
        &self.prefix
    }

    /// Exact decoded payload length, checked against the host storage.
    pub fn length(&self) -> u64 {
        self.length
    }

    /// Return the complete caller-owned storage at its current position.
    pub fn into_inner(self) -> S {
        self.storage
    }
}

impl<S: Seek> SpooledPayload<S> {
    /// Rewind after caller-owned prefix validation, under the same work allowance.
    /// Capture leaves storage immediately after the prefix so validation errors
    /// retain precedence over a later host seek, cancellation or budget failure.
    pub fn rewind(
        &mut self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), DeploymentError> {
        let context = bounded::Context::new(budget, cancelled);
        let result = context
            .seek(&mut self.storage, 0)
            .and_then(|()| context.check());
        context.result(result)
    }
}

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

/// Capture the qualified single CAB member without interpreting its native payload.
pub fn read<R: Read + Seek>(
    source: &mut R,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, DeploymentError> {
    read_validated(source, budget, cancelled, |_| Ok(()))
}

/// Validate a captured payload before its terminal work/cancellation check.
/// The callback owns payload interpretation; CAB decoding stays format-neutral.
pub fn read_validated<R, E>(
    source: &mut R,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
    validate: impl FnOnce(&[u8]) -> Result<(), E>,
) -> Result<Vec<u8>, E>
where
    R: Read + Seek,
    E: From<DeploymentError>,
{
    let bytes = capture(source, budget, cancelled)?;
    validate(&bytes)?;
    check(budget, cancelled)?;
    Ok(bytes)
}

fn capture<R: Read + Seek>(
    source: &mut R,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, DeploymentError> {
    let _span = tracing::debug_span!("access.deployment.read").entered();
    check(budget, cancelled)?;
    let length = source.seek(SeekFrom::End(0)).map_err(ReadError::from)?;
    if !(HEADER_BYTES as u64..=MAX_PACKAGE_BYTES).contains(&length) {
        return Err(ReadError::StructureLimit.into());
    }
    let mut bytes = allocate(length as usize, budget)?;
    tablecraft_io::read_exact_at(source, 0, &mut bytes, budget, cancelled)
        .map_err(ReadError::from)?;
    let size = preflight(&bytes, budget, cancelled)?;
    // Preflight validated this logical end. Retain the physical capture charge,
    // but never present an unverified signature overlay to the CAB decoder.
    let cabinet_bytes = dword(&bytes, SIZE_OFFSET)?;
    bytes.truncate(cabinet_bytes);
    budget
        .charge(Resource::Bytes, DECOMPRESSOR_WORK_BYTES)
        .map_err(ReadError::from)?;
    let mut cabinet =
        cab::Cabinet::new(Cursor::new(bytes)).map_err(|_| DeploymentError::MalformedPackage)?;
    // Preflight requires exactly one file. Its original name is data only.
    let name = cabinet
        .folder_entries()
        .next()
        .and_then(|folder| folder.file_entries().next())
        .ok_or(DeploymentError::MalformedPackage)?
        .name()
        .to_owned();
    let mut data = allocate(size, budget)?;
    let mut reader = cabinet
        .read_file(&name)
        .map_err(|_| DeploymentError::MalformedPackage)?;
    for chunk in data.chunks_mut(COPY_CHUNK_BYTES) {
        check(budget, cancelled)?;
        tablecraft_io::read_exact(&mut reader, chunk, budget, cancelled).map_err(|error| {
            match error {
                tablecraft_io::IoError::Io(_) => DeploymentError::MalformedPackage,
                error => DeploymentError::Read(error.into()),
            }
        })?;
    }
    Ok(data)
}

fn allocate(size: usize, budget: &mut ResourceBudget) -> Result<Vec<u8>, DeploymentError> {
    budget
        .charge(Resource::Bytes, size as u64)
        .map_err(ReadError::from)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(|_| ReadError::Allocation)?;
    bytes.resize(size, 0);
    Ok(bytes)
}
fn check(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), DeploymentError> {
    if cancelled() {
        return Err(ReadError::Cancelled.into());
    }
    budget.charge(Resource::Steps, 1).map_err(ReadError::from)?;
    Ok(())
}
fn word(bytes: &[u8], at: usize) -> Result<u16, DeploymentError> {
    Ok(u16::from_le_bytes(
        bytes
            .get(at..at + size_of::<u16>())
            .ok_or(DeploymentError::MalformedPackage)?
            .try_into()
            .unwrap(),
    ))
}
fn dword(bytes: &[u8], at: usize) -> Result<usize, DeploymentError> {
    Ok(u32::from_le_bytes(
        bytes
            .get(at..at + size_of::<u32>())
            .ok_or(DeploymentError::MalformedPackage)?
            .try_into()
            .unwrap(),
    ) as usize)
}

fn preflight(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, DeploymentError> {
    check(budget, cancelled)?;
    let malformed = DeploymentError::MalformedPackage;
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
        return Err(DeploymentError::UnsupportedPackage);
    }
    let mut folder = HEADER_BYTES;
    let (folder_reserve, data_reserve) = if word(bytes, FLAGS_OFFSET)? & RESERVE_PRESENT != 0 {
        let header_reserve = usize::from(word(bytes, HEADER_BYTES)?);
        let sizes = bytes
            .get(HEADER_BYTES..HEADER_BYTES + RESERVE_SIZES_BYTES)
            .ok_or(DeploymentError::MalformedPackage)?;
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
        _ => return Err(DeploymentError::UnsupportedPackage),
    }
    let file = dword(bytes, FILE_TABLE_OFFSET)?;
    if file < folder + FOLDER_BYTES + folder_reserve {
        return Err(DeploymentError::MalformedPackage);
    }
    let size = dword(bytes, file)?;
    if !(PAYLOAD_PREFIX_BYTES..=MAX_DATABASE_BYTES).contains(&size) {
        return Err(ReadError::StructureLimit.into());
    }
    // Offset zero prevents hidden decompression work before the selected file;
    // folder zero rejects split cabinet file markers and ambiguous selection.
    if dword(bytes, file + 4)? != 0 || word(bytes, file + 8)? != 0 || blocks == 0 {
        return Err(DeploymentError::UnsupportedPackage);
    }
    let name_start = file + FILE_BYTES;
    let name = bytes
        .get(name_start..)
        .ok_or(DeploymentError::MalformedPackage)?;
    let name_length = name
        .iter()
        .take(MAX_NAME_BYTES + 1)
        .position(|b| *b == 0)
        .ok_or(DeploymentError::MalformedPackage)?;
    let name = &name[..name_length];
    if name.is_empty()
        || name
            .iter()
            .any(|b| *b < b' ' || matches!(b, b':' | b'/' | b'\\'))
        || name == b"."
        || name == b".."
    {
        return Err(DeploymentError::UnsupportedPackage);
    }
    if block < name_start + name.len() + 1 {
        return Err(DeploymentError::MalformedPackage);
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
            return Err(DeploymentError::MalformedPackage);
        }
        block += DATA_HEADER_BYTES + data_reserve + compressed;
        expanded += plain;
        if block > bytes.len() || expanded > size {
            return Err(DeploymentError::MalformedPackage);
        }
    }
    if expanded != size {
        return Err(DeploymentError::MalformedPackage);
    }
    Ok(size)
}
