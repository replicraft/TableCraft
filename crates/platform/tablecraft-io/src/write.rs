use crate::{IoError, progress::check, seek_absolute};
use std::io::{ErrorKind, Seek, Write};
use tablecraft_core::limits::ResourceBudget;

/// Seek and write every supplied byte. Caller charges bytes before invoking;
/// each seek/write attempt and successful completion checks cancellation and
/// charges work. Short writes continue, Interrupted absolute-seek/write retries
/// remain bounded, and zero progress fails with WriteZero. Failure can leave partial changes: this is not atomic
/// and does not flush or sync. A reported seek-position mismatch fails before any
/// write. Use a qualified journal before mutating a database.
pub fn write_exact_at<W: Write + Seek + ?Sized>(
    target: &mut W,
    offset: u64,
    mut bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), IoError> {
    seek_absolute(target, offset, budget, cancelled)?;
    while !bytes.is_empty() {
        check(budget, cancelled)?;
        match target.write(bytes) {
            Ok(0) => return Err(IoError::Io(ErrorKind::WriteZero)),
            Ok(count) => bytes = bytes.get(count..).ok_or(IoError::InvalidWriteCount)?,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        }
    }
    check(budget, cancelled)
}

#[cfg(test)]
mod tests;
