//! Fresh ordinary rows over retained schema history, without generation or indexes.
mod proof;
#[cfg(test)]
mod tests;

use super::{incrementing, indexed, retained};
use crate::{
    EditError, InspectError, NativeEditPlan, NativeImage, OmittedRowInsert, WriteBlocker,
    header::check,
    inspect_table_schema,
    table::mutation::{
        insert_defaults,
        preserved_source::Source,
        record_insertion::{self, properties},
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

fn qualify(
    image: &NativeImage,
    source: &Source,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Option<usize>>, EditError> {
    let schema = source.table.schema();
    if incrementing::mode::selected(schema, budget, cancelled)? {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    if schema.physical_layout().physical_indexes != 0
        || schema.physical_layout().logical_indexes != 0
        || !source.indexes.indexes().is_empty()
    {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let variables = record_insertion::current::ordinary_variables(schema, budget, cancelled)?;
    indexed::qualify(
        image,
        source,
        table,
        &[],
        indexed::LvalScope::Retained,
        budget,
        cancelled,
    )?;
    Ok(variables)
}

impl NativeImage {
    /// Select retained no-generation/no-index metadata before any consuming plan.
    /// A positive result is advisory and does not grant source or write authority.
    ///
    /// # Errors
    /// Native corruption, cancellation, allocation and explicit budget failures
    /// retain their typed causes; no failed source proof is retried.
    pub fn select_native_retained_keyless_insert_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        if !self.has_retained_table_source(table, budget, cancelled)? {
            return Ok(false);
        }
        let schema = inspect_table_schema(
            &mut self.reader(),
            self.catalog().catalog().scope(),
            table,
            budget,
            cancelled,
        )?;
        Ok(!schema.columns().is_empty()
            && schema.physical_layout().physical_indexes == 0
            && schema.physical_layout().logical_indexes == 0
            && !incrementing::mode::selected(&schema, budget, cancelled)?)
    }

    /// Fully qualify an ordinary retained INSERT source, including empty requests.
    /// All active fields must be non-generated and the table must have no indexes.
    ///
    /// # Errors
    /// Refuses unsupported source, declaration, row geometry or property evidence,
    /// corruption, cancellation, allocation failure and explicit budget exhaustion.
    pub fn check_native_retained_keyless_insert_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let source = retained::capture(self, table, budget, cancelled)?;
        qualify(self, &source, table, budget, cancelled)?;
        Ok(())
    }

    pub(crate) fn plan_retained_keyless_omitted_row_inserts(
        self,
        table: &str,
        requests: &[OmittedRowInsert],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let _span = tracing::debug_span!("native.retained_keyless_insert.plan").entered();
        let source = retained::capture(&self, table, budget, cancelled)?;
        let variables = qualify(&self, &source, table, budget, cancelled)?;
        let rows = insert_defaults::prepare_retained(
            &self,
            source.table.schema(),
            requests,
            budget,
            cancelled,
        )?;
        properties::require_text_declarations(
            &self,
            source.table.schema(),
            &rows,
            budget,
            cancelled,
        )?;
        let (facts, optional) = indexed::qualify(
            &self,
            &source,
            table,
            &rows,
            indexed::LvalScope::Retained,
            budget,
            cancelled,
        )?;
        if optional {
            return Err(EditError::Preservation);
        }
        let mut records = incrementing::vector(rows.len(), budget, cancelled)?;
        for row in &rows {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Rows, 1)
                .map_err(InspectError::from)?;
            records.push(record_insertion::current::build(
                source.table.schema(),
                &variables,
                row.values(),
                budget,
                cancelled,
            )?);
        }
        let after = indexed::stage(
            &self,
            &source,
            &rows,
            &records,
            indexed::StageIndexes {
                facts: &facts,
                scope: indexed::LvalScope::Retained,
            },
            budget,
            cancelled,
        )?;
        drop(facts);
        let scope = self.catalog().catalog().scope();
        let plan = NativeEditPlan {
            before: self.bytes,
            after,
            changed_rows: requests.len(),
        };
        proof::prove(&plan, scope, table, requests, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(plan)
    }
}
