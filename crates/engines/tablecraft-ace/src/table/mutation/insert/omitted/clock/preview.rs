//! Explicit clock authorization is additive to literal-only omission previews.
use super::super::{OmittedRowInsert, Original, preview::qualify, stage};
use super::bind;
use crate::table::mutation::insert::{AuditRequest, default_clock, finish_with_clock};
use crate::{
    Ace16AutoNumberInsertPreview, Ace16WritePreview, EditError, IncrementingAutoNumber, NativeImage,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::functions::dates::clock::Clock;

impl NativeImage {
    /// Preview omitted fields with an explicitly authorized operation clock.
    /// Only needed Date()/Now() defaults on legacy DATE fields use the provider.
    /// All needed declarations and generation bounds are checked first. One
    /// reading supplies the whole operation; supplied values/NULL and operations
    /// needing no clock never invoke it. This is an engineering timing policy,
    /// not native Access save or per-statement timing qualification.
    pub fn preview_ace16_insert_rows_with_omissions_and_clock(
        self,
        table: &str,
        requests: &[OmittedRowInsert],
        clock: &dyn Clock,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16WritePreview, EditError> {
        let _span = tracing::debug_span!(
            "native.edit.preview.insert_omissions_clock",
            rows = requests.len()
        )
        .entered();
        let original = Original::Ordinary(requests);
        let checked = qualify(&self, table, original, budget, cancelled)?;
        let bound = bind(&self, &checked, original, budget, cancelled)?;

        // The immutable original operation input stays local to the coordinator,
        // independent of all mutable producer materialization and staging facts.
        let reading = if bound.needs_clock() {
            Some(default_clock::capture(clock, budget, cancelled)?)
        } else {
            None
        };
        let prepared = bound.resolve(reading, budget, cancelled)?;
        let staged = stage::consume(&self, &checked, prepared, budget, cancelled)?;
        finish_with_clock(
            self,
            table,
            AuditRequest::Omitted(original),
            &staged,
            reading,
            budget,
            cancelled,
        )
    }

    /// Preview generated companions and needed Date()/Now() defaults with one
    /// explicit operation reading. The exact source LAST and requested generated
    /// range are checked before invoking the clock. Stored mode is not inferred.
    pub fn preview_ace16_incrementing_insert_rows_with_omissions_and_clock(
        self,
        table: &str,
        requests: &[OmittedRowInsert],
        policy: IncrementingAutoNumber,
        clock: &dyn Clock,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16AutoNumberInsertPreview, EditError> {
        let _span = tracing::debug_span!(
            "native.edit.preview.insert_incrementing_omissions_clock",
            rows = requests.len()
        )
        .entered();
        let original = Original::Incrementing { requests, policy };
        let checked = qualify(&self, table, original, budget, cancelled)?;
        let bound = bind(&self, &checked, original, budget, cancelled)?;

        let reading = if bound.needs_clock() {
            Some(default_clock::capture(clock, budget, cancelled)?)
        } else {
            None
        };
        let prepared = bound.resolve(reading, budget, cancelled)?;
        let mut staged = stage::consume(&self, &checked, prepared, budget, cancelled)?;
        let candidate = finish_with_clock(
            self,
            table,
            AuditRequest::Omitted(original),
            &staged,
            reading,
            budget,
            cancelled,
        )?;
        let plan = staged.autonumber.take().ok_or(EditError::Preservation)?;
        Ok(Ace16AutoNumberInsertPreview::from_candidate(
            candidate,
            plan.source.column,
            plan.values,
        ))
    }
}
