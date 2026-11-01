//! Complete original rows with source-derived existing or represented EOF placement.
mod placement;
mod proof;

use crate::{
    EditError, InspectError, NativeEditPlan, NativeImage, RowInsert,
    header::check,
    limits::MAX_LIVE_ROWS,
    table::mutation::{fixed_records, record_insertion::properties},
};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Plan one complete original request dialect. Geometry is derived from
    /// qualified records and original map authority, never caller permission.
    pub(crate) fn plan_ordinary_complete_row_inserts(
        self,
        table: &str,
        requests: &[RowInsert],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let _span = tracing::debug_span!("native.ordinary.complete_insert.plan").entered();
        let source = super::capture_action_source(&self, table, budget, cancelled)?;
        if !source.indexes.indexes().is_empty() {
            // Derive an unavoidable root split from original metadata before
            // selecting one planner. Existing fitting paths remain unchanged.
            if super::root_split::selected(&self, &source, requests)? {
                return super::root_split::plan(self, source, table, requests, budget, cancelled);
            }
            return super::indexed::plan(self, source, table, requests, budget, cancelled);
        }

        source
            .records
            .len()
            .checked_add(requests.len())
            .filter(|count| *count <= MAX_LIVE_ROWS)
            .ok_or(InspectError::StructureLimit)?;
        properties::require_text_declarations(
            &self,
            source.table.schema(),
            requests,
            budget,
            cancelled,
        )?;
        super::records::qualify_complete(source.table.schema(), requests, budget, cancelled)?;
        let encoded = super::records::encode(source.table.schema(), requests, budget, cancelled)?;

        // Every record is qualified and encoded before metadata reservations.
        // Reservations determine the request-order first-fit EOF set, which is
        // certified against the original image before candidate construction.
        let reserved = placement::reserve(&self, &source, &encoded, budget, cancelled)?;
        let mut after = fixed_records::copy(self.as_bytes(), budget, cancelled)?;
        if reserved.new_pages() != 0 {
            reserved.extend_image(
                &self,
                &mut after,
                source.table.schema().definition_page(),
                budget,
                cancelled,
            )?;
        }
        reserved.apply(&self, &encoded, &mut after, budget, cancelled)?;

        let scope = self.catalog().catalog().scope();
        let plan = NativeEditPlan {
            before: self.bytes,
            after,
            changed_rows: requests.len(),
        };
        proof::prove(&plan, scope, table, requests, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(plan)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod scalar_proof_tests;
