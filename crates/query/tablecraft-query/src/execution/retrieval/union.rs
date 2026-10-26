//! Retain only current UNION populations across recursive arm reads.
use super::{OutputRow, Runtime};
use crate::{
    QueryError,
    execution::{Environment, cell::CellValue, context::OuterScope},
    plan::ReadPlan,
    source::TextPolicy,
};

impl Runtime<'_, '_> {
    #[inline(never)]
    pub(super) fn read_union(
        &mut self,
        plan: &ReadPlan,
        first: &ReadPlan,
        rest: &[(bool, ReadPlan)],
        outer: &[OuterScope<'_>],
    ) -> Result<Vec<OutputRow>, QueryError> {
        let mut policies = Vec::new();
        let mut complete = Vec::new();
        if self.options.collation_resolver.is_some() {
            self.budget.charge_allocation(
                first.text_policies.len() as u64,
                size_of::<TextPolicy>() as u64,
            )?;
            policies
                .try_reserve_exact(first.text_policies.len())
                .map_err(|_| QueryError::Allocation)?;
            policies.resize(first.text_policies.len(), TextPolicy::Default);
            self.budget
                .charge_allocation(first.text_policies.len() as u64, size_of::<bool>() as u64)?;
            complete
                .try_reserve_exact(first.text_policies.len())
                .map_err(|_| QueryError::Allocation)?;
            complete.resize(first.text_policies.len(), false);
        }
        let mut rows = self.read(first, outer)?;
        self.promote_union_rows(&mut rows, plan.active_types(self.options.declared_integral))?;
        if !policies.is_empty() {
            self.merge_union_policies(&rows, &first.text_policies, &mut policies, &mut complete)?;
            self.apply_union_policies(&mut rows, &policies, &complete)?;
        }
        for (all, arm) in rest {
            let mut incoming = self.read(arm, outer)?;
            self.promote_union_rows(
                &mut incoming,
                plan.active_types(self.options.declared_integral),
            )?;
            if !policies.is_empty() {
                self.merge_union_policies(
                    &incoming,
                    &arm.text_policies,
                    &mut policies,
                    &mut complete,
                )?;
            }
            for row in incoming {
                self.push(&mut rows, row)?;
            }
            if !policies.is_empty() {
                self.apply_union_policies(&mut rows, &policies, &complete)?;
            }
            if !*all {
                rows = self.unique(
                    rows,
                    &policies,
                    plan.active_types(self.options.declared_integral),
                )?;
            }
        }
        self.union_output_rows(rows, plan, outer)
    }

    #[inline(never)]
    fn union_output_rows(
        &mut self,
        rows: Vec<Vec<CellValue>>,
        plan: &ReadPlan,
        outer: &[OuterScope<'_>],
    ) -> Result<Vec<OutputRow>, QueryError> {
        let mut output = Vec::new();
        for row in rows {
            let env = Environment {
                aggregate_results: None,
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
        Ok(output)
    }
}
