//! Physical outer-LVAL eligibility does not grant scalar or mutation authority.
use crate::{
    NativeColumn,
    format::{
        data_type::{LONG_TEXT, OLE},
        layout::UPDATABLE_FLAG,
    },
    table::{calculated, long_value::profile},
};

// Observed on the live system attachment FileData backing field. Its value
// wrapper stays opaque; only the already established outer LVAL is followed.
const OBSERVED_OPAQUE_COMPLEX_EXT_FLAGS: u8 = 0x10;

pub(super) fn readable(column: &NativeColumn) -> bool {
    if column.fixed() || !matches!(column.native_type(), LONG_TEXT | OLE) {
        return false;
    }
    if profile::ordinary(column) {
        return true;
    }

    if column.scalar_type().is_some() {
        return true;
    }
    // The calculated Memo reader follows this same outer LVAL before decoding
    // the cache wrapper. Unsupported calculated declarations remain refused.
    if column.native_type() == LONG_TEXT && calculated::supported(column) {
        return true;
    }

    column.native_type() == OLE
        && column.storage_length() == 0
        && column.flags() == UPDATABLE_FLAG
        && column.extended_flags() == OBSERVED_OPAQUE_COMPLEX_EXT_FLAGS
}
