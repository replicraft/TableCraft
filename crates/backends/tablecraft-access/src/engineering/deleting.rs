//! Identity-bound original SQL selection finishes through one delete service.
mod guards;
use super::{DeleteMode, EngineeringCopy, EngineeringError, EngineeringRequest, preview};
use crate::editing::{NativeEditError, snapshot};
use tablecraft_ace::{InspectError, NativeImage};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_db::{DeletableTable, RowHandle, Snapshot};

/// Separate engineering deletion source; never a normal native save editor.
pub struct AccessEngineeringDeleter {
    image: NativeImage,
    snapshot: Snapshot,
    mode: DeleteMode,
}

impl AccessEngineeringDeleter {
    /// Capture one complete original scalar table under the operation allowance.
    /// Native constraints and deletion admission remain required at finish.
    pub fn new(
        image: NativeImage,
        table: &str,
        mode: DeleteMode,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeEditError> {
        let _span = tracing::debug_span!("access.engineering.delete.capture").entered();
        let snapshot = snapshot::capture(&image, table, budget, cancelled)?;

        Ok(Self {
            image,
            snapshot,
            mode,
        })
    }
}

impl DeletableTable for AccessEngineeringDeleter {
    type Error = NativeEditError;
    type Plan = EngineeringCopy;

    fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    fn plan_deletes(
        self,
        rows: &[RowHandle],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<EngineeringCopy, NativeEditError> {
        let _span = tracing::debug_span!("access.engineering.delete.plan").entered();
        check(budget, cancelled)?;

        if rows.len() > NativeImage::MAX_DELETE_BATCH_ROWS {
            return Err(InspectError::StructureLimit.into());
        }

        budget
            .charge_allocation(rows.len() as u64, size_of::<usize>() as u64)
            .map_err(InspectError::from)?;
        let mut ordinals = Vec::new();
        ordinals
            .try_reserve_exact(rows.len())
            .map_err(|_| InspectError::Allocation)?;

        for &row in rows {
            check(budget, cancelled)?;
            ordinals.push(self.snapshot.ordinal(row)?);
        }

        // The actual exact-record capture rejects duplicate/missing ordinals and
        // requalifies the immutable source, including an empty selection.
        preview(
            self.image,
            EngineeringRequest::DeleteOrdinals {
                table: self.snapshot.name(),
                ordinals: &ordinals,
                mode: self.mode,
            },
            budget,
            cancelled,
        )
        .map_err(|error| match error {
            EngineeringError::Engine(error) => NativeEditError::Edit(error),
        })
    }
}

fn check(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), NativeEditError> {
    if cancelled() {
        return Err(InspectError::Cancelled.into());
    }
    budget
        .charge(Resource::Steps, 1)
        .map_err(InspectError::from)?;
    Ok(())
}

#[cfg(test)]
pub(in crate::engineering) mod tests;
