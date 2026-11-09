//! A separate candidate language for nullable LONG with explicit DefaultValue 0.
//! TCAC0001/0002 source languages and normal save APIs are unchanged.
mod build;
mod repeated;
pub use repeated::RepeatedDefaultZeroAddColumnCandidate;
#[cfg(test)]
mod tests;
mod verify;

use super::super::{AddColumnCandidate, common, owned};
use super::CreatedAddColumnError;
use crate::{
    CreateColumn, CreateDatabaseError, EditError, InspectError, NativeImage, create_ace16_database,
    header::check,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::ScalarType;
use thiserror::Error;

/// Exact persisted Required declaration for the newly appended nullable field.
/// Absence and an explicit false property are distinct native metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreatedDefaultZeroColumnRequired {
    /// Do not introduce a Required property.
    Absent,
    /// Persist Required as the native Boolean false property.
    StoredFalse,
}

/// Refusal from the explicit first external-property candidate profile.
#[derive(Debug, Error)]
pub enum CreatedDefaultZeroAddColumnError {
    /// Complete canonical generated-primary source proof failed.
    #[error(transparent)]
    Created(#[from] CreatedAddColumnError),
    /// Inspection, cancellation or cumulative resource limit.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Constructive declaration cannot fit or is invalid.
    #[error(transparent)]
    Declaration(#[from] CreateDatabaseError),
    /// Existing checked record placement or represented allocation refused.
    #[error(transparent)]
    Placement(#[from] EditError),
    /// Existing properties are outside the independently qualified dictionary.
    #[error("source is outside the explicit default-zero property profile")]
    Source,
    /// The candidate failed complete complement or reopened semantic proof.
    #[error("default-zero add-column preservation check failed")]
    Preservation,
}

/// Candidate only: no intent, journal, adapter or GUI permission is supplied.
pub struct CreatedDefaultZeroAddColumnCandidate {
    image: AddColumnCandidate,
    required: CreatedDefaultZeroColumnRequired,
}

impl NativeImage {
    /// Inspect the complete source profile for an explicit Number declaration.
    ///
    /// This name-independent check constructs no candidate or write capability.
    /// It repeats the same canonical image and property checks used by planning;
    /// an actual review must still qualify its source and proposed field afresh.
    pub fn check_created_default_zero_add_column_source(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), CreatedDefaultZeroAddColumnError> {
        let _span = tracing::debug_span!("native.default_zero_add_column.check_source").entered();
        let (_, definition) = super::source::capture(self, table, budget, cancelled)?;
        let columns = definition.borrowed_columns(budget, cancelled)?;
        source_properties(&columns, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(())
    }

    /// Add a nullable ordinary LONG with explicit constant DefaultValue 0.
    ///
    /// Requires the complete canonical empty generated-primary creator image,
    /// zero timestamps, one through fifteen source fields, and either absent
    /// properties or only constant zero defaults and explicit Required false.
    /// Appends one represented EOF LVAL page and preserves catalog root identity through checked same-page
    /// forwarding when necessary. Existing external properties, broader property
    /// dictionaries, native histories and repeated external-property ADD refuse.
    /// The complete target property body must prefer one external row (65 through
    /// 4076 bytes); shorter inline targets and multi-row chains are not inferred.
    /// This separate candidate does not widen either existing no-default profile.
    pub fn prepare_created_add_default_zero_int32_column_candidate(
        self,
        table: &str,
        column: &str,
        required: CreatedDefaultZeroColumnRequired,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<CreatedDefaultZeroAddColumnCandidate, CreatedDefaultZeroAddColumnError> {
        let _span =
            tracing::debug_span!("native.created_add_default_zero.prepare_candidate").entered();
        check(budget, cancelled)?;
        super::super::super::request::name(column, budget, cancelled)?;
        let (checked, definition) = super::source::capture(&self, table, budget, cancelled)?;
        let mut columns = definition.borrowed_columns(budget, cancelled)?;
        let default_first = source_properties(&columns, budget, cancelled)?;
        budget
            .charge_allocation(1, size_of::<CreateColumn<'_>>() as u64)
            .map_err(InspectError::from)?;
        columns
            .try_reserve_exact(1)
            .map_err(|_| InspectError::Allocation)?;

        let zero = Value::Int32(0);
        let mut appended =
            CreateColumn::new(column, ScalarType::Int32).with_default_literal(&zero)?;
        if required == CreatedDefaultZeroColumnRequired::StoredFalse {
            appended = appended.with_required(false);
        }
        columns.push(appended);

        let rebuilt =
            create_ace16_database(checked.schema().name(), &columns, &[], budget, cancelled)?;
        let intermediate = common::copy_definition(
            &self,
            rebuilt.as_bytes(),
            checked.schema().definition_page() as usize * crate::format::page::PAGE_BYTES,
            budget,
            cancelled,
        )?;
        // Existing profile's complete TD/index complement check applies before
        // the separate catalog/LVAL edit. Its source and wire remain unchanged.
        super::preservation::verify(&self, &intermediate, &checked, column, budget, cancelled)?;
        let body = build::properties(&columns, default_first, budget, cancelled)?;
        let (after, change) =
            build::candidate(&self, &checked, intermediate, body, budget, cancelled)?;
        verify::candidate(&self, &after, &checked, column, &change, budget, cancelled)?;
        let table = owned(checked.schema().name(), budget)?;
        let column = owned(column, budget)?;
        let count = columns.len();
        drop(columns);
        check(budget, cancelled)?;
        Ok(CreatedDefaultZeroAddColumnCandidate {
            required,
            image: AddColumnCandidate {
                before: self,
                after,
                table,
                column,
                columns: count,
            },
        })
    }
}

impl CreatedDefaultZeroAddColumnCandidate {
    /// Exact requested Required-property presence, retained for original intent.
    pub const fn required_property(&self) -> CreatedDefaultZeroColumnRequired {
        self.required
    }

    /// Complete unchanged captured original.
    pub fn before_bytes(&self) -> &[u8] {
        self.image.before_bytes()
    }

    /// Complete candidate, including its single appended LVAL page.
    pub fn after_bytes(&self) -> &[u8] {
        self.image.after_bytes()
    }

    /// Source-resolved table spelling.
    pub fn table(&self) -> &str {
        self.image.table()
    }

    /// Explicit proposed field spelling.
    pub fn column(&self) -> &str {
        self.image.column()
    }

    /// Complete field count after this candidate.
    pub const fn column_count(&self) -> usize {
        self.image.column_count()
    }
}

/// Retain exact source policy presence and determine the existing dictionary order.
fn source_properties(
    columns: &[CreateColumn<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, CreatedDefaultZeroAddColumnError> {
    let mut default_first = true;
    for field in columns {
        check(budget, cancelled)?;
        if field.required() == Some(true)
            || field.allow_zero_length().is_some()
            || field.description().is_some()
            || field
                .default_literal()
                .is_some_and(|value| value != &Value::Int32(0))
        {
            return Err(CreatedDefaultZeroAddColumnError::Source);
        }
        default_first &= field.required().is_none();
    }
    Ok(default_first)
}
