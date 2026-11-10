//! Immutable capture grows from actual reads after format-owned envelope checks.
#[cfg(test)]
mod tests;
use crate::{Error, controls::check};
use std::io::{Read, Seek, SeekFrom};
use tablecraft_access::inspect_input_envelope;
use tablecraft_core::limits::ResourceBudget;

const COPY_CHUNK: usize = 64 * 1024;

pub(super) fn read<R: Read + Seek>(
    source: &mut R,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    let length = inspect_input_envelope(source, work, cancelled)?;
    let size = usize::try_from(length).map_err(|_| Error::SnapshotLimit)?;
    source
        .seek(SeekFrom::Start(0))
        .map_err(tablecraft_ace::InspectError::from)?;
    let mut bytes = Vec::new();
    while bytes.len() < size {
        check(cancelled)?;
        let count = COPY_CHUNK.min(size - bytes.len());
        let needed = bytes.len().checked_add(count).ok_or(Error::SnapshotLimit)?;
        if needed > bytes.capacity() {
            let target = if bytes.capacity() == 0 {
                COPY_CHUNK.min(size)
            } else {
                bytes
                    .capacity()
                    .checked_mul(2)
                    .ok_or(Error::SnapshotLimit)?
                    .min(size)
                    .max(needed)
            };
            work.charge_allocation((target - bytes.capacity()) as u64, 1)?;
            bytes.try_reserve_exact(target - bytes.len())?;
        }
        let start = bytes.len();
        bytes.resize(needed, 0);
        tablecraft_io::read_exact(source, &mut bytes[start..needed], work, cancelled)?;
    }
    check(cancelled)?;
    if source
        .seek(SeekFrom::End(0))
        .map_err(tablecraft_ace::InspectError::from)?
        != length
    {
        return Err(Error::SourceChanged);
    }
    Ok(bytes)
}
