use super::super::schema::NativeColumn;
use crate::{
    ReadProfile,
    format::{
        data_type,
        layout::{AUTO_NUMBER_FLAG, FIXED_FLAG, UPDATABLE_FLAG},
    },
};

// Observed on version-history parents in the independent fixtures. Qualifies
// only the fixed stored key; no history behavior is inferred from this bit.
pub(super) const OBSERVED_HISTORY_EXT_FLAG: u8 = 0x20;

pub(in crate::table) fn supported_key(column: &NativeColumn) -> bool {
    matches!(
        column.read_profile(),
        ReadProfile::Ace2016 | ReadProfile::Ace2019
    ) && column.native_type() == data_type::COMPLEX
        && column.fixed()
        && column.storage_length() == data_type::INT32_BYTES
        && column.flags() == FIXED_FLAG | UPDATABLE_FLAG | AUTO_NUMBER_FLAG
        && matches!(column.extended_flags(), 0 | OBSERVED_HISTORY_EXT_FLAG)
}
