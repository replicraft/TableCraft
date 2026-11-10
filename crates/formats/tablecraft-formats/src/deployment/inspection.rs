//! Inert physical CAB geometry preflight before a host captures its outer bytes.
use super::{DeploymentError, layout};
use std::io::{Read, Seek};
use tablecraft_core::limits::ResourceBudget;

/// Validate the existing supported CAB envelope without decompressing payloads.
/// Returns the actual physical source length, including its qualified overlay.
/// Native payload validity and content authentication are not established here.
pub fn inspect_envelope<R: Read + Seek>(
    source: &mut R,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<u64, DeploymentError> {
    let _span = tracing::debug_span!("deployment.envelope.inspect").entered();
    Ok(layout::read(source, budget, cancelled)?.physical_bytes)
}
