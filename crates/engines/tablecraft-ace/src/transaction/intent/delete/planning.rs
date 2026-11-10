//! Normal deletion planning and the existing fixed-length durable prepare path.
mod requalify;
use super::NativeDeleteIntent;
use crate::transaction::intent::RecoverableEditError;
use crate::{NativeEditPlan, NativeImage, PreparedNativeEdit, RowDelete};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{DurableStorage, JournalError, ResizableDurableStorage};

/// A qualified normal DELETE plan retaining the exact original raw requests.
/// There is no public constructor from journal bytes or engineering candidates.
pub struct RecoverableNativeDeletePlan {
    plan: NativeEditPlan,
    intent: NativeDeleteIntent,
}

impl NativeImage {
    /// Bind original current-source deletion witnesses to the TCND0002 family.
    #[cfg(feature = "ace16-create")]
    pub fn plan_recoverable_current_row_deletes(
        self,
        table: &str,
        deletes: Vec<RowDelete>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeDeletePlan, RecoverableEditError> {
        let _span = tracing::debug_span!("native.current_delete.plan_recoverable").entered();
        let intent =
            NativeDeleteIntent::new_current(table, deletes, self.as_bytes(), budget, cancelled)?;
        let plan = intent.plan(self, budget, cancelled)?;
        Ok(RecoverableNativeDeletePlan { plan, intent })
    }

    /// Rebuild retained DELETE after binding its exact ordered raw witnesses.
    /// Empty selections still consume the complete retained source proof.
    #[cfg(feature = "ace16-create")]
    pub fn plan_recoverable_retained_row_deletes(
        self,
        table: &str,
        deletes: Vec<RowDelete>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeDeletePlan, RecoverableEditError> {
        let _span = tracing::debug_span!("native.retained_delete.plan_recoverable").entered();
        if !self
            .has_retained_table_source(table, budget, cancelled)
            .map_err(crate::EditError::from)?
        {
            return Err(crate::EditError::Unsupported(crate::WriteBlocker::RowLayout).into());
        }
        let intent =
            NativeDeleteIntent::new_retained(table, deletes, self.as_bytes(), budget, cancelled)?;
        let plan = intent.plan(self, budget, cancelled)?;
        if plan.before_bytes().len() != plan.after_bytes().len()
            || plan.changed_rows() != intent.deletes().len()
        {
            return Err(crate::EditError::Preservation.into());
        }
        crate::header::check(budget, cancelled).map_err(crate::EditError::from)?;
        Ok(RecoverableNativeDeletePlan { plan, intent })
    }

    /// Qualify native target-local absence and reconstruct original row deletion.
    /// The retained intent contains requests, not final patches or permissions.
    pub fn plan_recoverable_ordinary_row_deletes(
        self,
        table: &str,
        deletes: Vec<RowDelete>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeDeletePlan, RecoverableEditError> {
        let _span = tracing::debug_span!("native.ordinary_delete.plan_recoverable").entered();
        let intent = NativeDeleteIntent::new(table, deletes, self.as_bytes(), budget, cancelled)?;
        let plan = self.plan_ordinary_row_deletes(table, intent.deletes(), budget, cancelled)?;
        Ok(RecoverableNativeDeletePlan { plan, intent })
    }
}

impl RecoverableNativeDeletePlan {
    /// Original request record bound to this plan's complete BEFORE image.
    pub fn intent(&self) -> &NativeDeleteIntent {
        &self.intent
    }

    /// Complete qualified original bytes.
    pub fn before_bytes(&self) -> &[u8] {
        self.plan.before_bytes()
    }

    /// Complete qualified replacement bytes, not a durable completion receipt.
    pub fn after_bytes(&self) -> &[u8] {
        self.plan.after_bytes()
    }

    /// Number of original live roots removed by this plan.
    pub fn deleted_rows(&self) -> usize {
        self.plan.changed_rows()
    }

    /// Prepare the existing V1 expected-image journal using caller-owned storage.
    /// Hosts retain namespace, exclusivity and interrupted-recovery obligations.
    pub fn prepare<'a, D: DurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedNativeEdit<'a, D, J>, JournalError> {
        if self.intent.is_retained() {
            return Err(JournalError::ImageSize);
        }
        self.plan.prepare(target, journal, budget, cancelled)
    }

    /// Prepare only TCND3 through its distinct V3 journal family.
    pub fn prepare_retained<'a, D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<crate::PreparedResizableNativeEdit<'a, D, J>, JournalError> {
        if !self.intent.is_retained() {
            return Err(JournalError::ImageSize);
        }
        self.plan
            .prepare_retained(target, journal, budget, cancelled)
    }
}
