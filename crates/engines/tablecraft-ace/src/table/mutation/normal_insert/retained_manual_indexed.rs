//! Ordinary retained presence with manual LONG indexes and existing page capacity.
mod proof;

#[cfg(test)]
mod tests;

use super::{incrementing, indexed, retained};
use crate::{
    EditError, InspectError, NativeEditPlan, NativeImage, OmittedRowInsert, RowInsert,
    WriteBlocker,
    format::{
        data_type::INT32,
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
    },
    header::check,
    inspect_table_indexes,
    table::mutation::{
        fixed_records::indexed::CheckedIndexes,
        insert_defaults,
        preserved_source::Source,
        record_insertion::{self, properties},
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

const LOCAL_INDEX_KIND_MAX: u8 = 1;

fn declarations(
    source: &Source,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Option<usize>>, EditError> {
    let schema = source.table.schema();
    if incrementing::mode::selected(schema, budget, cancelled)? {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    if schema.physical_layout().physical_indexes == 0 || source.indexes.indexes().is_empty() {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }

    for index in source.indexes.indexes() {
        check(budget, cancelled)?;
        let [key] = index.columns() else {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        };
        if index.native_type() > LOCAL_INDEX_KIND_MAX
            || index.foreign_reference().is_some()
            || index
                .constraints()
                .ok_or(InspectError::Unsupported)?
                .ignore_nulls()
        {
            return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
        }

        budget
            .charge(Resource::Steps, schema.columns().len() as u64)
            .map_err(InspectError::from)?;
        let field = schema
            .columns()
            .iter()
            .find(|field| field.number() == key.number())
            .ok_or(EditError::Preservation)?;
        if field.native_type() != INT32
            || field.scalar_type() != Some(ScalarType::Int32)
            || field.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
            || field.extended_flags() != 0
            || !field.fixed()
            || field.storage_length() != size_of::<i32>()
            || field.complex_type_id().is_some()
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }

    record_insertion::current::ordinary_variables(schema, budget, cancelled)
}

fn qualify<'image>(
    image: &'image NativeImage,
    source: &Source,
    table: &str,
    rows: &[RowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CheckedIndexes<'image>, EditError> {
    declarations(source, budget, cancelled)?;
    let (facts, _) =
        indexed::qualify_retained_manual(image, source, table, rows, budget, cancelled)?;
    Ok(facts)
}

impl NativeImage {
    /// Positively select manual retained LONG index metadata before consuming proof.
    /// This advisory result grants no source, default or insertion authority.
    ///
    /// # Errors
    /// Returns native inspection, cancellation, allocation and explicit resource
    /// failures. Unsupported metadata returns false without retrying source proof.
    pub fn select_native_retained_manual_indexed_insert_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        if !self.has_retained_table_source(table, budget, cancelled)? {
            return Ok(false);
        }
        let inspection = inspect_table_indexes(
            &mut self.reader(),
            self.catalog().catalog().scope(),
            table,
            budget,
            cancelled,
        )?;
        let schema = inspection.schema();
        if schema.columns().is_empty() || incrementing::mode::selected(schema, budget, cancelled)? {
            return Ok(false);
        }
        let mut selected = false;
        for index in inspection.indexes() {
            check(budget, cancelled)?;
            for key in index.columns() {
                budget
                    .charge(Resource::Steps, schema.columns().len() as u64)
                    .map_err(InspectError::from)?;
                if let Some(field) = schema
                    .columns()
                    .iter()
                    .find(|field| field.number() == key.number())
                {
                    selected |= field.native_type() == INT32
                        && field.scalar_type() == Some(ScalarType::Int32)
                        && field.flags() == (FIXED_FLAG | UPDATABLE_FLAG)
                        && field.extended_flags() == 0
                        && field.fixed()
                        && field.storage_length() == size_of::<i32>();
                }
            }
        }
        check(budget, cancelled)?;
        Ok(selected)
    }

    /// Check all source, program, ordinary record and physical index evidence.
    ///
    /// # Errors
    /// Refuses unsupported source, programs, declarations, row/index geometry or
    /// ownership, corruption, cancellation, allocation and resource exhaustion.
    pub fn check_native_retained_manual_indexed_insert_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let source = retained::capture(self, table, budget, cancelled)?;
        qualify(self, &source, table, &[], budget, cancelled)?;
        Ok(())
    }

    pub(crate) fn plan_retained_manual_indexed_omitted_row_inserts(
        self,
        table: &str,
        requests: &[OmittedRowInsert],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let _span = tracing::debug_span!("native.retained_manual_indexed_insert.plan").entered();
        let source = retained::capture(&self, table, budget, cancelled)?;
        let variables = declarations(&source, budget, cancelled)?;
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
        let facts = qualify(&self, &source, table, &rows, budget, cancelled)?;
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
        let after = indexed::stage_retained_manual(
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
