//! Private normal mutation composition; UPDATE, DELETE and INSERT keep distinct intents.
use crate::editing::journal::NativeJournalInsertPlan;
use tablecraft_ace::{
    NativeCompleteInsertIntent, NativeDeleteIntent, NativeEofInsertIntent, NativeInsertIntent,
    NativeIntentError, NativeOmittedInsertIntent, NativeUpdateIntent, PreparedNativeUpdate,
    RecoverableNativeDeletePlan, RecoverableNativeEditPlan,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{DurableStorage, JournalError, ResizableDurableStorage};

pub(super) enum MutationPlan {
    Update(RecoverableNativeEditPlan),
    Delete(RecoverableNativeDeletePlan),
    Insert(NativeJournalInsertPlan),
}

impl MutationPlan {
    pub(super) fn changed_rows(&self) -> usize {
        match self {
            Self::Update(plan) => plan.changed_rows(),
            Self::Delete(plan) => plan.deleted_rows(),
            Self::Insert(plan) => plan.inserted_rows(),
        }
    }

    pub(super) fn encode_intent(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, NativeIntentError> {
        match self {
            Self::Update(plan) => plan.intent().encode(budget, cancelled),
            Self::Delete(plan) => plan.intent().encode(budget, cancelled),
            Self::Insert(plan) => plan.encode_intent(budget, cancelled),
        }
    }

    pub(super) fn prepare<'a, D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedNativeUpdate<'a, D, J>, JournalError> {
        match self {
            Self::Update(plan) => plan.prepare_update(target, journal, budget, cancelled),
            Self::Delete(plan) => plan
                .prepare(target, journal, budget, cancelled)
                .map(PreparedNativeUpdate::Fixed),
            Self::Insert(plan) => plan.prepare_insert(target, journal, budget, cancelled),
        }
    }
}

pub(super) enum MutationIntent {
    Update(NativeUpdateIntent),
    Delete(NativeDeleteIntent),
    Insert(NativeInsertIntent),
    EofInsert(NativeEofInsertIntent),
    CompleteInsert(NativeCompleteInsertIntent),
    OmittedInsert(NativeOmittedInsertIntent),
}

impl MutationIntent {
    pub(super) fn decode(
        raw: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        // Closed framing selection occurs before decode; a failed decoder is
        // never retried as another action or promoted from its record prefix.
        if raw.starts_with(b"TCND") {
            NativeDeleteIntent::decode(raw, budget, cancelled).map(Self::Delete)
        } else if raw.starts_with(b"TCNA0004") {
            NativeCompleteInsertIntent::decode(raw, budget, cancelled).map(Self::CompleteInsert)
        } else if raw.starts_with(b"TCNA0003") {
            NativeEofInsertIntent::decode(raw, budget, cancelled).map(Self::EofInsert)
        } else if raw.starts_with(b"TCNA0002") {
            NativeOmittedInsertIntent::decode(raw, budget, cancelled).map(Self::OmittedInsert)
        } else if raw.starts_with(b"TCNA") {
            NativeInsertIntent::decode(raw, budget, cancelled).map(Self::Insert)
        } else {
            NativeUpdateIntent::decode(raw, budget, cancelled).map(Self::Update)
        }
    }
}
