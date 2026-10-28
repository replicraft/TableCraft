//! Load the source before entering filter/group/projection scratch.
use super::{OutputRow, Runtime};
use crate::{
    QueryError,
    execution::{
        Environment,
        cell::CellValue,
        context::{GroupKeys, OuterScope},
    },
    plan::{ReadPlan, RelationNode, SelectPlan},
};

impl Runtime<'_, '_> {
    pub(super) fn select(
        &mut self,
        select: &SelectPlan,
        plan: &ReadPlan,
        outer: &[OuterScope<'_>],
    ) -> Result<Vec<OutputRow>, QueryError> {
        let input = self.select_input(select, outer)?;
        self.finish_select(input, select, plan, outer)
    }

    #[inline(never)]
    fn select_input(
        &mut self,
        select: &SelectPlan,
        outer: &[OuterScope<'_>],
    ) -> Result<Vec<Vec<CellValue>>, QueryError> {
        let constraint = match (&select.source.node, &select.filter) {
            (
                RelationNode::Table {
                    namespace,
                    name,
                    columns,
                    projection,
                    expansion,
                    ..
                },
                Some(filter),
            ) if expansion.is_empty() && select.parent.is_none() => {
                let text_hints = self
                    .namespace_source(*namespace)?
                    .text_equality_hints(name)?;
                if text_hints {
                    self.check_table_schema(&select.source.node)?;
                }
                self.scan_constraint(filter, columns, projection, text_hints)?
            }
            _ => None,
        };
        if let Some(parent) = &select.parent {
            self.parent_rows(&select.source, parent, outer)
        } else {
            self.relation_constrained(&select.source, outer, constraint)
        }
    }

    #[inline(never)]
    pub(in crate::execution) fn finish_select(
        &mut self,
        input: Vec<Vec<CellValue>>,
        select: &SelectPlan,
        plan: &ReadPlan,
        outer: &[OuterScope<'_>],
    ) -> Result<Vec<OutputRow>, QueryError> {
        let mut filtered = Vec::new();
        for row in input {
            self.tick()?;
            if let Some(filter) = &select.filter {
                let value = self.eval(
                    filter,
                    Environment {
                        aggregate_results: None,
                        collections: None,
                        row: &row,
                        outer,
                        output: &[],
                        group: None,
                        group_keys: None,
                    },
                )?;
                if !self.evaluator().predicate_truth(&value)?.is_true() {
                    continue;
                }
            }
            self.push(&mut filtered, row)?;
        }
        let mut result = Vec::new();
        if select.aggregate {
            let groups = self.groups(filtered, select, outer)?;
            for group in groups {
                let env = Environment {
                    aggregate_results: None,
                    collections: None,
                    row: group.rows.first().map_or(&[], Vec::as_slice),
                    outer,
                    output: &[],
                    group: Some(&group.rows),
                    group_keys: Some(GroupKeys {
                        retained_values: false,
                        expressions: &select.group,
                        values: &group.keys,
                    }),
                };
                if let Some(row) = self.project(select, plan, env)? {
                    self.push(&mut result, row)?;
                }
            }
        } else {
            for row in filtered {
                if let Some(row) = self.project(
                    select,
                    plan,
                    Environment {
                        aggregate_results: None,
                        collections: None,
                        row: &row,
                        outer,
                        output: &[],
                        group: None,
                        group_keys: None,
                    },
                )? {
                    self.push(&mut result, row)?;
                }
            }
        }
        Ok(result)
    }
}
