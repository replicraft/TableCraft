//! Immutable capture grows from actual reads after format-owned envelope checks.
#[cfg(test)]
mod tests;
use crate::{Error, controls::check};
use std::io::{Cursor, Read, Seek};
use tablecraft_access::inspect_input_envelope;
use tablecraft_core::limits::{Resource, ResourceBudget};

const COPY_CHUNK: usize = 64 * 1024;

pub(super) fn read<R: Read + Seek>(
    source: &mut R,
    expected: Option<&[u8]>,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    let length = if let Some(expected) = expected {
        // Guarded reopen selects its private witness before inspecting content.
        // Changed actual bytes must not select a different format or decoder.
        let length = inspect_input_envelope(&mut Cursor::new(expected), work, cancelled)?;
        if tablecraft_io::seek_to_end(source, work, cancelled)? != length {
            return Err(Error::SourceChanged);
        }
        length
    } else {
        inspect_input_envelope(source, work, cancelled)?
    };
    let size = usize::try_from(length).map_err(|_| Error::SnapshotLimit)?;
    tablecraft_io::seek_absolute(source, 0, work, cancelled)?;
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
        if let Some(expected) = expected {
            check(cancelled)?;
            work.charge(Resource::Steps, count as u64)?;
            if expected.get(start..needed) != Some(&bytes[start..needed]) {
                return Err(Error::SourceChanged);
            }
        }
    }
    check(cancelled)?;
    if tablecraft_io::seek_to_end(source, work, cancelled)? != length {
        return Err(Error::SourceChanged);
    }
    Ok(bytes)
}
