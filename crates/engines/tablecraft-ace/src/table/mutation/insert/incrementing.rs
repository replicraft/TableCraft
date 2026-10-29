//! Caller-selected generation feeds the existing complete INSERT pipeline.
use super::{
    Ace16AutoNumberInsertPreview, AuditRequest, AutoNumberRowInsert, IncrementingAutoNumber,
    RowInsert, Staged, autonumber, finish_with, gates, prepare_with, preview, record,
};
use crate::{EditError, InspectError, NativeImage, TableCheck, WriteBlocker, header::check};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

impl NativeImage {
    /// Construct an incrementing AutoNumber engineering candidate under an
    /// explicit caller policy and exact original last-issued counter.
    ///
    /// Companion values exclude the generated primary column. This operation
    /// does not detect or certify the stored New Values mode, evaluate defaults,
    /// override explicit IDs or grant a normal save/journal capability.
    pub fn preview_ace16_incrementing_insert_rows(
        self,
        table: &str,
        requests: &[AutoNumberRowInsert],
        policy: IncrementingAutoNumber,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16AutoNumberInsertPreview, EditError> {
        let _span = tracing::debug_span!(
            "native.edit.preview.insert_incrementing",
            rows = requests.len()
        )
        .entered();
        check(budget, cancelled)?;
        preview::qualify(&self)?;
        if requests.len() > crate::limits::MAX_LIVE_ROWS {
            return Err(InspectError::StructureLimit.into());
        }
        let checked = gates::before_incrementing_insert(&self, table, budget, cancelled)?;
        if checked
            .rows()
            .checked_add(requests.len())
            .is_none_or(|count| count > crate::limits::MAX_LIVE_ROWS)
        {
            return Err(InspectError::StructureLimit.into());
        }

        let mut staged = prepare(&self, &checked, requests, policy, budget, cancelled)?;
        let candidate = finish_with(
            self,
            table,
            AuditRequest::Incrementing { requests, policy },
            &staged,
            budget,
            cancelled,
        )?;
        let plan = staged.autonumber.take().ok_or(EditError::Preservation)?;
        Ok(Ace16AutoNumberInsertPreview::from_candidate(
            candidate,
            plan.source.column,
            plan.values,
        ))
    }
}

pub(super) fn prepare(
    image: &NativeImage,
    checked: &TableCheck,
    requests: &[AutoNumberRowInsert],
    policy: IncrementingAutoNumber,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Staged, EditError> {
    let source = autonumber::counter::bind(image, checked, policy, budget, cancelled)?;
    let profile =
        record::Profile::bind_incrementing(checked.schema(), source.column, budget, cancelled)?;
    let plan = autonumber::counter::prepare(source, requests.len(), budget, cancelled)?;
    let rows = normalize(
        checked.schema().columns().len(),
        source.column,
        requests,
        &plan.values,
        budget,
        cancelled,
    )?;
    let mut staged = prepare_with(
        image,
        checked,
        &rows,
        source.column,
        profile,
        budget,
        cancelled,
    )?;
    staged.autonumber = Some(plan);
    Ok(staged)
}

fn normalize(
    columns: usize,
    generated: usize,
    requests: &[AutoNumberRowInsert],
    keys: &[i32],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<RowInsert>, EditError> {
    if generated >= columns || requests.len() != keys.len() {
        return Err(EditError::Preservation);
    }
    budget
        .charge_allocation(requests.len() as u64, size_of::<RowInsert>() as u64)
        .map_err(InspectError::from)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(requests.len())
        .map_err(|_| InspectError::Allocation)?;
    for (request, key) in requests.iter().zip(keys) {
        check(budget, cancelled)?;
        if request.values().len() != columns - 1 {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        budget
            .charge_allocation(columns as u64, size_of::<Value>() as u64)
            .map_err(InspectError::from)?;
        budget
            .charge(Resource::Steps, columns as u64)
            .map_err(InspectError::from)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(columns)
            .map_err(|_| InspectError::Allocation)?;
        let mut companions = request.values().iter();
        for position in 0..columns {
            check(budget, cancelled)?;
            if position == generated {
                values.push(Value::Int32(*key));
            } else {
                let value = companions.next().ok_or(EditError::Preservation)?;
                values.push(autonumber::request::copy_value(value, budget, cancelled)?);
            }
        }
        if companions.next().is_some() {
            return Err(EditError::Preservation);
        }
        rows.push(RowInsert::new(values));
    }
    Ok(rows)
}
