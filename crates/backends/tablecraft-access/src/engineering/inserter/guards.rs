//! Complete supplied values finish through the actual insertion metadata owner.
use super::AccessEngineeringInserter;
use crate::NativeEditError;
use crate::engineering::guards::{GuardBackendError, GuardPreparationSummary, summary};
use tablecraft_ace::{BeforeEventGuardSelector, BeforeEventIntents, RowInsert};
use tablecraft_core::{limits::ResourceBudget, values::Value};

impl AccessEngineeringInserter {
    pub(in crate::engineering) fn prepare_before_guards(
        self,
        selectors: &[BeforeEventGuardSelector<'_>],
        values: Vec<Value>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<GuardPreparationSummary, GuardBackendError> {
        summary::check(budget, cancelled)?;
        if self.policy.is_some() || values.len() != self.columns.len() {
            return Err(NativeEditError::Unsupported.into());
        }
        let requests = [RowInsert::new(values)];
        let report = self.image.prepare_ace16_before_event_guards(
            &self.name,
            selectors,
            BeforeEventIntents::Insert(&requests),
            budget,
            cancelled,
        )?;
        summary::retain(&report, requests.len(), budget, cancelled)
    }
}
