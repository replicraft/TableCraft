//! Qualify one typed selection, then publish complete immutable membership only.
use super::{
    Criterion, FILTER_STEPS, FilterComparison, FilterError, FilterHandle, FilterSelectionTarget,
    FilterState, FilterTarget, MAX_FILTER_FIELDS, RETAINED_BYTES, criteria, issue_id,
};
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check, comparison_budget},
    document::Document,
};
use std::{cell::RefCell, sync::Arc};
use tablecraft_access::{NativeBrowser, NativeCollationResolver, TablePageRequest};
use tablecraft_ace::{inspect_database_sort_order, inspect_table_schema};
use tablecraft_core::limits::{Resource, ResourceBudget};

impl Document {
    pub(crate) fn filter_selection(
        &mut self,
        target: &FilterSelectionTarget,
        comparison: FilterComparison,
        base: Option<&FilterHandle>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        check(cancelled)?;
        if target.document_revision != self.revision {
            return Err(Error::StaleDocument);
        }

        match (base, target.filter_id) {
            (None, None) => {}
            (Some(base), Some(id)) => {
                self.check_filter(base)?;
                if base.target().filter_id != id
                    || base.target().table != target.table
                    || base
                        .state
                        .ordinals
                        .binary_search(&target.parent_ordinal)
                        .is_err()
                {
                    return Err(FilterError::InvalidSelection.into());
                }
            }
            _ => return Err(FilterError::StaleHandle.into()),
        }

        let mut native_work = budget();
        let schema = inspect_table_schema(
            &mut self.source,
            SCOPE,
            &target.table,
            &mut native_work,
            cancelled,
        )?;
        let selected_position =
            criteria::field_position(&schema, &target.column, &mut native_work, cancelled)?;
        let selected = &schema.columns()[selected_position];
        criteria::require_field(selected)?;

        let order = inspect_database_sort_order(&mut self.source, &mut native_work, cancelled)?;
        let comparisons = RefCell::new(comparison_budget());
        let resolver =
            NativeCollationResolver::new_deferred_default(order, &comparisons, cancelled)?;
        let mut work = ResourceBudget::new(RETAINED_BYTES, 0, FILTER_STEPS);
        work.charge_allocation(MAX_FILTER_FIELDS as u64, size_of::<Criterion>() as u64)?;
        let mut predicates = Vec::new();
        predicates.try_reserve_exact(MAX_FILTER_FIELDS)?;
        if let Some(base) = base {
            for criterion in &base.state.definition.state.criteria {
                check(cancelled)?;
                work.charge(
                    Resource::Steps,
                    1 + criterion.column.len() as u64 + target.column.len() as u64,
                )?;
                if criterion.column != target.column {
                    predicates.push(criteria::copy(criterion, &mut work, cancelled)?);
                }
            }
        }
        if predicates.len() == MAX_FILTER_FIELDS {
            return Err(FilterError::CriterionLimit.into());
        }

        let mut browser = NativeBrowser::new(&mut self.source, SCOPE, native_work, cancelled);
        let selected_page = browser.projected_page(
            &target.table,
            &[&target.column],
            TablePageRequest::new(target.parent_ordinal, 1)?,
        )?;
        let selected_value = selected_page
            .rows()
            .first()
            .and_then(|row| row.first())
            .ok_or(FilterError::InvalidSelection)?;
        predicates.push(criteria::selected(
            &target.column,
            selected_value,
            comparison,
            selected_page
                .columns()
                .first()
                .ok_or(FilterError::InvalidSelection)?
                .type_descriptor(),
            &mut work,
            cancelled,
        )?);
        drop(selected_page);

        let (ordinals, source_rows) = super::scan::membership(
            &mut browser,
            &schema,
            &predicates,
            &resolver,
            &mut work,
            cancelled,
        )?;
        check(cancelled)?;

        let table = criteria::copy_text(&target.table, &mut work, cancelled)?;
        work.charge_allocation(1, size_of::<FilterState>() as u64)?;
        let handle = FilterHandle {
            state: Arc::new(FilterState {
                source: Arc::clone(&self.filter_source),
                target: FilterTarget {
                    document_revision: self.revision,
                    table,
                    filter_id: issue_id()?,
                },
                definition: super::definition::create(
                    &target.table,
                    predicates,
                    &mut work,
                    cancelled,
                )?,
                ordinals,
                source_rows,
            }),
        };
        self.filter_page(&handle, 0, true, cancelled)
    }

    pub(crate) fn check_filter(&self, handle: &FilterHandle) -> Result<(), Error> {
        if handle.target().document_revision != self.revision
            || !Arc::ptr_eq(&handle.state.source, &self.filter_source)
        {
            return Err(FilterError::StaleHandle.into());
        }
        Ok(())
    }
}
