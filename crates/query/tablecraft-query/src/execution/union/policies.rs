//! Resolve each UNION arm before merging the whole output cohort.
use super::Runtime;
use crate::{QueryError, execution::cell::CellValue, source::TextPolicy};

impl Runtime<'_, '_> {
    pub(in crate::execution) fn merge_union_policies(
        &mut self,
        rows: &[Vec<CellValue>],
        fallback: &[TextPolicy],
        policies: &mut [TextPolicy],
        complete: &mut [bool],
    ) -> Result<(), QueryError> {
        for column in 0..policies.len() {
            let mut observed = TextPolicy::Default;
            let mut authoritative = false;
            for row in rows {
                self.tick()?;
                let cell = row.get(column).ok_or(QueryError::Width)?;
                observed = observed.merge(cell.policy);
                authoritative |= cell.policy_complete;
            }
            if !authoritative {
                observed = observed.merge(*fallback.get(column).ok_or(QueryError::Width)?);
            }
            policies[column] = policies[column].merge(observed);
            complete[column] |= authoritative;
        }
        Ok(())
    }

    pub(in crate::execution) fn apply_union_policies(
        &mut self,
        rows: &mut [Vec<CellValue>],
        policies: &[TextPolicy],
        complete: &[bool],
    ) -> Result<(), QueryError> {
        for row in rows {
            for (column, cell) in row.iter_mut().enumerate() {
                self.tick()?;
                cell.policy = *policies.get(column).ok_or(QueryError::Width)?;
                cell.policy_complete = *complete.get(column).ok_or(QueryError::Width)?;
            }
        }
        Ok(())
    }
}
