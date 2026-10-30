//! Guard finish over the actual original snapshot/value-conversion owner.
use super::AccessEngineeringEditor;
use crate::NativeEditError;
use crate::engineering::guards::{GuardBackendError, GuardPreparationSummary, summary};
use tablecraft_ace::{BeforeEventGuardSelector, BeforeEventIntents, FixedScalarUpdate};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{CellUpdate, ColumnUpdates};

impl AccessEngineeringEditor {
    pub(in crate::engineering) fn prepare_before_guards(
        self,
        selectors: &[BeforeEventGuardSelector<'_>],
        column: &str,
        updates: &[CellUpdate],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<GuardPreparationSummary, GuardBackendError> {
        summary::check(budget, cancelled)?;
        let positions =
            self.validate_columns(&[ColumnUpdates { column, updates }], budget, cancelled)?;
        let [position] = positions.as_slice() else {
            return Err(NativeEditError::Unsupported.into());
        };
        let requests = self.requests(
            *position,
            updates,
            FixedScalarUpdate::new,
            budget,
            cancelled,
        )?;

        let report = self.image.prepare_ace16_before_event_guards(
            self.snapshot.name(),
            selectors,
            BeforeEventIntents::FixedUpdate {
                column,
                updates: &requests,
            },
            budget,
            cancelled,
        )?;
        summary::retain(&report, requests.len(), budget, cancelled)
    }
}
