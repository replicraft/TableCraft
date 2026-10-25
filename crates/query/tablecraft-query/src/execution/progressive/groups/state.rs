//! Key cells retain first values and complete conservative provenance.
use crate::{
    QueryError,
    execution::{Runtime, aggregate::accumulator::Accumulator, cell::CellValue},
    source::TextPolicy,
};

pub(super) struct Group {
    pub(super) keys: Vec<CellValue>,
    pub(super) accumulators: Vec<Accumulator>,
}

pub(in crate::execution::progressive) struct KeyPolicy {
    policy: TextPolicy,
    complete: bool,
}

impl Runtime<'_, '_> {
    pub(in crate::execution::progressive) fn progressive_key_cohort(
        &mut self,
        width: usize,
    ) -> Result<Vec<KeyPolicy>, QueryError> {
        if self.options.collation_resolver.is_none() {
            return Ok(Vec::new());
        }
        self.budget
            .charge_allocation(width as u64, size_of::<KeyPolicy>() as u64)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(width)
            .map_err(|_| QueryError::Allocation)?;
        for _ in 0..width {
            self.tick()?;
            result.push(KeyPolicy {
                policy: TextPolicy::Default,
                complete: false,
            });
        }
        Ok(result)
    }

    pub(in crate::execution::progressive) fn observe_progressive_keys(
        &mut self,
        cohort: &mut [KeyPolicy],
        keys: &[CellValue],
    ) -> Result<(), QueryError> {
        if cohort.is_empty() {
            return Ok(());
        }
        if cohort.len() != keys.len() {
            return Err(QueryError::Width);
        }
        for (policy, cell) in cohort.iter_mut().zip(keys) {
            self.tick()?;
            policy.policy = policy.policy.merge(cell.policy);
            policy.complete |= cell.policy_complete;
        }
        Ok(())
    }

    pub(in crate::execution::progressive) fn merge_progressive_keys(
        &mut self,
        retained: &mut [CellValue],
        incoming: &[CellValue],
    ) -> Result<(), QueryError> {
        if retained.len() != incoming.len() {
            return Err(QueryError::Width);
        }
        for (retained, incoming) in retained.iter_mut().zip(incoming) {
            self.tick()?;
            retained.policy = retained.policy.merge(incoming.policy);
            retained.policy_complete |= incoming.policy_complete;
        }
        Ok(())
    }

    pub(super) fn complete_progressive_keys(
        &mut self,
        groups: &mut [Group],
        cohort: &[KeyPolicy],
    ) -> Result<(), QueryError> {
        if cohort.is_empty() {
            return Ok(());
        }
        for group in groups {
            self.complete_progressive_key_cells(&mut group.keys, cohort)?;
        }
        Ok(())
    }

    pub(in crate::execution::progressive) fn complete_progressive_key_cells(
        &mut self,
        keys: &mut [CellValue],
        cohort: &[KeyPolicy],
    ) -> Result<(), QueryError> {
        if cohort.is_empty() {
            return Ok(());
        }
        if keys.len() != cohort.len() {
            return Err(QueryError::Width);
        }
        for (cell, policy) in keys.iter_mut().zip(cohort) {
            self.tick()?;
            cell.policy = policy.policy;
            cell.policy_complete = policy.complete;
        }
        Ok(())
    }
}
