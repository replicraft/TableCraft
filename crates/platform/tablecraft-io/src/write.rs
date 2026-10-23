use crate::IoError;
use std::io::{ErrorKind, Seek, SeekFrom, Write};
use tablecraft_core::limits::{Resource, ResourceBudget};

/// Seek and write every supplied byte. Caller charges bytes before invoking;
/// each seek/write attempt and completion checks cancellation and charges work.
/// Short writes continue, Interrupted retries remain bounded, and zero progress
/// fails with WriteZero. Failure can leave partial changes: this is not atomic
/// and does not flush or sync. A reported seek-position mismatch fails before any
/// write. Use a qualified journal before mutating a database.
pub fn write_exact_at<W: Write + Seek + ?Sized>(
    target: &mut W,
    offset: u64,
    mut bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), IoError> {
    check(budget, cancelled)?;
    if target.seek(SeekFrom::Start(offset))? != offset {
        return Err(IoError::InvalidSeekPosition);
    }
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
fn check(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), IoError> {
    if cancelled() {
        return Err(IoError::Cancelled);
    }
    budget.charge(Resource::Steps, 1)?;
    Ok(())
}

#[cfg(test)]
mod tests;
