//! Complete key cohorts and every membership comparison precede operand replay.
use super::state::{AssignedRow, Captured, Partition};
use crate::{
    QueryError,
    execution::{Runtime, progressive::having::DeferredGroup},
};
use tablecraft_expressions::EvalError;

impl Runtime<'_, '_> {
    pub(super) fn partition_text_groups(
        &mut self,
        captured: &mut Captured<'_>,
    ) -> Result<Partition, QueryError> {
        for candidate in &mut captured.candidates {
            self.complete_progressive_key_cells(&mut candidate.keys, &captured.cohort)?;
        }
        let mut partition = Partition {
            groups: Vec::new(),
            rows: Vec::new(),
        };
        let mut lookup = None;
        for candidate in std::mem::take(&mut captured.candidates) {
            self.tick()?;
            // A None lookup uses the existing strict tuple comparer in original
            // representative order. No Text or partial-key hash is introduced.
            let (index, _) = self.exact_key_match(
                &mut lookup,
                &candidate.keys,
                &partition.groups,
                |group| &group.keys,
                &captured.policies,
            )?;
            let index = if let Some(index) = index {
                self.merge_progressive_keys(&mut partition.groups[index].keys, &candidate.keys)?;
                index
            } else {
                let index = partition.groups.len();
                self.push_text_group_item(
                    &mut partition.groups,
                    DeferredGroup {
                        keys: candidate.keys,
                        accumulators: Vec::new(),
                        rows: Vec::new(),
                        count: 0,
                    },
                )?;
                index
            };
            self.tick()?;
            let group = &mut partition.groups[index];
            group.count = group.count.checked_add(1).ok_or(EvalError::Overflow)?;
            if let Some(row) = candidate.row {
                self.push_text_group_item(&mut partition.rows, AssignedRow { group: index, row })?;
            }
        }
        Ok(partition)
    }
}
