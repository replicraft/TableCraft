//! Normal original-row witnesses, without preview or mutation authority.
use crate::{
    EditError, InspectError, NativeImage, RowDelete,
    header::check,
    table::mutation::{
        preserved_source::Source,
        record_roots,
        rows::{self, RowAssignment},
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

struct Ordinal(usize);

impl RowAssignment for Ordinal {
    fn row(&self) -> usize {
        self.0
    }
}

impl NativeImage {
    /// Capture exact resolved original ordinary-root payloads for a normal DELETE request.
    ///
    /// Ordinals belong to this captured image. Output follows source row order;
    /// duplicates and missing ordinals refuse the entire preparation. Empty
    /// selections still perform the complete native target/program/ownership
    /// and DELETE action admission. Returned payloads grant no save permission:
    /// planning and recovery independently repeat every original source check.
    pub fn prepare_ordinary_row_deletes(
        &self,
        table: &str,
        ordinals: &[usize],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<RowDelete>, EditError> {
        let _span = tracing::debug_span!("native.ordinary.delete.select").entered();
        check(budget, cancelled)?;
        let source = super::capture(self, table, budget, cancelled)?;
        if ordinals.len() > crate::limits::MAX_LIVE_ROWS {
            return Err(InspectError::StructureLimit.into());
        }

        prepare(self, &source, ordinals, budget, cancelled)
    }
}

pub(super) fn prepare(
    image: &NativeImage,
    source: &Source,
    ordinals: &[usize],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<RowDelete>, EditError> {
    budget
        .charge_allocation(ordinals.len() as u64, size_of::<Ordinal>() as u64)
        .map_err(InspectError::from)?;
    let mut selected = Vec::new();
    selected
        .try_reserve_exact(ordinals.len())
        .map_err(|_| InspectError::Allocation)?;
    for &ordinal in ordinals {
        check(budget, cancelled)?;
        selected.push(Ordinal(ordinal));
    }
    rows::unique(&selected, budget, cancelled)?;

    budget
        .charge_allocation(selected.len() as u64, size_of::<RowDelete>() as u64)
        .map_err(InspectError::from)?;
    let mut requests = Vec::new();
    requests
        .try_reserve_exact(selected.len())
        .map_err(|_| InspectError::Allocation)?;
    let captured =
        record_roots::capture(image, source.table.schema(), &selected, budget, cancelled)?;
    for root in captured.selected {
        check(budget, cancelled)?;
        let record = source
            .records
            .get(root.update.0)
            .ok_or(EditError::Conflict)?;
        budget
            .charge(Resource::Steps, record.data.len() as u64)
            .map_err(InspectError::from)?;
        if record.reference != root.reference
            || record.offset != root.final_record.offset
            || record.data != root.final_record.data
        {
            return Err(EditError::Preservation);
        }
        let data = &root.final_record.data;
        budget
            .charge_allocation(data.len() as u64, size_of::<u8>() as u64)
            .map_err(InspectError::from)?;
        budget
            .charge(Resource::Steps, data.len() as u64)
            .map_err(InspectError::from)?;
        let mut payload = Vec::new();
        payload
            .try_reserve_exact(data.len())
            .map_err(|_| InspectError::Allocation)?;
        payload.extend_from_slice(data);
        requests.push(RowDelete::new(root.update.0, payload));
    }
    check(budget, cancelled)?;
    Ok(requests)
}
