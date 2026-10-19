use crate::{
    NativeColumn, ReadProfile,
    format::{data_type, layout::UPDATABLE_FLAG},
};

// The reviewed Jackcess 5.0.4 ACE 16 templates declare Decimal Value as length 9,
// precision 18/scale 0. Its generated flat table stores the usual 17-byte field.
// This is a conceptual template hint, never an alternative record decoder.
const CONCEPTUAL_DECIMAL_BYTES: usize = 9;
const CONCEPTUAL_DECIMAL_PARAMETERS: (u8, u8) = (18, 0);

pub(super) fn conceptual_decimal(column: &NativeColumn) -> bool {
    matches!(
        column.read_profile(),
        ReadProfile::Ace2016 | ReadProfile::Ace2019
    ) && column.native_type() == data_type::NUMERIC
        && column.storage_length() == CONCEPTUAL_DECIMAL_BYTES
        && column.decimal_parameters() == Some(CONCEPTUAL_DECIMAL_PARAMETERS)
        && !column.fixed()
        && column.flags() == UPDATABLE_FLAG
        && column.extended_flags() == 0
}

pub(super) fn same_length(
    template: &NativeColumn,
    flat: &NativeColumn,
    template_rows: u32,
) -> bool {
    template.storage_length() == flat.storage_length()
        || (template_rows == 0
            && conceptual_decimal(template)
            && flat.storage_length() == data_type::NUMERIC_BYTES
            && flat.decimal_parameters() == template.decimal_parameters()
            && !flat.fixed()
            && flat.flags() == UPDATABLE_FLAG
            && flat.extended_flags() == 0)
}
