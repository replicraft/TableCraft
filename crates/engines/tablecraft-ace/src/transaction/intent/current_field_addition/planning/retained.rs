//! Retained declarations retain their own source and V3 persistence boundary.
use super::RecoverableCurrentFieldAdditionPlan;
#[cfg(feature = "ace16-create")]
use super::{Candidate, Error, Intent};
#[cfg(feature = "ace16-create")]
use crate::{NativeImage, header::check};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    self, DurableStorage, JournalError, PreparedResizableImage, ResizableDurableStorage,
};

#[cfg(feature = "ace16-create")]
impl NativeImage {
    /// Seal the retained nullable LONG/default-zero request as TCAC0010.
    pub fn plan_recoverable_retained_number_field_addition(
        self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableCurrentFieldAdditionPlan, Error> {
        let _span = tracing::debug_span!("native.retained_number_field_addition.plan").entered();
        let candidate = Candidate::Number(
            self.prepare_retained_number_field_addition(table, column, budget, cancelled)?,
        );
        let intent = Intent::capture(&candidate, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(RecoverableCurrentFieldAdditionPlan { candidate, intent })
    }

    /// Seal retained nullable Short Text, no default and explicit AZL, as TCAC0011.
    pub fn plan_recoverable_retained_short_text_field_addition(
        self,
        table: &str,
        column: &str,
        max_utf16_units: usize,
        unicode_compression: bool,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableCurrentFieldAdditionPlan, Error> {
        let _span =
            tracing::debug_span!("native.retained_short_text_field_addition.plan").entered();
        let candidate = Candidate::ShortText(self.prepare_retained_short_text_field_addition(
            table,
            column,
            max_utf16_units,
            unicode_compression,
            budget,
            cancelled,
        )?);
        let intent = Intent::capture(&candidate, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(RecoverableCurrentFieldAdditionPlan { candidate, intent })
    }
}

impl RecoverableCurrentFieldAdditionPlan {
    /// Prepare only retained10/11 through the same explicitly checked V3 owner.
    pub fn prepare_retained<'a, D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedResizableImage<'a, D, J>, JournalError> {
        let _span = tracing::debug_span!("native.retained_field_addition.prepare").entered();
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
