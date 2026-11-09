//! Explicit one-operation clock input for fresh normal legacy DATE defaults.
mod bind;
mod proof;
mod selection;

use crate::table::mutation::insert_defaults::clock as default_clock;
use crate::{
    EditError, InspectError, NativeEditPlan, NativeImage, OmittedRowInsert, PropertyGroupKind,
    TableProperties, TableSchema, WriteBlocker,
    format::data_type::{DATE_TIME, INT32, LONG_TEXT},
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_expressions::functions::dates::clock::{Clock, ClockReading};
use tablecraft_schema::ScalarType;

const PROFILE_COLUMNS: usize = 2;
const FIXED_ROW_BYTES: usize = size_of::<u16>() + size_of::<i32>() + size_of::<f64>();
const DDL_PROPERTY: u8 = 1;
const FIXED_UPDATABLE_FLAGS: u8 =
    crate::format::layout::FIXED_FLAG | crate::format::layout::UPDATABLE_FLAG;

impl NativeImage {
    /// Rebuild original presence with retained execution input; no host callback.
    pub(crate) fn plan_ordinary_clock_row_inserts(
        self,
        table: &str,
        requests: &[OmittedRowInsert],
        reading: Option<ClockReading>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let bound = prepare(&self, table, requests, budget, cancelled)?;
        finish(self, table, requests, bound, reading, budget, cancelled)
    }

    /// Bind every needed declaration and validate carriers/capacity before one read.
    pub(crate) fn capture_ordinary_clock_row_inserts(
        self,
        table: &str,
        requests: &[OmittedRowInsert],
        provider: &dyn Clock,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(NativeEditPlan, Option<ClockReading>), EditError> {
        let bound = prepare(&self, table, requests, budget, cancelled)?;
        let reading = if bound.needs_clock() {
            let captured = default_clock::capture(provider, budget, cancelled)?;
            // DATE/Now consume civil whole seconds; unused Timer bits are not
            // retained as part of this operation's canonical execution input.
            Some(ClockReading::new(captured.date_time()).map_err(EditError::DefaultEvaluation)?)
        } else {
            None
        };
        let plan = finish(self, table, requests, bound, reading, budget, cancelled)?;
        Ok((plan, reading))
    }
}

fn prepare(
    image: &NativeImage,
    table: &str,
    requests: &[OmittedRowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bind::BoundRows, EditError> {
    let source = super::capture_source(image, table, budget, cancelled)?;
    source
        .records
        .len()
        .checked_add(requests.len())
        .filter(|total| *total <= crate::limits::MAX_LIVE_ROWS)
        .ok_or(InspectError::StructureLimit)?;
    if !source.indexes.indexes().is_empty() {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    qualify(source.table.schema(), budget, cancelled)?;
    super::records::qualify(source.table.schema(), &[], budget, cancelled)?;
    let bound = bind::capture(image, source.table.schema(), requests, budget, cancelled)?;
    let validation = bound.validation_rows(budget, cancelled)?;
    // Only validate shape/carriers and reserve original metadata. This does
    // not build an AFTER image, stage values or retain placeholder rows.
    let encoded = super::records::encode(source.table.schema(), &validation, budget, cancelled)?;
    let _reservation = super::placement::reserve(image, &source, &encoded, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(bound)
}

fn finish(
    image: NativeImage,
    table: &str,
    requests: &[OmittedRowInsert],
    bound: bind::BoundRows,
    reading: Option<ClockReading>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeEditPlan, EditError> {
    let resolved = bound.resolve(reading, budget, cancelled)?;
    let scope = image.catalog().catalog().scope();
    let plan = image.plan_ordinary_row_inserts(table, &resolved, budget, cancelled)?;
    proof::prove(&plan, scope, table, requests, reading, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(plan)
}

fn qualify(
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    if schema.columns().len() != PROFILE_COLUMNS
        || schema.layout.max_columns != PROFILE_COLUMNS
        || schema.layout.variable_slots != 0
        || schema.layout.fixed_end != FIXED_ROW_BYTES
        || schema.layout.logical_indexes != 0
        || schema.layout.physical_indexes != 0
    {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    for (position, (column, (native, scalar, width))) in schema
        .columns()
        .iter()
        .zip([
            (INT32, ScalarType::Int32, size_of::<i32>()),
            (DATE_TIME, ScalarType::DateTime, size_of::<f64>()),
        ])
        .enumerate()
    {
        check(budget, cancelled)?;
        if column.number() != position
            || column.field.offset != position * size_of::<i32>()
            || column.complex_type_id().is_some()
            || column.native_type() != native
            || column.scalar_type() != Some(scalar)
            || !column.fixed()
            || column.flags() != FIXED_UPDATABLE_FLAGS
            || column.extended_flags() != 0
            || column.storage_length() != width
        {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
    }
    Ok(())
}

fn qualify_properties(
    schema: &TableSchema,
    properties: &TableProperties,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    if properties.groups().len() > 1 {
        return Err(EditError::Unsupported(WriteBlocker::Properties));
    }
    let date_name = schema.columns()[1].name();
    for group in properties.groups() {
        check(budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                group.name().len().max(date_name.len()) as u64,
            )
            .map_err(InspectError::from)?;
        if group.kind() != PropertyGroupKind::Column
            || !group.name().eq_ignore_ascii_case(date_name)
            || group.properties().len() != 1
        {
            return Err(EditError::Unsupported(WriteBlocker::Properties));
        }
        let property = &group.properties()[0];
        check(budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                property.name().len().max("DefaultValue".len()) as u64,
            )
            .map_err(InspectError::from)?;
        if !property.name().eq_ignore_ascii_case("DefaultValue")
            || property.native_type() != LONG_TEXT
            || property.flags() != DDL_PROPERTY
        {
            return Err(EditError::Unsupported(WriteBlocker::Properties));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
