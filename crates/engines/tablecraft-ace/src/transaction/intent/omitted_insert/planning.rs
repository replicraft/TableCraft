//! Normal insertion planning and the existing fixed-length durable prepare path.
mod requalify;
use super::NativeOmittedInsertIntent;
use crate::transaction::intent::RecoverableEditError;
use crate::{
    EditError, NativeEditPlan, NativeImage, OmittedRowInsert, PreparedNativeEdit,
    PreparedResizableNativeEdit, header::check,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{DurableStorage, JournalError, ResizableDurableStorage};

/// A qualified normal INSERT plan retaining the exact original presence rows.
/// There is no public constructor from journal bytes or engineering candidates.
pub struct RecoverableNativeOmittedInsertPlan {
    plan: NativeEditPlan,
    intent: NativeOmittedInsertIntent,
}

impl NativeImage {
    /// Bind original current-history presence before qualified INSERT planning.
    /// Older omission families keep their original source and default semantics.
    #[cfg(feature = "ace16-create")]
    pub fn plan_recoverable_current_omitted_row_inserts(
        self,
        table: &str,
        rows: Vec<OmittedRowInsert>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeOmittedInsertPlan, RecoverableEditError> {
        let _span =
            tracing::debug_span!("native.current_omitted_insert.plan_recoverable").entered();
        if !self.select_native_current_insert_source(table, budget, cancelled)? {
            return Err(EditError::Unsupported(crate::WriteBlocker::RowLayout).into());
        }
        let intent = NativeOmittedInsertIntent::new_current(
            table,
            rows,
            self.as_bytes(),
            budget,
            cancelled,
        )?;
        let plan = intent.plan(self, budget, cancelled)?;
        if plan.before_bytes().len() != plan.after_bytes().len()
            || plan.changed_rows() != intent.rows().len()
        {
            return Err(EditError::Preservation.into());
        }
        check(budget, cancelled).map_err(EditError::from)?;
        Ok(RecoverableNativeOmittedInsertPlan { plan, intent })
    }

    /// Retained surroundings use their own source purpose and TCNA0008 request.
    #[cfg(feature = "ace16-create")]
    pub fn plan_recoverable_retained_omitted_row_inserts(
        self,
        table: &str,
        rows: Vec<OmittedRowInsert>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeOmittedInsertPlan, RecoverableEditError> {
        let _span =
            tracing::debug_span!("native.retained_omitted_insert.plan_recoverable").entered();
        if !self
            .has_retained_table_source(table, budget, cancelled)
            .map_err(EditError::from)?
        {
            return Err(EditError::Unsupported(crate::WriteBlocker::RowLayout).into());
        }
        let intent = NativeOmittedInsertIntent::new_retained(
            table,
            rows,
            self.as_bytes(),
            budget,
            cancelled,
        )?;
        let plan = intent.plan(self, budget, cancelled)?;
        if plan.before_bytes().len() != plan.after_bytes().len()
            || plan.changed_rows() != intent.rows().len()
        {
            return Err(EditError::Preservation.into());
        }
        check(budget, cancelled).map_err(EditError::from)?;
        Ok(RecoverableNativeOmittedInsertPlan { plan, intent })
    }

    /// Opaque retained surroundings require positive original geometry and TCNA0009.
    #[cfg(feature = "ace16-create")]
    pub fn plan_recoverable_retained_opaque_omitted_row_inserts(
        self,
        table: &str,
        rows: Vec<OmittedRowInsert>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeOmittedInsertPlan, RecoverableEditError> {
        let _span = tracing::debug_span!("native.retained_opaque_omitted_insert.plan_recoverable")
            .entered();
        if !self.select_native_retained_opaque_insert_table(table, budget, cancelled)? {
            return Err(EditError::Unsupported(crate::WriteBlocker::RowLayout).into());
        }
        let intent = NativeOmittedInsertIntent::new_retained_opaque(
            table,
            rows,
            self.as_bytes(),
            budget,
            cancelled,
        )?;
        let plan = intent.plan(self, budget, cancelled)?;
        if plan.before_bytes().len() != plan.after_bytes().len()
            || plan.changed_rows() != intent.rows().len()
        {
            return Err(EditError::Preservation.into());
        }
        check(budget, cancelled).map_err(EditError::from)?;
        Ok(RecoverableNativeOmittedInsertPlan { plan, intent })
    }

    /// Qualify native target-local absence and reconstruct original presence/default insertion.
    /// The retained intent contains requests, not final patches or permissions.
    pub fn plan_recoverable_ordinary_omitted_row_inserts(
        self,
        table: &str,
        rows: Vec<OmittedRowInsert>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeOmittedInsertPlan, RecoverableEditError> {
        let _span =
            tracing::debug_span!("native.ordinary_omitted_insert.plan_recoverable").entered();
        let intent =
            NativeOmittedInsertIntent::new(table, rows, self.as_bytes(), budget, cancelled)?;
        let plan =
            self.plan_ordinary_omitted_row_inserts(table, intent.rows(), budget, cancelled)?;
        if plan.before_bytes().len() != plan.after_bytes().len()
            || plan.changed_rows() != intent.rows().len()
        {
            return Err(EditError::Preservation.into());
        }
        check(budget, cancelled).map_err(EditError::from)?;
        Ok(RecoverableNativeOmittedInsertPlan { plan, intent })
    }
}

impl RecoverableNativeOmittedInsertPlan {
    /// Original presence request record bound to this plan's complete BEFORE image.
    pub fn intent(&self) -> &NativeOmittedInsertIntent {
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

    /// Number of explicitly requested rows added by this plan.
    pub fn inserted_rows(&self) -> usize {
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
        if self.intent.is_retained() || self.intent.is_retained_opaque() {
            return Err(JournalError::ImageSize);
        }
        self.plan.prepare(target, journal, budget, cancelled)
    }

    /// Prepare only TCNA8/9 through the V3 journal; original V1 families refuse.
    pub fn prepare_retained<'a, D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedResizableNativeEdit<'a, D, J>, JournalError> {
        if !(self.intent.is_retained() || self.intent.is_retained_opaque()) {
            return Err(JournalError::ImageSize);
        }
        self.plan
            .prepare_retained(target, journal, budget, cancelled)
    }
}
