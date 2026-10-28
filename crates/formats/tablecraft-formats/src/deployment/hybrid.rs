//! Preserve qualified small-image capture, selecting host storage only above its ceilings.
use super::{
    DeploymentError, MAX_DATABASE_BYTES, MAX_PACKAGE_BYTES, SpooledPayload, layout,
    read_validated as capture, spool,
};
use crate::ReadError;
use std::io::{Read, Seek, Write};
use tablecraft_core::limits::ResourceBudget;

/// Decoded payload storage selected under the existing capture ceilings.
pub enum Selected<S> {
    /// Complete in-memory payload.
    Captured(Vec<u8>),
    /// Complete caller-owned spool with an opaque prefix, awaiting explicit rewind.
    Spooled(SpooledPayload<S>),
}

/// Capture small payloads and lazily request host storage for larger qualified CABs.
pub fn read<R, S, F>(
    source: &mut R,
    make_spool: F,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Selected<S>, DeploymentError>
where
    R: Read + Seek,
    S: Read + Write + Seek,
    F: FnOnce() -> Result<S, ReadError>,
{
    read_validated(source, make_spool, budget, cancelled, |_| Ok(()))
}

/// Select bounded capture or a spool, validating a capture before its final check.
/// Spooled prefix validation and rewind remain explicit caller-owned operations.
pub fn read_validated<R, S, F, E>(
    source: &mut R,
    make_spool: F,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
    validate_capture: impl FnOnce(&[u8]) -> Result<(), E>,
) -> Result<Selected<S>, E>
where
    R: Read + Seek,
    S: Read + Write + Seek,
    F: FnOnce() -> Result<S, ReadError>,
    E: From<DeploymentError>,
{
    let layout = layout::read(source, budget, cancelled)?;
    if layout.physical_bytes <= MAX_PACKAGE_BYTES
        && layout.database_bytes <= MAX_DATABASE_BYTES as u64
    {
        return capture(source, budget, cancelled, validate_capture).map(Selected::Captured);
    }
    Ok(Selected::Spooled(spool::read_layout(
        source, make_spool, layout, budget, cancelled,
    )?))
}
