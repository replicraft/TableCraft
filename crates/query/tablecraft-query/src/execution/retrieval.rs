mod crosstab;
mod finish;
mod select;
mod union;

#[cfg(test)]
mod tests;

use super::{Environment, Runtime};
use crate::execution::cell::CellValue;
use crate::execution::context::OuterScope;
use crate::{
    QueryError,
    plan::{ReadNode, ReadPlan, SelectPlan},
    profile::MAX_RUNTIME_READ_DEPTH,
};
use std::cmp::Ordering;

pub(super) struct OutputRow {
    pub(super) values: Vec<CellValue>,
    pub(super) keys: Vec<CellValue>,
    pub(super) records: Vec<CellValue>,
}
impl Runtime<'_, '_> {
    pub(super) fn read(
        &mut self,
        plan: &ReadPlan,
        outer: &[OuterScope<'_>],
    ) -> Result<Vec<Vec<CellValue>>, QueryError> {
        self.tick()?;
        let read_depth = self.discovery.active_read_depth.get();
        if read_depth >= MAX_RUNTIME_READ_DEPTH {
            return Err(QueryError::StructureLimit);
        }

        let previous = std::mem::replace(&mut self.owner, plan.owner);
        self.discovery.active_read_depth.set(read_depth + 1);
        let result = self.read_owned(plan, outer);
        self.discovery.active_read_depth.set(read_depth);
        self.owner = previous;
        result
    }

    fn read_owned(
        &mut self,
        plan: &ReadPlan,
        outer: &[OuterScope<'_>],
    ) -> Result<Vec<Vec<CellValue>>, QueryError> {
        let rows = match &plan.node {
            ReadNode::Crosstab(cross) => return self.read_crosstab(cross, plan),
            ReadNode::Select(select) => self.select(select, plan, outer)?,
            ReadNode::Union { first, rest } => self.read_union(plan, first, rest, outer)?,
        };
        self.finish_read(rows, plan)
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
            values.push(self.retained_expression_operand(
                expr,
                Environment {
                    output: &values,
                    ..env
                },
            )?);
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
    fn unique(
        &mut self,
        mut rows: Vec<Vec<CellValue>>,
        policies: &[crate::source::TextPolicy],
        types: &[crate::OutputType],
    ) -> Result<Vec<Vec<CellValue>>, QueryError> {
        self.unify_cell_policies(&mut rows, Vec::as_mut_slice)?;
        let mut lookup = if rows.is_empty() {
            None
        } else {
            self.exact_output_keys(types)?
        };
        let mut result: Vec<Vec<CellValue>> = Vec::new();
        for row in rows {
            self.validate_key_row(&row)?;
            let (representative, fingerprint) =
                self.exact_key_match(&mut lookup, &row, &result, Vec::as_slice, policies)?;
            if representative.is_none() {
                let position = result.len();
                self.push(&mut result, row)?;
                if let (Some(lookup), Some(fingerprint)) = (&mut lookup, fingerprint) {
                    lookup.insert(fingerprint, position, self.budget, self.options.cancelled)?;
                }
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
