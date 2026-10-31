//! Normal positive-program ACE UPDATE, independent of engineering packaging.
mod mixed;
mod proof;
pub(in crate::table::mutation) mod protection;
mod stage;

use super::{EditError, Int32Update, NativeEditPlan, WriteBlocker, ace_profile, gates, rows};
use crate::{
    BeforeChangeFieldPreparation, BeforeEventGuardError, BeforeEventIntents, FixedScalarUpdate,
    InspectError, NativeImage, TableCheck, TableSchema, TableSnapshot, header::check,
    inspect_table_schema, read_table, table::indexes::mutation::normal_index_audit::Ownership,
};
use tablecraft_core::{
    ids::ObjectId,
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

impl NativeImage {
    /// Check native positive-BeforeChange compilation for normal Int32 editing.
    ///
    /// This reads the captured original only. It neither proves macro absence
    /// nor returns persistence authority; a complete original-intent plan and
    /// independent proof are required for every later commit and recovery.
    pub fn check_native_before_change_int32_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let _span = tracing::debug_span!("native.before_change.normal.table").entered();
        ace_profile::qualify(self)?;
        let schema = inspect_table_schema(
            &mut self.reader(),
            self.catalog().catalog().scope(),
            table,
            budget,
            cancelled,
        )?;
        if mixed::selected(&schema, budget, cancelled)? {
            return mixed::preflight(self, table, budget, cancelled);
        }
        let column = schema
            .columns()
            .first()
            .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
        prepare(
            self,
            schema.object_id(),
            column.name(),
            &[],
            budget,
            cancelled,
        )?;
        Ok(())
    }

    pub(crate) fn plan_before_change_int32_updates(
        self,
        table: &str,
        column: &str,
        updates: &[Int32Update],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        plan(self, table, column, updates, budget, cancelled)
    }
}

fn event_error(error: BeforeEventGuardError) -> EditError {
    match error {
        BeforeEventGuardError::Inspect(error) => error.into(),
        BeforeEventGuardError::Intent(error) => error,
        BeforeEventGuardError::Evaluation(error) => EditError::BeforeChangeEvaluation(error),
        BeforeEventGuardError::Preservation => EditError::Preservation,
        _ => EditError::Unsupported(WriteBlocker::ActiveContent),
    }
}

fn prepare<'image>(
    image: &'image NativeImage,
    object: ObjectId,
    column: &str,
    updates: &[Int32Update],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<BeforeChangeFieldPreparation<'image>, EditError> {
    budget
        .charge_allocation(updates.len() as u64, size_of::<FixedScalarUpdate>() as u64)
        .map_err(InspectError::from)?;
    let mut original = Vec::new();
    original
        .try_reserve_exact(updates.len())
        .map_err(|_| InspectError::Allocation)?;
    for update in updates {
        check(budget, cancelled)?;
        original.push(FixedScalarUpdate::new(
            update.row(),
            Value::Int32(update.expected()),
            Value::Int32(update.value()),
        ));
    }
    image
        .prepare_native_before_change_fields(
            object,
            BeforeEventIntents::FixedUpdate {
                column,
                updates: &original,
            },
            budget,
            cancelled,
        )
        .map_err(event_error)
}

/// Construction remains private to the original-image normal planner.
pub(super) struct Witness<'a> {
    address: usize,
    length: usize,
    object: ObjectId,
    column: &'a str,
}

impl Witness<'_> {
    pub(super) fn require(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        check(budget, cancelled)?;
        if image.as_bytes().as_ptr() as usize != self.address
            || image.as_bytes().len() != self.length
            || schema.object_id() != self.object
        {
            return Err(EditError::Preservation);
        }
        // Re-derive native references and compile all programs from the same
        // original image, never from cached final rows or serialized selectors.
        prepare(image, self.object, self.column, &[], budget, cancelled)?;
        Ok(())
    }
}

