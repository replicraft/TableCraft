//! Stored attachment projections retain their explicit child schema and origin.
use super::Binder;
use crate::{
    OutputType, QueryError,
    plan::{Expr, RecordsetSlot, Slot},
    source::{self, RecordsetOutput},
};
use std::sync::Arc;

pub(super) fn projection(slot: &Slot) -> Result<Expr, QueryError> {
    let recordset = slot.recordset.as_ref().ok_or(QueryError::Source)?;
    let OutputType::Recordset { parent_nullable } = slot.output_type.conservative() else {
        return Err(QueryError::Source);
    };
    Ok(Expr::Recordset {
        index: recordset.index,
        parent_nullable,
        origin: recordset.origin,
    })
}

impl Binder<'_> {
    pub(super) fn retain_recordset_slot(
        &mut self,
        recordset: &RecordsetSlot,
        offset: usize,
    ) -> Result<RecordsetSlot, QueryError> {
        self.work()?;
        let index = recordset
            .index
            .checked_add(offset)
            .ok_or(QueryError::StructureLimit)?;
        // NO_CLONE: The new relation slot outlives the borrowed input relation;
        // sharing its immutable schema retains identity without copying payloads.
        let schema = Arc::clone(&recordset.schema);
        Ok(RecordsetSlot {
            index,
            origin: recordset.origin,
            schema,
        })
    }

    pub(super) fn recordset_outputs(
        &mut self,
        expressions: &[Expr],
        slots: &[Slot],
    ) -> Result<Vec<RecordsetOutput>, QueryError> {
        let count = expressions
            .iter()
            .filter(|expression| matches!(expression, Expr::Recordset { .. }))
            .count();
        if count == 0 {
            return Ok(Vec::new());
        }
        self.charge(
            count
                .checked_mul(size_of::<RecordsetOutput>())
                .ok_or(QueryError::StructureLimit)?,
        )?;
        let mut outputs = Vec::new();
        outputs
            .try_reserve_exact(count)
            .map_err(|_| QueryError::Allocation)?;
        for (index, expression) in expressions.iter().enumerate() {
            self.work()?;
            let Expr::Recordset { origin, .. } = expression else {
                continue;
            };
            let mut selected: Option<&RecordsetSlot> = None;
            for slot in slots {
                self.work()?;
                if let Some(recordset) = &slot.recordset
                    && recordset.origin == *origin
                {
                    if let Some(previous) = selected {
                        if !source::schemas_match(&previous.schema, &recordset.schema, |amount| {
                            for _ in 0..amount {
                                self.work()?;
                            }
                            Ok(())
                        })? {
                            return Err(QueryError::Source);
                        }
                    }
                    selected = Some(recordset);
                }
            }
            let recordset = selected.ok_or(QueryError::Source)?;
            // NO_CLONE: Output metadata remains available after relation binding
            // and for empty results; Arc shares the captured child declaration.
            let schema = Arc::clone(&recordset.schema);
            outputs.push(RecordsetOutput { index, schema });
        }
        Ok(outputs)
    }
}

#[cfg(test)]
mod tests;
