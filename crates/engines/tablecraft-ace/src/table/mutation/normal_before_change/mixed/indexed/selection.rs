//! Readonly original-key route facts; never a save or macro-absence permit.
use crate::{
    EditError, InspectError, NativeImage, WriteBlocker,
    format::{data_type::INT32, row::MASK_BITS},
    header::check,
    inspect_table_indexes,
    limits::MAX_LIVE_ROWS,
    table::{
        mutation::{guards::buffer, record_roots, rows::RowAssignment},
        record_layout,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

pub(super) enum Route {
    LegacyUnindexed,
    Indexed,
}

struct Ordinal(usize);

impl RowAssignment for Ordinal {
    fn row(&self) -> usize {
        self.0
    }
}

pub(super) fn capture(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Route, EditError> {
    let indexes = inspect_table_indexes(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?;
    let schema = indexes.schema();
    if schema.columns().len() != schema.layout.max_columns {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if field.number() != position {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
    }
    let mut fields = buffer::vector(schema.columns().len(), budget, cancelled)?;
    for index in indexes.indexes() {
        for key in index.columns() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, schema.columns().len() as u64)
                .map_err(InspectError::from)?;
            let field = schema
                .columns()
                .iter()
                .find(|field| field.number() == key.number())
                .ok_or(InspectError::Malformed)?;
            if field.native_type() != INT32
                || field.scalar_type() != Some(ScalarType::Int32)
                || !field.fixed()
                || field.storage_length() != size_of::<i32>()
                || field.extended_flags() != 0
            {
                // Metadata selection, never a caught planner failure. The
                // original qualifier must still decide its original scope.
                return Ok(Route::LegacyUnindexed);
            }
            budget
                .charge(Resource::Steps, fields.len() as u64)
                .map_err(InspectError::from)?;
            if !fields.contains(&field.number()) {
                fields.push(field.number());
            }
        }
    }
    let count = usize::try_from(schema.declared_rows())
        .ok()
        .filter(|count| *count <= MAX_LIVE_ROWS)
        .ok_or(InspectError::StructureLimit)?;
    let mut ordinals = buffer::vector(count, budget, cancelled)?;
    for ordinal in 0..count {
        check(budget, cancelled)?;
        ordinals.push(Ordinal(ordinal));
    }
    let roots = record_roots::capture(image, schema, &ordinals, budget, cancelled)?;
    let mut nullable = false;
    for root in roots.selected {
        check(budget, cancelled)?;
        let layout = record_layout::parse(&root.final_record.data, schema, budget)?;
        if layout.count != schema.layout.max_columns {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        for &number in &fields {
            check(budget, cancelled)?;
            let mask = layout
                .mask_start
                .checked_add(number / MASK_BITS)
                .ok_or(InspectError::Malformed)?;
            let byte = root
                .final_record
                .data
                .get(mask)
                .ok_or(InspectError::Malformed)?;
            nullable |= byte & (1 << (number % MASK_BITS)) == 0;
        }
    }
    Ok(if nullable {
        Route::LegacyUnindexed
    } else {
        Route::Indexed
    })
}