fn qualify(
    image: &NativeImage,
    table: &str,
    column: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableCheck, EditError> {
    ace_profile::qualify(image)?;
    let scope = image.catalog().catalog().scope();
    let schema = inspect_table_schema(&mut image.reader(), scope, table, budget, cancelled)?;
    qualify_schema(image, table, column, &schema, budget, cancelled)
}

fn qualify_schema(
    image: &NativeImage,
    table: &str,
    column: &str,
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableCheck, EditError> {
    let scope = image.catalog().catalog().scope();
    let witness = Witness {
        address: image.as_bytes().as_ptr() as usize,
        length: image.as_bytes().len(),
        object: schema.object_id(),
        column,
    };
    let checked = gates::before_native_fields(image, table, &witness, budget, cancelled)?;
    if !checked.indexes().is_empty() {
        Ownership::capture(image, &checked, budget, cancelled)?;
    }
    let constraints =
        crate::check_table_constraints(&mut image.reader(), scope, table, budget, cancelled)?;
    if !constraints.relationships().is_empty() {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    for (index, field) in checked.schema().columns().iter().enumerate() {
        for prior in &checked.schema().columns()[..index] {
            check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    field.name().len().max(prior.name().len()) as u64,
                )
                .map_err(InspectError::from)?;
            if field.name().eq_ignore_ascii_case(prior.name()) {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
        }
    }
    Ok(checked)
}

fn validate_intents(
    snapshot: &TableSnapshot,
    column: &str,
    updates: &[Int32Update],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    rows::unique(updates, budget, cancelled)?;
    let target = super::field::fixed_column(
        snapshot.schema(),
        column,
        ScalarType::Int32,
        size_of::<i32>(),
    )?;
    for update in updates {
        check(budget, cancelled)?;
        if snapshot
            .rows()
            .get(update.row())
            .and_then(|row| row.get(target))
            != Some(&Value::Int32(update.expected()))
        {
            return Err(EditError::Conflict);
        }
    }
    // Preserve supplied no-ops until AFTER required-program discovery/complete
    // preflight. Their native event timing is not inferred from logical models.
    for update in updates {
        check(budget, cancelled)?;
        if update.expected() == update.value() {
            return Err(EditError::Unsupported(WriteBlocker::EventTiming));
        }
    }
    Ok(())
}

pub(crate) fn plan(
    image: NativeImage,
    table: &str,
    column: &str,
    updates: &[Int32Update],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeEditPlan, EditError> {
    let _span = tracing::debug_span!("native.before_change.normal.plan").entered();
    ace_profile::qualify(&image)?;
    let schema = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?;
    if mixed::selected(&schema, budget, cancelled)? {
        return mixed::plan(image, table, column, updates, budget, cancelled);
    }
    let checked = qualify_schema(&image, table, column, &schema, budget, cancelled)?;
    let scope = image.catalog().catalog().scope();
    let snapshot = read_table(&mut image.reader(), scope, table, budget, cancelled)?;
    validate_intents(&snapshot, column, updates, budget, cancelled)?;
    let mut prepared = prepare(
        &image,
        checked.schema().object_id(),
        column,
        updates,
        budget,
        cancelled,
    )?;
    if let Some(rejection) = prepared.take_rejection() {
        return Err(EditError::BeforeChangeRejected(rejection));
    }
    let protection = protection::Protection::capture(&image, checked.schema(), budget, cancelled)?;
    let staged = stage::derive(
        &image,
        &checked,
        &snapshot,
        &prepared,
        &protection,
        budget,
        cancelled,
    )?;
    drop(prepared);
    let plan = image.finish_updates(
        table,
        &staged.patches,
        &staged.indexes,
        staged.changed_rows,
        budget,
        cancelled,
    )?;
    proof::prove(&plan, scope, table, column, updates, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(plan)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod evaluation_tests;

#[cfg(test)]
mod index_tests;

#[cfg(test)]
mod tree_tests;
