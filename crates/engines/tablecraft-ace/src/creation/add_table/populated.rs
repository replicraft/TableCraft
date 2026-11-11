//! One populated table candidate over its complete immutable original image.
mod packing;
pub(in crate::creation::add_table) mod proof;
pub(in crate::creation::add_table) mod rows;

use super::{AddTableError, AddTableSourceProfile, native, retained};
use crate::{
    CreateDatabaseError, FirstTableDefinition, InspectError, NativeImage, NativeImageSource,
    creation::{
        input::{Columns, Rows},
        request,
    },
    header::check,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use thiserror::Error;

/// Refusal before a materialized relation becomes a native populated-table candidate.
#[derive(Debug, Error)]
pub enum PopulatedAddTableError {
    /// The request is not a native manual declaration without inherited properties.
    #[error("populated creation requires an explicit unadorned manual declaration")]
    Declaration,
    /// Original storage is outside the positively qualified retained append family.
    #[error("source is outside the populated append family")]
    Source,
    /// Exact scalar carrier, native declaration or physical record failure.
    #[error(transparent)]
    Create(#[from] CreateDatabaseError),
    /// Native inspection, allocation, caller resource or cancellation failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Append ownership, destination-name, allocation or preservation refusal.
    #[error(transparent)]
    Append(#[from] AddTableError),
}

/// A sealed complete AFTER candidate, not journal preparation or save authority.
/// The original image binds declarations and materialized rows to one native
/// reconstruction. No SQL, query source or external callback is executed here.
#[must_use = "retain the complete candidate or explicitly discard it"]
pub struct PopulatedAddTablePlan {
    before: NativeImage,
    after: NativeImageSource,
    definition: FirstTableDefinition,
    rows: Vec<Vec<Value>>,
}

impl NativeImage {
    /// Construct one complete keyless populated table at fresh EOF roles while
    /// preserving every original byte outside qualified catalog/map allocation.
    /// Rows must supply every exact scalar value; NULL never invokes a default.
    /// DATA packing uses actual record sizes and representable physical slots,
    /// with no aggregate row ceiling beyond native geometry and caller budgets.
    ///
    /// This candidate does not grant filesystem, journal or recovery authority.
    /// Old empty-table intent families remain unchanged.
    ///
    /// # Errors
    /// Refuses unsupported original storage, inherited properties/generation/keys,
    /// wrong carriers, oversized individual records, conflicting names, failed
    /// preservation proof, cancellation, allocation and resource exhaustion.
    pub fn plan_populated_add_table(
        self,
        definition: FirstTableDefinition,
        rows: Vec<Vec<Value>>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PopulatedAddTablePlan, PopulatedAddTableError> {
        let _span = tracing::debug_span!("native.populated_add_table.plan").entered();
        check(budget, cancelled)?;
        if !definition.is_native()
            || !definition.is_manual()
            || definition.primary_position().is_some()
        {
            return Err(PopulatedAddTableError::Declaration);
        }
        if !retained::selected(&self, budget, cancelled)? {
            return Err(PopulatedAddTableError::Source);
        }
        let (after, profile) = {
            let columns = definition.borrowed_columns(budget, cancelled)?;
            for column in &columns {
                check(budget, cancelled)?;
                if column.incrementing_autonumber()
                    || column.primary_key_name().is_some()
                    || column.required().is_some()
                    || column.default_literal().is_some()
                    || column.allow_zero_length().is_some()
                    || column.description().is_some()
                    || column.unicode_compression()
                {
                    return Err(PopulatedAddTableError::Declaration);
                }
            }
            let metadata = request::validate_native(
                definition.table(),
                Columns::Values(&columns),
                Rows::Values(&[]),
                budget,
                cancelled,
            )?;
            budget
                .charge_allocation(rows.len() as u64, size_of::<&[Value]>() as u64)
                .map_err(InspectError::from)?;
            let mut borrowed = Vec::new();
            borrowed
                .try_reserve_exact(rows.len())
                .map_err(|_| InspectError::Allocation)?;
            for row in &rows {
                check(budget, cancelled)?;
                borrowed.push(row.as_slice());
            }
            let prepared = self::rows::Prepared::capture(
                Columns::Values(&columns),
                Rows::Values(&borrowed),
                metadata.fields(),
                budget,
                cancelled,
            )?;
            native::build(&self, &definition, Some(&prepared), budget, cancelled)?
        };
        if profile != AddTableSourceProfile::Retained {
            return Err(PopulatedAddTableError::Source);
        }
        check(budget, cancelled)?;
        Ok(PopulatedAddTablePlan {
            before: self,
            after,
            definition,
            rows,
        })
    }
}

impl PopulatedAddTablePlan {
    /// Complete immutable original to which the candidate is bound.
    pub fn before_bytes(&self) -> &[u8] {
        self.before.as_bytes()
    }
    /// Complete independently checked table and row image, without save permission.
    pub fn after_bytes(&self) -> &[u8] {
        self.after.as_bytes()
    }
    /// Requested destination name, never a replacement target.
    pub fn table(&self) -> &str {
        self.definition.table()
    }
    /// Exact selected scalar row count, including zero.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }
    /// Explicit scalar declaration count; no primary or generated field is added.
    pub fn column_count(&self) -> usize {
        self.definition.column_count()
    }
}

#[cfg(test)]
mod tests;
