//! Typed retained gallery ADD, separate from the original Number/Text grammars.
use super::{
    CurrentFieldAdditionError as Error, CurrentFieldAdditionSource, Target, creation_error,
    definition::Plan, facts, prepare,
};
use crate::{
    NativeImage, RetainedFieldAdditionDefinition, RetainedFieldAdditionType,
    table::mutation::current_schema::{self, Family},
};
use tablecraft_core::limits::ResourceBudget;

/// Sealed typed metadata addition, retaining exact original request and source.
pub struct NativeRetainedTypedFieldAdditionCandidate {
    proof: current_schema::Source,
    after: Vec<u8>,
    definition: RetainedFieldAdditionDefinition,
    facts: CurrentFieldAdditionSource,
}

impl NativeRetainedTypedFieldAdditionCandidate {
    /// Complete immutable original after ownership/program qualification.
    pub fn before_bytes(&self) -> &[u8] {
        self.proof.image().as_bytes()
    }

    /// Exact metadata-only replacement; no old record is backfilled.
    pub fn after_bytes(&self) -> &[u8] {
        &self.after
    }

    /// Source-resolved table spelling.
    pub fn table(&self) -> &str {
        self.proof.table().schema.name()
    }

    /// Owned original typed declaration.
    pub fn definition(&self) -> &RetainedFieldAdditionDefinition {
        &self.definition
    }

    /// Exact requested new field name.
    pub fn column(&self) -> &str {
        self.definition.name()
    }

    /// Native ordinal allocated from the retained high-water mark.
    pub const fn native_number(&self) -> usize {
        self.facts.number
    }

    /// Actual stored creation ID.
    pub const fn column_id(&self) -> u16 {
        self.facts.column_id
    }

    /// Replacement active-field count, independent of retained ordinal count.
    pub const fn column_count(&self) -> usize {
        self.facts.active_count + 1
    }
}

impl NativeImage {
    /// Measure fitting name capacity for an unnamed validated typed declaration.
    /// Existing-name collisions are checked only by consuming preparation.
    pub fn inspect_retained_typed_field_addition_source(
        &self,
        table: &str,
        target: RetainedFieldAdditionType,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<CurrentFieldAdditionSource, Error> {
        let current = Family::Retained.observe(self, table, budget, cancelled)?;
        let plan = Plan::for_target(
            self,
            &current.schema,
            Target::Typed(target),
            budget,
            cancelled,
        )?;
        facts(
            self,
            &current,
            &plan,
            Target::Typed(target),
            budget,
            cancelled,
        )
    }

    /// Append one gallery scalar declaration with unchanged historical records.
    pub fn prepare_retained_typed_field_addition(
        self,
        table: &str,
        definition: &RetainedFieldAdditionDefinition,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeRetainedTypedFieldAdditionCandidate, Error> {
        let target = definition.target();
        let parts = prepare(
            self,
            table,
            definition.name(),
            Target::Typed(target),
            Family::Retained,
            budget,
            cancelled,
        )?;
        let definition = RetainedFieldAdditionDefinition::from_name(parts.column, target)
            .map_err(creation_error)?;
        Ok(NativeRetainedTypedFieldAdditionCandidate {
            proof: parts.proof,
            after: parts.after,
            definition,
            facts: parts.facts,
        })
    }
}

#[cfg(test)]
mod tests;
