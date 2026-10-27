//! Exact ordinary LVAL fields qualified by storage observations.
use crate::{
    NativeColumn,
    format::{
        data_type::{LONG_TEXT, OLE},
        layout::UPDATABLE_FLAG,
    },
};

const COMPRESSED_UNICODE_FLAG: u8 = 1;

pub(in crate::table) fn ordinary(field: &NativeColumn) -> bool {
    !field.fixed()
        && field.storage_length() == 0
        && field.flags() == UPDATABLE_FLAG
        && matches!(
            (field.native_type(), field.extended_flags()),
            (LONG_TEXT | OLE, 0) | (LONG_TEXT, COMPRESSED_UNICODE_FLAG)
        )
}
