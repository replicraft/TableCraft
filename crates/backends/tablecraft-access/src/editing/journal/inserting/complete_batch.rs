//! Complete original rows become one real TCNA4 action without payload clones.
use super::{Error, NativeEditError, NativeJournalInsertPlan, NativeJournalInserter, check};
#[cfg(feature = "ace16-create")]
use tablecraft_ace::RecoverableEditError;
use tablecraft_ace::{InspectError, NativeCompleteInsertIntent, RowInsert};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::CompleteInsertableBatch;

impl CompleteInsertableBatch for NativeJournalInserter {
    fn plan_insert_rows(
        self,
        rows: Vec<Vec<Value>>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        let _span = tracing::debug_span!("access.insert.plan_complete_batch").entered();
        check(budget, cancelled)?;
        #[cfg(feature = "ace16-create")]
        if self
            .image
            .select_native_retained_manual_indexed_insert_table(&self.name, budget, cancelled)
            .map_err(RecoverableEditError::from)?
        {
            return self.plan_supplied_presence_rows(rows, budget, cancelled);
        }
        #[cfg(feature = "ace16-create")]
        if self
            .image
            .select_native_retained_keyless_insert_table(&self.name, budget, cancelled)
            .map_err(RecoverableEditError::from)?
        {
            return self.plan_supplied_presence_rows(rows, budget, cancelled);
        }
        // Each encoded row needs its two-u32 envelope even before cell payloads.
        // Fresh native intent and Source checks retain their tighter bounds.
        if rows.len() > NativeCompleteInsertIntent::MAX_ENCODED_BYTES / (2 * size_of::<u32>()) {
            return Err(NativeEditError::from(InspectError::StructureLimit).into());
        }
        for row in &rows {
            check(budget, cancelled)?;
            if row.len() != self.columns.len() {
                return Err(
                    NativeEditError::from(tablecraft_ace::EditError::Unsupported(
                        tablecraft_ace::WriteBlocker::Column,
                    ))
                    .into(),
                );
            }
        }
        budget
            .charge_allocation(rows.len() as u64, size_of::<RowInsert>() as u64)
            .map_err(InspectError::from)
            .map_err(NativeEditError::from)?;
        let mut native = Vec::new();
        native
            .try_reserve_exact(rows.len())
            .map_err(|_| NativeEditError::from(InspectError::Allocation))?;
        for row in rows {
            check(budget, cancelled)?;
            native.push(RowInsert::new(row));
        }
        check(budget, cancelled)?;
        self.image
            .plan_recoverable_ordinary_complete_row_inserts(&self.name, native, budget, cancelled)
            .map(NativeJournalInsertPlan::CompleteTyped)
            .map_err(Error::from)
    }
}
