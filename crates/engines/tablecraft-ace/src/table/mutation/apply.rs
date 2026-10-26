use super::{EditError, NativeEditPlan, WriteBlocker, gates, patch::BytePatch, preservation};
use crate::{
    IndexCheckStatus, InspectError, NativeImage, check_table, check_table_constraints,
    header::check, preservation::COPY_CHUNK_BYTES, table::indexes::IndexDataPatch,
};
use std::{io::Cursor, ops::Range};
use tablecraft_core::limits::{Resource, ResourceBudget};

impl NativeImage {
    pub(super) fn finish_updates<P: BytePatch>(
        self,
        table: &str,
        fields: &[P],
        indexes: &[IndexDataPatch],
        changed_rows: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        self.finish_tables_updates(&[table], fields, indexes, changed_rows, budget, cancelled)
    }

    pub(super) fn finish_tables_updates<P: BytePatch>(
        self,
        tables: &[&str],
        fields: &[P],
        indexes: &[IndexDataPatch],
        changed_rows: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let length = self.bytes.len();
        budget
            .charge_allocation(length as u64, 1)
            .map_err(InspectError::from)?;
        let mut after = Vec::new();
        after
            .try_reserve_exact(length)
            .map_err(|_| InspectError::Allocation)?;
        for chunk in self.bytes.chunks(COPY_CHUNK_BYTES) {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, chunk.len() as u64)
                .map_err(InspectError::from)?;
            after.extend_from_slice(chunk);
        }
        let mut structural_change = false;
        for patch in fields {
            check(budget, cancelled)?;
            structural_change |= patch.requires_structural_check();
            apply(&mut after, patch.offset(), patch.before(), patch.after())?;
        }
        for patch in indexes {
            check(budget, cancelled)?;
            apply(&mut after, patch.offset, &patch.before, &patch.after)?;
        }
        let count = fields.len() + indexes.len();
        budget
            .charge_allocation(count as u64, size_of::<Range<usize>>() as u64)
            .map_err(InspectError::from)?;
        let mut spans = Vec::new();
        spans
            .try_reserve_exact(count)
            .map_err(|_| InspectError::Allocation)?;
        for patch in fields {
            spans.push(patch.offset()..patch.offset() + patch.before().len());
        }
        for patch in indexes {
            spans.push(patch.offset..patch.offset + patch.before.len());
        }
        preservation::prove(&self.bytes, &after, spans, budget, cancelled)?;
        let scope = self.catalog().catalog().scope();
        for table in tables {
            if (!indexes.is_empty() || structural_change)
                && check_table(&mut Cursor::new(&after), scope, table, budget, cancelled)?
                    .indexes()
                    .iter()
                    .any(|i| i.status() != IndexCheckStatus::Checked)
            {
                return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
            }
            gates::qualified(&check_table_constraints(
                &mut Cursor::new(&after),
                scope,
                table,
                budget,
                cancelled,
            )?)?;
        }
        check(budget, cancelled)?;
        Ok(NativeEditPlan {
            before: self.bytes,
            after,
            changed_rows,
        })
    }
}

fn apply(bytes: &mut [u8], offset: usize, before: &[u8], after: &[u8]) -> Result<(), EditError> {
    if before.len() != after.len() {
        return Err(EditError::Preservation);
    }
    let end = offset
        .checked_add(before.len())
        .ok_or(EditError::Preservation)?;
    let target = bytes.get_mut(offset..end).ok_or(EditError::Preservation)?;
    if target != before {
        return Err(EditError::Preservation);
    }
    target.copy_from_slice(after);
    Ok(())
}
