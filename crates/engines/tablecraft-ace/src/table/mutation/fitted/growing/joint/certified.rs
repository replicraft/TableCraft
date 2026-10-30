//! Only fresh original groups and fully reconstructed records can seed keys.
use super::super::super::{
    record_proof::{self, verified::Verified},
    request::{self, FittedColumnUpdates},
    serialization::RecordPolicy,
};
use crate::{
    EditError, NativeImage, TableCheck,
    format::{page::PAGE_BYTES, row::reference_page},
    table::{
        indexes::RowKeyUpdate,
        mutation::{
            page_update::roots,
            variable_split::metadata::Metadata,
            variable_update::{
                map_audit::{
                    ReplayAuthority,
                    placement_certificate::{self, CertifiedPlacement},
                },
                placement::DataView,
            },
        },
    },
};
use tablecraft_core::limits::ResourceBudget;

pub(in crate::table) struct CertifiedRows<'a> {
    phase: CertifiedPlacement<'a>,
    verified: Verified,
}

impl CertifiedRows<'_> {
    pub(in crate::table) fn phase(&self) -> &CertifiedPlacement<'_> {
        &self.phase
    }

    pub(in crate::table) fn original(&self) -> &TableCheck {
        &self.verified.checked
    }

    pub(in crate::table) fn columns(&self) -> &[usize] {
        &self.verified.columns
    }

    pub(in crate::table) fn changed_rows(&self) -> usize {
        self.verified.records.len()
    }

    pub(in crate::table) fn keys(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<RowKeyUpdate<'_>>, EditError> {
        self.verified.keys(budget, cancelled)
    }
}

pub(super) fn certify<'a>(
    image: &'a NativeImage,
    table: &str,
    groups: &[FittedColumnUpdates<'_>],
    data: DataView<'_>,
    after: &'a [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CertifiedRows<'a>, EditError> {
    let verified = record_proof::records(
        image,
        table,
        groups,
        RecordPolicy::GrowingPreferred,
        budget,
        cancelled,
    )?;
    verified.require_source(image)?;
    let schema = verified.checked.schema();
    let metadata = Metadata::capture_for_mixed_updates(
        image,
        schema,
        verified.checked.indexes(),
        &verified.columns,
        budget,
        cancelled,
    )?;

    // Semantic no-ops still require qualified root and terminal ownership.
    let bound = request::bind(schema, groups, budget, cancelled)?;
    let captured = roots::capture(image, schema, &bound.rows, budget, cancelled)?;
    for root in captured.selected {
        request::check(budget, cancelled)?;
        metadata.require_data_page(reference_page(root.reference), budget, cancelled)?;
        let terminal = u32::try_from(root.final_record.offset / PAGE_BYTES)
            .map_err(|_| EditError::Preservation)?;
        metadata.require_data_page(terminal, budget, cancelled)?;
    }

    let phase = placement_certificate::prove(
        placement_certificate::Operation {
            image,
            schema,
            records: &verified.records,
            authority: ReplayAuthority::ProtectedRepresented(&metadata),
        },
        data,
        after,
        budget,
        cancelled,
    )?;

    Ok(CertifiedRows { phase, verified })
}
