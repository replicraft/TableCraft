//! Unadorned Currency ADD changes only an actual qualified keyless definition.
mod proof;
#[cfg(test)]
mod tests;

use super::{CurrentFieldAdditionError as Error, creation_error, definition::Plan};
use crate::{
    DeclaredFieldAdditionDefinition, InspectError, NativeImage, NativeImageSource,
    PropertyGroupKind,
    format::layout::{FIXED_FLAG, UPDATABLE_FLAG},
    header::check,
    table::mutation::current_schema::{self, Family},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

/// Independently checked metadata-only Currency addition, without save authority.
pub struct NativeDeclaredFieldAdditionCandidate {
    source: current_schema::Source,
    after: Vec<u8>,
    definition: DeclaredFieldAdditionDefinition,
    native_table: i32,
    native_number: usize,
    column_id: u16,
}

/// Consuming original/effect split used only by the checked intent owner.
pub(crate) struct DeclaredFieldAdditionParts {
    pub(crate) effect: DeclaredFieldAdditionEffect,
    pub(crate) definition: DeclaredFieldAdditionDefinition,
}

/// A sealed independently proved effect; no raw-byte constructor is exposed.
pub(crate) struct DeclaredFieldAdditionEffect {
    source: current_schema::Source,
    after: NativeImageSource,
    native_table: i32,
    native_number: usize,
    column_id: u16,
}

impl DeclaredFieldAdditionEffect {
    pub(crate) fn before_bytes(&self) -> &[u8] {
        self.source.image().as_bytes()
    }
    pub(crate) fn after_bytes(&self) -> &[u8] {
        self.after.as_bytes()
    }
    pub(crate) fn after_shared_bytes(&self) -> bytes::Bytes {
        self.after.shared_bytes()
    }
    pub(crate) fn table(&self) -> &str {
        self.source.table().schema.name()
    }
    pub(crate) const fn native_table_id(&self) -> i32 {
        self.native_table
    }
    pub(crate) fn definition_page(&self) -> u32 {
        self.source.table().schema.definition_page()
    }
    pub(crate) const fn native_number(&self) -> usize {
        self.native_number
    }
    pub(crate) const fn column_id(&self) -> u16 {
        self.column_id
    }
    pub(crate) fn column_count(&self) -> usize {
        self.source.table().schema.columns().len() + 1
    }
}

impl NativeDeclaredFieldAdditionCandidate {
    pub(crate) fn into_parts(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<DeclaredFieldAdditionParts, InspectError> {
        let Self {
            source,
            after,
            definition,
            native_table,
            native_number,
            column_id,
        } = self;
        let after = NativeImageSource::from_vec(after, budget, cancelled)?;
        Ok(DeclaredFieldAdditionParts {
            effect: DeclaredFieldAdditionEffect {
                source,
                after,
                native_table,
                native_number,
                column_id,
            },
            definition,
        })
    }

    /// Complete immutable original; no physical scope is inferred from this slice.
    pub fn before_bytes(&self) -> &[u8] {
        self.source.image().as_bytes()
    }
    /// Complete independently checked replacement, not a durable plan.
    pub fn after_bytes(&self) -> &[u8] {
        &self.after
    }
    /// Actual original table spelling.
    pub fn table(&self) -> &str {
        self.source.table().schema.name()
    }
    /// Exact requested new field spelling.
    pub fn column(&self) -> &str {
        self.definition.name()
    }
    /// Exact unadorned original field request.
    pub fn definition(&self) -> &DeclaredFieldAdditionDefinition {
        &self.definition
    }
    /// Original native table identity, separate from caller catalog scope.
    pub const fn native_table_id(&self) -> i32 {
        self.native_table
    }
    /// Original target definition root.
    pub fn definition_page(&self) -> u32 {
        self.source.table().schema.definition_page()
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
        self.source.table().schema.columns().len() + 1
    }
}

impl NativeImage {
    /// Append one explicit nullable Currency field to a qualified retained keyless table.
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
    pub fn prepare_retained_declared_field_addition(
        self,
        table: &str,
        definition: DeclaredFieldAdditionDefinition,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeDeclaredFieldAdditionCandidate, Error> {
        let _span = tracing::debug_span!("native.field_addition.prepare_declared").entered();
        let source = Family::Retained.capture(self, table, budget, cancelled)?;
        qualify_source(&source, &definition, budget, cancelled)?;
        let current = source.table();
        let plan = Plan::for_field(
            source.image(),
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
        let after =
            plan.build_field(source.image(), definition.name(), field, budget, cancelled)?;
        proof::candidate(&source, &plan, &definition, &after, budget, cancelled)?;
        let mut native_table = None;
        for origin in source.image().catalog().origins() {
            check(budget, cancelled)?;
            if origin.object_id() == current.schema.object_id()
                && native_table.replace(origin.native_id()).is_some()
            {
                return Err(InspectError::Malformed.into());
            }
        }
        Ok(NativeDeclaredFieldAdditionCandidate {
            native_table: native_table.ok_or(InspectError::Malformed)?,
            native_number: plan.number,
            column_id: plan.column_id,
            source,
            after,
            definition,
        })
    }
}

fn qualify_source(
    source: &current_schema::Source,
    definition: &DeclaredFieldAdditionDefinition,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    let current = source.table();
    let layout = current.schema.physical_layout();
    if layout.physical_indexes != 0
        || layout.logical_indexes != 0
        || current.schema.columns().len() >= crate::limits::MAX_USER_COLUMNS
    {
        return Err(Error::Source);
    }
    for field in current.schema.columns() {
        check(budget, cancelled)?;
        budget.charge(
            Resource::Steps,
            field.name().len().max(definition.name().len()) as u64,
        )?;
        // Native Unicode identifier equivalence is not qualified by a Text
        // comparer or ASCII folding. Restrict only this target field namespace.
        if !field.name().is_ascii()
            || field.flags() != (UPDATABLE_FLAG | if field.fixed() { FIXED_FLAG } else { 0 })
        {
            return Err(Error::Source);
        }
        if field.name().eq_ignore_ascii_case(definition.name()) {
            return Err(Error::DuplicateName);
        }
    }
    for group in current.properties.groups() {
        check(budget, cancelled)?;
        if group.kind() == PropertyGroupKind::Column {
            budget.charge(
                Resource::Steps,
                group.name().len().max(definition.name().len()) as u64,
            )?;
            if !group.name().is_ascii() || group.name().eq_ignore_ascii_case(definition.name()) {
                return Err(Error::Source);
            }
        }
    }
    Ok(())
}
