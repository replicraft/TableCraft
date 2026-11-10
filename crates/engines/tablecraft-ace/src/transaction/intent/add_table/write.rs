use super::{AddTableIntentError, codec};
use crate::{AddTablePlan, AddTableSourceProfile};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    self, CommitError, DurableStorage, JournalError, PreparedResizableImage, RecoveryOutcome,
    ResizableDurableStorage,
};

/// A source-bound add-table transition retaining its selected image journal.
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
        let profile = if self.is_native() {
            match self.source_profile() {
                AddTableSourceProfile::Mixed => codec::Profile::NativeMixed,
                AddTableSourceProfile::Retained => codec::Profile::NativeRetained,
                AddTableSourceProfile::LegacyGenerated => {
                    return Err(AddTableIntentError::UnsupportedProfile);
                }
            }
        } else {
            match self.source_profile() {
                AddTableSourceProfile::LegacyGenerated => {
                    codec::legacy_profile(self.before_bytes().len() as u64)
                        .ok_or(AddTableIntentError::BeforeMismatch)?
                }
                AddTableSourceProfile::Mixed => codec::Profile::Mixed,
                AddTableSourceProfile::Retained => codec::Profile::Retained,
            }
        };
        let before_binding = profile.digest(self.before_bytes(), budget, cancelled)?;
        codec::encode(
            self.definition(),
            profile,
            self.before_bytes().len() as u64,
            before_binding,
            budget,
            cancelled,
        )
    }

    /// Prepare against the retained target's exact complete BEFORE bytes.
    /// Retained sources use V3; older construction families keep their V2 grammar.
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
        if self.is_native() {
            return Err(JournalError::InvalidJournal);
        }
        let prepared = match self.source_profile() {
            AddTableSourceProfile::Retained => journal::prepare_resizable_v3_expected(
                target,
                journal,
                self.before_bytes(),
                self.after_bytes(),
                budget,
                cancelled,
            )?,
            AddTableSourceProfile::LegacyGenerated | AddTableSourceProfile::Mixed => {
                journal::prepare_resizable_expected(
                    target,
                    journal,
                    self.before_bytes(),
                    self.after_bytes(),
                    budget,
                    cancelled,
                )?
            }
        };
        Ok(PreparedAddTable { prepared })
    }

    /// Prepare only native TCAT7 construction through its selected V3 journal.
    pub fn prepare_native<'a, D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedAddTable<'a, D, J>, JournalError> {
        if !self.is_native() {
            return Err(JournalError::InvalidJournal);
        }
        let prepared = journal::prepare_resizable_v3_expected(
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
