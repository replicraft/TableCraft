use crate::{
    ComplexColumnLinks, ComplexColumnSchema, InspectError, NativeColumn, ReadProfile, TableSchema,
    format::{
        data_type,
        layout::{AUTO_NUMBER_FLAG, COMPLEX_FK_EXT_FLAG, FIXED_FLAG, UPDATABLE_FLAG},
    },
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::{Column, ScalarType};

pub(super) const URL: usize = 0;
pub(super) const NAME: usize = 1;
pub(super) const TYPE: usize = 2;
pub(super) const DATA: usize = 3;
pub(super) const TIMESTAMP: usize = 4;
pub(super) const FLAGS: usize = 5;
pub(in crate::table::complex) const FIELD_COUNT: usize = 6;
const REFERENCE_COUNT: usize = 2;
// Native attachment marker on empty conceptual templates and the qualified
// modern child payload fields. It never grants generic scalar-read eligibility.
const CONCEPTUAL_ATTACHMENT_EXT_FLAG: u8 = 0x10;
const MODERN_IDENTITY_EXT_FLAG: u8 = 0x04;
pub(super) const FIELDS: [(&str, u8); FIELD_COUNT] = [
    ("FileURL", data_type::LONG_TEXT),
    ("FileName", data_type::SHORT_TEXT),
    ("FileType", data_type::SHORT_TEXT),
    ("FileData", data_type::OLE),
    ("FileTimeStamp", data_type::DATE_TIME),
    ("FileFlags", data_type::INT32),
];

const MEMBER_TYPES: [ScalarType; FIELD_COUNT] = [
    ScalarType::Text,
    ScalarType::Text,
    ScalarType::Text,
    ScalarType::Binary,
    ScalarType::DateTime,
    ScalarType::Int32,
];

pub(super) fn member_type(name: &str) -> Option<ScalarType> {
    let role = FIELDS
        .iter()
        .position(|(field, _)| field.eq_ignore_ascii_case(name))?;
    Some(MEMBER_TYPES[role])
}

pub(in crate::table::complex) struct Columns {
    pub positions: [usize; FIELD_COUNT],
    pub identity: usize,
    foreign_key: usize,
}
impl Columns {
    pub(in crate::table::complex) fn kind(
        &self,
        position: usize,
    ) -> Result<ScalarType, InspectError> {
        if position == self.identity || position == self.foreign_key {
            return Ok(ScalarType::Int32);
        }
        let role = self
            .positions
            .iter()
            .position(|&value| value == position)
            .ok_or(InspectError::Malformed)?;
        Ok(MEMBER_TYPES[role])
    }
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
pub(in crate::table::complex) fn qualify(
    column: &ComplexColumnSchema,
    flat: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Columns, InspectError> {
    let template = column.type_table();
    check(budget, cancelled)?;
    if flat.object_id() != column.flat_table().object_id()
        || flat.definition_page() != column.flat_table().definition_page()
        || flat.header != column.flat_table().header
        || flat.layout != column.flat_table().layout
    {
        return Err(InspectError::Malformed);
    }
    if template.columns().len() != FIELD_COUNT
        || template.declared_rows() != 0
        || flat.columns().len() != FIELD_COUNT + REFERENCE_COUNT
    {
        return Err(InspectError::Unsupported);
    }
    budget.charge(
        Resource::Steps,
        (flat.columns().len() * REFERENCE_COUNT) as u64,
    )?;
    let reference = |number| {
        flat.columns()
            .iter()
            .enumerate()
            .find(|(_, field)| field.number() == number)
            .ok_or(InspectError::Malformed)
    };
    let (foreign_key, foreign) = reference(column.foreign_key_number())?;
    let (identity, id) = reference(column.identity_number())?;
    let modern = matches!(
        foreign.read_profile(),
        ReadProfile::Ace2007 | ReadProfile::Ace2016 | ReadProfile::Ace2019
    ) && !foreign.fixed()
        && !id.fixed()
        && foreign.flags() == UPDATABLE_FLAG
        && foreign.extended_flags() == COMPLEX_FK_EXT_FLAG
        && id.flags() == UPDATABLE_FLAG | AUTO_NUMBER_FLAG
        && id.extended_flags() == MODERN_IDENTITY_EXT_FLAG;
    let legacy = foreign.fixed()
        && id.fixed()
        && foreign.flags() & AUTO_NUMBER_FLAG == 0
        && id.flags() & AUTO_NUMBER_FLAG != 0
        && foreign.extended_flags() == COMPLEX_FK_EXT_FLAG
        && id.extended_flags() == 0;
    if !(legacy || modern)
        || foreign_key == identity
        || [foreign, id].iter().any(|field| {
            field.native_type() != data_type::INT32
                || field.storage_length() != data_type::INT32_BYTES
        })
    {
        return Err(InspectError::Unsupported);
    }
    let mut positions = [0; FIELD_COUNT];
    for (i, (name, kind)) in FIELDS.iter().enumerate() {
        check(budget, cancelled)?;
        budget.charge(
            Resource::Steps,
            (template.columns().len() + flat.columns().len()) as u64,
        )?;
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
        if position == foreign_key || position == identity || positions[..i].contains(&position) {
            return Err(InspectError::Malformed);
        }
        if !qualified(conceptual, *kind, true, modern) || !qualified(field, *kind, false, modern) {
            return Err(InspectError::Unsupported);
        }
        positions[i] = position;
    }
    Ok(Columns {
        positions,
        identity,
        foreign_key,
    })
}
fn qualified(field: &NativeColumn, kind: u8, conceptual: bool, modern: bool) -> bool {
    let flags = UPDATABLE_FLAG | if field.fixed() { FIXED_FLAG } else { 0 };
    field.native_type() == kind
        && field.fixed() == matches!(kind, data_type::DATE_TIME | data_type::INT32)
        && field.flags() == flags
        && if modern {
            field.extended_flags() == CONCEPTUAL_ATTACHMENT_EXT_FLAG
        } else {
            field.extended_flags() == 0
                || (conceptual && field.extended_flags() == CONCEPTUAL_ATTACHMENT_EXT_FLAG)
        }
        && match kind {
            data_type::INT32 => field.storage_length() == data_type::INT32_BYTES,
            data_type::DATE_TIME => field.storage_length() == data_type::DATE_TIME_BYTES,
            data_type::SHORT_TEXT => {
                field.storage_length() > 0
                    && field.storage_length() <= data_type::MAX_SHORT_TEXT_BYTES
                    && field.storage_length().is_multiple_of(size_of::<u16>())
            }
            data_type::LONG_TEXT | data_type::OLE => field.storage_length() == 0,
            _ => false,
        }
}

/// Build carriers only after the complete resolved attachment declaration qualifies.
/// Raw fields retain their native flags, offsets and variable ordinals.
pub(in crate::table::complex) fn projection(
    column: &ComplexColumnSchema,
    flat: &TableSchema,
    names: Option<&[&str]>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<usize>, Vec<Column>), InspectError> {
    let qualified = qualify(column, flat, budget, cancelled)?;
    let count = names.map_or(flat.columns().len(), <[&str]>::len);
    if count > flat.columns().len() {
        return Err(InspectError::Malformed);
    }
    budget.charge_allocation(count as u64, size_of::<usize>() as u64)?;
    budget.charge_allocation(count as u64, size_of::<Column>() as u64)?;
    let mut positions = Vec::new();
    let mut columns = Vec::new();
    positions
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    columns
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    for (position, native) in flat.columns().iter().enumerate() {
        check(budget, cancelled)?;
        let selected = match names {
            Some(names) => {
                let mut matched = false;
                for name in names {
                    check(budget, cancelled)?;
                    matched |= native.name().eq_ignore_ascii_case(name);
                }
                matched
            }
            None => true,
        };
        if selected {
            positions.push(position);
            columns.push(crate::table::snapshot::projected_column(
                native,
                qualified.kind(position)?,
                budget,
            )?);
        }
    }
    if positions.len() != count {
        return Err(InspectError::Malformed);
    }
    Ok((positions, columns))
}
