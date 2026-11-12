//! Metadata selects the retained indexed LONG language before consuming proof.
use super::{Family, Source};
use crate::format::{
    data_type::INT32,
    layout::{FIXED_FLAG, UPDATABLE_FLAG},
};
use crate::table::{
    indexes::mutation::normal_index_audit::Ownership,
    mutation::fixed_records::indexed::retained_long::RetainedLongIndexes, record_layout,
};
use crate::{
    EditError, IndexInspection, InspectError, NativeImage, WriteBlocker, header::check,
    inspect_table_indexes,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

const LOCAL_INDEX_KIND_MAX: u8 = 1;

fn selected(
    inspection: &IndexInspection,
    column: Option<&str>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<usize>, EditError> {
    let mut result = None;
    for (position, field) in inspection.schema().columns().iter().enumerate() {
        check(budget, cancelled)?;
        if let Some(column) = column {
            budget
                .charge(Resource::Steps, field.name().len().max(column.len()) as u64)
                .map_err(InspectError::from)?;
            if !field.name().eq_ignore_ascii_case(column) {
                continue;
            }
        }
        if field.native_type() != INT32
            || field.scalar_type() != Some(ScalarType::Int32)
            || !field.fixed()
            || field.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
            || field.extended_flags() != 0
            || field.storage_length() != size_of::<i32>()
            || field.complex_type_id().is_some()
        {
            continue;
        }
        let mut indexed = false;
        for index in inspection.indexes() {
            for key in index.columns() {
                check(budget, cancelled)?;
                indexed |= key.number() == field.number();
            }
        }
        if indexed {
            if column.is_some() && result.is_some() {
                return Err(EditError::Preservation);
            }
            if result.is_none() {
                result = Some(position);
            }
        }
    }
    Ok(result)
}

impl NativeImage {
    /// Metadata-only rescue for all-indexed tables; consuming proof remains required.
    pub fn select_native_retained_indexed_long_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        self.select_retained_indexed_long(table, None, budget, cancelled)
    }

    /// Metadata-only original field selection, including a zero-match request.
    pub fn select_native_retained_indexed_long_column(
        &self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        self.select_retained_indexed_long(table, Some(column), budget, cancelled)
    }

    fn select_retained_indexed_long(
        &self,
        table: &str,
        column: Option<&str>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        check(budget, cancelled)?;
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
        if column.is_none() {
            // Rescue all-indexed tables without preempting the existing coarse
            // capability for mixed tables and their historical sparse records.
            for field in inspection.schema().columns() {
                let mut indexed = false;
                for index in inspection.indexes() {
                    for key in index.columns() {
                        check(budget, cancelled)?;
                        indexed |= key.number() == field.number();
                    }
                }
                if !indexed {
                    return Ok(false);
                }
            }
        }
        Ok(selected(&inspection, column, budget, cancelled)?.is_some())
    }

    /// Qualify the original retained source and every physical owner without effects.
    pub fn check_native_retained_indexed_long_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let source = capture(self, table, budget, cancelled)?;
        let position = selected(&source.facts().indexes, None, budget, cancelled)?
            .ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
        for record in &source.facts().records {
            Family::Retained.qualify_record(
                source.facts().table.schema(),
                position,
                &record.data,
                budget,
                cancelled,
            )?;
        }
        let facts = RetainedLongIndexes::capture(self, table, source.facts(), budget, cancelled)?;
        let _ownership =
            Ownership::capture_retained_long(self, facts.checked(), budget, cancelled)?;
        Ok(())
    }
}

pub(super) fn capture<'image>(
    image: &'image NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source<'image>, EditError> {
    let source = Family::Retained.capture(image, table, budget, cancelled)?;
    let schema = source.facts().table.schema();
    for index in source.facts().indexes.indexes() {
        check(budget, cancelled)?;
        let constraints = index
            .constraints()
            .ok_or(EditError::Unsupported(WriteBlocker::ConstraintCoverage))?;
        let [key] = index.columns() else {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        };
        if index.native_type() > LOCAL_INDEX_KIND_MAX
            || index.foreign_reference().is_some()
            || constraints.ignore_nulls()
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
            || !field.fixed()
            || field.storage_length() != size_of::<i32>()
            || field.extended_flags() != 0
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }
    if selected(&source.facts().indexes, None, budget, cancelled)?.is_none() {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    // Existing independent projected INDEX reconstruction owns complete records,
    // not historical absent-field materialization. Keep that source boundary exact.
    for record in &source.facts().records {
        check(budget, cancelled)?;
        if record_layout::parse(&record.data, schema, budget)?.count != schema.layout.max_columns {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
    }
    Ok(source)
}

pub(super) fn column(
    source: &Source<'_>,
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    selected(&source.facts().indexes, Some(name), budget, cancelled)?
        .ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))
}
