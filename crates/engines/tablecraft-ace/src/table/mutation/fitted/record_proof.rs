//! Fresh original requests/records authorize rows and joint key maintenance.
use super::{
    request::{self, FittedColumnUpdates},
    stage::Staged,
};
use crate::{
    EditError, InspectError, NativeImage,
    format::{
        reader::Reader,
        row::{COLUMN_COUNT_BYTES, MASK_BITS},
    },
    table::{
        indexes::{IndexDataPatch, RowKeyUpdate, index_row_patches, qualify_row_indexes},
        mutation::{gates, page_update::roots, variable_update::record_proof},
        record, record_layout,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

struct ProvenRow {
    reference: u32,
    original: Vec<Value>,
    final_values: Vec<Value>,
}

pub(super) fn prove(
    image: &NativeImage,
    table: &str,
    groups: &[FittedColumnUpdates<'_>],
    staged: &Staged,
    indexes: &[IndexDataPatch],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    // Capture qualification from this original image again. Neither a producer
    // TableCheck nor any staged column/key cache determines proof authority.
    let original = gates::before_slot(image, table, budget, cancelled)?;
    let schema = original.schema();
    let bound = request::bind(schema, groups, budget, cancelled)?;
    qualify_row_indexes(
        schema,
        original.indexes(),
        &bound.columns,
        budget,
        cancelled,
    )?;
    if bound.columns != staged.affected {
        return Err(EditError::Preservation);
    }

    budget
        .charge_allocation(schema.columns().len() as u64, size_of::<usize>() as u64)
        .map_err(InspectError::from)?;
    let mut projection = Vec::new();
    projection
        .try_reserve_exact(schema.columns().len())
        .map_err(|_| InspectError::Allocation)?;
    budget
        .charge(Resource::Steps, original.indexes().len() as u64)
        .map_err(InspectError::from)?;
    let key_columns = original
        .indexes()
        .iter()
        .try_fold(0_usize, |count, index| {
            count
                .checked_add(index.index().columns().len())
                .ok_or(InspectError::StructureLimit)
        })?;
    // Each key can scan the source schema while prepaying names and again
    // during qualification. Use actual key membership, not a guessed width.
    let lookup_width = schema
        .columns()
        .len()
        .checked_add(schema.columns().len())
        .and_then(|count| count.checked_add(groups.len()))
        .and_then(|count| count.checked_add(1))
        .ok_or(InspectError::StructureLimit)?;
    let work = key_columns
        .checked_mul(lookup_width)
        .and_then(|count| count.checked_add(groups.len()))
        .ok_or(InspectError::StructureLimit)?;

    for (position, field) in schema.columns().iter().enumerate() {
        request::check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, work as u64)
            .map_err(InspectError::from)?;
        for group in groups {
            request::pay_names(field.name(), group.name(), budget, cancelled)?;
        }
        for index in original.indexes() {
            for key in index.index().columns() {
                if let Some(candidate) = schema
                    .columns()
                    .iter()
                    .find(|candidate| candidate.number() == key.number())
                {
                    for group in groups {
                        request::pay_names(candidate.name(), group.name(), budget, cancelled)?;
                    }
                }
            }
        }
        let requested = groups
            .iter()
            .any(|group| group.name().eq_ignore_ascii_case(field.name()));
        let sibling = original.indexes().iter().any(|index| {
            index
                .index()
                .columns()
                .iter()
                .any(|key| key.number() == field.number())
                && index.index().columns().iter().any(|key| {
                    schema.columns().iter().any(|candidate| {
                        candidate.number() == key.number()
                            && groups
                                .iter()
                                .any(|group| group.name().eq_ignore_ascii_case(candidate.name()))
                    })
                })
        });
        if requested || sibling {
            projection.push(position);
        }
    }
    if projection != staged.projection {
        return Err(EditError::Preservation);
    }

    let captured = roots::capture(image, schema, &bound.rows, budget, cancelled)?;
    budget
        .charge_allocation(
            captured.selected.len() as u64,
            size_of::<ProvenRow>() as u64,
        )
        .map_err(InspectError::from)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(captured.selected.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };

    for root in captured.selected {
        reader.tick()?;
        let before = &root.final_record.data;
        let layout = record_layout::parse(before, schema, reader.budget)?;
        if layout.count != schema.layout.max_columns {
            return Err(EditError::Preservation);
        }
        let before_values =
            record::decode_selected(before, schema, Some(&projection), &mut reader)?;
        let mut changed = false;
        for group in groups {
            reader.work(group.len() + schema.columns().len() + projection.len())?;
            request::pay_name_scan(schema, group.name(), reader.budget, cancelled)?;
            let column = schema
                .columns()
                .iter()
                .position(|field| field.name().eq_ignore_ascii_case(group.name()))
                .ok_or(EditError::Preservation)?;
            if let Some(cell) = (0..group.len())
                .filter_map(|i| group.cell(i))
                .find(|cell| cell.row == root.update.0)
            {
                let at = projection
                    .iter()
                    .position(|&candidate| candidate == column)
                    .ok_or(EditError::Preservation)?;
                let value = before_values.get(at).ok_or(EditError::Preservation)?;
                if !request::values_equal(value, cell.expected, reader.budget, cancelled)? {
                    return Err(EditError::Conflict);
                }
                changed |=
                    !request::values_equal(cell.expected, cell.value, reader.budget, cancelled)?;
            }
        }
        if !changed {
            continue;
        }

        // This is the separate existing immutable record-proof implementation,
        // not page_update::record::build or a producer-built after image.
        let variable = &groups[bound.variable_group];
        reader.work(variable.len())?;
        let cell = (0..variable.len())
            .filter_map(|i| variable.cell(i))
            .find(|cell| cell.row == root.update.0);
        let variable_value = if let Some(cell) = cell {
            if request::values_equal(cell.expected, cell.value, reader.budget, cancelled)? {
                None
            } else {
                Some(cell.value)
            }
        } else {
            None
        };
        let mut expected = if let Some(value) = variable_value {
            record_proof::reconstruct_declared(
                schema,
                bound.columns[bound.variable_group],
                before,
                value,
                reader.budget,
                cancelled,
            )?
        } else {
            super::stage::copy(before, reader.budget, cancelled)?
        };
        if expected.len() != before.len() {
            return Err(EditError::Preservation);
        }

        for group in groups {
            if !matches!(group, FittedColumnUpdates::Fixed { .. }) {
                continue;
            }
            reader.work(group.len() + schema.columns().len())?;
            let Some(cell) = (0..group.len())
                .filter_map(|i| group.cell(i))
                .find(|cell| cell.row == root.update.0)
            else {
                continue;
            };
            request::pay_name_scan(schema, group.name(), reader.budget, cancelled)?;
            let field = schema
                .columns()
                .iter()
                .find(|field| field.name().eq_ignore_ascii_case(group.name()))
                .ok_or(EditError::Preservation)?;
            let (bytes, width, present) = request::fixed_bytes(field, cell.value)?;
            if present && width != 0 {
                let start = COLUMN_COUNT_BYTES + field.field.offset;
                let end = start
                    .checked_add(width)
                    .ok_or(InspectError::StructureLimit)?;
                if end > layout.data_end {
                    return Err(EditError::Preservation);
                }
                expected
                    .get_mut(start..end)
                    .ok_or(EditError::Preservation)?
                    .copy_from_slice(&bytes[..width]);
            }
            let mask = expected
                .get_mut(layout.mask_start + field.number() / MASK_BITS)
                .ok_or(EditError::Preservation)?;
            let bit = 1 << (field.number() % MASK_BITS);
            *mask = (*mask & !bit) | if present { bit } else { 0 };
        }

        let producer = staged
            .records
            .get(rows.len())
            .ok_or(EditError::Preservation)?;
        let final_values =
            record::decode_selected(&expected, schema, Some(&projection), &mut reader)?;
        reader.work(before.len() + expected.len() + projection.len())?;
        if producer.reference != root.reference
            || producer.offset != root.final_record.offset
            || producer.before != *before
            || producer.after != expected
            || !request::rows_equal(
                &producer.original_values,
                &before_values,
                reader.budget,
                cancelled,
            )?
            || !request::rows_equal(
                &producer.final_values,
                &final_values,
                reader.budget,
                cancelled,
            )?
        {
            return Err(EditError::Preservation);
        }
        rows.push(ProvenRow {
            reference: root.reference,
            original: before_values,
            final_values,
        });
    }
    if rows.len() != staged.records.len() {
        return Err(EditError::Preservation);
    }

    // Rebuild joint final key inputs exclusively from independently proven rows.
    reader
        .budget
        .charge_allocation(rows.len() as u64, size_of::<RowKeyUpdate<'_>>() as u64)
        .map_err(InspectError::from)?;
    let mut keys = Vec::new();
    keys.try_reserve_exact(rows.len())
        .map_err(|_| InspectError::Allocation)?;
    for row in &rows {
        reader.tick()?;
        keys.push(RowKeyUpdate {
            reference: row.reference,
            columns: &projection,
            before: &row.original,
            after: &row.final_values,
        });
    }
    let expected_indexes = index_row_patches(
        image,
        schema,
        original.indexes(),
        &bound.columns,
        &keys,
        reader.budget,
        cancelled,
    )?;
    if expected_indexes.len() != indexes.len() {
        return Err(EditError::Preservation);
    }
    for (expected, actual) in expected_indexes.iter().zip(indexes) {
        reader.tick()?;
        reader.work(expected.before.len() + expected.after.len())?;
        if expected.offset != actual.offset
            || expected.before != actual.before
            || expected.after != actual.after
        {
            return Err(EditError::Preservation);
        }
    }
    reader.tick()?;
    Ok(rows.len())
}
