//! Closed actual INSERT families; omitted presence never becomes a TCNA1 body.
use tablecraft_ace::{
    NativeIntentError, PreparedNativeEdit, RecoverableNativeInsertPlan,
    RecoverableNativeOmittedInsertPlan,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{DurableStorage, JournalError};

/// A qualified complete-row or original-presence native INSERT operation.
/// Both variants retain source-bound native admission and independent proof.
pub enum NativeJournalInsertPlan {
    /// Exact complete supplied rows, retaining the original TCNA0001 contract.
    Complete(RecoverableNativeInsertPlan),
    /// Original supplied/omitted fields, retaining the distinct TCNA0002 body.
    Omitted(RecoverableNativeOmittedInsertPlan),
}

impl NativeJournalInsertPlan {
    /// Complete original source bytes, not a storage capability.
    pub fn before_bytes(&self) -> &[u8] {
        match self {
            Self::Complete(plan) => plan.before_bytes(),
            Self::Omitted(plan) => plan.before_bytes(),
        }
    }

    /// Complete independently checked replacement bytes.
    pub fn after_bytes(&self) -> &[u8] {
        match self {
            Self::Complete(plan) => plan.after_bytes(),
            Self::Omitted(plan) => plan.after_bytes(),
        }
    }

    /// Number of explicitly requested rows, without a matched-old-row count.
    pub fn inserted_rows(&self) -> usize {
        match self {
            Self::Complete(plan) => plan.inserted_rows(),
            Self::Omitted(plan) => plan.inserted_rows(),
        }
    }

    /// Encode the exact original native family; no expanded defaults are substituted.
    pub fn encode_intent(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, NativeIntentError> {
        match self {
            Self::Complete(plan) => plan.intent().encode(budget, cancelled),
            Self::Omitted(plan) => plan.intent().encode(budget, cancelled),
        }
    }

    /// Prepare the same fixed-length V1 durable family through caller-owned storage.
    pub fn prepare<'a, D: DurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedNativeEdit<'a, D, J>, JournalError> {
        match self {
            Self::Complete(plan) => plan.prepare(target, journal, budget, cancelled),
            Self::Omitted(plan) => plan.prepare(target, journal, budget, cancelled),
        }
    }
}
