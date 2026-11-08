//! Exact empty DATA and primary-root pages, without production serializers.
use super::super::{AddTableError, geometry::Role, source};
use crate::{InspectError, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const PAGE_BYTES: usize = 4096;
const DATA_HEADER_BYTES: u16 = 14;
const INDEX_PAYLOAD_BYTES: u16 = 3616;

pub(super) fn verify(
    after: &[u8],
    role: Role,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), AddTableError> {
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, (4 * PAGE_BYTES) as u64)
        .map_err(InspectError::from)?;
    let mut expected = [0; PAGE_BYTES];
    expected[..2].copy_from_slice(&[1, 1]);
    expected[2..4].copy_from_slice(&(PAGE_BYTES as u16 - DATA_HEADER_BYTES).to_le_bytes());
    expected[4..8].copy_from_slice(&(role.definition as u32).to_le_bytes());
    if source::page(after, role.data) != expected {
        return Err(AddTableError::Preservation);
    }
    expected.fill(0);
    expected[..2].copy_from_slice(&[4, 1]);
    expected[2..4].copy_from_slice(&INDEX_PAYLOAD_BYTES.to_le_bytes());
    expected[4..8].copy_from_slice(&(role.definition as u32).to_le_bytes());
    if source::page(after, role.root) != expected {
        return Err(AddTableError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}
