//! Snapshot-issued handles select original ordinals inside the engine guard owner.
use super::AccessEngineeringDeleter;
use crate::NativeEditError;
use crate::engineering::guards::{GuardBackendError, GuardPreparationSummary, summary};
use tablecraft_ace::{BeforeEventGuardSelector, BeforeEventIntents, InspectError, NativeImage};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::RowHandle;

impl AccessEngineeringDeleter {
    pub(in crate::engineering) fn prepare_before_guards(
        self,
        selectors: &[BeforeEventGuardSelector<'_>],
        rows: &[RowHandle],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<GuardPreparationSummary, GuardBackendError> {
        summary::check(budget, cancelled)?;
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
            summary::check(budget, cancelled)?;
            ordinals.push(self.snapshot.ordinal(row).map_err(NativeEditError::from)?);
        }

        let report = self.image.prepare_ace16_before_event_guards(
            self.snapshot.name(),
            selectors,
            BeforeEventIntents::DeleteOrdinals(&ordinals),
            budget,
            cancelled,
        )?;
        summary::retain(&report, ordinals.len(), budget, cancelled)
    }
}
