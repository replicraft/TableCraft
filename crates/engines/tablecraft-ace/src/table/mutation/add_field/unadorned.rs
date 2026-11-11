//! Unadorned scalar ADD preserves qualified source metadata and original records.
mod proof;
#[cfg(test)]
mod tests;

use super::{
    CurrentFieldAdditionError as Error, creation_error, definition::Plan, unadorned_source,
};
use crate::{
    InspectError, NativeImage, NativeSortOrder, UnadornedFieldAdditionDefinition, header::check,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

/// Independently checked metadata-only scalar addition, without save authority.
pub struct NativeUnadornedFieldAdditionCandidate {
    source: unadorned_source::Source,
    after: Vec<u8>,
    definition: UnadornedFieldAdditionDefinition,
    native_table: i32,
    native_number: usize,
    column_id: u16,
}

impl NativeUnadornedFieldAdditionCandidate {
    /// Complete immutable original; no physical scope is inferred from this slice.
    pub fn before_bytes(&self) -> &[u8] {
        self.source.proof().image().as_bytes()
    }
    /// Complete independently checked replacement, not a durable plan.
    pub fn after_bytes(&self) -> &[u8] {
        &self.after
    }
    /// Actual original table spelling.
    pub fn table(&self) -> &str {
        self.source.proof().table().schema.name()
    }
    /// Exact requested new field spelling.
    pub fn column(&self) -> &str {
        self.definition.name()
    }
    /// Exact unadorned original field request.
    pub fn definition(&self) -> &UnadornedFieldAdditionDefinition {
        &self.definition
    }
    /// Original native table identity, separate from caller catalog scope.
    pub const fn native_table_id(&self) -> i32 {
        self.native_table
    }
    /// Original target definition root.
    pub fn definition_page(&self) -> u32 {
        self.source.proof().table().schema.definition_page()
    }
    /// New native ordinal allocated from the retained high-water count.
    pub const fn native_number(&self) -> usize {
        self.native_number
    }
    /// Actual stored creation-time field identity.
    pub const fn column_id(&self) -> u16 {
        self.column_id
    }
    /// Replacement active-field count, separate from the native high-water count.
    pub fn column_count(&self) -> usize {
        self.source.proof().table().schema.columns().len() + 1
    }
}

impl NativeImage {
    /// Append one explicit ordinary scalar field to a qualified retained source.
    /// Required, default and other property absence remain unchanged; existing
    /// records are not rewritten or backfilled. Target field names must currently
    /// be ASCII because native Unicode identifier equivalence is unqualified;
    /// this does not restrict table names or unrelated objects. This returns no
    /// persistence authority.
    ///
    /// # Errors
    /// Refuses unsupported source/index/program/dependency facts, conflicting names,
    /// actual definition/ordinal capacity, malformed input, preservation failure,
    /// allocation, cancellation and cumulative caller limits.
    pub fn prepare_retained_unadorned_field_addition(
        self,
        table: &str,
        definition: UnadornedFieldAdditionDefinition,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeUnadornedFieldAdditionCandidate, Error> {
        let _span = tracing::debug_span!("native.field_addition.prepare_declared").entered();
        let source = unadorned_source::capture(self, table, definition.name(), budget, cancelled)?;
        let order = text_order(&source, &definition, budget, cancelled)?;
        let current = source.proof().table();
        let plan = Plan::for_field(
            source.proof().image(),
            &current.schema,
            definition.field(0).map_err(creation_error)?,
            budget,
            cancelled,
        )?;
        if definition.name().len() > plan.maximum_name_units {
            return Err(Error::Capacity);
        }
        let field = definition
            .field(plan.fixed_offset)
            .map_err(creation_error)?;
        let after = plan.build_field_with_order(
            source.proof().image(),
            definition.name(),
            field,
            order,
            budget,
            cancelled,
        )?;
        proof::candidate(&source, &plan, &definition, &after, budget, cancelled)?;
        let mut native_table = None;
        for origin in source.proof().image().catalog().origins() {
            check(budget, cancelled)?;
            if origin.object_id() == current.schema.object_id()
                && native_table.replace(origin.native_id()).is_some()
            {
                return Err(InspectError::Malformed.into());
            }
        }
        Ok(NativeUnadornedFieldAdditionCandidate {
            native_table: native_table.ok_or(InspectError::Malformed)?,
            native_number: plan.number,
            column_id: plan.column_id,
            source,
            after,
            definition,
        })
    }
}

fn text_order(
    source: &unadorned_source::Source,
    definition: &UnadornedFieldAdditionDefinition,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<NativeSortOrder>, Error> {
    if definition.column().scalar_type() != ScalarType::Text {
        return Ok(None);
    }
    let order = crate::inspect_database_sort_order(
        &mut source.proof().image().reader(),
        budget,
        cancelled,
    )?;
    if order.general_family().is_none() {
        return Err(Error::Source);
    }
    Ok(Some(order))
}
