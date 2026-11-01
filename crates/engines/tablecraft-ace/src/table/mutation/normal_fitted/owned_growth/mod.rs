//! Normal complete-row growth within the table's existing owned DATA pages.
mod proof;
mod selection;
mod stage;

#[cfg(test)]
mod tests;

use crate::{
    EditError, FittedColumnUpdates, InspectError, NativeEditPlan, NativeImage, WriteBlocker,
    format::page::PAGE_BYTES,
    header::check,
    inspect_table_schema,
    table::mutation::{
        normal_ordinary,
        preserved_source::{Source, validation},
        record_placement::audit,
        record_serialization,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const NATIVE_CATALOG_FIELDS: usize = 17;
const CATALOG_NAME: &str = "MSysObjects";

impl NativeImage {
    pub(crate) fn plan_ordinary_owned_growth_updates(
        self,
        table: &str,
        groups: &[FittedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let _span = tracing::debug_span!("native.ordinary.owned_growth.plan").entered();
        check(budget, cancelled)?;
        let source = capture(&self, table, budget, cancelled)?;
        let prepared = stage::prepare(&self, &source, groups, budget, cancelled)?;
        let mut after = record_serialization::copy(self.as_bytes(), budget, cancelled)?;
        for page in &prepared.plan.pages {
            check(budget, cancelled)?;
            let start = usize::try_from(page.number)
                .ok()
                .and_then(|number| number.checked_mul(PAGE_BYTES))
                .ok_or(EditError::Preservation)?;
            let end = start
                .checked_add(PAGE_BYTES)
                .ok_or(EditError::Preservation)?;
            let destination = after.get_mut(start..end).ok_or(EditError::Preservation)?;
            budget
                .charge(Resource::Steps, PAGE_BYTES as u64)
                .map_err(InspectError::from)?;
            if destination != page.before.as_slice() || page.after.len() != PAGE_BYTES {
                return Err(EditError::Preservation);
            }
            destination.copy_from_slice(&page.after);
        }
        audit::final_pages(&prepared.plan, &after, budget, cancelled)?;
        prepared
            .space
            .audit(self.as_bytes(), &after, budget, cancelled)?;
        validation::expected(&self, table, &source.table, &after, budget, cancelled)?;
        let scope = self.catalog().catalog().scope();
        let plan = NativeEditPlan {
            before: self.bytes,
            after,
            changed_rows: prepared.changed_rows,
        };
        proof::prove(&plan, scope, table, groups, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(plan)
    }
}

// This first normal growth profile is target-local native17 admission. The
// global15 closed-content route retains its existing fitting-record policy.
fn require_native_catalog(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let schema = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        CATALOG_NAME,
        budget,
        cancelled,
    )?;
    if schema.columns().len() != NATIVE_CATALOG_FIELDS {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    Ok(())
}

fn capture(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    require_native_catalog(image, budget, cancelled)?;
    normal_ordinary::capture(image, table, budget, cancelled)
}

#[cfg(test)]
mod alternate_tests;

#[cfg(test)]
mod mixed_event_sources;
