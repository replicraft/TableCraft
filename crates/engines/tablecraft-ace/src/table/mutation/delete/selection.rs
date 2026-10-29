//! Snapshot-local ordinal selection for exact engineering DELETE requests.
use super::RowDelete;
use crate::{
    EditError, InspectError, NativeImage,
    format::reader::Reader,
    header::check,
    limits::MAX_LIVE_ROWS,
    table::mutation::{gates, preview, rows},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

#[cfg(test)]
mod tests;

struct Ordinal(usize);

impl rows::RowAssignment for Ordinal {
    fn row(&self) -> usize {
        self.0
    }
}

impl NativeImage {
    /// Resolve live-row ordinals to exact raw DELETE requests in this image.
    ///
    /// Results follow source row order, independent of request order. Duplicate
    /// or missing ordinals refuse the whole selection. Ordinals are local to
    /// this captured snapshot; the resulting requests retain exact payload
    /// preconditions for subsequent deletion previews on the same image.
    /// Existing ACE16 DELETE admission and ordinary-record restrictions apply.
    /// This reads no active content and grants no save or macro permission.
    pub fn prepare_ace16_row_deletes(
        &self,
        table: &str,
        ordinals: &[usize],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<RowDelete>, EditError> {
        let _span = tracing::debug_span!("native.delete.select", rows = ordinals.len()).entered();
        check(budget, cancelled)?;
        preview::qualify(self)?;
        if ordinals.len() > MAX_LIVE_ROWS {
            return Err(InspectError::StructureLimit.into());
        }

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
        let before = gates::before_delete(self, table, budget, cancelled)?;

        budget
            .charge_allocation(selected.len() as u64, size_of::<RowDelete>() as u64)
            .map_err(InspectError::from)?;
        let mut deletes = Vec::new();
        deletes
            .try_reserve_exact(selected.len())
            .map_err(|_| InspectError::Allocation)?;

        let mut source = self.reader();
        let mut reader = Reader {
            source: &mut source,
            header: self.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        rows::visit_with_budget(
            &mut reader,
            before.schema(),
            &selected,
            |budget, row, ordinal| {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, row.data.len() as u64)
                    .map_err(InspectError::from)?;
                budget
                    .charge_allocation(row.data.len() as u64, size_of::<u8>() as u64)
                    .map_err(InspectError::from)?;
                let mut expected = Vec::new();
                expected
                    .try_reserve_exact(row.data.len())
                    .map_err(|_| InspectError::Allocation)?;
                expected.extend_from_slice(row.data);
                deletes.push(RowDelete::new(ordinal.0, expected));
                Ok(())
            },
        )?;

        check(budget, cancelled)?;
        Ok(deletes)
    }
}
