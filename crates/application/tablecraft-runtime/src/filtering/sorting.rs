//! Native view sorting retains source identity, filter preferences and off state.
mod order;

use super::{
    Criterion, FILTER_STEPS, FilterDefinition, FilterError, FilterHandle, FilterState,
    FilterTarget, RETAINED_BYTES, SortSpec, SortTarget, criteria, definition, issue_id, scan,
};
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check, comparison_budget},
    document::Document,
};
use std::{cell::RefCell, sync::Arc};
use tablecraft_access::{NativeBrowser, NativeCollationResolver};
use tablecraft_ace::{inspect_database_sort_order, inspect_table_schema};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::TypeDescriptor;

pub(super) struct SortIntent {
    pub(super) spec: SortSpec,
    pub(super) expected: Option<TypeDescriptor>,
}

pub(super) struct ViewBuild {
    pub(super) table: String,
    pub(super) predicates: Vec<Criterion>,
    pub(super) sort: Option<SortIntent>,
    pub(super) ordinals: Vec<u32>,
    pub(super) source_rows: u32,
    pub(super) enabled: bool,
    pub(super) offset: u32,
}

pub(super) fn inherited_sort(
    definition: &FilterDefinition,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<SortIntent>, Error> {
    definition
        .state
        .sort
        .as_ref()
        .map(|sort| {
            Ok(SortIntent {
                spec: SortSpec {
                    column: criteria::copy_text(&sort.spec.column, work, cancelled)?,
                    descending: sort.spec.descending,
                },
                expected: Some(sort.declaration),
            })
        })
        .transpose()
}

fn copied_members(
    handle: &FilterHandle,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u32>, Error> {
    work.charge_allocation(handle.state.ordinals.len() as u64, size_of::<u32>() as u64)?;
    let mut members = Vec::new();
    members.try_reserve_exact(handle.state.ordinals.len())?;
    for &ordinal in &handle.state.ordinals {
        check(cancelled)?;
        work.charge(Resource::Steps, 1)?;
        members.push(ordinal);
    }
    Ok(members)
}

impl Document {
    pub(crate) fn sort_view(
        &mut self,
        target: &SortTarget,
        sort: Option<&SortSpec>,
        base: Option<&FilterHandle>,
        enabled: bool,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        check(cancelled)?;
        if target.document_revision != self.revision {
            return Err(Error::StaleDocument);
        }
        match (base, target.filter_id) {
            (None, None) => {}
            (Some(handle), Some(id))
                if handle.target().filter_id == id && handle.target().table == target.table =>
            {
                self.check_filter(handle)?
            }
            _ => return Err(FilterError::StaleHandle.into()),
        }
        let mut work = ResourceBudget::new(RETAINED_BYTES, 0, FILTER_STEPS);
        let mut predicates = Vec::new();
        let (ordinals, source_rows) = if let Some(handle) = base {
            work.charge_allocation(
                handle.state.definition.state.criteria.len() as u64,
                size_of::<Criterion>() as u64,
            )?;
            predicates.try_reserve_exact(handle.state.definition.state.criteria.len())?;
            for criterion in &handle.state.definition.state.criteria {
                predicates.push(criteria::copy(criterion, &mut work, cancelled)?);
            }
            (
                copied_members(handle, &mut work, cancelled)?,
                handle.source_rows(),
            )
        } else {
            let mut native = budget();
            let schema = inspect_table_schema(
                &mut self.source,
                SCOPE,
                &target.table,
                &mut native,
                cancelled,
            )?;
            let database_order =
                inspect_database_sort_order(&mut self.source, &mut native, cancelled)?;
            let comparisons = RefCell::new(comparison_budget());
            let resolver = NativeCollationResolver::new_deferred_default(
                database_order,
                &comparisons,
                cancelled,
            )?;
            let mut browser = NativeBrowser::new(&mut self.source, SCOPE, native, cancelled);
            scan::membership(
                &mut browser,
                &schema,
                &predicates,
                &resolver,
                &mut work,
                cancelled,
            )?
        };
        let sort = sort
            .map(|sort| -> Result<SortIntent, Error> {
                Ok(SortIntent {
                    spec: SortSpec {
                        column: criteria::copy_text(&sort.column, &mut work, cancelled)?,
                        descending: sort.descending,
                    },
                    expected: None,
                })
            })
            .transpose()?;
        let table = criteria::copy_text(&target.table, &mut work, cancelled)?;
        self.publish_view(
            ViewBuild {
                table,
                predicates,
                sort,
                ordinals,
                source_rows,
                enabled,
                offset: 0,
            },
            &mut work,
            cancelled,
        )
    }

    pub(crate) fn clear_filter(
        &mut self,
        handle: &FilterHandle,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        self.check_filter(handle)?;
        let mut work = ResourceBudget::new(RETAINED_BYTES, 0, FILTER_STEPS);
        work.charge_allocation(handle.source_rows() as u64, size_of::<u32>() as u64)?;
        let mut ordinals = Vec::new();
        ordinals.try_reserve_exact(handle.source_rows() as usize)?;
        for ordinal in 0..handle.source_rows() {
            check(cancelled)?;
            work.charge(Resource::Steps, 1)?;
            ordinals.push(ordinal);
        }
        let sort = inherited_sort(&handle.state.definition, &mut work, cancelled)?;
        let table = criteria::copy_text(&handle.target().table, &mut work, cancelled)?;
        self.publish_view(
            ViewBuild {
                table,
                predicates: Vec::new(),
                sort,
                ordinals,
                source_rows: handle.source_rows(),
                enabled: false,
                offset: 0,
            },
            &mut work,
            cancelled,
        )
    }

    pub(super) fn filter_for_mode(
        &mut self,
        handle: &FilterHandle,
        enabled: bool,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<FilterHandle, Error> {
        self.check_filter(handle)?;
        let Some(sort) = &handle.state.definition.state.sort else {
            return Ok(handle.clone());
        };
        if handle
            .state
            .order
            .as_ref()
            .is_some_and(|order| order.enabled == enabled)
        {
            return Ok(handle.clone());
        }

        // A mode change is a new bounded computation, never a mutation of the
        // published handle. Failed or cancelled ordering leaves that view valid.
        let mut retained = ResourceBudget::new(RETAINED_BYTES, 0, FILTER_STEPS);
        let ordinals = copied_members(handle, &mut retained, cancelled)?;
        let mut predicates = Vec::new();
        retained.charge_allocation(
            handle.state.definition.state.criteria.len() as u64,
            size_of::<Criterion>() as u64,
        )?;
        predicates.try_reserve_exact(handle.state.definition.state.criteria.len())?;
        for criterion in &handle.state.definition.state.criteria {
            predicates.push(criteria::copy(criterion, &mut retained, cancelled)?);
        }
        let (bound, order) = order::build(
            &mut self.source,
            order::SortInput {
                table: &handle.target().table,
                spec: &sort.spec,
                expected: Some(sort.declaration),
                members: &ordinals,
                source_rows: handle.source_rows(),
                enabled,
            },
            &mut retained,
            cancelled,
        )?;
        let table = criteria::copy_text(&handle.target().table, &mut retained, cancelled)?;
        let definition =
            definition::create(&table, predicates, Some(bound), &mut retained, cancelled)?;
        retained.charge_allocation(1, size_of::<FilterState>() as u64)?;
        Ok(FilterHandle {
            state: Arc::new(FilterState {
                source: Arc::clone(&handle.state.source),
                target: FilterTarget {
                    document_revision: handle.target().document_revision,
                    table,
                    filter_id: handle.target().filter_id,
                },
                definition,
                ordinals,
                source_rows: handle.source_rows(),
                order: Some(order),
            }),
        })
    }

    pub(super) fn publish_view(
        &mut self,
        input: ViewBuild,
        work: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        let enabled = input.enabled && !input.predicates.is_empty();
        let (sort, order) = match input.sort {
            Some(sort) => {
                let (bound, order) = order::build(
                    &mut self.source,
                    order::SortInput {
                        table: &input.table,
                        spec: &sort.spec,
                        expected: sort.expected,
                        members: &input.ordinals,
                        source_rows: input.source_rows,
                        enabled,
                    },
                    work,
                    cancelled,
                )?;
                (Some(bound), Some(order))
            }
            None => (None, None),
        };
        work.charge_allocation(1, size_of::<FilterState>() as u64)?;
        let definition = definition::create(&input.table, input.predicates, sort, work, cancelled)?;
        let handle = FilterHandle {
            state: Arc::new(FilterState {
                source: Arc::clone(&self.filter_source),
                target: FilterTarget {
                    document_revision: self.revision,
                    table: input.table,
                    filter_id: issue_id()?,
                },
                definition,
                ordinals: input.ordinals,
                source_rows: input.source_rows,
                order,
            }),
        };
        let count = if enabled {
            handle.matched_rows()
        } else {
            handle.source_rows()
        };
        self.filter_page(
            &handle,
            input.offset.min(count.saturating_sub(1)),
            enabled,
            cancelled,
        )
    }
}
