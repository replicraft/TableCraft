use super::{Environment, Runtime};
use crate::execution::cell::CellValue;
use crate::execution::context::OuterScope;
use crate::{
    QueryError,
    plan::{ReadNode, ReadPlan, RelationNode, SelectPlan},
};
use std::cmp::Ordering;

pub(super) struct OutputRow {
    pub(super) values: Vec<CellValue>,
    pub(super) keys: Vec<CellValue>,
    pub(super) records: Vec<CellValue>,
}
pub(super) struct Group {
    pub(super) keys: Vec<CellValue>,
    pub(super) rows: Vec<Vec<CellValue>>,
}
impl Runtime<'_, '_> {
    pub(super) fn read(
        &mut self,
        plan: &ReadPlan,
        outer: &[OuterScope<'_>],
    ) -> Result<Vec<Vec<CellValue>>, QueryError> {
        self.tick()?;
        let mut rows = match &plan.node {
            ReadNode::Crosstab(cross) => {
                let result = self.crosstab(cross)?;
                if result.columns.len() != plan.names.len() {
                    return Err(QueryError::SchemaChanged);
                }
                for (actual, expected) in result.columns.iter().zip(&plan.names) {
                    self.tick()?;
                    self.budget.charge(
                        tablecraft_core::limits::Resource::Steps,
                        (actual.len() + expected.len()) as u64,
                    )?;
                    if actual != expected {
                        return Err(QueryError::SchemaChanged);
                    }
                }
                // Inner crosstab already evaluated every cell, sorted and
                // applied its TOP. Outer consumers retain all resulting cells.
                return Ok(result.rows);
            }
            ReadNode::Select(select) => self.select(select, plan, outer)?,
            ReadNode::Union { first, rest } => {
                let mut policies = Vec::new();
                if self.options.collation_resolver.is_some() {
                    self.budget.charge_allocation(
                        first.text_policies.len() as u64,
                        size_of::<crate::source::TextPolicy>() as u64,
                    )?;
                    policies
                        .try_reserve_exact(first.text_policies.len())
                        .map_err(|_| QueryError::Allocation)?;
                    policies.extend_from_slice(&first.text_policies);
                }
                let mut rows = self.read(first, outer)?;
                self.promote_union_rows(&mut rows, &plan.output_types)?;
                for (all, arm) in rest {
                    for (policy, other) in policies.iter_mut().zip(&arm.text_policies) {
                        *policy = policy.merge(*other);
                    }
                    let mut incoming = self.read(arm, outer)?;
                    self.promote_union_rows(&mut incoming, &plan.output_types)?;
                    for row in incoming {
                        self.push(&mut rows, row)?;
                    }
                    if !*all {
                        rows = self.unique(rows, &policies)?;
                    }
                }
                let mut output = Vec::new();
                for row in rows {
                    let env = Environment {
                        collections: None,
                        row: &row,
                        outer,
                        output: &row,
                        group: None,
                        group_keys: None,
                    };
                    let mut keys = self.allocate_expression_row(plan.sort.len())?;
                    for (expr, _) in &plan.sort {
                        keys.push(self.comparison_key(expr, env)?);
                    }
                    self.push(
                        &mut output,
                        OutputRow {
                            values: row,
                            keys,
                            records: Vec::new(),
                        },
                    )?;
                }
                output
            }
        };
        let records =
            matches!(&plan.node, ReadNode::Select(select) if !select.record_keys.is_empty());
        if plan.distinct || records {
            if records {
                self.unify_cell_policies(&mut rows, |row| &mut row.records)?;
            } else {
                self.unify_cell_policies(&mut rows, |row| &mut row.values)?;
            }
            let record_policies = if let ReadNode::Select(select) = &plan.node {
                if records {
                    self.expression_policies(&select.record_keys)?
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };
            let policies = if records {
                record_policies.as_slice()
            } else {
                &plan.text_policies
            };
            let mut unique: Vec<OutputRow> = Vec::new();
            for row in rows {
                let mut duplicate = false;
                for other in &unique {
                    let same = if records {
                        self.same_expression_row(&row.records, &other.records, policies)?
                    } else {
                        self.same_expression_row(&row.values, &other.values, policies)?
                    };
                    if same {
                        duplicate = true;
                        break;
                    }
                }
                if !duplicate {
                    self.push(&mut unique, row)?;
                }
            }
            rows = unique;
        }
        rows = self.sort_rows(rows, plan)?;
        self.apply_top(&mut rows, plan)?;
        let mut result = Vec::new();
        for row in rows {
            self.push(&mut result, row.values)?;
        }
        Ok(result)
    }
    fn select(
        &mut self,
        select: &SelectPlan,
        plan: &ReadPlan,
        outer: &[OuterScope<'_>],
    ) -> Result<Vec<OutputRow>, QueryError> {
        let mut filtered = Vec::new();
        let constraint = match (&select.source.node, &select.filter) {
            (
                RelationNode::Table {
                    name,
                    columns,
                    projection,
                    expansion,
                    ..
                },
                Some(filter),
            ) if expansion.is_empty() && select.parent.is_none() => {
                let text_hints = self.source.text_equality_hints(name)?;
                if text_hints {
                    self.check_table_schema(&select.source.node)?;
                }
                self.scan_constraint(filter, columns, projection, text_hints)?
            }
            _ => None,
        };
        let input = if let Some(parent) = &select.parent {
            self.parent_rows(&select.source, parent, outer)?
        } else {
            self.relation_constrained(&select.source, outer, constraint)?
        };
        for row in input {
            self.tick()?;
            if let Some(filter) = &select.filter {
                let value = self.eval(
                    filter,
                    Environment {
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
                    collections: None,
                    row: group.rows.first().map_or(&[], Vec::as_slice),
                    outer,
                    output: &[],
                    group: Some(&group.rows),
                    group_keys: Some(super::context::GroupKeys {
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
    fn project(
        &mut self,
        select: &SelectPlan,
        plan: &ReadPlan,
        env: Environment<'_>,
    ) -> Result<Option<OutputRow>, QueryError> {
        if let Some(having) = &select.having
            && !self.eval_predicate(having, env)?.is_true()
        {
            return Ok(None);
        }
        let mut values = self.allocate_expression_row(select.projection.len())?;
        for expr in &select.projection {
            values.push(self.retained_expression_operand(expr, env)?);
        }
        let mut keys = self.allocate_expression_row(plan.sort.len())?;
        for (expr, _) in &plan.sort {
            keys.push(self.comparison_key(
                expr,
                Environment {
                    output: &values,
                    ..env
                },
            )?);
        }
        let mut records = self.allocate_expression_row(select.record_keys.len())?;
        for expr in &select.record_keys {
            records.push(self.comparison_key(expr, env)?);
        }
        Ok(Some(OutputRow {
            values,
            keys,
            records,
        }))
    }
    pub(super) fn groups(
        &mut self,
        rows: Vec<Vec<CellValue>>,
        select: &SelectPlan,
        outer: &[OuterScope<'_>],
    ) -> Result<Vec<Group>, QueryError> {
        if select.group.is_empty() {
            let mut groups = Vec::new();
            self.push(
                &mut groups,
                Group {
                    keys: Vec::new(),
                    rows,
                },
            )?;
            return Ok(groups);
        }
        let policies = self.expression_policies(&select.group)?;
        let mut groups: Vec<Group> = Vec::new();
        for row in rows {
            let mut keys = self.allocate_expression_row(select.group.len())?;
            for expr in &select.group {
                keys.push(self.repeatable_comparison_key(
                    expr,
                    Environment {
                        collections: None,
                        row: &row,
                        outer,
                        output: &[],
                        group: None,
                        group_keys: None,
                    },
                )?);
            }
            let mut index = None;
            for (i, group) in groups.iter().enumerate() {
                if self.same_expression_row(&keys, &group.keys, &policies)? {
                    index = Some(i);
                    break;
                }
            }
            if let Some(i) = index {
                for (retained, incoming) in groups[i].keys.iter_mut().zip(&keys) {
                    retained.policy = retained.policy.merge(incoming.policy);
                }
                self.push(&mut groups[i].rows, row)?;
            } else {
                let mut members = Vec::new();
                self.push(&mut members, row)?;
                self.push(
                    &mut groups,
                    Group {
                        keys,
                        rows: members,
                    },
                )?;
            }
        }
        Ok(groups)
    }
    fn unique(
        &mut self,
        mut rows: Vec<Vec<CellValue>>,
        policies: &[crate::source::TextPolicy],
    ) -> Result<Vec<Vec<CellValue>>, QueryError> {
        self.unify_cell_policies(&mut rows, Vec::as_mut_slice)?;
        let mut result: Vec<Vec<CellValue>> = Vec::new();
        for row in rows {
            let mut duplicate = false;
            for other in &result {
                if self.same_expression_row(&row, other, policies)? {
                    duplicate = true;
                    break;
                }
            }
            if !duplicate {
                self.push(&mut result, row)?;
            }
        }
        Ok(result)
    }
    pub(super) fn compare_keys(
        &mut self,
        left: &[CellValue],
        right: &[CellValue],
        plan: &ReadPlan,
    ) -> Result<Ordering, QueryError> {
        self.tick()?;
        for ((left, right), (expr, descending)) in left.iter().zip(right).zip(&plan.sort) {
            let policy = self
                .text_policy(expr)?
                .merge(left.policy)
                .merge(right.policy);
            let order = self.order_key(left, right, policy)?;
            if order != Ordering::Equal {
                return Ok(if *descending { order.reverse() } else { order });
            }
        }
        Ok(Ordering::Equal)
    }
}
