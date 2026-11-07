mod declaration;

use super::{FirstTableReopen, ReviewedFirstTable, ReviewedFirstTableState};
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check},
    document::Document,
    editing::{EditError, ids::issue_id},
};
use bytes::Bytes;
use std::sync::Arc;
use tablecraft_ace::NativeImage;
use tablecraft_builders::table::TableDraftSnapshot;
use tablecraft_core::limits::Resource;

const REOPEN_COPY_CHUNK_BYTES: usize = 16 * 1024;

impl Document {
    pub(crate) fn review_first_table(
        &mut self,
        document_revision: u64,
        snapshot: &TableDraftSnapshot,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        let _span = tracing::debug_span!("application.review_first_table").entered();
        if document_revision != self.revision {
            return Err(Error::StaleDocument);
        }
        if !self.source.editable_native() {
            return Err(EditError::ReadOnlySource.into());
        }
        let mut work = budget();
        check(cancelled)?;
        let definition = declaration::capture(snapshot, &mut work, cancelled)?;
        let image = NativeImage::capture(&mut self.source, SCOPE, &mut work, cancelled)?;
        let plan = image.plan_first_table(definition, &mut work, cancelled)?;

        // The expected AFTER is content evidence for reopening, never a payload
        // accepted by the save path. Saving rederives its own native plan.
        work.charge_allocation(plan.after_bytes().len() as u64, 1)?;
        let mut expected = Vec::new();
        expected.try_reserve_exact(plan.after_bytes().len())?;
        for chunk in plan.after_bytes().chunks(REOPEN_COPY_CHUNK_BYTES) {
            check(cancelled)?;
            work.charge(Resource::Steps, chunk.len() as u64)?;
            expected.extend_from_slice(chunk);
        }
        work.charge_allocation(self.edits.path.as_os_str().len() as u64, 1)?;
        work.charge_allocation(snapshot.table_name().len() as u64, 1)?;
        let mut table = String::new();
        table.try_reserve_exact(snapshot.table_name().len())?;
        table.push_str(snapshot.table_name());
        work.charge_allocation(1, size_of::<FirstTableReopen>() as u64)?;
        work.charge_allocation(1, size_of::<ReviewedFirstTableState>() as u64)?;
        check(cancelled)?;
        let reopen = Arc::new(FirstTableReopen {
            path: self.edits.path.clone(),
            expected: Bytes::from(expected),
            table,
            columns: plan.column_count(),
            document_revision,
            draft_revision: snapshot.revision(),
        });
        let capability = ReviewedFirstTable {
            state: Box::new(ReviewedFirstTableState {
                id: issue_id()?,
                reopen,
                original: self.edits.original.clone(),
                definition: plan.into_definition(),
                valid: Arc::clone(&self.edits.valid),
            }),
        };
        Ok(Response::FirstTableReviewed { capability })
    }
}

#[cfg(test)]
mod tests;
