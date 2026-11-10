use super::{FieldDropError as Error, NativeFieldDropIdentity, definition, source, verify};
use crate::{NativeImage, header::check, table::mutation::catalog_property};
use tablecraft_core::limits::ResourceBudget;

/// Sealed unpublished saved-field removal with complete original and AFTER images.
/// This owns no journal or normal save permission and admits no later gapped edit.
pub struct NativeFieldDropCandidate {
    pub(super) source: source::Source,
    after: Vec<u8>,
}

impl NativeFieldDropCandidate {
    /// Complete original including the protected OFF and retired property rows.
    pub fn before_bytes(&self) -> &[u8] {
        self.source.proof.image().as_bytes()
    }

    /// Complete same-length appended-property-snapshot candidate.
    pub fn after_bytes(&self) -> &[u8] {
        &self.after
    }

    /// Actual original table spelling.
    pub fn table(&self) -> &str {
        self.source.evidence.observed.schema.name()
    }

    /// Actual original selected field spelling.
    pub fn column(&self) -> &str {
        self.source.evidence.observed.column().name()
    }

    /// Source-bound native field identity, independent of visible position.
    pub fn identity(&self) -> NativeFieldDropIdentity {
        self.source.evidence.identity()
    }
}

impl NativeImage {
    /// Inspect actual DROP source/target restrictions without an AFTER image.
    /// This retains no save authority. Physical property fitting remains checked
    /// during preparation; a source-eligible field can still exhaust fitted space.
    pub fn inspect_saved_field_drop_source(
        &self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeFieldDropIdentity, Error> {
        source::request(table, column, budget, cancelled)?;
        self.check_field_history_name_auto_correct_off_source(budget, cancelled)?;
        Ok(source::Evidence::capture(self, table, column, budget, cancelled)?.identity())
    }

    /// Prepare one nonprimary Number/Text removal from the settled empty OFF source.
    /// Preserves native numbering/high-water counts, unrelated indexes and every
    /// prior property ROW. No persistence, later mutation or native Access claim.
    pub fn prepare_saved_field_drop(
        self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeFieldDropCandidate, Error> {
        let _span = tracing::debug_span!("native.saved_field_drop.prepare").entered();
        let source = source::Source::capture(self, table, column, budget, cancelled)?;
        let page = definition::page(source.proof.image(), &source.evidence, budget, cancelled)?;
        let mut after = definition::copy_image(source.proof.image(), budget, cancelled)?;
        let base = source.evidence.observed.schema.definition_page() as usize
            * crate::format::page::PAGE_BYTES;
        after[base..base + page.len()].copy_from_slice(&page);
        // Move the genuine encoder output into the real allocator/parent owner.
        // Evidence retains the original semantic metadata; verification compares
        // the placed body independently against exactly one original group block.
        let mut source = source;
        let body = std::mem::take(&mut source.evidence.body);
        let change = catalog_property::compose_fitted_at_origin(
            source.proof.image(),
            &source.evidence.observed.schema,
            body,
            budget,
            cancelled,
        )?;
        change.apply(&mut after, budget, cancelled)?;
        verify::candidate(
            source.proof.image(),
            &source.evidence,
            &after,
            &change,
            budget,
            cancelled,
        )?;
        check(budget, cancelled)?;
        Ok(NativeFieldDropCandidate { source, after })
    }
}
