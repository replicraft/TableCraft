//! Normal original-presence INSERT; complete-row TCNA1 behavior is unchanged.
mod proof;

use crate::table::mutation::insert_defaults;
use crate::{EditError, NativeEditPlan, NativeImage, OmittedRowInsert, header::check};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Resolve only omitted ordinary fields from fresh qualified declarations.
    /// Explicit stored NULL is supplied data, never a generation/default marker.
    pub(crate) fn plan_ordinary_omitted_row_inserts(
        self,
        table: &str,
        requests: &[OmittedRowInsert],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let _span = tracing::debug_span!("native.ordinary_insert.omitted.plan").entered();
        let source = super::capture_source(&self, table, budget, cancelled)?;
        if super::incrementing::mode::selected(source.table.schema(), budget, cancelled)? {
            return super::incrementing::plan(self, source, table, requests, budget, cancelled);
        }
        // Qualify the real complete ordinary declaration even for an empty
        // operation, before inspecting a needed default or allocating a row.
        // Generated/calculated/unsupported fields cannot enter through NULL.
        super::records::qualify(source.table.schema(), &[], budget, cancelled)?;
        let resolved =
            insert_defaults::prepare(&self, source.table.schema(), requests, budget, cancelled)?;
        drop(source);
        let scope = self.catalog().catalog().scope();
        let plan = self.plan_ordinary_row_inserts(table, &resolved, budget, cancelled)?;
        proof::prove(&plan, scope, table, requests, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(plan)
    }
}

#[cfg(test)]
mod date_tests;
