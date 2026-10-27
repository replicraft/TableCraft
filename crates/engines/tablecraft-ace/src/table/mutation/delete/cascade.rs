//! Frozen original-association expansion for the separate acyclic cascade service.
mod capture;
mod graph;
mod state;

use super::{Ace16DeletePreview, RowDelete, TableRowDeletes, batch};
use crate::{
    EditError, InspectError, NativeImage, TableCheck, header::check, table::mutation::preview,
};
use tablecraft_core::limits::ResourceBudget;

/// This proof can only be constructed after the cascade component and its complete
/// original tables have passed the action-specific admission and closure checks.
pub(super) struct Proof {
    checks: Vec<TableCheck>,
}

impl Proof {
    pub(super) fn into_checks(self) -> Vec<TableCheck> {
        self.checks
    }
}

pub(super) fn reserve<T>(
    values: &mut Vec<T>,
    budget: &mut ResourceBudget,
) -> Result<(), EditError> {
    budget
        .charge_allocation(1, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    values
        .try_reserve_exact(1)
        .map_err(|_| InspectError::Allocation)?;
    Ok(())
}

impl NativeImage {
    /// Expand original cross-table Cascade associations into one checked candidate.
    ///
    /// This separate engineering service requires enforced known actions. Every
    /// outgoing Cascade lookup uses genuine required primary parent keys and the
    /// existing exact mutation-key carriers. Incoming-only child deletion fires no
    /// lookup and retains the existing full FK/final-membership checks instead.
    /// Reachable Cascade edges must form an acyclic cross-table graph. Self edges,
    /// cyclic cascades, SET NULL, unknown/unenforced rules and active content refuse;
    /// unrelated components do not activate or expand. Every named empty seed still
    /// qualifies its reachable component.
    ///
    /// All explicit raw requests are checked in the original image before diamond
    /// or overlapping-root closure deduplication. Cascade matching uses frozen
    /// original associations; nullable all-NULL child keys contribute no matches.
    /// Restrict edges retain the explicit batch's complete final-image membership
    /// rule. This does not establish per-row mixed-action or native SQL timing.
    ///
    /// Expanded roots share the existing batch table/row ceilings and cumulative
    /// budget. Existing exact record, index, retirement, global-map and per-table
    /// ownership/sibling audits complete before publication. Normal ACE saves and
    /// the RESTRICT-only preview entrypoints remain unchanged.
    pub fn preview_ace16_cascading_deletes(
        self,
        groups: &[TableRowDeletes<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16DeletePreview, EditError> {
        let _span =
            tracing::debug_span!("native.delete.cascade_preview", groups = groups.len()).entered();
        check(budget, cancelled)?;
        preview::qualify(&self)?;
        let mut state = state::State::prepare(&self, groups, budget, cancelled)?;
        state.expand(&self, budget, cancelled)?;
        state.close(budget, cancelled)?;
        let mut checks = Vec::new();
        let mut owned = Vec::new();
        for table in state.tables {
            check(budget, cancelled)?;
            let requests = capture::records(
                &self,
                table.before.schema(),
                &table.selected,
                budget,
                cancelled,
            )?;
            let name = table.before.schema().name();
            budget
                .charge_allocation(name.len() as u64, 1)
                .map_err(InspectError::from)?;
            let mut owned_name = String::new();
            owned_name
                .try_reserve_exact(name.len())
                .map_err(|_| InspectError::Allocation)?;
            owned_name.push_str(name);
            reserve(&mut checks, budget)?;
            reserve(&mut owned, budget)?;
            checks.push(table.before);
            owned.push((owned_name, requests));
        }
        let mut expanded = Vec::new();
        for (name, requests) in &owned {
            reserve(&mut expanded, budget)?;
            expanded.push(TableRowDeletes::new(name, requests));
        }
        batch::run(
            self,
            &expanded,
            batch::Admission::Cascading(Proof { checks }),
            budget,
            cancelled,
        )
    }
}
