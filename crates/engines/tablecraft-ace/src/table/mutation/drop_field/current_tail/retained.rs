//! Retained tail DROP uses its own source family and shared exact snapshot owner.
use super::{Error, NativeCurrentTailFieldDropCandidate, definition, source, verify};
use crate::{
    NativeFieldDropIdentity, NativeImage, header::check,
    table::mutation::catalog_property::retained_snapshot,
};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Inspect the selected retained tail without constructing a replacement.
    pub fn inspect_retained_tail_field_drop_source(
        &self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeFieldDropIdentity, Error> {
        Ok(source::observe_retained(self, table, column, budget, cancelled)?.identity())
    }

    /// Remove one qualified nullable unindexed retained tail declaration and
    /// its property group, preserving record bytes and all native high-water marks.
    pub fn prepare_retained_tail_field_drop(
        self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeCurrentTailFieldDropCandidate, Error> {
        let _span = tracing::debug_span!("native.retained_tail_field_drop.prepare").entered();
        let mut source = source::capture_retained(self, table, column, budget, cancelled)?;
        let before = source.proof.image();
        let page = definition::page(before, &source.evidence, budget, cancelled)?;
        let mut after = definition::copy_image(before, budget, cancelled)?;
        let base = source.evidence.observed.schema.definition_page() as usize
            * crate::format::page::PAGE_BYTES;
        after[base..base + page.len()].copy_from_slice(&page);
        let body = std::mem::take(&mut source.evidence.body);
        let change =
            retained_snapshot::compose(before, source.proof.table(), body, budget, cancelled)?;
        change.apply(&mut after, budget, cancelled)?;
        verify::retained_candidate(&source, &after, &change, budget, cancelled)?;
        verify::survivors(&source, &after, budget, cancelled)?;
        drop(change);
        check(budget, cancelled)?;
        Ok(NativeCurrentTailFieldDropCandidate { source, after })
    }
}

#[cfg(test)]
mod tests;
