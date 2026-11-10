//! Native preparation retains the qualified operation's fixed or resizable family.
use super::write::PreparedNativeEdit;
use crate::NativeEditPlan;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    self, CommitError, DurableStorage, JournalError, PreparedResizableImage, RecoveryOutcome,
    ResizableDurableStorage,
};

/// A prepared native replacement with an explicitly resizable target capability.
/// The host retains exclusive offline access and the same durable journal pair.
#[must_use = "commit or explicitly recover the selected resizable native edit journal"]
pub struct PreparedResizableNativeEdit<'a, D, J> {
    prepared: PreparedResizableImage<'a, D, J>,
    changed_rows: usize,
}

/// One normal operation's prepared journal family, chosen before journal effects.
#[must_use = "commit or explicitly recover the retained native edit journal"]
pub enum PreparedNativeUpdate<'a, D, J> {
    /// Existing equal-length operations retain their original V1 journal.
    Fixed(PreparedNativeEdit<'a, D, J>),
    /// Qualified allocation or retained images use their selected V2/V3 journal.
    Resizable(PreparedResizableNativeEdit<'a, D, J>),
}

impl NativeEditPlan {
    pub(in crate::transaction) fn prepare_resizable<
        'a,
        D: ResizableDurableStorage,
        J: DurableStorage,
    >(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedResizableNativeEdit<'a, D, J>, JournalError> {
        let _span = tracing::debug_span!("native.edit.prepare_resizable").entered();
        let prepared = journal::prepare_resizable_expected(
            target,
            journal,
            self.before_bytes(),
            self.after_bytes(),
            budget,
            cancelled,
        )?;
        Ok(PreparedResizableNativeEdit {
            prepared,
            changed_rows: self.changed_rows(),
        })
    }

    pub(in crate::transaction) fn prepare_retained<
        'a,
        D: ResizableDurableStorage,
        J: DurableStorage,
    >(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedResizableNativeEdit<'a, D, J>, JournalError> {
        let _span = tracing::debug_span!("native.edit.prepare_retained").entered();
        let prepared = journal::prepare_resizable_v3_expected(
            target,
            journal,
            self.before_bytes(),
            self.after_bytes(),
            budget,
            cancelled,
        )?;
        Ok(PreparedResizableNativeEdit {
            prepared,
            changed_rows: self.changed_rows(),
        })
    }
}

impl<D: ResizableDurableStorage, J: DurableStorage> PreparedResizableNativeEdit<'_, D, J> {
    /// Commit the qualified image and return its original changed-row count.
    pub fn commit(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<usize, CommitError> {
        let _span = tracing::debug_span!("native.edit.commit_resizable").entered();
        self.prepared.commit(budget, cancelled)?;
        Ok(self.changed_rows)
    }

    /// Restore the original bytes and length, retaining the journal for cleanup.
    pub fn rollback(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.prepared.rollback(budget, cancelled)
    }
}

impl<D: ResizableDurableStorage, J: DurableStorage> PreparedNativeUpdate<'_, D, J> {
    /// Complete the already selected journal family without switching protocols.
    pub fn commit(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<usize, CommitError> {
        match self {
            Self::Fixed(prepared) => prepared.commit(budget, cancelled),
            Self::Resizable(prepared) => prepared.commit(budget, cancelled),
        }
    }

    /// Roll back using the capability required by the already selected family.
    pub fn rollback(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        match self {
            Self::Fixed(prepared) => prepared.rollback(budget, cancelled),
            Self::Resizable(prepared) => prepared.rollback(budget, cancelled),
        }
    }
}
