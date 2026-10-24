//! Retry only idempotent seek targets within one cumulative work allowance.
use crate::{IoError, progress::check};
use std::io::{ErrorKind, Seek, SeekFrom};
use tablecraft_core::limits::ResourceBudget;

/// Seek to an absolute byte offset, checking the reported position.
///
/// Interrupted attempts retry the same absolute target, even if a failed attempt
/// moved the cursor. Each attempt and successful completion checks cancellation
/// and charges work. Other errors and incorrect reported positions fail directly.
/// The host owns callback timeouts; this does not provide a positional-I/O,
/// coherent-file or atomic-write guarantee.
pub fn seek_absolute<S: Seek + ?Sized>(
    source: &mut S,
    offset: u64,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), IoError> {
    let _span = tracing::trace_span!("io.seek.absolute").entered();
    position(
        source,
        SeekFrom::Start(offset),
        Some(offset),
        budget,
        cancelled,
    )?;
    Ok(())
}

/// Seek to the current end and return its reported byte position.
///
/// Interrupted attempts repeat End(0) under the same work/cancellation controls.
/// This changes the cursor and does not restore its former position. The caller
/// owns source stability: a live file can change length between observations.
pub fn seek_to_end<S: Seek + ?Sized>(
    source: &mut S,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<u64, IoError> {
    let _span = tracing::trace_span!("io.seek.end").entered();
    position(source, SeekFrom::End(0), None, budget, cancelled)
}

fn position<S: Seek + ?Sized>(
    source: &mut S,
    target: SeekFrom,
    expected: Option<u64>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<u64, IoError> {
    loop {
        check(budget, cancelled)?;
        match source.seek(target) {
            Ok(actual) => {
                if expected.is_some_and(|expected| actual != expected) {
                    return Err(IoError::InvalidSeekPosition);
                }
                check(budget, cancelled)?;
                return Ok(actual);
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) => return Err(error.into()),
        }
    }
}

#[cfg(test)]
mod tests;
