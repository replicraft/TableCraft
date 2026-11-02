//! One atomic TCNA2 action retains every original row and presence bit.
use super::{Error, NativeEditError, NativeJournalInsertPlan, NativeJournalInserter, check};
use tablecraft_ace::{InsertFieldValue, InspectError, NativeOmittedInsertIntent, OmittedRowInsert};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{InsertValue, OmittingInsertableBatch};

impl OmittingInsertableBatch for NativeJournalInserter {
    fn plan_insert_rows_with_omissions(
        self,
        rows: Vec<Vec<InsertValue>>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        check(budget, cancelled)?;
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

        // Every encoded row needs at least its two-u32 framing. The engine
        // separately enforces its current live-row, schema and carrier bounds.
        if rows.len() > NativeOmittedInsertIntent::MAX_ENCODED_BYTES / (2 * size_of::<u32>()) {
            return Err(NativeEditError::from(InspectError::StructureLimit).into());
        }
        budget
            .charge_allocation(rows.len() as u64, size_of::<OmittedRowInsert>() as u64)
            .map_err(InspectError::from)
            .map_err(NativeEditError::from)?;
        let mut native = Vec::new();
        native
            .try_reserve_exact(rows.len())
            .map_err(|_| NativeEditError::from(InspectError::Allocation))?;

        for row in rows {
            check(budget, cancelled)?;
            budget
                .charge_allocation(row.len() as u64, size_of::<InsertFieldValue>() as u64)
                .map_err(InspectError::from)
                .map_err(NativeEditError::from)?;
            let mut fields = Vec::new();
            fields
                .try_reserve_exact(row.len())
                .map_err(|_| NativeEditError::from(InspectError::Allocation))?;
            for value in row {
                check(budget, cancelled)?;
                fields.push(match value {
                    InsertValue::Supplied(value) => InsertFieldValue::Supplied(value),
                    InsertValue::Omitted => InsertFieldValue::Omitted,
                });
            }
            native.push(OmittedRowInsert::new(fields));
        }

        check(budget, cancelled)?;
        self.image
            .plan_recoverable_ordinary_omitted_row_inserts(&self.name, native, budget, cancelled)
            .map(NativeJournalInsertPlan::Omitted)
            .map_err(Error::from)
    }
}
