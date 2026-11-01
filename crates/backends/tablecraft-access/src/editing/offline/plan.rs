//! Private normal mutation composition; UPDATE and DELETE keep distinct intents.
use tablecraft_ace::{
    NativeDeleteIntent, NativeIntentError, NativeUpdateIntent, PreparedNativeUpdate,
    RecoverableNativeDeletePlan, RecoverableNativeEditPlan,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{DurableStorage, JournalError, ResizableDurableStorage};

pub(super) enum MutationPlan {
    Update(RecoverableNativeEditPlan),
    Delete(RecoverableNativeDeletePlan),
}

impl MutationPlan {
    pub(super) fn changed_rows(&self) -> usize {
        match self {
            Self::Update(plan) => plan.changed_rows(),
            Self::Delete(plan) => plan.deleted_rows(),
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
        }
    }
}

pub(super) enum MutationIntent {
    Update(NativeUpdateIntent),
    Delete(NativeDeleteIntent),
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
        } else {
            NativeUpdateIntent::decode(raw, budget, cancelled).map(Self::Update)
        }
    }
}
