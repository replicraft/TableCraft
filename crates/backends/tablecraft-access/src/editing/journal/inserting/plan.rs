//! Closed actual INSERT families; omitted presence never becomes a TCNA1 body.
use tablecraft_ace::{
    NativeImage, NativeIntentError, NativeRecoveryQualificationError, PreparedNativeEdit,
    PreparedNativeUpdate, RecoverableNativeClockInsertPlan, RecoverableNativeCompleteInsertPlan,
    RecoverableNativeEofInsertPlan, RecoverableNativeInsertPlan,
    RecoverableNativeOmittedInsertPlan,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{DurableStorage, JournalError, ResizableDurableStorage};

/// A qualified complete-row or original-presence native INSERT operation.
/// All variants retain source-bound native admission and independent proof.
pub enum NativeJournalInsertPlan {
    /// Complete original exact carriers with source-derived V1/V2 geometry, TCNA4.
    CompleteTyped(RecoverableNativeCompleteInsertPlan),
    /// Exact complete supplied rows, retaining the original TCNA0001 contract.
    Complete(RecoverableNativeInsertPlan),
    /// Original complete rows with one represented EOF DATA page, TCNA3/V2.
    AllocateEof(RecoverableNativeEofInsertPlan),
    /// Original supplied/omitted fields, retaining their selected TCNA2/6/7 family.
    Omitted(RecoverableNativeOmittedInsertPlan),
    /// Original presence and one retained civil clock input, TCNA5/V1.
    Clock(RecoverableNativeClockInsertPlan),
}

impl NativeJournalInsertPlan {
    /// Requalify the same selected TCNA family against one fresh complete source.
    /// Captured clock input remains in its original intent; no provider is sampled.
    /// Neither native/proof pool is reset and no journal is created.
    pub fn requalify_with_budgets(
        self,
        fresh: NativeImage,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeRecoveryQualificationError> {
        match self {
            Self::CompleteTyped(plan) => plan
                .requalify_with_budgets(fresh, native, proof, cancelled)
                .map(Self::CompleteTyped),
            Self::Complete(plan) => plan
                .requalify_with_budgets(fresh, native, proof, cancelled)
                .map(Self::Complete),
            Self::AllocateEof(plan) => plan
                .requalify_with_budgets(fresh, native, proof, cancelled)
                .map(Self::AllocateEof),
            Self::Omitted(plan) => plan
                .requalify_with_budgets(fresh, native, proof, cancelled)
                .map(Self::Omitted),
            Self::Clock(plan) => plan
                .requalify_with_budgets(fresh, native, proof, cancelled)
                .map(Self::Clock),
        }
    }

    /// Complete original source bytes, not a storage capability.
    pub fn before_bytes(&self) -> &[u8] {
        match self {
            Self::CompleteTyped(plan) => plan.before_bytes(),
            Self::Complete(plan) => plan.before_bytes(),
            Self::AllocateEof(plan) => plan.before_bytes(),
            Self::Omitted(plan) => plan.before_bytes(),
            Self::Clock(plan) => plan.before_bytes(),
        }
    }

    /// Complete independently checked replacement bytes.
    pub fn after_bytes(&self) -> &[u8] {
        match self {
            Self::CompleteTyped(plan) => plan.after_bytes(),
            Self::Complete(plan) => plan.after_bytes(),
            Self::AllocateEof(plan) => plan.after_bytes(),
            Self::Omitted(plan) => plan.after_bytes(),
            Self::Clock(plan) => plan.after_bytes(),
        }
    }

    /// Number of explicitly requested rows, without a matched-old-row count.
    pub fn inserted_rows(&self) -> usize {
        match self {
            Self::CompleteTyped(plan) => plan.inserted_rows(),
            Self::Complete(plan) => plan.inserted_rows(),
            Self::AllocateEof(plan) => plan.inserted_rows(),
            Self::Omitted(plan) => plan.inserted_rows(),
            Self::Clock(plan) => plan.inserted_rows(),
        }
    }

    /// Encode the exact original native family; no expanded defaults are substituted.
    pub fn encode_intent(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, NativeIntentError> {
        match self {
            Self::CompleteTyped(plan) => plan.intent().encode(budget, cancelled),
            Self::Complete(plan) => plan.intent().encode(budget, cancelled),
            Self::AllocateEof(plan) => plan.intent().encode(budget, cancelled),
            Self::Omitted(plan) => plan.intent().encode(budget, cancelled),
            Self::Clock(plan) => plan.intent().encode(budget, cancelled),
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
            Self::CompleteTyped(plan) => plan.prepare(target, journal, budget, cancelled),
            Self::Complete(plan) => plan.prepare(target, journal, budget, cancelled),
            Self::AllocateEof(_) => Err(JournalError::ImageSize),
            Self::Omitted(plan) => plan.prepare(target, journal, budget, cancelled),
            Self::Clock(plan) => plan.prepare(target, journal, budget, cancelled),
        }
    }

    /// Prepare the preselected family without changing original request identity.
    /// Growing complete rows use V2; complete/omitted existing rows retain V1.
    pub fn prepare_insert<'a, D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedNativeUpdate<'a, D, J>, JournalError> {
        match self {
            Self::CompleteTyped(plan) => plan.prepare_insert(target, journal, budget, cancelled),
            Self::Complete(plan) => plan
                .prepare(target, journal, budget, cancelled)
                .map(PreparedNativeUpdate::Fixed),
            Self::Omitted(plan) => plan
                .prepare(target, journal, budget, cancelled)
                .map(PreparedNativeUpdate::Fixed),
            Self::Clock(plan) => plan
                .prepare(target, journal, budget, cancelled)
                .map(PreparedNativeUpdate::Fixed),
            Self::AllocateEof(plan) => plan
                .prepare_insert(target, journal, budget, cancelled)
                .map(PreparedNativeUpdate::Resizable),
        }
    }
}
