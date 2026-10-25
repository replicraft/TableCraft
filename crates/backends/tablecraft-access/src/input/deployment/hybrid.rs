//! Preserve qualified small-image capture, selecting host storage only above its ceilings.
use super::{
    AccessInputError, MAX_DATABASE_BYTES, MAX_PACKAGE_BYTES, layout, read as capture, spool,
};
use std::io::{Read, Seek, Write};
use tablecraft_ace::InspectError;
use tablecraft_core::limits::ResourceBudget;

pub(in crate::input) enum Selected<S> {
    Captured(Vec<u8>),
    Spooled(S),
}

pub(in crate::input) fn read<R, S, F>(
    source: &mut R,
    make_spool: F,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Selected<S>, AccessInputError>
where
    R: Read + Seek,
    S: Read + Write + Seek,
    F: FnOnce() -> Result<S, InspectError>,
{
    let layout = layout::read(source, budget, cancelled)?;
    if layout.physical_bytes <= MAX_PACKAGE_BYTES
        && layout.database_bytes <= MAX_DATABASE_BYTES as u64
    {
        return capture(source, budget, cancelled).map(Selected::Captured);
    }
    spool::read_layout(source, make_spool, layout, budget, cancelled).map(Selected::Spooled)
}
