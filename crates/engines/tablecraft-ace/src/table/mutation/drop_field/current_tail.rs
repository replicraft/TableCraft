//! Populated native tail removal with retained row and historical storage.
mod source;
mod verify;

#[cfg(test)]
mod tests;

use super::{FieldDropError as Error, NativeFieldDropIdentity, definition};
use crate::{NativeImage, header::check, table::mutation::catalog_property};
use tablecraft_core::limits::ResourceBudget;

/// Sealed current-source tail removal, distinct from the old empty-table family.
/// Existing records retain their exact native masks, offsets and retired values.
pub struct NativeCurrentTailFieldDropCandidate {
    source: source::Source,
    after: Vec<u8>,
}

impl NativeCurrentTailFieldDropCandidate {
    /// Complete original native bytes under the positive current-table proof.
    pub fn before_bytes(&self) -> &[u8] {
        self.source.proof.image().as_bytes()
    }

    /// Complete metadata-only replacement; no DATA or INDEX page is rewritten.
    pub fn after_bytes(&self) -> &[u8] {
        &self.after
    }

    /// Original native table spelling.
    pub fn table(&self) -> &str {
        self.source.evidence.observed.schema.name()
    }

    /// Original selected tail field spelling.
    pub fn column(&self) -> &str {
        self.source.evidence.observed.column().name()
    }

    /// Original logical field number and separately retained creation-time ID.
    pub fn identity(&self) -> NativeFieldDropIdentity {
        self.source.evidence.identity()
    }

    /// Complete original live-row count, unchanged by field deletion.
    pub fn declared_rows(&self) -> u32 {
        self.source.evidence.observed.schema.declared_rows()
    }
}

impl NativeImage {
    /// Inspect populated tail-field eligibility without creating an AFTER or lease.
    /// This independently proves programs, dependencies, native rows and ownership;
    /// it does not retry the original empty-field history profile.
    pub fn inspect_current_tail_field_drop_source(
        &self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeFieldDropIdentity, Error> {
        let _span = tracing::debug_span!("native.current_tail_field_drop.inspect").entered();
        Ok(source::observe(self, table, column, budget, cancelled)?.identity())
    }

    /// Remove one nullable, unindexed native tail LONG or Short Text declaration.
    /// Native high-water counts, creation IDs, every old record and unrelated
    /// property/index/storage ownership remain intact. A separate original-request
    /// family must qualify any journal save or cold recovery.
    pub fn prepare_current_tail_field_drop(
        self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeCurrentTailFieldDropCandidate, Error> {
        let _span = tracing::debug_span!("native.current_tail_field_drop.prepare").entered();
        let mut source = source::capture(self, table, column, budget, cancelled)?;
        let before = source.proof.image();
        let page = definition::page(before, &source.evidence, budget, cancelled)?;
        let mut after = definition::copy_image(before, budget, cancelled)?;
        let base = source.evidence.observed.schema.definition_page() as usize
            * crate::format::page::PAGE_BYTES;
        after[base..base + page.len()].copy_from_slice(&page);

        let body = std::mem::take(&mut source.evidence.body);
        let change = catalog_property::compose_fitted_at_origin(
            source.proof.image(),
            &source.evidence.observed.schema,
            body,
            budget,
            cancelled,
        )?;
        change.apply(&mut after, budget, cancelled)?;
        super::verify::candidate(
            source.proof.image(),
            &source.evidence,
            &after,
            &change,
            budget,
            cancelled,
        )?;
        verify::survivors(&source, &after, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(NativeCurrentTailFieldDropCandidate { source, after })
    }
}
