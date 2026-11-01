//! One represented DATA page for complete ordinary INSERT, without index growth.
mod proof;

use crate::{
    EditError, InspectError, NativeEditPlan, NativeImage, RowInsert, WriteBlocker,
    format::page::PAGE_BYTES,
    header::check,
    limits::MAX_LIVE_ROWS,
    table::mutation::{fixed_records, record_insertion::properties},
};
use tablecraft_core::limits::ResourceBudget;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OrdinaryInsertPlanKind {
    Existing,
    AllocateEof,
}

impl NativeImage {
    /// Inspect actual original rows, constraints, encoding and capacity before
    /// choosing one closed planner. No failed physical operation is retried.
    pub(crate) fn ordinary_insert_plan_kind(
        &self,
        table: &str,
        requests: &[RowInsert],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<OrdinaryInsertPlanKind, EditError> {
        let _span = tracing::debug_span!("native.ordinary.insert.select_plan").entered();
        let source = super::capture_source(self, table, budget, cancelled)?;
        if !source.indexes.indexes().is_empty() {
            // This selector grants no indexed admission. The unchanged existing
            // planner still owns complete tree qualification and final proof.
            return Ok(OrdinaryInsertPlanKind::Existing);
        }
        properties::require_text_declarations(
            self,
            source.table.schema(),
            requests,
            budget,
            cancelled,
        )?;
        super::records::qualify(source.table.schema(), requests, budget, cancelled)?;
        let encoded = super::records::encode(source.table.schema(), requests, budget, cancelled)?;
        let reserved =
            super::placement::reserve_growing(self, &source, &encoded, budget, cancelled)?;
        let kind = if reserved.uses_eof() {
            OrdinaryInsertPlanKind::AllocateEof
        } else {
            OrdinaryInsertPlanKind::Existing
        };
        check(budget, cancelled)?;
        Ok(kind)
    }

    /// Plan complete supplied rows with exactly one represented new DATA page.
    /// The normal complete-row fixed-length API retains its original contract.
    pub(crate) fn plan_ordinary_eof_row_inserts(
        self,
        table: &str,
        requests: &[RowInsert],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let _span = tracing::debug_span!("native.ordinary.insert.eof.plan").entered();
        let source = super::capture(&self, table, budget, cancelled)?;
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
        super::records::qualify(source.table.schema(), requests, budget, cancelled)?;
        let encoded = super::records::encode(source.table.schema(), requests, budget, cancelled)?;
        let reserved =
            super::placement::reserve_growing(&self, &source, &encoded, budget, cancelled)?;
        if !reserved.uses_eof() {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let mut after = fixed_records::copy(self.as_bytes(), budget, cancelled)?;
        reserved.extend_image(
            &self,
            &mut after,
            source.table.schema().definition_page(),
            budget,
            cancelled,
        )?;
        reserved.apply(&self, &encoded, &mut after, budget, cancelled)?;
        if self.as_bytes().len().checked_add(PAGE_BYTES) != Some(after.len()) {
            return Err(EditError::Preservation);
        }
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
