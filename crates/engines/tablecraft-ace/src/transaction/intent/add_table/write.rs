use super::{AddTableIntentError, binding, codec};
use crate::AddTablePlan;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    self, CommitError, DurableStorage, JournalError, PreparedResizableImage, RecoveryOutcome,
    ResizableDurableStorage,
};

/// The source-bound add-table transition prepared through one V2 image journal.
/// Dropping retains the journal; callers must explicitly recover before access.
#[must_use = "commit or explicitly recover the retained add-table journal"]
pub struct PreparedAddTable<'a, D, J> {
    prepared: PreparedResizableImage<'a, D, J>,
}

impl AddTablePlan {
    /// Encode original declarations and the complete actual BEFORE binding.
    /// Encoding supplies no source, persistence or recovery permission.
    pub fn encode_intent(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, AddTableIntentError> {
        let before_binding = binding::digest(self.before_bytes(), budget, cancelled)?;
        codec::encode(
            self.definition(),
            self.before_bytes().len() as u64,
            before_binding,
            budget,
            cancelled,
        )
    }

    /// Prepare only against the retained target's exact populated BEFORE bytes.
    /// The host owns exclusive target/journal association and durable namespace
    /// setup. This method creates no file or journal directory.
    pub fn prepare<'a, D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedAddTable<'a, D, J>, JournalError> {
        let _span = tracing::debug_span!("native.add_table.prepare").entered();
        let prepared = journal::prepare_resizable_expected(
            target,
            journal,
            self.before_bytes(),
            self.after_bytes(),
            budget,
            cancelled,
        )?;
        Ok(PreparedAddTable { prepared })
    }
}

impl<D: ResizableDurableStorage, J: DurableStorage> PreparedAddTable<'_, D, J> {
    /// Commit the already proved schema image, without inventing row counts.
    pub fn commit(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), CommitError> {
        let _span = tracing::debug_span!("native.add_table.commit").entered();
        self.prepared.commit(budget, cancelled)
    }

    /// Restore the exact original bytes and length, retaining the journal.
    pub fn rollback(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span = tracing::debug_span!("native.add_table.rollback").entered();
        self.prepared.rollback(budget, cancelled)
    }
}
