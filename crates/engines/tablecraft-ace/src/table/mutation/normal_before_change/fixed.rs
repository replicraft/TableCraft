//! Normal original fixed requests with required native BeforeChange programs.
mod proof;
mod selection;

#[cfg(test)]
mod tests;

use super::{event_error, mixed};
use crate::{
    EditError, FixedColumnUpdates, InspectError, NativeEditPlan, NativeImage, WriteBlocker,
    header::check,
    inspect_table_schema,
    table::mutation::{
        ace_profile,
        fixed_records::{self, binding, stage},
        guards::{buffer, fields::batch, native_events},
        patch::BytePatch,
        preserved_source::{Source, validation},
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

impl NativeImage {
    /// Select the new fixed-carrier context using original readonly facts.
    /// False retains the existing Int32 service; a failed mutation is never
    /// caught or retried through another writer.
    pub fn select_native_before_change_fixed_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        let _span = tracing::debug_span!("native.before_change.normal.fixed.select").entered();
        selection::selected(self, table, budget, cancelled)
    }

    /// Compile native required programs for the selected fixed context without
    /// executing any event row or granting permission for a later request.
    pub fn check_native_before_change_fixed_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let _span = tracing::debug_span!("native.before_change.normal.fixed.table").entered();
        let source = capture(self, table, budget, cancelled)?;
        let mut target = None;
        for (position, field) in source.table.schema().columns().iter().enumerate() {
            check(budget, cancelled)?;
            if selection::eligible_wide(&source, position, budget, cancelled)? {
                target = Some(field.name());
                break;
            }
        }
        let target = target.ok_or(EditError::Unsupported(WriteBlocker::Column))?;
        let original = [FixedColumnUpdates::new(
            source.table.schema().name(),
            target,
            &[],
        )];
        preflight(self, &source, &original, budget, cancelled)
    }

    /// Internal normal fixed planner; journal intent owns the public entry.
    /// The caller supplies original user requests, never selected programs or
    /// final macro-written values. No target or journal write occurs here.
    pub(crate) fn plan_before_change_fixed_updates(
        self,
        table: &str,
        original: &[FixedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let _span = tracing::debug_span!("native.before_change.normal.fixed.plan").entered();
        let source = capture(&self, table, budget, cancelled)?;
        preflight(&self, &source, original, budget, cancelled)?;
        validate_original(&source, original, budget, cancelled)?;
        let mut prepared = prepare(&self, &source, original, budget, cancelled)?;
        if let Some(rejection) = prepared.take_rejection() {
            return Err(EditError::BeforeChangeRejected(rejection));
        }
        prepared.require_source(&self, source.table.schema(), budget, cancelled)?;
        let groups = prepared
            .groups(source.table.schema(), budget, cancelled)
            .map_err(event_error)?;
        let (patches, changed_rows) =
            stage::prepare_normal(&self, &source, &groups, budget, cancelled)?;
        let mut after = fixed_records::copy(self.as_bytes(), budget, cancelled)?;
        for patch in patches {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, patch.before().len() as u64)
                .map_err(InspectError::from)?;
            let end = patch
                .offset()
                .checked_add(patch.before().len())
                .ok_or(EditError::Preservation)?;
            let output = after
                .get_mut(patch.offset()..end)
                .ok_or(EditError::Preservation)?;
            if output != patch.before() {
                return Err(EditError::Preservation);
            }
            output.copy_from_slice(patch.after());
        }
        validation::expected(&self, table, &source.table, &after, budget, cancelled)?;
        drop(groups);
        drop(prepared);
        let scope = self.catalog().catalog().scope();
        let plan = NativeEditPlan {
            before: self.bytes,
            after,
            changed_rows,
        };
        proof::prove(&plan, scope, table, original, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(plan)
    }
}

fn capture(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    ace_profile::qualify(image)?;
    let schema = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?;
    if !selection::ordinary_schema(&schema, budget, cancelled)? {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    let source = mixed::capture(image, table, budget, cancelled)?;
    let schema = source.table.schema();
    if source.table.columns().len() != schema.columns().len() {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    Ok(source)
}

fn prepare<'image>(
    image: &'image NativeImage,
    source: &Source,
    original: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<batch::Prepared<'image>, EditError> {
    let selected = native_events::programs(image, source.table.schema(), budget, cancelled)
        .map_err(event_error)?;
    let facts = batch::SourceFacts {
        table: &source.table,
        indexes: &source.indexes,
        dependencies: &source.dependencies,
    };
    batch::prepare(image, facts, selected, original, budget, cancelled).map_err(event_error)
}

/// Discover and compile the complete original program before checking timing.
/// Empty original groups retain their actual target declarations; no event row
/// is evaluated in this preflight.
fn preflight(
    image: &NativeImage,
    source: &Source,
    original: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let mut empty = buffer::vector(original.len(), budget, cancelled)?;
    for group in original {
        check(budget, cancelled)?;
        empty.push(FixedColumnUpdates::new(group.table(), group.column(), &[]));
    }
    let prepared = prepare(image, source, &empty, budget, cancelled)?;
    if prepared.rejection().is_some() {
        return Err(EditError::Preservation);
    }
    Ok(())
}

/// Field/carrier/duplicate checks precede stale comparisons; all stale checks
/// precede uncertain equal-set timing. Macro compilation already completed.
fn validate_original(
    source: &Source,
    original: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    binding::all_normal(source, original, budget, cancelled)?;
    for group in original {
        let mut selected = None;
        for (position, field) in source.table.columns().iter().enumerate() {
            check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    field.name().len().max(group.column().len()) as u64,
                )
                .map_err(InspectError::from)?;
            if field.name().eq_ignore_ascii_case(group.column())
                && selected.replace(position).is_some()
            {
                return Err(EditError::Preservation);
            }
        }
        let position = selected.ok_or(EditError::Unsupported(WriteBlocker::Column))?;
        for update in group.updates() {
            check(budget, cancelled)?;
            // All admitted carriers have at most eight payload bytes.
            budget
                .charge(Resource::Steps, (2 * size_of::<i64>()) as u64)
                .map_err(InspectError::from)?;
            let before = source
                .table
                .rows()
                .get(update.row())
                .and_then(|row| row.get(position))
                .ok_or(EditError::Conflict)?;
            if before != update.expected() {
                return Err(EditError::Conflict);
            }
        }
    }
    for group in original {
        for update in group.updates() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, (2 * size_of::<i64>()) as u64)
                .map_err(InspectError::from)?;
            if update.expected() == update.value() {
                return Err(EditError::Unsupported(WriteBlocker::EventTiming));
            }
        }
    }
    Ok(())
}
