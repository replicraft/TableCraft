pub(super) mod joined;
pub(super) mod read_source;

use super::{Binder, Parameter, resolve};
use crate::{QueryError, Source, plan::Expr};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_sql::{
    query::{Query, Statement},
    select::TableSource,
};

pub(crate) struct BoundAssignment {
    pub column: usize,
    pub value: Expr,
}

pub(crate) struct BoundUpdate {
    pub parameters: Vec<Parameter>,
    pub assignments: Vec<BoundAssignment>,
    pub filter: Option<Expr>,
}

pub(crate) fn bind_update(
    query: &Query,
    source: &dyn Source,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<BoundUpdate, QueryError> {
    bind_assignments(query, source, false, false, budget, cancelled, None)
}

pub(crate) fn bind_batch_update(
    query: &Query,
    source: &dyn Source,
    budget: &mut ResourceBudget,
    allow_parameters: bool,
    cancelled: &dyn Fn() -> bool,
) -> Result<BoundUpdate, QueryError> {
    bind_assignments(
        query,
        source,
        true,
        allow_parameters,
        budget,
        cancelled,
        None,
    )
}

fn bind_assignments(
    query: &Query,
    source: &dyn Source,
    multiple: bool,
    allow_parameters: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
    target_columns: Option<read_source::ReadSourceUpdateTarget<'_>>,
) -> Result<BoundUpdate, QueryError> {
    if !allow_parameters && !query.parameters.is_empty() {
        return Err(QueryError::Unsupported);
    }

    let Statement::Update(update) = &query.statement else {
        return Err(QueryError::Unsupported);
    };
    let [table @ TableSource::Table(selected)] = update.sources.as_slice() else {
        return Err(QueryError::Unsupported);
    };
    if let Some(target) = &target_columns {
        if selected.database.is_some() {
            return Err(QueryError::Unsupported);
        }
        if !selected.name.as_str().eq_ignore_ascii_case(target.table) {
            return Err(QueryError::Source);
        }
    }
    if update.assignments.is_empty() || (!multiple && update.assignments.len() != 1) {
        return Err(QueryError::Unsupported);
    }

    if allow_parameters {
        super::mutation_parameters::admit(&query.parameters, Some(budget), cancelled)?;
    }

    let mut binder = Binder::mutation(source, budget, cancelled);

    binder.parameters = binder.bind_parameters(&query.parameters)?;
    binder.parameter_scope = binder.scope_indices(binder.parameters.len())?;

    let relation = binder.relation(
        table,
        &[],
        1,
        crate::external_sources::SourceNamespace::Root,
    )?;

    if let Some(target) = &target_columns {
        let crate::plan::RelationNode::Table {
            columns,
            multivalues,
            members,
            recordsets,
            text_collations,
            ..
        } = &relation.node
        else {
            return Err(QueryError::Unsupported);
        };
        if !multivalues.is_empty() || !members.is_empty() || !recordsets.is_empty() {
            return Err(QueryError::Unsupported);
        }
        binder.work()?;
        if let Some(budget) = &mut binder.budget {
            budget.charge(
                Resource::Steps,
                columns
                    .len()
                    .checked_add(text_collations.len())
                    .ok_or(QueryError::StructureLimit)? as u64,
            )?;
        }
        if columns != target.columns || text_collations != target.text_collations {
            return Err(QueryError::SchemaChanged);
        }
    }

    let count = update.assignments.len();
    let bytes = count
        .checked_mul(size_of::<usize>() + size_of::<BoundAssignment>())
        .ok_or(QueryError::StructureLimit)?;
    binder.charge(bytes)?;
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(count)
        .map_err(|_| QueryError::Allocation)?;
    for assignment in &update.assignments {
        binder.work()?;
        if let Some(budget) = &mut binder.budget {
            // Pay the complete duplicate scan before inspecting prior targets.
            budget.charge(Resource::Steps, columns.len() as u64)?;
        }

        let column =
            resolve(&assignment.target, &relation.slots)?.ok_or(QueryError::UnknownName)?;
        if columns.contains(&column) {
            return Err(QueryError::DuplicateAssignment);
        }
        columns.push(column);
    }

    let scopes = [relation.slots.as_slice()];
    let mut assignments = Vec::new();
    assignments
        .try_reserve_exact(count)
        .map_err(|_| QueryError::Allocation)?;
    for (assignment, column) in update.assignments.iter().zip(columns) {
        let value = binder.expression(&assignment.value, &scopes, false, 1)?;
        if !super::mutation_scalar::admitted(&value) {
            return Err(QueryError::Unsupported);
        }
        assignments.push(BoundAssignment { column, value });
    }

    let filter = update
        .selection
        .as_ref()
        .map(|filter| binder.expression(filter, &scopes, false, 1))
        .transpose()?;
    if let Some(filter) = &filter {
        if target_columns.is_some() {
            super::mutation_reads::admit(filter, &mut binder)?;
        } else if !super::mutation_scalar::admitted(filter) {
            return Err(QueryError::Unsupported);
        }
    }

    Ok(BoundUpdate {
        parameters: binder.parameters,
        assignments,
        filter,
    })
}
