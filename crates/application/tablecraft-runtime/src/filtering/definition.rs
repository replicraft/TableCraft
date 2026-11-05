//! Explicit fresh-source reapplication of preferences, never old row identities.
use super::{
    Criterion, DefinitionState, FILTER_STEPS, FilterDefinition, FilterError, FilterHandle,
    FilterState, FilterTarget, MAX_FILTER_FIELDS, RETAINED_BYTES, criteria, issue_id,
};
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check, comparison_budget},
    document::Document,
};
use std::{cell::RefCell, sync::Arc};
use tablecraft_access::{NativeBrowser, NativeCollationResolver};
use tablecraft_ace::{inspect_database_sort_order, inspect_table_schema};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn create(
    table: &str,
    criteria: Vec<Criterion>,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<FilterDefinition, Error> {
    let table = criteria::copy_text(table, work, cancelled)?;
    work.charge_allocation(1, size_of::<DefinitionState>() as u64)?;
    Ok(FilterDefinition {
        state: Arc::new(DefinitionState { table, criteria }),
    })
}

impl Document {
    pub(crate) fn reapply_filter(
        &mut self,
        definition: &FilterDefinition,
        document_revision: u64,
        enabled: bool,
        offset: u32,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        check(cancelled)?;
        if document_revision != self.revision {
            return Err(Error::StaleDocument);
        }
        if definition.state.criteria.is_empty()
            || definition.state.criteria.len() > MAX_FILTER_FIELDS
        {
            return Err(FilterError::CriterionLimit.into());
        }

        let mut native_work = budget();
        let schema = inspect_table_schema(
            &mut self.source,
            SCOPE,
            definition.table(),
            &mut native_work,
            cancelled,
        )?;
        let order = inspect_database_sort_order(&mut self.source, &mut native_work, cancelled)?;
        let comparisons = RefCell::new(comparison_budget());
        let resolver =
            NativeCollationResolver::new_deferred_default(order, &comparisons, cancelled)?;
        let mut work = ResourceBudget::new(RETAINED_BYTES, 0, FILTER_STEPS);
        work.charge_allocation(
            definition.state.criteria.len() as u64,
            size_of::<Criterion>() as u64,
        )?;
        let mut predicates = Vec::new();
        predicates.try_reserve_exact(definition.state.criteria.len())?;
        for predicate in &definition.state.criteria {
            predicates.push(criteria::copy(predicate, &mut work, cancelled)?);
        }

        let mut browser = NativeBrowser::new(&mut self.source, SCOPE, native_work, cancelled);
        let (ordinals, source_rows) = super::scan::membership(
            &mut browser,
            &schema,
            &predicates,
            &resolver,
            &mut work,
            cancelled,
        )?;
        let table = criteria::copy_text(definition.table(), &mut work, cancelled)?;
        work.charge_allocation(1, size_of::<FilterState>() as u64)?;
        let handle = FilterHandle {
            state: Arc::new(FilterState {
                source: Arc::clone(&self.filter_source),
                target: FilterTarget {
                    document_revision,
                    table,
                    filter_id: issue_id()?,
                },
                definition: create(definition.table(), predicates, &mut work, cancelled)?,
                ordinals,
                source_rows,
            }),
        };
        let count = if enabled {
            handle.matched_rows()
        } else {
            handle.source_rows()
        };
        self.filter_page(
            &handle,
            offset.min(count.saturating_sub(1)),
            enabled,
            cancelled,
        )
    }
}
