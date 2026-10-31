//! Normal positive-native-program writes over an actual preserved projection.
mod proof;
mod stage;

use super::super::{
    EditError, Int32Update, NativeEditPlan, WriteBlocker, ace_profile, field,
    guards::{buffer, fields::batch, native_events},
    preserved_source::{self, validation},
    rows,
};
use super::event_error;
use crate::table::mutation::requests::FixedColumnUpdates;
use crate::{
    FixedScalarUpdate, NativeImage, TableSchema,
    format::{
        data_type::INT32,
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
    },
    header::check,
    inspect_table_schema,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::ScalarType;

/// Metadata chooses the implementation before qualification. An unsuccessful
/// ordinary plan is never retried through this route.
pub(super) fn selected(
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    for column in schema.columns() {
        check(budget, cancelled)?;
        if column.native_type() != INT32
            || column.scalar_type() != Some(ScalarType::Int32)
            || column.extended_flags() != 0
            || column.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
            || !column.fixed()
            || column.storage_length() != size_of::<i32>()
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn prepare<'image>(
    image: &'image NativeImage,
    source: &preserved_source::Source,
    column: &str,
    updates: &[Int32Update],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<batch::Prepared<'image>, EditError> {
    let selected = native_events::programs(image, source.table.schema(), budget, cancelled)
        .map_err(event_error)?;
    let mut original = buffer::vector(updates.len(), budget, cancelled)?;
    for update in updates {
        check(budget, cancelled)?;
        original.push(FixedScalarUpdate::new(
            update.row(),
            Value::Int32(update.expected()),
            Value::Int32(update.value()),
        ));
    }
    batch::prepare(
        image,
        batch::SourceFacts {
            table: &source.table,
            indexes: &source.indexes,
            dependencies: &source.dependencies,
        },
        selected,
        &[FixedColumnUpdates::new(
            source.table.schema().name(),
            column,
            &original,
        )],
        budget,
        cancelled,
    )
    .map_err(event_error)
}

pub(super) fn capture(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<preserved_source::Source, EditError> {
    ace_profile::qualify(image)?;
    let schema = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?;
    let selected =
        native_events::programs(image, &schema, budget, cancelled).map_err(event_error)?;
    preserved_source::capture_native_before_change(image, table, &selected, budget, cancelled)
}

pub(super) fn validate(
    source: &preserved_source::Source,
    column: &str,
    updates: &[Int32Update],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    rows::unique(updates, budget, cancelled)?;
    let native = field::fixed_column(
        source.table.schema(),
        column,
        ScalarType::Int32,
        size_of::<i32>(),
    )?;
    stage::target(source, native, budget, cancelled)?;
    let position = source
        .table
        .columns()
        .iter()
        .position(|field| field.name().eq_ignore_ascii_case(column))
        .ok_or(EditError::Preservation)?;
    for update in updates {
        check(budget, cancelled)?;
        if source
            .table
            .rows()
            .get(update.row())
            .and_then(|row| row.get(position))
            != Some(&Value::Int32(update.expected()))
        {
            return Err(EditError::Conflict);
        }
    }
    // Discovery and complete dormant compilation precede uncertain timing
    // refusal, even when the caller supplied only equal assignments.
    for update in updates {
        check(budget, cancelled)?;
        if update.expected() == update.value() {
            return Err(EditError::Unsupported(WriteBlocker::EventTiming));
        }
    }
    Ok(())
}

pub(super) fn plan(
    image: NativeImage,
    table: &str,
    column: &str,
    updates: &[Int32Update],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeEditPlan, EditError> {
    let _span = tracing::debug_span!("native.before_change.normal.mixed.plan").entered();
    let source = capture(&image, table, budget, cancelled)?;
    // Compile before intent/timing refusal. No row executes for this preflight.
    prepare(&image, &source, column, &[], budget, cancelled)?;
    validate(&source, column, updates, budget, cancelled)?;
    let mut prepared = prepare(&image, &source, column, updates, budget, cancelled)?;
    if let Some(rejection) = prepared.take_rejection() {
        return Err(EditError::BeforeChangeRejected(rejection));
    }
    let staged = stage::derive(&image, &source, &prepared, budget, cancelled)?;
    drop(prepared);
    validation::expected(
        &image,
        table,
        &source.table,
        &staged.bytes,
        budget,
        cancelled,
    )?;
    let scope = image.catalog().catalog().scope();
    let plan = NativeEditPlan {
        before: image.bytes,
        after: staged.bytes,
        changed_rows: staged.changed_rows,
    };
    proof::prove(&plan, scope, table, column, updates, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(plan)
}

/// Actual Source admission plus complete dormant compilation, without executing event rows.
/// Pick a qualified writable target; the first field may be a readonly counter.
pub(super) fn preflight(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let source = capture(image, table, budget, cancelled)?;
    for (position, column) in source.table.schema().columns().iter().enumerate() {
        check(budget, cancelled)?;
        match stage::target(&source, position, budget, cancelled) {
            Ok(()) => {
                prepare(image, &source, column.name(), &[], budget, cancelled)?;
                return Ok(());
            }
            Err(EditError::Unsupported(WriteBlocker::Column | WriteBlocker::IndexedColumn)) => {}
            Err(error) => return Err(error),
        }
    }
    Err(EditError::Unsupported(WriteBlocker::Column))
}

#[cfg(test)]
mod proof_tests;
