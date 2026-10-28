//! Complete duplicate elimination, ordering and TOP after recursive input reads.
use super::{OutputRow, Runtime};
use crate::{
    QueryError,
    execution::cell::CellValue,
    plan::{ReadNode, ReadPlan},
};

impl Runtime<'_, '_> {
    #[inline(never)]
    pub(in crate::execution) fn finish_read(
        &mut self,
        mut rows: Vec<OutputRow>,
        plan: &ReadPlan,
    ) -> Result<Vec<Vec<CellValue>>, QueryError> {
        let records =
            matches!(&plan.node, ReadNode::Select(select) if !select.record_keys.is_empty());
        if plan.distinct || records {
            // ORDER BY expressions have already been evaluated for every input
            // row. Preserve their contributing runtime policies before a
            // duplicate can discard the only field-policy-bearing key.
            if !plan.sort.is_empty() {
                self.unify_cell_policies(&mut rows, |row| &mut row.keys)?;
            }
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
            let mut lookup = if rows.is_empty() {
                None
            } else if records {
                let ReadNode::Select(select) = &plan.node else {
                    return Err(QueryError::Source);
                };
                self.exact_field_keys(&select.record_keys, &select.source)?
            } else {
                self.exact_output_keys(plan.active_types(self.options.declared_integral))?
            };
            let mut unique: Vec<OutputRow> = Vec::new();
            for row in rows {
                self.validate_key_row(if records { &row.records } else { &row.values })?;
                let keys = if records { &row.records } else { &row.values };
                let (representative, fingerprint) = self.exact_key_match(
                    &mut lookup,
                    keys,
                    &unique,
                    |row| if records { &row.records } else { &row.values },
                    policies,
                )?;
                if representative.is_none() {
                    let position = unique.len();
                    self.push(&mut unique, row)?;
                    if let (Some(lookup), Some(fingerprint)) = (&mut lookup, fingerprint) {
                        lookup.insert(
                            fingerprint,
                            position,
                            self.budget,
                            self.options.cancelled,
                        )?;
                    }
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
}
