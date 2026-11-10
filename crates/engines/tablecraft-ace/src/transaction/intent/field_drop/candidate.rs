//! Distinct native source proofs share one original-request journal owner.
use super::{FieldDropIntentError, codec::Profile};
use crate::header::check;
use crate::{
    NativeCurrentTailFieldDropCandidate, NativeFieldDropCandidate, NativeFieldDropIdentity,
};
use tablecraft_core::limits::ResourceBudget;

pub(super) enum Candidate {
    Empty(Box<NativeFieldDropCandidate>),
    PopulatedTail(Box<NativeCurrentTailFieldDropCandidate>),
}

impl Candidate {
    pub(super) fn empty(
        candidate: NativeFieldDropCandidate,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, FieldDropIntentError> {
        check(budget, cancelled)?;
        budget.charge_allocation(1, size_of::<NativeFieldDropCandidate>() as u64)?;
        Ok(Self::Empty(Box::new(candidate)))
    }

    pub(super) fn populated_tail(
        candidate: NativeCurrentTailFieldDropCandidate,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, FieldDropIntentError> {
        check(budget, cancelled)?;
        budget.charge_allocation(1, size_of::<NativeCurrentTailFieldDropCandidate>() as u64)?;
        Ok(Self::PopulatedTail(Box::new(candidate)))
    }

    pub(super) fn before_bytes(&self) -> &[u8] {
        match self {
            Self::Empty(candidate) => candidate.before_bytes(),
            Self::PopulatedTail(candidate) => candidate.before_bytes(),
        }
    }

    pub(super) fn after_bytes(&self) -> &[u8] {
        match self {
            Self::Empty(candidate) => candidate.after_bytes(),
            Self::PopulatedTail(candidate) => candidate.after_bytes(),
        }
    }

    pub(super) fn table(&self) -> &str {
        match self {
            Self::Empty(candidate) => candidate.table(),
            Self::PopulatedTail(candidate) => candidate.table(),
        }
    }

    pub(super) fn column(&self) -> &str {
        match self {
            Self::Empty(candidate) => candidate.column(),
            Self::PopulatedTail(candidate) => candidate.column(),
        }
    }

    pub(super) fn identity(&self) -> NativeFieldDropIdentity {
        match self {
            Self::Empty(candidate) => candidate.identity(),
            Self::PopulatedTail(candidate) => candidate.identity(),
        }
    }

    pub(super) fn profile(&self) -> Profile {
        match self {
            Self::Empty(_) => Profile::Empty,
            Self::PopulatedTail(_) => Profile::PopulatedTail,
        }
    }
}
