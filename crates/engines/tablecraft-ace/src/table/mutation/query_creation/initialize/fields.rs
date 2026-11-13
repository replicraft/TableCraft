//! Native system declarations, separate from SQL/query evaluation metadata.
use crate::{creation::profile::Field, format::data_type};

const fn field(
    name: &'static str,
    kind: u8,
    offset: Option<usize>,
    width: usize,
    variable: usize,
) -> Field<'static> {
    Field {
        name,
        kind,
        offset,
        width,
        decimal: None,
        native_flags: Some(if offset.is_some() { 3 } else { 2 }),
        native_variable: Some(variable),
        unicode_compression: false,
    }
}

pub(super) const FIELDS: [Field<'static>; 8] = [
    field(
        "ObjectId",
        data_type::INT32,
        Some(0),
        data_type::INT32_BYTES,
        0,
    ),
    field(
        "Attribute",
        data_type::BYTE,
        Some(4),
        data_type::BYTE_BYTES,
        0,
    ),
    field("Order", data_type::SHORT_BINARY, None, 4, 0),
    field("Name1", data_type::SHORT_TEXT, None, 510, 1),
    field("Name2", data_type::SHORT_TEXT, None, 510, 2),
    field("Expression", data_type::LONG_TEXT, None, 0, 3),
    field("Flag", data_type::INT16, Some(5), data_type::INT16_BYTES, 4),
    field(
        "LvExtra",
        data_type::INT32,
        Some(7),
        data_type::INT32_BYTES,
        4,
    ),
];
