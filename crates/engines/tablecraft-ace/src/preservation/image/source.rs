//! Whole-allocation ownership for immutable native snapshots.
use crate::{InspectError, header::check};
use bytes::Bytes;
use tablecraft_core::limits::ResourceBudget;

// Conservative library-control allowance, independent of bytes' private layout.
// The payload's original capacity is accounted separately, including unused space.
pub(super) const SHARING_CONTROL_BYTES: u64 = (size_of::<Bytes>() + size_of::<Vec<u8>>()) as u64;

/// An immutable whole allocation transferred from a caller-owned vector.
///
/// This intentionally has no constructor accepting `Bytes` or a borrowed slice:
/// either could conceal a much larger retained allocation. Clones share this exact
/// allocation; native capture still admits its complete retained capacity.
/// No database validation or mutation authority is established by this owner.
#[derive(Clone)]
pub struct NativeImageSource {
    bytes: Bytes,
    capacity: usize,
}

impl NativeImageSource {
    /// Transfer an existing vector without copying its payload. The caller owns
    /// the vector's earlier allocation/read accounting. This operation pays for
    /// shared control storage before creating it and checks cancellation first.
    pub fn from_vec(
        bytes: Vec<u8>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        check(budget, cancelled)?;
        budget.charge_allocation(SHARING_CONTROL_BYTES, 1)?;
        let capacity = bytes.capacity();
        let bytes = Bytes::from(bytes);
        // Exact-size vectors can defer their control allocation until first clone.
        // Materialize it while the explicit control allowance is in scope.
        drop(bytes.clone());

        Ok(Self { bytes, capacity })
    }

    /// Exact immutable payload, without exposing a mutable or sliced owner.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Complete original payload capacity retained by this owner and its clones.
    pub fn retained_capacity(&self) -> usize {
        self.capacity
    }

    /// Share bytes with a read cursor. Derived/sliced `Bytes` cannot be admitted
    /// back as a whole native owner; only this original witness can be captured.
    pub fn shared_bytes(&self) -> Bytes {
        self.bytes.clone()
    }

    pub(super) fn into_bytes(self) -> Bytes {
        self.bytes
    }
}
