//! Guard finish over the actual original snapshot/value-conversion owner.
use super::{AccessEngineeringEditor, ScalarUpdateMode, check};
use crate::NativeEditError;
use crate::engineering::guards::before_change::BeforeChangeBackendError;
use crate::engineering::guards::{GuardBackendError, GuardPreparationSummary, summary};
use tablecraft_ace::{
    Ace16BeforeChangePreview, BeforeEventGuardSelector, BeforeEventIntents, FixedColumnUpdates,
    FixedScalarUpdate, InspectError,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{CellUpdate, ColumnUpdates};
use tablecraft_schema::ScalarType;

impl AccessEngineeringEditor {
    /// New batch facade is intentionally projected-rich and Default only. Other
    /// existing services retain their explicit modes and native admission paths.
    pub(in crate::engineering) fn require_before_change_batch_profile(
        &self,
    ) -> Result<(), NativeEditError> {
        if self.mode != ScalarUpdateMode::Default || !self.projected_schema {
            return Err(NativeEditError::Unsupported);
        }
        Ok(())
    }

    pub(in crate::engineering) fn preview_before_change_batch(
        self,
        selectors: &[BeforeEventGuardSelector<'_>],
        columns: &[ColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16BeforeChangePreview, BeforeChangeBackendError> {
        check(budget, cancelled)?;
        self.require_before_change_batch_profile()?;
        if columns.is_empty() || columns.len() > self.snapshot.columns().len() {
            return Err(NativeEditError::Unsupported.into());
        }
        // Preserve the actual name policy, snapshot identity, duplicate/ordinal,
        // carrier and retained native declaration checks used by other finishes.
        let positions = self.validate_columns(columns, budget, cancelled)?;
        for &position in &positions {
            check(budget, cancelled)?;
            let target = self
                .snapshot
                .columns()
                .get(position)
                .ok_or(NativeEditError::Unsupported)?;
            if !matches!(
                target.data_type(),
                ScalarType::Boolean
                    | ScalarType::Int32
                    | ScalarType::Int64
                    | ScalarType::Currency
                    | ScalarType::DateTime
            ) {
                return Err(NativeEditError::Unsupported.into());
            }
        }
        budget
            .charge_allocation(
                columns.len() as u64,
                size_of::<Vec<FixedScalarUpdate>>() as u64,
            )
            .map_err(InspectError::from)
            .map_err(NativeEditError::from)?;
        let mut requests = Vec::new();
        requests
            .try_reserve_exact(columns.len())
            .map_err(|_| NativeEditError::from(InspectError::Allocation))?;
        for (column, position) in columns.iter().zip(positions) {
            requests.push(self.requests(
                position,
                column.updates,
                FixedScalarUpdate::new,
                budget,
                cancelled,
            )?);
        }
        budget
            .charge_allocation(
                columns.len() as u64,
                size_of::<FixedColumnUpdates<'_>>() as u64,
            )
            .map_err(InspectError::from)
            .map_err(NativeEditError::from)?;
        let mut groups = Vec::new();
        groups
            .try_reserve_exact(columns.len())
            .map_err(|_| NativeEditError::from(InspectError::Allocation))?;
        for (column, rows) in columns.iter().zip(&requests) {
            check(budget, cancelled)?;
            groups.push(FixedColumnUpdates::new(
                self.snapshot.name(),
                column.column,
                rows,
            ));
        }
        check(budget, cancelled)?;
        self.image
            .preview_ace16_before_change_batch_updates(
                self.snapshot.name(),
                selectors,
                &groups,
                budget,
                cancelled,
            )
            .map_err(BeforeChangeBackendError::Preview)
    }

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
