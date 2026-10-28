//! Explicit complete extraction to a host-owned fresh seekable spool.
use super::{
    COPY_CHUNK_BYTES, DECOMPRESSOR_WORK_BYTES, DeploymentError, PAYLOAD_PREFIX_BYTES,
    SpooledPayload,
    bounded::{Context, Source},
    check, layout,
};
use crate::ReadError;
use std::io::{self, Read, Seek, Write};
use tablecraft_core::limits::{Resource, ResourceBudget};

/// Completely decode a CAB member into fresh caller-owned storage.
/// Validate its captured opaque prefix before explicitly rewinding for publication.
pub fn read<R, S, F>(
    source: &mut R,
    make_spool: F,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<SpooledPayload<S>, DeploymentError>
where
    R: Read + Seek,
    S: Read + Write + Seek,
    F: FnOnce() -> Result<S, ReadError>,
{
    let _span = tracing::debug_span!("access.deployment.spool").entered();
    let layout = layout::read(source, budget, cancelled)?;
    read_layout(source, make_spool, layout, budget, cancelled)
}

pub(super) fn read_layout<R, S, F>(
    source: &mut R,
    make_spool: F,
    layout: layout::Layout,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<SpooledPayload<S>, DeploymentError>
where
    R: Read + Seek,
    S: Read + Write + Seek,
    F: FnOnce() -> Result<S, ReadError>,
{
    budget
        .charge(
            Resource::Bytes,
            layout.allocation_bytes
                + DECOMPRESSOR_WORK_BYTES
                + COPY_CHUNK_BYTES as u64
                + PAYLOAD_PREFIX_BYTES as u64,
        )
        .map_err(ReadError::from)?;
    let mut chunk = Vec::new();
    chunk
        .try_reserve_exact(COPY_CHUNK_BYTES)
        .map_err(|_| ReadError::Allocation)?;
    chunk.resize(COPY_CHUNK_BYTES, 0);

    check(budget, cancelled)?;
    let mut spool = make_spool()?;
    check(budget, cancelled)?;
    if tablecraft_io::seek_to_end(&mut spool, budget, cancelled).map_err(ReadError::from)? != 0 {
        return Err(DeploymentError::UnsupportedPackage);
    }
    tablecraft_io::seek_absolute(&mut spool, 0, budget, cancelled).map_err(ReadError::from)?;
    tablecraft_io::seek_absolute(source, 0, budget, cancelled).map_err(ReadError::from)?;

    let context = Context::new(budget, cancelled);
    let result = extract(source, &mut spool, &layout, &context, &mut chunk);
    let prefix = context.result(result)?;
    Ok(SpooledPayload {
        storage: spool,
        prefix,
        length: layout.database_bytes,
    })
}

fn extract<R: Read + Seek, S: Read + Write + Seek>(
    source: &mut R,
    spool: &mut S,
    layout: &layout::Layout,
    context: &Context<'_>,
    chunk: &mut [u8],
) -> io::Result<[u8; PAYLOAD_PREFIX_BYTES]> {
    let input = Source {
        source,
        context,
        end: layout.cabinet_bytes,
        position: 0,
    };
    let mut cabinet = cab::Cabinet::new(input)?;
    let stored_name = cabinet
        .folder_entries()
        .next()
        .and_then(|folder| folder.file_entries().next())
        .ok_or(io::ErrorKind::InvalidData)?
        .name();
    let mut name = String::new();
    name.try_reserve_exact(stored_name.len())
        .map_err(|_| context.fail(ReadError::Allocation))?;
    name.push_str(stored_name);
    let mut reader = cabinet.read_file(&name)?;
    let mut remaining = layout.database_bytes;
    while remaining != 0 {
        context.check()?;
        let length = remaining.min(chunk.len() as u64) as usize;
        context.charge(Resource::Bytes, length as u64)?;
        context.read_exact(&mut reader, &mut chunk[..length])?;
        context.write_exact(spool, &chunk[..length])?;
        remaining -= length as u64;
    }
    context.check()?;
    if reader.read(&mut [0])? != 0 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    drop(reader);
    drop(cabinet);

    context.flush(spool)?;
    if context.end(spool)? != layout.database_bytes {
        return Err(context.fail(ReadError::Malformed));
    }
    context.seek(spool, 0)?;
    let mut prefix = [0; PAYLOAD_PREFIX_BYTES];
    context.charge(Resource::Bytes, prefix.len() as u64)?;
    context.read_host_exact(spool, &mut prefix)?;
    Ok(prefix)
}
