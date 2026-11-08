//! Paid immutable expected bytes for read-only guarded schema/settings reopening.
use crate::{Error, controls::check};
use bytes::Bytes;
use tablecraft_ace::NativeImageSource;
use tablecraft_core::limits::{Resource, ResourceBudget};

const COPY_CHUNK_BYTES: usize = 16 * 1024;

pub(super) fn capture(
    bytes: &[u8],
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Bytes, Error> {
    work.charge_allocation(bytes.len() as u64, 1)?;
    let mut expected = Vec::new();
    expected.try_reserve_exact(bytes.len())?;
    for chunk in bytes.chunks(COPY_CHUNK_BYTES) {
        check(cancelled)?;
        work.charge(Resource::Steps, chunk.len() as u64)?;
        expected.extend_from_slice(chunk);
    }
    let expected = NativeImageSource::from_vec(expected, work, cancelled)?.shared_bytes();
    Ok(expected)
}
