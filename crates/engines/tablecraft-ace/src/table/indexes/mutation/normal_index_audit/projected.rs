//! Key-only original raw I32 population for complete mixed scalar tables.
use super::{Ownership, population::Row, vector, wire};
use crate::{
    EditError, IndexCheckStatus, InspectError, NativeImage, TableCheck, WriteBlocker,
    format::{
        allocation,
        bytes::u32_at,
        data_type::INT32,
        page::{DATA_SIGNATURE, OWNER_OFFSET},
        reader::{Reader, live_row_range, row_count},
        row::{COLUMN_COUNT_BYTES, MASK_BITS, reference},
    },
    header::check,
    table::record_layout,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

pub(in crate::table) fn reconstruct<F>(
    image: &NativeImage,
    checked: &TableCheck,
    expected: &mut [u8],
    mut final_value: F,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError>
where
    F: FnMut(usize, usize, i32, &mut ResourceBudget) -> Result<i32, EditError>,
{
    reconstruct_nullable(
        image,
        checked,
        expected,
        |ordinal, position, original, budget| match original {
            Some(original) => final_value(ordinal, position, original, budget).map(Some),
            None => Ok(None),
        },
        budget,
        cancelled,
    )
}

/// Original NULL cells are passed as None without reading inactive payload.
/// The callback must be bound to the proof's own fresh original requests;
/// producer key deltas, event reports and candidate bytes are not authority.
pub(in crate::table) fn reconstruct_nullable<F>(
    image: &NativeImage,
    checked: &TableCheck,
    expected: &mut [u8],
    mut final_value: F,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError>
where
    F: FnMut(usize, usize, Option<i32>, &mut ResourceBudget) -> Result<Option<i32>, EditError>,
{
    let _span = tracing::debug_span!("native.before_change.index_projected_proof").entered();
    check(budget, cancelled)?;
    if expected.len() != image.as_bytes().len() || checked.indexes().is_empty() {
        return Err(EditError::Preservation);
    }
    let schema = checked.schema();
    let ownership = Ownership::capture(image, checked, budget, cancelled)?;
    let mut positions = vector(schema.columns().len(), budget, cancelled)?;
    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        let mut indexed = false;
        for index in checked.indexes() {
            if index.status() != IndexCheckStatus::Checked {
                return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
            }
            for key in index.index().columns() {
                check(budget, cancelled)?;
                indexed |= key.number() == field.number();
            }
        }
        if !indexed {
            continue;
        }
        if field.native_type() != INT32
            || field.scalar_type() != Some(ScalarType::Int32)
            || !field.fixed()
            || field.storage_length() != size_of::<i32>()
            || field.extended_flags() != 0
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        positions.push(position);
    }

    let mut population = vector(checked.rows(), budget, cancelled)?;
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    for number in allocation::pages(&mut reader, schema.layout.usage)? {
        let page = reader.page(number)?;
        if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || u32_at(&page, OWNER_OFFSET)? != schema.definition_page()
        {
            return Err(InspectError::Malformed.into());
        }
        for slot in 0..row_count(&page)? {
            reader.tick()?;
            if live_row_range(&page, slot)?.is_none() {
                continue;
            }
            if population.len() >= checked.rows() {
                return Err(EditError::Preservation);
            }
            reader
                .budget
                .charge(Resource::Rows, 1)
                .map_err(InspectError::from)?;
            let slot = u8::try_from(slot).map_err(|_| InspectError::Malformed)?;
            let record = reader.row_location(number, slot, schema.definition_page(), &page)?;
            let layout = record_layout::parse(&record.data, schema, reader.budget)?;
            if layout.count != schema.layout.max_columns {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            let mut before = vector(positions.len(), reader.budget, cancelled)?;
            let mut after = vector(positions.len(), reader.budget, cancelled)?;
            let ordinal = population.len();
            for &position in &positions {
                reader.tick()?;
                let field = &schema.columns()[position];
                let start = COLUMN_COUNT_BYTES
                    .checked_add(field.field.offset)
                    .ok_or(EditError::Preservation)?;
                let end = start
                    .checked_add(size_of::<i32>())
                    .ok_or(EditError::Preservation)?;
                let mask = layout
                    .mask_start
                    .checked_add(field.number() / MASK_BITS)
                    .ok_or(EditError::Preservation)?;
                if end > layout.mask_start {
                    return Err(EditError::Preservation);
                }
                let present = record.data.get(mask).ok_or(EditError::Preservation)?
                    & (1 << (field.number() % MASK_BITS))
                    != 0;
                let value = if present {
                    reader.work(size_of::<i32>())?;
                    Some(i32::from_le_bytes(
                        record
                            .data
                            .get(start..end)
                            .ok_or(EditError::Preservation)?
                            .try_into()
                            .map_err(|_| EditError::Preservation)?,
                    ))
                } else {
                    None
                };
                before.push(value);
                after.push(final_value(ordinal, position, value, reader.budget)?);
            }
            population.push(Row {
                reference: reference(number, slot),
                before,
                after,
            });
        }
    }
    if population.len() != checked.rows() {
        return Err(EditError::Preservation);
    }
    for (position, index) in checked.indexes().iter().enumerate() {
        reader.tick()?;
        reader.work(position)?;
        let native = index.index();
        if checked.indexes()[..position]
            .iter()
            .any(|prior| prior.index().data_number() == native.data_number())
        {
            continue;
        }
        let tree = ownership.require(image, schema, native, reader.budget, cancelled)?;
        wire::reconstruct(
            &wire::Operation {
                image,
                checked,
                index: native,
                population: wire::Population::Nullable(&population),
                columns: wire::Columns::Keys(&positions),
                tree,
            },
            expected,
            reader.budget,
            cancelled,
        )?;
    }
    check(reader.budget, cancelled)?;
    Ok(())
}
