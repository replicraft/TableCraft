//! Original supplied/omitted fields precede one selected native default family.
use super::{Error, NativeEditError, NativeJournalInsertPlan, NativeJournalInserter, check};
use tablecraft_ace::{
    EditError, InsertFieldValue, InspectError, NativeOmittedInsertIntent, OmittedRowInsert,
    RecoverableEditError, WriteBlocker,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::InsertValue;
use tablecraft_expressions::functions::dates::clock::Clock;

impl NativeJournalInserter {
    pub(super) fn plan_presence(
        self,
        values: Vec<InsertValue>,
        provider: Option<&dyn Clock>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeJournalInsertPlan, Error> {
        check(budget, cancelled)?;
        let fields = convert_fields(values, budget, cancelled)?;
        budget
            .charge_allocation(1, size_of::<OmittedRowInsert>() as u64)
            .map_err(InspectError::from)
            .map_err(NativeEditError::from)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(1)
            .map_err(|_| NativeEditError::from(InspectError::Allocation))?;
        rows.push(OmittedRowInsert::new(fields));
        self.finish_presence(rows, provider, budget, cancelled)
    }

    pub(super) fn plan_presence_rows(
        self,
        rows: Vec<Vec<InsertValue>>,
        provider: Option<&dyn Clock>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeJournalInsertPlan, Error> {
        check(budget, cancelled)?;
        for row in &rows {
            check(budget, cancelled)?;
            if row.len() != self.columns.len() {
                return Err(
                    NativeEditError::from(EditError::Unsupported(WriteBlocker::Column)).into(),
                );
            }
        }
        // Each encoded row needs its two-u32 envelope. Native default/profile
        // checks retain their tighter row, capacity and intent-size bounds.
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
            native.push(OmittedRowInsert::new(convert_fields(
                row, budget, cancelled,
            )?));
        }
        check(budget, cancelled)?;
        self.finish_presence(native, provider, budget, cancelled)
    }

    fn finish_presence(
        self,
        rows: Vec<OmittedRowInsert>,
        provider: Option<&dyn Clock>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeJournalInsertPlan, Error> {
        if let Some(provider) = provider
            && self
                .image
                .select_native_ordinary_clock_defaults(&self.name, &rows, budget, cancelled)
                .map_err(RecoverableEditError::from)?
        {
            return self
                .image
                .plan_recoverable_ordinary_clock_row_inserts(
                    &self.name, rows, provider, budget, cancelled,
                )
                .map(NativeJournalInsertPlan::Clock)
                .map_err(Error::from);
        }
        self.image
            .plan_recoverable_ordinary_omitted_row_inserts(&self.name, rows, budget, cancelled)
            .map(NativeJournalInsertPlan::Omitted)
            .map_err(Error::from)
    }
}

fn convert_fields(
    values: Vec<InsertValue>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<InsertFieldValue>, Error> {
    budget
        .charge_allocation(values.len() as u64, size_of::<InsertFieldValue>() as u64)
        .map_err(InspectError::from)
        .map_err(NativeEditError::from)?;
    let mut fields = Vec::new();
    fields
        .try_reserve_exact(values.len())
        .map_err(|_| NativeEditError::from(InspectError::Allocation))?;
    for value in values {
        check(budget, cancelled)?;
        fields.push(match value {
            InsertValue::Supplied(value) => InsertFieldValue::Supplied(value),
            InsertValue::Omitted => InsertFieldValue::Omitted,
        });
    }
    Ok(fields)
}
