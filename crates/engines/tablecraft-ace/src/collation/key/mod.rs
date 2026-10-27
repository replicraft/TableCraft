//! General and exact French Legacy physical keys, using reviewed static weights.
//! These physical encodings do not supply SQL equality.
mod buffer;
mod encode;
mod french;
mod tail;
#[cfg(test)]
mod tests;
mod weights;
use crate::{
    InspectError, NativeSortOrder, format::data_type::MAX_SHORT_TEXT_BYTES, header::check,
};
pub(super) use buffer::Work;
pub(super) use encode::{ComparisonKey, encode_full, primary_byte_bound};
use tablecraft_core::limits::ResourceBudget;

const MAX_UNITS: usize = MAX_SHORT_TEXT_BYTES / size_of::<u16>();
const SPACE_UNIT: u16 = b' ' as u16;

/// Encoded non-NULL native text payload, without the column's value marker or
/// row/child reference. Does not expose a SQL comparison or text serialization.
pub struct NativeTextKey {
    bytes: Vec<u8>,
}
impl NativeTextKey {
    /// Native index bytes. Descending keys include the native inversion suffix.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Encode a General or exact French Legacy (1036/0/0) physical index payload under
/// explicit work limits. Indexing retains at most 255 UTF-16 units, then removes
/// trailing ASCII spaces. This truncation cannot compare arbitrary SQL strings.
pub fn encode_text_index_key(
    order: NativeSortOrder,
    text: &str,
    descending: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeTextKey, InspectError> {
    check(budget, cancelled)?;
    let mut units = [0; MAX_UNITS];
    let mut length = 0;
    for unit in text.encode_utf16().take(MAX_UNITS) {
        check(budget, cancelled)?;
        units[length] = unit;
        length += 1;
    }
    encode_utf16_text_index_key(order, &units[..length], descending, budget, cancelled)
}

/// Encode raw native UTF-16 units, including individual surrogate units, without
/// replacement-character coercion. Index-prefix truncation can itself separate a
/// surrogate pair. This does not make ill-formed UTF-16 a core Text value.
pub fn encode_utf16_text_index_key(
    order: NativeSortOrder,
    units: &[u16],
    descending: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeTextKey, InspectError> {
    check(budget, cancelled)?;
    if order.general_family().is_none() && !order.is_french_legacy() {
        return Err(InspectError::Unsupported);
    }
    let mut units = &units[..units.len().min(MAX_UNITS)];
    while units.last() == Some(&SPACE_UNIT) {
        check(budget, cancelled)?;
        units = &units[..units.len() - 1];
    }
    let mut work = buffer::Work { budget, cancelled };
    let bytes = if let Some(family) = order.general_family() {
        encode::encode(family, units, descending, &mut work)?
    } else {
        encode::encode_french(units, descending, &mut work)?
    };
    check(budget, cancelled)?;
    Ok(NativeTextKey { bytes })
}
