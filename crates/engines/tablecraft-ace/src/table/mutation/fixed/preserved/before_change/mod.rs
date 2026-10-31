//! Explicit selected-program rich fixed batch, never a normal save or journal.
mod proof;

use super::{copy, source, stage};
use crate::{
    Ace16BeforeChangePreview, Ace16WritePreview, BeforeChangePreviewError, BeforeEventGuardError,
    BeforeEventGuardSelector, EditError, FixedColumnUpdates, NativeImage,
    header::check,
    inspect_table_schema,
    table::mutation::{
        guards::fields::batch::{self, SelectedPrograms, SourceFacts},
        patch::BytePatch,
        preview as native_preview,
    },
};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Apply original fixed-column user intents and complete explicitly selected
    /// Int32 BeforeChange programs to one preserved rich-schema candidate.
    ///
    /// Current starts with all original Int32 assignments; Old remains original.
    /// Ordinary wide fixed user values stay outside the Int32 macro projection.
    /// Only unindexed/nondependent targets may change; indexed ordinary Int32
    /// operands may be read. All targets and dormant programs qualify before any
    /// event runs. Final constraints apply after the complete transformation.
    ///
    /// Selector order is explicit engineering order, not native required-event
    /// discovery or dispatch. This returns the trace-only physical candidate type,
    /// never a partial public logical final-row report or a native save capability.
    pub fn preview_ace16_before_change_batch_updates(
        self,
        table: &str,
        selectors: &[BeforeEventGuardSelector<'_>],
        original: &[FixedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16BeforeChangePreview, BeforeChangePreviewError> {
        let _span = tracing::debug_span!("native.before_change.batch_preview").entered();
        preview(self, table, selectors, original, budget, cancelled)
    }
}

fn preview(
    image: NativeImage,
    table: &str,
    selectors: &[BeforeEventGuardSelector<'_>],
    original: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16BeforeChangePreview, BeforeChangePreviewError> {
    check(budget, cancelled).map_err(BeforeEventGuardError::from)?;
    native_preview::qualify(&image)?;
    let schema = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )
    .map_err(BeforeEventGuardError::from)?;
    let selected = SelectedPrograms::capture(&image, &schema, selectors, budget, cancelled)?;
    let qualified = source::capture_before_change(&image, table, &selected, budget, cancelled)?;
    let mut prepared = batch::prepare(
        &image,
        SourceFacts {
            table: &qualified.table,
            indexes: &qualified.indexes,
            dependencies: &qualified.dependencies,
        },
        selected,
        original,
        budget,
        cancelled,
    )?;
    if let Some(rejection) = prepared.take_rejection() {
        return Err(BeforeChangePreviewError::Rejected(rejection));
    }
    let groups = prepared.groups(qualified.table.schema(), budget, cancelled)?;
    // This is the actual existing stage, over one real original Source. It does
    // not call a public preview, manufacture a candidate or chain a second edit.
    let (patches, changed_rows) = stage::prepare(
        &image,
        &qualified,
        &groups,
        groups.len() > 1,
        budget,
        cancelled,
    )?;
    let mut after = copy(image.as_bytes(), budget, cancelled)?;
    for patch in patches {
        check(budget, cancelled).map_err(BeforeEventGuardError::from)?;
        let end = patch
            .offset()
            .checked_add(patch.before().len())
            .ok_or(EditError::Preservation)?;
        let selected = after
            .get_mut(patch.offset()..end)
            .ok_or(EditError::Preservation)?;
        if selected != patch.before() || patch.before().len() != patch.after().len() {
            return Err(EditError::Preservation.into());
        }
        selected.copy_from_slice(patch.after());
    }

    // The proof recaptures its own selected original, replays original user
    // intent, and supplies its own derived groups to the existing byte auditor.
    proof::prove(
        &image,
        table,
        selectors,
        original,
        proof::Observed {
            prepared: &prepared,
            bytes: &after,
            changed_rows,
        },
        budget,
        cancelled,
    )?;
    {
        let candidate = image
            .recapture_fixed_candidate(&after, budget, cancelled)
            .map_err(BeforeEventGuardError::from)?;
        let candidate_schema = inspect_table_schema(
            &mut candidate.reader(),
            candidate.catalog().catalog().scope(),
            table,
            budget,
            cancelled,
        )
        .map_err(BeforeEventGuardError::from)?;
        let rebound = prepared.admission().rebind_candidate(
            &candidate,
            &candidate_schema,
            budget,
            cancelled,
        )?;
        source::capture_before_change(&candidate, table, &rebound, budget, cancelled)?;
    }
    check(budget, cancelled).map_err(BeforeEventGuardError::from)?;
    drop(groups);
    let report = prepared.into_report();
    let candidate = Ace16WritePreview::from_candidate(image.bytes, after, changed_rows);
    Ok(Ace16BeforeChangePreview::from_parts(
        candidate,
        report.programs,
        report.trace,
        report.assignments,
    ))
}

#[cfg(test)]
mod tests;
