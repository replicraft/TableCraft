//! Group membership uses complete key-column provenance before partitioning.
use super::{Environment, Runtime, cell::CellValue, context::OuterScope, exact_keys::ExactKeys};
use crate::{QueryError, plan::SelectPlan, source::TextPolicy};

pub(super) struct Group {
    pub(super) keys: Vec<CellValue>,
    pub(super) rows: Vec<Vec<CellValue>>,
}

struct KeyedRow {
    keys: Vec<CellValue>,
    row: Vec<CellValue>,
}

impl Runtime<'_, '_> {
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
        let mut groups = Vec::new();
        let mut index = if rows.is_empty() {
            None
        } else {
            self.exact_field_keys(&select.group, &select.source)?
        };
        if self.options.collation_resolver.is_some() {
            // A later runtime field identity may distinguish or equate earlier
            // neutral keys. Evaluate once and retain all key columns before
            // any comparison can determine group membership.
            let mut keyed = Vec::new();
            for row in rows {
                let keys = self.group_key(&row, select, outer)?;
                self.push(&mut keyed, KeyedRow { keys, row })?;
            }
            self.unify_cell_policies(&mut keyed, |input| &mut input.keys)?;
            for input in keyed {
                self.group_member(&mut groups, input.keys, input.row, &policies, &mut index)?;
            }
        } else {
            // An explicit global comparer is stable for the entire operation;
            // preserve incremental key storage when no field resolver is used.
            for row in rows {
                let keys = self.group_key(&row, select, outer)?;
                self.group_member(&mut groups, keys, row, &policies, &mut index)?;
            }
        }
        Ok(groups)
    }

    fn group_key(
        &mut self,
        row: &[CellValue],
        select: &SelectPlan,
        outer: &[OuterScope<'_>],
    ) -> Result<Vec<CellValue>, QueryError> {
        let mut keys = self.allocate_expression_row(select.group.len())?;
        for expression in &select.group {
            keys.push(self.repeatable_comparison_key(
                expression,
                Environment {
                    aggregate_results: None,
                    collections: None,
                    row,
                    outer,
                    output: &[],
                    group: None,
                    group_keys: None,
                },
            )?);
        }
        Ok(keys)
    }

    fn group_member(
        &mut self,
        groups: &mut Vec<Group>,
        keys: Vec<CellValue>,
        row: Vec<CellValue>,
        policies: &[TextPolicy],
        lookup: &mut Option<ExactKeys>,
    ) -> Result<(), QueryError> {
        let (index, fingerprint) =
            self.exact_key_match(lookup, &keys, groups, |group| &group.keys, policies)?;
        if let Some(index) = index {
            for (retained, incoming) in groups[index].keys.iter_mut().zip(&keys) {
                retained.policy = retained.policy.merge(incoming.policy);
                retained.policy_complete |= incoming.policy_complete;
            }
            self.push(&mut groups[index].rows, row)?;
        } else {
            let position = groups.len();
            let mut members = Vec::new();
            self.push(&mut members, row)?;
            self.push(
                groups,
                Group {
                    keys,
                    rows: members,
                },
            )?;
            if let (Some(lookup), Some(fingerprint)) = (lookup.as_mut(), fingerprint) {
                lookup.insert(fingerprint, position, self.budget, self.options.cancelled)?;
            }
        }
        Ok(())
    }
}
