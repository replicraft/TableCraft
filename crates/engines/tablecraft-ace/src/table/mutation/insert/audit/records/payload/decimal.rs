//! Independent base-2^32 NUMERIC reconstruction from the explicit coefficient.
use crate::{EditError, InspectError, NativeColumn, WriteBlocker, header::check};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const LIMB_BYTES: usize = size_of::<u32>();
const LIMBS: usize = size_of::<u128>() / LIMB_BYTES;
const SIGN_BYTES: usize = size_of::<u8>();
const WIRE_BYTES: usize = SIGN_BYTES + LIMBS * LIMB_BYTES;
const LIMB_RADIX: u128 = 1_u128 << u32::BITS;
const NEGATIVE: u8 = 0x80;

pub(super) fn encode(
    field: &NativeColumn,
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    check(budget, cancelled)?;
    let value = match value {
        Value::Null => return Ok(Vec::new()),
        Value::Decimal(value) => value,
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    };
    let (precision, scale) = field
        .decimal_parameters()
        .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
    let mut magnitude = value.coefficient().unsigned_abs();
    if value.scale() != scale || magnitude >= 10_u128.pow(u32::from(precision)) {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }

    budget
        .charge_allocation(WIRE_BYTES as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, WIRE_BYTES as u64)
        .map_err(InspectError::from)?;
    let mut wire = Vec::new();
    wire.try_reserve_exact(WIRE_BYTES)
        .map_err(|_| InspectError::Allocation)?;
    wire.resize(WIRE_BYTES, 0);
    if value.coefficient() < 0 {
        wire[0] = NEGATIVE;
    }
    for limb in (0..LIMBS).rev() {
        check(budget, cancelled)?;
        let value = u32::try_from(magnitude % LIMB_RADIX).map_err(|_| EditError::Preservation)?;
        magnitude /= LIMB_RADIX;
        let start = SIGN_BYTES + limb * LIMB_BYTES;
        wire[start..start + LIMB_BYTES].copy_from_slice(&value.to_le_bytes());
    }
    if magnitude != 0 {
        return Err(EditError::Preservation);
    }
    Ok(wire)
}
