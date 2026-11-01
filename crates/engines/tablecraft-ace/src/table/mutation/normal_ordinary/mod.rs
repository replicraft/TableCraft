//! Ordinary fixed requests require fresh complete content ownership proof.
mod proof;

#[cfg(test)]
mod tests;

use crate::{
    EditError, FixedColumnUpdates, InspectError, NativeEditPlan, NativeImage,
    header::check,
    table::mutation::{
        fixed_records::{self, binding, stage},
        ordinary_content,
        patch::BytePatch,
        preserved_source::{self, Source, validation},
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const CARRIER_COMPARISON_BYTES: usize = 2 * size_of::<i64>();

impl NativeImage {
    /// Plan original fixed assignments only after proving complete ordinary
    /// content. The caller supplies no absence flag, program or creator token.
    pub(crate) fn plan_ordinary_fixed_updates(
        self,
        table: &str,
        original: &[FixedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let _span = tracing::debug_span!("native.ordinary.fixed.plan").entered();
        let source = capture(&self, table, budget, cancelled)?;
        preflight(&source, original, budget, cancelled)?;

        let (patches, changed_rows) =
            stage::prepare_normal(&self, &source, original, budget, cancelled)?;
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
    let certificate = ordinary_content::capture(image, budget, cancelled)?;
    preserved_source::capture_closed_content(image, table, &certificate, budget, cancelled)
}

/// Qualify all declarations and carriers before comparing any expected value.
/// Equal supplied values remain ordinary no-ops because the content proof has
/// already established that no supported or unresolved program storage exists.
fn preflight(
    source: &Source,
    original: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let (bound, _) = binding::all_normal(source, original, budget, cancelled)?;
    for (group, selected) in original.iter().zip(bound) {
        check(budget, cancelled)?;
        for update in group.updates() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, CARRIER_COMPARISON_BYTES as u64)
                .map_err(InspectError::from)?;
            let before = source
                .table
                .rows()
                .get(update.row())
                .and_then(|row| row.get(selected.column))
                .ok_or(EditError::Conflict)?;
            if before != update.expected() {
                return Err(EditError::Conflict);
            }
        }
    }
    check(budget, cancelled)?;
    Ok(())
}
