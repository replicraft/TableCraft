//! Fresh inline-only target checks, separate from readonly LVAL preservation.
use super::binding::{Bound, Target};
use crate::{
    EditError, FittedColumnUpdates, InspectError, LongValueCheckStatus, NativeColumn, NativeImage,
    PropertyGroupKind, WriteBlocker, check_table_long_values,
    format::data_type::{LONG_TEXT, OLE},
    header::check,
    inspect_table_properties,
    table::{
        mutation::{preserved_source::Source, record_serialization::long_value},
        record_layout,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const DESCRIPTOR_BYTES: usize = 3 * size_of::<u32>();
const RESERVED_BYTES: usize = DESCRIPTOR_BYTES - size_of::<u32>();
const INLINE_TAG: u32 = 0x8000_0000;
const INLINE_LIMIT: usize = 64;
const MASK_BITS: usize = u8::BITS as usize;

pub(super) fn validate(field: &NativeColumn, value: &Value) -> Result<(), EditError> {
    long_value::qualify(field)?;
    if matches!(
        (field.native_type(), value),
        (LONG_TEXT | OLE, Value::Null) | (LONG_TEXT, Value::Text(_)) | (OLE, Value::Binary(_))
    ) {
        Ok(())
    } else {
        Err(EditError::Unsupported(WriteBlocker::Column))
    }
}

pub(super) fn qualify(
    image: &NativeImage,
    source: &Source,
    groups: &[FittedColumnUpdates<'_>],
    bound: &Bound,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    if !bound.has_long_targets {
        return Ok(());
    }
    let schema = source.table.schema();
    let scope = image.catalog().catalog().scope();
    let storage =
        check_table_long_values(&mut image.reader(), scope, schema.name(), budget, cancelled)?;
    if storage.rows() != source.records.len() {
        return Err(EditError::Preservation);
    }
    for (group, target) in groups.iter().zip(&bound.targets) {
        check(budget, cancelled)?;
        let Target::Variable { column } = *target else {
            continue;
        };
        let field = &schema.columns()[column];
        if !field.is_ordinary_long_value() {
            continue;
        }
        let mut selected = None;
        for report in storage.columns() {
            check(budget, cancelled)?;
            if report.column_number() == field.number() && selected.replace(report).is_some() {
                return Err(EditError::Preservation);
            }
        }
        let report = selected.ok_or(EditError::Preservation)?;
        if report.status() != LongValueCheckStatus::Checked
            || report
                .values()
                .nulls()
                .checked_add(report.values().inline())
                != Some(source.records.len())
            || report.values().single() != 0
            || report.values().chained() != 0
            || report.owned_pages() != 0
            || report.candidate_pages() != 0
            || report.owned_rows() != 0
            || report.distinct_chunks() != 0
            || report.shared_references() != 0
            || report.shared_owned_pages() != 0
        {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        // Check all original carriers, even for an empty update group. An
        // external reference cannot disappear without a separate retirement law.
        for original in &source.records {
            check(budget, cancelled)?;
            let layout = record_layout::parse(&original.data, schema, budget)?;
            let variable = field.field.variable;
            let start = *layout
                .offsets
                .get(variable)
                .ok_or(EditError::Preservation)?;
            let end = *layout
                .offsets
                .get(variable + 1)
                .ok_or(EditError::Preservation)?;
            let raw = original
                .data
                .get(start..end)
                .ok_or(EditError::Preservation)?;
            let present = original.data[layout.mask_start + field.number() / MASK_BITS]
                & (1 << (field.number() % MASK_BITS))
                != 0;
            if !present {
                if !raw.is_empty() {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                }
                continue;
            }
            budget
                .charge(Resource::Steps, raw.len() as u64)
                .map_err(InspectError::from)?;
            let word = u32::from_le_bytes(
                raw.get(..size_of::<u32>())
                    .ok_or(EditError::Preservation)?
                    .try_into()
                    .map_err(|_| EditError::Preservation)?,
            );
            let length = (word & !INLINE_TAG) as usize;
            if word & INLINE_TAG == 0
                || length > INLINE_LIMIT
                || raw.len() != DESCRIPTOR_BYTES + length
                || raw.get(size_of::<u32>()..DESCRIPTOR_BYTES) != Some(&[0; RESERVED_BYTES])
            {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
        }
        let mut empty_text = false;
        for index in 0..group.len() {
            check(budget, cancelled)?;
            let cell = group.cell(index).ok_or(EditError::Preservation)?;
            empty_text |= matches!(cell.value, Value::Text(value) if value.is_empty());
        }
        if empty_text {
            let properties = inspect_table_properties(
                &mut image.reader(),
                scope,
                schema.name(),
                budget,
                cancelled,
            )?;
            if properties.object_id() != schema.object_id() {
                return Err(EditError::Preservation);
            }
            let mut declared = false;
            for group in properties.groups() {
                check(budget, cancelled)?;
                budget
                    .charge(
                        Resource::Steps,
                        group.name().len().max(field.name().len()) as u64,
                    )
                    .map_err(InspectError::from)?;
                if group.kind() != PropertyGroupKind::Column
                    || !group.name().eq_ignore_ascii_case(field.name())
                {
                    continue;
                }
                for property in group.properties() {
                    check(budget, cancelled)?;
                    budget
                        .charge(
                            Resource::Steps,
                            property.name().len().max("AllowZeroLength".len()) as u64,
                        )
                        .map_err(InspectError::from)?;
                    declared |= property.name().eq_ignore_ascii_case("AllowZeroLength");
                }
                break;
            }
            if !declared {
                return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
            }
        }
    }
    check(budget, cancelled)?;
    Ok(())
}
