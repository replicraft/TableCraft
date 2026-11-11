//! Closed typed/first/repeated column request composition, never decoder fallback.
use super::super::Cause;
use tablecraft_ace::{
    InspectError, NativeImage, RecoverableCurrentFieldAdditionPlan,
    RecoverableDeclaredFieldAdditionPlan, RecoverableDefaultZeroAddColumnPlan,
    RecoverableFieldAdditionAfterOffHistoryPlan, RecoverableFieldAdditionAfterOffPlan,
    RecoverableFieldAdditionPlan, RecoverableRepeatedDefaultZeroAddColumnPlan,
    RecoverableRetainedTypedFieldAdditionPlan, RecoverableUnadornedFieldAdditionPlan,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{DurableStorage, ResizableDurableStorage};

pub(in crate::editing::offline) enum Plan {
    RetainedTyped(Box<RecoverableRetainedTypedFieldAdditionPlan>),
    Declared(Box<RecoverableDeclaredFieldAdditionPlan>),
    Unadorned(Box<RecoverableUnadornedFieldAdditionPlan>),
    First(RecoverableDefaultZeroAddColumnPlan),
    Repeated(RecoverableRepeatedDefaultZeroAddColumnPlan),
    Typed(RecoverableFieldAdditionPlan),
    AfterOff(RecoverableFieldAdditionAfterOffPlan),
    AfterOffHistory(RecoverableFieldAdditionAfterOffHistoryPlan),
    Current(Box<RecoverableCurrentFieldAdditionPlan>),
}

impl Plan {
    pub(super) fn is_retained(&self) -> bool {
        matches!(
            self,
            Self::RetainedTyped(_) | Self::Declared(_) | Self::Unadorned(_)
        ) || matches!(self, Self::Current(plan) if plan.intent().is_retained())
    }

    pub(super) fn before_bytes(&self) -> &[u8] {
        match self {
            Self::RetainedTyped(plan) => plan.before_bytes(),
            Self::Unadorned(plan) => plan.before_bytes(),
            Self::Declared(plan) => plan.before_bytes(),
            Self::First(plan) => plan.before_bytes(),
            Self::Repeated(plan) => plan.before_bytes(),
            Self::Typed(plan) => plan.before_bytes(),
            Self::AfterOff(plan) => plan.before_bytes(),
            Self::AfterOffHistory(plan) => plan.before_bytes(),
            Self::Current(plan) => plan.before_bytes(),
        }
    }

    pub(super) fn table(&self) -> &str {
        match self {
            Self::RetainedTyped(plan) => plan.table(),
            Self::Unadorned(plan) => plan.table(),
            Self::Declared(plan) => plan.table(),
            Self::First(plan) => plan.table(),
            Self::Repeated(plan) => plan.table(),
            Self::Typed(plan) => plan.table(),
            Self::AfterOff(plan) => plan.table(),
            Self::AfterOffHistory(plan) => plan.table(),
            Self::Current(plan) => plan.table(),
        }
    }

    pub(super) fn column(&self) -> &str {
        match self {
            Self::RetainedTyped(plan) => plan.column(),
            Self::Unadorned(plan) => plan.column(),
            Self::Declared(plan) => plan.column(),
            Self::First(plan) => plan.column(),
            Self::Repeated(plan) => plan.column(),
            Self::Typed(plan) => plan.column(),
            Self::AfterOff(plan) => plan.column(),
            Self::AfterOffHistory(plan) => plan.column(),
            Self::Current(plan) => plan.column(),
        }
    }

    pub(super) fn requalify_with_budgets(
        self,
        fresh: NativeImage,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Cause> {
        match self {
            Self::RetainedTyped(plan) => plan
                .requalify_with_budgets(fresh, native, proof, cancelled)
                .map(Box::new)
                .map(Self::RetainedTyped)
                .map_err(Cause::RetainedTypedFieldAdditionIntent),
            Self::Unadorned(plan) => {
                let plan = plan
                    .requalify_with_budgets(fresh, native, proof, cancelled)
                    .map_err(Cause::UnadornedFieldAdditionIntent)?;
                native
                    .charge_allocation(1, size_of::<RecoverableUnadornedFieldAdditionPlan>() as u64)
                    .map_err(InspectError::from)
                    .map_err(Cause::Native)?;
                Ok(Self::Unadorned(Box::new(plan)))
            }
            Self::Declared(plan) => {
                let plan = plan
                    .requalify_with_budgets(fresh, native, proof, cancelled)
                    .map_err(Cause::DeclaredFieldAdditionIntent)?;
                native
                    .charge_allocation(1, size_of::<RecoverableDeclaredFieldAdditionPlan>() as u64)
                    .map_err(InspectError::from)
                    .map_err(Cause::Native)?;
                Ok(Self::Declared(Box::new(plan)))
            }
            Self::First(plan) => plan
                .requalify_with_budgets(fresh, native, proof, cancelled)
                .map(Self::First)
                .map_err(Cause::DefaultZeroAddColumnIntent),
            Self::Repeated(plan) => plan
                .requalify_with_budgets(fresh, native, proof, cancelled)
                .map(Self::Repeated)
                .map_err(Cause::RepeatedDefaultZeroAddColumnIntent),
            Self::Typed(plan) => plan
                .requalify_with_budgets(fresh, native, proof, cancelled)
                .map(Self::Typed)
                .map_err(Cause::FieldAdditionIntent),
            Self::AfterOff(plan) => plan
                .requalify_with_budgets(fresh, native, proof, cancelled)
                .map(Self::AfterOff)
                .map_err(Cause::FieldAdditionAfterOffIntent),
            Self::AfterOffHistory(plan) => plan
                .requalify_with_budgets(fresh, native, proof, cancelled)
                .map(Self::AfterOffHistory)
                .map_err(Cause::FieldAdditionAfterOffHistoryIntent),
            Self::Current(plan) => plan
                .requalify_with_budgets(fresh, native, proof, cancelled)
                .map(Box::new)
                .map(Self::Current)
                .map_err(Cause::CurrentFieldAdditionIntent),
        }
    }

    pub(in crate::editing::offline) fn encode_intent(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, Cause> {
        match self {
            Self::RetainedTyped(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::RetainedTypedFieldAdditionIntent),
            Self::Unadorned(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::UnadornedFieldAdditionIntent),
            Self::Declared(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::DeclaredFieldAdditionIntent),
            Self::First(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::DefaultZeroAddColumnIntent),
            Self::Repeated(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::RepeatedDefaultZeroAddColumnIntent),
            Self::Typed(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::FieldAdditionIntent),
            Self::AfterOff(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::FieldAdditionAfterOffIntent),
            Self::AfterOffHistory(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::FieldAdditionAfterOffHistoryIntent),
            Self::Current(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::CurrentFieldAdditionIntent),
        }
    }

    pub(in crate::editing::offline) fn commit<D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &mut D,
        journal: &mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), Cause> {
        // Retained10/11/12 and declared13/unadorned14 select V3; all original profiles keep checked V2.
        let prepared = match self {
            Self::RetainedTyped(plan) => {
                return plan
                    .prepare_retained(target, journal, budget, cancelled)
                    .map_err(Cause::Prepare)?
                    .commit(budget, cancelled)
                    .map_err(Cause::Commit);
            }
            Self::Unadorned(plan) => {
                return plan
                    .prepare_retained(target, journal, budget, cancelled)
                    .map_err(Cause::Prepare)?
                    .commit(budget, cancelled)
                    .map_err(Cause::Commit);
            }
            Self::Declared(plan) => {
                return plan
                    .prepare_retained(target, journal, budget, cancelled)
                    .map_err(Cause::Prepare)?
                    .commit(budget, cancelled)
                    .map_err(Cause::Commit);
            }
            Self::First(plan) => plan.prepare(target, journal, budget, cancelled),
            Self::Repeated(plan) => plan.prepare(target, journal, budget, cancelled),
            Self::Typed(plan) => plan.prepare(target, journal, budget, cancelled),
            Self::AfterOff(plan) => plan.prepare(target, journal, budget, cancelled),
            Self::AfterOffHistory(plan) => plan.prepare(target, journal, budget, cancelled),
            Self::Current(plan) if plan.intent().is_retained() => {
                return plan
                    .prepare_retained(target, journal, budget, cancelled)
                    .map_err(Cause::Prepare)?
                    .commit(budget, cancelled)
                    .map_err(Cause::Commit);
            }
            Self::Current(plan) => plan.prepare(target, journal, budget, cancelled),
        }
        .map_err(Cause::Prepare)?;
        prepared.commit(budget, cancelled).map_err(Cause::Commit)
    }
}
