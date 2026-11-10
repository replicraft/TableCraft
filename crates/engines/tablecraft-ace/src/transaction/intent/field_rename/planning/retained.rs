//! Retained schema rename binds its original identity and selects V3 explicitly.
use super::{Candidate, Error, Intent, RecoverableFieldRenamePlan};
use crate::{NativeImage, header::check};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    self, DurableStorage, JournalError, PreparedResizableImage, ResizableDurableStorage,
};

impl NativeImage {
    /// Bind only the positively qualified retained schema field rename source.
    pub fn plan_recoverable_retained_schema_field_rename(
        self,
        table: &str,
        column: &str,
        new_name: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableFieldRenamePlan, Error> {
        let _span = tracing::debug_span!("native.retained_schema_field_rename.plan").entered();
        let candidate = Candidate::RetainedAscii(
            self.prepare_retained_schema_field_rename(table, column, new_name, budget, cancelled)?,
        );
        let intent = Intent::capture(
            super::super::Profile::RetainedSchemaAscii,
            &candidate,
            budget,
            cancelled,
        )?;
        check(budget, cancelled)?;
        RecoverableFieldRenamePlan::new(candidate, intent, budget, cancelled)
    }
}

impl RecoverableFieldRenamePlan {
    /// Prepare only retained TCFR6 with the same checked V3 journal owner.
    pub fn prepare_retained<'a, D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedResizableImage<'a, D, J>, JournalError> {
        let _span = tracing::debug_span!("native.retained_schema_field_rename.prepare").entered();
        if !self.intent.is_retained_schema() {
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
