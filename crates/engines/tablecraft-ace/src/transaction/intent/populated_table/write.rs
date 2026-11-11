//! Only the sealed populated candidate can prepare a V3 native image journal.
use super::{PopulatedTableIntentError, binding, codec};
use crate::PopulatedAddTablePlan;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    self, CommitError, DurableStorage, JournalError, PreparedResizableImage, RecoveryOutcome,
    ResizableDurableStorage,
};

/// One source-bound V3 journal for the complete populated image.
/// Dropping retains the journal and requires explicit recovery/diagnosis.
#[must_use = "commit or explicitly roll back the retained populated-table journal"]
pub struct PreparedPopulatedAddTable<'a, D, J> {
    prepared: PreparedResizableImage<'a, D, J>,
}

impl PopulatedAddTablePlan {
    /// Serialize the original declaration and materialized values against their
    /// complete actual BEFORE. No query or native field encoding is persisted.
    pub fn encode_intent(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, PopulatedTableIntentError> {
        let binding = binding::digest_retained(self.before_bytes(), budget, cancelled)?;
        codec::encode(
            self.definition(),
            self.rows(),
            self.before_bytes().len() as u64,
            binding,
            budget,
            cancelled,
        )
    }

    /// Prepare only through V3 against this candidate's exact complete BEFORE.
    /// The host supplies exclusive association and durable original intent;
    /// this method creates no file, namespace or alternate protocol route.
    pub fn prepare_populated<'a, D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedPopulatedAddTable<'a, D, J>, JournalError> {
        let _span = tracing::debug_span!("native.populated_table.prepare").entered();
        let prepared = journal::prepare_resizable_v3_expected(
            target,
            journal,
            self.before_bytes(),
            self.after_bytes(),
            budget,
            cancelled,
        )?;
        Ok(PreparedPopulatedAddTable { prepared })
    }
}

impl<D: ResizableDurableStorage, J: DurableStorage> PreparedPopulatedAddTable<'_, D, J> {
    /// Commit the already proved declaration and entire selected relation once.
    pub fn commit(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), CommitError> {
        let _span = tracing::debug_span!("native.populated_table.commit").entered();
        self.prepared.commit(budget, cancelled)
    }

    /// Restore exact original bytes and length while retaining the same journal.
    pub fn rollback(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span = tracing::debug_span!("native.populated_table.rollback").entered();
        self.prepared.rollback(budget, cancelled)
    }
}
