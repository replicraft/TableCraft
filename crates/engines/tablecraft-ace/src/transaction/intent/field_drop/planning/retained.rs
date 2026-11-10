//! Retained tail DROP binds its original identity and selects V3 explicitly.
use super::{Candidate, Error, Intent, RecoverableFieldDropPlan};
use crate::{NativeImage, header::check};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    self, DurableStorage, JournalError, PreparedResizableImage, ResizableDurableStorage,
};

impl NativeImage {
    /// Bind only the positively qualified retained tail-field DROP source.
    pub fn plan_recoverable_retained_tail_field_drop(
        self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableFieldDropPlan, Error> {
        let _span = tracing::debug_span!("native.retained_tail_field_drop.plan").entered();
        let candidate = Candidate::populated_tail(
            self.prepare_retained_tail_field_drop(table, column, budget, cancelled)?,
            budget,
            cancelled,
        )?;
        let intent = Intent::capture(&candidate, budget, cancelled)?;
        check(budget, cancelled)?;
        RecoverableFieldDropPlan::new(candidate, intent, budget, cancelled)
    }
}

impl RecoverableFieldDropPlan {
    /// Prepare only retained TCFD3 with the same checked V3 journal owner.
    pub fn prepare_retained<'a, D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedResizableImage<'a, D, J>, JournalError> {
        let _span = tracing::debug_span!("native.retained_tail_field_drop.prepare").entered();
        if !self.intent.is_retained() {
            return Err(JournalError::ImageSize);
        }
        journal::prepare_resizable_v3_expected(
            target,
            journal,
            self.before_bytes(),
            self.after_bytes(),
            budget,
            cancelled,
        )
    }
}
