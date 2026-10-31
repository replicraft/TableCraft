//! Guard finish over the actual original snapshot/value-conversion owner.
use super::{AccessEngineeringEditor, check};
use crate::NativeEditError;
use crate::engineering::guards::before_change::BeforeChangeBackendError;
use crate::engineering::guards::{GuardBackendError, GuardPreparationSummary, summary};
use tablecraft_ace::{
    Ace16BeforeChangePreview, BeforeEventGuardSelector, BeforeEventIntents, FixedScalarUpdate,
};
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
        let requests = self.guard_requests(column, updates, budget, cancelled)?;

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

    pub(in crate::engineering) fn preview_before_change(
        self,
        selectors: &[BeforeEventGuardSelector<'_>],
        column: &str,
        updates: &[CellUpdate],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16BeforeChangePreview, BeforeChangeBackendError> {
        check(budget, cancelled)?;
        let requests = self.guard_requests(column, updates, budget, cancelled)?;

        self.image
            .preview_ace16_before_change_updates(
                self.snapshot.name(),
                selectors,
                column,
                &requests,
                budget,
                cancelled,
            )
            .map_err(BeforeChangeBackendError::Preview)
    }

    fn guard_requests(
        &self,
        column: &str,
        updates: &[CellUpdate],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<FixedScalarUpdate>, NativeEditError> {
        let positions =
            self.validate_columns(&[ColumnUpdates { column, updates }], budget, cancelled)?;
        let [position] = positions.as_slice() else {
            return Err(NativeEditError::Unsupported);
        };
        self.requests(
            *position,
            updates,
            FixedScalarUpdate::new,
            budget,
            cancelled,
        )
    }
}
