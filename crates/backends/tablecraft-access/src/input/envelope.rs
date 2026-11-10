//! Format-owned bounds precede complete immutable host allocation.
use super::AccessInputError;
use std::io::{Read, Seek};
use tablecraft_ace::{InspectError, inspect_header};
use tablecraft_core::limits::ResourceBudget;

/// Inspect native or deployment geometry and return its actual physical length.
/// Raw native files use the engine's header/page/2-GiB envelope. CAB inputs use
/// their container's existing geometry. No payloads or active content execute.
pub fn inspect_input_envelope<R: Read + Seek>(
    source: &mut R,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<u64, AccessInputError> {
    let _span = tracing::debug_span!("access.input.envelope.inspect").entered();
    let mut magic = [0; size_of::<u32>()];
    tablecraft_io::read_exact_at(source, 0, &mut magic, budget, cancelled)
        .map_err(InspectError::from)?;
    if magic == *b"MSCF" {
        Ok(tablecraft_formats::deployment::inspect_envelope(
            source, budget, cancelled,
        )?)
    } else {
        let header = inspect_header(source, budget, cancelled)?;
        Ok(u64::from(header.page_count()) * u64::from(header.page_size()))
    }
}
