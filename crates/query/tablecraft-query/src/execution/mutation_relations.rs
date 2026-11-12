//! Joined scalar frames keep the selected occurrence's original handle separately.
use crate::execution::cell::CellValue;
use crate::{
    QueryError,
    execution::{Environment, Runtime},
    plan::{Relation, RelationNode},
};
use tablecraft_core::limits::Resource;
use tablecraft_db::{ProjectedRow, RowHandle, Snapshot};
use tablecraft_sql::select::JoinKind;

pub(super) fn frames(
    runtime: &mut Runtime<'_, '_>,
    relation: &Relation,
    target: usize,
    snapshot: &Snapshot,
    leaf: &mut usize,
) -> Result<Vec<Frame>, QueryError> {
    runtime.tick()?;
    match &relation.node {
        RelationNode::Table {
            columns,
            projection,
            ..
        } => {
            runtime.check_table_schema(&relation.node)?;
            let selected = *leaf == target;
            *leaf = leaf.checked_add(1).ok_or(QueryError::StructureLimit)?;
            let mut frames = Vec::new();
            if selected {
                for (ordinal, row) in snapshot.rows().iter().enumerate() {
                    runtime.tick()?;
                    let view =
                        ProjectedRow::select(row, projection).map_err(|_| QueryError::Source)?;
                    let cells = runtime.copy_projected_row(columns, projection, view)?;
                    let handle = snapshot.row_handle(ordinal).ok_or(QueryError::Source)?;
                    push(
                        runtime,
                        &mut frames,
                        Frame {
                            cells,
                            target: Some(handle),
                        },
                    )?;
                }
            } else {
                for cells in runtime.relation(relation, &[])? {
                    push(
                        runtime,
                        &mut frames,
                        Frame {
                            cells,
                            target: None,
                        },
                    )?;
                }
            }
            Ok(frames)
        }
        RelationNode::Join {
            left,
            right,
            kind,
            condition,
        } => join(
            runtime,
            left,
            right,
            Some((*kind, condition)),
            target,
            snapshot,
            leaf,
        ),
        RelationNode::Cross(left, right) => {
            join(runtime, left, right, None, target, snapshot, leaf)
        }
        _ => Err(QueryError::Unsupported),
    }
}

fn join(
    runtime: &mut Runtime<'_, '_>,
    left: &Relation,
    right: &Relation,
    join: Option<(JoinKind, &crate::plan::Expr)>,
    target: usize,
    snapshot: &Snapshot,
    leaf: &mut usize,
) -> Result<Vec<Frame>, QueryError> {
    let lhs = frames(runtime, left, target, snapshot, leaf)?;
    let rhs = frames(runtime, right, target, snapshot, leaf)?;
    runtime
        .budget
        .charge_allocation(rhs.len() as u64, size_of::<bool>() as u64)?;
    let mut matched = Vec::new();
    matched
        .try_reserve_exact(rhs.len())
        .map_err(|_| QueryError::Allocation)?;
    matched.resize(rhs.len(), false);
    let mut output = Vec::new();
    for left in &lhs {
        let mut hit = false;
        for (position, right) in rhs.iter().enumerate() {
            runtime.tick()?;
            let cells = runtime.join_row(&left.cells, &right.cells)?;
            let accepted = match join {
                Some((_, condition)) => runtime
                    .eval_predicate(
                        condition,
                        Environment {
                            aggregate_results: None,
                            collections: None,
                            row: &cells,
                            outer: &[],
                            output: &[],
                            group: None,
                            group_keys: None,
                        },
                    )?
                    .is_true(),
                None => true,
            };
            if accepted {
                if left.target.is_some() && right.target.is_some() {
                    return Err(QueryError::Source);
                }
                hit = true;
                matched[position] = true;
                push(
                    runtime,
                    &mut output,
                    Frame {
                        cells,
                        target: left.target.or(right.target),
                    },
                )?;
            }
        }
        if !hit && join.is_some_and(|(kind, _)| kind == JoinKind::Left) {
            let cells = runtime.join_missing(&left.cells, right.row_width(), false)?;
            push(
                runtime,
                &mut output,
                Frame {
                    cells,
                    target: left.target,
                },
            )?;
        }
    }
    if join.is_some_and(|(kind, _)| kind == JoinKind::Right) {
        for (position, right) in rhs.iter().enumerate() {
            runtime.tick()?;
            if !matched[position] {
                let cells = runtime.join_missing(&right.cells, left.row_width(), true)?;
                push(
                    runtime,
                    &mut output,
                    Frame {
                        cells,
                        target: right.target,
                    },
                )?;
            }
        }
    }
    Ok(output)
}

pub(super) struct Frame {
    pub(super) cells: Vec<CellValue>,
    pub(super) target: Option<RowHandle>,
}

pub(super) fn push<T>(
    runtime: &mut Runtime<'_, '_>,
    values: &mut Vec<T>,
    value: T,
) -> Result<(), QueryError> {
    runtime.tick()?;
    if values.len() == values.capacity() {
        let added = values.capacity().max(1);
        runtime
            .budget
            .charge(Resource::Steps, values.len() as u64)?;
        runtime
            .budget
            .charge_allocation(added as u64, size_of::<T>() as u64)?;
        values
            .try_reserve_exact(added)
            .map_err(|_| QueryError::Allocation)?;
    }
    values.push(value);
    Ok(())
}
