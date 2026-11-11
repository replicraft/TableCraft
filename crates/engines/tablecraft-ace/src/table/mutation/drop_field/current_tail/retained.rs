//! Retained tail DROP keeps exact source languages and shared snapshot placement.
use super::{Error, NativeCurrentTailFieldDropCandidate, definition, source, verify};
use crate::{
    NativeFieldDropIdentity, NativeImage,
    header::check,
    inspect_table_schema,
    table::mutation::{catalog_property::retained_snapshot, saved_field},
};
use source::Selection;
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Inspect the selected original retained tail without constructing AFTER.
    pub fn inspect_retained_tail_field_drop_source(
        &self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeFieldDropIdentity, Error> {
        Ok(
            source::observe(self, table, column, Selection::Retained, budget, cancelled)?
                .identity(),
        )
    }

    /// Remove one qualified nullable unindexed retained LONG/Text tail, preserving
    /// record bytes and native high-water marks under its original action language.
    pub fn prepare_retained_tail_field_drop(
        self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeCurrentTailFieldDropCandidate, Error> {
        prepare(self, table, column, Selection::Retained, budget, cancelled)
    }

    /// Select the typed language from actual tail metadata before full proof.
    pub fn select_native_retained_typed_tail_field_drop_source(
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
        if schema.columns().len() < 2
            || schema
                .columns()
                .last()
                .is_none_or(|last| !last.name().eq_ignore_ascii_case(column))
        {
            return Err(Error::Source);
        }
        Ok(saved_field::retained_field_uses_typed_language(
            self, table, column, budget, cancelled,
        )?)
    }

    /// Inspect the full schema and actual scalar tail without granting row writes.
    pub fn inspect_retained_typed_tail_field_drop_source(
        &self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeFieldDropIdentity, Error> {
        Ok(source::observe(
            self,
            table,
            column,
            Selection::RetainedTyped,
            budget,
            cancelled,
        )?
        .identity())
    }

    /// Remove one ordinary typed tail and its exact property group. Existing
    /// records, native high-water counts, indexes and property history remain.
    pub fn prepare_retained_typed_tail_field_drop(
        self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeCurrentTailFieldDropCandidate, Error> {
        prepare(
            self,
            table,
            column,
            Selection::RetainedTyped,
            budget,
            cancelled,
        )
    }
}

fn prepare(
    image: NativeImage,
    table: &str,
    column: &str,
    selection: Selection,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeCurrentTailFieldDropCandidate, Error> {
    let _span = tracing::debug_span!("native.retained_tail_field_drop.prepare").entered();
    let mut source = source::capture(image, table, column, selection, budget, cancelled)?;
    let before = source.proof.image();
    let page = definition::page(before, &source.evidence, budget, cancelled)?;
    let mut after = definition::copy_image(before, budget, cancelled)?;
    let base = source.evidence.observed.schema.definition_page() as usize
        * crate::format::page::PAGE_BYTES;
    after[base..base + page.len()].copy_from_slice(&page);
    let body = std::mem::take(&mut source.evidence.body);
    let change = retained_snapshot::compose(before, source.proof.table(), body, budget, cancelled)?;
    change.apply(&mut after, budget, cancelled)?;
    verify::retained_candidate(&source, &after, &change, budget, cancelled)?;
    verify::survivors(&source, &after, budget, cancelled)?;
    drop(change);
    check(budget, cancelled)?;
    Ok(NativeCurrentTailFieldDropCandidate { source, after })
}

#[cfg(test)]
mod tests;
