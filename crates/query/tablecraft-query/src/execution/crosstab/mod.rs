//! Bounded, buffered row groups and pivot cells. No source-schema discovery I/O.
use crate::execution::cell::CellValue;
mod headings;
use super::{CellQueryResult, Environment, Runtime, context::GroupKeys, retrieval::OutputRow};
use crate::{
    QueryError,
    plan::{CrosstabPlan, ReadNode},
    profile::SOURCE_SCHEMA_POLICY,
};
use tablecraft_core::values::Value;

impl Runtime<'_, '_> {
    pub(super) fn crosstab(&mut self, plan: &CrosstabPlan) -> Result<CellQueryResult, QueryError> {
        if let Some(index) = plan.materialized {
            return self.materialized_crosstab(index);
        }
        let ReadNode::Select(select) = &plan.read.node else {
            return Err(QueryError::Unsupported);
        };
        let initial_pivot_policy = self.text_policy(&plan.pivot)?;
        let mut pivot_policy = initial_pivot_policy;
        let cell = select.projection.last().ok_or(QueryError::Grouping)?;
        let width = select.projection.len() - 1;
        let empty = Environment {
            collections: None,
            row: &[],
            outer: &[],
            output: &[],
            group: None,
            group_keys: None,
        };
        let mut headings = Vec::new();
        if let Some(fixed) = &plan.headings {
            for expr in fixed {
                let value = self.scalar_key(expr, empty)?;
                self.add_heading(&mut headings, &value, width, true, pivot_policy)?;
            }
        }
        let mut rows = Vec::new();
        // Dynamic headings include rejected parents. The callback runs before
        // membership and carries the evaluated pivot once, without rescanning.
        let dynamic = plan.headings.is_none();
        // Default provenance may acquire a field policy only after later rows.
        // Preserve every candidate distinction until that identity is known.
        let deferred_headings = dynamic
            && self.options.collation_resolver.is_some()
            && initial_pivot_policy == crate::source::TextPolicy::Default;
        let input = if let Some(parent) = &select.parent {
            self.parent_rows_before_filter(&select.source, parent, &[], &mut |runtime, row| {
                if !dynamic {
                    return Ok(None);
                }
                let pivot = runtime.repeatable_key(&plan.pivot, Environment { row, ..empty })?;
                pivot_policy = pivot_policy.merge(pivot.policy);
                if deferred_headings {
                    runtime.retain_heading_candidate(&mut headings, &pivot)?;
                } else {
                    runtime.add_heading(&mut headings, &pivot, width, false, pivot_policy)?;
                }
                Ok(Some(pivot))
            })?
        } else {
            // No scalar constraint may prune the pre-WHERE discovery domain.
            self.relation_constrained(&select.source, &[], None)?
        };
        for mut row in input {
            self.tick()?;
            let discovered = if dynamic {
                Some(if select.parent.is_some() {
                    row.pop().ok_or(QueryError::Width)?
                } else {
                    let pivot =
                        self.repeatable_key(&plan.pivot, Environment { row: &row, ..empty })?;
                    pivot_policy = pivot_policy.merge(pivot.policy);
                    if deferred_headings {
                        self.retain_heading_candidate(&mut headings, &pivot)?;
                    } else {
                        self.add_heading(&mut headings, &pivot, width, false, pivot_policy)?;
                    }
                    pivot
                })
            } else {
                None
            };
            let env = Environment { row: &row, ..empty };
            if let Some(filter) = &select.filter
                && !self.eval_predicate(filter, env)?.is_true()
            {
                continue;
            }
            let pivot = match discovered {
                Some(pivot) => pivot,
                None => self.repeatable_key(&plan.pivot, env)?,
            };
            pivot_policy = pivot_policy.merge(pivot.policy);
            self.push(&mut row, pivot)?;
            self.push(&mut rows, row)?;
        }
        if plan.headings.is_none() {
            if deferred_headings {
                headings = self.merge_dynamic_headings(headings, pivot_policy, width)?;
            }
            headings = self.sort_headings(headings, pivot_policy)?;
        } else if pivot_policy != initial_pivot_policy {
            self.check_fixed_heading_policy(&headings, pivot_policy)?;
        }
        let mut columns = Vec::new();
        for name in &plan.read.names[..width] {
            let name = self.copy_heading_name(name)?;
            self.push(&mut columns, name)?;
        }
        for heading in &headings {
            let name = self.heading_name(super::values::scalar(heading)?)?;
            if columns
                .iter()
                .any(|column| SOURCE_SCHEMA_POLICY.names.equals(column, &name))
            {
                return Err(QueryError::AmbiguousName);
            }
            self.push(&mut columns, name)?;
        }
        let mut output = Vec::new();
        for group in self.groups(rows, select, &[])? {
            let group_keys = Some(GroupKeys {
                expressions: &select.group,
                values: &group.keys,
            });
            let env = Environment {
                row: group.rows.first().map_or(&[], Vec::as_slice),
                group: Some(&group.rows),
                group_keys,
                ..empty
            };
            if let Some(having) = &select.having
                && !self.eval_predicate(having, env)?.is_true()
            {
                continue;
            }
            let mut values = self.allocate_expression_row(columns.len())?;
            for expr in &select.projection[..width] {
                values.push(self.retained_expression_operand(expr, env)?);
            }
            let mut keys = self.allocate_expression_row(plan.read.sort.len())?;
            for (expr, _) in &plan.read.sort {
                keys.push(self.comparison_key(
                    expr,
                    Environment {
                        output: &values,
                        ..env
                    },
                )?);
            }
            // Member rows move into one pivot bucket without cloning payloads.
            let mut buckets: Vec<Vec<Vec<CellValue>>> = Vec::new();
            for _ in &headings {
                self.push(&mut buckets, Vec::new())?;
            }
            for row in group.rows {
                let pivot = row.last().ok_or(QueryError::Width)?;
                for (index, heading) in headings.iter().enumerate() {
                    if self.equal(
                        super::values::scalar(pivot)?,
                        super::values::scalar(heading)?,
                        pivot_policy.merge(pivot.policy).merge(heading.policy),
                    )? {
                        self.push(&mut buckets[index], row)?;
                        break;
                    }
                }
            }
            for bucket in &buckets {
                // A missing intersection is NULL, distinct from a nonempty cell
                // whose operands happen to all be NULL.
                if bucket.is_empty() {
                    values.push(CellValue::scalar(Value::Null));
                    continue;
                }
                values.push(self.retained_expression_operand(
                    cell,
                    Environment {
                        row: bucket.first().ok_or(QueryError::Grouping)?.as_slice(),
                        group: Some(bucket),
                        // Cell expressions may reuse row-group keys outside
                        // their aggregate operands. Keep the complete group's
                        // accumulated policies, not only this bucket's rows.
                        group_keys,
                        ..empty
                    },
                )?);
            }
            self.push(
                &mut output,
                OutputRow {
                    values,
                    keys,
                    records: Vec::new(),
                },
            )?;
        }
        let mut output = self.sort_rows(output, &plan.read)?;
        self.apply_top(&mut output, &plan.read)?;
        let mut rows = Vec::new();
        for row in output {
            self.push(&mut rows, row.values)?;
        }
        self.tick()?;
        let mut output_types = self.copy_output_types(&plan.read.output_types[..width])?;
        let cell_type = plan
            .read
            .output_types
            .last()
            .ok_or(QueryError::Grouping)?
            .with_null();
        for _ in width..columns.len() {
            self.push(&mut output_types, cell_type)?;
        }
        Ok(CellQueryResult {
            columns,
            output_types,
            rows,
        })
    }
}

impl Runtime<'_, '_> {
    fn materialized_crosstab(&mut self, index: usize) -> Result<CellQueryResult, QueryError> {
        self.tick()?;
        let materialized = self.discovery.materialized.borrow();
        let cached = materialized.get(index).ok_or(QueryError::SchemaChanged)?;
        let mut columns = Vec::new();
        for name in cached.result.columns() {
            let name = self.copy_heading_name(name)?;
            self.push(&mut columns, name)?;
        }
        let mut rows = Vec::new();
        for row in cached.result.rows() {
            if row.len() != columns.len() {
                return Err(QueryError::Width);
            }
            let mut copied = self.allocate_expression_row(row.len())?;
            for value in row {
                copied.push(self.copy_expression(value)?);
            }
            self.push(&mut rows, copied)?;
        }
        let output_types = self.copy_output_types(cached.result.output_types())?;
        Ok(CellQueryResult {
            columns,
            output_types,
            rows,
        })
    }
}
