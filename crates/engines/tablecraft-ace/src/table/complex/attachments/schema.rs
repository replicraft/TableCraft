use crate::{
    ComplexColumnLinks, ComplexColumnSchema, InspectError, NativeColumn, TableSchema,
    format::{
        data_type,
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
    },
    header::check,
};
use tablecraft_core::limits::ResourceBudget;

pub(super) const URL: usize = 0;
pub(super) const NAME: usize = 1;
pub(super) const TYPE: usize = 2;
pub(super) const DATA: usize = 3;
pub(super) const TIMESTAMP: usize = 4;
pub(super) const FLAGS: usize = 5;
pub(super) const FIELD_COUNT: usize = 6;
// Observed only on the empty conceptual attachment template; generated child
// fields have ordinary scalar flags. This does not qualify flagged record data.
const CONCEPTUAL_ATTACHMENT_EXT_FLAG: u8 = 0x10;
pub(super) const FIELDS: [(&str, u8); FIELD_COUNT] = [
    ("FileURL", data_type::LONG_TEXT),
    ("FileName", data_type::SHORT_TEXT),
    ("FileType", data_type::SHORT_TEXT),
    ("FileData", data_type::OLE),
    ("FileTimeStamp", data_type::DATE_TIME),
    ("FileFlags", data_type::INT32),
];

pub(super) struct Columns {
    pub positions: [usize; FIELD_COUNT],
    pub identity: usize,
}
pub(super) fn read(
    links: &ComplexColumnLinks,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Columns, InspectError> {
    qualify(
        links.column(),
        links.child_snapshot().schema(),
        budget,
        cancelled,
    )
}
pub(super) fn qualify(
    column: &ComplexColumnSchema,
    flat: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Columns, InspectError> {
    let template = column.type_table();
    if template.columns().len() != FIELD_COUNT || template.declared_rows() != 0 {
        return Err(InspectError::Unsupported);
    }
    let mut positions = [0; FIELD_COUNT];
    for (i, (name, kind)) in FIELDS.iter().enumerate() {
        check(budget, cancelled)?;
        let conceptual = template
            .columns()
            .iter()
            .find(|field| field.name().eq_ignore_ascii_case(name))
            .ok_or(InspectError::Unsupported)?;
        let (position, field) = flat
            .columns()
            .iter()
            .enumerate()
            .find(|(_, field)| field.name().eq_ignore_ascii_case(name))
            .ok_or(InspectError::Malformed)?;
        if !qualified(conceptual, *kind, true)
            || !qualified(field, *kind, false)
            || field.scalar_type().is_none()
        {
            return Err(InspectError::Unsupported);
        }
        positions[i] = position;
    }
    let identity = flat
        .columns()
        .iter()
        .position(|field| field.number() == column.identity_number())
        .ok_or(InspectError::Malformed)?;
    Ok(Columns {
        positions,
        identity,
    })
}
fn qualified(field: &NativeColumn, kind: u8, conceptual: bool) -> bool {
    let flags = UPDATABLE_FLAG | if field.fixed() { FIXED_FLAG } else { 0 };
    field.native_type() == kind
        && field.fixed() == matches!(kind, data_type::DATE_TIME | data_type::INT32)
        && field.flags() == flags
        && (field.extended_flags() == 0
            || (conceptual && field.extended_flags() == CONCEPTUAL_ATTACHMENT_EXT_FLAG))
}
