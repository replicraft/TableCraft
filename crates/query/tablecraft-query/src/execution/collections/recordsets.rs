//! Immutable child records retain their schema, presence and unavailable payloads.
use super::{RecordsetEntry, Runtime};
use crate::QueryError;
use std::sync::Arc;
use tablecraft_core::{limits::Resource, values::Value};
use tablecraft_db::{OwnedRecordset, RecordsetSchema, StoredCell};

impl Runtime<'_, '_> {
    pub(super) fn retain_recordset(
        &mut self,
        value: &Arc<OwnedRecordset>,
        expected: &RecordsetSchema,
    ) -> Result<Arc<OwnedRecordset>, QueryError> {
        self.tick()?;
        if !crate::source::schemas_match(expected, value.schema(), |work| {
            self.budget.charge(Resource::Steps, work as u64)?;
            self.tick()
        })? {
            return Err(QueryError::Source);
        }
        self.budget.charge(
            Resource::Rows,
            u64::try_from(value.len()).map_err(|_| QueryError::StructureLimit)?,
        )?;
        self.budget.charge(
            Resource::Bytes,
            u64::try_from(value.retained_bytes()).map_err(|_| QueryError::StructureLimit)?,
        )?;
        for row in value.rows() {
            self.tick()?;
            for cell in row {
                self.tick()?;
                if let StoredCell::Value(Value::Text(text)) = cell
                    && text.len() > self.options.text_limit()
                {
                    return Err(tablecraft_expressions::EvalError::TextLimit.into());
                }
            }
        }
        // NO_CLONE: The result outlives the source callback. Share the validated
        // immutable owner instead of copying metadata rows or uninspected payloads.
        Ok(Arc::clone(value))
    }

    pub(super) fn share_recordsets(
        &mut self,
        values: &[RecordsetEntry],
    ) -> Result<Vec<RecordsetEntry>, QueryError> {
        self.tick()?;
        self.budget
            .charge_allocation(values.len() as u64, size_of::<RecordsetEntry>() as u64)?;
        let mut shared = Vec::new();
        shared
            .try_reserve_exact(values.len())
            .map_err(|_| QueryError::Allocation)?;
        for (origin, value) in values {
            self.tick()?;
            // NO_CLONE: Joined and projected frames share the same child snapshot;
            // copying a frame must not duplicate its complete metadata rows.
            shared.push((*origin, value.as_ref().map(Arc::clone)));
        }
        Ok(shared)
    }
}
