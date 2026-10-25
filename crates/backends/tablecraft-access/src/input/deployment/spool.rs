//! Explicit complete extraction to a host-owned fresh seekable spool.
use super::{
    AccessInputError, COPY_CHUNK_BYTES, DECOMPRESSOR_WORK_BYTES,
    bounded::{Context, Source},
    check, layout,
};
use std::io::{self, Read, Seek, Write};
use tablecraft_ace::{FileFormat, Header, InspectError};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::input) fn read<R, S, F>(
    source: &mut R,
    make_spool: F,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<S, AccessInputError>
where
    R: Read + Seek,
    S: Read + Write + Seek,
    F: FnOnce() -> Result<S, InspectError>,
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
) -> Result<S, AccessInputError>
where
    R: Read + Seek,
    S: Read + Write + Seek,
    F: FnOnce() -> Result<S, InspectError>,
{
    budget
        .charge(
            Resource::Bytes,
            layout.allocation_bytes
                + DECOMPRESSOR_WORK_BYTES
                + COPY_CHUNK_BYTES as u64
                + Header::PREFIX_BYTES as u64,
        )
        .map_err(InspectError::from)?;
    let mut chunk = Vec::new();
    chunk
        .try_reserve_exact(COPY_CHUNK_BYTES)
        .map_err(|_| InspectError::Allocation)?;
    chunk.resize(COPY_CHUNK_BYTES, 0);

    check(budget, cancelled)?;
    let mut spool = make_spool()?;
    check(budget, cancelled)?;
    if tablecraft_io::seek_to_end(&mut spool, budget, cancelled).map_err(InspectError::from)? != 0 {
        return Err(AccessInputError::UnsupportedPackage);
    }
    tablecraft_io::seek_absolute(&mut spool, 0, budget, cancelled).map_err(InspectError::from)?;
    tablecraft_io::seek_absolute(source, 0, budget, cancelled).map_err(InspectError::from)?;

    let context = Context::new(budget, cancelled);
    let result = extract(source, &mut spool, &layout, &context, &mut chunk);
    let header = context.result(result)?;
    if !matches!(
        header.format(),
        FileFormat::Ace2007 | FileFormat::Ace2010 | FileFormat::Ace2016 | FileFormat::Ace2019
    ) {
        return Err(AccessInputError::UnsupportedPackage);
    }
    Ok(spool)
}

fn extract<R: Read + Seek, S: Read + Write + Seek>(
    source: &mut R,
    spool: &mut S,
    layout: &layout::Layout,
    context: &Context<'_>,
    chunk: &mut [u8],
) -> io::Result<Header> {
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
        .map_err(|_| context.fail(InspectError::Allocation))?;
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
        return Err(context.fail(InspectError::Malformed));
    }
    context.seek(spool, 0)?;
    let mut prefix = [0; Header::PREFIX_BYTES];
    context.charge(Resource::Bytes, prefix.len() as u64)?;
    // Spool read failures are host failures, not malformed compression errors.
    context.read_host_exact(spool, &mut prefix)?;
    let header =
        Header::parse(&prefix, layout.database_bytes).map_err(|error| context.fail(error))?;
    context.seek(spool, 0)?;
    context.check()?;
    Ok(header)
}
