//! Only immutable requests and original metadata can seed index continuation.
use super::{
    ReplayAuthority,
    placement_certificate::{self, CertifiedPlacement},
};
use crate::{
    EditError, NativeImage, TableCheck,
    format::{page::PAGE_BYTES, row::reference_page},
    header::check,
    table::{
        indexes::{SlotKeyUpdate, qualify_split_slot_indexes},
        mutation::{
            gates,
            page_update::roots,
            variable_split::metadata::Metadata,
            variable_update::{
                authority::{Operation, Verified},
                placement::DataView,
            },
        },
    },
};
use tablecraft_core::limits::ResourceBudget;

pub(in crate::table) struct CertifiedData<'a> {
    phase: CertifiedPlacement<'a>,
    original: TableCheck,
    verified: Verified,
}

impl CertifiedData<'_> {
    pub(in crate::table) fn phase(&self) -> &CertifiedPlacement<'_> {
        &self.phase
    }

    pub(in crate::table) fn original(&self) -> &TableCheck {
        &self.original
    }

    pub(in crate::table) fn column(&self) -> usize {
        self.verified.column
    }

    pub(in crate::table) fn changed_rows(&self) -> usize {
        self.verified.records.len()
    }

    pub(in crate::table) fn keys(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<SlotKeyUpdate<'_>>, EditError> {
        self.verified.keys(budget, cancelled)
    }
}

pub(in crate::table::mutation) fn certify<'a>(
    image: &'a NativeImage,
    operation: &Operation<'_>,
    data: DataView<'_>,
    after: &'a [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CertifiedData<'a>, EditError> {
    check(budget, cancelled)?;
    if data.append.original_length != image.as_bytes().len()
        || data.append.length != after.len()
        || data.length > after.len()
    {
        return Err(EditError::Preservation);
    }

    let original = gates::before_slot(image, operation.table, budget, cancelled)?;
    let verified = operation.prove(image, &original, budget, cancelled)?;
    qualify_split_slot_indexes(
        original.schema(),
        original.indexes(),
        verified.column,
        budget,
        cancelled,
    )?;
    let metadata = Metadata::capture(
        image,
        original.schema(),
        original.indexes(),
        verified.column,
        budget,
        cancelled,
    )?;

    // Include selected semantic no-ops: they still cannot establish a DATA
    // operation on storage which the complete census identifies as metadata.
    let captured = roots::capture(
        image,
        original.schema(),
        operation.requests(),
        budget,
        cancelled,
    )?;
    for root in captured.selected {
        check(budget, cancelled)?;
        metadata.require_data_page(reference_page(root.reference), budget, cancelled)?;
        let terminal = u32::try_from(root.final_record.offset / PAGE_BYTES)
            .map_err(|_| EditError::Preservation)?;
        metadata.require_data_page(terminal, budget, cancelled)?;
    }

    let phase = placement_certificate::prove(
        placement_certificate::Operation {
            image,
            schema: original.schema(),
            records: &verified.records,
            authority: ReplayAuthority::QualifiedCandidate(&metadata),
        },
        data,
        after,
        budget,
        cancelled,
    )?;

    Ok(CertifiedData {
        phase,
        original,
        verified,
    })
}
