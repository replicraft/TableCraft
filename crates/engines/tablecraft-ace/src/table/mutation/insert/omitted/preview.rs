//! Source-bound omission entry points reuse one preparation and final proof.
use super::{OmittedRowInsert, Original, stage};
use crate::table::mutation::insert::{AuditRequest, finish_with};
use crate::{
    Ace16AutoNumberInsertPreview, Ace16WritePreview, EditError, IncrementingAutoNumber,
    InspectError, NativeImage, TableCheck,
    header::check,
    limits::MAX_LIVE_ROWS,
    table::mutation::{gates, preview},
};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Preview presence-bearing ordinary rows, resolving only omitted fields.
    /// Supplied NULL is exact. Unknown needed defaults refuse; no function,
    /// ambient clock, implicit generation or normal-save capability is supplied.
    pub fn preview_ace16_insert_rows_with_omissions(
        self,
        table: &str,
        requests: &[OmittedRowInsert],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16WritePreview, EditError> {
        let _span = tracing::debug_span!(
            "native.edit.preview.insert_omissions",
            rows = requests.len()
        )
        .entered();
        let original = Original::Ordinary(requests);
        let checked = qualify(&self, table, original, budget, cancelled)?;
        let staged = stage::build(&self, &checked, original, budget, cancelled)?;
        finish_with(
            self,
            table,
            AuditRequest::Omitted(original),
            &staged,
            budget,
            cancelled,
        )
    }

    /// Preview omitted companions under an explicit source LAST/+1 policy.
    /// The generated primary is selected independently from ordinary omissions.
    pub fn preview_ace16_incrementing_insert_rows_with_omissions(
        self,
        table: &str,
        requests: &[OmittedRowInsert],
        policy: IncrementingAutoNumber,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16AutoNumberInsertPreview, EditError> {
        let _span = tracing::debug_span!(
            "native.edit.preview.insert_incrementing_omissions",
            rows = requests.len()
        )
        .entered();
        let original = Original::Incrementing { requests, policy };
        let checked = qualify(&self, table, original, budget, cancelled)?;
        let mut staged = stage::build(&self, &checked, original, budget, cancelled)?;
        let candidate = finish_with(
            self,
            table,
            AuditRequest::Omitted(original),
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

pub(in crate::table::mutation::insert) fn qualify(
    image: &NativeImage,
    table: &str,
    original: Original<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableCheck, EditError> {
    check(budget, cancelled)?;
    preview::qualify(image)?;
    let count = original.requests().len();
    if count > MAX_LIVE_ROWS {
        return Err(InspectError::StructureLimit.into());
    }
    let checked = match original {
        Original::Ordinary(_) => gates::before_insert(image, table, budget, cancelled)?,
        Original::Incrementing { .. } => {
            gates::before_incrementing_insert(image, table, budget, cancelled)?
        }
    };
    if checked
        .rows()
        .checked_add(count)
        .is_none_or(|rows| rows > MAX_LIVE_ROWS)
    {
        return Err(InspectError::StructureLimit.into());
    }
    Ok(checked)
}
