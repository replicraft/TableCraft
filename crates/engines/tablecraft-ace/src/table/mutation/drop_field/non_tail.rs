//! Separate retained non-tail retirement preserves stored ordinals and record bytes.
use super::{
    FieldDropError as Error, NativeFieldDropIdentity, schema_candidate,
    schema_source::{self, Selection, Source},
};
use crate::{NativeImage, inspect_table_schema};
use tablecraft_core::limits::ResourceBudget;

/// Sealed non-tail DROP over a completely qualified retained schema. This does
/// not grant row serialization authority or belong to an older tail language.
pub struct NativeRetainedNonTailFieldDropCandidate {
    source: Source,
    after: Vec<u8>,
}

impl NativeRetainedNonTailFieldDropCandidate {
    /// Complete original bytes under the retained schema-purpose certificate.
    pub fn before_bytes(&self) -> &[u8] {
        self.source.proof.image().as_bytes()
    }

    /// Complete replacement; all DATA/INDEX bytes and native high water remain.
    pub fn after_bytes(&self) -> &[u8] {
        &self.after
    }

    /// Exact original table spelling, independent of visible captions.
    pub fn table(&self) -> &str {
        self.source.evidence.observed.schema.name()
    }

    /// Exact original non-tail field spelling.
    pub fn column(&self) -> &str {
        self.source.evidence.observed.column().name()
    }

    /// Original table/column identity including stored logical number and ID.
    pub fn identity(&self) -> NativeFieldDropIdentity {
        self.source.evidence.identity()
    }

    /// Original live record count, unchanged by this metadata-only operation.
    pub fn declared_rows(&self) -> u32 {
        self.source.evidence.observed.schema.declared_rows()
    }
}

impl NativeImage {
    /// Select the non-tail request language from an actual retained source and
    /// active native descriptor position. No source or mutation proof is granted.
    pub fn select_native_retained_non_tail_field_drop_source(
        &self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, Error> {
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
        let selected = schema
            .columns()
            .iter()
            .position(|field| field.name().eq_ignore_ascii_case(column))
            .ok_or(Error::Source)?;
        Ok(selected + 1 < schema.columns().len())
    }

    /// Inspect a selected ordinary non-tail declaration with complete source,
    /// properties, dependencies, programs and ownership checks; allocate no AFTER.
    pub fn inspect_retained_non_tail_field_drop_source(
        &self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeFieldDropIdentity, Error> {
        Ok(schema_source::observe(
            self,
            table,
            column,
            Selection::RetainedNonTail,
            budget,
            cancelled,
        )?
        .identity())
    }

    /// Remove one qualified ordinary unindexed non-tail field and its property
    /// group, preserving all physical records and survivor native identities.
    pub fn prepare_retained_non_tail_field_drop(
        self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeRetainedNonTailFieldDropCandidate, Error> {
        let candidate = schema_candidate::prepare(
            self,
            table,
            column,
            Selection::RetainedNonTail,
            budget,
            cancelled,
        )?;
        Ok(NativeRetainedNonTailFieldDropCandidate {
            source: candidate.source,
            after: candidate.after,
        })
    }
}
