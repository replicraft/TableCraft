//! Ordinary fitting records use fresh native admission and original requests.
pub(in crate::table::mutation) mod binding;
mod external;
pub(in crate::table::mutation) mod indexed;
mod inline;
pub(in crate::table::mutation) mod owned_growth;
mod proof;
pub(in crate::table::mutation) mod stage;
pub(in crate::table::mutation) mod text;

use crate::{
    EditError, FileFormat, FittedColumnUpdates, InspectError, NativeColumn, NativeEditPlan,
    NativeImage, WriteBlocker,
    format::{
        data_type::{MAX_SHORT_BINARY_BYTES, MAX_SHORT_TEXT_UNITS, SHORT_BINARY, SHORT_TEXT},
        layout::{UNICODE_COMPRESSION_FLAG, UPDATABLE_FLAG},
        reader::Reader,
    },
    header::check,
    inspect_table_indexes, inspect_table_schema,
    table::{
        mutation::{
            normal_ordinary,
            preserved_source::{Source, validation},
            record_serialization,
        },
        record,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

impl NativeImage {
    /// Select a capability attempt from native declarations and index metadata.
    /// This reads no user records and never grants write permission.
    pub fn select_native_ordinary_fitted_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        let _span = tracing::debug_span!("native.ordinary.fitted.select").entered();
        check(budget, cancelled)?;
        if !matches!(
            self.catalog().header().format(),
            FileFormat::Ace2016 | FileFormat::Ace2019
        ) {
            return Ok(false);
        }
        let scope = self.catalog().catalog().scope();
        let schema = inspect_table_schema(&mut self.reader(), scope, table, budget, cancelled)?;
        let indexes = inspect_table_indexes(&mut self.reader(), scope, table, budget, cancelled)?;
        for field in schema.columns() {
            check(budget, cancelled)?;
            if !variable_shape(field) {
                continue;
            }
            let mut indexed = false;
            for index in indexes.indexes() {
                for key in index.columns() {
                    check(budget, cancelled)?;
                    indexed |= key.number() == field.number();
                }
            }
            if !indexed || matches!(field.native_type(), SHORT_BINARY | SHORT_TEXT) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Check actual ordinary unindexed Short Text/Binary capability without
    /// staging values, evaluating programs or retaining a permission token.
    /// Every later plan repeats native admission and original target binding.
    pub fn check_native_ordinary_fitted_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let _span = tracing::debug_span!("native.ordinary.fitted.table").entered();
        if indexed::capability_selected(self, table, budget, cancelled)? {
            return indexed::check_table(self, table, budget, cancelled);
        }
        if text::capability_selected(self, table, budget, cancelled)? {
            return text::check_table(self, table, budget, cancelled);
        }
        let source = normal_ordinary::capture(self, table, budget, cancelled)?;
        for (column, field) in source.table.schema().columns().iter().enumerate() {
            check(budget, cancelled)?;
            if !variable_shape(field) {
                continue;
            }
            let mut blocked = false;
            for dependency in &source.dependencies {
                check(budget, cancelled)?;
                blocked |= *dependency == column;
            }
            for index in source.indexes.indexes() {
                for key in index.columns() {
                    check(budget, cancelled)?;
                    blocked |= key.number() == field.number();
                }
            }
            if blocked {
                continue;
            }
            let groups = [FittedColumnUpdates::Variable {
                column: field.name(),
                updates: &[],
            }];
            binding::all(&source, &groups, budget, cancelled)?;
            return Ok(());
        }
        Err(EditError::Unsupported(WriteBlocker::Column))
    }

    pub(crate) fn plan_ordinary_fitted_updates(
        self,
        table: &str,
        groups: &[FittedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let _span = tracing::debug_span!("native.ordinary.fitted.plan").entered();
        check(budget, cancelled)?;
        if indexed::selected(&self, table, groups, budget, cancelled)? {
            return indexed::plan(self, table, groups, budget, cancelled);
        }
        if text::selected(&self, table, groups, budget, cancelled)? {
            return text::plan(self, table, groups, budget, cancelled);
        }
        let source = normal_ordinary::capture(&self, table, budget, cancelled)?;
        let (patches, changed_rows) = stage::prepare(&self, &source, groups, budget, cancelled)?;
        let mut after = record_serialization::copy(self.as_bytes(), budget, cancelled)?;
        for patch in patches {
            check(budget, cancelled)?;
            let end = patch
                .offset
                .checked_add(patch.before.len())
                .ok_or(EditError::Preservation)?;
            let destination = after
                .get_mut(patch.offset..end)
                .ok_or(EditError::Preservation)?;
            budget
                .charge(Resource::Steps, patch.before.len() as u64)
                .map_err(InspectError::from)?;
            if destination != patch.before.as_slice() || patch.before.len() != patch.after.len() {
                return Err(EditError::Preservation);
            }
            destination.copy_from_slice(&patch.after);
        }
        validation::expected(&self, table, &source.table, &after, budget, cancelled)?;
        let scope = self.catalog().catalog().scope();
        let plan = NativeEditPlan {
            before: self.bytes,
            after,
            changed_rows,
        };
        proof::prove(&plan, scope, table, groups, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(plan)
    }
}

/// The immutable Source already owns exclusive resolved roots. Verify its exact
/// raw record still belongs to this image before decoding selected real cells.
pub(in crate::table::mutation) fn original_values(
    image: &NativeImage,
    source: &Source,
    ordinal: usize,
    columns: &[usize],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Value>, EditError> {
    check(budget, cancelled)?;
    let original = source.records.get(ordinal).ok_or(EditError::Conflict)?;
    let end = original
        .offset
        .checked_add(original.data.len())
        .ok_or(EditError::Preservation)?;
    budget
        .charge(Resource::Steps, original.data.len() as u64)
        .map_err(InspectError::from)?;
    if image.as_bytes().get(original.offset..end) != Some(original.data.as_slice()) {
        return Err(EditError::Preservation);
    }
    let mut bytes = image.reader();
    let mut reader = Reader {
        source: &mut bytes,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    Ok(record::decode_selected(
        &original.data,
        source.table.schema(),
        Some(columns),
        &mut reader,
    )?)
}

fn variable_shape(field: &NativeColumn) -> bool {
    !field.fixed()
        && field.flags() == UPDATABLE_FLAG
        && field.storage_length() != 0
        && match field.native_type() {
            SHORT_TEXT => {
                field.extended_flags() & !UNICODE_COMPRESSION_FLAG == 0
                    && field.storage_length().is_multiple_of(size_of::<u16>())
                    && field.storage_length() / size_of::<u16>() <= MAX_SHORT_TEXT_UNITS
            }
            SHORT_BINARY => {
                field.extended_flags() == 0 && field.storage_length() <= MAX_SHORT_BINARY_BYTES
            }
            _ => false,
        }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod sentinel_source_controls;

#[cfg(all(test, feature = "journal"))]
mod binary_update_sources;

#[cfg(test)]
mod binary_tests;
